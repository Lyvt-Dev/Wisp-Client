# Voice chat protocol (Simple Voice Chat compatible)

Wisp's voice chat (`crates/wisp-voicechat`) talks to servers that run the Simple Voice Chat
mod or plugin, so Wisp players can talk with Java players who have it. It is a clean-room
implementation for interoperability: it was written from this description of the wire protocol,
and contains no code, assets or translations of the original mod (which is "All Rights
Reserved"). Compatibility versions: **20** (every released Simple Voice Chat 2.6.x,
on Paper, Fabric and the rest) and **21** (the 2.7 prereleases). The tables below are version 21;
see "Version 20" for what differs. The client asks for 20 first and, with no answer after 5 s,
for 21; it tells the two apart by the secret's layout.

Last checked on 2026-11-14 against the official 2.7.0-pre1+26.3 sources (branch `26.3`, commit
`137d62f`, `voicechat_compatibility_version=21`, `mod_compatible_version=2.6.x`). Every layout
below matched. The server only answers a `request_secret` whose version equals its own, so a
different version is simply "no voice chat on this server", never a half-working connection.

Integers are big-endian. `varint` is Minecraft's VarInt. `uuid` is two big-endian i64 (most
significant first). `bytes` is a varint length then the bytes. `utf(n)` is a varint byte length
then UTF-8, at most `n` characters. `bool` is one byte, 0 or 1.

## Plugin channels (Minecraft custom payloads, play state)

The client announces the channels in `minecraft:register` (NUL-separated ids) and then sends
`voicechat:request_secret`. The server only sends the voice packets below to a client whose
request carried a compatible version.

| Channel | Direction | Body |
| --- | --- | --- |
| `voicechat:request_secret` | to server | i32 compatibility version |
| `voicechat:secret` | to client | 16 raw bytes secret, uuid connection id, i32 UDP port, uuid player, u8 codec (0 VOIP, 1 AUDIO, 2 RESTRICTED_LOWDELAY), i32 MTU (512..2048), f64 voice distance, i32 keep-alive ms, bool groups enabled, utf(32767) voice host, bool recording allowed |
| `voicechat:client_state` | to server | bool muted, bool disabled |
| `voicechat:state` | to client | one player state |
| `voicechat:states` | to client | i32 count, then that many player states |
| `voicechat:remove_state` | to client | uuid player (they left) |
| `voicechat:joined_group` | to client | bool has group, [uuid group], bool wrong password |

Not implemented yet (groups and volume categories), read from the same sources:

| Channel | Direction | Body |
| --- | --- | --- |
| `voicechat:create_group` | to server | utf(24) name, bool has password, [utf(256) password], i16 type (0 normal, 1 open, 2 isolated) |
| `voicechat:set_group` | to server | uuid group, bool has password, [utf(256) password] |
| `voicechat:leave_group` | to server | empty |
| `voicechat:add_group` | to client | uuid id, utf(24) name, bool has password, bool persistent, bool hidden, i16 type |
| `voicechat:remove_group` | to client | uuid group |
| `voicechat:add_category` | to client | utf(16) id, utf(16) name, opt utf(256) name key, opt utf(256) description, opt utf(256) description key, bool has icon, [16×16 i32 ARGB], opt utf(16) parent (`opt` = bool then the value) |
| `voicechat:remove_category` | to client | utf(16) id |

Player state: u8 flags (bit 0 muted, 1 force muted, 2 disabled, 3 force disabled, 4
disconnected, 5 has group), uuid player, utf(256) name, [uuid group if bit 5].

An empty voice host means "the Minecraft server's host" (the official client uses the IP the
Minecraft connection resolved to). A host with a port (`host:port`) or a bare port overrides the
UDP address.

The official client sends `request_secret` as soon as the play state starts (Fabric's early
`JOIN` event), and again after the integrated server's port changes.

## UDP

To server: `0xFF`, uuid connection id, bytes(encrypted). To client: `0xFF`, bytes(encrypted).

Encrypted = 12-byte random IV, then AES-128-GCM ciphertext with its 16-byte tag appended. The key
is the 16-byte secret; there is no associated data. The plaintext is one type byte then the body:

| Id | Packet | Body |
| --- | --- | --- |
| 1 | Mic (to server) | bytes opus (≤1275), i64 sequence, bool whispering |
| 2 | Player sound | uuid channel, uuid sender, bytes opus, i64 sequence, f32 distance, u8 flags (1 whisper, 2 category), [utf(16) category] |
| 3 | Group sound | uuid channel, uuid sender, bytes opus, i64 sequence, u8 flags, [utf(16) category] |
| 4 | Location sound | uuid channel, uuid sender, f64 x, f64 y, f64 z, bytes opus, i64 sequence, f32 distance, u8 flags, [utf(16) category] |
| 5 | Authenticate | empty |
| 6 | Authenticate ack | 16 raw bytes nonce |
| 7 | Ping | uuid id, i64 timestamp |
| 8 | Keep-alive | empty |
| 9 | Connection check | 16 raw bytes nonce (the ack's) |
| 10 | Connection check ack | empty |

Flow: send Authenticate every second until the ack arrives; then send Connection check with the
ack's nonce every second until its ack arrives: the connection is up. Echo every Keep-alive and
every Ping back unchanged. With no keep-alive for 10 × the keep-alive interval the connection is
lost; request a new secret after 10 s.

## Audio

Muted sends nothing, in push-to-talk and voice activation alike, and whispering is off while
muted (the original's `PTTMicrophoneProcessor`/`VoiceMicrophoneProcessor`).

Opus, 48 kHz mono, 20 ms frames (960 samples), the application from the codec byte; the
original caps an encoded frame at the secret's MTU. A frame with
empty opus data ends a stretch of speech (decoder reset). Sequence numbers count up per sender
from 0; gaps up to a few frames are concealed by the decoder. Received sound is keyed by the
channel uuid. Player sound is placed at the sender's position, dropped past `distance + 1` and
faded linearly to 0 at `distance`; group sound is not placed.

## Version 20 (released 2.6.x)

Checked on 2026-11-14 against branch `2.6.x/26.3` (`mod_version=2.6.28+26.3`,
`voicechat_compatibility_version=20`). Everything not listed matches version 21.

- `voicechat:secret` has no connection id: 16 raw bytes secret, i32 UDP port, uuid player, then
  the rest as above. UDP packets to the server carry the **player** uuid where 21 has the
  connection id.
- Mute and disable go on `voicechat:update_state` with only bool disabled (no
  `voicechat:client_state`). Muted simply sends nothing.
- Player state: bool disabled, bool disconnected, uuid player, utf(32767) name, bool has group,
  [uuid group].
- Authenticate (5) carries uuid player then the 16 raw bytes secret; Authenticate ack (6) and
  Connection check (9) are empty.
