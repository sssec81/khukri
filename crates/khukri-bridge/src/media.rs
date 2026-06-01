use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::{fs, io::Read};

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, watch};
use tokio::time::{timeout, Duration};

const PROGRESS_PREFIX: &str = "__KHUKRI_PROGRESS__:";
const FINAL_PATH_PREFIX: &str = "__KHUKRI_FINAL_PATH__:";
const YTDLP_TIMEOUT: Duration = Duration::from_secs(3600);

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
pub struct YtDlpJob {
    pub id: String,
    pub url: String,
    pub output_path: PathBuf,
    pub quality: MediaQuality,
    pub headers: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct YtDlpProgress {
    pub phase: String,
    pub bytes_done: u64,
    pub total_bytes: Option<u64>,
    pub speed_bps: u64,
    pub eta_seconds: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct YtDlpOutcome {
    pub final_path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct RawProgress {
    status: String,
    downloaded_bytes: Option<String>,
    total_bytes: Option<String>,
    total_bytes_estimate: Option<String>,
    speed: Option<String>,
    eta: Option<String>,
}

pub fn should_use_ytdlp(source: Option<&str>, quality: Option<&str>) -> bool {
    quality.is_some() || matches!(source, Some("blade") | Some("stream"))
}

pub fn build_arguments(job: &YtDlpJob) -> Vec<String> {
    let ffmpeg_binary = resolve_ffmpeg_binary().ok();
    build_arguments_with_ffmpeg(job, ffmpeg_binary.as_deref())
}

fn build_arguments_with_ffmpeg(job: &YtDlpJob, ffmpeg_binary: Option<&Path>) -> Vec<String> {
    let mut args = vec![
        "--no-config".to_string(),
        "--no-playlist".to_string(),
        "--newline".to_string(),
        "--progress".to_string(),
        "--progress-template".to_string(),
        format!(
            "download:{PROGRESS_PREFIX}{{\"status\":\"%(progress.status)s\",\"downloaded_bytes\":\"%(progress.downloaded_bytes)s\",\"total_bytes\":\"%(progress.total_bytes)s\",\"total_bytes_estimate\":\"%(progress.total_bytes_estimate)s\",\"speed\":\"%(progress.speed)s\",\"eta\":\"%(progress.eta)s\"}}"
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

pub fn parse_progress_line(line: &str) -> Option<YtDlpProgress> {
    let payload = line.trim().strip_prefix(PROGRESS_PREFIX)?;
    let raw: RawProgress = serde_json::from_str(payload).ok()?;
    Some(YtDlpProgress {
        phase: raw.status,
        bytes_done: parse_u64_like(raw.downloaded_bytes).unwrap_or(0),
        total_bytes: parse_u64_like(raw.total_bytes)
            .or_else(|| parse_u64_like(raw.total_bytes_estimate)),
        speed_bps: parse_u64_like(raw.speed).unwrap_or(0),
        eta_seconds: parse_u64_like(raw.eta),
    })
}

pub fn parse_final_path_line(line: &str) -> Option<PathBuf> {
    let payload = line.trim().strip_prefix(FINAL_PATH_PREFIX)?;
    let path: String = serde_json::from_str(payload).ok()?;
    Some(PathBuf::from(path))
}

pub fn resolve_ytdlp_binary() -> Result<PathBuf> {
    if let Some(explicit) = std::env::var_os("KHUKRI_YTDLP_BIN") {
        let path = PathBuf::from(explicit);
        if path.exists() {
            return Ok(path);
        }
        bail!("KHUKRI_YTDLP_BIN does not exist: {}", path.display());
    }

    let exe = std::env::current_exe().context("failed to resolve current bridge executable")?;
    let sidecar_name = platform_sidecar_name()?;
    let mut candidates = Vec::new();
    candidates.push(app_data_dir().join("sidecar").join(sidecar_name));

    if let Some(dir) = exe.parent() {
        candidates.push(dir.join(sidecar_name));
        candidates.push(dir.join("sidecar").join(sidecar_name));
        if let Some(target_dir) = dir.parent() {
            candidates.push(target_dir.join(sidecar_name));
            candidates.push(target_dir.join("sidecar").join(sidecar_name));
            if let Some(repo_root) = target_dir.parent() {
                candidates.push(repo_root.join("sidecar").join(sidecar_name));
            }
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("sidecar").join(sidecar_name));
    }

    candidates
        .into_iter()
        .find(|candidate| candidate.exists())
        .map(|candidate| candidate.canonicalize().unwrap_or(candidate))
        .ok_or_else(|| {
            anyhow::anyhow!("could not find bundled yt-dlp sidecar for {}", sidecar_name)
        })
}

pub fn resolve_ffmpeg_binary() -> Result<PathBuf> {
    if let Some(explicit) = std::env::var_os("KHUKRI_FFMPEG_BIN") {
        let path = PathBuf::from(explicit);
        if path.exists() {
            return Ok(path);
        }
        bail!("KHUKRI_FFMPEG_BIN does not exist: {}", path.display());
    }

    let exe = std::env::current_exe().context("failed to resolve current bridge executable")?;
    let sidecar_name = platform_ffmpeg_name()?;
    let mut candidates = Vec::new();
    candidates.push(app_data_dir().join("sidecar").join(sidecar_name));

    if let Some(dir) = exe.parent() {
        candidates.push(dir.join(sidecar_name));
        candidates.push(dir.join("sidecar").join(sidecar_name));
        if let Some(target_dir) = dir.parent() {
            candidates.push(target_dir.join(sidecar_name));
            candidates.push(target_dir.join("sidecar").join(sidecar_name));
            if let Some(repo_root) = target_dir.parent() {
                candidates.push(repo_root.join("sidecar").join(sidecar_name));
            }
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("sidecar").join(sidecar_name));
    }

    candidates
        .into_iter()
        .find(|candidate| candidate.exists())
        .map(|candidate| candidate.canonicalize().unwrap_or(candidate))
        .ok_or_else(|| {
            anyhow::anyhow!("could not find bundled ffmpeg sidecar for {}", sidecar_name)
        })
}

fn build_ytdlp_command(path: &Path) -> Result<Command> {
    #[cfg(target_os = "macos")]
    {
        if file_starts_with_shebang(path)? {
            let python = resolve_python3_path()?;
            tracing::info!(binary = %path.display(), interpreter = %python.display(), "bridge launching yt-dlp script via python3");
            let mut command = Command::new(python);
            command.arg(path);
            return Ok(command);
        }
    }

    tracing::info!(binary = %path.display(), "bridge launching yt-dlp executable directly");
    Ok(Command::new(path))
}

#[allow(dead_code)]
pub async fn run_ytdlp<F>(job: YtDlpJob, mut on_progress: F) -> Result<YtDlpOutcome>
where
    F: FnMut(YtDlpProgress) + Send,
{
    let (_cancel_tx, cancel_rx) = watch::channel(false);
    run_ytdlp_with_cancel(job, &mut on_progress, cancel_rx).await
}

pub async fn run_ytdlp_with_cancel<F>(
    job: YtDlpJob,
    on_progress: &mut F,
    mut cancel_rx: watch::Receiver<bool>,
) -> Result<YtDlpOutcome>
where
    F: FnMut(YtDlpProgress) + Send,
{
    let binary = resolve_ytdlp_binary()?;
    tracing::info!(binary = %binary.display(), url = %job.url, quality = ?job.quality, "bridge starting yt-dlp job");
    let mut command = build_ytdlp_command(&binary)?;
    let mut child = command
        .args(build_arguments(&job))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to spawn yt-dlp at {}", binary.display()))?;

    let stdout = child.stdout.take().context("yt-dlp stdout pipe missing")?;
    let stderr = child.stderr.take().context("yt-dlp stderr pipe missing")?;
    let (line_tx, mut line_rx) = mpsc::unbounded_channel::<String>();

    let stdout_task = tokio::spawn(read_lines(stdout, line_tx.clone()));
    let stderr_task = tokio::spawn(read_lines(stderr, line_tx));

    let mut final_path = None::<PathBuf>;
    let mut last_detail = None::<String>;
    let drain = async {
        loop {
            tokio::select! {
                changed = cancel_rx.changed() => {
                    if changed.is_ok() && *cancel_rx.borrow() {
                        return Err(anyhow::anyhow!("yt-dlp job cancelled by Khukri"));
                    }
                    if changed.is_err() {
                        return Err(anyhow::anyhow!("yt-dlp cancellation channel closed"));
                    }
                }
                line = line_rx.recv() => {
                    let Some(line) = line else {
                        return Ok(());
                    };

                    if let Some(progress) = parse_progress_line(&line) {
                        on_progress(progress);
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
            }
        }
    };

    match timeout(YTDLP_TIMEOUT, drain).await {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            let _ = child.kill().await;
            return Err(error);
        }
        Err(_) => {
            let _ = child.kill().await;
            bail!(
                "yt-dlp job timed out after {} seconds",
                YTDLP_TIMEOUT.as_secs()
            );
        }
    }

    let status = child.wait().await.context("failed to wait for yt-dlp")?;
    stdout_task.await.context("stdout reader task failed")??;
    stderr_task.await.context("stderr reader task failed")??;

    if !status.success() {
        let reason = format_media_failure(&status.to_string(), last_detail.as_deref());
        tracing::warn!(binary = %binary.display(), %reason, "bridge yt-dlp job failed");
        bail!("{}", reason);
    }

    let final_path = final_path.unwrap_or_else(|| expected_output_path(&job.output_path));
    if !final_path.is_file() {
        bail!(
            "yt-dlp finished but final merged file was not found: {}",
            final_path.display()
        );
    }
    tracing::info!(binary = %binary.display(), final_path = %final_path.display(), "bridge yt-dlp job completed");
    Ok(YtDlpOutcome { final_path })
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

fn file_starts_with_shebang(path: &Path) -> Result<bool> {
    let mut file =
        fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut buf = [0u8; 2];
    let read = file
        .read(&mut buf)
        .with_context(|| format!("failed to read {}", path.display()))?;
    Ok(read == 2 && buf == [b'#', b'!'])
}

#[cfg(target_os = "macos")]
fn resolve_python3_path() -> Result<PathBuf> {
    if let Some(explicit) = std::env::var_os("KHUKRI_PYTHON3_BIN") {
        let path = PathBuf::from(explicit);
        if path.exists() {
            return Ok(path);
        }
        bail!("KHUKRI_PYTHON3_BIN does not exist: {}", path.display());
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

    bail!("python3 not found for macOS yt-dlp script execution; set KHUKRI_PYTHON3_BIN")
}

fn format_media_failure(status: &str, detail: Option<&str>) -> String {
    match detail {
        Some(detail) if !detail.is_empty() => format!("yt-dlp failed ({status}): {detail}"),
        _ => format!("yt-dlp exited with status {status}"),
    }
}

async fn read_lines<R>(reader: R, tx: mpsc::UnboundedSender<String>) -> Result<()>
where
    R: AsyncRead + Unpin,
{
    let mut lines = BufReader::new(reader).lines();
    while let Some(line) = lines.next_line().await? {
        let _ = tx.send(line);
    }
    Ok(())
}

fn platform_sidecar_name() -> Result<&'static str> {
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
        bail!("yt-dlp sidecar is not configured for this target triple")
    }
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

fn platform_ffmpeg_name() -> Result<&'static str> {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        return Ok("ffmpeg-x86_64-pc-windows-msvc.exe");
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        return Ok("ffmpeg-x86_64-unknown-linux-gnu");
    }

    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        return Ok("ffmpeg-x86_64-apple-darwin");
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        return Ok("ffmpeg-aarch64-apple-darwin");
    }

    #[cfg(not(any(
        all(target_os = "windows", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    {
        bail!("ffmpeg sidecar is not configured for this target triple")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_job(quality: MediaQuality) -> YtDlpJob {
        YtDlpJob {
            id: "job-1".to_string(),
            url: "https://example.com/watch?v=abc".to_string(),
            output_path: PathBuf::from("D:/downloads/sample.bin"),
            quality,
            headers: vec![("Referer".to_string(), "https://example.com".to_string())],
        }
    }

    #[test]
    fn quality_defaults_to_best() {
        assert_eq!(MediaQuality::parse(None), MediaQuality::Best);
        assert_eq!(MediaQuality::parse(Some("unknown")), MediaQuality::Best);
    }

    #[test]
    fn quality_specific_arguments_are_built() {
        let audio_args = build_arguments(&sample_job(MediaQuality::AudioOnly));
        assert!(audio_args.contains(&"--no-playlist".to_string()));
        assert!(audio_args.windows(2).any(|part| part[0] == "--paths"
            && part[1].starts_with("home:")
            && part[1].contains("downloads")));
        assert!(audio_args.windows(2).any(|part| part[0] == "--paths"
            && part[1].starts_with("temp:")
            && part[1].contains("media-temp")));
        assert!(audio_args
            .windows(2)
            .any(|part| part == ["-x", "--audio-format"]));
        assert!(audio_args.contains(&"mp3".to_string()));

        let hd_args = build_arguments_with_ffmpeg(
            &sample_job(MediaQuality::P1080),
            Some(Path::new("/tmp/ffmpeg")),
        );
        assert!(hd_args.contains(
            &"bestvideo[vcodec^=avc1][height<=1080]+bestaudio[ext=m4a]/bestvideo[ext=mp4][height<=1080]+bestaudio[ext=m4a]/bestvideo[height<=1080]+bestaudio/best[height<=1080][acodec!=none]/best".to_string()
        ));
        assert!(hd_args
            .windows(2)
            .any(|part| part == ["--ffmpeg-location", "/tmp/ffmpeg"]));
    }

    #[test]
    fn progress_json_lines_parse() {
        let progress = parse_progress_line(
            "__KHUKRI_PROGRESS__:{\"status\":\"downloading\",\"downloaded_bytes\":\"1048576\",\"total_bytes\":\"2097152\",\"speed\":\"524288\",\"eta\":\"2\"}",
        )
        .expect("progress should parse");

        assert_eq!(progress.phase, "downloading");
        assert_eq!(progress.bytes_done, 1_048_576);
        assert_eq!(progress.total_bytes, Some(2_097_152));
        assert_eq!(progress.speed_bps, 524_288);
        assert_eq!(progress.eta_seconds, Some(2));
    }

    #[test]
    fn progress_json_uses_total_bytes_estimate_fallback() {
        let progress = parse_progress_line(
            "__KHUKRI_PROGRESS__:{\"status\":\"downloading\",\"downloaded_bytes\":\"1048576\",\"total_bytes\":\"NA\",\"total_bytes_estimate\":\"2097152\",\"speed\":\"524288\",\"eta\":\"2\"}",
        )
        .expect("progress should parse");

        assert_eq!(progress.total_bytes, Some(2_097_152));
    }

    #[test]
    fn final_path_lines_parse() {
        let path = parse_final_path_line("__KHUKRI_FINAL_PATH__:\"D:\\\\downloads\\\\sample.mp4\"")
            .expect("path should parse");
        assert_eq!(path, PathBuf::from("D:\\downloads\\sample.mp4"));
    }

    #[test]
    fn stream_and_blade_sources_use_ytdlp() {
        assert!(should_use_ytdlp(Some("blade"), None));
        assert!(should_use_ytdlp(Some("stream"), None));
        assert!(should_use_ytdlp(Some("browser"), Some("720p")));
        assert!(!should_use_ytdlp(Some("browser"), None));
    }

    #[test]
    fn selector_falls_back_to_progressive_when_ffmpeg_missing() {
        assert_eq!(
            format_selector(MediaQuality::Best, false),
            "best[acodec!=none][vcodec!=none]/best"
        );
        assert_eq!(
            format_selector(MediaQuality::P1080, false),
            "best[height<=1080][acodec!=none][vcodec!=none]/best[height<=1080]/best"
        );
    }
}
