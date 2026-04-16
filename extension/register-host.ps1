# Khukri Native Messaging Host Registration Script (Windows)
# Usage: Run as user to register the native messaging host manifest for Chrome

$ErrorActionPreference = 'Stop'

$hostId = 'com.khukri.host'
$manifestPath = "$PSScriptRoot\\$hostId.json"
$chromeRegPath = "HKCU:\\Software\\Google\\Chrome\\NativeMessagingHosts\\$hostId"

# Find the absolute path to the bridge binary (assume in parent folder for now)
$bridgePath = (Resolve-Path "$PSScriptRoot\\..\\target\\release\\khukri-bridge.exe").Path

# Write manifest JSON
$manifest = @{
  name = $hostId
  description = 'Khukri Native Messaging Host'
  path = $bridgePath
  type = 'stdio'
  allowed_origins = @('chrome-extension://*')
} | ConvertTo-Json -Depth 4

Set-Content -Path $manifestPath -Value $manifest -Encoding UTF8

# Register in registry
New-Item -Path $chromeRegPath -Force | Out-Null
Set-ItemProperty -Path $chromeRegPath -Name '(default)' -Value $manifestPath

Write-Host "Native messaging host registered for Chrome: $manifestPath"