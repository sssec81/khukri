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

FFmpeg binaries are generated locally and ignored by Git because of their
size. `ffmpeg.version` records the pinned sources and checksums. Fetch only the
macOS beta assets with:

```bash
bash sidecar/fetch-ffmpeg.sh macos
```

The runtime looks for these target-triple filenames:

- `ffmpeg-x86_64-pc-windows-msvc.exe`
- `ffmpeg-x86_64-unknown-linux-gnu`
- `ffmpeg-x86_64-apple-darwin`
- `ffmpeg-aarch64-apple-darwin`

The current macOS FFmpeg 8.1.2 asset is x86_64. The aarch64 filename contains
the same binary and runs through Rosetta 2 for the free beta. Replace it with a
native, checksum-pinned ARM build before a stable Apple Silicon release.

## Native bridge

`scripts/build-macos-beta.sh` compiles `khukri-bridge` and copies it to the
target-triple sidecar filename expected by Tauri. Generated bridge binaries are
ignored by Git.

You can override discovery during development with:

- `KHUKRI_YTDLP_BIN`
- `KHUKRI_FFMPEG_BIN`

## Managed updates

When the desktop app updater is enabled, Khukri stores runtime-managed yt-dlp updates under:

- `$KHUKRI_DATA_DIR/sidecar/`

These managed binaries are preferred over the bundled baseline sidecars at runtime.
