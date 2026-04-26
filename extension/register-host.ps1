#Requires -Version 5.1
<#
.SYNOPSIS
    Registers the Khukri Native Messaging host for Chrome, Edge, and Brave on Windows.

.DESCRIPTION
    Validates the extension ID, sets KHUKRI_EXTENSION_ORIGIN, and delegates to
    khukri-bridge --register so the bridge's validate_extension_origin check is
    always exercised. Then copies the generated manifest to the Edge and Brave
    NativeMessagingHosts registry keys and manifest directories if those browsers
    are installed.

.PARAMETER ExtensionId
    The 32-character Chrome/Edge/Brave extension ID shown in the browser's
    extension management page (developer mode on), or the ID assigned by the
    Chrome Web Store after publishing.

.PARAMETER BridgePath
    Optional. Full path to khukri-bridge.exe. Defaults to searching the release
    and debug build directories relative to this script.

.PARAMETER DryRun
    Print what would happen without writing any files or registry keys.

.EXAMPLE
    .\register-host.ps1 -ExtensionId abcdefghijklmnopabcdefghijklmnop

.EXAMPLE
    .\register-host.ps1 -ExtensionId abcdefghijklmnopabcdefghijklmnop -DryRun

.EXAMPLE
    .\register-host.ps1 abcdefghijklmnopabcdefghijklmnop `
        -BridgePath "C:\Program Files\Khukri\khukri-bridge.exe"
#>

[CmdletBinding(SupportsShouldProcess)]
param(
    [Parameter(Mandatory, Position = 0)]
    [string]$ExtensionId,

    [Parameter()]
    [string]$BridgePath,

    [Parameter()]
    [switch]$DryRun
)

$ErrorActionPreference = 'Stop'

# ── Validate extension ID ────────────────────────────────────────────────────

if ($ExtensionId -notmatch '^[a-p]{32}$') {
    Write-Error "Invalid extension ID '$ExtensionId'." `
        + " Chrome extension IDs are exactly 32 lowercase letters a-p." `
        + " Find yours in chrome://extensions with developer mode on."
    exit 1
}

$Origin = "chrome-extension://${ExtensionId}/"

# ── Locate bridge binary ─────────────────────────────────────────────────────

$RepoRoot = Split-Path $PSScriptRoot -Parent

if (-not $BridgePath) {
    $Candidates = @(
        Join-Path $RepoRoot 'target\release\khukri-bridge.exe',
        Join-Path $RepoRoot 'target\debug\khukri-bridge.exe'
    )
    foreach ($c in $Candidates) {
        if (Test-Path $c) { $BridgePath = $c; break }
    }
    if (-not $BridgePath) {
        $found = Get-Command khukri-bridge.exe -ErrorAction SilentlyContinue
        if ($found) { $BridgePath = $found.Source }
    }
}

if (-not $BridgePath -or -not (Test-Path $BridgePath)) {
    Write-Error "khukri-bridge.exe not found.`n" `
        + "  Build it first:  cargo build -p khukri-bridge --release`n" `
        + "  Or pass:         -BridgePath 'C:\path\to\khukri-bridge.exe'"
    exit 2
}

$HostId = 'com.khukri.host'

# ── Chrome: manifest lives next to the bridge binary; registry key points to it ──

$ManifestDir  = Split-Path $BridgePath -Parent
$ManifestPath = Join-Path $ManifestDir "$HostId.json"

$ChromeRegKey = "HKCU:\Software\Google\Chrome\NativeMessagingHosts\$HostId"
$EdgeRegKey   = "HKCU:\Software\Microsoft\Edge\NativeMessagingHosts\$HostId"
$BraveRegKey  = "HKCU:\Software\BraveSoftware\Brave-Browser\NativeMessagingHosts\$HostId"

# ── Dry-run summary ──────────────────────────────────────────────────────────

if ($DryRun) {
    Write-Host "[dry-run] Would register native messaging host:"
    Write-Host "  Bridge binary : $BridgePath"
    Write-Host "  Extension ID  : $ExtensionId"
    Write-Host "  Origin        : $Origin"
    Write-Host "  Manifest path : $ManifestPath"
    Write-Host "  Chrome reg    : $ChromeRegKey"
    Write-Host "  Edge reg      : $EdgeRegKey   (if Edge installed)"
    Write-Host "  Brave reg     : $BraveRegKey  (if Brave installed)"
    exit 0
}

# ── Primary registration via bridge --register ───────────────────────────────

Write-Host "Registering native messaging host..."
Write-Host "  Bridge  : $BridgePath"
Write-Host "  Origin  : $Origin"

$env:KHUKRI_EXTENSION_ORIGIN = $Origin
$proc = Start-Process -FilePath $BridgePath `
    -ArgumentList '--register' `
    -NoNewWindow -Wait -PassThru
if ($proc.ExitCode -ne 0) {
    Write-Error "khukri-bridge --register exited with code $($proc.ExitCode)."
    exit 2
}

Write-Host "  Written : $ManifestPath"
Write-Host "  Chrome reg written"

# ── Copy registry key to Edge ─────────────────────────────────────────────────

$EdgeExe = "${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe"
if (Test-Path $EdgeExe) {
    New-Item -Path $EdgeRegKey -Force | Out-Null
    Set-ItemProperty -Path $EdgeRegKey -Name '(default)' -Value $ManifestPath
    Write-Host "  Edge reg written"
} else {
    Write-Host "  Edge not detected — skipped"
}

# ── Copy registry key to Brave ────────────────────────────────────────────────

$BraveExe = "$env:LOCALAPPDATA\BraveSoftware\Brave-Browser\Application\brave.exe"
if (Test-Path $BraveExe) {
    New-Item -Path $BraveRegKey -Force | Out-Null
    Set-ItemProperty -Path $BraveRegKey -Name '(default)' -Value $ManifestPath
    Write-Host "  Brave reg written"
} else {
    Write-Host "  Brave not detected — skipped"
}

# ── Done ─────────────────────────────────────────────────────────────────────

Write-Host ""
Write-Host "Done. Restart Chrome/Edge/Brave for the change to take effect."
Write-Host ""
Write-Host "If downloads are not intercepted, verify:"
Write-Host "  1. The extension is loaded and enabled."
Write-Host "  2. The extension ID matches: $ExtensionId"
Write-Host "  3. The bridge binary is accessible at: $BridgePath"
