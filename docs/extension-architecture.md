# Extension Architecture

This document gives AI-assisted sessions one place to understand the current MV3 extension message flow without re-deriving it from the code each time.

## Runtime boundaries

- `extension/service-worker.js`: Manifest V3 service worker. Owns download interception, prompt routing, retry queue state, stream memory, and native messaging.
- `extension/content-script.js`: isolated-world bridge between page messages and extension messaging.
- `extension/content-script-main.js`: MAIN-world fetch/XHR observer for stream discovery.
- `extension/blade-ui.js`: in-page YouTube blade UI and quality selection.
- `extension/prompt.html` + `extension/prompt.js`: ask-mode prompt window.

## Message flows

### Browser download interception

`chrome.downloads.onCreated` in the service worker:

1. Cancels the browser download synchronously.
2. Loads the intercept mode from storage.
3. In `ask` mode, stores a prompt payload in `chrome.storage.session` and opens `prompt.html`.
4. In `auto` mode, sends the normalized payload directly to the native bridge.

### Ask-mode prompt

`prompt.js`:

1. Opens a keepalive port immediately so the service worker stays awake while the dialog is visible.
2. Reads the token from the query string.
3. Requests the stored payload via runtime messaging.
4. Renders filename, size, and URL details.
5. Sends the chosen action back to the service worker.
6. Closes the prompt window.

If the service worker is unreachable during the decision step, the session-storage retry queue is responsible for eventual recovery on the next worker wake-up.

### YouTube blade pill

`blade-ui.js` click flow:

1. Sends `type: 'queue_download'` with `source: 'blade'`.
2. The service worker waits briefly for a usable remembered stream candidate.
3. If a `videoplayback`, `m3u8`, or `mpd` URL is available, that URL is handed to the native bridge.
4. If no usable stream is ready, the page URL is sent instead so `yt-dlp` can resolve it.

### Stream detection

MAIN-world capture:

1. `content-script-main.js` patches page-owned `fetch` and `XMLHttpRequest`.
2. Matching media requests are posted onto `window`.
3. `content-script.js` receives those page messages in the isolated world.
4. The isolated script forwards `stream_detected` to the service worker.
5. The service worker keeps the best available candidate per tab.

Service-worker capture:

1. `chrome.webRequest.onBeforeRequest` also watches for `videoplayback`, `m3u8`, and `mpd` URLs.
2. Matching requests are scored and stored alongside content-script discoveries.

## Storage keys

- `intercept_mode`: `'ask' | 'auto'`
- `quality_preferences`: per-origin selected media quality
- `dismissed_sites`: per-origin dismissal expirations stored in `chrome.storage.local` with a 7-day TTL
- `khukri_prompt_<token>`: transient prompt payload stored in `chrome.storage.session`
- `khukri_retry_queue`: transient retry queue stored in `chrome.storage.session`

## Known limitation

- Chrome/Brave can ignore `type: 'popup'` when `prompt.html` is opened from the MV3 service worker, causing the prompt to appear as a normal tab.
