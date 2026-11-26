# Wisp Client

Wisp 0.8.0 is a native GPUI launcher for the Rust Minecraft engine, with the owner's
requested dark violet sidebar, scenic voxel banner, account/quick-action cards,
news, and fixed Play bar.

## Windows setup

Build from the repository root:

    ./packaging/windows/build-installer.ps1

Requires Rust stable/MSVC, Visual Studio C++ build tools, Windows SDK, Git, and Inno
Setup 6. Output: dist/Wisp-Client-0.8.0-Setup-x64.exe plus SHA-256 checksum,
and the standalone Wisp-Aurora-1.0.0.hshader package with its checksum.
The installer includes the Rust client and preparation tool. Users do not need Rust
or Visual Studio. See [Windows packaging](../packaging/windows/README.md).

## First game launch

1. Click **Install & play**. If needed, sign in with the Microsoft account that
   owns Minecraft: Java Edition. Installation continues after sign-in.
2. Wisp downloads the pinned Minecraft files, matching server JAR and preparation
   runtime directly from Mojang, verifies their checksums and prepares resources.
   Valid files are reused when retrying an interrupted installation.
3. Wisp creates the instance and starts the **bundled Rust game** automatically.
   Later launches use **Play** without repeating setup.

The Rust client is already part of the Windows installer and repository. Downloaded
Minecraft files stay in the launcher's private data directory and are never bundled
or committed. No separate Minecraft installation, server-JAR selection, or manual
JDK installation is required for the normal flow. Java is used only by resource
exporters; gameplay runs in Rust.

Advanced **Prepare local files**, **Use prepared resources**, and **Import local
build** remain available. Local preparation needs the matching game files and
JDK 25; this is an optional advanced path. Existing profiles/worlds are preserved.
Mods, Versions and Shaders use dedicated catalog pages with search, filters,
sorting and grid/list views. Counts, installed state and launch history are local
data, not mock catalog statistics. Native client modules currently include Zoom;
Java loaders such as Fabric are not supported. The original Wisp Aurora shader
can be installed and selected per instance; native .hshader imports are validated.
Resource packs open their instance directory; Multiplayer is in the game's menu.
See [Aurora](../shaders/wisp/aurora/README.md) for its effects, options and limits.

## Microsoft account

The microsoft.com/link device-code flow includes Xbox Live/XSTS exchange,
Minecraft ownership/profile checks, and refresh before account-based launches.
Windows encrypts Wisp's own sign-in with current-user DPAPI. Sessions reach the
game through standard input, without token arguments or profile JSON. Other
platforms retain sign-ins only until Wisp closes.

Wisp 0.5.0 includes the owner's registered public Microsoft client ID. Microsoft
accepted the live device-code request and returned authorization_pending when polled.
Choose **Sign in with Microsoft**, open microsoft.com/link, and enter Wisp's code.
Completing user sign-in, Xbox authentication, and Minecraft API access still need
an end-to-end test; Minecraft may require application approval. See
[Microsoft application setup](docs/microsoft-sign-in.md). No client secret is used.
WISP_MSA_CLIENT_ID can override the registration at build time or runtime.

## Data and development

Profiles stay at the local data directory's
wisp-local-launcher/local-profiles.json. WISP_LAUNCHER_HOME overrides it.
Old account files are never read. Setup, upgrade, and uninstall preserve profiles
and worlds. Wisp telemetry/cosmetics/relay integration stays disabled.

Build with cargo run --manifest-path launcher/Cargo.toml --release.
WISP_ENGINE_DIR may point to an installed engine folder during development.
Linux needs the GPUI packages in launcher/shell.nix; macOS needs Xcode Command Line
Tools. Old scene modules remain only for regression tests.

CLI commands: add/list/select/remove/play; install [--play]; setup --minecraft-dir --server-jar;
account status/sign-in/sign-out/check. Add accepts --binary, --resources, --home,
and optional --online. Remove only forgets a profile.
Game output: logs/local-launcher.log in the game folder.
Setup output: logs/install.log and logs/prepare.log in the launcher data folder.

    cargo fmt --manifest-path launcher/Cargo.toml --check
    cargo test --manifest-path launcher/Cargo.toml --locked --all-targets
    cargo clippy --manifest-path launcher/Cargo.toml --locked --all-targets -- -D warnings

Follow AGENTS.md isolation rules. Tests use fixture accounts and fresh directories.
Real gameplay and live login require your own resources and app registration.

[Artwork provenance and prompts](assets/artwork/README.md).

Wisp 0.5.0 also renames the native engine, resource preparer, internal crates and identifiers, and game startup artwork. Upgrade handling preserves earlier profiles and worlds; original licenses and explicit compatibility aliases remain. See UPSTREAM.md for provenance.

On Windows, starting the game keeps the console hidden. Diagnostic output remains in the instance’s logs/local-launcher.log file.

## Client mods

Select an instance and open **Mods > Install Zoom** or **Import .amod**.
Start Minecraft, hold **Z** and scroll to adjust magnification beyond 10×.
Enable/disable and options are per instance. Restart the game after launcher changes.
This first module API supports Zoom; see [format, controls and limits](../docs/amod.md).

## Built-in client features (0.9)

Open Mods with an instance selected to configure Full Bright and the built-in
client features. These switches share the game's Options / Wisp / Extras
preferences and are locked while playing. Wisp HUD is optional and off by
default. Enable Mod Menu to open the Extras page with Right Shift. See
[release notes](../packaging/windows/releases/0.9.0.md) for the feature scope.
