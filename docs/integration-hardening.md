# Integration Hardening Notes

These notes capture likely integration-tier failure modes as Khukri moves from a standalone engine to Browser -> Bridge -> Engine -> UI.

## Potential Integration Hurdles

### 1. The Zombies Problem (Process Management)

Risk:
- In Sprint 2, the Native Messaging bridge is spawned by Chrome.
- If Chrome is force-closed or the bridge crashes, downloads can keep running unexpectedly and hold file or SQLite resources.

Mitigation:
- Add a heartbeat between bridge and engine.
- If IPC is severed for more than N seconds, cancel download tasks and persist paused state.
- Add an integration test that kills the parent process mid-download and validates resume readiness.

### 2. The Authentication Wall (Session Hand-off)

Risk:
- A URL that works in browser can fail in engine with 403 if request context is incomplete.
- Cookies, User-Agent, and Referer often must match browser state.

Mitigation:
- Bridge must pass a structured header set (CustomHeaders) to the engine.
- Preserve origin and referer context in bridge payloads.
- Add negative tests for 403 fallback and explicit surfaced errors.

Note:
- Some sites also enforce IP pinning or TLS fingerprint checks; full browser parity is not always possible with reqwest alone.

### 3. SQLite Locked Errors (Multi-writer pressure)

Risk:
- Bridge and UI can issue writes while engine is persisting segment progress.
- SQLite supports one writer at a time; busy errors can appear under contention.

Mitigation:
- Enable WAL mode at DB initialization.
- Set a busy timeout and keep transactions short.
- Prefer a single write path for high-frequency progress writes.

### 4. Native Messaging Path Friction (Windows/Linux)

Risk:
- Registration paths are absolute. If binaries move, host registration breaks silently.

Mitigation:
- KHU-202 self-installer must support register/repair flows.
- Recompute absolute path on each repair run and rewrite host manifest/registry key.

## Technical Blind Spots to Monitor

| Component | Potential Issue | Why It Matters |
|---|---|---|
| Throttling | Buffer bloat at very low caps | Small caps can increase latency and destabilize long transfers |
| Disk I/O | SQLite write amplification | Frequent tiny DB updates increase write pressure and overhead |
| yt-dlp | Version drift | Site extractor changes can break media flow quickly |

## Recommended Bridge Stress Test

Before shipping extension UX, run a bridge-first stress scenario:

1. Trigger a multi-GB download through the Native Messaging bridge.
2. Kill Chrome/bridge mid-transfer.
3. Verify engine transitions to paused/failed state predictably.
4. Relaunch bridge and confirm resume only fetches incomplete segments.

Passing this test early reduces uncertainty before Sprint 3 UI integration.

## MV3 Bridge Notes

Chrome MV3 service workers are ephemeral by design. That matters here because Khukri's browser side is not a general web app; it is a coordinator for a local native process.

### Service Worker Keepalive

Risk:
- MV3 service workers can suspend after inactivity, so a local WebSocket bridge can be dropped when the worker sleeps.
- A reconnect loop adds extra state and failure modes without giving Khukri anything the native bridge actually needs.

Mitigation:
- Prefer Native Messaging over a localhost WebSocket server for Sprint 2.
- Use `chrome.runtime.connectNative()` so the browser owns the port lifecycle and the native process remains tied to the active connection.
- If long-running coordination is ever needed outside the native port, evaluate an offscreen document as a fallback only after the native path proves insufficient.

### Native Messaging vs WebSockets

| Feature | WebSockets | Native Messaging |
|---|---|---|
| Persistence | Can drop when the SW suspends | Tied to the native port lifecycle |
| Security | Requires a local server and exposed port handling | OS-level bridge with no localhost port exposure |
| Protocol | JSON over TCP | JSON framed with a 4-byte length header over stdin/stdout |
| Installation | Just the extension | Requires host manifest registration |

### Host Manifest Installation Plan

Khukri should handle host registration with a small binary flag or installer command instead of asking the user to hand-edit files.

Preferred plan:
- Add a `--register` / `--repair` flow to the Rust bridge binary.
- Detect the OS at runtime and write the Native Messaging host manifest to the correct location.
- Recompute the binary's absolute path during repair so moved installs can be fixed without manual cleanup.

This keeps the user flow simple:
- First run: register the host.
- After moving the install: run repair and rewrite the manifest path.
- If the browser/bridge disconnects: rely on the engine's existing cancel/pause path to preserve state.
