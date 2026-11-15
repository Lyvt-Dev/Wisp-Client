# Cosmetics

Small items Wisp players wear: a hat, something on the back, a shirt, a pet on the shoulder, a
style for their name tag, a trail of particles behind them, an aura around them. They are drawn by Wisp clients only and change nothing in the game.
**On by default** (Ludovic, 2026-11-04: on when the shop opens), other players' and your own shown;
the settings below hide them.

## Rules

- **Only Wisp clients see them.** Other Wisp players learn what someone wears from Wisp's
  cosmetics service, keyed by the Minecraft UUID. The game protocol doesn't change: servers, vanilla
  clients and anticheats receive nothing.
- **No gameplay effect of any kind**, no capes or anything that looks or acts like one, no random
  draws.
- **A setting hides them**: other players' cosmetics, and your own.
- **Zero cost when none is in view**, and a vertex budget per item.

## The item format (`crates/wisp-cosmetics`)

One JSON file per item. Coordinates are model pixels (1/16 block) in the player model's space:
y points down, as in vanilla's `ModelPart`s and Blockbench's "Modded Entity" models.

```json
{
  "format": 1,
  "id": "example_hat",
  "slot": "head",
  "bone": "head",
  "attach": { "offset": [0, -8, 0], "rotation": [0, 0, 0], "scale": 1 },
  "texture": { "png": "<base64 PNG>" },
  "parts": [
    { "name": "brim", "pivot": [0, 0, 0], "rotation": [0, 0, 0],
      "cubes": [{ "origin": [-4, -2, -4], "size": [8, 2, 8], "uv": [0, 0], "inflate": 0, "mirror": false }],
      "children": [] }
  ],
  "animations": [
    { "part": "brim", "motion": "swing", "axis": "z", "amplitude": 10, "period": 40, "phase": 0 }
  ]
}
```

- `slot`: `head`, `back`, `weapon`, `torso`, `shoulder`, `name_tag`, `trail` or `aura`; a player wears at most
  one item per slot. A `torso` item is hidden under a chestplate (not an elytra). A `weapon` is carried on the
  back (a sword across it) beside a cape, and hidden under an elytra like a back item.
- `bone`: the player model part the item follows (`head`, `body`, `left_arm`, `right_arm`,
  `left_leg`, `right_leg`; `head` if left out). The item sits at the bone's matrix times `attach` (offset in pixels,
  rotation in degrees, a uniform scale in (0, 2]).
- `parts`: a tree of parts, each with a pivot, a rotation in degrees and boxes with Minecraft's box
  UV (`ModelPart.Cube`'s faces and order), grown by `inflate`, mirrored in x on request. Names are
  unique and never start with `cosmetic`.
- A part's **`quads`** (optional) are faces given corner by corner, `{"vertices": [[x, y, z, u, v] ×4]}`
  in model pixels and UV in 0..1, in vanilla's corner order: how a Blockbench model with rotated cubes
  and per-face UVs is carried. `examples/shop_items.rs` (`bbmodel_quads`) converts a `.bbmodel`
  (Daylight, `crates/wisp-shop/assets/daylight.bbmodel`).
- A **top-level part's `bone`** (optional) makes it ride another bone than the item's, under the
  same attachment: a shirt's sleeves (`"bone": "right_arm"`) follow the arms as they swing. Only
  top-level parts take one; the reach limit counts from each part's own bone.
- `animations`: loops on top of a part's rest pose, a sine of `period` ticks (4 to 1200) started
  `phase` periods in. `swing` turns the part `amplitude` degrees (at most 45) about the axis, `bob`
  moves it `amplitude` pixels (at most 4), `spin` turns it all the way round once per period.

A **name tag style** (`"slot": "name_tag"`) has no model: its texture is a square mark of at most
16×16 drawn before the player's name, and `"tag": {"line": "#rrggbb"}` the colour of a line under
it. `parts` stays empty and there are no animations; no other slot has a `tag`.

```json
{ "format": 1, "id": "example_tag", "slot": "name_tag", "texture": { "png": "<base64 8×8 PNG>" },
  "parts": [], "tag": { "line": "#a98fff" } }
```

A **trail** (`"slot": "trail"`) has no model and no texture: particles are left behind the wearer
while they move (more than 0.03 blocks a tick; not across a teleport of 4 blocks or more). `trail`
holds its `style` and 1 to 4 `colours`, head (where the wearer is) to tail:

- `stardust`: soft dust drifting up from around the feet, each mote one of the colours fading to
  the last, more of it for faster steps, and now and then a small white sparkle.
- `comet`: an unbroken line of large dust along the path, the first colour fading to the last.

```json
{ "format": 1, "id": "example_trail", "slot": "trail",
  "trail": { "style": "stardust", "colours": ["#f6eeff", "#c7a3ff", "#8a4dff"] } }
```

The game emits trails, auras and sparkles once a tick (`game/cosmetics.rs` `tick_cosmetic_trails`), for the player and
for the players in view when other players' cosmetics are shown and *Others' auras and trails* is on, as particles drawn at full
brightness (they glow at night; `ParticleEngine::add_glowing`): the
Particles setting thins them (Decreased) or hides them (Minimal), and none appear more than 32
blocks from the camera.

An **aura** (`"slot": "aura"`) is the same kind of item, but its particles surround the wearer
all the time, standing still too. `aura` holds its `style` and 1 to 4 `colours`:

- `storm`: short zigzag bolts of small motes beside the body, mostly the first colour, plus loose
  sparks close to it.
- `orbit`: three stars circling the waist (one per colour), rising and falling a little; their
  dust draws the ring.
- `nebula`: three strands of stardust spiralling from the feet to above the head, running through
  the colours as they climb, in a faint mist of the last colour.

```json
{ "format": 1, "id": "example_aura", "slot": "aura",
  "aura": { "style": "storm", "colours": ["#ffffff", "#9fe8ff", "#5b8cff"] } }
```

`"sparkles": ["#rrggbb", ...]` (a `shoulder` item only, 1 to 4 colours, optional): glowing
motes shed around the pet now and then, at its attachment beside the wearer's shoulder.

An **animated texture**: `"texture": {"png": ..., "frames": 24, "frametime": 2}`, the PNG a
vertical strip of `frames` equal frames, each shown `frametime` game ticks, looping. The game puts
the frame that is due into the item's atlas slot as the animation runs (`game/cosmetics.rs`
`store_cosmetic`); pictures (the Shop's cards, the Wardrobe) show the first frame. A frame larger
than 64×64 makes an HD item: the Shop's capes are 4× Minecraft's cape (a 40×64×4 box at a quarter
scale, 88×68 texels), the Founders Cape at its own 256×128 artwork's resolution.

`"glow": true` (any model item, optional) draws the item at full brightness whatever the light
where its wearer stands: a star pet stays bright at night and underground.

Limits, checked when an item is read (`Item::parse`); an item past one is refused, never drawn:

| Limit | Value | Why |
|---|---|---|
| Vertices | 1024 (42 boxes) | the player's own model and outer layer are 288 |
| Texture | 256×256 at most a frame, 1 to 64 frames of 1 to 100 ticks | up to 64×64 takes a skin slot of the atlas, larger one of its 16 HD cosmetic slots (256×256) |
| Parts | 16 | |
| Reach | 24 pixels from the bone's pivot, animations included | items stay near their wearer |

Unknown fields are refused, so a file never carries anything the game would silently ignore.

## The cosmetics server (`crates/wisp-cosmetics-server`)

Wisp's own service for the protocol below, with Mojang-verified sign-in (`README.md` there:
running, deploying, pointing the launcher at it). The wearer's game publishes what it wears
(`PUT /v1/wear`, the launcher Shop's item files, checked by `Item::parse` on the server) and the
emote it plays (`POST`/`DELETE /v1/emote`); other games read `POST /v1/worn` and `POST
/v1/emotes` for the players in view (`wisp_cosmetics::share`). The game plays other players'
emotes at their place (`game/emotes.rs` `tick_remote_emotes`). `emotesShared` in
`wisp-extras.txt` (launcher: Settings › Cosmetics & emotes) stops sharing your own. With the
Shop's folder and a service both set, the game reads its own player from the folder and everyone
else from the service (`wisp_cosmetics::Mixed`).

## What other players wear (`wisp_cosmetics::Cosmetics`)

The game asks Wisp's cosmetics service (HTTPS, JSON) what the players in view wear, and draws
from its answers. The frame never waits on the network: it reads a cache, and a thread of its own
does the asking.

- **One request for the players in view**: `POST /v1/worn` with up to 64 UUIDs
  (`{"players": ["<uuid hex>", …]}`) answers `{"ttl": seconds, "players": {"<uuid hex>": [{"id",
  "hash"}, …]}}`; a player who wears nothing is left out. Only the UUIDs of players the game draws
  are sent, nothing else about them or the server.
- **Cached for the service's TTL** (kept within 1 minute to 1 hour). Past it the last answer stays
  drawn while the player is asked for again. The 4096 players seen most recently are remembered.
- **Item files** come from `GET /v1/items/<id>?hash=<hash>` once and are kept on disk under their
  hash (they never change for one); a file past the limits is left out and not asked for again.
- **Offline**: a request that fails leaves every answer as it was, and the next one waits 15 s, then
  30 s, doubling up to 10 minutes. A player whose item didn't arrive is asked for again after 15 s.
- **Across restarts**: the last answers are kept beside the item files (`worn.json`: each player's item
  ids and hashes, written only when an answer changes them). They are read only when the service doesn't
  answer, so a game started while it is down still shows what players last wore, until it answers.

## In the game

- **The flag**: `wisp_cosmetics::ON_BY_DEFAULT` is `true`. `WISP_COSMETICS=0` turns
  everything off for one run (no thread, no request, no setting, no drawing), `=1` on;
  `WISP_COSMETICS_API=<url>` points the game at another service, and
  `WISP_COSMETICS_LOCAL=<folder>` reads items from a folder instead: every player and every
  mannequin wears its first item of each slot (the art and the bench, without the network);
  `WISP_COSMETICS_LOCAL_PLAYER=<uuid>` limits that to one player. The launcher's Shop
  (`launcher/src/shop.rs`) starts the game this way with what its signed-in player equipped.
- **Settings** (Extras, under "Cosmetics", shown only with the flag on), the same three as the
  launcher's Wardrobe (`wisp_cosmetics::prefs`, lines of `wisp-extras.txt`):
  - *My cosmetics: Shown / Hidden* is the account's (`PUT /v1/visible`): hidden, every Wisp
    player sees you without them, you included. The game signs in for that one call the way the
    launcher does (`wisp_cosmetics::account`, Mojang's `join` with the player's session) and
    notes what the service confirmed in `cosmetics-visible.txt`; offline it tries again after 15 s,
    doubling to 10 minutes, until the service takes it. An unchanged switch sends nothing.
  - *Other players' cosmetics: Shown → Name tags only → Hidden*. Hidden, the game asks for nothing
    about them; name tags only, their name tag styles show and nothing on their bodies.
  - *Others' auras and trails*, off by default: other players' trails, auras and pet sparkles.
- **Name tag styles** (`game/names.rs`): the mark, 8 text pixels square, before the name and a
  one-pixel line in the style's colour under the name's background, in the name tag's own pass
  (the mark is in the atlas like the other items' textures).
- **Wisp's badge** (`wisp_badge`, the app icon's feather as a 16×16 mark): the launcher writes it
  into every player's worn folder (`shop::WISP_BADGE`); it isn't sold and can't be taken off. The
  game shows it even when the player hid their own or others' cosmetics (others' only while the
  game still asks about them, so not with *Hidden*). It replaced the Star, Crown and Moon badges.
- **Drawing** (`game/cosmetics.rs`): an item's texture takes one of the skin atlas's 64×64 slots
  and its boxes are posed on the wearer's bone each frame, through the same GPU skin path as the
  player model (the CPU path where that is off), lit and culled with the player. A helmet hides a
  head item, an elytra a back item; a back item (the Shop's capes) takes the place of the
  player's Mojang cape. The third person view and the inventory's player preview show your own.
- **Cost**: a player with nothing to wear adds one cache lookup; with no cosmetic in view the frame
  does no cosmetics work beyond that. The bench scene `stress_avatars` (300 mannequins, 20×15)
  measures the cost with `WISP_COSMETICS=1 WISP_COSMETICS_LOCAL=…` against the same run off.

## A still picture (`wisp_cosmetics::preview`)

The launcher shows items on the player's own skin without the game: `preview::render` draws the
player model at rest (vanilla's `PlayerModel` boxes on a 64×64 skin, wide or slim arms, the outer
layers included) and each item on its bone on the CPU, with a depth buffer and cutout alpha, seen
from any angle.

## The Wardrobe's account check (`launcher/src/wardrobe.rs`)

The launcher talks to the same service for the player's own items: the catalogue, what the account
owns, its purchase history, what each slot wears and whether other players see it. Buying and
claiming the founder set happen on the website's shop (https://github.com/Lyvt-Dev/Wisp-Client/blob/main/docs/cosmetics.md), never in the
launcher.

- **Who the player is**: the service hands out a one-time server id (`POST /v1/auth/start`), the
  launcher tells Mojang's session server that the account joins it
  (`account::session::join_server`, the call a client makes before joining an online-mode server),
  and the service asks Mojang (`hasJoined`) before it answers `POST /v1/auth/finish` with a
  session of its own. The Minecraft token goes to Mojang only; the service never sees it.
- **Then**: `GET /v1/me`, `PUT /v1/equipped {"slot", "item"}` and
  `PUT /v1/visible {"visible"}` ("Show my cosmetics": off, `/v1/worn` leaves the player out) with
  that session (`Authorization: Bearer`), kept in memory for the time the service gave, less a
  minute.
- **The founder set** (Founder Visor, Founder Tee, Founder Backpack, Founder Tag) is free and can
  be claimed on the website only between the window's start and end; after that it is retired for
  good and never offered again. Players who claimed it keep it, and it shows in the Wardrobe.
- **Launcher themes** are never claimed or sold: they are free for everyone in Settings ›
  Appearance (*Launcher theme*: Wisp or First Light, `wisp_launcher::themes::THEMES`). A
  catalogue entry with `"slot": "theme"` is ignored.
- **Offline**: the last catalogue and answer, and the changes not sent yet, are kept per account
  in the launcher's data folder (`wardrobe/<uuid>.json`; never a session or token), item files
  under their hash. A change shows at once; the ones the service couldn't take (offline, or a
  failure on its side) are sent in order when it answers again, and a newer change of the same
  slot replaces a waiting one.

## The Wardrobe page (`launcher/src/wardrobe_page.rs`)

A top-bar page between Servers and Versions, for the selected Microsoft account (signed out, it asks
you to sign in). It shows only when the service has a founder set to show (or with
`WISP_COSMETICS=1`, for development), whatever cosmetics in the game do (`wisp_launcher::shop`): the launcher asks `GET /v1/shop` at start and every
half hour, with no session and nothing about the player. The page shows when the set can be claimed
now (`open`), when the public window has dates, or when the selected account's own answer, kept from
its last visit, does (a claim, or a preview the service grants that account). With no window set, the
page stays hidden.

- **A row per slot** (head, back, shirt, shoulder pet, name tag) with what it wears,
  "Nothing equipped" or "None owned yet"; **your player** in the middle, on your own skin, wearing
  it, with your name tag style over the head (`wisp_cosmetics::preview`); **what you own for the
  chosen slot**, "Nothing" first. Equipping shows at once.
- **The shop is on the website**: an *Open the shop* button by the page's title opens
  `https://github.com/Lyvt-Dev/Wisp-Client/blob/main/docs/cosmetics.md` in the browser (browsing, prices, the founder claim, purchases through
  Paddle). An account that owns nothing yet sees "You don't own any cosmetics yet." with that button
  instead. **Your purchases** lists the account's orders (date, item, price, state, order number).
- **Three switches**: *Show my cosmetics* (the account's, on the service: off, everyone sees you
  without them), *Hide other players' cosmetics* and *Show other players' auras and trails* (off by
  default). The last two are lines of the game's `wisp-extras.txt` (`wisp_cosmetics::prefs`:
  `cosmeticsOthers` = `shown`, `name_tags` or `hidden`, `cosmeticsAuras`), the same ones the
  game's settings change.
- **States**: offline, the page keeps working from what's kept on disk and says how many changes
  wait to sync; a service failure says what you wear still shows in the game, with *Try again* and
  *Copy details* (host, status, time).
- `WISP_LAUNCHER_PAGE=wardrobe` in a development build shows the page for a made-up account
  wearing `WISP_COSMETICS_LOCAL`'s items; `WISP_WARDROBE_STATE=offline|failed|empty`
  shows that state.
