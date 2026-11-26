# Starts the Wisp server (cosmetics and friends) on this PC and gives it a public HTTPS address,
# then points this PC's launcher at it. Double-click Start-Wisp-Server.cmd instead of running
# this directly. Everything it needs is set up on the first run: Rust (to build the server) and
# cloudflared (the free public address), both through winget.
$ErrorActionPreference = 'Stop'
$here = $PSScriptRoot
$repo = (Resolve-Path (Join-Path $here '..\..')).Path
$port = 8080
$data = Join-Path $env:LOCALAPPDATA 'WispServer'
New-Item -ItemType Directory -Force -Path $data | Out-Null

function Say($text) { Write-Host "  $text" }
function Step($text) { Write-Host ''; Write-Host "> $text" -ForegroundColor Cyan }

Write-Host ''
Write-Host '  Wisp server' -ForegroundColor Green
Write-Host '  Cosmetics and friends for every Wisp player. Keep this window open.'

# 1. The server program: built once, again when its source is newer.
Step 'Getting the server ready'
$exe = Join-Path $here 'target\release\wisp-cosmetics-server.exe'
$sources = Get-ChildItem -Recurse -File (Join-Path $here 'src'), (Join-Path $repo 'crates\wisp-cosmetics\src')
$stale = -not (Test-Path $exe) -or ($sources | Where-Object { $_.LastWriteTime -gt (Get-Item $exe).LastWriteTime })
if ($stale) {
    $tooling = Join-Path $repo '.local\tooling'
    if (Test-Path (Join-Path $tooling 'cargo\bin\cargo.exe')) {
        $env:CARGO_HOME = Join-Path $tooling 'cargo'
        $env:RUSTUP_HOME = Join-Path $tooling 'rustup'
        $env:PATH = (Join-Path $tooling 'cargo\bin') + ';' + $env:PATH
    }
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Say 'Installing Rust (needed once to build the server)...'
        winget install --id Rustlang.Rustup -e --accept-source-agreements --accept-package-agreements | Out-Host
        $env:PATH = (Join-Path $env:USERPROFILE '.cargo\bin') + ';' + $env:PATH
        if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
            throw 'Rust did not install. Install it from https://rustup.rs and run this again.'
        }
    }
    Say 'Building the server (a few minutes the first time)...'
    & cargo build --release --manifest-path (Join-Path $here 'Cargo.toml')
    if ($LASTEXITCODE -ne 0) { throw 'The server did not build. See the messages above.' }
}
Say 'Ready.'

# 2. Start it.
Step 'Starting the server'
$env:PORT = "$port"
$env:WISP_COSMETICS_DB = Join-Path $data 'wisp.sqlite3'
$server = Start-Process -FilePath $exe -PassThru -WindowStyle Hidden `
    -RedirectStandardError (Join-Path $data 'server.log') -RedirectStandardOutput (Join-Path $data 'server.out.log')
$up = $false
for ($i = 0; $i -lt 30 -and -not $up; $i++) {
    Start-Sleep -Milliseconds 500
    try { $up = (Invoke-WebRequest -UseBasicParsing "http://127.0.0.1:$port/healthz" -TimeoutSec 2).Content -eq 'ok' } catch {}
}
if (-not $up) { throw "The server did not start. Its log: $(Join-Path $data 'server.log')" }
Say "Running on this PC (port $port). Data: $data"

# 3. A public HTTPS address, so friends anywhere can reach it.
Step 'Opening a public address'
if (-not (Get-Command cloudflared -ErrorAction SilentlyContinue)) {
    Say 'Installing cloudflared (free, needed once for the public address)...'
    winget install --id Cloudflare.cloudflared -e --accept-source-agreements --accept-package-agreements | Out-Host
    $env:PATH = [Environment]::GetEnvironmentVariable('PATH', 'Machine') + ';' + [Environment]::GetEnvironmentVariable('PATH', 'User')
}
$url = $null
$tunnel = $null
if (Get-Command cloudflared -ErrorAction SilentlyContinue) {
    $tunnelLog = Join-Path $data 'tunnel.log'
    Remove-Item $tunnelLog -ErrorAction SilentlyContinue
    $tunnel = Start-Process -FilePath (Get-Command cloudflared).Source -PassThru -WindowStyle Hidden `
        -ArgumentList @('tunnel', '--no-autoupdate', '--url', "http://127.0.0.1:$port") `
        -RedirectStandardError $tunnelLog -RedirectStandardOutput (Join-Path $data 'tunnel.out.log')
    for ($i = 0; $i -lt 60 -and -not $url; $i++) {
        Start-Sleep -Milliseconds 500
        if (Test-Path $tunnelLog) {
            $match = Select-String -Path $tunnelLog -Pattern 'https://[a-z0-9-]+\.trycloudflare\.com' | Select-Object -First 1
            if ($match) { $url = $match.Matches[0].Value }
        }
    }
}
if (-not $url) {
    Say 'No public address (cloudflared missing or blocked). Friends on this network can use:'
    $url = "http://$((Get-NetIPAddress -AddressFamily IPv4 | Where-Object { $_.PrefixOrigin -eq 'Dhcp' } | Select-Object -First 1).IPAddress):$port"
}
Say $url

# 4. Point this PC's launcher at it.
Step 'Setting up the launcher on this PC'
$prefsPath = Join-Path $env:LOCALAPPDATA 'wisp-local-launcher\launcher-prefs.json'
try {
    $prefs = if (Test-Path $prefsPath) { Get-Content $prefsPath -Raw | ConvertFrom-Json } else { [pscustomobject]@{} }
    $prefs | Add-Member -NotePropertyName service_url -NotePropertyValue $url -Force
    New-Item -ItemType Directory -Force -Path (Split-Path $prefsPath) | Out-Null
    [IO.File]::WriteAllText($prefsPath, ($prefs | ConvertTo-Json -Depth 10))
    Say 'Done. Restart the launcher if it is open.'
} catch {
    Say "Could not write $prefsPath. Paste the address in Settings, General, Wisp server."
}
try { Set-Clipboard -Value $url } catch {}

Write-Host ''
Write-Host "  Your Wisp server: $url" -ForegroundColor Green
Write-Host '  (copied to the clipboard)'
Write-Host ''
Write-Host '  Friends paste it in the launcher: Settings > General > Wisp server.'
Write-Host '  This free address changes each time you start the server. For one that never'
Write-Host '  changes, see docs/wisp-server.md (Railway).'
Write-Host ''
Write-Host '  Close this window to stop the server.' -ForegroundColor Yellow
try {
    while (-not $server.HasExited) { Start-Sleep -Seconds 2 }
    Write-Host '  The server stopped. Its log:' (Join-Path $data 'server.log') -ForegroundColor Red
} finally {
    if ($tunnel -and -not $tunnel.HasExited) { Stop-Process -Id $tunnel.Id -Force -ErrorAction SilentlyContinue }
    if (-not $server.HasExited) { Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue }
}
