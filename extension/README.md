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
- Requires the Khukri Native Messaging host to be registered as `com.khukri.host`
- For YouTube and similar SPAs, the pill overlay is re-injected after navigation changes
- The current dev workflow clears `dismissed_sites` on extension install/startup so the pill is easy to re-test after reloads

## UI/UX Highlights
- Pill overlay uses Gurkha Green, Tiger Amber, glassmorphism, and an IDM-style player-adjacent layout
- Appears after a 1.5 second delay without shifting page layout
- Dismisses per-origin using `chrome.storage.local` during a session, with dev-time reset on install/startup
- Blade exposes a hover quality picker with per-origin persistence in `chrome.storage.local`
- Blade clicks queue a native download through the service worker with the selected media quality

## Sprint 2 - KHU-201-KHU-205 Acceptance Criteria
- [x] `manifest.json` targets MV3 with correct permissions
- [x] Service worker intercepts `onCreated` and cancels browser download to hand-off
- [x] Active bridge sessions use `chrome.runtime.connectNative()` (long-lived port)
- [x] Service worker observes stream patterns and content script provides blob/video fallback
- [x] Pill overlay is robust, premium, and context-aware
- [x] Blade UI matches the current Sprint 2 reviewed behavior for delay, dismissal, and player-adjacent placement
