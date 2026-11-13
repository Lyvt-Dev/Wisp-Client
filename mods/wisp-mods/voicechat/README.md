# Simple Voice Chat (Wisp port)

Proximity voice chat for Wisp that talks to servers running the Simple Voice Chat mod or
plugin, so you can talk with Java players who have it. Built into the engine
(`crates/wisp-voicechat`); this package only switches it on for an instance.

It is a clean-room, interoperable implementation written from `docs/voicechat-protocol.md`; it
contains no code or assets of the original mod by Max Henkel.

In game: V opens the voice chat menu (activation, push-to-talk key, microphone and speaker,
volumes, threshold, mute). Caps Lock is push-to-talk by default, M mutes, N switches voice chat
off.
