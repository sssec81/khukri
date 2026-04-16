use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc as std_mpsc;
use std::thread;

use anyhow::{Context, Result};
use khukri_engine::{db, spawn_download, DownloadConfig, DownloadProgress, DownloadStatus};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePoolOptions;
use tokio::sync::mpsc;

const HOST_ID: &str = "com.khukri.host";

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum Incoming {
    #[serde(rename = "queue_download")]
    QueueDownload {
        url: String,
        filename: Option<String>,
        size: Option<u64>,
        quality: Option<String>,
        source: Option<String>,
        #[serde(rename = "pageUrl")]
        page_url: Option<String>,
        #[serde(rename = "customHeaders", default)]
        custom_headers: HashMap<String, String>,
    },
}

#[derive(Debug, Serialize)]
struct BridgeEvent {
    #[serde(rename = "type")]
    kind: &'static str,
    id: String,
    status: &'static str,
    bytes_done: u64,
    total_bytes: Option<u64>,
    speed_bps: u64,
    eta_seconds: Option<u64>,
    segments_done: u32,
    segments_total: Option<u32>,
    source: Option<String>,
    output_path: Option<String>,
    message: Option<String>,
}

#[derive(Debug, Serialize)]
struct HostManifest {
    name: String,
    description: String,
    path: String,
    #[serde(rename = "type")]
    host_type: String,
    allowed_origins: Vec<String>,
}

fn read_message() -> Result<Incoming> {
    let mut len_buf = [0u8; 4];
    io::stdin().read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    io::stdin().read_exact(&mut buf)?;
    Ok(serde_json::from_slice(&buf)?)
}

fn write_message<T: Serialize>(writer: &mut impl Write, msg: &T) -> Result<()> {
    let body = serde_json::to_vec(msg)?;
    let len = (body.len() as u32).to_le_bytes();
    writer.write_all(&len)?;
    writer.write_all(&body)?;
    writer.flush()?;
    Ok(())
}

fn downloads_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Some(profile) = std::env::var_os("USERPROFILE") {
            return PathBuf::from(profile).join("Downloads");
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join("Downloads");
        }
    }

    std::env::temp_dir().join("khukri-downloads")
}

fn app_data_dir() -> PathBuf {
    if let Some(explicit) = std::env::var_os("KHUKRI_DATA_DIR") {
        return PathBuf::from(explicit);
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
            return PathBuf::from(local_app_data).join("Khukri");
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Some(data_home) = std::env::var_os("XDG_DATA_HOME") {
            return PathBuf::from(data_home).join("khukri");
        }
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(".local").join("share").join("khukri");
        }
    }

    std::env::temp_dir().join("khukri-data")
}

fn sqlite_url(path: &Path) -> String {
    format!("sqlite:{}?mode=rwc", path.display())
}

fn sanitize_filename(name: &str) -> String {
    let file_name = Path::new(name)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("download.bin");
    let sanitized: String = file_name
        .chars()
        .map(|ch| match ch {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            _ => ch,
        })
        .collect();
    if sanitized.trim().is_empty() {
        "download.bin".to_string()
    } else {
        sanitized
    }
}

fn filename_from_url(url: &str) -> String {
    let trimmed = url.split('?').next().unwrap_or(url).trim_end_matches('/');
    let candidate = trimmed.rsplit('/').next().unwrap_or("download.bin");
    sanitize_filename(candidate)
}

fn browser_headers(
    page_url: Option<&str>,
    custom_headers: HashMap<String, String>,
) -> Vec<(String, String)> {
    let mut headers: Vec<(String, String)> = custom_headers.into_iter().collect();

    if let Some(page_url) = page_url {
        if !headers
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case("Referer"))
        {
            headers.push(("Referer".to_string(), page_url.to_string()));
        }
    }

    headers
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

async fn make_pool() -> Result<sqlx::SqlitePool> {
    let data_dir = app_data_dir();
    fs::create_dir_all(&data_dir)?;
    let db_path = data_dir.join("khukri.db");
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&sqlite_url(&db_path))
        .await?;
    db::run_migrations(&pool).await?;
    Ok(pool)
}

fn extension_origin_from_env() -> String {
    std::env::var("KHUKRI_EXTENSION_ORIGIN")
        .unwrap_or_else(|_| "chrome-extension://replace-with-your-extension-id/".to_string())
}

fn native_host_manifest(binary_path: &Path) -> HostManifest {
    HostManifest {
        name: HOST_ID.to_string(),
        description: "Khukri Native Messaging Host".to_string(),
        path: binary_path.display().to_string(),
        host_type: "stdio".to_string(),
        allowed_origins: vec![extension_origin_from_env()],
    }
}

#[cfg(target_os = "windows")]
fn register_native_host(binary_path: &Path) -> Result<()> {
    let bridge_dir = binary_path
        .parent()
        .context("bridge binary has no parent directory")?;
    let manifest_path = bridge_dir.join(format!("{HOST_ID}.json"));
    let manifest = serde_json::to_vec_pretty(&native_host_manifest(binary_path))?;
    fs::write(&manifest_path, manifest)?;

    let reg_key = format!(r"HKCU\Software\Google\Chrome\NativeMessagingHosts\{HOST_ID}");
    let status = std::process::Command::new("reg")
        .args([
            "add",
            &reg_key,
            "/ve",
            "/t",
            "REG_SZ",
            "/d",
            &manifest_path.display().to_string(),
            "/f",
        ])
        .status()
        .context("failed to launch reg.exe")?;

    if !status.success() {
        anyhow::bail!("failed to register native host in Windows registry");
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn register_native_host(binary_path: &Path) -> Result<()> {
    let config_dir = std::env::var_os("HOME")
        .map(PathBuf::from)
        .context("HOME is not set")?
        .join(".config")
        .join("google-chrome")
        .join("NativeMessagingHosts");
    fs::create_dir_all(&config_dir)?;
    let manifest_path = config_dir.join(format!("{HOST_ID}.json"));
    let manifest = serde_json::to_vec_pretty(&native_host_manifest(binary_path))?;
    fs::write(&manifest_path, manifest)?;
    Ok(())
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
fn register_native_host(_binary_path: &Path) -> Result<()> {
    anyhow::bail!("native host registration is only implemented for Windows and Linux")
}

fn should_register(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--register" || arg == "--repair")
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(io::stderr)
        .with_max_level(tracing::Level::INFO)
        .init();

    let args: Vec<String> = std::env::args().collect();
    let exe_path = std::env::current_exe().context("failed to resolve bridge binary path")?;

    if should_register(&args) {
        register_native_host(&exe_path)?;
        return Ok(());
    }

    let pool = make_pool().await?;
    let output_root = downloads_dir();
    fs::create_dir_all(&output_root)?;

    let (writer_tx, writer_rx) = std_mpsc::channel::<BridgeEvent>();
    let writer_thread = thread::spawn(move || {
        let mut stdout = io::stdout();
        while let Ok(event) = writer_rx.recv() {
            write_message(&mut stdout, &event)?;
        }
        Result::<()>::Ok(())
    });

    let (read_tx, mut read_rx) = mpsc::unbounded_channel::<Result<Incoming>>();
    thread::spawn(move || {
        loop {
            let next = read_message();
            let should_stop = next.is_err();
            if read_tx.send(next).is_err() {
                break;
            }
            if should_stop {
                break;
            }
        }
    });

    while let Some(message) = read_rx.recv().await {
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
                quality: _quality,
                source,
                page_url,
                custom_headers,
            } => {
                let resolved_name = filename
                    .as_deref()
                    .map(sanitize_filename)
                    .unwrap_or_else(|| filename_from_url(&url));
                let output_path = output_root.join(&resolved_name);
                let mut config = DownloadConfig::new(&url, &output_path);
                config.allowed_root = Some(output_root.clone());
                config.custom_headers = browser_headers(page_url.as_deref(), custom_headers);

                let handle = spawn_download(config, pool.clone());
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
                tokio::spawn(async move {
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
                tokio::spawn(async move {
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
                });
            }
        }
    }

    drop(writer_tx);
    writer_thread.join().map_err(|_| anyhow::anyhow!("writer thread panicked"))??;
    Ok(())
}
