mod media;
mod paths;
mod protocol;
mod registration;
mod request;

use std::fs;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc as std_mpsc;
use std::sync::Arc;
use std::thread;

use anyhow::{Context, Result};
use chrono::{Local, Timelike};
use khukri_engine::{
    db, spawn_download, DownloadConfig, DownloadProgress, DownloadStatus, ThrottleConfig,
};
use media::{
    configured_browser_session, configured_settings, should_use_ytdlp, MediaQuality, YtDlpJob,
};
use paths::{app_data_dir, downloads_dir, sqlite_url};
use protocol::{read_message, write_message, BridgeEvent, Incoming};
use request::{browser_headers, filename_from_url, sanitize_filename};
use sqlx::sqlite::SqlitePoolOptions;
use tokio::sync::{mpsc, watch, Semaphore};
use tokio::task::JoinSet;
use tokio::time::{sleep, timeout, Duration};

const BRIDGE_CANCEL_POLL_INTERVAL: Duration = Duration::from_millis(500);
const BRIDGE_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);

fn scheduler_window_open(settings: &media::BridgeSettings) -> bool {
    if !settings.scheduler_enabled {
        return true;
    }

    let hour = Local::now().hour() as u8;
    if settings.scheduler_start_hour <= settings.scheduler_end_hour {
        (settings.scheduler_start_hour..=settings.scheduler_end_hour).contains(&hour)
    } else {
        hour >= settings.scheduler_start_hour || hour <= settings.scheduler_end_hour
    }
}

fn status_label(status: DownloadStatus) -> &'static str {
    match status {
        DownloadStatus::Queued => "queued",
        DownloadStatus::Active => "active",
        DownloadStatus::Paused => "paused",
        DownloadStatus::Complete => "complete",
        DownloadStatus::Failed => "failed",
    }
}

fn unix_now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn progress_event(
    progress: &DownloadProgress,
    source: Option<String>,
    output_path: Option<String>,
) -> BridgeEvent {
    BridgeEvent {
        kind: "progress",
        id: progress.id.clone(),
        status: status_label(progress.status),
        bytes_done: progress.bytes_done,
        total_bytes: progress.total_bytes,
        speed_bps: progress.speed_bps,
        eta_seconds: progress.eta_seconds,
        segments_done: progress.segments_done,
        segments_total: progress.segments_total,
        source,
        output_path,
        message: None,
    }
}

fn media_progress_event(
    id: &str,
    progress: &media::YtDlpProgress,
    source: Option<String>,
    output_path: Option<String>,
) -> BridgeEvent {
    BridgeEvent {
        kind: "progress",
        id: id.to_string(),
        status: "active",
        bytes_done: progress.bytes_done,
        total_bytes: progress.total_bytes,
        speed_bps: progress.speed_bps,
        eta_seconds: progress.eta_seconds,
        segments_done: 0,
        segments_total: None,
        source,
        output_path,
        message: Some(progress.phase.clone()),
    }
}

async fn make_pool() -> Result<sqlx::SqlitePool> {
    let data_dir = app_data_dir();
    fs::create_dir_all(&data_dir)?;
    let db_path = data_dir.join("state.db");
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&sqlite_url(&db_path))
        .await?;
    // Match the WAL + busy-timeout settings used by the Tauri app so that
    // concurrent access between the bridge and the desktop app doesn't cause
    // "database is locked" errors.
    sqlx::query("PRAGMA journal_mode = WAL")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA busy_timeout = 5000")
        .execute(&pool)
        .await?;
    db::run_migrations(&pool).await?;
    Ok(pool)
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(io::stderr)
        .with_max_level(tracing::Level::INFO)
        .init();

    let args: Vec<String> = std::env::args().collect();
    let exe_path = std::env::current_exe().context("failed to resolve bridge binary path")?;

    if registration::requested(&args) {
        registration::register(&exe_path)?;
        return Ok(());
    }

    let pool = make_pool().await?;
    let download_slots = Arc::new(Semaphore::new(configured_settings().max_concurrent));

    let (writer_tx, writer_rx) = std_mpsc::channel::<BridgeEvent>();
    let writer_thread = thread::spawn(move || {
        let mut stdout = io::stdout();
        while let Ok(event) = writer_rx.recv() {
            write_message(&mut stdout, &event)?;
        }
        Result::<()>::Ok(())
    });

    let (read_tx, mut read_rx) = mpsc::unbounded_channel::<Result<Incoming>>();
    let port_closed = Arc::new(AtomicBool::new(false));
    let port_closed_for_reader = port_closed.clone();
    thread::spawn(move || loop {
        let next = read_message();
        let should_stop = next.is_err();
        if should_stop {
            port_closed_for_reader.store(true, Ordering::SeqCst);
        }
        if read_tx.send(next).is_err() {
            break;
        }
        if should_stop {
            break;
        }
    });

    // Every job observes this signal. Native Messaging closes stdin when its
    // port disappears, so EOF becomes the bridge-wide graceful-pause signal.
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let mut download_tasks = JoinSet::new();

    'messages: while let Some(message) = read_rx.recv().await {
        let message = match message {
            Ok(message) => message,
            Err(err) => {
                let _ = writer_tx.send(BridgeEvent {
                    kind: "error",
                    id: "bridge".to_string(),
                    status: "failed",
                    bytes_done: 0,
                    total_bytes: None,
                    speed_bps: 0,
                    eta_seconds: None,
                    segments_done: 0,
                    segments_total: None,
                    source: None,
                    output_path: None,
                    message: Some(err.to_string()),
                });
                break;
            }
        };

        match message {
            Incoming::QueueDownload {
                url,
                filename,
                size: _size,
                quality,
                source,
                page_url,
                custom_headers,
            } => {
                // Hold one permit for the lifetime of the transfer. The bridge
                // is a separate process, so it must enforce the same basic
                // concurrency setting as the desktop queue.
                let mut bridge_settings = configured_settings();
                while !scheduler_window_open(&bridge_settings) {
                    if port_closed.load(Ordering::SeqCst) {
                        break 'messages;
                    }
                    sleep(Duration::from_secs(1)).await;
                    bridge_settings = configured_settings();
                }
                let permit = download_slots
                    .clone()
                    .acquire_owned()
                    .await
                    .map_err(|_| anyhow::anyhow!("download queue is shutting down"))?;
                let output_root = bridge_settings.download_root.unwrap_or_else(downloads_dir);
                fs::create_dir_all(&output_root)?;
                let resolved_name = filename
                    .as_deref()
                    .map(sanitize_filename)
                    .unwrap_or_else(|| filename_from_url(&url));
                let output_path = output_root.join(&resolved_name);
                let headers = browser_headers(page_url.as_deref(), custom_headers);

                if should_use_ytdlp(source.as_deref(), quality.as_deref()) {
                    let id = format!(
                        "media-{}",
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis()
                    );
                    let job = YtDlpJob {
                        id: id.clone(),
                        url,
                        output_path: output_path.clone(),
                        quality: MediaQuality::parse(quality.as_deref()),
                        headers,
                        browser_session: configured_browser_session(),
                        proxy_url: bridge_settings.proxy_url.clone(),
                        bandwidth_cap: bridge_settings.bandwidth_cap,
                        concurrent_fragments: bridge_settings.thread_override,
                    };
                    let source_clone = source.clone();
                    let quality_clone = quality.clone();
                    let path_display = output_path.display().to_string();
                    let tx = writer_tx.clone();
                    let pool_for_media = pool.clone();
                    let mut bridge_shutdown = shutdown_rx.clone();

                    db::upsert_download(
                        &pool,
                        &id,
                        &job.url,
                        &path_display,
                        None,
                        "normal",
                        None,
                        unix_now_secs(),
                    )
                    .await?;
                    db::set_download_request_metadata(
                        &pool,
                        &id,
                        quality.as_deref(),
                        source.as_deref(),
                    )
                    .await?;
                    db::set_download_status(&pool, &id, "active").await?;

                    let _ = writer_tx.send(BridgeEvent {
                        kind: "progress",
                        id: id.clone(),
                        status: "queued",
                        bytes_done: 0,
                        total_bytes: None,
                        speed_bps: 0,
                        eta_seconds: None,
                        segments_done: 0,
                        segments_total: None,
                        source: source.clone(),
                        output_path: Some(path_display.clone()),
                        message: Some("starting yt-dlp".to_string()),
                    });

                    download_tasks.spawn(async move {
                        let _permit = permit;
                        let (cancel_tx, cancel_rx) = watch::channel(false);
                        let cancel_pool = pool_for_media.clone();
                        let cancel_id = job.id.clone();
                        let cancel_task = tokio::spawn(async move {
                            loop {
                                tokio::select! {
                                    changed = bridge_shutdown.changed() => {
                                        if changed.is_err() || *bridge_shutdown.borrow() {
                                            // Persist pause before stopping yt-dlp so its error
                                            // path preserves resumable state rather than marking
                                            // the IPC disconnect as a download failure.
                                            let _ = db::set_download_status(
                                                &cancel_pool,
                                                &cancel_id,
                                                "paused",
                                            ).await;
                                            let _ = cancel_tx.send(true);
                                            break;
                                        }
                                    }
                                    _ = sleep(BRIDGE_CANCEL_POLL_INTERVAL) => {
                                        match db::get_download(&cancel_pool, &cancel_id).await {
                                            Ok(Some(row)) if row.status == "active" => {}
                                            _ => {
                                                let _ = cancel_tx.send(true);
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        });

                        // Chrome receives the native-messaging events below, but the desktop
                        // app is a separate process and hydrates its queue from SQLite. Keep a
                        // lightweight writer here so both surfaces observe the same progress.
                        let (progress_db_tx, mut progress_db_rx) =
                            mpsc::unbounded_channel::<media::YtDlpProgress>();
                        let progress_pool = pool_for_media.clone();
                        let progress_id = job.id.clone();
                        let progress_writer = tokio::spawn(async move {
                            while let Some(mut progress) = progress_db_rx.recv().await {
                                while let Ok(newer) = progress_db_rx.try_recv() {
                                    progress = newer;
                                }
                                let _ = db::set_download_progress(
                                    &progress_pool,
                                    &progress_id,
                                    progress.bytes_done,
                                    progress.total_bytes,
                                    progress.speed_bps,
                                    progress.eta_seconds,
                                )
                                .await;
                                sleep(Duration::from_millis(200)).await;
                            }
                        });

                        let mut on_progress = |progress| {
                            let _ = tx.send(media_progress_event(
                                &job.id,
                                &progress,
                                source_clone.clone(),
                                Some(path_display.clone()),
                            ));
                            let _ = progress_db_tx.send(progress);
                        };

                        let result =
                            media::run_ytdlp_with_cancel(job.clone(), &mut on_progress, cancel_rx)
                                .await;
                        drop(progress_db_tx);
                        let _ = progress_writer.await;
                        cancel_task.abort();

                        match result {
                            Ok(outcome) => {
                                let final_size = std::fs::metadata(&outcome.final_path)
                                    .map(|m| m.len())
                                    .unwrap_or(0);
                                if final_size > 0 {
                                    let _ = db::set_download_progress(
                                        &pool_for_media,
                                        &job.id,
                                        final_size,
                                        Some(final_size),
                                        0,
                                        None,
                                    )
                                    .await;
                                }

                                let _ = db::set_download_file_path(
                                    &pool_for_media,
                                    &job.id,
                                    &outcome.final_path.display().to_string(),
                                )
                                .await;
                                let _ = db::set_download_request_metadata(
                                    &pool_for_media,
                                    &job.id,
                                    quality_clone.as_deref(),
                                    source_clone.as_deref(),
                                )
                                .await;
                                let _ =
                                    db::set_download_status(&pool_for_media, &job.id, "complete")
                                        .await;
                                let _ = tx.send(BridgeEvent {
                                    kind: "progress",
                                    id: job.id.clone(),
                                    status: "complete",
                                    bytes_done: final_size,
                                    total_bytes: if final_size > 0 {
                                        Some(final_size)
                                    } else {
                                        None
                                    },
                                    speed_bps: 0,
                                    eta_seconds: None,
                                    segments_done: 0,
                                    segments_total: None,
                                    source: source.clone(),
                                    output_path: Some(outcome.final_path.display().to_string()),
                                    message: Some("yt-dlp complete".to_string()),
                                });
                            }
                            Err(err) => {
                                let current_status = db::get_download(&pool_for_media, &job.id)
                                    .await
                                    .ok()
                                    .flatten()
                                    .map(|row| row.status);
                                let preserved_status = match current_status.as_deref() {
                                    Some("paused") => Some("paused"),
                                    Some("cancelled") | None => Some("cancelled"),
                                    Some("complete") => Some("complete"),
                                    _ => None,
                                };
                                if let Some(status) = preserved_status {
                                    let _ = tx.send(BridgeEvent {
                                        kind: "progress",
                                        id: job.id.clone(),
                                        status,
                                        bytes_done: 0,
                                        total_bytes: None,
                                        speed_bps: 0,
                                        eta_seconds: None,
                                        segments_done: 0,
                                        segments_total: None,
                                        source: source.clone(),
                                        output_path: Some(path_display.clone()),
                                        message: Some(err.to_string()),
                                    });
                                    return;
                                }
                                let _ = db::set_download_request_metadata(
                                    &pool_for_media,
                                    &job.id,
                                    quality_clone.as_deref(),
                                    source_clone.as_deref(),
                                )
                                .await;
                                let _ = db::set_download_failed(
                                    &pool_for_media,
                                    &job.id,
                                    &err.to_string(),
                                )
                                .await;
                                let _ = tx.send(BridgeEvent {
                                    kind: "error",
                                    id: job.id.clone(),
                                    status: "failed",
                                    bytes_done: 0,
                                    total_bytes: None,
                                    speed_bps: 0,
                                    eta_seconds: None,
                                    segments_done: 0,
                                    segments_total: None,
                                    source: source.clone(),
                                    output_path: Some(path_display.clone()),
                                    message: Some(err.to_string()),
                                });
                            }
                        }
                    });
                    continue;
                }

                let mut config = DownloadConfig::new(&url, &output_path);
                config.allowed_root = Some(output_root.clone());
                config.custom_headers = headers;
                config.proxy_url = bridge_settings.proxy_url;
                config.throttle = ThrottleConfig {
                    bytes_per_sec: bridge_settings.bandwidth_cap,
                };
                config.override_threads = bridge_settings.thread_override;

                let handle = spawn_download(config, pool.clone());
                let cancel = handle.cancellation_token();
                let mut rx = handle.subscribe();
                let source_clone = source.clone();
                let path_display = output_path.display().to_string();
                let path_for_progress = path_display.clone();
                let path_for_wait = path_display.clone();

                let initial = rx.borrow().clone();
                let _ = writer_tx.send(progress_event(
                    &initial,
                    source_clone.clone(),
                    Some(path_display.clone()),
                ));

                let tx = writer_tx.clone();
                download_tasks.spawn(async move {
                    while rx.changed().await.is_ok() {
                        let snapshot = rx.borrow().clone();
                        let _ = tx.send(progress_event(
                            &snapshot,
                            source_clone.clone(),
                            Some(path_for_progress.clone()),
                        ));
                        if matches!(
                            snapshot.status,
                            DownloadStatus::Complete
                                | DownloadStatus::Failed
                                | DownloadStatus::Paused
                        ) {
                            break;
                        }
                    }
                });

                let tx = writer_tx.clone();
                let mut bridge_shutdown = shutdown_rx.clone();
                download_tasks.spawn(async move {
                    let _permit = permit;
                    let cancellation_task = tokio::spawn(async move {
                        if bridge_shutdown.changed().await.is_err() || *bridge_shutdown.borrow() {
                            cancel.cancel();
                        }
                    });
                    match handle.wait().await {
                        Ok(()) => {}
                        Err(err) => {
                            let _ = tx.send(BridgeEvent {
                                kind: "error",
                                id: "bridge-download".to_string(),
                                status: "failed",
                                bytes_done: 0,
                                total_bytes: None,
                                speed_bps: 0,
                                eta_seconds: None,
                                segments_done: 0,
                                segments_total: None,
                                source: source.clone(),
                                output_path: Some(path_for_wait),
                                message: Some(err.to_string()),
                            });
                        }
                    }
                    cancellation_task.abort();
                });
            }
        }
    }

    // Do not let the Tokio runtime tear active jobs down before they have
    // persisted their paused state. Bound the grace period so a stuck network
    // operation cannot keep the native host alive indefinitely.
    let _ = shutdown_tx.send(true);
    if timeout(BRIDGE_SHUTDOWN_TIMEOUT, async {
        while let Some(result) = download_tasks.join_next().await {
            if let Err(err) = result {
                tracing::warn!(error = %err, "bridge download task failed during shutdown");
            }
        }
    })
    .await
    .is_err()
    {
        tracing::warn!("timed out waiting for downloads to pause; aborting remaining tasks");
        download_tasks.abort_all();
        while download_tasks.join_next().await.is_some() {}
    }

    drop(writer_tx);
    writer_thread
        .join()
        .map_err(|_| anyhow::anyhow!("writer thread panicked"))??;
    Ok(())
}
