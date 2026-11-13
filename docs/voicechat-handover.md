# Handover: Simple Voice Chat port for Wisp

(The project's general state is in [HANDOVER.md](../HANDOVER.md).)

Status as of 2026-11-10. Work stopped mid-integration at the owner's request. The code is
committed with 0.10.13 and compiles, but voice chat stays off until the launcher installs its
module ("Next steps" 2) and it has been tested against a real server (3). Last released version: 0.10.12, commit `2ac6a49` (calm Black Hole +
font crash fix), pushed to `codex/resource-pack-loading` and `codex/wisp-rebrand`.

## Goal

A built-in proximity voice chat that talks to servers running the Simple Voice Chat mod or plugin,
so Wisp players can talk with Java players who have it. Owner decisions:
- A port inside Wisp, not Java instances.
- First version: proximity voice only. Groups come later.
- A full settings menu like the original's: push-to-talk or voice activation, microphone and
  speaker selection, volumes and so on.

**License note:** Simple Voice Chat is "All Rights Reserved". This is a clean-room
reimplementation for interoperability. It was written from a protocol description
(`docs/voicechat-protocol.md`), and no code or assets of the original are copied. That is
generally allowed but a legal grey area; the owner was told and chose to go ahead.

## Done

### 1. Crate `crates/wisp-voicechat` (complete, 12 tests pass, clippy clean)
- `protocol.rs`: Minecraft buffer encodings, plugin channel payloads (`request_secret`,
  `secret`, `client_state`, `state`/`states`), all 10 UDP packet types, and AES-128-GCM with the
  IV first. Compatibility version 21 (Simple Voice Chat 2.6/2.7 for 26.3).
- `connection.rs`: a UDP thread. It resolves the address, following SRV redirects through a
  resolver passed in, then sends Authenticate every second until the ack arrives, then the
  connection check, echoes keep-alives and pings, and detects a lost connection after 10 times
  the keep-alive interval. Tested against a stand-in server on localhost.
- `audio.rs`:
  - Microphone (cpal input): resampling to 48 kHz, Opus mono 20 ms frames, push-to-talk or
    voice activation with hold times, whisper, gain, a level meter, and an empty frame at the
    end of speech.
  - Speaker (cpal output, its own stream): one jitter buffer per sender, Opus decoding with
    concealment of lost frames, 3D panning, linear fade with distance, per-player volume, and a
    "who is talking" list.
- `lib.rs`: the `VoiceChat` facade the game uses: `joined`, `payload`, `take_outgoing`, `tick`,
  `apply(Settings)`, `state`, `talking`, push-to-talk and whisper, the mic test.
- New dependencies: `aes-gcm 0.10`, `getrandom 0.2`, `opus 0.3` (libopus built from source with
  CMake) and `cpal 0.16`.
- New `.cargo/config.toml` sets `CMAKE_POLICY_VERSION_MINIMUM=3.5`. Without it, libopus's old
  CMakeLists doesn't configure with CMake 4. CI needs CMake too; GitHub runners have it.

### 2. Client module (`crates/wisp-mods/src/amod.rs`)
- Besides `zoom`/`camera-zoom`, module `voicechat` with capability `voice-chat` is now allowed.
- `VOICECHAT_SOURCE` points to the new manifest in `mods/wisp-mods/voicechat/`, which also has
  a README.
- The `[zoom]` section is now optional (`#[serde(default)]`). `is_zoom()` and `is_voicechat()`
  were added, and the test is extended.

### 3. Game wiring (written, not compiling yet)
- `crates/wisp-client/src/game/voicechat.rs` is new:
  - **Keys** (rebindable in Options › Mods): push-to-talk = Caps Lock, whisper = unbound,
    mute = M, voice chat off = N, menu = V.
  - **V opens the voice chat page** in Options › Mods. It shows the status (connected and the
    range), activation mode, threshold, mic amplification, volume, microphone and speaker
    (clicking cycles through the devices), mic test with level, muted, voice chat off, and a mic
    error if there is one.
  - **Settings** are saved to `wisp-mods/.wisp-mod-data/voicechat/settings.json`.
  - **HUD:** mic status at the bottom left (Talking / Muted / Voice off / No microphone /
    Connecting / Disconnected) and the names of the players currently talking. Whispering is
    shown in violet.
  - **Joining:** when the player has joined a server (`remote.logged_in`), it sends
    `minecraft:register` with the voicechat channels, then `voicechat:request_secret`.
- Hooks in existing files:
  - `game.rs`: `mod voicechat`, the fields `voicechat` and `voicechat_module`, the HUD draw on
    Playing/Chat, and `tick_voicechat()` in the tick.
  - `remote.rs`: a `voicechat_payload` arm before the other custom payloads, and
    `tick_voicechat()` in the tick.
  - `native_hooks.rs`: `voicechat_key` in `native_key`, the row in `native_mods`, and the
    settings page and its clicks.
  - `options.rs`: closing the options screen ends the mic test.
  - `amods.rs`: the voicechat module is kept out of the zoom list and sets `voicechat_module`.
  - `Cargo.toml` (wisp-client): adds the `wisp-voicechat` dependency.

## Next steps

1. ~~Fix the 3 compile errors~~: done on 2026-11-10 (`server_redirect`, `pub(super)` on
   `Remote.players` / `level_players`). The game compiles with voice chat in it, inactive until
   the `voicechat` module is installed; this shipped in 0.10.13.
2. **Launcher:** in `launcher/src/mods.rs`, add `install_voicechat` next to `install_zoom` (writes
   `voicechat.amod` from `amod::VOICECHAT_SOURCE`), and a card in the mods catalog
   (`dashboard/mods_catalog.rs`) with a toggle. The zoom options (`change_option`, `config`) must
   only apply to `is_zoom()` modules. Check that the launcher's mod list doesn't show zoom
   settings for the voice chat module.
3. **Interop test against the real mod** (the only way to be sure of the protocol), in an
   isolated folder under `.local/tooling/` and never committed: a Fabric 26.3 server with Fabric
   API and Simple Voice Chat from Modrinth, started with the launcher's Java runtime
   (`%LOCALAPPDATA%\wisp-local-launcher\minecraft\runtime`). Then:
   - join with the QA harness;
   - check in the log that `connected` appears;
   - ideally talk to a real Java client with the mod.

   Not yet verified:
   - when exactly the original client sends `request_secret`. We send it after the Login packet,
     plus `minecraft:register` (needed on Paper/Bukkit);
   - whether a Fabric server needs the channels announced during the configuration phase;
   - Opus details of the original (channels/bitrate; we use mono, VOIP, default bitrate).
4. Note in `Agent.md` (owner decisions) and `THIRD_PARTY_NOTICES.md`:
   - the clean-room port;
   - libopus (BSD), aes-gcm and cpal;
   - CMake as a build requirement.
5. Release: bump `launcher/Cargo.toml`, add `packaging/windows/releases/<version>.md`, commit,
   build the installer (`packaging/windows/build-installer.ps1`, previously through the
   scratchpad's `build-installer-12.cmd`), push to both branches.

## Later (not in v1)
- Groups (create, join, leave, password, group HUD).
- Speaking icon above the head (`native_render_name_tag`).
- A per-player volume menu.
- RNNoise denoiser and automatic gain control.
- A real slider UI instead of buttons that cycle through values.

## Uncommitted files
`Cargo.toml`, `Cargo.lock`, `.cargo/config.toml`, `crates/wisp-voicechat/`,
`docs/voicechat-protocol.md`, `mods/wisp-mods/voicechat/`, `crates/wisp-mods/src/amod.rs`,
`crates/wisp-client/Cargo.toml`, and `crates/wisp-client/src/game/{voicechat.rs, game.rs,
amods.rs, native_hooks.rs, options.rs, remote.rs}`. `referenzen/` is the owner's own folder, as
before.
