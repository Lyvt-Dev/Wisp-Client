# Puts the "Wisp Server" folder on this PC's desktop (or -Target): the double-click files, their
# scripts, the README and einstellungen.env, which remembers where this repository is. Running it
# again updates the scripts and the README and keeps the settings and the data.
#
#   powershell -ExecutionPolicy Bypass -File packaging\wisp-servers\Install-Wisp-Server-Folder.ps1
param([string]$Target = (Join-Path ([Environment]::GetFolderPath('Desktop')) 'Wisp Server'))
$ErrorActionPreference = 'Stop'
$here = $PSScriptRoot
$repo = (Resolve-Path (Join-Path $here '..\..')).Path

New-Item -ItemType Directory -Force -Path $Target, (Join-Path $Target 'skripte'), (Join-Path $Target 'programme'), (Join-Path $Target 'daten') | Out-Null
Copy-Item -Force (Join-Path $here 'skripte\*.ps1') (Join-Path $Target 'skripte')
Copy-Item -Force (Join-Path $here 'cmd\*.cmd') $Target
Copy-Item -Force (Join-Path $here 'README.md') $Target

$settings = Join-Path $Target 'einstellungen.env'
if (-not (Test-Path $settings)) {
    [IO.File]::WriteAllLines($settings, [string[]]@(
        '# Einstellungen fuer die Wisp-Server in diesem Ordner. Zeilen mit # sind Kommentare.',
        '# Wo der Wisp-Code liegt (zum Bauen und Aktualisieren):',
        "WISP_REPO=$repo",
        '# Dein Minecraft-Account (UUID), der als Einziger Creator bestimmen darf. "1 - Einrichten.cmd" traegt ihn ein.',
        'WISP_ADMINS=',
        '# Port des Hauptservers:',
        'PORT=8080',
        '# Port des Relays (Mit Freunden spielen):',
        'RELAY_PORT=7000',
        '# Optional: nur diese Item-IDs duerfen getragen werden (Komma-getrennt). Leer = alle gueltigen.',
        'WISP_COSMETICS_ITEMS='
    ))
} else {
    # A moved repository: keep everything else.
    $lines = Get-Content $settings | ForEach-Object { if ($_ -match '^\s*WISP_REPO\s*=') { "WISP_REPO=$repo" } else { $_ } }
    [IO.File]::WriteAllLines($settings, [string[]]$lines)
}
Write-Host "Wisp Server folder: $Target" -ForegroundColor Green
