# Sprint 3 Status

Date: 2026-04-24
Scope: EPIC-03 - The Handle

## Board

| Ticket | Title | Status | Notes |
|---|---|---|---|
| KHU-301 | Tauri 2.0 app scaffold + engine integration | In Progress | `src-tauri/` is wired into the workspace as `khukri-app`; `get_queue`, `start_download`, `pause_download`, `resume_download`, `cancel_download`, settings, and folder-open commands exist. `cargo check -p khukri-app` passes in Ubuntu 24.04 / WSL after Tauri system dependencies are installed, and Linux runtime launch has been visually verified from `cargo tauri dev` screenshots. |
| KHU-302 | "All Downloads" list view with progress bars | In Progress | Downloads list UI, progress bars, speed/ETA display, keyboard navigation, and row actions are implemented in `src/`. Progress events are throttled to 500ms on the Tauri side. Failed-download inline reason display is now wired through SQLite, Tauri, and the frontend queue row rendering. |
| KHU-303 | Settings panel | In Progress | General, Performance, Scheduler, Proxy, and Appearance sections are implemented. Settings persist to the app data directory and update in memory immediately. |
| KHU-304 | System tray integration | In Progress | Tray menu, hide-to-tray close behavior, pause all, resume all, open dashboard, and quit flows are implemented. The current icon is a placeholder asset added so the Tauri build can succeed; branded production icons still need replacement. |
| KHU-305 | Dark mode + light mode theming | In Progress | Dark, light, and system-following theme modes are present in the frontend. Persistence is wired through settings. Formal contrast verification is still pending. |

## Verification

- `cargo check -p khukri-app` passed on Ubuntu 24.04 / WSL on 2026-04-24
- `cargo tauri info` reported a valid Rust/Tauri toolchain after installing:
- `libgtk-3-dev`
- `libwebkit2gtk-4.1-dev`
- `librsvg2-dev`
- `cargo tauri dev` was visually verified from Linux/WSL screenshots on 2026-04-24

## Current Shape

- Tauri backend lives in `src-tauri/`
- Frontend shell lives in `src/`
- Frontend localization now uses a single source of truth at `src/i18n/en.json`

## Remaining Gaps

- Windows runtime verification is still pending
- Windows cold-start and RAM budget targets have not been measured yet
- Tray/menu state is functional, but dynamic enable/disable behavior for Pause All vs Resume All is not implemented yet
- The current icon asset is a temporary placeholder, not final brand artwork
