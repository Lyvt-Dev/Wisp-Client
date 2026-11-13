# wisp-cosmetics-server (the Wisp server)

**Setting it up: [docs/wisp-server.md](../../docs/wisp-server.md)** (one double-click on your
PC, Railway, or one Docker command).

One server for everything players share: **friends** (requests, who is online and where; see
`src/friends.rs`) and **cosmetics and emotes** on any Minecraft server (vanilla, Paper, LAN). Games ask this server what the players in view wear and play, keyed by their
Minecraft UUID; nothing goes through the game protocol. Protocol: `src/lib.rs`.

- **Sign-in** is the check an online-mode server makes: the game tells Mojang it joins a one-time
  server id, and this server asks Mojang (`hasJoined`). The Minecraft token never reaches it.
- **Items** are uploaded by the wearer's game and checked exactly as the game checks them
  (`wisp_cosmetics::Item::parse`: size, vertex and reach limits), then kept under their hash.
- **Emotes** live in memory only: an emote shows to others for as long as it plays (30 s at most).

## Run it

```sh
cargo run --release --manifest-path crates/wisp-cosmetics-server/Cargo.toml
```

| Variable | Default | |
|---|---|---|
| `PORT` | `8080` | |
| `WISP_COSMETICS_DB` | `cosmetics.sqlite3` | one SQLite file: keep it on a volume |
| `WISP_COSMETICS_ITEMS` | (any valid item) | comma-separated item ids players may wear, e.g. the Shop's |

It needs HTTPS in front of it (Railway, Fly.io and most hosts give you that).

## Deploy on Railway

1. New project → Deploy from GitHub repo → this repository.
2. Settings → Config file: `crates/wisp-cosmetics-server/railway.json` (builds the Dockerfile).
3. Add a volume mounted at `/data`.
4. Networking → Generate domain. Check `https://<domain>/healthz` answers `ok`.

Any Docker host works: `docker build -f crates/wisp-cosmetics-server/Dockerfile .` from the
repository root, then run it with a volume at `/data` and port 8080 exposed.

## Point Wisp at it

Put the URL (no trailing slash) in both places, then build a new launcher:

- `launcher/src/shop.rs`: `SERVICE`
- `crates/wisp-cosmetics/src/lib.rs`: `API`

`WISP_COSMETICS_API=<url>` overrides both for one run (testing).

## Limits to know

- The Shop's wallet is local, so the server cannot check that a player bought what they wear;
  `WISP_COSMETICS_ITEMS` limits uploads to known item ids, and every file passes the game's
  checks. A modified client could still upload its own art under a known id.
- A change to what someone wears reaches other games within a minute (the game keeps each
  answer that long); emotes show within about a second.
