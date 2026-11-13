# Handover: where things stand (2026-11-06)

**0.10.9** (trails drawn in the game, the Cosmic Fox pet, the owl redrawn, glowing pets) is the
current work; 0.10.8 (audio retry, GUI `tile` scaling, corrected pack support list) is released.
See the 0.10.9 section of [Agent.md](Agent.md).
Read [Agent.md](Agent.md) first for the project rules; this page is only the current state.

## Released

| Version | What | State |
|---|---|---|
| **0.10.0** | Fast resource-pack loading (indexed overlay), native static Nuit skies, metadata-only pack warning | Published: tag `launcher-v0.10.0` at `84fb854` |
| **0.10.5** | Shop page with Wisp Coins (1,000 welcome + 100/day, never sold), Founders Cape (owner's artwork), Nebula/Aurora capes, 3 badges, Nova the Star Owl, 2 trails | Pushed to `codex/wisp-rebrand` at `1fc5bc3`; the owner installed 0.10.5 |

Releases publish when `codex/wisp-rebrand` (or `main`) is fast-forwarded: the `launcher.yml`
workflow builds the installer and creates `launcher-v<version>`. Work happens on
`codex/client-features-installer` (local branch `codex/resource-pack-loading`).

## 0.10.6 (released; details below)

### 1. Shop capes didn't show in game: fixed and verified

- **Cause:** the owner's Minecraft account has a Mojang cape. `covered()` in
  `crates/wisp-client/src/game/cosmetics.rs` hid every Wisp back item when the player had a
  Mojang cape, so the bought Founders Cape never drew.
- **Fix:** an Wisp back item now replaces the Mojang cape (`Game::cosmetic_on_back`;
  `player_cape_geometry` in `game/perspective.rs` takes an `own` flag and skips the Mojang cape).
  An elytra still hides the back item. `docs/cosmetics.md` updated.
- **Verified:** offscreen runs in `.local/tooling/cape-qa-0.10.6/` (`run-cape-qa.ps1`,
  `cape_ctl.py`) with a red stand-in Mojang cape (`WISP_OFFLINE_CAPE`) and the Founders Cape
  equipped. The installed 0.10.5 shows the red cape (`before.png`); the fixed build shows the
  Founders Cape (`after.png`). Game Clippy (`-D warnings`) passed.

### 2. New logo everywhere: done, checked in the launcher

- Master: `docs/media/brand/wisp-logo.png` (the owner's logo, 1254×1254).
- `packaging/windows/generate-icons.ps1` now cuts every icon from that PNG (rounded 22 % tile)
  and has been run. Regenerated: `crates/wisp-theme/icons/wisp-256.png` (launcher sidebar),
  `wisp-64.png` (game and launcher window icon, HDR screen), `packaging/icons/wisp.ico`
  (game/launcher exe icon and installer `SetupIconFile`), hicolor PNGs and scalable SVG,
  `wisp.icns`, macOS installer backgrounds, `launcher/assets/icons/wisp.svg`.
- Not updated: the SVG masters and generated files used by `cargo run -p xtask -- icons`
  (`docs/media/brand/*.svg`, `icon-1024.png`, web favicons, GitHub avatar) and the brand
  README text, which still describe the old mark.

### 3. Version bump

- `launcher/Cargo.toml` and `launcher/Cargo.lock`: 0.10.6.
- `packaging/windows/releases/0.10.6.md` written (cape fix). **Add a line about the new logo.**

## To do next

- Confirm `launcher-v0.10.6` appears on GitHub (the workflow run can't be watched from this
  machine without `gh` signed in).
- Redraw the SVG brand masters (`docs/media/brand/*.svg`) as the new logo, then run
  `cargo run -p xtask -- icons` for the web favicons and avatars.
- Tell the owner: their cape shows in third person (F5) and the inventory preview, never in
  first person.

## Known gaps (not started)

- **Resource packs:** core shaders run only for the GUI text and the block outline; see the
  gaps table in `docs/resource-packs.md`.
- **Gengar font boxes:** not reproducible; the owner's `.wisp-cache` rename is pending (the
  game was open).

- **Trails** can be bought and equipped, but the game has no trail renderer (Shop cards say
  "In game soon").
- **No shared cosmetics service:** Shop items live in the launcher's `shop.json` and only the
  wearer sees them; the Founders Cape's "first 50 players" limit isn't enforced.
- **Brand masters:** the old SVG marks and their xtask outputs (see 2 above).

## Promo video (outside the repo)

- Project: `.local/promo-video/` (Remotion; `npx remotion render src/index.ts Promo|PromoVertical`).
- Outputs: `dist/Wisp-Promo.mp4` (16:9) and `dist/Wisp-Promo-TikTok.mp4` (9:16), 30 s at
  1080p60, with the owner's ElevenLabs narration (free plan: credit "Voice: ElevenLabs").
- The video still shows the 0.10.5 launcher screenshots; no FPS numbers are claimed on purpose.

## Owner's resource pack

`Girl_Sky_26.3_Metadata_Updated` in their instance has only `pack.mcmeta` and `pack.png`, so it
can't change anything. The full repaired pack is `dist/Girl_Sky_26.3_Wisp.zip`; the owner is
copying it into their `resourcepacks` folder themself.
