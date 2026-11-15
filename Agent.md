# Wisp-Client project handoff

Updated 5 November 2026. Read AGENTS.md and CONTRIBUTING.md.

## Owner decisions

- Private fork: Lyvt-Dev/Wisp-Client. Original upstream provenance is in UPSTREAM.md.
- Windows installer, native Rust Minecraft gameplay, Microsoft device-code login via microsoft.com/link.
- The latest supplied Wisp reference supersedes earlier layouts: luminous violet controls, proportional sidebar, lake hero, account/quick-action/engine cards, update panel and bottom Play dock. Render real data; never ship the reference username, avatar, instance name or status as mock data.
- The owner explicitly requested renaming internal code and identifiers as well as all visible branding.
- Preserve existing profiles, worlds, settings and account files through explicit compatibility handling.
- Preserve copyright/license notices. Never commit Mojang jars, assets or prepared data.
- The owner now requires every new installer version to be published as a GitHub release too. Successful trusted-branch launcher workflows publish the installer and checksum under `launcher-v<version>`. Increment launcher/Cargo.toml and add packaging/windows/releases/<version>.md for each new installer. Published version assets are preserved. Work on a feature branch; no messages, PR comments or issues were requested.

## Current implementation

- Launcher is a separate Rust workspace, package wisp-launcher, version 0.8.0.
- Launcher uses native GPUI, a violet sidebar/dashboard, original generated scenic artwork, icons, account and quick-action cards, and bottom Play dock.
- Registered public Microsoft application ID is embedded in launcher/build.rs. No client secret is used.
- Microsoft device flow leads through Xbox/Minecraft authentication and checks ownership before network installation. Tokens use the existing DPAPI account store on Windows.
- Install & Play downloads the pinned Minecraft 26.3 inputs and Mojang preparation JDK, prepares resources with the native wisp-prepare plus original Java exporters, creates an instance, then launches wisp-client. Gameplay is Rust.
- The game command uses Windows CREATE_NO_WINDOW, so Play and Install & Play do not open a console. A native child-process regression test verifies GetConsoleWindow is null and stdout/stderr still reach logs/local-launcher.log.
- Sessions use the game's --session-stdin pipe; child processes clear both new and legacy inherited account variables. Closing the launcher leaves gameplay running.
- Windows installer bundles both native engine executables, the original exporter source kit, checksums and licenses. Static MSVC runtime; per-user Inno Setup; no administrator access required. Stable installer AppId supports upgrades.
- All original project crates, modules, Java exporter packages, WIT namespaces, executables, paths and source identifiers have Wisp names. Existing external native mods must rebuild against the renamed crates.
- Boot screen draws the folded-A mark plus Wisp wordmark; Windows/Linux/macOS icons share the same vector. macOS disk-image artwork also uses Wisp.
- wisp-compat contains deliberate legacy aliases: old environment variables, atomic default launcher-home rename, profile path rebasing, reserved settings/resource/world-extension filename migrations, and NBT attachment key read fallback. New names take precedence; existing data is not merged or overwritten. World display names are not changed.
- Launcher rebinding changes an old bundled game path only when its parent is the current installed engine directory. Imported external programs are left alone.
- Hosted telemetry/cosmetics/server/Discord defaults are unconfigured; the fork does not impersonate upstream services. The launcher Mods page supports per-instance API 1 client modules, initially Zoom.
- Inno removes the exact obsolete engine program files and their bundled exporter-source folders on upgrade. Uninstall leaves player data intact.

## Validation and limits

- Compiler check passed for renamed game and preparer on Windows x64 MSVC.
- Four compatibility tests passed, covering atomic profile/world moves, collision protection, settings precedence, environment aliases and byte-preserving resource/attachment migration.
- Windows release launcher suite: 32 tests passed, including migrated profile/world/resource loading. Clippy passed with warnings denied.
- NBT module: 13 tests passed, including legacy namespaced attachments. Mod API marker: 3 tests passed.
- Tooling suite: 639 tests ran successfully (28 platform/dependency skips) with an isolated Python runtime.
- Full native game/preparer build passed. Final packaging and probe results are recorded with the delivered installer notes.
- Every game/launcher test must use fresh isolated WISP_HOME, WISP_LAUNCHER_HOME, XDG_DATA_HOME, XDG_CONFIG_HOME and launcher trash/clipboard/desktop paths, plus WISP_NO_AUDIO=1 and WISP_LAUNCHER_NO_SELF_UPDATE=1. Do not inspect real account files or run installer smoke tests over a user's installed launcher.
- This rebrand has not been visually tested in a real signed-in game session. No Minecraft ownership or end-to-end game download was simulated as successful.

## Build

Use Rust stable, x86_64-pc-windows-msvc, Visual Studio C++ Build Tools/Windows SDK, and Inno Setup 6.
Run packaging/windows/build-installer.ps1. It initializes MSVC in a child shell and builds static native binaries.
Stage source path renames before packaging: the preparation kit enumerates original Java sources using git ls-files.
Public-tree guard: tools/check_public_tree.py. Original Java exporter sources are allowed; Mojang files are not.

## Delivery

Current work branch: codex/wisp-rebrand, based on codex/rust-engine-login at 99130f5f4de9e439803dbfa38e9b931f6d9189d5.
Use the connected GitHub account for Lyvt-Dev. Local CLI authentication is separate; do not ask the owner to sign in again merely to upload source.
Do not rewrite main or force a changed remote branch. Preserve local changes and verify exact tree SHA when publishing through Git data APIs.

## 0.6.0 reference redesign

- dashboard/reference_views.rs implements the supplied reference. Geometry scales to both viewport width and height; Home has no scrolling, while management pages may scroll. The play dock has a fixed height so the news card remains fully visible.
- Sidebar highlights use explicit margins and width, keeping the whole selection within the sidebar. Solid hover backgrounds that clipped at the sidebar edge have been removed.
- The titlebar drag region is a separate sibling of the minimize/maximize/close buttons. Buttons use direct GPUI window actions; native non-client hit-test areas no longer cover them.
- Account name comes from saved/authenticated Minecraft account data. account_art.rs downloads only official textures.minecraft.net hashes over HTTPS, with byte/decoder limits, and extracts the skin face and hat. Missing skins use a generic account glyph. No screenshot avatar/name is embedded.
- Versions come from the pinned engine and package metadata; selected instances come from Settings; installation/running/closed/error status comes from actual tasks and Child::try_wait. Imported engine versions are labeled unknown rather than guessed from the bundled game. News describes the actual shipped build.
- Native game/preparer code is unchanged; local installers may reuse the validated 0.5.0 engine payload for this launcher-only redesign.
- Scenic hero/news assets were reconstructed from the supplied reference with built-in image_gen. Prompt/provenance records are in launcher/assets/artwork/README.md.

## 0.7.0 client modules and server cleanup

- The owner requested removal of the old featured server and a Wisp `.amod` format, with Zoom first and wheel zoom beyond the old 10× cap.
- Game startup now performs a one-time address-specific server cleanup, backing up original servers.dat bytes and preserving other entries and unknown fields. No featured server is inserted.
- `wisp-mods::amod` validates API 1 declarative client modules. The first handler is Zoom, compiled in Rust and activated by an installed manifest ZIP. Unsupported code/modules are rejected. This is not arbitrary code or Java-mod support.
- Mods page installs/imports, enables/disables and configures modules per selected instance. Fresh instances have none. Game Mods UI exposes the module and a rebindable Z key; scrolling while held claims wheel input before hotbar changes.
- Camera zoom uses logarithmic magnification, smooth transitions and proportional mouse sensitivity, with 10^12 magnification/positive-FOV precision guards. Focus loss resets held state; spyglass behavior stays separate.
- SDK builds new content and client packages as `.amod`; `.hmod` content remains compatible. `--template client` generates a Zoom module manifest. See docs/amod.md.
- This release changes native gameplay: build and bundle the new game/preparer. Do not reuse the old 0.5.0 payload used for launcher-only 0.6.0.
- Regression tests cover package/API validation, deep zoom/release/focus behavior, preservation of other instance data and backup-preserving server cleanup. Real signed-in gameplay still requires manual validation.
- Windows 0.7 validation: 35 launcher tests and strict launcher Clippy passed; 39 mod-library tests and 4 server-list tests passed. The SDK built and checked both the real Zoom package and a generated client template. Fresh native CLI probes verified launcher/preparer 0.7, game Launch API and the ownership gate before downloads. The Mods page was visually inspected with an isolated instance; install, setting persistence, disable and window close were exercised successfully.
- Expanded mod validation: all 61 tests with feature `index` and all 3 SDK tests passed. Read-only aliases accept pre-rebrand manifest filenames in content/modpack/shader archives, and old WIT import namespaces during metadata inspection. Archive bytes and signature checks are unchanged. New files and all visible UI use Wisp names. A native-bundle test's expected alphabetical order was corrected for `wisp-mixin.toml`.
- Final strict Clippy passed for the game, protocol, mods, SDK and shader API, including all targets. All 42 shader API unit/integration tests passed; the launcher suite and strict Clippy were rerun successfully after the compatibility change. The existing startup migration's nested error condition was collapsed to satisfy Clippy without changing its behavior.

## 0.8.0 catalogs and Aurora

- The three new references define Mods, Versions and Shaders: compact account/instance header, luminous sidebar, scenic summary banner, filters, real search, tabs, sorting and grid/list views. Home retains its earlier reference. Catalog content scrolls inside its results area.
- Catalogs show actual local instances, .amod metadata and native shader packages. No Fabric/Java versions, invented ratings, download counts, usernames or sample instances are included. Favourites and successful process-launch history persist in catalog-preferences.json without overwriting corrupt files.
- Zoom remains the first native client module; updates preserve disabled state and other preferences. Search supports Unicode, native IME and clipboard actions.
- Wisp Aurora 1.0.0 is original MIT WGSL under shaders/wisp/aurora. It targets the general vanilla-friendly cinematic style requested with Complementary Reimagined as visual reference; it is not a port or identical reproduction. No foreign shader source or assets are included.
- Aurora adds shadows, water reflection/refraction, clouds, foliage motion, coloured lights, bloom, shafts and edge smoothing. It is embedded in the launcher's catalog and also packaged separately as .hshader. Selection is opt-in and per instance; Low/Balanced/High presets preserve unrelated options.
- Shader installs/imports run the real native shader checker, including backend translation. Graphics selection uses pickedShaderPack in wisp-graphics.txt. Quality options use shaderpacks/<filename>.txt; preserve unknown preferences.
- The installer workflow includes Aurora and its checksum in GitHub releases. Add versioned release notes before each new installer. Published release assets remain immutable.
- Native Vulkan testing used an isolated generated world and a copy of prepared resources, without touching player worlds/accounts. All seven Aurora stages compiled on Intel Iris Xe. This is not a performance benchmark or signed-in gameplay validation. Final checks are recorded with the installer deliverables.
- All 40 Windows launcher tests passed, including real shader validation, per-instance selection/options, corrupt imports, disabled Zoom update state, Unicode search, favorites/history and existing profile/account/world preservation. Low, Balanced and High shader configurations compiled in the isolated native Vulkan renderer. The Aurora catalog thumbnail is an actual unedited test screenshot.
- Desktop automation stopped after a browser URL safety check failed during release setup. The 0.8 launcher layout therefore has compiler/test coverage but still needs a native visual interaction pass; do not claim screenshot-identical UI verification. GitHub Actions successfully published launcher-v0.7.0 without browser automation.

## 0.9.0 built-in client features

- Based on the released 0.8.0 source (c7a5a8c), not the older main branch.
- Mods catalog gains per-instance built-in feature controls; the same wisp-extras.txt
  settings back the game Extras page. Unknown preferences survive edits, malformed
  feature values are reported, and launcher controls are locked while playing.
- Full Bright, FPS, food/saturation, Mouse Tweaks, compact F3, shulker tooltip grid,
  armor durability, Right Shift menu, single-model dropped items and hitbox controls.
- Optional Wisp HUD with translucent panels and sunset accents, off by default.
- Keep the existing installer AppId and login/catalog/shader functionality.
- These are Rust equivalents, not Java mod JARs; see versioned release notes for limits.
- Source used for the 0.9.0 installer: 6ba07cd5d4a5c3daf84a94378ae30f2efb07ddfe.
- GitHub Actions run 42079695001 passed Linux checks and the Windows test,
  strict Clippy and installer build jobs. The installer artifact includes Aurora.
- Local Windows validation passed: 41 launcher tests, 9 Extras tests, 6 compatibility
  tests, game compiler check, formatting and public-tree checks.
- Installer: Wisp-Client-0.9.0-Setup-x64.exe (36,186,968 bytes).
  SHA-256: ab8e61f78f7623a497423214ff0f42b64856044eaba3fc62ef736bfd2579f07e.
- The launcher opened with an isolated test profile, but desktop app access approval
  timed out before visual inspection. No verified in-game visual or signed-in
  gameplay test is claimed. No installer was run over the user's installed launcher.
- Current working branch: codex/client-features-installer. The released 0.8 source
  and installer AppId are preserved. The earlier main-based edits were saved in a
  Git stash before switching this workspace to the pushed 0.9 source.
- Build tools downloaded for this task live under ignored .local/tooling; the
  system PATH was not modified. The installer itself needs no Rust toolchain.

- Published installer release: https://github.com/Lyvt-Dev/Wisp-Client/releases/tag/launcher-v0.9.0

## 0.10.0 resource packs and static custom skies

- The owner reported a resource pack that appeared to load indefinitely and requested
  native custom sky support. The selected 26.3 ZIP/folder contained metadata and an
  icon only; its older complete archive contained Nuit schema-1 sky assets.
- Prepared resource overlays now register an indexed view of the original archive.
  Unchanged resources retain archive reads; pack overrides, merged JSON, hidden
  companions and filters are represented in the view, including cache hits/rebuilds.
- Native static Nuit square-textured and monocolor sky layers plus vanilla celestial
  visibility controls. See docs/custom-skies.md for the supported subset and limits.
  The Nuit MIT format/layout reference is attributed in THIRD_PARTY_NOTICES.md.
- Metadata-only packs are identified in the game and excluded from overlay building.
  Reload staging logs include phase timings. No infinite reload loop was established.
- Read-only Windows model-I/O probe: 1,024 identical model files read in 12,567 ms
  individually versus 25 ms through the indexed overlay. This is a file-I/O phase
  measurement, not an overall reload or FPS benchmark.
- A repaired user pack is kept locally under ignored dist/, using the original
  supplied artwork unchanged. It must not be committed or uploaded with the release.
  The user's original archives, selected pack and game settings were not modified.
- Local checks: 22 resource-pack tests and 4 custom sky tests passed. An isolated
  native run loaded the repaired pack and drew the static sky; a reload completed.
- Source for 0.10.0: 84fb854, pushed to codex/client-features-installer and
  fast-forwarded onto codex/wisp-rebrand so the launcher workflow publishes
  launcher-v0.10.0. The publish run itself was not observed from this machine.

## 0.10.5 Shop

- The owner asked for a Shop page in the sidebar, styled like the supplied mock-up,
  with a currency and only capes, badges, a pet and trails. The first cape is the
  Founders Cape (night sky, light border, four-pointed Wisp star).
- Owner decision: capes are allowed here, overriding the upstream "no capes" rule in
  docs/cosmetics.md. Coins are earned (1,000 welcome, 100 per day), never sold.
- launcher/src/shop.rs keeps coins and items in shop.json; equipped items are written
  to shop/worn and the game reads them via WISP_COSMETICS_LOCAL, limited to the
  signed-in player with WISP_COSMETICS_LOCAL_PLAYER. No shared service exists, so
  other players don't see them; the Founders "first 50" limit is not enforced.
- Item files and card art come from crates/wisp-cosmetics/examples/shop_items.rs
  (checked by Item::parse, drawn with preview::render). The Founders Cape uses the
  owner's texture (crates/wisp-shop/assets/founders_cape_texture.png). Trails have card
  art and can be equipped, but the game has no trail renderer yet.
- Layout follows the owner's Shop mock-up (title row, banner slides, tab boxes, search
  with filter/sort, three-column cards beside a highlight card); the page scrolls.
  WISP_LAUNCHER_PAGE=shop opens it directly in debug builds.
- Checks: 47 launcher tests, strict launcher Clippy, 26 cosmetics tests, cosmetics
  Clippy, game check, formatting and public-tree check passed. The page was inspected
  from screenshots of an isolated debug launcher; the in-game cape was not yet seen.

## 0.10.6 Shop capes in game and the new logo

- The owner's Minecraft account has a Mojang cape; `covered()` hid every Wisp back item
  under it, so the bought Founders Cape never drew. An Wisp back item now replaces the
  Mojang cape (`Game::cosmetic_on_back`, `player_cape_geometry(.., own, ..)`); an elytra still
  hides it. Verified offscreen (`.local/tooling/cape-qa-0.10.6`, `run-cape-qa.ps1`,
  `cape_ctl.py`) with a stand-in cape through `WISP_OFFLINE_CAPE`: 0.10.5 drew the stand-in,
  the fix draws the Founders Cape.
- New logo: master `docs/media/brand/wisp-logo.png` (the owner's). `packaging/windows/
  generate-icons.ps1` cuts every icon from it (launcher sidebar, window icon, .ico for the
  executables and installer, hicolor, .icns, macOS backgrounds). The xtask SVG masters and the
  web/avatar images still show the old mark.
- Checks: 47 launcher tests, launcher and game Clippy with warnings denied, 15 window/icon and
  12 theme tests, formatting and the public-tree check passed. The launcher was inspected from a
  screenshot of an isolated debug build.

## 0.10.7 Shop cosmetics in singleplayer when signed in

- The integrated server names its own player by an offline UUID (`lan/conn.rs`: memory
  connections derive the profile from the name), while the launcher limits the worn folder to the
  Microsoft account's UUID (`WISP_COSMETICS_LOCAL_PLAYER`). The own avatar never matched, so
  the owner still saw their Mojang cape in 0.10.6. `Game::worn` now asks for the own avatar by the
  signed-in session's UUID (`auth::current()`), falling back to the in-world profile offline.
- Offline regression run passed (`cape-qa-0.10.6/after2.png`). A signed-in run can't be faked
  offscreen; the owner confirms it in game.
- Open: the owner's text turns into boxes with the Gengar pack; not reproducible in a clean
  instance (startup, F3+T, cache reuse all render). Their `.wisp-cache` is to be renamed and
  rebuilt; if boxes remain, read their `logs/latest.log` right after it happens.

## 0.10.9 Trails in the game, the Cosmic Fox, a redrawn owl

- HD animated textures: `texture.frames`/`frametime` (a vertical strip), frames up to 256×256
  (`MAX_TEXTURE`, `MAX_FRAMES` 64). Textures over 64×64 take one of 16 `wisp/cosmetic_hd{n}`
  256×256 atlas slots (`hud::HD_COSMETIC_SLOTS`, own `SkinSlots` in `Cosmetics::hd_slots`);
  `store_cosmetic` uploads the due frame (`frame_at`, from `effect_ticks`) for items drawn in
  the last two frames. Capes are a 40×64×4 box at scale 0.25 with 88×68 texels, 24 frames of 2
  ticks, glowing; the Founders Cape is the owner's 256×128 artwork cropped to the cape, with
  added sheen, halo, breathing star and twinkles (`founders_frames`).
- Mannequins (local item files) show trails, auras and sparkles too.

- Auras: `"slot": "aura"`, `"aura": {"style": "storm"|"orbit"|"nebula", "colours": [...]}`,
  emitted every tick by `emit_aura` (standing still too). Shop items `storm_aura`,
  `starlight_orbit`, `nebula_aura` (Kind::Aura, tab "Auras", icon `aura.svg`); the banner's third
  slide is the Storm Aura (`banner_storm_aura.png`).
- Pet sparkles: `"sparkles": [...]` on shoulder items, `emit_sparkles` at the attachment beside
  the shoulder. The model's +x is the wearer's left, so the world offset is
  `(cos body_yaw, sin body_yaw) * offset_x / 16`. Pets are bigger: owl 0.9, fox 0.8, offset x 7.
- Others' trails, auras and sparkles need `extras.cosmetics_auras` ("Others' auras and trails",
  off by default) as well as others' cosmetics shown; your own always show.
- All effect particles use `ParticleEngine::add_glowing` (full bright).

- Item format: `"slot": "trail"` with `"trail": {"style": "stardust"|"comet", "colours": [...]}`
  (1 to 4 `#rrggbb`, no parts, no texture); optional `"glow": true` on any model item (drawn at
  full block and sky light). `docs/cosmetics.md` and `wisp-cosmetics` tests cover both.
- Game: `Game::tick_cosmetic_trails` (`game/cosmetics.rs`) runs after `tick_player_model` in
  both the integrated and the remote tick. It emits through `ParticleEngine::add` for the own
  avatar (`player_model`) and for other players in `self.mobs` when others' cosmetics are shown,
  only while the wearer moved more than 0.03 blocks that tick and less than 4 (no trail across a
  teleport). Stardust: `dust_transition` motes plus an occasional small white `dust`; comet: a line of
  large `dust_transition` along the path.
- The owl's geometry was wrong since 0.10.5: its children's cubes repeated their pivot (cubes
  are placed from the part's pivot), so the head and wings floated off the body. Rebuilt in
  `examples/shop_items.rs` with a 64×64 texture. New `cosmic_fox` (10 boxes, scale 0.5). Both
  glow. Regenerate with `cargo run -p wisp-cosmetics --example shop_items -- crates/wisp-shop/assets`.
- Launcher: trails carry item files now and are written to `shop/worn`; the banner's second
  slide and the Featured highlight are the fox (`banner_cosmic_fox.png`, `highlight.png`).
- Diagnostics: a warning when the default font has no providers (`font.rs`) and for the first
  five sound files that fail to open (`audio.rs`). The owner's font boxes and silence happen
  only on their main PC (another PC is fine with the same pack and build); not reproduced here
  with any launcher-like setup, including `\\?\` working folders (Rust joins `/` paths onto
  verbatim ones correctly, so that is not the cause).

## 0.10.8 Audio comes back after a lost device; pack support list corrected

- Audio: after `DeviceNotAvailable` the output reopens once; when that failed (no device at
  all, e.g. the owner's Bluetooth headphones off), the mixer kept the lost device's name, so the
  same device returning was no change for `should_change_device` and the game stayed silent.
  `Audio::output_down` now retries once a second (own `retry_ticks`) until a device opens, and
  logs the failure once.
- GUI sprites: the `gui.scaling` `tile` type is read (`ui.rs` `parse_nine_slice`: a nine-slice
  with no borders and a tiled centre is `blitTiledSprite`); `stretch` still has no metadata.
- `docs/resource-packs.md` was stale: interpolate, `blur`/`clamp`, TrueType and text-component
  fonts, credits, villager hats, worn trims, locator bar and post chains are all implemented
  (each row now names its code and tests). Real gaps: core shaders beyond text and the block
  outline, plus the partial OptiFine/ETF rows.
- Gengar font boxes: still not reproducible, also with an in-game pack switch from the Packs
  screen (`cape-qa-0.10.6/packswitch/`). The pack only replaces `ascii.png` (HD) and legacy
  `unicode_page_*.png`, plus `§r§f` codes in `lang/en_*.json`.
- Known failing test, older than this release: `options_ui::tests::a_clicked_extras_tooltip_leaves_with_the_pointer`
  (its assert message underflows when the tooltip stays).

## 0.12.0 Lunar-style launcher; unreleased appearance settings and real content

- 0.12.0 (commit 57bb78f): the owner asked for the layout and style of their Lunar Client reference
  (`referenzen/`). Thin top bar, slim icon rail, friends column, Home with a centred Launch
  button, shortcuts and news cards. Our own branding and artwork; no Lunar assets.
- After 0.12.0 the owner asked for appearance settings and said the launcher felt empty.
  - `dashboard/theme.rs`: views are written in the default greys/green; every neutral grey is
    drawn through `tone()` and every green through `accent_tone()` (HSL shift), so
    `prefs.accent` (8 colours), `prefs.backdrop` (Midnight, Black, Slate, Ocean, Plum),
    `prefs.banner` and `prefs.side_panel` recolour the whole launcher live. New views must
    wrap their colours the same way.
  - `local_content.rs`: resource packs (zip or folder with `pack.mcmeta`; description and
    `pack.png`), worlds under `saves/`, and `servers.dat` (a small read-only NBT reader).
    `dashboard/content.rs` caches them per instance and page (8 s) with crisp icons.
  - Home: "Jump back in" world cards (`--quickPlaySingleplayer`), a last-played chip. Servers
    page: saved servers with Join (`--quickPlayMultiplayer`). Side column: daily coins,
    instances with Play, equipped items, friends note. There is still no friends service.
  - Checked with screenshots of an isolated launcher home (`launcher-film-home` copy) in
    Midnight/green, Slate/blue, Black/violet and Plum/pink. Clicking Join or a world was not
    tried end to end.

## Friends on the Wisp server (cosmetics and friends in one server)

- The owner asked for a full friend system on the same server as the cosmetics, and for setup to
  be as easy as possible. `crates/wisp-cosmetics-server/src/friends.rs`: requests by Minecraft
  name (the name is learnt at sign-in), accept/decline/remove, presence in memory (launcher or
  playing, server address, 90 s), `last_seen` in SQLite. Same Mojang `hasJoined` sign-in.
- Launcher: `friends.rs` (client, Mojang `join` via `GameCredentials::join_server`, Mojang profile
  for friends' heads), `dashboard/friends_view.rs` (worker thread: sync every 25 s, commands;
  right column with add box, requests, friends with Join, sent requests, daily reward). The
  Instances list left the right column at the owner's request.
- The server address can be set in Settings > General (`prefs.service_url`,
  `shop::set_service`), so no rebuild is needed; `SERVICE`/`API` still bake one in.
- Setup: `docs/wisp-server.md`. `Start-Wisp-Server.cmd` builds and runs it on a Windows PC,
  opens a Cloudflare quick tunnel and writes the address into that PC's launcher prefs (not run
  end to end by the agent: it installs Rust/cloudflared and changes the real prefs). Railway via
  `railway.json` with a `/data` volume; `docker-compose.yml` + Caddy for a VPS.
- Verified: server tests (requests, accept, ask-back, decline, remove, presence); a local server
  with the owner's real account signed in from an isolated launcher and showed seeded friends,
  heads and a request.
- In game (owner request): the Wisp Menu (Right Shift) has a Friends tab under the categories
  (`wisp_menu/friends.rs` draws, `game/friends_menu.rs` acts). The list comes from the
  cosmetics sharing thread (`wisp_cosmetics::share`: `POST /v1/presence` every 20 s with
  `"from": "game"` and the real server, plus `/v1/friends/...`), with the same session as wear
  and emotes. The server keeps game presence apart (`Live.in_game`) and prefers it while fresh,
  so the launcher's launch-time guess doesn't override a server switch. Join leaves the world
  (`leave_server`, saving singleplayer) and connects once the title is back
  (`friends_frame`). Friend notices slide in top right (online, new request). Avatars are the
  name's initial on a colour, not skins. Only works when the launcher hands the game
  `WISP_COSMETICS_API` (an address in Settings or built in) and a Microsoft account.

## In-game HUD widgets (owner asked for a better look and function)

- Placement by anchor (`opt.<widget>.at`, `hud_widgets::anchor_place`/`anchor_of`): a widget by
  the right or bottom edge grows away from it, one in the middle grows both ways. Older `pos`
  (top left) still reads until the widget is dragged once.
- `WidgetMemory`: a plate keeps its widest width 1.5 s with the content centred, so changing
  numbers don't shake it; Keystrokes' keys fade out over 0.18 s after release.
- Glass top highlight on plates; Ping without bars colours its value by latency.

## Launcher Locker

- The owner asked for a Locker page: the player's skin wearing their bought items, to try looks.
  `dashboard/locker_view.rs` draws it with `wisp_cosmetics::preview` (CPU, cached per skin,
  angle and selection); the skin is the account's 64 x 64 texture from `account_art`. Picking an
  item only tries it on; "Equip these" writes the wallet like the shop does. Trails, auras and
  emotes have no model and are marked "Shows in game only". The side panel's Equipped list was
  removed at the owner's request.

## 0.14.0 Wisp Client

- The owner asked (2026-11-13) for the name "Wisp Client", a feather icon, black and white
  colours, a fresh install, and a clean public repository: `Lyvt-Dev/Wisp-Client`, one commit,
  code only. Crates are `wisp-*`, variables `WISP_*`, Java packages `dev.wisplab`, lang keys
  `wisp.*`. No legacy upgrade code: `wisp-compat` only reads `WISP_*` variables and makes the
  launcher's data folder (`wisp-local-launcher`). `third_party/` is untouched. `LICENSE` keeps
  the upstream copyright line, as the MIT license requires.
- Product name "Wisp Client": installer, exe, Start menu, window titles; installer
  `Wisp-Client-<version>-Setup-x64.exe`.
- Icon: `docs/media/brand/wisp-logo.png` (white feather on black, drawn with GDI+; vector in
  `docs/media/brand/icon.svg`), cut into every size by `packaging/windows/generate-icons.ps1`.
- Colours: launcher accent `Accent::Mono` ("Black & White", the default, greys instead of the
  green) on `Backdrop::Black`; `wisp-theme`'s violet tokens are greys, accents black (light
  themes) or near white with dark text (dark themes); `tokens.css` regenerated.
- Releases: the `local-launcher` workflow publishes from `main`. On a private repository it needs
  Actions minutes; the public repository has them free.

## Unreleased: four emotes, Wisp Original Cape and Stray Head

- The owner asked (2026-11-13) for emotes Dark Ascension (dark energy envelops the player), Bruh
  (facepalm), L Dance (an L gesture dance), Celestial Wings (cosmic wings appear behind the player;
  the owner's reference lists it as an emote), and cosmetics Wisp Original Cape and Stray Head
  (a head item; the reference picture `cosmeticideas/strayhead.jpg` is a conical straw hat).
- Game: `game/emotes/moves.rs` holds the four. They have no lens (`Emote::has_lens`, `lens_at`
  now returns an Option): each is a pose over time (`moves_at`: lift, tilt, arms, head, body turn,
  stepping legs) that the own model (`moves_pose`) and other players' models
  (`tick_remote_emotes`) take, particles and sounds (`moves_effects`) and a framing camera
  (`moves_camera`). The wheel has 10 slots now (slots 5-8 the new ones, 9 empty).
- Prices (chosen by the agent, change in both `launcher/src/shop.rs` and `Emote::price`): Celestial
  Wings 1,800, Dark Ascension 1,400, L Dance 450, Bruh 300, Wisp Original Cape 1,000, Stray Head 600.
- Launcher: `Kind::Head` ("Hats", icon `assets/icons/hat.svg`), slot `head` in the item file.
- `shop_items`: `original_frames` draws the Wisp mark from `crates/wisp-theme/icons/wisp-256.png`
  on an HD cape (24 frames); `stray_item` is a 20-segment double-sided cone (quads) with a
  copper band and a straw knot; the emote cards are `Posed` (the wearer posed through
  `preview::Draw::poses`, effects projected round them). Not yet seen in a running game.

## Featured server: zedix.de (owner decision, supersedes the 0.7.0 removal)

- The owner asked for zedix.de as a featured server that can't be deleted.
- Game (`multiplayer.rs`): `Screen::open` (title screen, Refresh, back from a server) pins
  `FEATURED_ADDRESS` first in servers.dat: `servers::prepend` when missing (other entries and
  fields kept, unreadable files untouched), moved up when the player moved it. Edit and Delete are
  off for it (`is_featured`: any case, trailing dot, default port).
- Launcher (`server_ping::FEATURED_*`): a pinned Featured card with live status and Join on Home
  and Servers, independent of servers.dat; the regular Servers list leaves it out.

## Unreleased: Settings rework and shared cosmetics

- Several accounts (`auth/storage.rs`: `wisp-accounts.dat`, Use/Remove on the Account page).
  Sign-in opens Microsoft's page in a launcher window (`auth/window.rs`, wry/WebView2, run as the
  hidden `sign-in-window` subcommand) and catches the redirect to the app's registered desktop URI
  (`.../oauth2/nativeclient`); `http://localhost` is not registered for the app ID. Device code
  stays as "Use a code instead". Owner confirmed sign-in, switching and shared cosmetics against a
  local server (`.local/Start Wisp (local cosmetics).cmd`, `WISP_COSMETICS_API`).
- Launcher Settings (`dashboard/settings_view.rs`, `prefs.rs`, `launcher-prefs.json`): sections
  General, Game (window size, fullscreen, render distance as game args, previous log), Appearance
  (interface size, banner autoplay), Cosmetics & emotes (written into each instance's
  `wisp-extras.txt`), Accounts, Storage (folder sizes, clear cache), About.
- The owner asked that everyone sees each other's cosmetics and emotes and chose a hosted
  service: `crates/wisp-cosmetics-server` (own workspace, Railway/Docker). Not deployed yet:
  `shop::SERVICE` and `wisp_cosmetics::API` are empty until the owner hosts it.
- Remote emotes play particles, sounds and the lens at the other player's place; their model is
  not posed (no lying back or lifting on other screens).

## 0.11.4 Inventory Search and HUD Editor

- The owner asked for the two features 0.11.3 left open. Verified offscreen with screenshots:
  search "iron" darkens every other item and slot; the editor moved the info panel, cycled colour
  and background, and saved them. Not judged: chest screens on a server, GUI scales other than
  the QA window's.

## 0.11.3 Client modules

- The owner asked for every feature of a list of PvP-client mods that Wisp lacked. Built as 25
  client modules (`extras::MODULES`, one `module!` row each, off by default, also on the launcher
  Mods page): information HUD, Freelook, Always Day, Item Physics, Arrow Trails, Target Crosshair,
  Health Indicators, Damage Numbers, TNT Timer, Hit Info, Item Highlighter, Chat Highlights, Chat
  Peek, Waypoints. Details in docs/extras.md.
- Skipped as not applicable: Java libraries and Java performance mods. Already present elsewhere:
  fog/weather off, culling, borderless fullscreen, FOV.
- 0.11.4 adds Inventory Search and the HUD Editor (docs/extras.md). In-game items sit in front of
  flat GUI fills (vertex depth), so the search darkens non-matching items' vertices instead.
- Verified offscreen in isolated worlds with screenshots; not judged: multiplayer servers, the
  mention ping (audio off in QA), the launcher Mods page with the new rows.

## 0.11.2 The Rook

- The owner asked for an emote after the chess meme "THE ROOOK": the rook explodes forward with a
  Brilliant Move icon above it. Wheel slot 4, offer `the_rook`, 700 coins (price chosen by the
  agent; the owner named none).
- `emotes.rs`: `rook_at` (timeline `ROOK_DROP` 0.7, `ROOK_LAND` 1.25, `ROOK_BADGE` 1.75,
  `ROOK_BLAST` 2.9, `ROOK_LEAVE` 6.6, `ROOK_END` 8.0), `rook_spot` (ground ahead with
  `full_collision`, plants ignored), `rook_arms`/`rook_tilt`, `rook_effects`, `rook_camera` (picks
  the clearest of several angles round the player, judged from fixed points so it never flips).
- Render: `black_hole.wgsl` `rook` (`params.w` 4, `gfx::Rook`): a ray-marched SDF Staunton rook
  in polished ivory reflecting the world, crumbling by noise; a teal "!!" badge billboard (our own
  drawing, not chess.com's asset); a ray-marched fireball blasting forward that leaves thin grey
  smoke. The card art is a crop of an isolated QA frame.
- Verified offscreen (fast profile) in an isolated world by day: wheel, fall, landing, badge,
  blast, smoke, crumble. Not judged: sound (audio off in QA), night look, frame rate.

## 0.11.1 Zedix Cape (0.11.0 was a local build; 0.11.1 is the first published 0.11 installer)

- The owner asked for an animated cape from `capeideas/Zedix2.png` for 400 coins: offer
  `zedix_cape` (Kind::Cape). The logo is kept as `crates/wisp-shop/assets/zedix_cape_logo.png`;
  `shop_items` `zedix_frames` scales the Z onto the HD cape's outer face (24 frames, 2 ticks) on a
  dark violet cape with drifting smoke and stars, a breathing halo, a pulsing neon edge with sparks
  and a light sweep across the letter. No banner slide.
- Checked: `Item::parse` (generator), generator Clippy with warnings denied, launcher shop tests.
  The card and frames were inspected as images; not yet seen in game.

## 0.11.0 False Dreams

- The owner asked for an emote "False Dreams" (500 coins): the player floats as when possessed
  in Stranger Things, the owner's song plays, eyes float round the player and look at the
  viewer, and the player's own view is a bit blurry.
- Game (`emotes.rs`, wheel slot 3): `dream_at` is the pose (lift up to `DREAMS_TOP` 1.8, lower
  under a roof via `dream_height`; head back, arms hanging through `Extra::emote_arms`, a jerk
  about every 9 s). The emote lasts `DREAMS_END` 86 s, the song's length (a test decodes it).
  Camera in front, a little below, circling slowly.
- Song: `crates/wisp-client/assets/emotes/false_dreams.mp3` (from `modelsforshop/FALSE_DREAMS.mp3`),
  built into the game and decoded once on a thread (`dream_song`). symphonia gained its `mp3`
  feature; `audio.rs` gained `decode_bytes` and `play_samples`/`set_samples_volume`/`stop_samples`
  (a channel outside the sound events, at master × Jukebox/Note Blocks volume). Own song plays
  by the listener; another player's from their place, heard within 32 blocks, synced by offset.
- Render: `black_hole.wgsl` `dreams` (`params.w` 3, `gfx::FalseDreams`): 9 analytic eyeballs
  (`dream_eye`) turned to the camera with glances and blinks, a red glow; the blur, ghosting,
  desaturation and dark red vignette only on the player's own screen (`emote_black_hole` sets
  `blur` 0 for another player's).
- HUD colours are given with red and blue swapped (`0xAABBGGRR` in effect): the Purple Hollow's
  wheel icon shows its orbs the other way round.
- Shop: offer `false_dreams`; card art is a crop of an isolated daylight QA frame.
- Verified offscreen in an isolated world at night and by day (debug build, about 1 frame a
  second): wheel, rise, eyes, blink, blur, landing. The song was not heard in QA (audio off);
  only its decoding and length are tested. Whether the owner may ship the song is theirs to say.

## 0.11.0 Purple Hollow

- Third emote (wheel slot 2, 1,200 coins, offer `purple_hollow`): a blue orb gathers over the
  right hand and a red one over the left, the hands bring them together into a purple sphere
  with lightning, and it is fired ahead. `emotes.rs`: `hollow_at` (where everything is),
  timeline `HOLLOW_BLUE` 0.7, `HOLLOW_RED` 2.5, `HOLLOW_JOIN` 4.7, `HOLLOW_MERGE` 6.3,
  `HOLLOW_FIRE` 8.6, `HOLLOW_AWAY` 10.7, `HOLLOW_END` 13.2.
- Render: `black_hole.wgsl` `hollow` (`params.w` 2; `Lens` gained `red` and `origin`,
  `gfx::HollowPurple`). A test checks the shader validates and `Lens` is `LensUniform`'s size.
- The arms are posed through `Extra::emote_arms` (`mobs/anim.rs`, after `humanoid`), which
  other players' models get too. The wheel's locked hint now names each emote's own price.
- Verified offscreen at night and by day (debug build, about 1 frame a second, so motion and
  sound were not judged). Release frame rate not measured.

## 0.10.14 Realistic Supernova Crown and Nebula Wings

- The owner found the Supernova Crown "not realistic" next to the Black Hole and the wings not
  good-looking. The Supernova is now drawn by `black_hole.wgsl` (`supernova`, chosen by
  `params.w`; `Lens` gained `nova` and `crown`, `gfx::Supernova`): a star with a boiling surface
  and corona, collapse, burst, a ray-marched filament shell with a pulsar, and an analytic crown
  on the head. Particles only add infall, debris and light falling onto the head.
- Nebula Wings: `shop_items` draws layered feathers (`wing_feathers`) at 256x256 per frame, each
  wing an arm panel and a hand child part that lags the beat.
- Verified offscreen at night (debug build). Release frame rate and daylight look not measured.

## 0.10.13 Daylight, Nebula Wings, Supernova Crown and a tidier Shop

- Shop: Featured groups cards by category (`shop_section`, "See all" opens the tab); no counts
  on tabs or headings; the owned count sits beside the star. New kinds `Weapon` and `Wings`;
  `Offer::creator` shows "by @name" on the card. Capes and wings share the back slot
  (`Kind::shares_slot`): wearing one takes the other off.
- Daylight (800, by @gold3nz): the creator's Blockbench model (`crates/wisp-shop/assets/daylight.bbmodel`)
  converted by `shop_items` (`bbmodel_quads`) into the new `quads` part field; new slot `weapon`
  (beside a cape, hidden under an elytra). `MAX_VERTICES` is now 1024 (the sword has 812).
- Nebula Wings (1,500): generated in `shop_items` (procedural animated nebula texture, 8 frames,
  alpha-cut feathers, flap animations, glow); back items may now have sparkles.
- Supernova Crown (800): second emote in `game/emotes.rs` (wheel slot 1), reusing the lens pass's
  flash and shock wave with no hole (`strength` 0). Card art is a crop of an isolated QA frame.
- The unfinished Simple Voice Chat port (see `docs/voicechat-handover.md`) is in the build but off: it only
  runs with the `voicechat` client module, which the launcher does not install yet.

## 0.10.12 Calm Black Hole and junction fallback

- The owner wanted the Black Hole "more chill and relaxed", the player floating on their back
  without gravity. `emotes.rs`: timeline `SUMMONED` 2.5, `FADE` 13, `GONE` 14.4, `END` 15.5; slow
  even orbit, no camera shake, sparse slow particles, beacon hum and amethyst chimes; it fades
  with a small ripple instead of imploding. The lying pose is `Extra::emote_tilt` (pitch 90 lies
  on the back, roll), applied about the model's middle in `mobs/render.rs` `base_transform_at`.
- A player crashed at start with os error 448 (untrusted mount point): their game process could
  not traverse the resource pack overlay's junctions (cause on that PC unknown). `packs.rs`
  `junction` now probes each new junction and copies when it won't open; `cache.rs` rebuilds a
  cached overlay whose junctions don't open (`junctions_open`).

## 0.10.11 Emotes and the Black Hole

- The owner asked for emotes on a B wheel, the first a realistic, "juicy" Black Hole the player
  orbits, sold in the Shop for 600 coins.
- Game: `game/emotes.rs` (wheel drawn over the HUD, input via `emote_key` in `key_pressed`,
  `emote_mouse` in `play_mouse`, `emote_look` in `turn_player`; timeline, model offset after
  `tick_player_model`, particles, sounds, orbit camera after `native_camera_pose_now`). The
  wheel unlocks the ids in `$WISP_COSMETICS_LOCAL/emotes.txt`.
- Render: `black_hole.wgsl`, drawn by the world post pass (`world_post.rs` `set_lens`,
  `Renderer::black_hole`): Schwarzschild ray march with lensing, accretion disk with beaming,
  depth-aware so nearer geometry stays in front. Not under frame generation; shader packs untested.
- Launcher: `Kind::Emote`, offer `black_hole` (600), any number equipped; `Wallet::dress` writes
  `emotes.txt`. Card art is an unedited crop of an isolated QA screenshot.
- Verified offscreen in an isolated world (debug build): wheel locked/owned, the full emote.
  Release-build frame rate not measured.
