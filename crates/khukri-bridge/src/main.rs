mod media;

use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc as std_mpsc;
use std::thread;

use anyhow::{Context, Result};
use khukri_engine::{db, spawn_download, DownloadConfig, DownloadProgress, DownloadStatus};
use media::{should_use_ytdlp, MediaQuality, YtDlpJob};
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
            return PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("khukri");
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
    // Strip query string and fragment before extracting the filename.
    let trimmed = url
        .split('?')
        .next()
        .unwrap_or(url)
        .split('#')
        .next()
        .unwrap_or(url)
        .trim_end_matches('/');
    let path_part = match trimmed.split_once("://") {
        Some((_, remainder)) => match remainder.split_once('/') {
            Some((_, path)) => path,
            None => return "download.bin".to_string(),
        },
        None => trimmed,
    };
    if path_part.is_empty() {
        return "download.bin".to_string();
    }
    let candidate = path_part.rsplit('/').next().unwrap_or("download.bin");
    if candidate.is_empty() {
        return "download.bin".to_string();
    }
    sanitize_filename(candidate)
}

/// Headers that must never be forwarded from the browser extension.
/// These are hop-by-hop headers or headers that could cause request smuggling,
/// SSRF amplification, or credential leakage.
const BLOCKED_HEADERS: &[&str] = &[
    "host",
    "content-length",
    "transfer-encoding",
    "connection",
    "keep-alive",
    "upgrade",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "authorization",
];

fn browser_headers(
    page_url: Option<&str>,
    custom_headers: HashMap<String, String>,
) -> Vec<(String, String)> {
    let mut headers: Vec<(String, String)> = custom_headers
        .into_iter()
        .filter(|(name, _)| {
            let lower = name.to_ascii_lowercase();
            !BLOCKED_HEADERS.contains(&lower.as_str())
        })
        .collect();

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

const PLACEHOLDER_ORIGIN: &str = "chrome-extension://replace-with-your-extension-id/";

fn extension_origin_from_env() -> Result<String> {
    let origin =
        std::env::var("KHUKRI_EXTENSION_ORIGIN").unwrap_or_else(|_| PLACEHOLDER_ORIGIN.to_string());
    validate_extension_origin(&origin)?;
    Ok(origin)
}

fn validate_extension_origin(origin: &str) -> Result<()> {
    if origin == PLACEHOLDER_ORIGIN || origin.contains("replace-with-your-extension-id") {
        anyhow::bail!(
            "KHUKRI_EXTENSION_ORIGIN is not set. \
             Set it to your extension's chrome-extension://<id>/ origin before registering."
        );
    }
    if !origin.starts_with("chrome-extension://") && !origin.starts_with("moz-extension://") {
        anyhow::bail!(
            "KHUKRI_EXTENSION_ORIGIN must start with chrome-extension:// or moz-extension://, got: {origin}"
        );
    }
    Ok(())
}

fn native_host_manifest(binary_path: &Path) -> Result<HostManifest> {
    Ok(HostManifest {
        name: HOST_ID.to_string(),
        description: "Khukri Native Messaging Host".to_string(),
        path: binary_path.display().to_string(),
        host_type: "stdio".to_string(),
        allowed_origins: vec![extension_origin_from_env()?],
    })
}

#[cfg(target_os = "windows")]
fn register_native_host(binary_path: &Path) -> Result<()> {
    let bridge_dir = binary_path
        .parent()
        .context("bridge binary has no parent directory")?;
    let manifest_path = bridge_dir.join(format!("{HOST_ID}.json"));
    let manifest = serde_json::to_vec_pretty(&native_host_manifest(binary_path)?)?;
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
    let manifest = serde_json::to_vec_pretty(&native_host_manifest(binary_path)?)?;
    fs::write(&manifest_path, manifest)?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn register_native_host(binary_path: &Path) -> Result<()> {
    let config_dir = std::env::var_os("HOME")
        .map(PathBuf::from)
        .context("HOME is not set")?
        .join("Library")
        .join("Application Support")
        .join("Google")
        .join("Chrome")
        .join("NativeMessagingHosts");
    fs::create_dir_all(&config_dir)?;
    let manifest_path = config_dir.join(format!("{HOST_ID}.json"));
    let manifest = serde_json::to_vec_pretty(&native_host_manifest(binary_path)?)?;
    fs::write(&manifest_path, manifest)?;
    Ok(())
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn register_native_host(_binary_path: &Path) -> Result<()> {
    anyhow::bail!("native host registration is not implemented for this platform")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── header sanitization ───────────────────────────────────────────────────

    #[test]
    fn blocked_headers_are_stripped() {
        let mut raw = HashMap::new();
        raw.insert("Host".to_string(), "evil.com".to_string());
        raw.insert("Content-Length".to_string(), "9999".to_string());
        raw.insert("Connection".to_string(), "keep-alive".to_string());
        raw.insert("Authorization".to_string(), "Bearer tok".to_string());
        raw.insert("Transfer-Encoding".to_string(), "chunked".to_string());
        raw.insert("X-Custom".to_string(), "ok".to_string());

        let result = browser_headers(None, raw);
        let names: Vec<String> = result.iter().map(|(k, _)| k.to_ascii_lowercase()).collect();
        assert!(!names.contains(&"host".to_string()));
        assert!(!names.contains(&"content-length".to_string()));
        assert!(!names.contains(&"connection".to_string()));
        assert!(!names.contains(&"authorization".to_string()));
        assert!(!names.contains(&"transfer-encoding".to_string()));
        assert!(names.contains(&"x-custom".to_string()));
    }

    #[test]
    fn referer_injected_when_absent() {
        let result = browser_headers(Some("https://example.com/page"), HashMap::new());
        let referer = result
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("Referer"));
        assert_eq!(
            referer.map(|(_, v)| v.as_str()),
            Some("https://example.com/page")
        );
    }

    #[test]
    fn referer_not_duplicated_when_present() {
        let mut raw = HashMap::new();
        raw.insert("Referer".to_string(), "https://custom.com/".to_string());
        let result = browser_headers(Some("https://page.com/"), raw);
        let count = result
            .iter()
            .filter(|(k, _)| k.eq_ignore_ascii_case("Referer"))
            .count();
        assert_eq!(count, 1);
        assert_eq!(result[0].1, "https://custom.com/");
    }

    // ── origin validation ─────────────────────────────────────────────────────

    #[test]
    fn placeholder_origin_is_rejected() {
        assert!(validate_extension_origin(PLACEHOLDER_ORIGIN).is_err());
        assert!(validate_extension_origin(
            "chrome-extension://replace-with-your-extension-id/extra"
        )
        .is_err());
    }

    #[test]
    fn valid_chrome_origin_is_accepted() {
        assert!(
            validate_extension_origin("chrome-extension://abcdefghijklmnopabcdefghijklmnop/")
                .is_ok()
        );
    }

    #[test]
    fn valid_moz_origin_is_accepted() {
        assert!(validate_extension_origin("moz-extension://some-uuid/").is_ok());
    }

    #[test]
    fn http_origin_is_rejected() {
        assert!(validate_extension_origin("https://evil.com/").is_err());
    }

    // ── filename sanitization ─────────────────────────────────────────────────

    #[test]
    fn sanitize_strips_path_traversal() {
        assert_eq!(sanitize_filename("../etc/passwd"), "passwd");
        assert_eq!(sanitize_filename("/etc/passwd"), "passwd");
    }

    #[test]
    fn sanitize_replaces_reserved_chars() {
        assert_eq!(sanitize_filename("file:name?.bin"), "file_name_.bin");
    }

    #[test]
    fn sanitize_empty_falls_back() {
        assert_eq!(sanitize_filename(""), "download.bin");
        assert_eq!(sanitize_filename("   "), "download.bin");
    }

    #[test]
    fn filename_from_url_strips_query() {
        assert_eq!(
            filename_from_url("https://example.com/file.zip?token=abc"),
            "file.zip"
        );
    }

    #[test]
    fn filename_from_url_trailing_slash() {
        assert_eq!(filename_from_url("https://example.com/"), "download.bin");
    }

    #[test]
    fn filename_from_url_strips_fragment() {
        assert_eq!(
            filename_from_url("https://example.com/file.zip#section"),
            "file.zip"
        );
    }

    #[test]
    fn filename_from_url_strips_query_and_fragment() {
        assert_eq!(
            filename_from_url("https://example.com/file.zip?token=abc#anchor"),
            "file.zip"
        );
    }
}

fn should_register(args: &[String]) -> bool {
    args.iter()
        .any(|arg| arg == "--register" || arg == "--repair")
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
    thread::spawn(move || loop {
        let next = read_message();
        let should_stop = next.is_err();
        if read_tx.send(next).is_err() {
            break;
        }
        if should_stop {
            break;
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
                quality,
                source,
                page_url,
                custom_headers,
            } => {
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
                    };
                    let source_clone = source.clone();
                    let path_display = output_path.display().to_string();
                    let tx = writer_tx.clone();

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

                    tokio::spawn(async move {
                        match media::run_ytdlp(job.clone(), |progress| {
                            let _ = tx.send(media_progress_event(
                                &job.id,
                                &progress,
                                source_clone.clone(),
                                Some(path_display.clone()),
                            ));
                        })
                        .await
                        {
                            Ok(outcome) => {
                                let _ = tx.send(BridgeEvent {
                                    kind: "progress",
                                    id: job.id.clone(),
                                    status: "complete",
                                    bytes_done: 0,
                                    total_bytes: None,
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
    writer_thread
        .join()
        .map_err(|_| anyhow::anyhow!("writer thread panicked"))??;
    Ok(())
}
