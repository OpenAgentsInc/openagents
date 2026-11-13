# Metaverse Engine transport v1

The bytes a page and its room carry for each frame, on a WebSocket or on
WebTransport. Profile:
[`aethyr-transport-v1.json`](../../proto/verse/aethyr-transport-v1.json).
Codec: `aethyr/net/transport.coax`; the page's link and the room's
`coax/web/room/room-wt.coax` both load it.

On a WebSocket every frame is its own bytes, unchanged, on the socket's
one ordered stream.

On WebTransport (a URL prefixed `wt+`), the first byte of each frame in
the page's pipe names its channel: `1 + CHANNEL`.

| Channel | Number | Delivery |
| --- | --- | --- |
| datagram | 0 | Unreliable, unordered; each datagram at most 1,200 bytes and decodes alone |
| combat | 1 | Reliable, ordered |
| loot | 2 | Reliable, ordered |
| chat | 3 | Reliable, ordered |
| session | 4 | Reliable, ordered; also any channel the codec doesn't know |

A realm picks each frame's channel by its first word (`room-wt.coax`
`RM_WT_ROUTES` for Everglade).

An input goes in a datagram that repeats the inputs the room hasn't
acknowledged, oldest first, `\n` (byte 10) apart: the newest 6 at most,
within 1,200 bytes. An owed input the same as the next one but for its
number (one less) and its last word goes as the single byte `~`; the room
makes it whole from the next frame. The room lets each input number
through once, so a lost datagram costs nothing. Inputs over 96 bytes are
sent but not kept for repeating.
