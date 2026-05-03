# Khukri Icon Assets

The icon bundle is generated from `icon.png`, the final 512 x 512 Khukri app mark.

Desktop bundle assets:

- `16x16.png`
- `32x32.png`
- `64x64.png`
- `128x128.png`
- `128x128@2x.png`
- `icon.ico`
- `icon.icns`
- `icon.png`

Regenerate the platform bundle with:

```bash
cargo tauri icon src-tauri/icons/icon.png -o target/khukri-icons
```

Then copy the desktop assets listed above into this directory. The full Tauri icon generator also emits mobile/AppX files that are not currently referenced by `tauri.conf.json`.
