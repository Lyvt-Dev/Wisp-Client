[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$Commit,
    [Parameter(Mandatory)][string]$Repository,
    [string]$InstallerDirectory,
    [string]$NotesFile
)
$ErrorActionPreference = 'Stop'
if ($Version -notmatch '^\d+\.\d+\.\d+(?:-[a-zA-Z0-9.-]+)?$') { throw 'Invalid launcher version.' }
if ($Commit -notmatch '^[a-f0-9]{40}$') { throw 'An exact source commit is required.' }
if ($Repository -ne 'Lyvt-Dev/Wisp-Client') { throw 'Release publishing is restricted to the owner repository.' }
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
if (-not $InstallerDirectory) { $InstallerDirectory = Join-Path $root 'dist' }
if (-not $NotesFile) { $NotesFile = Join-Path $PSScriptRoot "releases/$Version.md" }
$name = "Wisp-Client-$Version-Setup-x64.exe"
$installer = Join-Path $InstallerDirectory $name
$checksum = "$installer.sha256"
foreach ($file in @($installer, $checksum, $NotesFile)) {
    if (-not (Test-Path -LiteralPath $file -PathType Leaf)) { throw "Release file missing: $file" }
}
$digest = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash.ToLowerInvariant()
$expected = "$digest  $name"
if ((Get-Content -LiteralPath $checksum -Raw).Trim() -ne $expected) { throw 'Installer checksum does not match its sidecar.' }
$assets = @($installer, $checksum)
foreach ($shader in @(Get-ChildItem -LiteralPath $InstallerDirectory -Filter 'Wisp-*.hshader' -File)) {
    $sidecar = "$($shader.FullName).sha256"
    $hash = (Get-FileHash -LiteralPath $shader.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    if (-not (Test-Path -LiteralPath $sidecar -PathType Leaf) -or (Get-Content -LiteralPath $sidecar -Raw).Trim() -ne "$hash  $($shader.Name)") { throw 'Shader checksum does not match its sidecar.' }
    $assets += @($shader.FullName, $sidecar)
}
$tag = "launcher-v$Version"
$repoArgs = @('--repo', $Repository)
$existing = & gh release view $tag @repoArgs --json isDraft,assets 2>$null
if ($LASTEXITCODE -eq 0) {
    $release = $existing | ConvertFrom-Json
    if (-not $release.isDraft) {
        $names = @($release.assets | ForEach-Object name)
        foreach ($file in $assets) {
            if ([IO.Path]::GetFileName($file) -notin $names) { throw 'The existing published release is incomplete; inspect it before retrying.' }
        }
        Write-Host "Release $tag is already published. Bump launcher/Cargo.toml for a new installer; published assets are preserved."
        exit 0
    }
} else {
    $createArgs = @('release', 'create', $tag) + $repoArgs + @('--target', $Commit, '--title', "Wisp Client $Version", '--notes-file', $NotesFile, '--draft')
    if ($Version.Contains('-')) { $createArgs += '--prerelease' }
    & gh @createArgs
    if ($LASTEXITCODE -ne 0) { throw 'Could not create the draft release.' }
}
# Retry a partial upload only when its bytes match; never replace a published asset.
$metadata = & gh release view $tag @repoArgs --json assets,isDraft
if ($LASTEXITCODE -ne 0) { throw 'Could not inspect the draft release.' }
$release = $metadata | ConvertFrom-Json
if (-not $release.isDraft) { throw 'Release changed while publishing.' }
foreach ($file in $assets) {
    $assetName = [IO.Path]::GetFileName($file)
    $asset = $release.assets | Where-Object name -eq $assetName | Select-Object -First 1
    if ($asset) {
        $download = Join-Path ([IO.Path]::GetTempPath()) ('wisp-release-check-' + [guid]::NewGuid().ToString('N'))
        New-Item -ItemType Directory -Path $download | Out-Null
        & gh release download $tag @repoArgs --pattern $assetName --dir $download
        if ($LASTEXITCODE -ne 0) { throw 'Could not verify an existing draft asset.' }
        if ((Get-FileHash -LiteralPath $file).Hash -ne (Get-FileHash -LiteralPath (Join-Path $download $assetName)).Hash) { throw 'Existing draft asset differs; use a new version.' }
    } else {
        & gh release upload $tag @repoArgs $file
        if ($LASTEXITCODE -ne 0) { throw "Could not upload $assetName." }
    }
}
$editArgs = @('release', 'edit', $tag) + $repoArgs + @('--draft=false')
if (-not $Version.Contains('-')) { $editArgs += '--latest' }
& gh @editArgs
if ($LASTEXITCODE -ne 0) { throw 'Assets uploaded, but release publication failed.' }
& gh release view $tag @repoArgs --json url,assets
if ($LASTEXITCODE -ne 0) { throw 'Could not verify the published release.' }
