# Handover: Wisp for Android 0.15.2 (2026-11-17)

The owner's bug list from the first real phone: very laggy (top priority), buttons near the edge
closed the app, the setup screen looked plain, the joystick was mirrored. All four addressed in
0.15.2 (`packaging/android/releases/0.15.2.md`):

- Performance: `phone::apply_phone_defaults` (once per `PHONE_DEFAULTS_VERSION`, also on
  existing installs): world at 50 % resolution, fast graphics, render 6 / simulation 5, 60 FPS,
  battery mode, 2 MiB upload budget per frame, culling. **Not measured on a real phone** (none
  here, and the emulator uses SwiftShader): ask the owner how it runs; next steps if still slow
  are a lower resolution scale or render distance 5, and profiling with `adb logcat -s wisp`.
- Edges: `WispActivity` (`packaging/android/java`, the APK's first Java) hides the system bars
  sticky; touch buttons keep `Layout::edge` free; back on the setup's home does nothing.
- New setup home (`Setup::home`): feather, big Play, account and files cards, backdrop glow.
- Not yet seen in the emulator: the PC ran out of memory during the test (the emulator's game
  files may sit in `/sdcard/Download/wisp-files-backup` there; move them back into
  `/sdcard/Android/data/dev.wisp.client/files`).

# Handover: Wisp for Android (2026-11-16, branch `android-port`)

The owner asked for the Wisp Client on phones (Android APK, everything incl. cosmetics), released
on GitHub from a new branch. **Status: released as `android-v0.15.1` (2026-11-16), tested in the
emulator.** The general handover from 2026-11-13 follows further down. Details: `docs/android.md`,
`Agent.md` › "0.15.1 Wisp for Android".

## Where it is

- Branch **`android-port`** (local, tracks `wisp/android-port` on Lyvt-Dev/Wisp-Client), based on
  `store-and-creators`, all pushed. The last commit adds the Wisp-Menu touch button (star), the
  top buttons at the top left, the keyboard only after a tap, phone first-start options
  (`tutorialStep:none`, render distance 8) and the CI fixes; it was built and run in the emulator
  except the star (blocked by item 1 below).
- Version **0.15.1** (`launcher/Cargo.toml`): launcher gets Settings › Phone › Connect a phone,
  the APK has the same version. Notes: `packaging/android/releases/0.15.1.md`,
  `packaging/windows/releases/0.15.1.md`.
- Owner decisions: game files come from the player's own PC over Wi-Fi (they are made by Mojang's
  Java code and can't ship in the APK); Android 10+, arm64; release key made by Claude.

## What is done and verified (Android 14 emulator, x86_64 + arm64 translation, SwiftShader)

Setup screen (de/en), copying the real prepared 26.3 files (747 MB) from this PC through
`wisp-phone-link`, Microsoft device code + browser intent, background/foreground (surface
recreated), the game on the shared event loop, title screen, new world (saved and listed again),
joystick, look, hotbar tap, inventory, back button, soft keyboard. Tests: `cargo test -p
wisp-phone-link` (12), `cargo test -p wisp-client --lib -- phone:: touch::` (13). Clippy `-D
warnings` clean on wisp-client, wisp-phone-link, wisp-client-input, wisp-voicechat, launcher,
wisp-android (android target).

## Open, in this order

1. ~~Menu taps on title sub-screens are lost~~ **fixed and verified (2026-11-16, emulator):** the
   cause was the emulator's slow frames (>300 ms): `touch_frame` saw a tap as a held finger and
   sent a right click, which the title screens ignore. A held finger now right-clicks only over
   item slots (`touch_hold_right_clicks`: Inventory, Creative). World row, Play Selected World,
   the star (Wisp Menu), a module toggle and the menu's tabs all react to a tap now.
2. ~~Star opens the Wisp Menu~~ **verified.** The phone now also takes the PC launcher's Wisp
   server with the game files (`Hello::wisp_server` in `wisp-phone-link`, kept in
   `private/wisp-server.txt`, set for the run through `wisp_compat::set_override`). Store and
   Friends say "No Wisp server" with a phone hint when the launcher has none (`SERVICE` is empty
   by default, so the owner must set one in the launcher before pairing). Not yet seen with a
   real server: the emulator's files were copied before this.
3. ~~Push, CI~~ **done:** `d9ed5405` pushed; `android` run 43954363330 green (signed APK).
4. ~~Test the CI release APK~~ **done (2026-11-16):** the release key differs from the debug key,
   so `pm uninstall -k` is not enough: a full `adb uninstall dev.wisp.client` is needed (the
   copied files go too). Then: setup, address 10.0.2.2 + code, 747 MB copied with a test Wisp
   server, Play, Create New World by tap, star, Store tab shows the offers ("Sign in with
   Microsoft to use the Shop"), no tutorial toast on the fresh options.
5. ~~Release~~ **published 2026-11-16:** https://github.com/Lyvt-Dev/Wisp-Client/releases/tag/android-v0.15.1
   (APK + .sha256, `Wisp-Client-0.15.1-Setup-x64.exe` + .sha256 from CI run 43954363180), made
   with `--latest=false`: the website and `install.sh`/`install.ps1` use `releases/latest` and
   need a `launcher-v*` release there (0.14.9 stays Latest). The launcher's updater only reads
   `launcher-v*`, so PCs reach 0.15.1 only by installing it from this page or once `android-port`
   is merged to main (the owner decides; that publishes launcher-v0.15.1).

**Merged to main 2026-11-16** (fast-forward, plus `34ed5e66`: Android releases from main use
`--latest=false`). `launcher-v0.15.1` is published and Latest, but **by hand**: the workflow's
`publish-release` job got `HTTP 403: Resource not accessible by integration` on
`POST /releases` (also after creating the tag with the owner's token and a rerun; repository
default workflow permissions are "read", the job asks for `contents: write`). Done instead with
the owner's `gh`: tag on `0e694d41`, `gh release create --verify-tag --draft` with the run's
artifacts, `gh release edit --draft=false --latest`. Before the next launcher release, check
Settings › Actions › General › Workflow permissions ("Read and write") or publish by hand again.
(Windows PowerShell 5.1 can't run `publish-release.ps1`: gh's stderr stops it; it needs pwsh.)

Still open: try it on a real phone; the owner must set a Wisp server in the launcher (Settings ›
General) before pairing, otherwise Store and Friends stay off on the phone (`SERVICE` is empty).

Known limits (in the release notes): no Bluetooth controllers, no mic permission prompt, no system
clipboard, a quick tap at the very bottom edge in menus was missed (gesture nav).

## Tools for this work (all under ignored `.local/tooling`)

- `. packaging/android/env.sh` sets Rust, JDK 17, SDK, NDK 27, CMake/Ninja for the Android target.
- APK: `bash packaging/android/build-apk.sh --profile fast` (~8-12 min incremental, debug key in
  `.local/android-debug.jks`) → `dist/android/`. Release profile = LTO, much longer.
- Emulator: `ANDROID_AVD_HOME=.local/tooling/android/avd .local/tooling/android/sdk/emulator/emulator.exe
  -avd wisp-test -no-window -no-audio -gpu swiftshader_indirect`; adb in `sdk/platform-tools`.
  Screenshots: `adb exec-out screencap -p > shot.png`; taps: `adb shell input tap X Y` (2400x1080).
  The emulator already has the game files and a world, so Play works right away.
- Offer files like the launcher: `target/release/examples/offer.exe <resources/26.3> 135790`, phone
  address `10.0.2.2` in the emulator (broadcast discovery doesn't cross the emulator NAT).
- Release key: `%USERPROFILE%\Documents\Wisp-Android-Signatur` (keystore + PASSWORT-LESEN.txt);
  the owner must back it up. Secrets `WISP_ANDROID_KEYSTORE_BASE64`, `WISP_ANDROID_KEYSTORE_PASS`,
  `WISP_ANDROID_KEY_ALIAS` on Lyvt-Dev/Wisp-Client.

## Side question the owner asked (answered in chat)

Running a Wisp server on Arch Linux: build `wisp-server` and `wisp-client` there (`cargo build
--release -p wisp-server`, `cargo build --release --no-default-features -p wisp-client --bin
wisp-client`, same folder), copy the PC's `%LOCALAPPDATA%\wisp-local-launcher
esources.3`
and `wisp-server import-resources` it, then `wisp-server init` / `start` (docs/hosting.md).

---

# Handover: where things stand (2026-11-13)

Read [Agent.md](Agent.md) for the project rules and history; this page is the current state and
the open work. The product is **Wisp Client** (it was Astrum, before that Horizon upstream).

## Repositories and branches

- **New public repository: `Lyvt-Dev/Wisp-Client`** (the owner's choice: clean, code only). Its
  history started fresh: `aed153eb Wisp Client 0.14.0`, then `f95697c9` (new screenshots and
  logos). Pushing to its `main` runs `local-launcher`, which builds the installer and publishes
  `launcher-v<launcher version>`. Actions are free there (public).
- Local branch **`wisp-main`** tracks `wisp/main` (remote `wisp` =
  `https://github.com/Lyvt-Dev/Wisp-Client.git`). Work and push here:
  `git push wisp wisp-main:main`.
- The old private repository `Lyvt-Dev/Astrum-Rust` (remote `origin`, branches
  `codex/wisp-rebrand`, `codex/astrum-rebrand`) is left as it was. Its Actions failed on billing,
  so no release came from it after `launcher-v0.11.4`. Don't push there.
- Untracked and not for the repository: `capeideas/`, `cosmeticideas/`, `modelsforshop/`,
  `referenzen/` (the owner's references).

## Tools on this machine

- Rust: `CARGO_HOME=.local/tooling/cargo`, `RUSTUP_HOME=.local/tooling/rustup`, then
  `.local/tooling/cargo/bin` on `PATH` (cargo is not installed globally).
- GitHub CLI: `.local/tooling/gh/bin/gh.exe` (logged in as Lyvt-Dev, scopes repo + workflow).
- Inno Setup: `.local/tooling/inno/ISCC.exe`. Local installer:
  `powershell -File packaging/windows/build-installer.ps1 -IsccPath .local/tooling/inno/ISCC.exe`
  (output in `dist/`; about 20-40 min for a clean engine build).
- Python: `py -3` (no Pillow); images are drawn with PowerShell's System.Drawing.
- Commits: author `Lyvt <138261776+Lyvt-Dev@users.noreply.github.com>` per commit (`git -c
  user.name=... -c user.email=...`; no global git identity is set), message ending with the
  `Co-Authored-By` line.

## Release 0.14.0

- Pushed to `Lyvt-Dev/Wisp-Client` `main`; two `local-launcher` runs were in progress at
  2026-11-13 03:35 UTC (the second only skips publishing if the first published). Check:
  `gh run list --repo Lyvt-Dev/Wisp-Client --workflow launcher.yml` and `gh release list`.
- A local installer `dist/Wisp-Client-0.14.0-Setup-x64.exe` exists but was built before the
  repository move (it links to the old repository): don't hand it out; use the release's.
- Other workflows (parity, nightly, mod registry, telemetry server) need local data and will
  likely fail on GitHub; the owner may want them turned off.

## Open work (the owner's last requests, in this order)

### 1. Wisp Menu colours and logo, Wisp badge: done, committed in 0.14.1

- Accent buttons are white with dark text (`ON_ACCENT`), the old violet is gone, the menu header
  draws the feather (`Ui::feather` in `ui_theme.rs`). Compiles, clippy and the 879 lib tests pass.
  **Not yet looked at in the game** (Right Shift, Edit HUD): do that on the next run.
- Every player wears `wisp_badge` (written by `shop.rs` into the worn folder, shown even with
  cosmetics hidden); the Star, Crown and Moon badges left the Shop.

### 2. Automatic updates: done, released as 0.14.1

- `launcher/src/updater.rs` is wired in: `Prefs::auto_update` (default on), the switch in
  Settings › General › Updates, the check/download thread in `dashboard.rs` (`Event::UpdateReady`)
  and the install in the 200 ms loop once no game, launch, setup or bundled engine runs. The
  installer's `[Run]` starts the new launcher after a silent install.
- Launcher 0.14.1 is published as `launcher-v0.14.1`; 0.14.2 (wavy capes, rank prefixes) was pushed
  on 2026-11-13 and builds itself; 0.14.1 launchers should update to it automatically (first real
  test of the updater).
- Players on 0.14.0 install 0.14.1 by hand once. The first real test of the updater is the next
  release (0.14.2): a 0.14.1 launcher should update itself.

### 3. Wavy capes: the Mojang cape done and released in 0.14.2; Shop capes open

- Module `wavy_capes` (Visual, off by default; Extras row, Wisp Menu tile, `en_us.json`, launcher
  lists). `mobs/render.rs` `wavy_cape` cuts the `cape` part into 16 strips and bends them by
  `cape_bend(u, flap, lean, phase)`; the phase is `AvatarBob::cape_phase` (`camera.rs`, advanced in
  `tick` faster with speed), so remote players' capes wave too. Tests: `wavy_cape_tests`,
  `cape_wave_tests`. Not yet looked at in the game: tune the numbers in `cape_bend` if needed.
- Next: Shop capes (cosmetic `back` items, `game/cosmetics.rs` `cosmetics_geometry`, GPU skin
  path): bend their vertices with the same `cape_bend` on the CPU path when the module is on.

### 4. Rank prefixes (TAB plugin, zedix.de): fixed in 0.14.2, confirm on the server

- Cause found: mixed component lists arrive as NBT with `{"": element}` wrappers; both converters
  (`scoreboard::tag_to_json`, `item_render::tag_json`) kept them, so plain parts of a team prefix
  decoded as empty. Now unwrapped (`scoreboard::unwrap_list_element`, test
  `a_team_prefix_with_a_mixed_extra_list_keeps_every_part`).
- Not verified on the server. The team log (`remote_team`, first 1000 packets) now records each
  prefix and suffix: if the owner's rank still doesn't show, find the `Wisp: team` line that adds
  `Lyvt` in the instance's `logs/latest.log` and look at its prefix. Their own tag shows only with
  the `ownNameTag` extra, in third person (vanilla never shows it).

### Disk

- 2026-11-13: old build caches deleted (~109 GB): `launcher/target`, every `target/*` but
  `target/debug` (its `incremental` too) and `crates/astrum-cosmetics-server`. The next launcher
  or installer build compiles from scratch.

### Smaller open points

- Three old signed test packages in `crates/wisp-mods/tests/index` still contain the upstream's
  old manifest names (`horizon-mod.toml`, …), and four lines in `wisp-mods` / `wisp-shader-api`
  read them. Removing them needs the test mod rebuilt (WebAssembly), a new minisign test key and
  a regenerated `index-v1.json`.
- `LICENSE`, `UPSTREAM.md` and three launcher file headers keep the upstream copyright line on
  purpose (MIT requires it).
- GPU tests on this machine: `cargo test -p wisp-client-render --lib gfx::` crashes in parallel
  (heap corruption, the Intel driver); one at a time, four `pack_post::clear_budget_tests` fail
  ("fixture buffer must have RGBA16F") in the shader-pack loader. Not looked into.
- The owner should upload `docs/media/brand/social/github-social-1280x640.png` as the
  repository's social preview (Settings › General) and, if wanted, the avatar.
