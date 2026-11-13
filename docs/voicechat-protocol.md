# Voice chat protocol (Simple Voice Chat compatible)

Wisp's voice chat (`crates/wisp-voicechat`) talks to servers that run the Simple Voice Chat
mod or plugin, so Wisp players can talk with Java players who have it. It is a clean-room
implementation for interoperability: it was written from this description of the wire protocol,
and contains no code, assets or translations of the original mod (which is "All Rights
Reserved"). Compatibility version: **21** (Simple Voice Chat 2.6.x / 2.7.x for Minecraft 26.3).

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
| `voicechat:joined_group` | to client | bool has group, [uuid group], bool wrong password |

Player state: u8 flags (bit 0 muted, 1 force muted, 2 disabled, 3 force disabled, 4
disconnected, 5 has group), uuid player, utf(256) name, [uuid group if bit 5].

An empty voice host means "the Minecraft server's host". A host with a port (`host:port`) or a
bare port overrides the UDP address.

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

Opus, 48 kHz mono, 20 ms frames (960 samples), the application from the codec byte. A frame with
empty opus data ends a stretch of speech (decoder reset). Sequence numbers count up per sender
from 0; gaps up to a few frames are concealed by the decoder. Received sound is keyed by the
channel uuid. Player sound is placed at the sender's position, dropped past `distance + 1` and
faded linearly to 0 at `distance`; group sound is not placed.
