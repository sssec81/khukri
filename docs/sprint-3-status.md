# Sprint 3 Status

Date: 2026-04-25
Scope: EPIC-03 - The Handle

Overall status: Near Complete

## Board

| Ticket | Title | Status | Notes |
|---|---|---|---|
| KHU-301 | Tauri 2.0 app scaffold + engine integration | Near Complete | `src-tauri/` is wired into the workspace as `khukri-app`; queue, download lifecycle, settings, and folder-open commands exist. `cargo check -p khukri-engine` and `cargo check -p khukri-app` pass in Ubuntu 24.04 / WSL, and `cargo tauri dev` launches successfully. |
| KHU-302 | "All Downloads" list view with progress bars | Near Complete | Downloads list UI, progress bars, speed/ETA display, keyboard navigation, and row actions are implemented in `src/`. Pause/resume UI is optimistic, failed-download inline reason display is wired end-to-end, and queue rendering escapes user-controlled values. |
| KHU-303 | Settings panel | Near Complete | General, Performance, Scheduler, Proxy, and Appearance sections are implemented. Settings persist to the app data directory, apply immediately, and section reset now repopulates the form and reapplies theme/defaults. |
| KHU-304 | System tray integration | In Progress | Tray menu, hide-to-tray close behavior, pause all, resume all, open dashboard, and quit flows are implemented. The current icon is still a placeholder, and Pause All / Resume All do not yet toggle enabled state dynamically. |
| KHU-305 | Dark mode + light mode theming | Near Complete | Dark, light, and system-following theme modes are present in the frontend and persist through settings. Formal contrast verification is still pending. |

## Verification

- `cargo check -p khukri-engine` passed on Ubuntu 24.04 / WSL on 2026-04-25
- `cargo check -p khukri-app` passed on Ubuntu 24.04 / WSL on 2026-04-25
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

## Remaining Gaps

- Windows runtime verification is still pending
- Windows cold-start and RAM budget targets have not been measured yet
- Tray/menu state is functional, but dynamic enable/disable behavior for Pause All vs Resume All is not implemented yet
- The current icon asset is a temporary placeholder, not final brand artwork
- `Open Folder` is environment-sensitive under WSL/Linux desktop-less setups and is not reliable there
- A final QA pass is still needed for startup-failure messaging and a few edge-case UI transitions
