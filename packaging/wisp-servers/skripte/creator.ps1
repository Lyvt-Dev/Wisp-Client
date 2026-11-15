# Creators on this PC's main server: make a player a creator, give them a badge (PNG, at most
# 16x16), or take it away. Works on the data in daten\, with the server running or not.
. (Join-Path $PSScriptRoot 'common.ps1')

$exe = Join-Path $Bin 'wisp-cosmetics-server.exe'
if (-not (Test-Path $exe)) {
    Warn 'Der Hauptserver ist noch nicht gebaut: zuerst "1 - Einrichten.cmd".'
    exit 1
}
Set-ServerEnv

function Run([string[]]$arguments) {
    & $exe @arguments
    if ($LASTEXITCODE -eq 0) { Good 'Erledigt.' } else { Warn 'Das hat nicht geklappt (siehe oben).' }
}

Write-Host ''
Write-Host '  Creator verwalten' -ForegroundColor Green
Write-Host '  Spieler muessen Wisp einmal mit diesem Server benutzt haben.'
while ($true) {
    Write-Host ''
    Write-Host '  1) Spieler zum Creator machen'
    Write-Host '  2) Badge (PNG, max. 16x16) fuer einen Creator setzen'
    Write-Host '  3) Creator entfernen'
    Write-Host '  4) Beenden'
    switch ((Read-Host '  Auswahl').Trim()) {
        '1' {
            $name = (Read-Host '  Minecraft-Name').Trim()
            if ($name) { Run @('creator', $name) }
        }
        '2' {
            $name = (Read-Host '  Minecraft-Name des Creators').Trim()
            $file = (Read-Host '  Pfad zur PNG (Datei ins Fenster ziehen)').Trim().Trim('"')
            $line = (Read-Host '  Farbe der Linie unter dem Namen, z. B. #ff4d8d (Enter = weiss)').Trim()
            if (-not $line) { $line = '#ffffff' }
            if ($name -and $file) { Run @('badge', $name, $file, $line) }
        }
        '3' {
            $name = (Read-Host '  Minecraft-Name').Trim()
            if ($name) { Run @('uncreator', $name) }
        }
        default { exit 0 }
    }
}
