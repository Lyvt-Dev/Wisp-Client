# Wisp-Client: Einstieg für Agenten

**Zuerst [Agent.md](Agent.md) lesen.** Dort stehen die Vorgaben des Besitzers,
der aktuelle Projektstand und die Übergabe für die Weiterarbeit auf dem Laptop.
Das ist der private Fork `Lyvt-Dev/Wisp-Client`. Die folgenden Wisp-Hinweise
beschreiben die Upstream-Codebasis; deren öffentliche Issue-/Claim-/Release-Abläufe
sind kein Auftrag, für lokale Wisp-Arbeit Upstream-Issues oder Releases anzulegen.
Bei Abweichungen gelten die ausdrücklichen Vorgaben des Benutzers und der in
`Agent.md` dokumentierte Wisp-Projektumfang.

# Wisp: a map for agents

Minecraft: Java Edition 26.3 rewritten in Rust (wgpu). Vanilla is the specification wherever it has the
feature. Read [CONTRIBUTING.md](CONTRIBUTING.md) once; this page tells you where things are.
Feature → files → harness for the most-changed areas: [docs/architecture.md](docs/architecture.md).

## Rules that bite

- **Never commit Mojang's files**: game jars, assets, decompiled source, prepared resources (`.local/`).
  `tools/check_public_tree.py` rejects them; `git config core.hooksPath .githooks` runs it on commit.
- **Keep your runs away from real player data.** Any game or launcher you start gets fresh `mktemp -d`
  folders in `WISP_HOME` (the game's data), `WISP_LAUNCHER_HOME`, `XDG_DATA_HOME` and
  `XDG_CONFIG_HOME`, plus `WISP_LAUNCHER_NO_SELF_UPDATE=1` and `WISP_NO_AUDIO=1`, and, for the
  launcher, `WISP_LAUNCHER_TRASH`, `WISP_LAUNCHER_CLIPBOARD` and `WISP_LAUNCHER_DESKTOP` pointing into the scratch home. Never write to
  `~/.local/share/{applications,wisp,wisp-launcher}` or `~/.local/bin`.
- **Mark what an agent posts.** A PR body starts with a line naming the person whose agent opened it and
  the tool: `> 🤖 Opened by <login>'s AI agent (<tool, model>), not by <login> personally.`, where `<login>`
  is the GitHub account the PR is opened from (`gh api user --jq .login`). Issues say "Filed by", reviews
  "Review by". Every comment starts with `🤖 Agent:`. Write only as the person you work for: the lines on
  this page that name Ludovic are for his own agents (account `ludovic111`), never for anyone else's.
- **Attribution.** Commits end with a `Co-Authored-By:` line for the agent that wrote them, and PR bodies
  with that agent's own "Generated with" line. Example, Ludovic's agents (Claude Code on `ludovic111`):
  first line `> 🤖 Opened by Ludovic's AI agent (Claude), not by Ludovic himself.`, commits
  `Co-Authored-By: Claude <model> <noreply@anthropic.com>`, last line
  `🤖 Generated with [Claude Code](https://claude.com/claude-code)`.
- Everything players run is Rust. Python/Java only for dev tools and harness drivers in `tools/`.
- Wisp-only features follow [docs/design-system.md](docs/design-system.md) and are off by default where
  they would change the vanilla look.
- Small PRs (< ~500 lines). Labels: `feature` (players get something new), `skip-release` (they see no
  change), none for a fix. Agents never add `release-now`: the maintainer decides it. Merges ship in batches of 5, or after
  6 hours ([docs/release-flow.md](docs/release-flow.md)).
- Contributors first: `gh pr list --state open --search <area>` before you start; never duplicate theirs.
- **No hardcoded user-facing strings.** Keep UI translation-ready from the start. In the launcher (`launcher/`), wrap visible text with `tr("...")` or `trf("... {arg} ...", &[("arg", &val)])` from `crate::i18n` instead of raw string literals or `format!`. In the game, use translation keys (`Text::tr`, `lang::get`), never hardcoded English strings.
- **Claim before you start**: comment `/claim` on the issue (no issue yet: open one, then claim it). An assigned
  issue is taken: pick another. `/unclaim` when you stop; a claim with no open PR linking it is released after 5 days.

## Crates

Game workspace (root `Cargo.toml`); `launcher/` and `crates/wisp-telemetry-server` are separate workspaces.

| Crate | Owns |
|---|---|
| `wisp-client` | Gameplay library (`src/lib.rs`: `game.rs` + `game/`, `entry.rs`) and thin executable (`src/main.rs`). Also the dedicated server (`--dedicated`). Re-exports `wisp-client-lib` for existing mod paths. |
| `wisp-client-lib` | Reusable client systems, `wisp-ctl`, probes and their tests. Its manifest points to the existing sources under `crates/wisp-client/`; the library root is `src/client_lib.rs`. It never depends on gameplay. |
| `wisp-items` | Blocks and items below gameplay: `blocks.rs` (block facts), `items.rs`, `item_data.rs` (components), `enchant.rs`, `cooldowns.rs`, `bundle.rs`, `item_nbt.rs`. Re-exported as `wisp_client::items`... |
| `wisp-command` | Command parsing (Brigadier): the vanilla tree, argument types, selectors, suggestions, the tree's network form. Re-exported as `wisp_client::command` |
| `wisp-client-core` | The client lib's bottom layer: `text.rs`, `lang.rs`, `snbt.rs`, `nbt_path.rs`, `component.rs`, `scoreboard.rs`, `bossbar.rs`, `gamerules.rs`, `block_entity_nbt.rs`, registry/particle/sound tables. Re-exported under the old paths (`wisp_client::text`...) |
| `wisp-client-input` | Key bindings and key names, toggle keys, mouse options, controller settings and glyphs, the clipboard, window options (`keybinds.rs`, `gamepad.rs`, `mouse.rs`, `window.rs`...). Re-exported under the old paths |
| `wisp-client-render` | The client's world data, rendering and screens (chunks, meshing, light, streaming, `gfx`, HUD, menus, settings): ~80 modules that wisp-client re-exports under the same paths. Nothing in it reaches gameplay |
| `wisp-world` | Vanilla world data with no gameplay loop: NBT, regions, chunks, `datafix/`, `worldgen/`, `level/` (server-side block behaviour, ticks, redstone), entity physics, items, effects, food, damage |
| `wisp-core` | Simulation primitives: Java `Random`s, `jmath` (HotSpot-exact `Math`), positions, noise, density, paletted storage, `version.rs` (the pinned version) |
| `wisp-protocol` | Network protocol 777: codecs, framing, encryption, `packets/{handshake,status,login,configuration,play}.rs`, `entity_data.rs` |
| `wisp-assets` | Resource decoding: block-state and item models, baking, atlases |
| `wisp-prepare` | Builds `.local/wisp-client/26.3` from the player's own Minecraft install (runs the Java exporters) |
| `wisp-server` | `wisp-server`: `server.toml`, CLI, supervisor of the headless game ([docs/hosting.md](docs/hosting.md)) |
| `wisp-server-files` | `server.properties`, eula, ops/whitelist/bans, usercache |
| `wisp-anticheat` | The dedicated server's anticheat, off by default: `[anticheat]` settings, violation levels, the checks' rules; the hub runs them (`lan/anticheat.rs`) ([docs/anticheat.md](docs/anticheat.md)) |
| `wisp-cosmetics` | Cosmetic items Wisp clients draw on players (off by default): the item format, bones and animations ([docs/cosmetics.md](docs/cosmetics.md)) |
| `wisp-friends`, `wisp-relay` | Play with Friends: invite codes, encrypted tunnel, the relay ([docs/play-with-friends.md](docs/play-with-friends.md)) |
| `wisp-mods`, `cargo-wisp-mod` | Content mods (`.hmod`: manifest, content fence, no code), the registry index and `.hpack`, and `cargo wisp-mod` (`new`, `build`, `run`, `check`, `registry`) ([docs/mods.md](docs/mods.md)). Code mods are native, built by the launcher ([docs/native-mods.md](docs/native-mods.md)) |
| `wisp-port-pack` | Porting Java modpacks ([docs/modpacks.md](docs/modpacks.md)) |
| `wisp-optifine` | OptiFine-format pack files below every feature that reads them: `.properties`, the name/range/block matchers, paths and `optifine/` folders, and custom entity models (`cem/`: `.jem`/`.jpm` parsing and baking) |
| `wisp-shader-api` | Wisp's own WGSL shader packs ([docs/api/shaders.md](docs/api/shaders.md)): `wisp-shaders.toml`, the `.hshader` package; shared by the game, the launcher and the registry |
| `wisp-shaders` | Shader packs (the `shaders/` folder format), rebuilt clean-room from a written spec of the format; no wgpu |
| `wisp-theme` | Design tokens and fonts for the launcher, the game's own menus and the website |
| `wisp-telemetry`, `wisp-telemetry-server`, `wisp-bench` | Opt-in data collection, its ingest server, the benchmark report format |
| `wisp-dualsense`, `wisp-threads`, `wisp-release-sign`, `wisp-bots`, `xtask` | DualSense output, thread priorities, signed release checksums, load-test bots, repo tasks |
| `launcher/` | GPUI launcher: `main.rs` (UI), `backend.rs` (installs, updates), `cli.rs`, `mcp.rs`, `*_page.rs` |

Other top-level folders: `tools/` (harnesses and dev scripts), `tests/` (Python unit tests: `python3 -m
unittest discover -s tests`), `mods/wisp/src/test/java/dev/wisplab/parity/` (Java oracles),
`docs/`, `.github/workflows/`. The website (github.com/Lyvt-Dev/Wisp-Client) is the private Lyvt-Dev/Wisp-Client: its build
reads `README.md`, `launcher/links.json`, `docs/api/`, `roadmap/roadmap.json` and `crates/wisp-theme/tokens.css` from main.

## Write tests with signal

- Test observable behavior, not words in docs, comments or source. Keep checks for machine-consumed contracts.
- Use independent expected values or a vanilla oracle; do not derive the answer from the code under test.
- Test outcomes, not only calls to stubs. Use mocks to isolate behavior, not as the behavior itself.
- Skip tests of trivial constants, derives and getters, or behavior already covered by another test.
- Avoid pinning implementation details; a harmless refactor should not break a behavior test.
- Make slow tests faster without removing cases or weakening their coverage.
- Never delete parity harnesses or tests that pin vanilla behaviour: they are the spec.

## Build and test fast

```sh
python3 tools/pr_ready.py                  # the self-check: fmt, clippy -D warnings, touched crates' tests, harnesses; paste its block in the PR
python3 tools/pr_ready.py --quick          # same without harnesses
cargo test -p wisp-client -p wisp-client-lib --lib --bins --tests   # skips the 104 examples (pr_ready does this); `--doc` separately
cargo test -p wisp-client-lib --lib mod_content::tests:: -- --ignored   # the 5 slow content-root tests, #[ignore]d; pr_ready runs them when mod content, assets, blocks or items change
cargo build --profile fast -p wisp-client        # optimized, quick to link: use it to play or run lan_e2e
python3 tools/parity.py --changed origin/main       # the harnesses your branch can affect (CI's parity job)
python3 tools/parity.py --changed-files paths.txt --list   # which harnesses cover these files, without running them
python3 tools/parity.py mob_ai sculk       # named harnesses
python3 tools/lan_e2e.py --check chest     # two real games over LAN (server-thread host)
python3 tools/agent_control_e2e.py         # wisp-ctl / MCP end to end
```

- Harnesses and the game need the vanilla data in `.local/` (`minecraft/`, `wisp-client/26.3`, `datagen/`,
  `tools/`): see [docs/parity-workflow.md](docs/parity-workflow.md). Without it, pr_ready lists the
  harnesses for CI.
- A harness = `tools/test_<name>_parity.py` (324 of them) + a Java oracle (`mods/wisp/src/test/java/dev/wisplab/parity/Vanilla*Oracle.java`)
  + a Rust probe: `crates/<crate>/examples/<name>_{probe,fixture}.rs` (wisp-world 175, wisp-client 104),
  or the game binary with a fixture env var (`game/dispense_fixture.rs`, `game/sculk_fixture.rs`,
  `game/command_harness.rs`, `game/net_harness.rs`). Copy a nearby harness for a new one.
- Developing a native mod? Run `wisp-launcher settings native-profile dev` once. `dev` builds the game crate
  unoptimized: a mod edit rebuilds about 6× faster than with `performance`, while game logic runs up to about twice as
  slow. Use it while developing, not for playing; `performance` stays the default for players and releases
  ([docs/native-mods.md](docs/native-mods.md)).
- Probes build into `.local/parity-target/`; harnesses find binaries via `lab.cargo_target()`, never `target/`.
- Harness selection uses Cargo dep-info and reachable-code narrowing (`tools/parity_reach.py`).
  `wisp-client-lib` builds the control tool and client probes without gameplay; their dep-info
  excludes `game.rs` and `game/`, so gameplay edits do not rebuild or select those probes.
  Source paths remain in `crates/wisp-client/`. Measure actual harness selection on refreshed
  records with `tools/parity.py --changed-files paths.txt --list`; historical counts are not a guarantee.
- Rust integration tests: `crates/wisp-client/tests/` (`lan_bot.rs`, `generation_saves.rs`, `streaming_meshes.rs`).
- Linux needs `libasound2-dev libudev-dev`; `--no-default-features` drops audio and controllers (the server build).
- Run and drive the game (starts the `wisp-client` beside `wisp-ctl`; other arguments go to the game):
  `target/fast/wisp-ctl --socket $PWD/ctl.sock launch --world W --creative --offscreen --wait --resources $PWD/.local/wisp-client/26.3`,
  then `wisp-ctl --socket … state|keys|turn|command|screenshot|quit`
  ([docs/wisp-ctl-commands.md](docs/wisp-ctl-commands.md)).

## Vanilla reference

- Start parity work from the port ledger, not grep: `python3 tools/port_ledger.py issue N` (or `where
  Class.method`, `what file:line`), and `check --base origin/main` before the PR. How:
  [.claude/skills/vanilla-parity/SKILL.md](.claude/skills/vanilla-parity/SKILL.md), [docs/port-ledger.md](docs/port-ledger.md).
- Decompiled sources: `.local/minecraft/26.3/sources/{client,server}/` (`python3 tools/lab.py decompile`).
- Bytecode queries (callers through lambdas, every overload):
  `python3 tools/rea_vanilla.py method net.minecraft.world.level.block.DispenserBlock dispenseFrom`
  ([docs/decompilation-status.md](docs/decompilation-status.md)).
- A harness runs the oracle against the real 26.3 jars and the Rust probe on the same inputs, then compares
  their JSON. Oracle runs are cached by `tools/parity_shim.py`; `--fresh` reruns them.
- Mirror vanilla's `Math` calls with `wisp_core::jmath`, and vanilla's RNG with `wisp_core::random`:
  random draws must happen in vanilla's order.

## wisp-client: where things live

`L:` = lib (`crates/wisp-client/src/`, or `crates/wisp-client-render/src/` for the modules that crate holds, under
the same `wisp_client::<module>` paths), `G:` = gameplay (`crates/wisp-client/src/game/`), also in the lib in normal builds.
Gameplay keeps its `wisp_client::<module>` imports through a self-crate alias. The executable
contains only the allocator, mod API marker and a call to `entry::main`. For unit tests, the lib
excludes `entry` and `game`, and the executable test target owns `game` so its process-wide test
state stays separate from the library tests. This compiles gameplay twice for executable tests.

| Subsystem | Files |
|---|---|
| Game loop, tick, frame | `game.rs`: `fn run`, `impl ApplicationHandler for Game` (`window_event`, `about_to_wait`), `fn tick`, `fn tick_world`, `fn render`; L: `clock.rs`, `spans.rs`; `wisp-client-input/src/window.rs` |
| Rendering | L: `gfx.rs` (`Renderer::new`, `Renderer::render`), `gfx/` (cull, occlusion, translucency, upload, timing, lod, weather, stars, end_sky), `*.wgsl`, `particles.rs`, `block_entity_render.rs`, `item_render.rs`, `fog.rs`, `clouds.rs`, `camera.rs`, `first_person.rs`; G: `render_tick.rs`, `item_draw.rs` |
| Chunks, streaming, light | L: `streaming.rs` (worker pool: generate, light, mesh), `world.rs`, `chunk.rs`, `chunk_map.rs`, `light.rs`, `mesher.rs` + `mesher/`, `vanillagen.rs` (binds wisp-world's generator), `lod.rs` + `lod/`, `server_terrain.rs` (a server's kept chunks), `bluemap.rs` + `bluemap_detect.rs` (far terrain from its web map), `ticket_levels.rs`; G: `world_loading.rs`, `postprocess.rs`; game.rs `fn update_world` |
| World generation | `crates/wisp-world/src/worldgen/` (`generator*.rs`, `feature_graph.rs`, `structure/`, `carver/`) |
| Block behaviour | `crates/wisp-world/src/level/` (`mod.rs`, `tick.rs`, `hooks.rs`, `redstone.rs`, `place.rs`, `interact.rs`); game.rs `fn tick_blocks`, `fn process_level_events` |
| Mobs | L: `mobs.rs` (types), `mobs/` (`ai.rs`, `goals.rs`, `brain.rs`, `*_brain.rs`, `nav.rs`, `path.rs`, `species.rs`, `spawning.rs`, `render.rs`, `anim.rs`); G: `creatures.rs` (ticking, spawning, mob events), one file per mob family (`dragon.rs`, `wither.rs`, `villagers.rs`, `piglins.rs`, `golems.rs`...), `natural_spawning.rs` |
| Other entities | L: `entities.rs`, `projectiles.rs`, `arrows.rs`; G: `boats.rs`, `minecarts.rs`, `mounts.rs`, `armor_stands.rs`, `paintings.rs`, `frames.rs`, `fireworks.rs`, `tnt.rs`, `explosion.rs` |
| Player actions | game.rs (`fn use_hand`, `fn use_item`, `fn attack`, `fn tick_mining`, `fn break_block`); G: `hand.rs`, `combat.rs`, `consume.rs`, `potions.rs`, `shield.rs`, `crossbow.rs`, `trident.rs`, `fishing.rs`; L: `motion.rs` + `motion/` (movement), `collisions.rs`; crate `wisp-items`: `items.rs`, `item_data.rs`, `enchant.rs`, `blocks.rs` |
| Inventory and containers | L: `menu.rs` (`AbstractContainerMenu.clicked`), `container_screen.rs`, `containers.rs`, `creative.rs`, `crafting.rs`, `recipe_book*.rs`, `stations.rs`, `merchant.rs`; game.rs `fn menu_ops`, `fn click_inventory`; G: `stations.rs`, `trading.rs`, `recipe_book.rs`, `machines.rs` |
| Commands | parse: crate `wisp-command` (`args.rs`, `selector.rs`, `tree_*.rs`, `vanilla.rs`); run: G: `commands.rs` (dispatch) + `commands/*.rs` (`admin`, `execute`, `items`, `players`, `world_cmds`...); L: `scoreboard_cmd.rs`, `data_cmd.rs`; `wisp-client-core`: `nbt_path.rs`, `scoreboard.rs`; G: `functions.rs`, `command_blocks.rs` |
| UI screens | `Screen` enum in game.rs; L: `title.rs` + `title/`, `options_ui.rs`, `multiplayer.rs`, `pack_screen.rs`, `graphics_ui.rs`, `screen_draw.rs`, `ui.rs` (widgets), `hud.rs`, `hud_layout.rs`, `hud_overlays.rs`, `chat.rs`, `tab_list.rs`; G: `startup_menu.rs`, `screens.rs` (pause, death), `options.rs`, `lan_screen.rs`, `chat.rs`, `sign_edit.rs`, `books.rs` |
| Multiplayer client | L: `net.rs` (network thread); G: `remote.rs` (`fn handle_packet`, `fn tick_remote`), `remote_*.rs` (mob data, objects, projectiles, vehicles, stations) |
| LAN host / server side | L: `lan/` (`conn.rs` handshake + login threads, `hub.rs` + `hub/` play state, `chunks.rs`, `entity.rs`, `tracking.rs`, `guest.rs`, `playerdata.rs`); G: `lan.rs` (`LanHost`), `lan_actions.rs` (`fn as_guest`, `fn tick_guests`), `lan_*.rs` per feature, `lan_entities.rs` (tracked entities) |
| Integrated server | G: `integrated.rs` (the server thread), `levels.rs` (one level per dimension, `fn tick_server`), `server_travel.rs` |
| Dedicated server | G: `dedicated.rs`, `dedicated/status.rs`; crates `wisp-server-files`, `wisp-server`, `wisp-anticheat` (L: `lan/anticheat.rs`) |
| Save and load | L: `save.rs` (level.dat, regions, playerdata), `entity_nbt.rs`, `options_file.rs`; `wisp-client-core/src/block_entity_nbt.rs`; G: `saving.rs`, `persist.rs` (entities); `crates/wisp-world/src/{region.rs,nbt.rs,datafix/}` |
| Input | game.rs `fn window_event`, `fn key_pressed`, `fn play_mouse`; crate `wisp-client-input` (`keybinds.rs`, `keymapping.rs`, `key_names.rs`, `mouse.rs`, `input.rs`, `interaction_input.rs`, `gamepad.rs`, `clipboard.rs`); L: `osk.rs`; G: `pad.rs`, `pad_menu.rs` |
| Audio | L: `audio.rs` (mixer), `sound_engine.rs`, `sound_events.rs`, `music.rs`, `ambience.rs`; G: `music.rs`, `subtitle_hud.rs` |
| Resource packs | L: `packs.rs` + `packs/`, `pack_screen.rs`, `atlas_sources.rs`, `sprites.rs`, `mipmap.rs`, `model_library.rs`, `font.rs`, `mobs/cem.rs` + `mobs/cem_anim.rs` (OptiFine/EMF entity models and their animations); `wisp-client-core/src/lang.rs`; crates `wisp-assets`, `wisp-optifine`; G: `packs.rs` |
| Agent control | L: `control.rs` (socket), `bin/wisp-ctl/`; G: `control.rs`, `agent.rs`, `events.rs`, `ui_control.rs` ([docs/agent-control.md](docs/agent-control.md)) |
| Mods | crate `wisp-mods`; G: `mods.rs`; L: `mod_content.rs`, `mod_list.rs` |
| Wisp extras, settings | L: `extras.rs`, `graphics.rs`, `settings.rs`, `minimap.rs`; G: `extras.rs`, `tweaks.rs`, `options.rs` |

## How `Game` is organised

- `struct Game` (game.rs, ~290 fields) holds everything. Its methods are spread over ~180 files: game.rs keeps
  the core (tick, render, input, inventory, world open/close) and each `game/<feature>.rs` adds an
  `impl Game<'_>` block for one feature, with its own state types. Fields stay private to the gameplay module and its descendants. Gameplay tests run in the gameplay library, separate from reusable-library tests.
- Find a feature's code: `grep -rn "fn tick_<thing>\|fn <verb>_<thing>" crates/wisp-client/src/game*`;
  the file is usually `game/<thing>s.rs`. Who calls it: grep the name in game.rs's `fn tick` / `fn tick_world`.
- Fields that say which mode runs: `remote` (this window is a client of a server: multiplayer, LAN guest, or
  single player on its server thread), `lan` (`LanHost`: this world is open to the network), `integrated`
  (the window's own server thread), `serving` (this `Game` *is* that server thread), `options.dedicated`.
  `fn tick` returns `tick_remote()` when `remote` is set; a server ticks its levels (`fn tick_world`).
- A server's players run through the shared gameplay code: `as_guest` swaps the guest's
  state (position, inventory, mode, mining, item use) into `Game` for one call (`game/lan_actions.rs`), and
  `levels.rs` swaps whole dimensions in the same way. Changes reach clients through the block log, level
  events and the entity tracker (`lan/tracking.rs`, `game/lan_entities.rs`).
- Shared read-only data: `Resources` (`res`: blocks, items, recipes, mob types, biomes) and `PackResources`
  (`pack`), both in game.rs.

## Single player runs on a server thread

`game/integrated.rs` runs a second `Game` (`serving`) on a "Server thread" with one level per dimension;
the window joins it as a client through memory (`remote`), exactly as a LAN guest or a dedicated server's
player does (#1514). A window's `Game` never ticks a world itself: the old in-frame path is gone
(`--in-frame` and `--no-server-thread` only print a notice; `WISP_SERVER_THREAD` is ignored).
Open `[server-thread]` issues list the gaps left.

Single player, LAN guests and dedicated servers all run the server code, so a fix there serves all three.
Test the server thread when you touch player-facing gameplay:

```sh
cargo run --profile fast -p wisp-client -- --resources .local/wisp-client/26.3 --world T               # server thread
python3 tools/lan_e2e.py --check <name>                  # host on its server thread, guest is a server client
python3 tools/parity.py dedicated_server server_entities server_player_damage
```

## Docs worth knowing

[docs/parity-workflow.md](docs/parity-workflow.md) (runner, gate, shards, scopes) ·
[docs/native-client.md](docs/native-client.md) (options, controls, wisp-ctl) ·
[docs/server-levels.md](docs/server-levels.md) (levels per dimension) ·
[docs/rust-parity.md](docs/rust-parity.md) (what is ported) · [docs/known-issues.md](docs/known-issues.md) ·
[docs/release-flow.md](docs/release-flow.md) · [docs/README.md](docs/README.md) (index).
