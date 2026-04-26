# Packaging Strategy

Date: 2026-04-26
Scope: Size-aware packaging plan for the desktop app, `yt-dlp`, and FFmpeg sidecars

## Goal

Ship a desktop build that feels zero-config to end users without letting media sidecars bloat the installer unnecessarily.

## Size Targets

- Sprint 3 desktop app without media sidecars: aim for well under `100 MB`
- Sprint 4 desktop app with bundled `yt-dlp`: still aim for under `100 MB`
- Full media-capable install with FFmpeg available: acceptable target is `140 MB` to `230 MB`, depending on the FFmpeg payload

## Current Working Assumptions

- `yt-dlp` is relatively cheap to bundle and should ship with the app
- FFmpeg is the dominant size contributor
- A full FFmpeg bundle is harder to justify than a minimal or on-demand strategy

## Decision

Use a hybrid packaging strategy:

1. Bundle `yt-dlp` with the app by default
2. Do not bundle a large full FFmpeg distribution
3. Prefer either:
   - a minimal bundled FFmpeg payload containing only the binaries Khukri actually needs, or
   - first-use FFmpeg download if the size target for the base installer must stay low

## Recommended Default

For the first media-enabled release:

- Bundle pinned `yt-dlp`
- Bundle only the smallest FFmpeg subset Khukri needs for merge/stitch flows
- Include `ffmpeg` and `ffprobe`, but avoid shipping extra tools or a "full" FFmpeg package
- If the minimal bundled FFmpeg still pushes the installer too high, switch FFmpeg to first-use download while keeping `yt-dlp` bundled

## Why This Direction

### Bundle `yt-dlp`

`yt-dlp` is small enough to ship and is core to the media feature. Requiring the user to install it separately would break the "works out of the box" promise.

### Keep FFmpeg Minimal

FFmpeg is the size risk. The project should avoid bundling a large generic distribution when Khukri only needs a narrow subset of capabilities for media merging.

### Preserve a Lean Base Installer

If Khukri wants to stay close to a lightweight installer experience, FFmpeg is the right component to move out of the base package first, not `yt-dlp`.

## Packaging Options

### Option A — Bundle both `yt-dlp` and minimal FFmpeg

Best for:
- zero-config user experience
- offline-ready media support immediately after install

Tradeoff:
- larger installer

Expected range:
- roughly `140 MB` to `180 MB`

### Option B — Bundle `yt-dlp`, download FFmpeg on first use

Best for:
- smaller base installer
- keeping the main app closer to a lightweight footprint

Tradeoff:
- first merge/stitch flow needs a one-time download
- more updater/install logic

Expected range:
- base installer stays much smaller than a full media bundle

### Option C — Bundle both `yt-dlp` and a standard "essentials" FFmpeg package

Best for:
- fastest implementation path

Tradeoff:
- likely bigger than necessary
- easier to drift above the desired installer size

Expected range:
- roughly `170 MB` to `230 MB`

## Non-Goals

- Bundling a full FFmpeg distribution by default
- Requiring users to install `yt-dlp` manually
- Optimizing for the smallest possible installer at the expense of a confusing setup flow

## Platform Notes

- Windows is the primary size-sensitive path because that is the most likely target for mainstream users
- macOS and Linux packaging may need different sidecar layouts, but the same size principles should hold
- Sidecar packaging should be validated early in CI so platform-specific bundle drift does not go unnoticed

## Implementation Guidance

- Keep `yt-dlp` as a pinned sidecar with the updater strategy documented in `docs/adr-001-ytdlp-update-strategy.md`
- Treat FFmpeg as a separate packaging concern with its own size budget
- If FFmpeg is downloaded on first use, store it in the app data directory and version it explicitly
- Log FFmpeg version at startup or first use for debugging and auditability
- Document clearly in release notes whether FFmpeg is bundled or downloaded on demand

## Recommended Sprint 4 Decision

Start with this order:

1. Bundle pinned `yt-dlp`
2. Prototype with minimal FFmpeg only
3. Measure packaged installer size on Windows
4. If the installer is too large, move FFmpeg to first-use download

## Bottom Line

Khukri should bundle `yt-dlp` by default and treat FFmpeg as the size control lever. The safest packaging strategy is "bundle `yt-dlp`, keep FFmpeg minimal, and fall back to first-use FFmpeg download if the installer grows too large."
