# Sprint 3 Status

Date: 2026-04-26
Scope: EPIC-03 - The Handle

Overall status: Near Complete

## Board

| Ticket | Title | Status | Notes |
|---|---|---|---|
| KHU-301 | Tauri 2.0 app scaffold + engine integration | Near Complete | `src-tauri/` is wired into the workspace as `khukri-app`; queue, download lifecycle, settings, and folder-open commands exist. `cargo test --workspace` passes on native Windows, `cargo tauri dev` launches on native Windows, and both `cargo check -p khukri-engine` and `cargo check -p khukri-app` pass in Ubuntu 24.04 / WSL. |
| KHU-302 | "All Downloads" list view with progress bars | Near Complete | Downloads list UI, progress bars, speed/ETA display, keyboard navigation, and row actions are implemented in `src/`. Pause/resume UI is optimistic, failed-download inline reason display is wired end-to-end, and queue rendering escapes user-controlled values. |
| KHU-303 | Settings panel | Near Complete | General, Performance, Scheduler, Proxy, and Appearance sections are implemented. Settings persist to the app data directory, apply immediately, and section reset now repopulates the form and reapplies theme/defaults. |
| KHU-304 | System tray integration | In Progress | Tray menu, hide-to-tray close behavior, pause all, resume all, open dashboard, and quit flows are implemented. The current icon is still a placeholder, and Pause All / Resume All do not yet toggle enabled state dynamically. |
| KHU-305 | Dark mode + light mode theming | Near Complete | Dark, light, and system-following theme modes are present in the frontend and persist through settings. Formal contrast verification is still pending. |

## Verification

- `cargo check -p khukri-engine` passed on Ubuntu 24.04 / WSL on 2026-04-25
- `cargo check -p khukri-app` passed on Ubuntu 24.04 / WSL on 2026-04-25
- `cargo check -p khukri-engine` passed again on Ubuntu / WSL on 2026-04-26
- `cargo test --workspace` passed on native Windows on 2026-04-26: `khukri-bridge` 12 unit tests + 1 integration test, `khukri-engine` 29 unit tests + 6 integration tests, `khukri-app` 0 unit tests
- `cargo tauri dev` launched successfully on native Windows on 2026-04-26
- `cargo test -p khukri-engine` passed on Ubuntu / WSL on 2026-04-26: 17 unit tests and 6 integration tests
- `cargo test -p khukri-bridge` passed on Ubuntu / WSL on 2026-04-26: 1 integration test
- `cargo test -p khukri-app` was started on Ubuntu / WSL on 2026-04-26 but not completed, so no fresh app-test result is recorded here
- `cargo tauri info` reported a valid Rust/Tauri toolchain after installing:
- `libgtk-3-dev`
- `libwebkit2gtk-4.1-dev`
- `librsvg2-dev`
- `cargo tauri dev` launched successfully on Ubuntu 24.04 / WSL on 2026-04-25

## Current Shape

- Tauri backend lives in `src-tauri/`
- Frontend shell lives in `src/`
- Frontend localization now uses a single source of truth at `src/i18n/en.json`
- Queue lifecycle now covers start, pause, resume, cancel, remove, and failed-start surfacing
- Scheduler gating, proxy-aware requests, WAL mode, resumable progress seeding, and file cleanup on cancel/remove are implemented

## Security & Correctness Fixes (2026-04-26)

The following items from the code-review were resolved in a single pass:

| Item | Resolution |
|---|---|
| Unused `fd` warning in `prealloc.rs` | Removed unused `let fd = file.as_raw_fd()` — warning cleared |
| Bridge forwards dangerous headers | `browser_headers()` now strips `Host`, `Content-Length`, `Connection`, `Authorization`, `Transfer-Encoding`, and other hop-by-hop headers before passing to engine |
| `allowed_origins` placeholder is a release blocker | `extension_origin_from_env()` now returns `Result` and calls `validate_extension_origin()` — registration (`--register`) bails with a clear message if the env var is unset or still contains the placeholder |
| macOS native host registration unimplemented | Added `#[cfg(target_os = "macos")]` impl writing to `~/Library/Application Support/Google/Chrome/NativeMessagingHosts/` |
| `max_threads_by_size` was a no-op | `resolved_thread_count()` now enforces a 1 MiB minimum segment size: `max_by_min_segment = (total_bytes / 1MiB).clamp(1, 64)` |
| Resume silently restarts after formula change | Added `SEGMENT_FORMULA_VERSION = 2` constant; version stored in DB (migration 007); `segmented_download` forces a fresh start when stored version doesn't match |
| Missing tests | Added unit tests for `resolved_thread_count`, `can_reuse_segments`, `browser_headers`, `validate_extension_origin`, `sanitize_filename`, `filename_from_url`, and three `preallocate` scenarios (success, zero-byte, read-only failure) |

## Remaining Gaps

- Native Windows shell verification is now complete for build, test, and app launch
- Native Windows browser-extension handoff, registry registration flow, and one real download should still be walked end to end
- Windows cold-start and RAM budget targets have not been measured yet
- Tray/menu state is functional, but dynamic enable/disable behavior for Pause All vs Resume All is not implemented yet
- The current icon asset is a temporary placeholder, not final brand artwork
- `Open Folder` is environment-sensitive under WSL/Linux desktop-less setups and is not reliable there
- A final QA pass is still needed for startup-failure messaging and a few edge-case UI transitions
- `KHUKRI_EXTENSION_ORIGIN` env var is set by the registration scripts automatically — no manual step required
