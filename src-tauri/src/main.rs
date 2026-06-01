mod bootstrap;
mod media;
mod ytdlp_updater;

use chrono::{Local, Timelike};
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use khukri_engine::{
    db, spawn_download, DownloadConfig, DownloadHandle, DownloadProgress, DownloadStatus, Priority,
    ThrottleConfig,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, State,
};
use tauri_plugin_dialog::DialogExt;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::bootstrap::{app_data_dir, init_db, DbConfig};
use crate::media::{
    log_ffmpeg_version, should_use_ytdlp, MediaDownloadHandle, MediaJob, MediaQuality,
};
use crate::ytdlp_updater::{maybe_update_ytdlp, spawn_background_updater};

const UI_PROGRESS_INTERVAL: Duration = Duration::from_millis(500);
const SCHEDULER_POLL_INTERVAL: Duration = Duration::from_secs(30);
const TASK_SETTLE_TIMEOUT: Duration = Duration::from_secs(3);
const TASK_SETTLE_POLL_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct StartDownloadRequest {
    url: String,
    file_path: String,
    priority: Option<String>,
    override_threads: Option<u8>,
    bytes_per_sec: Option<u64>,
    quality: Option<String>,
    source: Option<String>,
    #[serde(default)]
    custom_headers: HashMap<String, String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DownloadListItem {
    id: String,
    url: String,
    file_path: String,
    file_exists: bool,
    total_bytes: Option<i64>,
    status: String,
    priority: String,
    throttle_bytes_per_sec: Option<i64>,
    media_quality: Option<String>,
    request_source: Option<String>,
    failure_reason: Option<String>,
    created_at: i64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DownloadProgressEvent {
    id: String,
    status: String,
    bytes_done: u64,
    total_bytes: Option<u64>,
    speed_bps: u64,
    eta_seconds: Option<u64>,
    segments_done: u32,
    segments_total: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GeneralSettings {
    default_download_path: String,
    max_concurrent: u8,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PerformanceSettings {
    thread_override: Option<u8>,
    bandwidth_cap: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SchedulerSettings {
    enabled: bool,
    start_hour: u8,
    end_hour: u8,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProxySettings {
    enabled: bool,
    url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AppearanceSettings {
    theme: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AppSettings {
    general: GeneralSettings,
    performance: PerformanceSettings,
    scheduler: SchedulerSettings,
    proxy: ProxySettings,
    appearance: AppearanceSettings,
    #[serde(default, rename = "onboarding_complete")]
    onboarding_complete: bool,
    #[serde(default, rename = "ytdlp_auto_update")]
    ytdlp_auto_update: bool,
    #[serde(default, rename = "ytdlp_last_check")]
    ytdlp_last_check: Option<i64>,
    #[serde(default, rename = "ytdlp_version")]
    ytdlp_version: Option<String>,
    #[serde(default, rename = "ytdlp_last_notified_failure")]
    ytdlp_last_notified_failure: Option<String>,
    #[serde(default, rename = "ytdlp_last_rate_limit")]
    ytdlp_last_rate_limit: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            general: GeneralSettings {
                default_download_path: app_data_dir().join("downloads").display().to_string(),
                max_concurrent: 3,
            },
            performance: PerformanceSettings {
                thread_override: None,
                bandwidth_cap: None,
            },
            scheduler: SchedulerSettings {
                enabled: false,
                start_hour: 0,
                end_hour: 23,
            },
            proxy: ProxySettings {
                enabled: false,
                url: String::new(),
            },
            appearance: AppearanceSettings {
                theme: "system".to_string(),
            },
            onboarding_complete: false,
            ytdlp_auto_update: true,
            ytdlp_last_check: None,
            ytdlp_version: None,
            ytdlp_last_notified_failure: None,
            ytdlp_last_rate_limit: false,
        }
    }
}

#[derive(Clone)]
struct ManagedDownload {
    task: ManagedTask,
}

#[derive(Clone)]
enum ManagedTask {
    Engine(Arc<Mutex<DownloadHandle>>),
    Media(Arc<MediaDownloadHandle>),
}

impl ManagedTask {
    async fn cancel(&self) {
        match self {
            ManagedTask::Engine(handle) => handle.lock().await.cancel(),
            ManagedTask::Media(handle) => handle.cancel(),
        }
    }
}

struct AppState {
    pool: SqlitePool,
    active: Arc<Mutex<HashMap<String, ManagedDownload>>>,
    cancelled: Arc<Mutex<HashSet<String>>>,
    settings: Arc<Mutex<AppSettings>>,
    quitting: Arc<AtomicBool>,
}

#[cfg(desktop)]
struct TrayMenuState {
    pause_all: MenuItem<tauri::Wry>,
    resume_all: MenuItem<tauri::Wry>,
}

fn settings_path() -> PathBuf {
    app_data_dir().join("settings.json")
}

fn load_settings_from_disk() -> AppSettings {
    let path = settings_path();
    let contents = match std::fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(_) => return AppSettings::default(),
    };

    serde_json::from_str(&contents).unwrap_or_else(|_| AppSettings::default())
}

fn save_settings_to_disk(settings: &AppSettings) -> Result<(), String> {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

fn parse_priority(value: Option<&str>) -> Priority {
    match value.unwrap_or("normal").to_ascii_lowercase().as_str() {
        "high" => Priority::High,
        "low" => Priority::Low,
        _ => Priority::Normal,
    }
}

fn infer_download_filename(url: &str) -> String {
    let parsed = url.split('?').next().unwrap_or(url).trim_end_matches('/');
    let name = parsed
        .rsplit('/')
        .next()
        .filter(|segment| !segment.is_empty())
        .unwrap_or("download.bin");

    name.to_string()
}

fn normalized_download_target(raw: &str, url: &str, prefer_directory: bool) -> PathBuf {
    let candidate = PathBuf::from(raw.trim());
    let raw = raw.trim();
    let ends_with_separator =
        raw.ends_with(std::path::MAIN_SEPARATOR) || raw.ends_with('/') || raw.ends_with('\\');
    let looks_like_directory = prefer_directory
        || ends_with_separator
        || candidate.as_path().is_dir()
        || candidate.file_name().is_none();

    if !looks_like_directory {
        return candidate;
    }

    candidate.join(infer_download_filename(url))
}

fn normalize_output_path(raw: &str, settings: &AppSettings, url: &str) -> PathBuf {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return normalized_download_target(&settings.general.default_download_path, url, true);
    }

    let is_default_directory = trimmed == settings.general.default_download_path.trim();
    normalized_download_target(trimmed, url, is_default_directory)
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

fn priority_rank(value: &str) -> u8 {
    match value.to_ascii_lowercase().as_str() {
        "high" => 2,
        "normal" => 1,
        _ => 0,
    }
}

fn download_id_for(url: &str, file_path: &str) -> String {
    let key = format!("{url}|{file_path}");
    Uuid::new_v5(&Uuid::NAMESPACE_URL, key.as_bytes()).to_string()
}

fn unix_now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn output_file_exists(file_path: &str) -> bool {
    Path::new(file_path).is_file()
}

fn map_download_row(row: db::DownloadRow) -> DownloadListItem {
    let file_exists = output_file_exists(&row.file_path);

    DownloadListItem {
        id: row.id,
        url: row.url,
        file_path: row.file_path,
        file_exists,
        total_bytes: row.total_bytes,
        status: row.status,
        priority: row.priority,
        throttle_bytes_per_sec: row.throttle_bytes_per_sec,
        media_quality: row.media_quality,
        request_source: row.request_source,
        failure_reason: row.failure_reason,
        created_at: row.created_at,
    }
}

fn request_from_row(row: &db::DownloadRow) -> StartDownloadRequest {
    StartDownloadRequest {
        url: row.url.clone(),
        file_path: row.file_path.clone(),
        priority: Some(row.priority.clone()),
        override_threads: None,
        bytes_per_sec: row
            .throttle_bytes_per_sec
            .and_then(|value| u64::try_from(value).ok()),
        quality: row.media_quality.clone(),
        source: row.request_source.clone(),
        custom_headers: HashMap::new(),
    }
}

fn map_progress_event(progress: &DownloadProgress) -> DownloadProgressEvent {
    DownloadProgressEvent {
        id: progress.id.clone(),
        status: status_label(progress.status).to_string(),
        bytes_done: progress.bytes_done,
        total_bytes: progress.total_bytes,
        speed_bps: progress.speed_bps,
        eta_seconds: progress.eta_seconds,
        segments_done: progress.segments_done,
        segments_total: progress.segments_total,
    }
}

fn request_with_settings(
    mut request: StartDownloadRequest,
    settings: &AppSettings,
) -> StartDownloadRequest {
    if request.override_threads.is_none() {
        request.override_threads = settings.performance.thread_override;
    }
    if request.bytes_per_sec.is_none() {
        request.bytes_per_sec = settings.performance.bandwidth_cap;
    }
    request
}

fn browser_headers(
    page_url: Option<&str>,
    custom_headers: HashMap<String, String>,
) -> Vec<(String, String)> {
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

fn is_scheduler_window_open(settings: &AppSettings) -> bool {
    if !settings.scheduler.enabled {
        return true;
    }

    let hour = Local::now().hour() as u8;
    let start = settings.scheduler.start_hour.min(23);
    let end = settings.scheduler.end_hour.min(23);

    if start <= end {
        (start..=end).contains(&hour)
    } else {
        hour >= start || hour <= end
    }
}

fn configured_proxy_url(settings: &AppSettings) -> Option<String> {
    if !settings.proxy.enabled {
        return None;
    }

    let trimmed = settings.proxy.url.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn cleanup_download_file(path: &str) -> Result<(), String> {
    let target = Path::new(path);
    if !target.exists() {
        return Ok(());
    }

    std::fs::remove_file(target)
        .map_err(|e| format!("failed to remove '{}': {e}", target.display()))
}

async fn cleanup_download_file_for_id(pool: &SqlitePool, id: &str) -> Result<(), String> {
    if let Some(row) = db::get_download(pool, id)
        .await
        .map_err(|e| e.to_string())?
    {
        cleanup_download_file(&row.file_path)?;
    }

    Ok(())
}

async fn refresh_download_snapshot(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<DownloadListItem>, String> {
    db::get_download(pool, id)
        .await
        .map_err(|e| e.to_string())
        .map(|row| row.map(map_download_row))
}

async fn wait_for_download_snapshot(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<DownloadListItem>, String> {
    // A single read is sufficient: start_managed_download constructs the DB
    // row before spawning the progress task, so the row is available
    // immediately. The call site already has a synthesized fallback for the
    // None case, so there is no need to poll.
    refresh_download_snapshot(pool, id).await
}

async fn emit_queue_updated(app: &tauri::AppHandle, pool: &SqlitePool) -> Result<(), String> {
    let queue = db::list_downloads(pool)
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(map_download_row)
        .collect::<Vec<_>>();
    sync_tray_menu_state(app, &queue)?;
    app.emit("queue-updated", &queue).map_err(|e| e.to_string())
}

#[cfg(desktop)]
fn sync_tray_menu_state(app: &tauri::AppHandle, queue: &[DownloadListItem]) -> Result<(), String> {
    let Some(tray_menu) = app.try_state::<TrayMenuState>() else {
        return Ok(());
    };

    let (can_pause, can_resume) = tray_action_state(queue);

    tray_menu
        .pause_all
        .set_enabled(can_pause)
        .map_err(|e| e.to_string())?;
    tray_menu
        .resume_all
        .set_enabled(can_resume)
        .map_err(|e| e.to_string())?;

    Ok(())
}

fn tray_action_state(queue: &[DownloadListItem]) -> (bool, bool) {
    let can_pause = queue
        .iter()
        .any(|item| item.status == "active" || item.status == "queued");
    let can_resume = queue.iter().any(|item| item.status == "paused");
    (can_pause, can_resume)
}

#[cfg(not(desktop))]
fn sync_tray_menu_state(
    _app: &tauri::AppHandle,
    _queue: &[DownloadListItem],
) -> Result<(), String> {
    Ok(())
}

async fn persist_queued_download(
    pool: &SqlitePool,
    request: &StartDownloadRequest,
    settings: &AppSettings,
    existing_id: Option<&str>,
) -> Result<DownloadListItem, String> {
    let request = request_with_settings(request.clone(), settings);
    let output_path = normalize_output_path(&request.file_path, settings, &request.url);
    let output_path_string = output_path.to_string_lossy().to_string();
    let id = existing_id
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| download_id_for(&request.url, &output_path_string));
    let priority = parse_priority(request.priority.as_deref());

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    db::upsert_download(
        pool,
        &id,
        &request.url,
        &output_path_string,
        None,
        priority.as_str(),
        request.bytes_per_sec,
        unix_now_secs(),
    )
    .await
    .map_err(|e| e.to_string())?;
    db::set_download_request_metadata(
        pool,
        &id,
        request.quality.as_deref(),
        request.source.as_deref(),
    )
    .await
    .map_err(|e| e.to_string())?;
    db::set_download_status(pool, &id, "queued")
        .await
        .map_err(|e| e.to_string())?;

    refresh_download_snapshot(pool, &id)
        .await?
        .ok_or_else(|| format!("queued download missing after insert: {id}"))
}

async fn promote_pending_downloads_with_parts(
    app: &tauri::AppHandle,
    pool: SqlitePool,
    active: Arc<Mutex<HashMap<String, ManagedDownload>>>,
    cancelled: Arc<Mutex<HashSet<String>>>,
    settings: Arc<Mutex<AppSettings>>,
) -> Result<(), String> {
    let scheduler_open = {
        let snapshot = settings.lock().await.clone();
        is_scheduler_window_open(&snapshot)
    };
    if !scheduler_open {
        return Ok(());
    }

    loop {
        let max_concurrent = settings.lock().await.general.max_concurrent as usize;

        // Lock active map and hold it to prevent race conditions
        let active_guard = active.lock().await;
        if active_guard.len() >= max_concurrent {
            break;
        }
        let active_ids: HashSet<String> = active_guard.keys().cloned().collect();
        drop(active_guard);

        let mut queued = db::list_downloads(&pool)
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|row| row.status == "queued" && !active_ids.contains(&row.id))
            .collect::<Vec<_>>();

        queued.sort_by(|left, right| {
            priority_rank(&right.priority)
                .cmp(&priority_rank(&left.priority))
                .then_with(|| left.created_at.cmp(&right.created_at))
                .then_with(|| left.id.cmp(&right.id))
        });

        let Some(row) = queued.into_iter().next() else {
            break;
        };

        let request = request_from_row(&row);

        if let Err(error) = start_managed_download(
            app.clone(),
            pool.clone(),
            active.clone(),
            cancelled.clone(),
            settings.clone(),
            request,
            None,
        )
        .await
        {
            db::set_download_failed(&pool, &row.id, &error)
                .await
                .map_err(|e| e.to_string())?;
            let _ = emit_queue_updated(app, &pool).await;
        }
    }

    Ok(())
}

async fn promote_pending_downloads(app: &tauri::AppHandle, state: &AppState) -> Result<(), String> {
    promote_pending_downloads_with_parts(
        app,
        state.pool.clone(),
        state.active.clone(),
        state.cancelled.clone(),
        state.settings.clone(),
    )
    .await
}

async fn wait_until_inactive(
    active: &Arc<Mutex<HashMap<String, ManagedDownload>>>,
    id: &str,
    timeout: Duration,
) -> bool {
    let started = Instant::now();
    loop {
        if !active.lock().await.contains_key(id) {
            return true;
        }

        if started.elapsed() >= timeout {
            return false;
        }

        tokio::time::sleep(TASK_SETTLE_POLL_INTERVAL).await;
    }
}

async fn start_or_queue_download(
    app: tauri::AppHandle,
    pool: SqlitePool,
    active: Arc<Mutex<HashMap<String, ManagedDownload>>>,
    cancelled: Arc<Mutex<HashSet<String>>>,
    settings: Arc<Mutex<AppSettings>>,
    request: StartDownloadRequest,
    existing_id: Option<String>,
) -> Result<DownloadListItem, String> {
    let settings_snapshot = settings.lock().await.clone();
    let request = request_with_settings(request, &settings_snapshot);
    let output_path = normalize_output_path(&request.file_path, &settings_snapshot, &request.url);
    let output_path_string = output_path.to_string_lossy().to_string();
    let requested_id = existing_id
        .clone()
        .unwrap_or_else(|| download_id_for(&request.url, &output_path_string));
    let max_concurrent = settings_snapshot.general.max_concurrent as usize;

    // Acquire lock once and hold it through duplicate check and concurrency limit check
    let active_guard = active.lock().await;

    if active_guard.contains_key(&requested_id) {
        drop(active_guard);
        return refresh_download_snapshot(&pool, &requested_id)
            .await?
            .ok_or_else(|| format!("download already active: {requested_id}"));
    }

    if !is_scheduler_window_open(&settings_snapshot) || active_guard.len() >= max_concurrent {
        drop(active_guard);
        let snapshot = persist_queued_download(
            &pool,
            &request,
            &settings_snapshot,
            existing_id.as_deref(),
        )
        .await?;
        emit_queue_updated(&app, &pool).await?;
        return Ok(snapshot);
    }

    drop(active_guard);
    start_managed_download(app, pool, active, cancelled, settings, request, existing_id).await
}

fn open_path_in_explorer(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("explorer.exe");
        command.arg(path);
        command
    };

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(path);
        command
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(path);
        command
    };

    command
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("failed to open folder: {e}"))
}

async fn start_managed_download(
    app: tauri::AppHandle,
    pool: SqlitePool,
    active: Arc<Mutex<HashMap<String, ManagedDownload>>>,
    cancelled: Arc<Mutex<HashSet<String>>>,
    settings: Arc<Mutex<AppSettings>>,
    request: StartDownloadRequest,
    existing_id: Option<String>,
) -> Result<DownloadListItem, String> {
    let settings_snapshot = settings.lock().await.clone();
    let request = request_with_settings(request, &settings_snapshot);
    let output_path = normalize_output_path(&request.file_path, &settings_snapshot, &request.url);
    let output_path_string = output_path.to_string_lossy().to_string();
    let priority = parse_priority(request.priority.as_deref());
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    if should_use_ytdlp(request.source.as_deref(), request.quality.as_deref()) {
        return start_managed_media_download(
            app,
            pool,
            active,
            cancelled,
            request,
            output_path,
            output_path_string,
            priority,
            existing_id,
        )
        .await;
    }

    let mut config = DownloadConfig::new(request.url.clone(), output_path);
    // Enforce output path sandbox: restrict downloads to the configured downloads directory.
    config.allowed_root = Some(PathBuf::from(
        &settings_snapshot.general.default_download_path,
    ));
    config.priority = priority.clone();
    config.override_threads = request.override_threads;
    config.throttle = ThrottleConfig {
        bytes_per_sec: request.bytes_per_sec,
    };
    config.proxy_url = configured_proxy_url(&settings_snapshot);

    // Convert custom_headers HashMap to Vec<(String, String)> and set on config.
    let headers: Vec<(String, String)> = request
        .custom_headers
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    config.custom_headers = headers;

    let handle = spawn_download(config, pool.clone());
    let id = handle.id().to_string();
    let progress = handle.subscribe();
    let managed = ManagedDownload {
        task: ManagedTask::Engine(Arc::new(Mutex::new(handle))),
    };
    db::set_download_request_metadata(
        &pool,
        &id,
        request.quality.as_deref(),
        request.source.as_deref(),
    )
    .await
    .map_err(|e| e.to_string())?;

    db::set_download_status(&pool, &id, "active")
        .await
        .map_err(|e| e.to_string())?;
    cancelled.lock().await.remove(&id);
    active.lock().await.insert(id.clone(), managed);

    let app_for_task = app.clone();
    let id_for_task = id.clone();
    let active_for_task = active.clone();
    let cancelled_for_task = cancelled.clone();
    let pool_for_task = pool.clone();
    tokio::spawn(async move {
        let mut progress_rx = progress;
        let mut last_emit = None::<Instant>;
        loop {
            let snapshot = progress_rx.borrow().clone();
            let is_terminal = matches!(
                snapshot.status,
                DownloadStatus::Complete | DownloadStatus::Failed | DownloadStatus::Paused
            );
            let should_emit = last_emit.is_none()
                || is_terminal
                || last_emit
                    .map(|instant| instant.elapsed() >= UI_PROGRESS_INTERVAL)
                    .unwrap_or(true);

            if should_emit {
                let payload = map_progress_event(&snapshot);
                let _ = app_for_task.emit("download-progress", &payload);
                last_emit = Some(Instant::now());
            }

            if is_terminal {
                active_for_task.lock().await.remove(&id_for_task);
                if cancelled_for_task.lock().await.remove(&snapshot.id) {
                    if matches!(snapshot.status, DownloadStatus::Paused) {
                        let _ = db::set_download_cancelled(&pool_for_task, &snapshot.id).await;
                        let _ = cleanup_download_file_for_id(&pool_for_task, &snapshot.id).await;
                    }
                }
                let _ = refresh_download_snapshot(&pool_for_task, &snapshot.id).await;
                let _ = emit_queue_updated(&app_for_task, &pool_for_task).await;
                break;
            }

            if progress_rx.changed().await.is_err() {
                active_for_task.lock().await.remove(&id_for_task);
                let _ = emit_queue_updated(&app_for_task, &pool_for_task).await;
                break;
            }
        }
    });

    let snapshot = wait_for_download_snapshot(&pool, &id)
        .await?
        .unwrap_or_else(|| DownloadListItem {
            id: id.clone(),
            url: request.url.clone(),
            file_path: output_path_string,
            file_exists: false,
            total_bytes: None,
            status: "queued".to_string(),
            priority: priority.as_str().to_string(),
            throttle_bytes_per_sec: request
                .bytes_per_sec
                .and_then(|value| i64::try_from(value).ok()),
            media_quality: request.quality.clone(),
            request_source: request.source.clone(),
            failure_reason: None,
            created_at: 0,
        });
    emit_queue_updated(&app, &pool).await?;
    Ok(snapshot)
}

async fn start_managed_media_download(
    app: tauri::AppHandle,
    pool: SqlitePool,
    active: Arc<Mutex<HashMap<String, ManagedDownload>>>,
    cancelled: Arc<Mutex<HashSet<String>>>,
    request: StartDownloadRequest,
    output_path: PathBuf,
    output_path_string: String,
    priority: Priority,
    existing_id: Option<String>,
) -> Result<DownloadListItem, String> {
    let quality = MediaQuality::parse(request.quality.as_deref());
    let id = existing_id.unwrap_or_else(|| download_id_for(&request.url, &output_path_string));
    let headers = browser_headers(request.source.as_deref(), request.custom_headers.clone());

    db::upsert_download(
        &pool,
        &id,
        &request.url,
        &output_path_string,
        None,
        priority.as_str(),
        request.bytes_per_sec,
        unix_now_secs(),
    )
    .await
    .map_err(|e| e.to_string())?;
    db::set_download_request_metadata(
        &pool,
        &id,
        request.quality.as_deref(),
        request.source.as_deref(),
    )
    .await
    .map_err(|e| e.to_string())?;
    db::set_download_status(&pool, &id, "active")
        .await
        .map_err(|e| e.to_string())?;

    let handle = media::spawn_media_download(MediaJob {
        id: id.clone(),
        url: request.url.clone(),
        output_path,
        quality,
        headers,
    });
    let progress = handle.subscribe();
    let handle = Arc::new(handle);
    let managed = ManagedDownload {
        task: ManagedTask::Media(handle.clone()),
    };

    cancelled.lock().await.remove(&id);
    active.lock().await.insert(id.clone(), managed);

    let app_for_task = app.clone();
    let id_for_task = id.clone();
    let active_for_task = active.clone();
    let cancelled_for_task = cancelled.clone();
    let pool_for_task = pool.clone();
    tokio::spawn(async move {
        let mut progress_rx = progress;
        let mut last_emit = None::<Instant>;
        loop {
            let snapshot = progress_rx.borrow().clone();
            let is_terminal = matches!(
                snapshot.status,
                DownloadStatus::Complete | DownloadStatus::Failed | DownloadStatus::Paused
            );
            let should_emit = last_emit.is_none()
                || is_terminal
                || last_emit
                    .map(|instant| instant.elapsed() >= UI_PROGRESS_INTERVAL)
                    .unwrap_or(true);

            if should_emit {
                let payload = map_progress_event(&snapshot);
                let _ = app_for_task.emit("download-progress", &payload);
                last_emit = Some(Instant::now());
            }

            if is_terminal {
                active_for_task.lock().await.remove(&id_for_task);
                match snapshot.status {
                    DownloadStatus::Complete => {
                        if let Some(final_path) = handle.final_path().await {
                            let _ = db::set_download_file_path(
                                &pool_for_task,
                                &snapshot.id,
                                &final_path.display().to_string(),
                            )
                            .await;
                        }
                        let _ =
                            db::set_download_status(&pool_for_task, &snapshot.id, "complete").await;
                    }
                    DownloadStatus::Paused => {
                        if cancelled_for_task.lock().await.remove(&snapshot.id) {
                            let _ = db::set_download_cancelled(&pool_for_task, &snapshot.id).await;
                            let _ =
                                cleanup_download_file_for_id(&pool_for_task, &snapshot.id).await;
                        } else {
                            let _ = db::set_download_status(&pool_for_task, &snapshot.id, "paused")
                                .await;
                        }
                    }
                    DownloadStatus::Failed => {
                        let reason = handle
                            .failure_reason()
                            .await
                            .unwrap_or_else(|| "yt-dlp download failed".to_string());
                        let _ =
                            db::set_download_failed(&pool_for_task, &snapshot.id, &reason).await;
                    }
                    _ => {}
                }
                let _ = emit_queue_updated(&app_for_task, &pool_for_task).await;
                break;
            }

            if progress_rx.changed().await.is_err() {
                active_for_task.lock().await.remove(&id_for_task);
                let _ = emit_queue_updated(&app_for_task, &pool_for_task).await;
                break;
            }
        }
    });

    let snapshot = wait_for_download_snapshot(&pool, &id)
        .await?
        .unwrap_or(DownloadListItem {
            id,
            url: request.url,
            file_path: output_path_string,
            file_exists: false,
            total_bytes: None,
            status: "active".to_string(),
            priority: priority.as_str().to_string(),
            throttle_bytes_per_sec: request
                .bytes_per_sec
                .and_then(|value| i64::try_from(value).ok()),
            media_quality: request.quality,
            request_source: request.source,
            failure_reason: None,
            created_at: unix_now_secs(),
        });
    emit_queue_updated(&app, &pool).await?;
    Ok(snapshot)
}

async fn pause_all_downloads(app: &tauri::AppHandle, state: &AppState) -> Result<(), String> {
    let downloads: Vec<(String, ManagedDownload)> = {
        let active = state.active.lock().await;
        active
            .iter()
            .map(|(id, download)| (id.clone(), download.clone()))
            .collect()
    };

    for (_, download) in &downloads {
        download.task.cancel().await;
    }

    db::set_download_status_where(&state.pool, &["active", "queued"], "paused")
        .await
        .map_err(|e| e.to_string())?;

    for (id, _) in &downloads {
        let _ = wait_until_inactive(&state.active, id, TASK_SETTLE_TIMEOUT).await;
    }

    emit_queue_updated(app, &state.pool).await?;
    Ok(())
}

async fn resume_all_downloads(app: tauri::AppHandle, state: &AppState) -> Result<(), String> {
    let rows = db::list_downloads(&state.pool)
        .await
        .map_err(|e| e.to_string())?;
    for row in rows {
        if row.status != "paused" {
            continue;
        }

        let request = request_from_row(&row);

        if let Err(error) = start_or_queue_download(
            app.clone(),
            state.pool.clone(),
            state.active.clone(),
            state.cancelled.clone(),
            state.settings.clone(),
            request,
            Some(row.id.clone()),
        )
        .await
        {
            eprintln!("failed to resume download {}: {}", row.id, error);
        }
    }

    Ok(())
}

fn show_main_window(app: &tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window was not found".to_string())?;
    window.show().map_err(|e| e.to_string())?;
    window.unminimize().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let pause_all = MenuItem::with_id(app, "tray-pause-all", "Pause All", false, None::<&str>)?;
    let resume_all = MenuItem::with_id(app, "tray-resume-all", "Resume All", false, None::<&str>)?;
    let open_dashboard = MenuItem::with_id(
        app,
        "tray-open-dashboard",
        "Open Dashboard",
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "tray-quit", "Quit", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[&pause_all, &resume_all, &separator, &open_dashboard, &quit],
    )?;

    app.manage(TrayMenuState {
        pause_all: pause_all.clone(),
        resume_all: resume_all.clone(),
    });

    let mut tray = TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("Khukri")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "tray-pause-all" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let state = app.state::<AppState>();
                    let _ = pause_all_downloads(&app, &state).await;
                });
            }
            "tray-resume-all" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let state = app.state::<AppState>();
                    let _ = resume_all_downloads(app.clone(), &state).await;
                });
            }
            "tray-open-dashboard" => {
                let _ = show_main_window(app);
            }
            "tray-quit" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let state = app.state::<AppState>();
                    state.quitting.store(true, Ordering::SeqCst);
                    let _ = pause_all_downloads(&app, &state).await;
                    app.exit(0);
                });
            }
            _ => {}
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        tray = tray.icon(icon);
    }

    tray.build(app)?;
    Ok(())
}

#[tauri::command]
async fn get_queue(state: State<'_, AppState>) -> Result<Vec<DownloadListItem>, String> {
    db::list_downloads(&state.pool)
        .await
        .map(|rows| rows.into_iter().map(map_download_row).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn start_download(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: StartDownloadRequest,
) -> Result<DownloadListItem, String> {
    start_or_queue_download(
        app,
        state.pool.clone(),
        state.active.clone(),
        state.cancelled.clone(),
        state.settings.clone(),
        request,
        None,
    )
    .await
}

#[tauri::command]
async fn pause_download(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let handle = {
        let active = state.active.lock().await;
        active.get(&id).cloned()
    };

    if let Some(download) = handle {
        download.task.cancel().await;
        db::set_download_status(&state.pool, &id, "paused")
            .await
            .map_err(|e| e.to_string())?;
        let _ = wait_until_inactive(&state.active, &id, TASK_SETTLE_TIMEOUT).await;
        let _ = emit_queue_updated(&app, &state.pool).await;
        return Ok(());
    }

    db::set_download_status(&state.pool, &id, "paused")
        .await
        .map_err(|e| e.to_string())?;
    let _ = emit_queue_updated(&app, &state.pool).await;
    Ok(())
}

#[tauri::command]
async fn resume_download(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<DownloadListItem, String> {
    let mut row = db::get_download(&state.pool, &id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("download not found: {id}"))?;

    if state.active.lock().await.contains_key(&id) {
        if row.status != "paused" {
            return Ok(map_download_row(row));
        }

        if !wait_until_inactive(&state.active, &id, TASK_SETTLE_TIMEOUT).await {
            return Err("download is still pausing; try resume again in a moment".to_string());
        }

        row = db::get_download(&state.pool, &id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("download not found: {id}"))?;
    }

    if row.status == "active" || row.status == "queued" {
        return Ok(map_download_row(row));
    }

    if row.status != "paused" {
        return Err(format!(
            "only paused downloads can be resumed: {}",
            row.status
        ));
    }

    let request = request_from_row(&row);

    start_or_queue_download(
        app,
        state.pool.clone(),
        state.active.clone(),
        state.cancelled.clone(),
        state.settings.clone(),
        request,
        Some(row.id.clone()),
    )
    .await
}

#[tauri::command]
async fn cancel_download(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let handle = {
        let active = state.active.lock().await;
        active.get(&id).cloned()
    };

    if let Some(download) = handle {
        state.cancelled.lock().await.insert(id.clone());
        download.task.cancel().await;
        return Ok(());
    }

    db::set_download_cancelled(&state.pool, &id)
        .await
        .map_err(|e| e.to_string())?;
    cleanup_download_file_for_id(&state.pool, &id).await?;
    let _ = emit_queue_updated(&app, &state.pool).await;
    Ok(())
}

#[tauri::command]
async fn remove_download(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    if state.active.lock().await.contains_key(&id) {
        return Err(format!("cannot remove active download: {id}"));
    }

    cleanup_download_file_for_id(&state.pool, &id).await?;
    db::delete_download(&state.pool, &id)
        .await
        .map_err(|e| e.to_string())?;
    let _ = emit_queue_updated(&app, &state.pool).await;
    Ok(())
}

#[tauri::command]
async fn pump_queue(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    promote_pending_downloads(&app, &state).await
}

#[tauri::command]
async fn open_download_folder(file_path: String) -> Result<(), String> {
    let candidate = PathBuf::from(file_path);
    if !candidate.exists() {
        return Err(format!(
            "downloaded file is missing: {}",
            candidate.display()
        ));
    }

    let target = if candidate.is_dir() {
        candidate
    } else {
        candidate
            .parent()
            .map(|parent| parent.to_path_buf())
            .ok_or_else(|| "download path has no parent directory".to_string())?
    };

    if !target.exists() {
        return Err(format!("folder does not exist: {}", target.display()));
    }

    open_path_in_explorer(&target)
}

#[tauri::command]
async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    Ok(state.settings.lock().await.clone())
}

#[tauri::command]
async fn update_settings(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<AppSettings, String> {
    save_settings_to_disk(&settings)?;
    {
        let mut current = state.settings.lock().await;
        *current = settings.clone();
    }
    let _ = app.emit("settings-updated", &settings);
    let _ = promote_pending_downloads(&app, &state).await;
    Ok(settings)
}

#[tauri::command]
async fn acknowledge_media_onboarding(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<AppSettings, String> {
    let next_settings = {
        let mut current = state.settings.lock().await;
        current.onboarding_complete = true;
        current.clone()
    };

    save_settings_to_disk(&next_settings)?;
    let _ = app.emit("settings-updated", &next_settings);
    Ok(next_settings)
}

#[tauri::command]
async fn check_ytdlp_updates_now(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    maybe_update_ytdlp(app, state.settings.clone(), true).await
}

#[tauri::command]
async fn pick_folder(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let path = app.dialog().file().blocking_pick_folder();
    Ok(path.map(|p| p.to_string()))
}

#[tauri::command]
async fn reset_settings_section(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    section: String,
) -> Result<AppSettings, String> {
    let defaults = AppSettings::default();
    let next_settings = {
        let mut current = state.settings.lock().await;
        match section.as_str() {
            "general" => current.general = defaults.general,
            "performance" => current.performance = defaults.performance,
            "scheduler" => current.scheduler = defaults.scheduler,
            "proxy" => current.proxy = defaults.proxy,
            "appearance" => current.appearance = defaults.appearance,
            _ => return Err(format!("unknown settings section: {section}")),
        }

        current.clone()
    };

    save_settings_to_disk(&next_settings)?;
    let _ = app.emit("settings-updated", &next_settings);
    let _ = promote_pending_downloads(&app, &state).await;
    Ok(next_settings)
}

fn app_pool_config() -> DbConfig {
    let data_dir = app_data_dir();
    let _ = std::fs::create_dir_all(&data_dir);
    DbConfig {
        url: format!("sqlite:{}?mode=rwc", data_dir.join("state.db").display()),
        max_connections: 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings_with_default_dir(default_dir: PathBuf) -> AppSettings {
        let mut settings = AppSettings::default();
        settings.general.default_download_path = default_dir.display().to_string();
        settings
    }

    fn tray_item(status: &str) -> DownloadListItem {
        DownloadListItem {
            id: format!("test-{status}"),
            url: "https://example.com/file.bin".to_string(),
            file_path: "/tmp/file.bin".to_string(),
            file_exists: false,
            total_bytes: None,
            status: status.to_string(),
            priority: "normal".to_string(),
            throttle_bytes_per_sec: None,
            media_quality: None,
            request_source: None,
            failure_reason: None,
            created_at: 0,
        }
    }

    #[test]
    fn empty_output_uses_default_directory_and_url_filename() {
        let default_dir = std::env::temp_dir().join("khukri-default-output");
        let settings = settings_with_default_dir(default_dir.clone());

        let path = normalize_output_path(
            "",
            &settings,
            "https://example.com/files/sample.bin?token=abc",
        );

        assert_eq!(path, default_dir.join("sample.bin"));
    }

    #[test]
    fn explicit_default_directory_value_is_treated_as_directory() {
        let default_dir = std::env::temp_dir().join("khukri-default-output");
        let settings = settings_with_default_dir(default_dir.clone());

        let path = normalize_output_path(
            &settings.general.default_download_path,
            &settings,
            "https://example.com/files/sample.bin",
        );

        assert_eq!(path, default_dir.join("sample.bin"));
    }

    #[test]
    fn output_file_exists_tracks_deleted_download_files() {
        let path = std::env::temp_dir().join(format!("khukri-file-exists-{}", Uuid::new_v4()));
        std::fs::write(&path, b"done").unwrap();

        assert!(output_file_exists(&path.display().to_string()));

        std::fs::remove_file(&path).unwrap();
        assert!(!output_file_exists(&path.display().to_string()));
    }

    #[tokio::test]
    async fn open_download_folder_reports_missing_file() {
        let path = std::env::temp_dir().join(format!("khukri-missing-file-{}", Uuid::new_v4()));
        let error = open_download_folder(path.display().to_string())
            .await
            .unwrap_err();

        assert!(error.contains("downloaded file is missing"));
    }

    #[test]
    fn tray_action_state_tracks_pause_and_resume_availability() {
        assert_eq!(tray_action_state(&[]), (false, false));
        assert_eq!(tray_action_state(&[tray_item("active")]), (true, false));
        assert_eq!(tray_action_state(&[tray_item("queued")]), (true, false));
        assert_eq!(tray_action_state(&[tray_item("paused")]), (false, true));
        assert_eq!(
            tray_action_state(&[
                tray_item("complete"),
                tray_item("failed"),
                tray_item("cancelled")
            ]),
            (false, false)
        );
        assert_eq!(
            tray_action_state(&[tray_item("active"), tray_item("paused")]),
            (true, true)
        );
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tauri::async_runtime::block_on(async move {
        let pool = init_db(&app_pool_config())
            .await
            .expect("failed to initialize Khukri app database");
        db::run_migrations(&pool)
            .await
            .expect("failed to run Khukri migrations");

        sqlx::query("UPDATE downloads SET status = 'paused' WHERE status = 'active'")
            .execute(&pool)
            .await
            .expect("failed to reset stale active downloads");

        let settings = load_settings_from_disk();
        save_settings_to_disk(&settings).expect("failed to save Khukri app settings");

        tauri::Builder::default()
            .manage(AppState {
                pool,
                active: Arc::new(Mutex::new(HashMap::new())),
                cancelled: Arc::new(Mutex::new(HashSet::new())),
                settings: Arc::new(Mutex::new(settings)),
                quitting: Arc::new(AtomicBool::new(false)),
            })
            .setup(|app| {
                setup_tray(app)?;
                tauri::async_runtime::spawn(async move {
                    log_ffmpeg_version().await;
                });
                let app_handle = app.handle().clone();
                let settings = app.state::<AppState>().settings.clone();
                spawn_background_updater(app_handle, settings);
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let state = app_handle.state::<AppState>();
                    let queue = db::list_downloads(&state.pool)
                        .await
                        .map(|rows| rows.into_iter().map(map_download_row).collect::<Vec<_>>())
                        .unwrap_or_default();
                    let _ = sync_tray_menu_state(&app_handle, &queue);
                });
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let state = app_handle.state::<AppState>();
                    let _ = promote_pending_downloads(&app_handle, &state).await;
                });
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    loop {
                        tokio::time::sleep(SCHEDULER_POLL_INTERVAL).await;
                        let state = app_handle.state::<AppState>();
                        if state.quitting.load(Ordering::SeqCst) {
                            break;
                        }
                        let _ = promote_pending_downloads(&app_handle, &state).await;
                    }
                });
                Ok(())
            })
            .plugin(tauri_plugin_dialog::init())
            .invoke_handler(tauri::generate_handler![
                get_queue,
                start_download,
                pause_download,
                resume_download,
                cancel_download,
                remove_download,
                pump_queue,
                open_download_folder,
                get_settings,
                update_settings,
                acknowledge_media_onboarding,
                check_ytdlp_updates_now,
                reset_settings_section,
                pick_folder
            ])
            .run(tauri::generate_context!())
            .expect("error while running tauri application");
    });
}

fn main() {
    run();
}
