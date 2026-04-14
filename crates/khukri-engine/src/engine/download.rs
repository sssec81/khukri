use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use futures::StreamExt;
use reqwest::Client;
use sqlx::SqlitePool;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::sync::Mutex;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::config::DownloadConfig;
use crate::db;
use crate::engine::prealloc::preallocate;
use crate::engine::retry::{is_permanent_failure, with_retry};
use crate::engine::segment::{build_segments, calc_thread_count};
use crate::engine::throttle::TokenBucket;
use crate::error::{KhukriError, Result};

type Bucket = Arc<Mutex<TokenBucket>>;

/// Entry point for a single download.
pub async fn start_download(config: DownloadConfig, pool: SqlitePool) -> Result<()> {
    let client = Arc::new(Client::builder().build()?);
    let download_id = Uuid::new_v4().to_string();

    // ── 1. HEAD: probe Content-Length + Accept-Ranges (with retry) ──────────
    // Status check is inside the closure so 5xx triggers a retry,
    // while permanent errors (403, 404) surface immediately.
    let head = with_retry(&config.retry, || {
        let client = client.clone();
        let url = config.url.clone();
        async move {
            let resp = client.head(&url).send().await.map_err(KhukriError::Http)?;
            let s = resp.status().as_u16();
            if is_permanent_failure(s) {
                return Err(KhukriError::PermanentError { status: s, url });
            }
            if s >= 500 {
                // Transient server error — retryable.
                return Err(KhukriError::Http(resp.error_for_status().unwrap_err()));
            }
            Ok(resp)
        }
    })
    .await?;

    let total_bytes = head
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());

    let accepts_ranges = head
        .headers()
        .get("accept-ranges")
        .map(|v| v.to_str().unwrap_or("") == "bytes")
        .unwrap_or(false);

    info!(
        id = %download_id,
        url = %config.url,
        total_bytes = ?total_bytes,
        accepts_ranges,
        priority = ?config.priority,
        "Starting download"
    );

    // ── 2. Persist download record ────────────────────────────────────────────
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    db::insert_download(
        &pool,
        &download_id,
        &config.url,
        config.file_path.to_str().unwrap_or(""),
        total_bytes,
        config.priority.as_str(),
        now,
    )
    .await?;

    db::set_download_status(&pool, &download_id, "active").await?;

    // ── 3. Prepare output directory ───────────────────────────────────────────
    if let Some(parent) = config.file_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    // ── 4. Route: segmented vs streaming ─────────────────────────────────────
    let bucket: Option<Bucket> = config
        .throttle
        .bytes_per_sec
        .filter(|&bps| bps > 0)
        .map(|bps| Arc::new(Mutex::new(TokenBucket::new(bps))));

    match total_bytes {
        Some(size) if accepts_ranges => {
            segmented_download(&config, &pool, &client, &download_id, size, bucket).await?;
        }
        Some(size) => {
            warn!("Server does not support range requests — single-thread fallback");
            streaming_download(&config, &client, &download_id, &pool, Some(size), bucket).await?;
        }
        None => {
            warn!("No Content-Length — streaming download (no segmenting or resume)");
            streaming_download(&config, &client, &download_id, &pool, None, bucket).await?;
        }
    }

    db::set_download_status(&pool, &download_id, "complete").await?;
    info!(id = %download_id, path = ?config.file_path, "Download complete");
    Ok(())
}

// ── Segmented path ────────────────────────────────────────────────────────────

async fn segmented_download(
    config: &DownloadConfig,
    pool: &SqlitePool,
    client: &Arc<Client>,
    download_id: &str,
    total_bytes: u64,
    bucket: Option<Bucket>,
) -> Result<()> {
    let thread_count = config
        .override_threads
        .unwrap_or_else(|| calc_thread_count(total_bytes));

    info!(thread_count, total_bytes, "Segmented download");

    let segments = build_segments(total_bytes, thread_count);
    let seg_pairs: Vec<(u64, u64)> =
        segments.iter().map(|s| (s.start_byte, s.end_byte)).collect();
    db::insert_segments(pool, download_id, &seg_pairs).await?;

    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&config.file_path)
        .await?;
    preallocate(&file, total_bytes).await?;
    drop(file);

    let incomplete = db::get_incomplete_segments(pool, download_id).await?;
    info!("{}/{} segment(s) remaining", incomplete.len(), segments.len());

    let mut handles = Vec::with_capacity(incomplete.len());

    for seg_row in incomplete {
        let client = client.clone();
        let pool = pool.clone();
        let url = config.url.clone();
        let file_path = config.file_path.clone();
        let retry_cfg = config.retry.clone();
        let bucket = bucket.clone();

        handles.push(tokio::spawn(async move {
            let seg_id = seg_row.id;
            let start = seg_row.start_byte as u64;
            let end = seg_row.end_byte as u64;

            let result = with_retry(&retry_cfg, || {
                let client = client.clone();
                let url = url.clone();
                let file_path = file_path.clone();
                let bucket = bucket.clone();
                async move { fetch_segment(&client, &url, &file_path, start, end, bucket).await }
            })
            .await;

            match result {
                Ok(()) => {
                    db::mark_segment_complete(&pool, seg_id).await?;
                    Ok::<(), KhukriError>(())
                }
                Err(e) => {
                    error!("Segment [{start}-{end}] failed: {e}");
                    Err(e)
                }
            }
        }));
    }

    let mut failed = false;
    for handle in handles {
        match handle.await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                error!("Segment error: {e}");
                failed = true;
            }
            Err(e) => {
                error!("Segment task panicked: {e}");
                failed = true;
            }
        }
    }

    if failed {
        db::set_download_status(pool, download_id, "failed").await?;
        return Err(KhukriError::MaxRetriesExceeded {
            attempts: config.retry.max_retries,
        });
    }

    Ok(())
}

// ── Streaming path ────────────────────────────────────────────────────────────

async fn streaming_download(
    config: &DownloadConfig,
    client: &Client,
    download_id: &str,
    pool: &SqlitePool,
    known_size: Option<u64>,
    bucket: Option<Bucket>,
) -> Result<()> {
    // Status check is inside the closure so 5xx is retried.
    let response = with_retry(&config.retry, || {
        let client = client.clone();
        let url = config.url.clone();
        async move {
            let resp = client.get(&url).send().await.map_err(KhukriError::Http)?;
            let s = resp.status().as_u16();
            if is_permanent_failure(s) {
                return Err(KhukriError::PermanentError { status: s, url });
            }
            if s >= 500 {
                return Err(KhukriError::Http(resp.error_for_status().unwrap_err()));
            }
            Ok(resp)
        }
    })
    .await?;

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&config.file_path)
        .await?;

    if let Some(size) = known_size {
        preallocate(&file, size).await?;
    }

    let mut stream = response.bytes_stream();
    let mut written: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let bytes = chunk?;
        if let Some(ref b) = bucket {
            b.lock().await.consume(bytes.len() as u64).await;
        }
        file.write_all(&bytes).await?;
        written += bytes.len() as u64;
    }

    file.flush().await?;

    // Only record a segment if bytes were actually written.
    // written = 0 (empty file) must not insert a bogus (0, 0) row.
    if written > 0 {
        db::insert_segments(pool, download_id, &[(0, written - 1)]).await?;
        let segs = db::get_incomplete_segments(pool, download_id).await?;
        if let Some(seg) = segs.first() {
            db::mark_segment_complete(pool, seg.id).await?;
        }
    }

    info!(written_bytes = written, "Streaming download complete");
    Ok(())
}

// ── Segment fetch ─────────────────────────────────────────────────────────────

async fn fetch_segment(
    client: &Client,
    url: &str,
    file_path: &std::path::Path,
    start: u64,
    end: u64,
    bucket: Option<Bucket>,
) -> Result<()> {
    let response = client
        .get(url)
        .header("Range", format!("bytes={start}-{end}"))
        .send()
        .await?;

    let status = response.status().as_u16();

    if is_permanent_failure(status) {
        return Err(KhukriError::PermanentError { status, url: url.to_string() });
    }

    // Must be 206 Partial Content.
    // - 200: server ignored our Range header and returned the full file; writing
    //        at `start` would corrupt output — classify as NoRangeSupport.
    // - 3xx: reqwest follows redirects automatically (up to 10); seeing a 3xx here
    //        means the redirect limit was exhausted — treat as NoRangeSupport since
    //        we cannot follow further and writing at offset would be wrong.
    // - 4xx/5xx (not already caught): surface as Http error so with_retry can decide.
    if status != 206 {
        return Err(if status >= 400 {
            KhukriError::Http(response.error_for_status().unwrap_err())
        } else {
            KhukriError::NoRangeSupport
        });
    }

    let mut file = OpenOptions::new().write(true).open(file_path).await?;
    file.seek(std::io::SeekFrom::Start(start)).await?;

    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let bytes = chunk?;
        if let Some(ref b) = bucket {
            b.lock().await.consume(bytes.len() as u64).await;
        }
        file.write_all(&bytes).await?;
    }

    file.flush().await?;
    Ok(())
}
