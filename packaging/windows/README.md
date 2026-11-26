# Wisp Windows installer

Build a Windows x64 setup executable from the local launcher:

```powershell
./packaging/windows/build-installer.ps1
```

Requirements: Rust stable with the `x86_64-pc-windows-msvc` target, Visual Studio
C++ build tools and a Windows SDK, and Inno Setup 6. The script accepts
`-IsccPath`, `-OutputDirectory`, and `-EngineBuildDirectory`. It builds the locked
Rust game/preparer (fast profile) and launcher (release) with a static MSVC
runtime, then creates `dist/Wisp-Client-<version>-Setup-x64.exe` and a SHA-256
checksum. It also validates and packages Wisp Aurora as a standalone .hshader
with a checksum. The same shader is embedded in the launcher's opt-in catalog.
Users do not need Rust, Visual Studio, or Inno Setup.

Setup installs for the current user, without administrator rights, under
`%LOCALAPPDATA%\Programs\Wisp Client` by default. It offers a directory picker,
creates a Start menu entry, and offers an optional desktop shortcut and launch
after installation. Windows Settings > Apps can uninstall it. Running a newer
setup upgrades the same application because the installer AppId stays fixed.

The installed launcher keeps its existing profile location. Uninstall removes
the installed program and shortcuts; it leaves profiles, game installations,
resources, logs, and worlds alone. The bundle includes the Rust engine, native
preparer, original exporter sources, pins, and licenses. No Minecraft assets,
game JARs, or prepared resources are bundled. The normal Install & play flow
verifies a signed-in owner, then downloads the
pinned Minecraft files, server JAR and preparation runtime from Mojang. Gameplay
runs in the bundled Rust client. Existing local files/builds can also be imported
through the advanced options. No manual Minecraft/JDK installation is required.

Version 0.5.0 includes Wisp's registered public Microsoft client ID. Minecraft
API approval may still be required for a completed game login; see
[application setup](../../launcher/docs/microsoft-sign-in.md). WISP_MSA_CLIENT_ID
can override the registration at build time/runtime, or through a repository
variable for CI. No borrowed application ID or client secret is included.

generate-icons.ps1 regenerates the original folded-A SVG and Windows ICO branding.

The initial installer is unsigned; Windows may display an unknown-publisher
notice. Signing can be added when a code-signing certificate is available.

The `windows-installer` job in `.github/workflows/launcher.yml` builds, tests, and
uploads the setup and checksum as a workflow artifact. Download that artifact and
run the setup executable inside it; the launcher binary is an internal build
input, and the distributed Windows artifact is the installer.

The owner also requires a GitHub release for every installer version. After both
check jobs pass on `main` or `codex/wisp-rebrand`, `publish-release` downloads the
Windows artifact, validates its checksum and publishes `launcher-v<version>` with
the setup, Aurora package and their checksums. Pull requests cannot publish releases. No personal token
is needed: only this job receives `contents: write` through the Actions token.
Increment `launcher/Cargo.toml` and add `packaging/windows/releases/<version>.md`
before shipping a new installer. Existing published version assets are never replaced.

Wisp 0.5.0 also renames the native engine, resource preparer, internal crates and identifiers, and game startup artwork. Upgrade handling preserves earlier profiles and worlds; original licenses and explicit compatibility aliases remain. See UPSTREAM.md for provenance.
