# First setup of the "Wisp Server" folder: builds the server programs, asks who the owner is
# (the only account that may choose creators) and writes einstellungen.env. Run again any time.
param([string]$Owner)
. (Join-Path $PSScriptRoot 'common.ps1')

Write-Host ''
Write-Host '  Wisp Server einrichten' -ForegroundColor Green
Write-Host '  Baut die Server-Programme und richtet dich als Owner ein.'

# 1. The programs.
Build-Servers

# 2. The owner: their Minecraft name, turned into the UUID the server checks.
Step 'Owner festlegen (nur dieser Account darf Creator bestimmen)'
$settings = Read-Settings
if ($settings['WISP_ADMINS']) { Say "Aktuell eingetragen: $($settings['WISP_ADMINS'])" }
while (-not $Owner) {
    $Owner = (Read-Host '  Dein Minecraft-Name (Enter = so lassen)').Trim()
    if (-not $Owner) { break }
}
if ($Owner) {
    $uuid = Get-MinecraftUuid $Owner
    if ($uuid) {
        Write-Setting 'WISP_ADMINS' $uuid
        Good "$Owner ist Owner (UUID $uuid)."
    } else {
        Warn "Mojang kennt den Namen $Owner nicht (oder ist nicht erreichbar). Trage deine UUID spaeter in einstellungen.env bei WISP_ADMINS ein."
    }
}

# 3. Done.
Write-Host ''
Good 'Fertig eingerichtet.'
Say 'Jetzt: "2 - Hauptserver starten.cmd" doppelklicken.'
Say 'Fuer einen Server, der immer laeuft, lies README.md, Abschnitt "Immer online".'
