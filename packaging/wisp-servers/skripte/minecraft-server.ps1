# Sets up a Wisp Minecraft server (the game, run headless: docs/hosting.md) in the folder
# "Minecraft-Server": installs wisp-server with Wisp's own installer, then asks the first-run
# questions (world, port, whitelist, operators).
. (Join-Path $PSScriptRoot 'common.ps1')

$repo = Get-Repo
$folder = Join-Path $Root 'Minecraft-Server'
New-Item -ItemType Directory -Force -Path $folder | Out-Null

Write-Host ''
Write-Host '  Wisp Minecraft-Server einrichten' -ForegroundColor Green
Write-Host '  Ein eigener Minecraft-26.3-Server, der mit Wisp laeuft (optional).'

if (-not (Get-Command wisp-server -ErrorAction SilentlyContinue)) {
    Step 'Installiere wisp-server'
    & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $repo 'packaging\server\install.ps1')
    $env:PATH = [Environment]::GetEnvironmentVariable('PATH', 'Machine') + ';' + [Environment]::GetEnvironmentVariable('PATH', 'User')
}
if (-not (Get-Command wisp-server -ErrorAction SilentlyContinue)) {
    throw 'wisp-server ist nicht installiert. Siehe docs/hosting.md im Wisp-Code.'
}

Push-Location $folder
try {
    if (-not (Test-Path 'server.toml')) {
        Step 'Erster Start: ein paar Fragen zur Welt'
        & wisp-server init
    }
    Step 'Starte den Minecraft-Server'
    & wisp-server start
    Good "Laeuft im Hintergrund. Ordner: $folder"
    Say 'Stoppen: im Ordner "wisp-server stop". Mehr: docs/hosting.md im Wisp-Code.'
} finally {
    Pop-Location
}
