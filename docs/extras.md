# Wisp extras

Optional conveniences built into the client: fullbright, dynamic lights, your own name tag, hiding block-break particles, auto-reconnect,
Mouse Tweaks-style inventory controls, inventory sorting, a smooth chat and a music page. **Every one is off by default**: a fresh
install looks and behaves exactly like vanilla 26.3, and turning an extra off
again restores vanilla's behaviour completely.

## Where they live

| Part | Code |
|---|---|
| Settings, parsing and saving | `crates/wisp-client-render/src/extras.rs` |
| FPS and click counters (for native mods) | `extras.rs` (pure logic, unit tested) |
| Dynamic lights: item glow, nearest sources | `crates/wisp-client-render/src/dynamic_light.rs` (unit tested), `game/dynamic_lights.rs`, `terrain.wgsl` and `entity.wgsl` (`dynamic_block_light`) |
| HUD drawing (a native mod's minimap, corner stacking) | `crates/wisp-client/src/extras_hud.rs` |
| Minimap colours, tiles and image (for the Minimap native mod) | `crates/wisp-client-render/src/minimap.rs` |
| Reconnect policy and backoff | `crates/wisp-client/src/reconnect.rs` |
| Game glue (input hooks, per-frame update) | `crates/wisp-client/src/game/extras.rs` |
| Mouse Tweaks-style controls, sort buttons | `crates/wisp-client/src/game/tweaks.rs` |
| Sort order and its clicks | `crates/wisp-client/src/inv_sort.rs` (pure, unit tested) |
| The music page's rows, and the pools it filters | `options_ui.rs` (`Page::Music`), `crates/wisp-client/src/music.rs` (`track_groups`, `MusicManager::set_off`), `sound_events.rs` (`sound_except`) |
| Cosmetics (show mine, show others'), off with their flag | `extras.rs`, `crates/wisp-client/src/game/cosmetics.rs` ([cosmetics.md](cosmetics.md)) |
| The settings page | `options_ui.rs` (`Page::Extras`): **Options → Wisp Settings... → Extras...** |

The settings are grouped by extra, under a header each, two buttons to a row as
vanilla's options lists are, and each says what it does in its tooltip. An
extra's own settings (its key, corner, size...) are greyed out while its switch
is off (`Item::parent`).

`game.rs` only gets one-line hooks (key, wheel, mouse, frame, HUD), so the
extras stay out of the way of other input and options work.

## Options storage

`wisp-extras.txt` beside `options.txt`, in the same `key:value` format as
`wisp-graphics.txt`. `options.txt` is untouched, so a vanilla client can share
the folder. Unknown or broken lines fall back to the defaults.

Extras' keys (the sort key) are Wisp settings stored in that file and bound on
the Extras page, not in vanilla's Key Binds screen: Wisp has no Wisp category
in Controls, and adding one would put a non-vanilla mapping into `options.txt`.

## The extras

### Fullbright (`fullbright`, default off)
- Displays blocks and entities with a fully bright lightmap, including in dark caves.
- Turning it off restores the normal lightmap using the current brightness,
  time of day and effects. The brightness slider keeps its normal range.

### Dynamic lights (`dynamicLights`, default off)
- A light source held by a player or mob (main or off hand), dropped as an item, or an entity on
  fire lights the blocks, entities and hand around it, as dynamic-light mods do. Not
  in vanilla; the idea is ported, not a mod's code (issue #814).
- An item glows like its block's default state (`getLightEmission`): torches 14, lanterns,
  glowstone, sea lanterns and jack o'lanterns 15, soul torches 10, a redstone torch 7; a lava bucket
  15. An entity on fire gives 15.
- Client-only, at render time: the light engine, meshes, saves and other players never see it.
  Each frame the game picks the 16 sources nearest the camera (within 96 blocks,
  `dynamic_light::select`); terrain.wgsl and entity.wgsl raise each vertex's block light to the
  source's level minus its distance in blocks when that is brighter, then light it through the
  lightmap as usual. The light doesn't stop at walls.
- Off: nothing is gathered and the shaders' loop runs zero times (the count is 0), so it costs
  nothing measurable (numbers in the pull request, #814).

### Moved to native mods

The **FPS counter**, **Keystrokes**, **Zoom** and the **Minimap** are official native mods now (in
Lyvt-Dev/Wisp-Client-mods under `native/<name>/`, [mods.md](mods.md#wisps-official-mods)), with the same look and
options. Players install them from the launcher's Mods page, and the launcher builds them into the game
([native-mods.md](native-mods.md)). The game keeps
the hook points they use (`crates/wisp-client/src/game/native_hooks.rs`), and `wisp-extras.txt` keeps the old
keys and `movedToMods`.

The game still draws the minimap, for the native mod (`native_minimap` in `native_hooks.rs`, [mods.md](mods.md)):
- a corner map (top right by default), square or round, small/medium/large, zoom 0.5 to 4 blocks per pixel, rotating
  with the player or north up, with the player arrow (vanilla's map decoration sprite);
- vanilla's `MapColor` per block, as map items use, with the map item's height shading and water-depth dithering;
- underground, in the Nether and under any roof, the floor around the player's height instead of the top block;
- one cached 16x16 tile per loaded chunk (packed map colour and height), filled nearest first and refreshed when
  blocks change within 0.3 ms a frame; the 128x128 image is recomposed only when the view moved or a tile changed, and
  uploaded into a dedicated atlas slot. With no mod showing a map, nothing runs.

### Own name tag (`ownNameTag`, default off)
- In third person (F5, either view), the player's own name shows above its head,
  as other players see it: the profile name in its team's prefix, suffix and
  colour, the below-name score under it within 10 blocks, faded and without the
  see-through copy while crouching. Vanilla never names the camera entity
  (`LivingEntityRenderer.shouldShowName`).
- Hidden in first person, while sleeping, in spectator mode, while invisible, and
  whenever the team's name tag visibility would hide it, exactly as the other
  players' tags (`game/names.rs`, `own_name_tag`). Client-only: nothing is sent.

### Hide block-break particles (`hideBlockParticles`, default off)
- Hides the debris particles that burst from a block when it breaks
  (`ClientLevel.addDestroyBlockEffect`) and the crumbs off the face you mine
  (`addBreakingParticles`). Other particles (critical hits, smoke, rain...) are
  unchanged, and the block still breaks and drops as usual. Client-only:
  nothing is sent (`game.rs`, `destroy_particles` and `crack_particle`).

### Auto-reconnect (`autoReconnect`, default off)
- On the "Connection lost" screen after a multiplayer connection drops, a
  countdown "Reconnecting in N s" with a Cancel button, then a new connection
  to the same address.
- Backoff 5, 10, 20, 40, 60 s; at most 5 attempts, then it stops. The count
  only resets after staying connected for a minute, so a server that drops
  players at once never causes an endless loop.
- Never after a kick, a ban, a whitelist or version refusal, a duplicate login,
  an online-mode refusal (no account signed in), or a session refusal from the
  account's login (invalid session, banned from playing online, multiplayer
  disabled, the server failing to verify the username). Unreachable session
  servers, on either side, are retried with the same backoff.
- Online-mode servers: the retry joins with the account the game signed in
  with, held in memory for the whole run (`auth::current()`); nothing is
  written to disk.

### Keep channels for UI sounds (`uiSoundReserve`, default off)
- Positional sounds (a block breaking, a mob, an explosion) start only while at
  least 16 of the 247 static sound channels are free; sounds played to the
  listener (`relative`, or no attenuation: UI clicks, toasts) may take those 16.
  A flood of world sounds (a big explosion, `/fill … destroy`, a mod breaking
  hundreds of blocks at once) then can't stop the pause menu's clicks.
- Vanilla 26.3 gives all 247 to whatever starts first
  (`Library.acquireChannel` returns null once the pool is full); with the
  extra off Wisp does exactly that. Below 231 sounds at once nothing
  changes either way. Streaming sounds (music, records) are untouched
  (`sound_engine.rs`, `listener_reserve`). Client-only.

### Mouse Tweaks controls (`mouseTweaks`, default off; `mouseTweaksWheel`, default on)
In the player inventory, chest-like containers (chests, barrels, ender chests,
shulker boxes, hoppers, dispensers and droppers), furnaces (also blast furnaces
and smokers) and crafting tables, in singleplayer and on servers. Anvils,
enchanting tables, merchants, beacons, brewing stands, crafters and the other
workstations keep vanilla's controls:
- Right-drag holding a stack: one item goes into each slot entered, at once
  (slots holding another item, or full, are skipped).
- Left-drag holding a stack: each slot entered that holds the same item is
  gathered into the hand, when it all fits. This takes over only while
  vanilla's drag covers one slot, so a left-drag across empty slots still
  splits the stack as vanilla does.
- Shift + left-drag: every slot entered is quick-moved (with a stack in hand,
  only the slots of its item).
- The wheel over a slot, with an empty hand (`mouseTweaksWheel`): down moves
  one item where a Shift click would send it (vanilla's `quickMoveStack`: fuel
  to the fuel slot, a smeltable to the input, into the crafting grid, across a
  container, between hotbar and main inventory, an armour piece onto the
  player), up brings one of the same item from the other side.
- Nothing goes into a result slot. The wheel down over one takes one craft, or a
  furnace's whole output, to the inventory when it all fits (a result can't be
  split or put back); the wheel up over one does nothing, and it isn't a source
  for the wheel up elsewhere. A right-drag skips it, and a right press on it is
  vanilla's.

Every action is ordinary slot clicks (`ContainerClick` on a server), so a
vanilla server sees nothing unusual. With the extra off, the hooks return at
once and the screens click exactly as vanilla's (`tools/test_menu_click_parity.py`
and the container screen tests are unchanged).

### Sorting (`sort`, default off; `sortKey`, unbound)
- A small button right of the container's title (chests, barrels, ender
  chests, shulker boxes) and one right of the "Inventory" line (the main
  inventory; the hotbar is left alone). The key, when bound on the Extras
  page, sorts the part under the cursor (the container when over no slot).
- Partial stacks of one item merge, then the stacks go in creative tab order
  (the search tab's), then by name; full stacks before the partial one,
  empty slots last. It needs an empty hand.
- Done with ordinary left clicks planned by `inv_sort::plan`, so a vanilla
  server accepts it; a unit test replays the planned clicks through the menu
  engine on random chests.

### Smooth chat (`chatAnimation`, default off; `chatIndicator`, default on)
- A new chat line slides in from the left while it fades in over a quarter of
  a second (5 ticks, `INTRO_TICKS`/`INTRO_WIDTH` in `Chat::layout`), instead of
  appearing whole on the tick it arrives. Its background and its indicator bar
  move with it, and the fractional tick from `Game::clock` drives the motion, so
  it moves every frame and not 20 times a second.
- Only the unfocused chat (the HUD's) animates: with the chat screen open the
  history is drawn as vanilla draws it, so clicking a line lands on its text.
- `chatIndicator` off removes the coloured bar at the left of a line. It hides
  the bar only, never the message.

### Scroll wheel changes settings (`wheelCyclesSettings`, default on)
- On, the wheel works as in vanilla: over a button in the settings it changes the button's value
  (`CycleButton.mouseScrolled`), and a button focused with the keyboard or a pad steps on the
  wheel wherever the cursor is.
- Off, the wheel only scrolls the list under the cursor, on every settings page, Video's included
  (`OptionsUi::wheel`, `GraphicsUi::wheel`). Scrolling through a long page then can't change an
  option that passes under the cursor (#5455). Client-only.

### Splitscreen (`splitscreen`, default off)

**Options → Wisp Settings... → Splitscreen...** turns local joining on or off at once.
In P1's single-player world, a controller P1 isn't using can press a button, then A to join
(or Y to play as P1, while P1 has no controller). Each
player picks an instance-local profile with its own options and saved progress. LB/RB on
the profile page move the view between shared positions, a new window or another monitor.
A joined player's **Splitscreen Settings** in the pause menu switches profiles in place;
Cancel keeps the current one. The switch is disabled while another player is joined; once
P1 is alone, turning it off restores the normal controller reader. Leaving it on while
alone keeps watching for joins without a compositor.

When Splitscreen is enabled, Play in the launcher also hands over its other signed-in
Microsoft accounts. They appear first in the profile picker, marked **(Microsoft)**, and
connect with their own sessions, skins and secure-chat keys. An account whose session is
unavailable appears as **(Offline)**: sign in again in the launcher and restart the game to
use it on an online-mode server. Launcher accounts cannot be forgotten from this page.

`WISP_SPLIT=N` remains a testing shortcut to enable it and add N-1 players.
The Setup map previews the monitors, windows and player views. Click a player (its name turns
yellow) and choose a window or monitor, or drag its tile onto a window, a place among another
window's players, or a monitor; then **Apply** to move the actual views. **Revert** and
**Done** discard unapplied moves. An emptied window closes only on Apply. A player joining or
leaving resets the draft so it cannot apply a move to the wrong seat. P1's pause-menu shortcut
opens Setup; another player's shortcut opens their profiles.

While a window has two or three players, a square at the bottom picks how they share it
(two: Auto, side by side, top and bottom; three: one wide on top, one tall on the left, side
by side); Apply saves it (`splitscreenTwo`, `splitscreenThree`). **Names** (top left,
`splitscreenNames`, on by default) keeps each player's name in the corner of their view; off,
it shows only for a moment when something changes. **Identify** makes every view say which
window it is in.

With a controller, A on a player's tile picks it up: left and right (or LB and RB) cycle where
it goes, the map showing it there, A drops it, B puts it back. **Give Up Pad** (top, while P1
has a controller in a world) frees P1's controller and offers it to join, for a friend to
press A; if nobody joins, the offer runs out, or it's declined, it drives P1 again. A player
whose controller disconnected shows "(no controller)" on its tile. Other players' pause menus
say **Leave** for disconnecting. `WISP_SPLIT_STATS=1` logs every 5 s where a frame's time
goes (each player's turn and render, the presents, the GPU wait) and each player's frame rate.
On a server P1 joined on this computer or its local network (a dedicated server, another
computer's LAN world: a loopback, private or link-local address, after the SRV redirect) the
others join the same server, each as its own profile. Anywhere else (a public server, even in
offline mode, or a friend's invite) an offline profile can't join: everyone who plays there owns
the game, so each player needs its own signed-in account. A name already on the server can't be
picked, and an online-mode server refuses offline profiles; both are said on the profile page
before it tries. A player the server turns away as it comes in picks again, told why; one kicked later
leaves; when P1 leaves the server, everyone does. The server's resource packs are asked once,
of P1: the others follow P1's answer.

### Music page (`musicPage`, default off; `musicOff`, none off by default)
- An extra like the rest: off, Music & Sounds is exactly vanilla's page and every
  pool plays as vanilla picks. On, it gains a **Music...** button.
- The page lists every track the loaded `sounds.json` can play
  (`SoundRegistry::tracks`): the built-in situations' own tracks first (menu,
  overworld, creative, underwater, the End, boss, credits), then the tracks only a
  biome's pool names, under **Overworld biomes** and **Nether biomes** (vanilla
  shares those between several biomes, so no heading claims one). A track a few
  pools share is listed once, from the first pool that names it, so every row has
  a pool its "play now" can use.
- **Left click** a track plays it now: it takes the music manager's slot (the track
  already playing stops), and the situation's own music waits until it ends.
- **On/Off** at the right of a row takes a track out of every pool it plays in, so
  a situation with all its tracks off plays nothing. Every track is on by default,
  so vanilla's random selection is unchanged; the off ones are `musicOff` in
  `wisp-extras.txt`.
- The pools are filtered where they are picked (`sound_except` in
  `sound_events.rs`), the manager is `music.rs`, and the page is in `options_ui.rs`.

### Client modules (`extras::MODULES`, all off by default)

On/off extras without settings of their own are rows of `MODULES` in `extras.rs`: one
`key:true|false` line each in `wisp-extras.txt`, one switch on the Extras page under the row's
header, and one toggle in the launcher's Mods page (`launcher/src/client_features.rs`). The game
asks `settings.extras.module("key")`. Adding a module is one `module!` row, its language keys
(`wisp.module.<key>` and `.tooltip`) and the code that reads it.

**Information HUD** (`game/extras.rs`, `module_lines`): lines in the HUD text corner (top left,
top right under F3) for CPS (`hud_cps`), ping from the player list, "local" in singleplayer
(`hud_ping`), eight-way facing and yaw (`hud_direction`), biome (`hud_biome`), server address
(`hud_server`), horizontal speed in blocks per second (`hud_speed`), the computer's clock
(`hud_clock`), in-game time and day (`hud_day_time`), weather (`hud_weather`), status effects
with level and time left (`hud_effects`), the held item's durability coloured by how much is
left (`hud_durability`) and arrows carried while holding a bow or crossbow (`hud_arrows`).
Client-only; off, nothing is computed.

**Visual** (all client-only):
- Always Day (`always_day`): the sky and lightmap are sampled at noon of the current day
  (`Game::visual_day_time`); the world's time, mob spawning and the server are untouched.
- Item Physics (`item_physics`, `item_draw.rs`): a dropped item at rest lies still, turned by its
  bob offset; a flat item (thinner than 0.1 along z in its ground transform) lies on its face, a
  block model stands on the ground. Items in the air spin and bob as vanilla's.
- Wavy Capes (`wavy_capes`, `mobs/render.rs` `wavy_cape`, `cape_bend`): every player's Mojang cape
  is cut into 16 strips along its height and bent instead of turned as one board: vanilla's lift
  is shared so the hem trails further than the shoulders, and a wave runs down it, stronger at the
  hem and with speed (its phase is `AvatarBob::cape_phase`, faster the faster the player moves).
  No strip turns into the back. Shop capes (cosmetic `back` items) are not bent yet.
- Arrow Trails (`arrow_trails`, `render_tick.rs`): every tick a flying arrow (not stuck) leaves six
  end rod sparks along the path it flew.
- Target Crosshair (`target_crosshair`, `hud_layout::tint_crosshair`): over a living mob or player
  (`crosshair_living`), the crosshair sprite is drawn red with normal blending instead of inverted.
- Freelook (`freelook`, `game/extras.rs`): holding Left Alt in a world turns the camera, not the
  player (`freelook_turn` before `Entity.turn`); first person switches to the back view while it is
  held and comes back on release. The emote wheel and a playing emote keep the camera.

**Combat** (all client-only):
- Health Indicators (`health_indicators`, `names.rs` `combat_tags`): a tag "health/max" over each
  living mob and player within 24 blocks, green, yellow or red by what is left, above the name when
  it has one. Max health is the mob's attribute.
- Damage Numbers (`damage_numbers`, `game/extras.rs` `tick_damage_numbers`): each client tick the
  health of mobs within 32 blocks is compared with the last tick's; a change starts a red "-n" (or
  green "+n") over the name line that rises 0.9 blocks over 24 ticks. It reads health as the client
  knows it, so on a server it follows what the server syncs.
- TNT Timer (`tnt_timer`): the seconds left on each lit TNT within 48 blocks, above it.
- Hit Info (`hit_info`): an HUD line with the eye-to-target distance of the last melee hit and the
  combo (hits each within 2 s of the last), recorded where the attack is sent (`Game::attack`,
  `remote_start_attack`).
- Item Highlighter (`item_highlighter`, `item_draw.rs`): dropped items get the glowing outline in
  their rarity's colour (`rarity_rgb`), white for common; vanilla's glowing still wins.

**Utility** (all client-only):
- Chat Highlights (`chat_highlights`, `chat.rs` `mentions`): a message that names the player as a
  whole word gets a gold indicator and one note block ping; the player's own `<name>` line and
  vanilla command feedback (`commands.*`) don't count. The name is the profile name the server knows.
- Chat Peek (`chat_peek`): holding H in a world draws the chat as the open chat screen does (every
  line, unfaded) without taking the keyboard.
- Waypoints (`waypoints`, `game/waypoints.rs`): `/wp add <name>` (no name: the coordinates),
  `/wp remove <name>`, `/wp list`, `/wp clear`, run by the window's game before anything reaches a
  server. Saved in `wisp-waypoints.txt` in the game folder by world (single player's world name)
  or server address and dimension. Each waypoint of the current place is a see-through label
  "name (distance)", drawn at most 10 blocks from the camera in its direction so it stays readable.

- Inventory Search (`inventory_search`, `game/inv_search.rs`): a search box over the inventory and
  the container screens drawn with slots (not the workstations that draw themselves). A slot
  whose item doesn't match the query (its shown name or its id, `_` read as a space) is shaded
  and its item drawn at a quarter of its colour; items sit in front of the shade, so the item's
  own vertices are darkened in `draw_inventory_slots`. Clicking the box takes the keyboard;
  Escape or Enter gives it back. The query is forgotten when the screen closes.

**HUD Editor** (Extras > HUD Editor: Open..., `game/hud_editor.rs`, `Screen::HudEditor`): over the
world, the information panel and Armor Status are outlined; dragging one moves it anywhere (saved
as fractions of the screen, `hudInfoPos`/`hudArmorPos`, `auto` for its corner), a right click puts
it back in its corner, the wheel over one steps the scale (`hudScale`, 0.5 to 2), and the buttons
cycle the text colour (`hudColor`, each line's own by default) and the plain background's opacity
(`hudBackground`, 0 to 100 %; Wisp HUD's glass plate keeps its look), Reset and Done. In the editor
an empty panel shows its name so it can still be placed. Drawing goes through
`extras_hud::draw_panel_at` and `scale_since`; a placed panel no longer takes room in its corner.

Already elsewhere: borderless fullscreen is Video > Fullscreen (borderless unless exclusive
fullscreen is chosen), fog off and weather off are Graphics options (`fog`, `weather`), entity and
particle culling are Graphics > culling modes, and vanilla's FOV slider and FOV Effects cover a
custom FOV.

## Shared rules

- All extras are hidden with F1 and drawn in the HUD layer: under screens, over
  the world, like vanilla's HUD. Extras in the top left corner give way to the
  debug screen (F3), which shows the FPS anyway.
- Corners stack: extras placed in the same corner stack downward (top corners)
  or upward (bottom corners) in the order a mod's minimap, then mods' panels. The top
  right corner starts below the status effect icons.

## Perf budget

Off: a flag check per frame per extra (no allocations, no world access).
A mod's minimap stays under 0.3 ms of CPU
per frame (tile budget) plus a 64 KiB texture upload when its image changes.
