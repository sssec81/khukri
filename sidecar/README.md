# Sidecar Assets

This directory holds bundled Sprint 4 media sidecars.

## yt-dlp baseline

- Pinned release: `2026.03.17`
- Upstream release page: <https://github.com/yt-dlp/yt-dlp/releases/tag/2026.03.17>
- Upstream asset naming mapped into Tauri target-triple filenames so `bundle.externalBin` can package the correct binary per target.

## Files

- `yt-dlp-x86_64-pc-windows-msvc.exe` <- upstream `yt-dlp.exe`
- `yt-dlp-x86_64-unknown-linux-gnu` <- upstream `yt-dlp_linux`
- `yt-dlp-x86_64-apple-darwin` <- upstream `yt-dlp_macos`
- `yt-dlp-aarch64-apple-darwin` <- upstream `yt-dlp_macos`

The macOS upstream binary is universal, so the same upstream asset is duplicated under both Apple target triples expected by Tauri builds.

## FFmpeg contract

FFmpeg sidecars are not committed yet, but Sprint 4 code now looks for these filenames when present:

- `ffmpeg-x86_64-pc-windows-msvc.exe`
- `ffmpeg-x86_64-unknown-linux-gnu`
- `ffmpeg-x86_64-apple-darwin`
- `ffmpeg-aarch64-apple-darwin`

You can override discovery during development with:

- `KHUKRI_YTDLP_BIN`
- `KHUKRI_FFMPEG_BIN`

## Managed updates

When the desktop app updater is enabled, Khukri stores runtime-managed yt-dlp updates under:

- `$KHUKRI_DATA_DIR/sidecar/`

These managed binaries are preferred over the bundled baseline sidecars at runtime.
