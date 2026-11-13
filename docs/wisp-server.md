# The Wisp server: friends and shared cosmetics

One small server does everything Wisp players share with each other:

- **Friends** in the launcher: add people by their Minecraft name, accept requests, see who is
  online, what they play and on which server, and join them with one click.
- **Cosmetics and emotes**: everyone sees what other Wisp players wear and play, on any
  Minecraft server.

It is one program with one data file. Players sign in to it the way an online-mode Minecraft
server checks them (Mojang's `hasJoined`), so it never sees anyone's password or Minecraft token.

Pick **one** of the three ways below. Then do [Point Wisp at it](#point-wisp-at-it).

| | Takes | Address | Runs when |
|---|---|---|---|
| [A. On your PC](#a-on-your-pc-one-double-click) | 1 double-click | changes each start | your PC is on |
| [B. Railway](#b-railway-always-on-recommended) | about 5 minutes | never changes | always |
| [C. Your own Linux server](#c-your-own-linux-server-one-command) | 1 command | your domain | always |

## A. On your PC (one double-click)

Good for trying it out with friends tonight.

1. Double-click **`crates/wisp-cosmetics-server/Start-Wisp-Server.cmd`**.
2. Wait. The first time it installs Rust and builds the server (a few minutes) and installs
   `cloudflared` for the public address; Windows may ask once to allow each install.
3. The window shows **Your Wisp server: https://something.trycloudflare.com**. It is already
   copied to the clipboard and already set in this PC's launcher.
4. Send the address to your friends. Keep the window open while you play; closing it stops the
   server.

The free address changes every time you start it again, so friends would have to paste the new
one. For an address that stays, use B or C. Your data stays on your PC in
`%LOCALAPPDATA%\WispServer`, so friends and cosmetics survive restarts either way.

## B. Railway (always on, recommended)

Railway runs the server for you, day and night, with a fixed HTTPS address.

1. Sign in at [railway.com](https://railway.com) with GitHub.
2. **New Project → Deploy from GitHub repo →** pick this repository.
3. Open the new service → **Settings**:
   - **Config file path**: `crates/wisp-cosmetics-server/railway.json`
4. Right-click the service → **Attach volume** → mount path **`/data`**. (Friends and cosmetics
   are kept here; without it they are lost on every deploy.)
5. **Settings → Networking → Generate Domain**. You get an address like
   `https://wisp-production.up.railway.app`.
6. Open `https://<that address>/healthz` in a browser. It says `ok` when the server runs.

Nothing else to set: the server finds the volume by itself. Every push to the repository
deploys the new version.

## C. Your own Linux server (one command)

On any Linux machine with Docker and a domain pointing at it (an A record):

```sh
git clone <this repository> && cd Wisp-Client
DOMAIN=wisp.example.com docker compose -f crates/wisp-cosmetics-server/docker-compose.yml up -d
```

Caddy gets the HTTPS certificate by itself. Your address is `https://wisp.example.com`. Data is
kept in a Docker volume; update with `git pull` and the same command.

## Point Wisp at it

**Quickest, no new build:** every player opens the launcher → **Settings → General → Wisp
server** and pastes the address. The status line under it says **Connected** when friends work.
The game uses the same address for cosmetics; the launcher hands it over when it starts the game.

**For everyone, built in:** put the address in `launcher/src/shop.rs` (`SERVICE`) and
`crates/wisp-cosmetics/src/lib.rs` (`API`), then release a new launcher. Players don't have to
paste anything; an address set in Settings still wins.

`WISP_COSMETICS_API=<address>` overrides both for one run (testing).

## Using friends

- The **right column** of the launcher is the friends list. Type a Minecraft name under **Add a
  friend** and press the button. They see your request at the top of their list and accept it with
  the tick.
- A friend can only be added once they have opened Wisp with this server at least once (that's
  how the server learns their name).
- Friends show **Online in launcher**, **Playing Wisp** or **Playing on \<server\>**; offline
  friends show when they were last seen. The play button next to a friend on a server joins that
  server. Hover a friend to remove them.
- The launcher says where you are about every 25 seconds. A friend whose launcher closed shows as
  offline within a minute and a half.
- **In the game**, press **Right Shift** and open **Friends** at the bottom of the menu's sidebar:
  the same list, with Accept/Decline, Add (just start typing a name), Remove (click twice) and
  **Join**, which leaves your world and joins the server your friend is on. The game tells
  friends the server you are really on, even after you switch servers. A small notice slides in
  at the top right when a friend comes online or sends you a request.

## Settings (optional)

| Variable | Default | |
|---|---|---|
| `PORT` | `8080` | the port it listens on |
| `WISP_COSMETICS_DB` | `/data/cosmetics.sqlite3` with a `/data` volume, else `cosmetics.sqlite3` | the one data file |
| `WISP_COSMETICS_ITEMS` | any valid item | comma-separated item ids players may wear, e.g. the Shop's |

## If something doesn't work

- **Settings says "Can't reach the Wisp server"**: open `<address>/healthz` in a browser. No
  `ok` means the server isn't running or the address is wrong (no trailing slash needed).
- **"Sign in with Microsoft to add friends"**: sign in on the launcher's profile page; friends
  belong to your Minecraft account.
- **"That player hasn't used Wisp yet"**: they need to open the launcher once with this server
  address set.
- **On your PC, the window closes at once**: run `Start-Wisp-Server.cmd` again and read the
  message; the server's log is `%LOCALAPPDATA%\WispServer\server.log`.

The protocol is described at the top of `crates/wisp-cosmetics-server/src/lib.rs` (cosmetics)
and `src/friends.rs` (friends).
