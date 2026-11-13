[CmdletBinding()]
param(
    [string]$OutputDirectory,
    [string]$IsccPath,
    [string]$EngineBuildDirectory
)

$ErrorActionPreference = 'Stop'
$repository = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $repository 'dist' }
$OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null

if (-not $IsccPath) {
    $compiler = Get-Command ISCC.exe -ErrorAction SilentlyContinue
    if ($compiler) { $IsccPath = $compiler.Source }
    else {
        $candidates = @(
            (Join-Path $env:LOCALAPPDATA 'Programs/Inno Setup 6/ISCC.exe'),
            (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6/ISCC.exe'),
            (Join-Path $env:ProgramFiles 'Inno Setup 6/ISCC.exe')
        )
        $IsccPath = $candidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
    }
}
if (-not $IsccPath -or -not (Test-Path -LiteralPath $IsccPath)) {
    throw 'Install Inno Setup 6 (winget install JRSoftware.InnoSetup), or pass -IsccPath.'
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw 'Install Rust stable with the x86_64-pc-windows-msvc toolchain and Visual Studio C++ build tools.'
}
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
if (-not (Test-Path -LiteralPath $vswhere)) { throw 'Visual Studio C++ build tools are required.' }
$visualStudio = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $visualStudio) { throw 'Finish installing Visual Studio C++ build tools and the Windows SDK first.' }
$developerCommand = Join-Path $visualStudio 'Common7/Tools/VsDevCmd.bat'
if (-not (Test-Path -LiteralPath $developerCommand)) { throw 'The Visual Studio developer environment was not found.' }

$manifest = Join-Path $repository 'launcher/Cargo.toml'
$metadata = & cargo metadata --manifest-path $manifest --no-deps --format-version 1
if ($LASTEXITCODE -ne 0) { throw 'Could not read the launcher version.' }
$version = (($metadata | ConvertFrom-Json).packages | Where-Object name -eq 'wisp-launcher').version
if (-not $version) { throw 'Launcher version missing.' }
$originalVersion = $env:WISP_VERSION
if (-not $EngineBuildDirectory) { $EngineBuildDirectory = Join-Path $repository 'target/installer-game' }
$originalRustflags = $env:RUSTFLAGS
$originalEncodedRustflags = $env:CARGO_ENCODED_RUSTFLAGS
$originalTargetDir = $env:CARGO_TARGET_DIR
$originalDeveloperCommand = $env:WISP_VSDEVCMD
$originalManifestPath = $env:WISP_MANIFEST_PATH
$buildTarget = Join-Path $repository 'launcher/target/installer'
try {
    $env:WISP_VERSION = $version
    # Keep the installer self-contained: the launcher statically links the MSVC runtime.
    $env:RUSTFLAGS = '-C target-feature=+crt-static'
    $env:CARGO_ENCODED_RUSTFLAGS = $null
    $env:CARGO_TARGET_DIR = $EngineBuildDirectory
    $env:WISP_VSDEVCMD = $developerCommand
    $env:WISP_MANIFEST_PATH = Join-Path $repository 'Cargo.toml'
    & $env:ComSpec /d /c 'call "%WISP_VSDEVCMD%" -no_logo -arch=x64 -host_arch=x64 && cargo build --manifest-path "%WISP_MANIFEST_PATH%" --locked --profile fast --target x86_64-pc-windows-msvc -p wisp-client -p wisp-prepare -p wisp-shader-api'
    if ($LASTEXITCODE -ne 0) { throw 'Rust engine build failed.' }
    $env:CARGO_TARGET_DIR = $buildTarget
    $env:WISP_VSDEVCMD = $developerCommand
    $env:WISP_MANIFEST_PATH = $manifest
    # Initialize MSVC in a child shell so an unrelated link.exe on PATH cannot win.
    & $env:ComSpec /d /c 'call "%WISP_VSDEVCMD%" -no_logo -arch=x64 -host_arch=x64 && cargo build --manifest-path "%WISP_MANIFEST_PATH%" --locked --release --target x86_64-pc-windows-msvc'
    if ($LASTEXITCODE -ne 0) { throw 'Launcher release build failed.' }
}
finally {
    $env:WISP_VERSION = $originalVersion
    $env:RUSTFLAGS = $originalRustflags
    $env:CARGO_ENCODED_RUSTFLAGS = $originalEncodedRustflags
    $env:CARGO_TARGET_DIR = $originalTargetDir
    $env:WISP_VSDEVCMD = $originalDeveloperCommand
    $env:WISP_MANIFEST_PATH = $originalManifestPath
}

$shaderSource = Join-Path $repository 'shaders/wisp/aurora'
$shaderVersion = [regex]::Match((Get-Content (Join-Path $shaderSource 'wisp-shaders.toml') -Raw), '(?m)^version\s*=\s*"([^"]+)"').Groups[1].Value
if (-not $shaderVersion) { throw 'Aurora shader version missing.' }
$shaderOutput = Join-Path $OutputDirectory "Wisp-Aurora-$shaderVersion.hshader"
& (Join-Path $EngineBuildDirectory 'x86_64-pc-windows-msvc/fast/wisp-shader.exe') pack $shaderSource --out $shaderOutput
if ($LASTEXITCODE -ne 0) { throw 'Aurora shader validation or packaging failed.' }
$shaderDigest = (Get-FileHash -LiteralPath $shaderOutput -Algorithm SHA256).Hash.ToLowerInvariant()
[IO.File]::WriteAllText("$shaderOutput.sha256", "$shaderDigest  $([IO.Path]::GetFileName($shaderOutput))`n")

$metadata = & cargo metadata --manifest-path $manifest --no-deps --format-version 1
if ($LASTEXITCODE -ne 0) { throw 'Could not read the launcher version.' }
$package = ($metadata | ConvertFrom-Json).packages | Where-Object name -eq 'wisp-launcher'
if (-not $package) { throw 'Launcher package was not found in Cargo metadata.' }
$version = $package.version
$binary = Join-Path $buildTarget 'x86_64-pc-windows-msvc/release/wisp-launcher.exe'
$engineStage = Join-Path $repository ('target/installer-package-' + [guid]::NewGuid().ToString('N') + '/engine')
$kitStage = Join-Path $engineStage 'kit'
New-Item -ItemType Directory -Force -Path $kitStage | Out-Null
foreach ($program in @('wisp-client.exe','wisp-prepare.exe')) {
    Copy-Item -LiteralPath (Join-Path $EngineBuildDirectory "x86_64-pc-windows-msvc/fast/$program") -Destination $engineStage
}
Copy-Item -LiteralPath (Join-Path $repository 'research-lock.json') -Destination $kitStage
Copy-Item -LiteralPath (Join-Path $repository 'LICENSE') -Destination $kitStage
$sourcePaths = @(& git -C $repository ls-files -- mods/wisp/src/test/java crates/wisp-prepare/src/exporters.rs)
if ($LASTEXITCODE -ne 0) { throw 'Could not enumerate the original resource-preparation sources.' }
foreach ($relativeSource in $sourcePaths) {
    if ($relativeSource.EndsWith('.java') -or $relativeSource -eq 'crates/wisp-prepare/src/exporters.rs') {
        $destination = Join-Path $kitStage $relativeSource
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $destination) | Out-Null
        Copy-Item -LiteralPath (Join-Path $repository $relativeSource) -Destination $destination
    }
}
& $IsccPath "/DAppVersion=$version" "/DLauncherBinary=$binary" "/DEngineDirectory=$engineStage" "/DInstallerOutput=$OutputDirectory" (Join-Path $PSScriptRoot 'wisp-launcher.iss')
if ($LASTEXITCODE -ne 0) { throw 'Installer compilation failed.' }

$installer = Join-Path $OutputDirectory "Wisp-Client-$version-Setup-x64.exe"
if (-not (Test-Path -LiteralPath $installer)) { throw 'The installer was not created.' }
$digest = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash.ToLowerInvariant()
"$digest  $([IO.Path]::GetFileName($installer))" | Set-Content -Encoding ascii -LiteralPath "$installer.sha256"
Write-Host "Installer: $installer"
