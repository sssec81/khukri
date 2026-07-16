# Contributing to Khukri

Thanks for taking the time to work on Khukri. Keep changes focused, test the path you touched, and leave the repository easier to understand than you found it.

## Development setup

The workspace contains three main runtime pieces:

- `khukri-engine`: download and persistence logic
- `khukri-bridge`: Chromium native messaging host
- `khukri-app`: Tauri desktop application

The frontend and extension use plain JavaScript, HTML, and CSS. Avoid adding a frontend build system unless the change clearly needs one.

Build and test from the repository root:

```bash
cargo build --workspace
cargo test --workspace
```

For desktop development:

```bash
cargo tauri dev
```

## Expectations

- Do not add telemetry or background network requests unrelated to downloads and update checks.
- Keep user-facing strings in `src/i18n/en.json`.
- Pin `yt-dlp` releases and verify checksums before replacing a managed binary.
- Use atomic replacement for downloaded sidecars and other managed executables.
- Keep native messaging stdout limited to framed protocol messages; logs belong on stderr.
- Treat browser-provided filenames, paths, URLs, and headers as untrusted input.
- Preserve cancellation behavior when changing download or child-process lifecycles.

## Important paths

Application state is stored in `$APP_DATA/khukri/state.db`. Set `KHUKRI_DATA_DIR` when a test or development run should not touch normal application data.

The Chromium native messaging host is named `com.khukri.host`. Messages use a four-byte little-endian length followed by a UTF-8 JSON body.

Media binaries can be overridden during development:

```text
KHUKRI_YTDLP_BIN
KHUKRI_FFMPEG_BIN
KHUKRI_YTDLP_JS_RUNTIME
```

## Tests

Add a regression test when fixing engine, bridge, persistence, or media-selection behavior. Extension changes should at least pass JavaScript syntax checks:

```bash
node --check extension/service-worker.js
node --check extension/content-script.js
node --check extension/blade-ui.js
node --check extension/prompt.js
```

Before opening a pull request, run:

```bash
cargo fmt --check
cargo test --workspace
git diff --check
```

If a platform-specific path cannot be tested locally, call that out in the pull request instead of implying it was verified.

## Pull requests

Use a short title that describes the behavior being changed. In the description, include:

- the problem
- the approach taken
- how it was tested
- any remaining platform or packaging caveats

Avoid mixing formatting sweeps or unrelated cleanup into a functional change.
