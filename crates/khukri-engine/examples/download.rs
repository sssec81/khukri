//! Quick CLI smoke-test for khukri-engine.
//!
//! Usage:
//!   cargo run --example download -- <url> <output-path> [speed-limit-bytes/s]
//!
//! Examples:
//!   # Basic download
//!   cargo run --example download -- https://speed.hetzner.de/100MB.bin /tmp/test.bin
//!
//!   # With 500 KB/s cap
//!   cargo run --example download -- https://speed.hetzner.de/100MB.bin /tmp/test.bin 512000

use std::path::PathBuf;
use std::time::Instant;

use sqlx::sqlite::SqlitePoolOptions;
use tokio_util::sync::CancellationToken;
use tracing::info;
use tracing_subscriber::EnvFilter;

use khukri_engine::{
    config::{DownloadConfig, Priority, RetryConfig, ThrottleConfig},
    db, start_download_with_cancel,
};

#[tokio::main]
async fn main() {
    // RUST_LOG=debug cargo run --example download -- ...
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("khukri_engine=info".parse().unwrap()),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: download <url> <output-path> [speed-limit-bytes/s]");
        eprintln!();
        eprintln!("Examples:");
        eprintln!(
            "  cargo run --example download -- https://speed.hetzner.de/100MB.bin /tmp/test.bin"
        );
        eprintln!("  cargo run --example download -- https://speed.hetzner.de/100MB.bin /tmp/test.bin 512000");
        std::process::exit(1);
    }

    let url = args[1].clone();
    let output = PathBuf::from(&args[2]);
    let speed_limit: Option<u64> = args.get(3).and_then(|s| s.parse().ok());

    if let Some(bps) = speed_limit {
        println!("Speed cap: {} KB/s", bps / 1024);
    }

    // SQLite state DB (created next to the output file, or cwd fallback).
    let db_path = output
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("khukri_state.db");

    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());
    info!("State DB: {db_url}");

    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect(&db_url)
        .await
        .expect("Cannot open SQLite DB");

    db::run_migrations(&pool).await.expect("Migration failed");

    let config = DownloadConfig {
        url: url.clone(),
        file_path: output.clone(),
        allowed_root: None,
        override_threads: None,
        retry: RetryConfig::default(),
        priority: Priority::Normal,
        throttle: ThrottleConfig {
            bytes_per_sec: speed_limit,
        },
        custom_headers: Vec::new(),
    };

    println!("Downloading: {url}");
    println!("         → {}", output.display());
    println!();

    let start = Instant::now();

    let cancel = CancellationToken::new();
    let download = start_download_with_cancel(config, pool, cancel.clone());
    tokio::pin!(download);

    let result = tokio::select! {
        res = &mut download => res,
        _ = tokio::signal::ctrl_c() => {
            eprintln!("Received Ctrl+C, cancelling download...");
            cancel.cancel();
            download.await
        }
    };

    match result {
        Ok(()) => {
            let elapsed = start.elapsed();
            let size = std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0);
            let mb = size as f64 / (1024.0 * 1024.0);
            let speed_mb = mb / elapsed.as_secs_f64();

            println!();
            println!("Done in {:.1}s", elapsed.as_secs_f64());
            println!("Size:  {:.2} MB", mb);
            println!("Speed: {:.2} MB/s", speed_mb);
        }
        Err(e) => {
            eprintln!("Download failed: {e}");
            std::process::exit(1);
        }
    }
}
