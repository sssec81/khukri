use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
#[cfg(target_os = "macos")]
use std::sync::{Mutex as StdMutex, OnceLock};
use std::{fs, io::Read};

use khukri_engine::{DownloadProgress, DownloadStatus};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, watch, Mutex};
use tokio_util::sync::CancellationToken;

use crate::bootstrap::app_data_dir;

const PROGRESS_PREFIX: &str = "__KHUKRI_PROGRESS__:";
const FINAL_PATH_PREFIX: &str = "__KHUKRI_FINAL_PATH__:";
#[cfg(target_os = "macos")]
static PREPARED_SIDECARS: OnceLock<StdMutex<std::collections::HashSet<PathBuf>>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaQuality {
    Best,
    P1080,
    P720,
    AudioOnly,
}

impl MediaQuality {
    pub fn parse(raw: Option<&str>) -> Self {
        match raw.unwrap_or("best").trim().to_ascii_lowercase().as_str() {
            "1080p" => Self::P1080,
            "720p" => Self::P720,
            "audio-only" | "audio_only" | "audio" => Self::AudioOnly,
            _ => Self::Best,
        }
    }

    pub fn format_selector(self) -> &'static str {
        match self {
            Self::Best => "bestvideo[vcodec^=avc1]+bestaudio[ext=m4a]/bestvideo[ext=mp4]+bestaudio[ext=m4a]/bestvideo+bestaudio/best",
            Self::P1080 => {
                "bestvideo[vcodec^=avc1][height<=1080]+bestaudio[ext=m4a]/bestvideo[ext=mp4][height<=1080]+bestaudio[ext=m4a]/bestvideo[height<=1080]+bestaudio/best[height<=1080][acodec!=none]/best"
            }
            Self::P720 => "bestvideo[vcodec^=avc1][height<=720]+bestaudio[ext=m4a]/bestvideo[ext=mp4][height<=720]+bestaudio[ext=m4a]/bestvideo[height<=720]+bestaudio/best[height<=720][acodec!=none]/best",
            Self::AudioOnly => "bestaudio/best",
        }
    }
}

#[derive(Debug, Clone)]
pub struct MediaJob {
    pub id: String,
    pub url: String,
    pub output_path: PathBuf,
    pub quality: MediaQuality,
    pub headers: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
struct ParsedProgress {
    phase: String,
    bytes_done: u64,
    total_bytes: Option<u64>,
    speed_bps: u64,
    eta_seconds: Option<u64>,
}

#[derive(Debug, serde::Deserialize)]
struct RawProgress {
    status: String,
    downloaded_bytes: Option<String>,
    total_bytes: Option<String>,
    speed: Option<String>,
    eta: Option<String>,
}

#[derive(Debug, Default)]
struct MediaRunState {
    final_path: Option<PathBuf>,
    failure_reason: Option<String>,
}

pub struct MediaDownloadHandle {
    cancel: CancellationToken,
    progress: watch::Receiver<DownloadProgress>,
    child: Arc<Mutex<Option<Child>>>,
    state: Arc<Mutex<MediaRunState>>,
}

impl MediaDownloadHandle {
    pub fn subscribe(&self) -> watch::Receiver<DownloadProgress> {
        self.progress.clone()
    }

    pub fn cancel(&self) {
        self.cancel.cancel();
        let child = self.child.clone();
        tauri::async_runtime::spawn(async move {
            if let Some(child) = child.lock().await.as_mut() {
                let _ = child.kill().await;
            }
        });
    }

    pub async fn final_path(&self) -> Option<PathBuf> {
        self.state.lock().await.final_path.clone()
    }

    pub async fn failure_reason(&self) -> Option<String> {
        self.state.lock().await.failure_reason.clone()
    }
}

pub fn spawn_media_download(job: MediaJob) -> MediaDownloadHandle {
    let (tx, rx) = watch::channel(DownloadProgress {
        id: job.id.clone(),
        status: DownloadStatus::Queued,
        bytes_done: 0,
        total_bytes: None,
        speed_bps: 0,
        eta_seconds: None,
        segments_done: 0,
        segments_total: None,
    });
    let cancel = CancellationToken::new();
    let child = Arc::new(Mutex::new(None));
    let state = Arc::new(Mutex::new(MediaRunState::default()));
    tokio::spawn(run_media_download(
        job.clone(),
        tx,
        cancel.clone(),
        child.clone(),
        state.clone(),
    ));

    MediaDownloadHandle {
        cancel,
        progress: rx,
        child,
        state,
    }
}

pub fn ytdlp_path() -> Result<PathBuf, String> {
    if let Some(explicit) = std::env::var_os("KHUKRI_YTDLP_BIN") {
        let path = PathBuf::from(explicit);
        if path.exists() {
            prepare_sidecar_for_execution(&path)?;
            return Ok(path);
        }
        return Err(format!(
            "KHUKRI_YTDLP_BIN does not exist: {}",
            path.display()
        ));
    }

    resolve_sidecar_path(platform_ytdlp_name()?, "KHUKRI_YTDLP_BIN")
}

pub fn ffmpeg_path() -> Option<PathBuf> {
    if let Some(explicit) = std::env::var_os("KHUKRI_FFMPEG_BIN") {
        let path = PathBuf::from(explicit);
        if path.exists() {
            if let Err(error) = prepare_sidecar_for_execution(&path) {
                tracing::warn!(binary = %path.display(), %error, "failed to prepare ffmpeg sidecar");
                return None;
            }
            return Some(path);
        }
        return None;
    }

    resolve_sidecar_path(platform_ffmpeg_name(), "KHUKRI_FFMPEG_BIN").ok()
}

pub async fn log_ffmpeg_version() {
    let Some(binary) = ffmpeg_path() else {
        tracing::info!(
            "ffmpeg sidecar not found; media downloads will rely on yt-dlp without merge support"
        );
        return;
    };

    let output = Command::new(&binary).arg("-version").output().await;

    match output {
        Ok(output) => {
            let line = String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .unwrap_or("ffmpeg version output unavailable")
                .to_string();
            tracing::info!(binary = %binary.display(), version = %line, "ffmpeg sidecar detected");
        }
        Err(error) => {
            tracing::warn!(binary = %binary.display(), %error, "failed to inspect ffmpeg sidecar version");
        }
    }
}

fn resolve_sidecar_path(name: &str, env_name: &str) -> Result<PathBuf, String> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let candidates = [
        app_data_dir().join("sidecar").join(name),
        cwd.join("sidecar").join(name),
        cwd.join("src-tauri").join("..").join("sidecar").join(name),
    ];

    candidates
        .into_iter()
        .find(|path| path.exists())
        .map(|path| path.canonicalize().unwrap_or(path))
        .ok_or_else(|| format!("could not find sidecar {name}; override with {env_name}"))
}

pub fn build_ytdlp_command(path: &Path) -> Result<Command, String> {
    #[cfg(target_os = "macos")]
    {
        if file_starts_with_shebang(path)? {
            let python = resolve_python3_path()?;
            tracing::info!(binary = %path.display(), interpreter = %python.display(), "launching yt-dlp script via python3");
            let mut command = Command::new(python);
            command.arg(path);
            return Ok(command);
        }
    }

    tracing::info!(binary = %path.display(), "launching yt-dlp executable directly");
    let command = Command::new(path);
    Ok(command)
}

pub fn prepare_sidecar_for_execution(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let prepared =
            PREPARED_SIDECARS.get_or_init(|| StdMutex::new(std::collections::HashSet::new()));

        {
            let guard = prepared
                .lock()
                .map_err(|_| "failed to lock prepared-sidecar cache".to_string())?;
            if guard.contains(&canonical) {
                return Ok(());
            }
        }

        best_effort_strip_quarantine(&canonical)?;
        if is_macho_binary(&canonical)? {
            ad_hoc_codesign(&canonical)?;
        }

        let mut guard = prepared
            .lock()
            .map_err(|_| "failed to lock prepared-sidecar cache".to_string())?;
        guard.insert(canonical);
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
    }

    Ok(())
}

fn file_starts_with_shebang(path: &Path) -> Result<bool, String> {
    let mut file =
        fs::File::open(path).map_err(|e| format!("failed to open {}: {e}", path.display()))?;
    let mut buf = [0u8; 2];
    let read = file
        .read(&mut buf)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    Ok(read == 2 && buf == [b'#', b'!'])
}

fn is_macho_binary(path: &Path) -> Result<bool, String> {
    let mut file =
        fs::File::open(path).map_err(|e| format!("failed to open {}: {e}", path.display()))?;
    let mut buf = [0u8; 4];
    let read = file
        .read(&mut buf)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    if read < 4 {
        return Ok(false);
    }

    Ok(matches!(
        buf,
        [0xfe, 0xed, 0xfa, 0xce]
            | [0xce, 0xfa, 0xed, 0xfe]
            | [0xfe, 0xed, 0xfa, 0xcf]
            | [0xcf, 0xfa, 0xed, 0xfe]
            | [0xca, 0xfe, 0xba, 0xbe]
            | [0xbe, 0xba, 0xfe, 0xca]
            | [0xca, 0xfe, 0xba, 0xbf]
            | [0xbf, 0xba, 0xfe, 0xca]
    ))
}

#[cfg(target_os = "macos")]
fn best_effort_strip_quarantine(path: &Path) -> Result<(), String> {
    let status = std::process::Command::new("/usr/bin/xattr")
        .args(["-dr", "com.apple.quarantine"])
        .arg(path)
        .status()
        .map_err(|e| format!("failed to run xattr for {}: {e}", path.display()))?;

    if status.success() {
        return Ok(());
    }

    Err(format!(
        "xattr returned non-zero status {} for {}",
        status,
        path.display()
    ))
}

#[cfg(target_os = "macos")]
fn ad_hoc_codesign(path: &Path) -> Result<(), String> {
    let status = std::process::Command::new("/usr/bin/codesign")
        .args(["--force", "--deep", "--sign", "-"])
        .arg(path)
        .status()
        .map_err(|e| format!("failed to run codesign for {}: {e}", path.display()))?;

    if status.success() {
        return Ok(());
    }

    Err(format!(
        "codesign returned non-zero status {} for {}",
        status,
        path.display()
    ))
}

#[cfg(target_os = "macos")]
fn resolve_python3_path() -> Result<PathBuf, String> {
    if let Some(explicit) = std::env::var_os("KHUKRI_PYTHON3_BIN") {
        let path = PathBuf::from(explicit);
        if path.exists() {
            return Ok(path);
        }
        return Err(format!(
            "KHUKRI_PYTHON3_BIN does not exist: {}",
            path.display()
        ));
    }

    for candidate in [
        "/opt/homebrew/bin/python3",
        "/usr/local/bin/python3",
        "/usr/bin/python3",
    ] {
        let path = PathBuf::from(candidate);
        if path.exists() {
            return Ok(path);
        }
    }

    Err("python3 not found for macOS yt-dlp script execution; set KHUKRI_PYTHON3_BIN".to_string())
}

async fn run_media_download(
    job: MediaJob,
    tx: watch::Sender<DownloadProgress>,
    cancel: CancellationToken,
    child_slot: Arc<Mutex<Option<Child>>>,
    state: Arc<Mutex<MediaRunState>>,
) -> Result<PathBuf, String> {
    let binary = ytdlp_path()?;
    let ffmpeg_binary = ffmpeg_path();
    tracing::info!(
        binary = %binary.display(),
        ffmpeg = ffmpeg_binary.as_ref().map(|path| path.display().to_string()).unwrap_or_else(|| "none".to_string()),
        url = %job.url,
        quality = ?job.quality,
        "starting yt-dlp media download"
    );
    let mut command = build_ytdlp_command(&binary)?;
    let mut child = command
        .args(build_arguments(&job, ffmpeg_binary.as_deref()))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn yt-dlp at {}: {e}", binary.display()))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "yt-dlp stdout pipe missing".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "yt-dlp stderr pipe missing".to_string())?;

    *child_slot.lock().await = Some(child);

    let (line_tx, mut line_rx) = mpsc::unbounded_channel::<String>();
    let stdout_task = tokio::spawn(read_lines(stdout, line_tx.clone()));
    let stderr_task = tokio::spawn(read_lines(stderr, line_tx));

    set_progress(&tx, |progress| {
        progress.status = DownloadStatus::Active;
    });

    let mut final_path = None::<PathBuf>;
    let mut last_detail = None::<String>;
    loop {
        if cancel.is_cancelled() {
            set_progress(&tx, |progress| {
                progress.status = DownloadStatus::Paused;
                progress.speed_bps = 0;
                progress.eta_seconds = None;
            });
            if let Some(child) = child_slot.lock().await.as_mut() {
                let _ = child.kill().await;
            }
            state.lock().await.failure_reason = Some("yt-dlp download cancelled".to_string());
            let _ = stdout_task.await.map_err(|e| e.to_string())?;
            let _ = stderr_task.await.map_err(|e| e.to_string())?;
            return Err("yt-dlp download cancelled".to_string());
        }

        let Some(line) = line_rx.recv().await else {
            break;
        };

        if let Some(progress) = parse_progress_line(&line) {
            let _phase = progress.phase.clone();
            set_progress(&tx, |current| {
                current.status = DownloadStatus::Active;
                current.bytes_done = progress.bytes_done;
                current.total_bytes = progress.total_bytes;
                current.speed_bps = progress.speed_bps;
                current.eta_seconds = progress.eta_seconds;
            });
            continue;
        }

        if let Some(path) = parse_final_path_line(&line) {
            final_path = Some(path);
            continue;
        }

        if let Some(detail) = parse_detail_line(&line) {
            last_detail = Some(detail);
        }
    }

    let status = {
        let mut guard = child_slot.lock().await;
        let mut child = guard
            .take()
            .ok_or_else(|| "yt-dlp child process disappeared".to_string())?;
        child.wait().await.map_err(|e| e.to_string())?
    };

    stdout_task
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    stderr_task
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;

    if !status.success() {
        set_progress(&tx, |progress| {
            progress.status = DownloadStatus::Failed;
            progress.speed_bps = 0;
            progress.eta_seconds = None;
        });
        let reason = format_media_failure(&status.to_string(), last_detail.as_deref());
        tracing::warn!(binary = %binary.display(), %reason, "yt-dlp media download failed");
        state.lock().await.failure_reason = Some(reason.clone());
        return Err(reason);
    }

    let final_path = final_path.unwrap_or_else(|| expected_output_path(&job.output_path));
    if !final_path.is_file() {
        set_progress(&tx, |progress| {
            progress.status = DownloadStatus::Failed;
            progress.speed_bps = 0;
            progress.eta_seconds = None;
        });
        let reason = format!(
            "yt-dlp finished but final merged file was not found: {}",
            final_path.display()
        );
        state.lock().await.failure_reason = Some(reason.clone());
        return Err(reason);
    }
    tracing::info!(binary = %binary.display(), final_path = %final_path.display(), "yt-dlp media download completed");
    state.lock().await.final_path = Some(final_path.clone());
    set_progress(&tx, |progress| {
        progress.status = DownloadStatus::Complete;
        progress.speed_bps = 0;
        progress.eta_seconds = None;
        if progress.total_bytes.is_some() {
            progress.bytes_done = progress.total_bytes.unwrap_or(progress.bytes_done);
        }
    });
    Ok(final_path)
}

fn build_arguments(job: &MediaJob, ffmpeg_binary: Option<&Path>) -> Vec<String> {
    let mut args = vec![
        "--no-config".to_string(),
        "--no-playlist".to_string(),
        "--newline".to_string(),
        "--progress".to_string(),
        "--progress-template".to_string(),
        format!(
            "download:{PROGRESS_PREFIX}{{\"status\":\"%(progress.status)s\",\"downloaded_bytes\":\"%(progress.downloaded_bytes)s\",\"total_bytes\":\"%(progress.total_bytes)s\",\"speed\":\"%(progress.speed)s\",\"eta\":\"%(progress.eta)s\"}}"
        ),
        "--print".to_string(),
        format!("after_move:{FINAL_PATH_PREFIX}%(filepath)j"),
        "--paths".to_string(),
        format!("home:{}", output_dir(&job.output_path).display()),
        "--paths".to_string(),
        format!("temp:{}", media_temp_dir(&job.id).display()),
        "-o".to_string(),
        output_template(&job.output_path),
        "-f".to_string(),
        format_selector(job.quality, ffmpeg_binary.is_some()).to_string(),
    ];

    if let Some(ffmpeg_binary) = ffmpeg_binary {
        args.push("--ffmpeg-location".to_string());
        args.push(ffmpeg_binary.display().to_string());
        if !matches!(job.quality, MediaQuality::AudioOnly) {
            args.push("--merge-output-format".to_string());
            args.push("mp4".to_string());
        }
    }

    if matches!(job.quality, MediaQuality::AudioOnly) {
        args.push("-x".to_string());
        args.push("--audio-format".to_string());
        args.push("mp3".to_string());
    }

    for (name, value) in &job.headers {
        args.push("--add-header".to_string());
        args.push(format!("{name}:{value}"));
    }

    args.push(job.url.clone());
    args
}

fn output_template(output_path: &Path) -> String {
    let stem = output_path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("download");

    format!("{stem}.%(ext)s")
}

fn output_dir(output_path: &Path) -> PathBuf {
    output_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn expected_output_path(output_path: &Path) -> PathBuf {
    let stem = output_path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("download");

    output_dir(output_path).join(format!("{stem}.mp4"))
}

fn media_temp_dir(job_id: &str) -> PathBuf {
    let path = app_data_dir().join("media-temp").join(job_id);
    let _ = fs::create_dir_all(&path);
    path
}

fn format_selector(quality: MediaQuality, ffmpeg_available: bool) -> &'static str {
    if ffmpeg_available {
        return quality.format_selector();
    }

    match quality {
        MediaQuality::Best => "best[acodec!=none][vcodec!=none]/best",
        MediaQuality::P1080 => {
            "best[height<=1080][acodec!=none][vcodec!=none]/best[height<=1080]/best"
        }
        MediaQuality::P720 => {
            "best[height<=720][acodec!=none][vcodec!=none]/best[height<=720]/best"
        }
        MediaQuality::AudioOnly => "bestaudio/best",
    }
}

fn parse_progress_line(line: &str) -> Option<ParsedProgress> {
    let payload = line.trim().strip_prefix(PROGRESS_PREFIX)?;
    let raw: RawProgress = serde_json::from_str(payload).ok()?;
    Some(ParsedProgress {
        phase: raw.status,
        bytes_done: parse_u64_like(raw.downloaded_bytes).unwrap_or(0),
        total_bytes: parse_u64_like(raw.total_bytes),
        speed_bps: parse_u64_like(raw.speed).unwrap_or(0),
        eta_seconds: parse_u64_like(raw.eta),
    })
}

fn parse_final_path_line(line: &str) -> Option<PathBuf> {
    let payload = line.trim().strip_prefix(FINAL_PATH_PREFIX)?;
    let path: String = serde_json::from_str(payload).ok()?;
    Some(PathBuf::from(path))
}

fn parse_detail_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty()
        || trimmed.starts_with(PROGRESS_PREFIX)
        || trimmed.starts_with(FINAL_PATH_PREFIX)
    {
        return None;
    }

    Some(trimmed.to_string())
}

fn format_media_failure(status: &str, detail: Option<&str>) -> String {
    match detail {
        Some(detail) if !detail.is_empty() => format!("yt-dlp failed ({status}): {detail}"),
        _ => format!("yt-dlp exited with status {status}"),
    }
}

fn parse_u64_like(value: Option<String>) -> Option<u64> {
    let raw = value?.trim().to_string();
    if raw.is_empty() || raw.eq_ignore_ascii_case("na") || raw.eq_ignore_ascii_case("none") {
        return None;
    }

    raw.parse::<u64>().ok().or_else(|| {
        raw.parse::<f64>()
            .ok()
            .map(|value| value.max(0.0).round() as u64)
    })
}

async fn read_lines<R>(reader: R, tx: mpsc::UnboundedSender<String>) -> Result<(), std::io::Error>
where
    R: AsyncRead + Unpin,
{
    let mut lines = BufReader::new(reader).lines();
    while let Some(line) = lines.next_line().await? {
        let _ = tx.send(line);
    }
    Ok(())
}

fn set_progress<F>(tx: &watch::Sender<DownloadProgress>, mut f: F)
where
    F: FnMut(&mut DownloadProgress),
{
    let mut next = tx.borrow().clone();
    f(&mut next);
    let _ = tx.send(next);
}

fn platform_ytdlp_name() -> Result<&'static str, String> {
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
        Err("yt-dlp sidecar is not configured for this target triple".to_string())
    }
}

fn platform_ffmpeg_name() -> &'static str {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        return "ffmpeg-x86_64-pc-windows-msvc.exe";
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        return "ffmpeg-x86_64-unknown-linux-gnu";
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        return "ffmpeg-x86_64-apple-darwin";
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        return "ffmpeg-aarch64-apple-darwin";
    }
    #[cfg(not(any(
        all(target_os = "windows", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    {
        return "ffmpeg-unsupported-target";
    }
}

pub fn should_use_ytdlp(source: Option<&str>, quality: Option<&str>) -> bool {
    quality.is_some() || matches!(source, Some("blade") | Some("stream"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_progress_line() {
        let progress = parse_progress_line(
            "__KHUKRI_PROGRESS__:{\"status\":\"downloading\",\"downloaded_bytes\":\"100\",\"total_bytes\":\"200\",\"speed\":\"50\",\"eta\":\"2\"}",
        )
        .unwrap();
        assert_eq!(progress.phase, "downloading");
        assert_eq!(progress.bytes_done, 100);
        assert_eq!(progress.total_bytes, Some(200));
    }

    #[test]
    fn detects_media_request() {
        assert!(should_use_ytdlp(Some("stream"), None));
        assert!(should_use_ytdlp(None, Some("720p")));
        assert!(!should_use_ytdlp(Some("browser"), None));
    }

    #[test]
    fn selector_falls_back_to_progressive_when_ffmpeg_missing() {
        assert_eq!(
            format_selector(MediaQuality::Best, false),
            "best[acodec!=none][vcodec!=none]/best"
        );
        assert_eq!(
            format_selector(MediaQuality::P720, false),
            "best[height<=720][acodec!=none][vcodec!=none]/best[height<=720]/best"
        );
    }

    #[test]
    fn ffmpeg_override_is_forwarded_to_ytdlp() {
        let temp_dir = std::env::temp_dir().join("khukri-ffmpeg-test");
        let _ = std::fs::create_dir_all(&temp_dir);
        let binary_path = temp_dir.join("ffmpeg.exe");
        let _ = std::fs::write(&binary_path, []);

        let args = build_arguments(
            &MediaJob {
                id: "job-1".to_string(),
                url: "https://example.com/watch?v=abc".to_string(),
                output_path: PathBuf::from("D:/downloads/sample.bin"),
                quality: MediaQuality::Best,
                headers: Vec::new(),
            },
            Some(binary_path.as_path()),
        );

        assert!(args.contains(&"--no-playlist".to_string()));
        assert!(args.windows(2).any(|part| part[0] == "--paths"
            && part[1].starts_with("home:")
            && part[1].contains("downloads")));
        assert!(args.windows(2).any(|part| part[0] == "--paths"
            && part[1].starts_with("temp:")
            && part[1].contains("media-temp")));

        assert!(args
            .windows(2)
            .any(|part| part[0] == "--ffmpeg-location"
                && part[1] == binary_path.display().to_string()));

        let _ = std::fs::remove_file(&binary_path);
        let _ = std::fs::remove_dir(&temp_dir);
    }
}
