# Khukri Chrome Extension (MV3)

## Purpose
Intercepts browser downloads and hands them off to the Khukri Native Messaging bridge for high-performance segmented downloading. Adds a premium, context-aware pill overlay for video downloads.

## Key Files
- `manifest.json`: MV3 manifest with `downloads`, `nativeMessaging`, `storage`, and `webRequest` permissions
- `service-worker.js`: Intercepts downloads, observes stream requests, and keeps a long-lived native host connection
- `content-script.js`: Blob/video fallback detector that forwards page context to the service worker
- `blade-ui.js`: Injects a premium pill overlay near the active player for video downloads (IDM-style, SPA-safe)
- `com.khukri.host.json`: Native messaging host manifest template

## Development
- Load this folder as an unpacked extension in Chrome
- The committed public key gives unpacked beta builds the stable extension ID `hlingdbecfefhglkbballggindegcmik`
- Packaged macOS beta builds register `com.khukri.host` automatically when Khukri starts
- Source-tree development builds can use `register-host.sh` or `register-host.ps1`
- For YouTube and similar SPAs, the pill overlay is re-injected after navigation changes
- Blade dismissals now persist in `chrome.storage.local` with a 7-day TTL per origin
- Current stable interception modes are `auto` and the current `ask` prompt flow
- If you see `Extension context invalidated` after reloads, close stale tabs and test from a fresh tab/session

## UI/UX Highlights
- Pill and prompt share the desktop app's warm-charcoal and indigo visual system
- Green is reserved for ready, successful, and completed states
- Appears after a 1.5 second delay without shifting page layout
- Dismisses per-origin using `chrome.storage.local` with a 7-day TTL
- Blade exposes a hover quality picker with per-origin persistence in `chrome.storage.local`
- Blade clicks queue a native download through the service worker with the selected media quality
- Opening the ask-mode prompt suppresses the blade pill so the two surfaces do not overlap

## Sprint 2 - KHU-201-KHU-205 Acceptance Criteria
- [x] `manifest.json` targets MV3 with correct permissions
- [x] Service worker intercepts `onCreated` and cancels browser download to hand-off
- [x] Active bridge sessions use `chrome.runtime.connectNative()` (long-lived port)
- [x] Service worker observes stream patterns and content script provides blob/video fallback
- [x] Pill overlay is robust, premium, and context-aware
- [x] Blade UI matches the current Sprint 2 reviewed behavior for delay, dismissal, and player-adjacent placement

## Current Intercept Modes
- `auto`: stable; browser download is canceled and handed to Khukri when bridge is available
- `ask`: working; shows a `Start in Khukri` / `Keep in Browser` prompt before handoff, with the remaining caveat that Chrome may open the prompt as a normal tab instead of a popup
