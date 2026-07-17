# Codebase Architecture

Khukri is a workspace with one reusable engine and two application processes:
the Tauri desktop app and the browser native-messaging bridge. Browser and
desktop jobs share SQLite state but do not share an in-memory event channel.

## Workspace boundaries

```text
crates/khukri-engine/       download engine and durable queue state
crates/khukri-bridge/       Chrome Native Messaging process
src-tauri/                  desktop process and Tauri commands
src/                        desktop webview UI
extension/                  Manifest V3 browser integration
sidecar/                    pinned media-tool metadata and fetch tooling
scripts/                    repeatable release tasks
```

## Engine

`khukri-engine` contains network transfer behavior and persistence that can be
used without Tauri or Chrome.

- `config.rs`: validated download configuration
- `db/`: migrations and SQLite queries
- `engine/download.rs`: transfer orchestration
- `engine/segment.rs`: segment planning
- `engine/retry.rs`: retry policy
- `engine/throttle.rs`: bandwidth limiting
- `engine/prealloc.rs`: output allocation

UI or browser-specific concepts should not be introduced into this crate.

## Native bridge

The bridge entry point coordinates job lifetimes. Supporting concerns live in
small modules:

- `media.rs`: yt-dlp and FFmpeg execution
- `paths.rs`: platform data and download directories
- `protocol.rs`: native-message schemas and length-prefixed framing
- `registration.rs`: platform host-manifest installation
- `request.rs`: filename normalization and browser-header filtering

Registration mode exits before database or download initialization. Runtime
messages are decoded by `protocol`, normalized by `request`, then routed to the
engine or media runner.

## Desktop app

The Tauri entry point owns queue orchestration and commands exposed to the
webview. Supporting concerns are separated as follows:

- `bootstrap.rs`: SQLite pool initialization
- `media.rs`: desktop-owned media jobs and sidecar discovery
- `native_host.rs`: packaged bridge discovery and repair
- `settings.rs`: settings schema, defaults, and disk persistence
- `ytdlp_updater.rs`: verified managed-sidecar updates

The webview receives serializable queue/settings DTOs; it does not access the
database directly.

## Cross-process progress

The bridge cannot emit Tauri events. Browser-owned media jobs persist live
bytes, total size, speed, and ETA to SQLite. The desktop app hydrates those
values during queue refresh. Desktop-owned jobs also emit Tauri progress events
for lower-latency updates.

## Maintenance rules

- Keep platform path logic in the relevant `paths`, `bootstrap`, or sidecar
  resolver module.
- Keep serialization/framing separate from job execution.
- Put SQL changes behind a numbered migration and a database-level test.
- Keep entry points focused on wiring and lifecycle orchestration.
- Add pure validation tests beside the module that owns the policy.
- Do not duplicate extension IDs or host names without documenting why both
  copies must remain synchronized.
