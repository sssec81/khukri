use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};
use tokio::process::Command;
use tokio::sync::Mutex;

use crate::bootstrap::app_data_dir;
use crate::AppSettings;

const GITHUB_RELEASES_LATEST: &str = "https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest";
const UPDATE_INTERVAL_SECS: i64 = 24 * 60 * 60;
const RATE_LIMIT_BACKOFF_SECS: i64 = 60 * 60;
const BUNDLED_YTDLP_VERSION: &str = include_str!("../../sidecar/yt-dlp.version");

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct YtdlpUpdateEvent {
    pub kind: String,
    pub message: String,
    pub version: Option<String>,
}

pub fn spawn_background_updater(app: AppHandle, settings: std::sync::Arc<Mutex<AppSettings>>) {
    if std::env::var_os("KHUKRI_YTDLP_BIN").is_some() {
        tracing::info!("skipping yt-dlp updater because KHUKRI_YTDLP_BIN is set");
        return;
    }

    tauri::async_runtime::spawn(async move {
        if let Err(error) = maybe_update_ytdlp(app.clone(), settings.clone(), false).await {
            tracing::warn!(%error, "yt-dlp update check failed");
        }
    });
}

pub async fn maybe_update_ytdlp(
    app: AppHandle,
    settings: std::sync::Arc<Mutex<AppSettings>>,
    force: bool,
) -> Result<(), String> {
    let now = unix_now_secs();
    let should_check = {
        let current = settings.lock().await.clone();
        if !force && !current.ytdlp_auto_update {
            return Ok(());
        }

        let backoff = if current.ytdlp_last_rate_limit {
            RATE_LIMIT_BACKOFF_SECS
        } else {
            UPDATE_INTERVAL_SECS
        };
        force
            || current
                .ytdlp_last_check
                .map(|last| now.saturating_sub(last) >= backoff)
                .unwrap_or(true)
    };

    if !should_check {
        return Ok(());
    }

    {
        let mut current = settings.lock().await;
        current.ytdlp_last_check = Some(now);
        current.ytdlp_last_rate_limit = false;
        save_settings(&current)?;
    }

    let release = match fetch_latest_release().await {
        Ok(release) => release,
        Err(error) => {
            let status = error.status();
            let rate_limited = status == Some(reqwest::StatusCode::FORBIDDEN);
            let reason = if rate_limited {
                "yt-dlp update check is rate-limited; backing off for one hour".to_string()
            } else {
                format!("yt-dlp update check failed: {error}")
            };
            handle_failure(&app, &settings, reason, None, rate_limited).await?;
            return Ok(());
        }
    };
    let current_version = {
        let current = settings.lock().await;
        current_effective_version(&current)
    };

    if release.tag_name.trim() == current_version.trim() {
        let mut current = settings.lock().await;
        current.ytdlp_version = Some(release.tag_name);
        save_settings(&current)?;
        if force {
            let _ = app.emit(
                "ytdlp-update-status",
                YtdlpUpdateEvent {
                    kind: "noop".to_string(),
                    message: "yt-dlp is already current.".to_string(),
                    version: current.ytdlp_version.clone(),
                },
            );
        }
        return Ok(());
    }

    let asset_name = match platform_release_asset_name() {
        Ok(name) => name,
        Err(error) => {
            handle_failure(&app, &settings, error, None, false).await?;
            return Ok(());
        }
    };
    let checksum_asset = release
        .assets
        .iter()
        .find(|asset| {
            asset.name.eq_ignore_ascii_case("SHA2-256SUMS") || asset.name.contains("SHA2-256SUMS")
        })
        .ok_or_else(|| "yt-dlp release checksums asset not found".to_string());
    let checksum_asset = match checksum_asset {
        Ok(asset) => asset,
        Err(error) => {
            handle_failure(
                &app,
                &settings,
                error,
                Some(release.tag_name.clone()),
                false,
            )
            .await?;
            return Ok(());
        }
    };
    let binary_asset = release
        .assets
        .iter()
        .find(|asset| asset.name == asset_name)
        .ok_or_else(|| {
            format!("yt-dlp release does not include asset for this platform: {asset_name}")
        });
    let binary_asset = match binary_asset {
        Ok(asset) => asset,
        Err(error) => {
            handle_failure(
                &app,
                &settings,
                error,
                Some(release.tag_name.clone()),
                false,
            )
            .await?;
            return Ok(());
        }
    };

    let checksums = download_text(&checksum_asset.browser_download_url)
        .await
        .map_err(|error| error.to_string());
    let checksums = match checksums {
        Ok(body) => body,
        Err(error) => {
            handle_failure(
                &app,
                &settings,
                format!("yt-dlp checksum download failed: {error}"),
                Some(release.tag_name.clone()),
                false,
            )
            .await?;
            return Ok(());
        }
    };
    let expected_sha = parse_release_checksum(&checksums, asset_name)
        .ok_or_else(|| format!("checksum for {asset_name} not found in release manifest"));
    let expected_sha = match expected_sha {
        Ok(value) => value,
        Err(error) => {
            handle_failure(
                &app,
                &settings,
                error,
                Some(release.tag_name.clone()),
                false,
            )
            .await?;
            return Ok(());
        }
    };
    let binary_bytes = download_bytes(&binary_asset.browser_download_url)
        .await
        .map_err(|error| error.to_string());
    let binary_bytes = match binary_bytes {
        Ok(bytes) => bytes,
        Err(error) => {
            handle_failure(
                &app,
                &settings,
                format!("yt-dlp binary download failed: {error}"),
                Some(release.tag_name.clone()),
                false,
            )
            .await?;
            return Ok(());
        }
    };
    let actual_sha = sha256_hex(&binary_bytes);
    if !actual_sha.eq_ignore_ascii_case(&expected_sha) {
        handle_failure(
            &app,
            &settings,
            "yt-dlp update failed: checksum mismatch".to_string(),
            Some(release.tag_name.clone()),
            false,
        )
        .await?;
        return Ok(());
    }

    let managed_dir = managed_sidecar_dir();
    std::fs::create_dir_all(&managed_dir).map_err(|e| e.to_string())?;
    let temp_path = managed_dir.join(format!("{}.tmp", platform_sidecar_name()?));
    let sidecar_name = platform_sidecar_name()?;
    let active_path = managed_dir.join(sidecar_name);
    let backup_path = managed_dir.join(format!("{sidecar_name}.bak"));

    std::fs::write(&temp_path, &binary_bytes).map_err(|e| e.to_string())?;
    make_executable_if_needed(&temp_path)?;

    let canary_version = run_canary(&temp_path).await;
    let canary_version = match canary_version {
        Ok(version) => version,
        Err(error) => {
            let _ = std::fs::remove_file(&temp_path);
            handle_failure(
                &app,
                &settings,
                format!("yt-dlp update failed: {error}"),
                Some(release.tag_name.clone()),
                false,
            )
            .await?;
            return Ok(());
        }
    };

    if backup_path.exists() {
        let _ = std::fs::remove_file(&backup_path);
    }
    if active_path.exists() {
        rename_or_copy(&active_path, &backup_path)?;
    }
    rename_or_copy(&temp_path, &active_path)?;
    make_executable_if_needed(&active_path)?;

    {
        let mut current = settings.lock().await;
        current.ytdlp_version = Some(release.tag_name.clone());
        current.ytdlp_last_notified_failure = None;
        current.ytdlp_last_rate_limit = false;
        save_settings(&current)?;
    }

    let message = format!("yt-dlp updated to {}", release.tag_name);
    let _ = app.emit(
        "ytdlp-update-status",
        YtdlpUpdateEvent {
            kind: "updated".to_string(),
            message,
            version: Some(canary_version),
        },
    );
    tracing::info!(version = %release.tag_name, binary = %active_path.display(), "yt-dlp sidecar updated");
    Ok(())
}

async fn fetch_latest_release() -> Result<GithubRelease, reqwest::Error> {
    client()?
        .get(GITHUB_RELEASES_LATEST)
        .send()
        .await?
        .error_for_status()?
        .json::<GithubRelease>()
        .await
}

async fn download_text(url: &str) -> Result<String, reqwest::Error> {
    client()?
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await
}

async fn download_bytes(url: &str) -> Result<Vec<u8>, reqwest::Error> {
    client()?
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
}

fn client() -> Result<reqwest::Client, reqwest::Error> {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("Khukri/0.1.0"));
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/vnd.github+json"),
    );
    reqwest::Client::builder()
        .default_headers(headers)
        .timeout(Duration::from_secs(30))
        .build()
}

async fn handle_failure(
    app: &AppHandle,
    settings: &std::sync::Arc<Mutex<AppSettings>>,
    reason: String,
    version: Option<String>,
    rate_limited: bool,
) -> Result<(), String> {
    let should_emit = {
        let mut current = settings.lock().await;
        current.ytdlp_last_rate_limit = rate_limited;
        let already_sent = current
            .ytdlp_last_notified_failure
            .as_deref()
            .map(|message| message == reason)
            .unwrap_or(false);
        if !already_sent {
            current.ytdlp_last_notified_failure = Some(reason.clone());
        }
        save_settings(&current)?;
        !already_sent
    };

    tracing::warn!(%reason, "yt-dlp update failed");
    if should_emit {
        let _ = app.emit(
            "ytdlp-update-status",
            YtdlpUpdateEvent {
                kind: "failed".to_string(),
                message: reason,
                version,
            },
        );
    }
    Ok(())
}

fn current_effective_version(settings: &AppSettings) -> String {
    settings
        .ytdlp_version
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| BUNDLED_YTDLP_VERSION.trim().to_string())
}

fn parse_release_checksum(body: &str, asset_name: &str) -> Option<String> {
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        let file_name = parts.last()?.trim_start_matches('*');
        if file_name == asset_name {
            return Some(parts[0].to_string());
        }
    }
    None
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn managed_sidecar_dir() -> PathBuf {
    app_data_dir().join("sidecar")
}

fn save_settings(settings: &AppSettings) -> Result<(), String> {
    let path = app_data_dir().join("settings.json");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

async fn run_canary(path: &Path) -> Result<String, String> {
    let output = Command::new(path)
        .arg("--version")
        .output()
        .await
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "canary execution failed with status {}",
            output.status
        ));
    }
    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if version.is_empty() {
        return Err("canary execution returned no version output".to_string());
    }
    Ok(version)
}

fn rename_or_copy(from: &Path, to: &Path) -> Result<(), String> {
    match std::fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(_) => {
            std::fs::copy(from, to).map_err(|e| e.to_string())?;
            std::fs::remove_file(from).map_err(|e| e.to_string())
        }
    }
}

fn unix_now_secs() -> i64 {
    chrono::Utc::now().timestamp()
}

fn platform_sidecar_name() -> Result<&'static str, String> {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        return Ok("yt-dlp-x86_64-pc-windows-msvc.exe");
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        return Ok("yt-dlp-x86_64-unknown-linux-gnu");
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        return Ok("yt-dlp-x86_64-apple-darwin");
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        return Ok("yt-dlp-aarch64-apple-darwin");
    }
    #[cfg(not(any(
        all(target_os = "windows", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    {
        Err("yt-dlp updater is not configured for this target triple".to_string())
    }
}

fn platform_release_asset_name() -> Result<&'static str, String> {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        return Ok("yt-dlp.exe");
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        return Ok("yt-dlp_linux");
    }
    #[cfg(all(
        target_os = "macos",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    {
        return Ok("yt-dlp_macos");
    }
    #[cfg(not(any(
        all(target_os = "windows", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    {
        Err("yt-dlp release asset is not configured for this target triple".to_string())
    }
}

fn make_executable_if_needed(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(path)
            .map_err(|e| e.to_string())?
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).map_err(|e| e.to_string())?;
    }

    #[cfg(not(unix))]
    {
        let _ = path;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_manifest_parses_expected_asset() {
        let manifest = "abc123  yt-dlp.exe\nfff999  yt-dlp_linux\n";
        assert_eq!(
            parse_release_checksum(manifest, "yt-dlp_linux"),
            Some("fff999".to_string())
        );
    }
}
