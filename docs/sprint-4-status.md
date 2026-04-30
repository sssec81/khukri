# Sprint 4 Status

Date: 2026-04-25
Scope: EPIC-04 - The Scabbard

Overall status: In Progress

## Goal

One-click YouTube and stream downloading from the browser using bundled `yt-dlp` and FFmpeg, without requiring the user to install extra tools manually.

## Board

| Ticket | Title | Status | Notes |
|---|---|---|---|
| KHU-401 | Bundle pinned yt-dlp sidecar binary | Complete | Pinned to `2026.03.17`; `sidecar/yt-dlp.version`, `sidecar/yt-dlp.sha256`, Tauri `externalBin`, and platform binaries are now in repo. |
| KHU-402 | yt-dlp invocation from Rust + quality selection | In Progress | Native bridge and desktop shell now route Blade/stream jobs through a `yt-dlp` job model with quality mapping, sidecar resolution, parsed progress events, richer failure reasons, and safer no-FFmpeg fallback selectors. End-to-end validation and final cancel/resume polish are still open. |
| KHU-403 | yt-dlp auto-updater | In Progress | Desktop settings now expose a `ytdlp_auto_update` toggle plus manual check action, and the Tauri app has a background GitHub Releases worker that downloads, checksum-verifies, canary-checks, and hot-swaps managed yt-dlp sidecars in app data. On-device validation and user-facing notification polish are still open. |
| KHU-404 | Bundle minimal FFmpeg for stream stitching | In Progress | Media invocations now look for a platform FFmpeg sidecar or `KHUKRI_FFMPEG_BIN`, pass `--ffmpeg-location` into `yt-dlp`, and log `ffmpeg -version` at desktop startup when present. Shipping the binaries and validating package size are still open. |
| KHU-405 | Quality selector in Floating Blade UI | In Progress | Blade UI now exposes a hover quality picker with per-site persistence in `chrome.storage.local`, and the selected value is forwarded to the native queue request. Browser-side polish and end-to-end validation are still pending. |
| KHU-406 | Legal/ToS notice in onboarding | In Progress | A blocking desktop onboarding notice now persists `settings.json:onboarding_complete` and requires an explicit `I Understand` acknowledgment. Accessibility and exact once-only behavior still need validation on-device. |

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
- Size-aware sidecar planning is documented in `docs/packaging-strategy.md`

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

## Related Docs

- `docs/packaging-strategy.md`
- `docs/adr-001-ytdlp-update-strategy.md`

## Implementation Notes

- Tauri sidecar packaging is wired through `src-tauri/tauri.conf.json` with `bundle.externalBin = ["../sidecar/yt-dlp"]`.
- The repo now carries Tauri target-triple filenames for Windows x64, Linux x64, and both Apple triples.
- The macOS files both point to the same upstream universal `yt-dlp_macos` asset because Tauri resolves sidecars by build target triple.
