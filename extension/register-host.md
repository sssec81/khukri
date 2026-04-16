# Khukri Native Messaging Host Registration

This folder contains scripts and a manifest template for registering the Khukri Native Messaging host for Chrome.

## Windows
- Run `register-host.ps1` in PowerShell (as user, not admin)
- This writes the manifest and sets the registry key for Chrome

## Linux
- Run `register-host.sh` (make executable with `chmod +x register-host.sh`)
- This writes the manifest to `~/.config/google-chrome/NativeMessagingHosts/`

## macOS
- Manual: Copy and adapt the Linux script to use the correct path for Chrome/Chromium on macOS

## Notes
- The manifest's `path` field must be the absolute path to the bridge binary
- Re-running the script repairs the manifest if the binary is moved
- Host ID: `com.khukri.host`