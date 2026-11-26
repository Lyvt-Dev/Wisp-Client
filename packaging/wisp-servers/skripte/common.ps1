# Shared by the scripts in the "Wisp Server" folder: where things are, the settings file, Rust,
# and building the server programs from the Wisp repository.
$ErrorActionPreference = 'Stop'
$Root = Split-Path $PSScriptRoot -Parent
$Bin = Join-Path $Root 'programme'
$Data = Join-Path $Root 'daten'
$SettingsFile = Join-Path $Root 'einstellungen.env'
New-Item -ItemType Directory -Force -Path $Bin, $Data | Out-Null

function Say($text) { Write-Host "  $text" }
function Step($text) { Write-Host ''; Write-Host "> $text" -ForegroundColor Cyan }
function Good($text) { Write-Host "  $text" -ForegroundColor Green }
function Warn($text) { Write-Host "  $text" -ForegroundColor Yellow }

# einstellungen.env: KEY=value lines, # for comments.
function Read-Settings {
    $settings = [ordered]@{}
    if (Test-Path $SettingsFile) {
        foreach ($line in Get-Content $SettingsFile) {
            $t = $line.Trim()
            if ($t -eq '' -or $t.StartsWith('#')) { continue }
            $i = $t.IndexOf('=')
            if ($i -gt 0) { $settings[$t.Substring(0, $i).Trim()] = $t.Substring($i + 1).Trim() }
        }
    }
    $settings
}

function Write-Setting($key, $value) {
    $lines = if (Test-Path $SettingsFile) { @(Get-Content $SettingsFile) } else { @() }
    $found = $false
    $lines = foreach ($line in $lines) {
        if ($line -match "^\s*$([regex]::Escape($key))\s*=") { $found = $true; "$key=$value" } else { $line }
    }
    if (-not $found) { $lines += "$key=$value" }
    [IO.File]::WriteAllLines($SettingsFile, [string[]]$lines)
}

# The Wisp repository these programs are built from (remembered in einstellungen.env).
function Get-Repo {
    $repo = (Read-Settings)['WISP_REPO']
    if (-not $repo -or -not (Test-Path (Join-Path $repo 'crates\wisp-cosmetics-server'))) {
        throw "Der Wisp-Code wurde nicht gefunden ($repo). Trage den Ordner in einstellungen.env bei WISP_REPO ein."
    }
    $repo
}

# Rust: the repository's own toolchain when it has one, else rustup (installed through winget).
function Use-Rust($repo) {
    $tooling = Join-Path $repo '.local\tooling'
    if (Test-Path (Join-Path $tooling 'cargo\bin\cargo.exe')) {
        $env:CARGO_HOME = Join-Path $tooling 'cargo'
        $env:RUSTUP_HOME = Join-Path $tooling 'rustup'
        $env:PATH = (Join-Path $tooling 'cargo\bin') + ';' + $env:PATH
    }
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Say 'Installiere Rust (wird einmal zum Bauen gebraucht)...'
        winget install --id Rustlang.Rustup -e --accept-source-agreements --accept-package-agreements | Out-Host
        $env:PATH = (Join-Path $env:USERPROFILE '.cargo\bin') + ';' + $env:PATH
        if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
            throw 'Rust konnte nicht installiert werden. Installiere es von https://rustup.rs und starte das hier erneut.'
        }
    }
}

# Builds the main server and the relay in release mode and copies them into programme\.
function Build-Servers {
    $repo = Get-Repo
    Use-Rust $repo
    Step 'Baue den Wisp-Hauptserver (beim ersten Mal ein paar Minuten)'
    & cargo build --release --manifest-path (Join-Path $repo 'crates\wisp-cosmetics-server\Cargo.toml')
    if ($LASTEXITCODE -ne 0) { throw 'Der Hauptserver liess sich nicht bauen. Siehe die Meldungen oben.' }
    Copy-Item -Force (Join-Path $repo 'crates\wisp-cosmetics-server\target\release\wisp-cosmetics-server.exe') $Bin
    Good 'Hauptserver fertig.'
    Step 'Baue das Relay (Mit Freunden spielen)'
    & cargo build --release --manifest-path (Join-Path $repo 'Cargo.toml') -p wisp-relay
    if ($LASTEXITCODE -ne 0) { throw 'Das Relay liess sich nicht bauen. Siehe die Meldungen oben.' }
    Copy-Item -Force (Join-Path $repo 'target\release\wisp-relay.exe') $Bin
    Good 'Relay fertig.'
}

# The main server's environment from einstellungen.env, its data in daten\.
function Set-ServerEnv {
    $s = Read-Settings
    $env:PORT = if ($s['PORT']) { $s['PORT'] } else { '8080' }
    $env:WISP_COSMETICS_DB = Join-Path $Data 'wisp.sqlite3'
    $env:WISP_ADMINS = $s['WISP_ADMINS']
    $env:WISP_COSMETICS_ITEMS = $s['WISP_COSMETICS_ITEMS']
}

# A Minecraft account's UUID (32 hex digits), asked of Mojang's public profile lookup.
function Get-MinecraftUuid($name) {
    try {
        $answer = Invoke-RestMethod -UseBasicParsing "https://api.mojang.com/users/profiles/minecraft/$name" -TimeoutSec 10
        return $answer.id
    } catch {
        return $null
    }
}
