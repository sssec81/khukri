# Free macOS Beta Distribution

This beta path does not require an Apple Developer subscription. The app is
ad-hoc signed, not notarized, so macOS will not trust it automatically.

## Build

The build includes the matching `yt-dlp`, FFmpeg, and native messaging bridge
sidecars. On Apple Silicon, the current FFmpeg beta binary is Intel-only and
requires Rosetta 2.

```bash
bash sidecar/fetch-ffmpeg.sh macos
bash scripts/build-macos-beta.sh
```

Artifacts are written to `artifacts/beta/`:

- `Khukri_0.1.0_aarch64.dmg` (name varies by architecture)
- `khukri-extension-beta.zip`
- `SHA256SUMS`

The build runs `cargo audit` when it is installed. `RUSTSEC-2023-0071` is
explicitly ignored because it is reachable only through SQLx's optional
PostgreSQL driver. Khukri disables SQLx default features and compiles only the
SQLite driver; `cargo tree --target all -i rsa@0.9.10` returns no compiled
dependency path. Do not carry this exception forward if PostgreSQL support is
ever enabled.

## Install the desktop app

1. Open the DMG and drag Khukri to Applications.
2. Try to open Khukri once. macOS may block it because it is not notarized.
3. Open **System Settings > Privacy & Security**.
4. Find the Khukri warning, choose **Open Anyway**, then confirm **Open**.

Do not tell testers to disable Gatekeeper globally.

## Install the beta extension

1. Unzip `khukri-extension-beta.zip` into a permanent folder.
2. Open `chrome://extensions`.
3. Enable **Developer mode**.
4. Choose **Load unpacked** and select the unzipped folder.
5. Launch Khukri once. It automatically registers the bundled native bridge
   for extension ID `hlingdbecfefhglkbballggindegcmik`.
6. Reload the extension if Chrome was already open.

The public key in `extension/manifest.json` keeps the unpacked beta extension
ID stable across tester machines. This is a developer beta flow; a normal
Chrome Web Store installation is still required for a public release.

## Uninstall

Remove Khukri from Applications and remove the extension from
`chrome://extensions`. The native host manifest may be removed manually from:

```text
~/Library/Application Support/Google/Chrome/NativeMessagingHosts/com.khukri.host.json
```
