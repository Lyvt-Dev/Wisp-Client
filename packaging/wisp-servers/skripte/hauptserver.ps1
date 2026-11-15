# Starts the Wisp main server (friends, cosmetics, creators, coins and the Shop) on this PC,
# opens a free public HTTPS address with cloudflared and points this PC's launcher at it.
. (Join-Path $PSScriptRoot 'common.ps1')

$exe = Join-Path $Bin 'wisp-cosmetics-server.exe'
if (-not (Test-Path $exe)) {
    Warn 'Der Hauptserver ist noch nicht gebaut: zuerst "1 - Einrichten.cmd".'
    exit 1
}
Set-ServerEnv
$port = $env:PORT

Write-Host ''
Write-Host '  Wisp-Hauptserver' -ForegroundColor Green
Write-Host '  Freunde, Cosmetics, Creator, Coins und Shop. Lass dieses Fenster offen.'
if (-not $env:WISP_ADMINS) { Warn 'Kein Owner eingetragen: niemand kann Creator bestimmen. ("1 - Einrichten.cmd")' }

Step 'Starte den Server'
$server = Start-Process -FilePath $exe -PassThru -WindowStyle Hidden `
    -RedirectStandardError (Join-Path $Data 'server.log') -RedirectStandardOutput (Join-Path $Data 'server.out.log')
$up = $false
for ($i = 0; $i -lt 30 -and -not $up; $i++) {
    Start-Sleep -Milliseconds 500
    try { $up = (Invoke-WebRequest -UseBasicParsing "http://127.0.0.1:$port/healthz" -TimeoutSec 2).Content -eq 'ok' } catch {}
}
if (-not $up) { throw "Der Server startet nicht. Sein Log: $(Join-Path $Data 'server.log')" }
Good "Laeuft auf diesem PC (Port $port). Daten: $Data"

Step 'Oeffentliche Adresse'
if (-not (Get-Command cloudflared -ErrorAction SilentlyContinue)) {
    Say 'Installiere cloudflared (kostenlos, einmal fuer die oeffentliche Adresse)...'
    winget install --id Cloudflare.cloudflared -e --accept-source-agreements --accept-package-agreements | Out-Host
    $env:PATH = [Environment]::GetEnvironmentVariable('PATH', 'Machine') + ';' + [Environment]::GetEnvironmentVariable('PATH', 'User')
}
$url = $null
$tunnel = $null
if (Get-Command cloudflared -ErrorAction SilentlyContinue) {
    $tunnelLog = Join-Path $Data 'tunnel.log'
    Remove-Item $tunnelLog -ErrorAction SilentlyContinue
    $tunnel = Start-Process -FilePath (Get-Command cloudflared).Source -PassThru -WindowStyle Hidden `
        -ArgumentList @('tunnel', '--no-autoupdate', '--url', "http://127.0.0.1:$port") `
        -RedirectStandardError $tunnelLog -RedirectStandardOutput (Join-Path $Data 'tunnel.out.log')
    for ($i = 0; $i -lt 60 -and -not $url; $i++) {
        Start-Sleep -Milliseconds 500
        if (Test-Path $tunnelLog) {
            $match = Select-String -Path $tunnelLog -Pattern 'https://[a-z0-9-]+\.trycloudflare\.com' | Select-Object -First 1
            if ($match) { $url = $match.Matches[0].Value }
        }
    }
}
if (-not $url) {
    Warn 'Keine oeffentliche Adresse (cloudflared fehlt oder ist blockiert). Im selben Netzwerk geht:'
    $ip = (Get-NetIPAddress -AddressFamily IPv4 | Where-Object { $_.PrefixOrigin -eq 'Dhcp' } | Select-Object -First 1).IPAddress
    $url = "http://${ip}:$port"
}
Good $url

Step 'Launcher auf diesem PC einstellen'
$prefsPath = Join-Path $env:LOCALAPPDATA 'wisp-local-launcher\launcher-prefs.json'
try {
    $prefs = if (Test-Path $prefsPath) { Get-Content $prefsPath -Raw | ConvertFrom-Json } else { [pscustomobject]@{} }
    $prefs | Add-Member -NotePropertyName service_url -NotePropertyValue $url -Force
    New-Item -ItemType Directory -Force -Path (Split-Path $prefsPath) | Out-Null
    [IO.File]::WriteAllText($prefsPath, ($prefs | ConvertTo-Json -Depth 10))
    Say 'Erledigt. Starte den Launcher neu, falls er offen ist.'
} catch {
    Warn "Konnte $prefsPath nicht schreiben. Trage die Adresse im Launcher ein: Settings > General > Wisp server."
}
try { Set-Clipboard -Value $url } catch {}

Write-Host ''
Write-Host "  Dein Wisp-Server: $url" -ForegroundColor Green
Write-Host '  (in die Zwischenablage kopiert)'
Write-Host ''
Write-Host '  Spieler tragen die Adresse im Launcher ein: Settings > General > Wisp server.'
Write-Host '  Diese kostenlose Adresse aendert sich bei jedem Start. Fuer eine feste Adresse: README.md, "Immer online".'
Write-Host ''
Write-Host '  Fenster schliessen = Server stoppen.' -ForegroundColor Yellow
try {
    while (-not $server.HasExited) { Start-Sleep -Seconds 2 }
    Warn "Der Server wurde beendet. Sein Log: $(Join-Path $Data 'server.log')"
} finally {
    if ($tunnel -and -not $tunnel.HasExited) { Stop-Process -Id $tunnel.Id -Force -ErrorAction SilentlyContinue }
    if (-not $server.HasExited) { Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue }
}
