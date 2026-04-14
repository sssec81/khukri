/// Integration tests for khukri-engine.
///
/// Spins up a local axum HTTP server for each test — no external network required.
/// Tests cover: segmented download, streaming fallback, retry on 5xx, permanent errors.
use std::net::SocketAddr;
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::get,
    Router,
};
use sha2::{Digest, Sha256};
use sqlx::sqlite::SqlitePoolOptions;
use tokio::net::TcpListener;
use tokio::time::{timeout, Duration};

use khukri_engine::{
    config::{DownloadConfig, Priority, RetryConfig, ThrottleConfig},
    db, spawn_download, start_download, DownloadStatus,
};

// ── Helpers ───────────────────────────────────────────────────────────────────

/// 2 MB of deterministic pseudo-random test data.
fn test_data() -> Arc<Vec<u8>> {
    Arc::new(
        (0u32..2 * 1024 * 1024)
            .map(|i| i.wrapping_mul(6_700_417) as u8)
            .collect(),
    )
}

fn sha256(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

/// Spawn a local HTTP server on a random port. Returns the bound address.
async fn spawn_server(router: Router) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.ok();
    });
    addr
}

/// Create a fresh SQLite pool for one test (clean slate every time).
async fn make_pool(tag: &str) -> sqlx::SqlitePool {
    let path = std::env::temp_dir().join(format!("khukri_it_{tag}.db"));
    let _ = std::fs::remove_file(&path);
    let pool = SqlitePoolOptions::new()
        .connect(&format!("sqlite:{}?mode=rwc", path.display()))
        .await
        .unwrap();
    db::run_migrations(&pool).await.unwrap();
    pool
}

/// Build a DownloadConfig suitable for integration tests:
/// fast retries (1ms base delay), 4 threads, no throttle.
fn cfg(url: impl Into<String>, out: impl Into<std::path::PathBuf>) -> DownloadConfig {
    DownloadConfig {
        url: url.into(),
        file_path: out.into(),
        override_threads: Some(4),
        retry: RetryConfig { max_retries: 3, base_delay_ms: 1 },
        priority: Priority::Normal,
        throttle: ThrottleConfig { bytes_per_sec: None },
    }
}

fn tmp(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(name)
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// Serves data with full Range-request support (returns 206 for Range, 200 for HEAD/full GET).
async fn range_handler(headers: HeaderMap, State(data): State<Arc<Vec<u8>>>) -> Response<axum::body::Body> {
    let total = data.len() as u64;

    if let Some(rh) = headers.get("range") {
        if let Some((start, end)) = parse_range(rh.to_str().unwrap_or(""), total) {
            let body = data[start as usize..=end as usize].to_vec();
            return Response::builder()
                .status(206)
                .header("Content-Range", format!("bytes {start}-{end}/{total}"))
                .header("Content-Length", (end - start + 1).to_string())
                .header("Accept-Ranges", "bytes")
                .body(axum::body::Body::from(body))
                .unwrap();
        }
        return Response::builder()
            .status(416) // Range Not Satisfiable
            .body(axum::body::Body::empty())
            .unwrap();
    }

    // HEAD or full GET
    Response::builder()
        .status(200)
        .header("Content-Length", total.to_string())
        .header("Accept-Ranges", "bytes")
        .body(axum::body::Body::from(data.as_ref().clone()))
        .unwrap()
}

fn parse_range(s: &str, total: u64) -> Option<(u64, u64)> {
    let s = s.strip_prefix("bytes=")?;
    let mut it = s.split('-');
    let start: u64 = it.next()?.parse().ok()?;
    let end_str = it.next()?;
    let end = if end_str.is_empty() { total - 1 } else { end_str.parse().ok()? };
    (start <= end && end < total).then_some((start, end))
}

/// HEAD always succeeds; GET requests return 503 for the first `n` calls, then succeed.
#[derive(Clone)]
struct FlakyState {
    data: Arc<Vec<u8>>,
    remaining_fails: Arc<AtomicU32>,
}

#[derive(Clone)]
struct CountedRangeState {
    data: Arc<Vec<u8>>,
    range_calls: Arc<AtomicU32>,
}

async fn flaky_handler(
    headers: HeaderMap,
    axum::extract::OriginalUri(uri): axum::extract::OriginalUri,
    State(s): State<FlakyState>,
) -> Response<axum::body::Body> {
    // HEAD always succeeds so the engine can probe Content-Length / Accept-Ranges.
    if headers.get("range").is_none() && !uri.path().contains("GET") {
        // Treat no-Range as HEAD-equivalent probe — let it through.
    }

    if headers.get("range").is_some()
        && s.remaining_fails
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                if n > 0 { Some(n - 1) } else { None }
            })
            .is_ok()
    {
        // Fail this segment request.
        return Response::builder()
            .status(503)
            .body(axum::body::Body::empty())
            .unwrap();
    }

    range_handler(headers, State(s.data)).await
}

async fn counted_range_handler(
    headers: HeaderMap,
    State(s): State<CountedRangeState>,
) -> Response<axum::body::Body> {
    if headers.get("range").is_some() {
        s.range_calls.fetch_add(1, Ordering::SeqCst);
    }
    range_handler(headers, State(s.data)).await
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// 1. Segmented download — verifies the output matches the source via SHA-256.
#[tokio::test]
async fn test_segmented_download_sha256() {
    let data = test_data();
    let expected = sha256(&data);

    let addr = spawn_server(
        Router::new()
            .route("/file", get(range_handler))
            .with_state(data.clone()),
    )
    .await;

    let out = tmp("khukri_it_seg.bin");
    let _ = std::fs::remove_file(&out);

    start_download(cfg(format!("http://{addr}/file"), &out), make_pool("seg").await)
        .await
        .expect("segmented download failed");

    let got = std::fs::read(&out).expect("output file missing");
    assert_eq!(got.len(), data.len(), "file size mismatch");
    assert_eq!(sha256(&got), expected, "SHA-256 mismatch — data corruption");
}

/// 2. Streaming fallback — server sends no Content-Length header.
///    Engine must fall back to single-thread streaming and produce correct output.
#[tokio::test]
async fn test_streaming_fallback_no_content_length() {
    let data = test_data();
    let d = data.clone();

    let addr = spawn_server(Router::new().route(
        "/stream",
        get(move || {
            let d = d.clone();
            async move {
                Response::builder()
                    .status(200)
                    // Deliberately no Content-Length, no Accept-Ranges
                    .body(axum::body::Body::from(d.as_ref().clone()))
                    .unwrap()
            }
        }),
    ))
    .await;

    let out = tmp("khukri_it_stream.bin");
    let _ = std::fs::remove_file(&out);

    start_download(
        cfg(format!("http://{addr}/stream"), &out),
        make_pool("stream").await,
    )
    .await
    .expect("streaming download failed");

    let got = std::fs::read(&out).unwrap();
    assert_eq!(got.len(), data.len());
    assert_eq!(sha256(&got), sha256(&data));
}

/// 3. Retry on transient 5xx — segment requests fail twice then succeed.
///    Download must complete correctly despite initial failures.
#[tokio::test]
async fn test_retry_on_transient_5xx() {
    let data = test_data();
    let state = FlakyState {
        data: data.clone(),
        remaining_fails: Arc::new(AtomicU32::new(2)),
    };

    let addr = spawn_server(
        Router::new()
            .route("/file", get(flaky_handler))
            .with_state(state),
    )
    .await;

    let out = tmp("khukri_it_retry.bin");
    let _ = std::fs::remove_file(&out);

    start_download(cfg(format!("http://{addr}/file"), &out), make_pool("retry").await)
        .await
        .expect("download should succeed after retries");

    let got = std::fs::read(&out).unwrap();
    assert_eq!(got.len(), data.len());
    assert_eq!(sha256(&got), sha256(&data));
}

/// 4. Permanent 403 — HEAD returns 403, engine must fail immediately without retrying.
#[tokio::test]
async fn test_permanent_403_not_retried() {
    let call_count = Arc::new(AtomicU32::new(0));
    let cc = call_count.clone();

    let addr = spawn_server(Router::new().route(
        "/forbidden",
        get(move || {
            let cc = cc.clone();
            async move {
                cc.fetch_add(1, Ordering::SeqCst);
                (StatusCode::FORBIDDEN, "forbidden")
            }
        }),
    ))
    .await;

    let out = tmp("khukri_it_403.bin");
    let result = start_download(
        cfg(format!("http://{addr}/forbidden"), &out),
        make_pool("perm403").await,
    )
    .await;

    assert!(result.is_err(), "expected error on 403");
    // HEAD is the only request — must not retry.
    assert_eq!(
        call_count.load(Ordering::SeqCst),
        1,
        "403 must not trigger retries"
    );
}

/// 5. Resume flow — mark one segment incomplete and verify only that segment is fetched.
#[tokio::test]
async fn test_resume_download_fetches_only_incomplete_segments() {
    let data = test_data();
    let range_calls = Arc::new(AtomicU32::new(0));

    let addr = spawn_server(
        Router::new()
            .route("/file", get(counted_range_handler))
            .with_state(CountedRangeState {
                data: data.clone(),
                range_calls: range_calls.clone(),
            }),
    )
    .await;

    let out = tmp("khukri_it_resume.bin");
    let _ = std::fs::remove_file(&out);
    let pool = make_pool("resume").await;
    let url = format!("http://{addr}/file");

    start_download(cfg(url.clone(), &out), pool.clone())
        .await
        .expect("initial download failed");

    range_calls.store(0, Ordering::SeqCst);

    let download_id: String = sqlx::query_scalar(
        "SELECT id FROM downloads WHERE url = ? AND file_path = ?",
    )
    .bind(&url)
    .bind(out.to_string_lossy().to_string())
    .fetch_one(&pool)
    .await
    .expect("download row not found");

    let segment_id: i64 = sqlx::query_scalar(
        "SELECT id FROM segments WHERE download_id = ? ORDER BY start_byte LIMIT 1",
    )
    .bind(&download_id)
    .fetch_one(&pool)
    .await
    .expect("segment row not found");

    sqlx::query("UPDATE segments SET completed = 0 WHERE id = ?")
        .bind(segment_id)
        .execute(&pool)
        .await
        .expect("failed to mark segment incomplete");

    start_download(cfg(url, &out), pool.clone())
        .await
        .expect("resume download failed");

    assert_eq!(
        range_calls.load(Ordering::SeqCst),
        1,
        "resume should fetch only one incomplete segment"
    );

    let got = std::fs::read(&out).expect("output file missing");
    assert_eq!(sha256(&got), sha256(&data));
}

/// 6. Progress API — spawned downloads expose real-time state and terminal status.
#[tokio::test]
async fn test_spawn_download_reports_progress() {
    let data = test_data();
    let addr = spawn_server(
        Router::new()
            .route("/file", get(range_handler))
            .with_state(data.clone()),
    )
    .await;

    let out = tmp("khukri_it_progress.bin");
    let _ = std::fs::remove_file(&out);

    let handle = spawn_download(
        cfg(format!("http://{addr}/file"), &out),
        make_pool("progress").await,
    );

    let mut rx = handle.subscribe();
    let mut saw_active = false;

    timeout(Duration::from_secs(20), async {
        loop {
            let snapshot = rx.borrow().clone();
            if snapshot.status == DownloadStatus::Active {
                saw_active = true;
            }
            if snapshot.status == DownloadStatus::Complete {
                break snapshot;
            }
            rx.changed().await.expect("progress channel closed unexpectedly");
        }
    })
    .await
    .expect("timed out waiting for completion");

    handle.wait().await.expect("spawned download failed");
    let final_state = rx.borrow().clone();

    assert!(saw_active, "expected at least one active progress state");
    assert_eq!(final_state.status, DownloadStatus::Complete);
    assert_eq!(final_state.bytes_done as usize, data.len());
    assert_eq!(final_state.total_bytes, Some(data.len() as u64));
}
