# Max Render 1.0.0

Wisp client module, API 1. Built into the engine; this package only switches it on for an instance.

Without it, render distance stops at 128 chunks. With it, any distance from 2 up to
2,147,483,647 chunks (the largest 32-bit integer) can be set:

- **Options > Video Settings > Render Distance**: the left half of the slider is 2 to 128 chunk
  by chunk as usual, the right half climbs from 128 to 2,147,483,647 in steps of about 14%.
- An exact number: `renderDistance:5000` in `options.txt`, `--render-distance 5000` on the game's
  command line, or `wisp-ctl render-distance 5000`.

Limits that stay:

- The world is 3,750,000 chunks across, so the engine never streams further than that, whatever the
  number says.
- Every chunk takes memory and time to generate and mesh. A distance of 5,000 means about 78 million
  chunks: no computer holds that, so the game loads what it can, and the RAM limit (if set) lowers
  the distance in use. Expect low frame rates and long loading far past 128.
- On a server, chunks only come as far as the server's own view distance (at most 127 in the
  protocol). Far distances are for singleplayer.

Switching the module off in game brings a distance above 128 down to 128. Starting the game
without it, a saved distance above 128 isn't accepted and the game's default (12) is used.

Build this folder with `cargo wisp-mod build` and import `target/wisp-mod/max_render.amod` in the
launcher's **Mods > Import .amod**.
