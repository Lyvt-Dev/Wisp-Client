# Starts the relay for "Play with friends" (invite codes, and the connection when two players
# can't reach each other directly) on this PC. Players outside your network need TCP port 7000
# forwarded to this PC in the router, or the relay online (README.md).
. (Join-Path $PSScriptRoot 'common.ps1')

$exe = Join-Path $Bin 'wisp-relay.exe'
if (-not (Test-Path $exe)) {
    Warn 'Das Relay ist noch nicht gebaut: zuerst "1 - Einrichten.cmd".'
    exit 1
}
$s = Read-Settings
$env:PORT = if ($s['RELAY_PORT']) { $s['RELAY_PORT'] } else { '7000' }
Write-Host ''
Write-Host '  Wisp-Relay (Mit Freunden spielen)' -ForegroundColor Green
Write-Host "  Port $($env:PORT). Fenster schliessen = Relay stoppen."
Write-Host ''
& $exe
