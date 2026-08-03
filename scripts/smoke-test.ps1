#Requires -Version 7.0
<#
    Installs the packaged NSIS bundle from src-tauri/target/release/bundle and
    boot-tests it end-to-end: silent-install, seed a throwaway workspace,
    `click.exe run --id <uuid>`, and confirm the launch engine actually ran
    both actions. Proves the shipped artifact works, not just that the source
    compiles (issue #24) — cargo test never calls tauri::Builder::run, so
    setup() (config load, tray, hotkeys) is otherwise never exercised in CI.

    This script overwrites %APPDATA%\com.launchpad.app\workspaces.json and
    kills any running click.exe. It refuses to run outside CI unless -Force
    is passed, and always restores whatever config existed before it ran.
#>
param(
    [string]$WorkDir = 'C:\click-smoke',
    [switch]$Force
)

$ErrorActionPreference = 'Stop'

function Write-Step {
    param([string]$Message)
    Write-Host "==> $Message" -ForegroundColor Cyan
}

if ($env:CI -ne 'true' -and -not $Force) {
    throw "Refusing to run outside CI: this script overwrites your real workspaces.json " +
          "and kills any running click.exe. Pass -Force if you really want that on this machine."
}

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$ConfigDir = Join-Path $env:APPDATA 'com.launchpad.app'
$ConfigPath = Join-Path $ConfigDir 'workspaces.json'
$ConfigBackupPath = Join-Path $WorkDir 'workspaces.json.backup'

New-Item -ItemType Directory -Force -Path $WorkDir | Out-Null

# Single-instance forwarding means a second click.exe would silently hand its
# `run --id` off to whatever instance is already running instead of booting
# fresh — kill any leftover instance before we rely on a clean boot.
Get-Process -Name 'click' -ErrorAction SilentlyContinue | Stop-Process -Force -Confirm:$false -ErrorAction SilentlyContinue

$hadExistingConfig = Test-Path $ConfigPath
if ($hadExistingConfig) {
    Write-Step "Backing up existing config to $ConfigBackupPath"
    Copy-Item -Path $ConfigPath -Destination $ConfigBackupPath -Force
}

$appProcess = $null
$stdout = $null
$stderr = $null
$exitCode = 0

try {
    # --- 1. Locate build artifacts ---
    Write-Step 'Locating installers'
    $nsisDir = Join-Path $RepoRoot 'src-tauri\target\release\bundle\nsis'
    $msiDir = Join-Path $RepoRoot 'src-tauri\target\release\bundle\msi'

    $installers = @(Get-ChildItem -Path $nsisDir -Filter '*-setup.exe' -ErrorAction Stop)
    if ($installers.Count -ne 1) {
        throw "Expected exactly one NSIS installer in $nsisDir, found $($installers.Count)"
    }
    $installer = $installers[0]
    if ($installer.Length -lt 1MB) {
        throw "NSIS installer looks broken: only $($installer.Length) bytes ($($installer.FullName))"
    }

    $msiFiles = @(Get-ChildItem -Path $msiDir -Filter '*.msi' -ErrorAction Stop)
    if ($msiFiles.Count -ne 1) {
        throw "Expected exactly one MSI installer in $msiDir, found $($msiFiles.Count)"
    }
    Write-Host "  NSIS: $($installer.FullName) ($([math]::Round($installer.Length / 1MB, 1)) MB)"
    Write-Host "  MSI:  $($msiFiles[0].FullName)"

    # --- 2. Silent install ---
    Write-Step "Installing silently to $WorkDir\app"
    $installDir = Join-Path $WorkDir 'app'
    # NSIS requires /D= to be the last argument and unquoted — hence a
    # default $WorkDir with no spaces in it.
    $installProc = Start-Process -FilePath $installer.FullName `
        -ArgumentList @('/S', "/D=$installDir") `
        -Wait -PassThru
    if ($installProc.ExitCode -ne 0) {
        Write-Warning "Installer exited with code $($installProc.ExitCode) (NSIS silent-install exit codes aren't always meaningful — checking for the binary regardless)"
    }

    $clickExe = Join-Path $installDir 'click.exe'
    if (-not (Test-Path $clickExe)) {
        Write-Warning "click.exe not at $clickExe — falling back to the uninstall registry key"
        $entry = Get-ItemProperty -Path @(
            'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*',
            'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*',
            'HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*'
        ) -ErrorAction SilentlyContinue |
            Where-Object { $_.DisplayName -eq 'click' } |
            Select-Object -First 1
        if ($entry -and $entry.InstallLocation) {
            $installDir = $entry.InstallLocation
            $clickExe = Join-Path $installDir 'click.exe'
        }
    }
    if (-not (Test-Path $clickExe)) {
        throw "Install did not produce click.exe (checked $installDir and the uninstall registry)"
    }
    Write-Host "  Installed: $clickExe"

    # --- 3. Seed a throwaway workspace ---
    Write-Step 'Seeding a smoke-test workspace'
    New-Item -ItemType Directory -Force -Path $ConfigDir | Out-Null

    $markerA = Join-Path $WorkDir 'marker-a.txt'
    $markerB = Join-Path $WorkDir 'marker-b.txt'
    $smokeCmdPath = Join-Path $WorkDir 'smoke.cmd'
    Remove-Item -Path $markerA, $markerB -Force -ErrorAction SilentlyContinue

    # A second action that is itself a .cmd file — deliberately exercises the
    # `.cmd`/`.bat` -> `cmd /C` routing in launch.rs, which nothing else covers.
    Set-Content -Path $smokeCmdPath -Encoding utf8NoBOM -Value @(
        '@echo off'
        'copy /Y NUL "%~dp0marker-b.txt" >NUL'
    )

    $cmdExe = Join-Path $env:SystemRoot 'System32\cmd.exe'
    $workspaceId = [guid]::NewGuid().ToString()

    # Hand-built to match the wire shape in model.rs: camelCase fields, and
    # the Action enum's externally-tagged `type` discriminant ("app"/"url").
    $workspaceFile = [ordered]@{
        version    = 1
        workspaces = @(
            [ordered]@{
                id             = $workspaceId
                name           = 'CI Smoke Test'
                description    = 'Seeded by scripts/smoke-test.ps1 — safe to delete.'
                icon           = $null
                color          = $null
                tags           = @()
                variables      = @{}
                launchStrategy = 'sequential'
                defaultDelayMs = 200
                hotkey         = $null
                actions        = @(
                    [ordered]@{
                        type         = 'app'
                        id           = [guid]::NewGuid().ToString()
                        label        = 'Marker A (plain exe)'
                        path         = $cmdExe
                        args         = @('/C', 'copy', '/Y', 'NUL', $markerA)
                        cwd          = $null
                        enabled      = $true
                        delayAfterMs = $null
                    },
                    [ordered]@{
                        type         = 'app'
                        id           = [guid]::NewGuid().ToString()
                        label        = 'Marker B (.cmd shim)'
                        path         = $smokeCmdPath
                        args         = @()
                        cwd          = $null
                        enabled      = $true
                        delayAfterMs = $null
                    }
                )
            }
        )
    }

    $workspaceFile | ConvertTo-Json -Depth 10 | Set-Content -Path $ConfigPath -Encoding utf8NoBOM

    # --- 4. Boot headless via the CLI path (FR-4.4) ---
    Write-Step "Booting click.exe run --id $workspaceId"
    $stdout = Join-Path $WorkDir 'click.stdout.log'
    $stderr = Join-Path $WorkDir 'click.stderr.log'

    $appProcess = Start-Process -FilePath $clickExe `
        -ArgumentList @('run', '--id', $workspaceId) `
        -RedirectStandardOutput $stdout `
        -RedirectStandardError $stderr `
        -NoNewWindow -PassThru

    # --- 5. Poll for both markers ---
    Write-Step 'Waiting for both markers (up to 30s)'
    $deadline = (Get-Date).AddSeconds(30)
    $sawA = $false
    $sawB = $false
    while ((Get-Date) -lt $deadline) {
        $sawA = Test-Path $markerA
        $sawB = Test-Path $markerB
        if ($sawA -and $sawB) { break }
        if ($appProcess.HasExited) {
            throw "click.exe exited early (code $($appProcess.ExitCode)) before both markers appeared."
        }
        Start-Sleep -Milliseconds 500
    }

    if (-not ($sawA -and $sawB)) {
        throw "Timed out waiting for markers. marker-a present: $sawA, marker-b present: $sawB"
    }
    Write-Host '  Both markers appeared — the launch engine ran both actions.'

    # --- 6. It's a tray app: it must still be running, not have exited ---
    Start-Sleep -Milliseconds 500
    if ($appProcess.HasExited) {
        throw "click.exe exited after launching (code $($appProcess.ExitCode)) — a tray app should keep running."
    }
    Write-Host '  click.exe is still running, as a tray app should be.'

    Write-Step 'Smoke test passed'
}
catch {
    $exitCode = 1
    Write-Host "SMOKE TEST FAILED: $_" -ForegroundColor Red
    foreach ($log in @($stdout, $stderr)) {
        if ($log -and (Test-Path $log)) {
            Write-Host "----- $log -----"
            Get-Content $log -ErrorAction SilentlyContinue | Write-Host
        }
    }
}
finally {
    Write-Step 'Tearing down'
    if ($appProcess -and -not $appProcess.HasExited) {
        Stop-Process -Id $appProcess.Id -Force -Confirm:$false -ErrorAction SilentlyContinue
    }
    Get-Process -Name 'click' -ErrorAction SilentlyContinue | Stop-Process -Force -Confirm:$false -ErrorAction SilentlyContinue

    if ($hadExistingConfig) {
        Copy-Item -Path $ConfigBackupPath -Destination $ConfigPath -Force
        Write-Host '  Restored original config from backup.'
    }
    elseif (Test-Path $ConfigPath) {
        Remove-Item -Path $ConfigPath -Force
        Write-Host '  Removed seeded smoke-test config (no original config existed).'
    }
}

exit $exitCode
