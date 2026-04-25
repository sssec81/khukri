# Sprint 4 Status

Date: 2026-04-25
Scope: EPIC-04 - The Scabbard

Overall status: Planned

## Goal

One-click YouTube and stream downloading from the browser using bundled `yt-dlp` and FFmpeg, without requiring the user to install extra tools manually.

## Board

| Ticket | Title | Status | Notes |
|---|---|---|---|
| KHU-401 | Bundle pinned yt-dlp sidecar binary | Planned | First blocking task for the sprint. Needs pinned version file, checksums, and Tauri packaging wiring. |
| KHU-402 | yt-dlp invocation from Rust + quality selection | Planned | Core execution path. Depends on KHU-401 and establishes the Rust-side job model and progress events. |
| KHU-403 | yt-dlp auto-updater | Planned | Should land after sidecar bundling and basic invocation are stable so update logic can target a known-good baseline. |
| KHU-404 | Bundle minimal FFmpeg for stream stitching | Planned | Required for merged video/audio outputs from yt-dlp. Packaging, licensing, and binary size need careful validation. |
| KHU-405 | Quality selector in Floating Blade UI | Planned | Browser-side UX for choosing `best`, `1080p`, `720p`, or `audio-only`. Depends on KHU-402 defining the supported `quality` contract. |
| KHU-406 | Legal/ToS notice in onboarding | Planned | One-time onboarding gate for media features. Can be implemented in parallel with KHU-405 once final text is confirmed from the PRD. |

## Recommended Order

1. KHU-401 - bundle and pin `yt-dlp`
2. KHU-402 - invoke `yt-dlp` from Rust and expose progress
3. KHU-404 - add FFmpeg stitching for split media outputs
4. KHU-405 - add the quality selector to the Blade UI
5. KHU-406 - add the onboarding/legal notice
6. KHU-403 - ship the updater after the baseline binary path is proven stable

## Definition Of Done

- User can trigger a YouTube download from the browser-facing Blade UI
- `best`, `1080p`, `720p`, and `audio-only` flows are supported
- Progress is visible in the desktop shell
- Audio/video split downloads can be stitched automatically when needed
- The app does not require the user to install `yt-dlp` or FFmpeg manually
- Auto-updater runs in the background without blocking the UI or breaking active downloads

## Dependencies

- Sprint 2 browser extension and bridge are already in place and provide the handoff path
- Sprint 3 desktop shell is nearly complete and can host media queue/progress UX
- Tauri sidecar packaging must be configured for platform-specific binaries
- FFmpeg licensing and packaging constraints must remain GPL-compatible

## Risks To Watch

- `yt-dlp` extractor drift can break media flows quickly, so pinning and updater rollback behavior matter
- Progress parsing needs to be resilient to output format variation across `yt-dlp` versions
- Platform packaging for sidecars can easily diverge between Windows, macOS, and Linux if not tested early
- FFmpeg binary size can bloat quickly if the build is not kept minimal
- Legal/onboarding wording must stay aligned with PRD Section 5C

## Open Questions

- Where should the sidecar binaries live in the packaged app on each target OS?
- Should media jobs reuse the existing download row model or get a media-specific subtype/status surface?
- How should failed stitching be presented in the UI: single failed row or separate download/stitch phases?
- Should the updater be automatic by default or opt-in from Settings?
