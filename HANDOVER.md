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
  `referenzen/` (the owner's references), `crates/astrum-cosmetics-server/target` (an old build
  folder, safe to delete).

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
- Launcher 0.14.1 (`packaging/windows/releases/0.14.1.md`); tests, fmt and clippy pass. Pushed to
  `wisp main`, so `local-launcher` builds and publishes `launcher-v0.14.1`. Check with
  `gh run list --repo Lyvt-Dev/Wisp-Client --workflow launcher.yml`.
- Players on 0.14.0 install 0.14.1 by hand once. The first real test of the updater is the next
  release (0.14.2): a 0.14.1 launcher should update itself.

### 3. Wavy capes: designed, not started

The owner wants capes that wave realistically (like the "Wavey Capes" mod), for the Mojang cape
and ideally the Shop capes, as a Wisp Menu module that can be switched off.
- The Mojang cape: `crates/wisp-client/src/mobs/render.rs` `pub fn cape` draws the
  `player#cape` model as one box turned by vanilla's flap/lean
  (`camera.rs` `ClientAvatarState::cape`). Plan: find the `cape` part's matrix the way
  `draw_normals` walks the tree, slice its six faces into ~16 strips along the cape's height and
  bend them along a curve: angle(u) = amplitude(u) * sin(phase - u*2.6π) + curl(u), with
  u = height 0..1, amplitude growing to the bottom and with speed (vanilla `lean`), phase from
  `m.age + alpha`; a gentle sway when standing. Stateless, so other players get it too.
- The Shop capes are cosmetic `back` items (`crates/wisp-client/src/game/cosmetics.rs`
  `cosmetics_geometry`, GPU skin path), animated only by the item's `animations`. Either split the
  cape into chained child parts with phased `swing` animations in
  `crates/wisp-cosmetics/examples/shop_items.rs` `cape_item` (data only, but not driven by
  movement), or bend their vertices like the Mojang cape on the CPU path.
- The module: add a `module!` row to `crates/wisp-client-render/src/extras.rs` `MODULES` and a tile
  in `extras/features.rs` `FEATURES` (category Visual), with an English text in
  `assets/wisp/lang/en_us.json` (the language test checks every key).

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
