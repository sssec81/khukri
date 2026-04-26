# ADR-001 — yt-dlp Sidecar Update Strategy

**Status:** Accepted  
**Date:** 2026-04-26  
**Deciders:** Khukri core team  
**Sprint scope:** Sprint 4 (KHU-402, KHU-403)

---

## Context

Khukri bundles yt-dlp as a sidecar binary to enable one-click media downloads from YouTube and other streaming platforms. yt-dlp releases frequently — often multiple tagged releases per week — because platform-side changes (YouTube signature algorithm updates, API format changes) can break extraction within hours of a platform deployment.

This creates a tension between three goals:

1. **Freshness** — an out-of-date yt-dlp silently fails on many URLs. Users see a generic download error with no indication that the tool itself is stale.
2. **Stability** — shipping an untested yt-dlp build can introduce regressions: broken extraction for previously working sites, changed CLI flags, or binary ABI differences on certain platforms.
3. **Security** — downloading and executing a new binary without integrity verification opens a supply-chain attack surface.

The v1.0 PRD draft proposed tracking `master` HEAD on a 24-hour cadence. That was removed in v1.1 because shipping HEAD means shipping code that has never been tagged, tested against the release matrix, or given a stable checksum. This ADR documents the replacement strategy and the tradeoffs considered.

---

## Decision

**Bundle a pinned tagged release at build time. Run a background update check every 24 hours against the yt-dlp GitHub Releases API. On a new tagged release: download the platform binary, verify SHA-256 against the release manifest, and hot-swap atomically. On any failure, retain the last known good binary.**

The full update flow is described in the Implementation section below.

---

## Alternatives Considered

### Option A — Never update (fully static bundle)

Ship one yt-dlp version at build time and never touch it. Users get a new yt-dlp only when they upgrade Khukri itself.

**Rejected.** yt-dlp's update cadence is driven by platform changes outside our control. A static bundle would be broken for many URLs within weeks of a Khukri release. This would make the media download feature unreliable and force frequent Khukri releases just to carry yt-dlp bumps.

### Option B — Track `master` HEAD every 24 hours

Download the latest commit artifact from the yt-dlp CI on a daily timer.

**Rejected.** Artifacts from `master` HEAD have no stable checksum published by the yt-dlp project. We cannot verify integrity against a known-good hash. HEAD also includes in-flight work that has not been tested across all platform configurations. The v1.0 PRD contained this approach; it was explicitly removed in v1.1 for these reasons.

### Option C — Pin to a specific release hash forever, manual bumps only

Update `sidecar/yt-dlp.version` and `sidecar/yt-dlp.sha256` in source control whenever the team decides to bump, with no runtime auto-update.

**Partially accepted as the fallback state.** The committed version file serves as the baseline binary that ships with each Khukri installer. However, requiring a Khukri release for every yt-dlp bump is too slow given yt-dlp's cadence. The auto-update layer (this decision) handles in-field updates between Khukri releases.

### Option D — Delegate to system-installed yt-dlp

Detect a system yt-dlp (`which yt-dlp`) and use it instead of bundling.

**Rejected for the primary path.** Most end users do not have yt-dlp installed. Requiring a separate install step breaks the "zero configuration" promise. Accepted as a developer override: if `KHUKRI_YTDLP_BIN` is set, Khukri uses that path and skips sidecar management entirely.

### Option E — Ship only tagged releases with a 7-day delay after tagging

Wait one week after a yt-dlp tag before offering it as an update, to allow community testing.

**Considered and rejected.** A one-week delay means a platform-breaking change can leave users with a broken feature for up to seven days. The yt-dlp project's own `--update` flag does not apply a delay. The integrity controls in this ADR (SHA-256 verification, canary check before swap) provide sufficient safety without an artificial delay.

---

## Implementation

### Build-time baseline

- `sidecar/yt-dlp.version` — the pinned release tag (e.g., `2026.04.30`) committed to source control.
- `sidecar/yt-dlp.sha256` — the expected SHA-256 hash of the platform-specific binary for this tag, committed to source control. One file covers the build platform; the full hash matrix for all three OSes is encoded in the CI build matrix.
- The Tauri build process (`tauri.conf.json` `externalBin`) bundles the binary at the path corresponding to the current build target.

### Runtime update check

Runs in a `tokio::spawn`ed background task, not on the main async executor, so it cannot block UI or engine operations.

1. **Interval** — check every 24 hours after application start. The timestamp of the last check is persisted in `settings.json` so restarts do not re-check immediately.

2. **Release discovery** — call the yt-dlp GitHub Releases API (`https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest`). Parse the `tag_name` field. Compare against `sidecar/yt-dlp.version`. If equal, exit early.

3. **Hash fetch** — download `SHA2-256SUMS` (or equivalent per-platform hash file) from the same GitHub release assets. This is a small text file served from GitHub's release CDN. Do not proceed without it.

4. **Binary download** — download the platform-appropriate binary (e.g., `yt-dlp`, `yt-dlp.exe`, `yt-dlp_macos`) from the release assets to a temporary path: `$KHUKRI_DATA_DIR/ytdlp-update.tmp`.

5. **Integrity verification** — SHA-256 hash the downloaded binary and compare it against the value from step 3. If they do not match: delete the temp file, log a warning, send a user notification "yt-dlp update failed: checksum mismatch", and abort. Retain the existing binary.

6. **Canary execution** — run `yt-dlp-update.tmp --version` in a subprocess. If it exits non-zero or produces no output: delete the temp file, log a warning, notify the user, and abort.

7. **Atomic hot-swap** — move the existing binary to `yt-dlp.bak`, then rename `yt-dlp-update.tmp` to the active binary path. Both operations use `std::fs::rename` on the same filesystem, making them atomic on POSIX systems. On Windows, `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING` is used. Update the in-memory version string; write the new version to `settings.json`.

8. **Notification** — emit a system notification: "yt-dlp updated to `<tag>`". Do not interrupt any active download.

9. **Rollback** — the `.bak` file is retained until the next successful update check. If a user reports a regression, they (or a support script) can copy the `.bak` back manually. Automatic rollback on first-run failure is not implemented because a broken new binary still passes the canary check in most real-world failure modes (extraction bugs, not startup bugs).

### Opt-out

A toggle in Settings > General > "Automatically update yt-dlp" (default: on) sets `ytdlp_auto_update: false` in `settings.json`. When off, the background task does not run. The user can trigger a manual check from Settings.

This toggle satisfies the zero-telemetry and user-control requirements in the PRD. It is the only outbound request from Khukri that is user-configurable; all download-initiated requests are user-initiated by definition.

### Developer override

If `KHUKRI_YTDLP_BIN` is set at startup, Khukri skips sidecar management entirely and exec's the provided path directly. No version check, no update, no hot-swap. This is the intended path for development, CI integration testing, and package manager installations (e.g., a distro that ships yt-dlp system-wide).

---

## Constraints and Non-Negotiables

These are inherited from the PRD (v1.1) and are not negotiable in this ADR:

| Constraint | Rationale |
|---|---|
| Tagged releases only — no `master` HEAD | HEAD has no stable checksum and carries untested code |
| SHA-256 verification before any swap | Supply-chain integrity — prevents MITM binary substitution |
| Hot-swap via rename, not in-place overwrite | Atomic on the target filesystem; no torn binary state |
| Retain last known good on any failure | Degraded-but-functional is better than broken |
| Update check is opt-out, not forced | Zero-telemetry commitment; user controls outbound connections |
| No DRM bypass, no credentials, no circumvention | Legal and license requirement |

---

## Failure Modes and Mitigations

| Failure | Detection | Mitigation |
|---|---|---|
| GitHub API rate-limited (60 req/hr unauthenticated) | HTTP 403 with `X-RateLimit-Remaining: 0` | Back off 1 hour; do not retry in a tight loop |
| GitHub API returns malformed JSON | `serde_json` parse error | Log, abort update, retain existing binary |
| Release asset not found for current platform | HTTP 404 on asset download | Log, notify user "no update available for your platform", abort |
| SHA-256 mismatch | Hash comparison fail | Delete temp file, notify user "checksum mismatch — update aborted", retain existing |
| Canary execution fails | Non-zero exit or timeout | Delete temp file, notify user, retain existing |
| Rename fails (cross-device) | `std::io::Error::CrossesDevices` | Fall back to copy + delete; if copy fails, abort and retain |
| Update applied but yt-dlp breaks extraction | Runtime extraction error on next use | User can toggle off auto-update; `.bak` available for manual restore |
| `KHUKRI_DATA_DIR` filesystem is read-only | Write error on temp file | Log error, skip update silently |

---

## Security Properties

- **No code execution before verification.** The binary is SHA-256 checked against the yt-dlp project's published hash file before the canary run. The hash file itself is served over HTTPS from GitHub's CDN with TLS certificate pinning via the OS trust store.
- **No hash hardcoding in source.** Hashes are fetched from the same release that provides the binary. Hardcoding would require a Khukri release for every yt-dlp update and is not feasible at yt-dlp's cadence.
- **Atomic swap.** There is no window where the active binary path contains a partial or unverified write.
- **No privilege escalation.** The sidecar lives in `$KHUKRI_DATA_DIR`, which is a user-writable directory. No admin or sudo required for updates.
- **Subprocess isolation.** yt-dlp is launched as a child process with a constrained argument list built by Khukri. It does not inherit Khukri's file descriptors beyond stdout/stderr pipes.

---

## Consequences

**Positive:**
- Users get working yt-dlp support within 24 hours of a platform-breaking upstream change without waiting for a Khukri release.
- The SHA-256 + canary check gate provides integrity assurance comparable to package manager workflows.
- The opt-out toggle satisfies the zero-telemetry and user-control requirements without making the feature unreliable by default.

**Negative / risks accepted:**
- The update check makes an outbound HTTPS request to `api.github.com` and GitHub's CDN. This is disclosed in the Settings panel and in the README. Users who need air-gapped operation must set `ytdlp_auto_update: false`.
- There is no cryptographic signature on the binary beyond SHA-256 (yt-dlp does not publish GPG-signed releases as of this writing). If the yt-dlp project's GitHub account were compromised and a malicious release published with matching hashes, the integrity check would pass. This risk is accepted as comparable to any package manager that trusts the upstream registry.
- The `.bak` rollback is manual-only. Automatic rollback on extraction failure is not implemented in Sprint 4 and is deferred to a future sprint if user reports warrant it.

---

## Related Tickets

| Ticket | Title |
|---|---|
| KHU-401 | Bundle yt-dlp sidecar binary at build time |
| KHU-402 | Implement yt-dlp 24-hour update check (tagged releases, SHA-256) |
| KHU-403 | Implement atomic hot-swap and rollback for yt-dlp binary |
| KHU-404 | Quality selector in Floating Blade UI (best / 1080p / 720p / audio-only) |
| KHU-405 | FFmpeg stitching for split video/audio streams |
| KHU-406 | Legal/ToS notice in onboarding |
