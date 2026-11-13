# Room relay v1

A relay room (`/api/room/relay/GAME/CODE`, `coax/web/room/room-relay.coax`)
seats the players of any game and passes each one's text frames to the
others. These are the relay's own frames. Profile:
[`room-relay-v2.json`](../../proto/verse/room-relay-v2.json). Codec:
`metaverse-engine/net/relay-wire.coax`. The room writes the frames with it, and every
page that sits in a relay room reads them with it: Everglade II
(`everglade/client/net.coax`), Everglade (`apps/openagents-web/old/v1/everglade-net.coax`),
the lockstep games (`apps/openagents-web/lockroom.coax`: Belt 2200 and Hold
the Stair) and One Shot (`apps/openagents-web/oneshot.coax`).

Text frames. Fields are one space apart, and numbers are whole and decimal.

| Frame | From | Fields |
| --- | --- | --- |
| `y` | room | `SEAT N T [G]`: you are in, at SEAT, with N others. T is the server's wall clock in ms. In an Everglade room G is the seat's generation, one more each time a socket takes the seat |
| `j` | room | `SEAT`: a player sat at SEAT. A joiner hears one for each player already in, after its `y` |
| `l` | room | `SEAT`: the player at SEAT left |
| `m` | room | `SEAT TEXT`: the player at SEAT sent TEXT, as it was sent. An Everglade room reads its seats' frames and passes none on |
| `t` | page | `MS`: a clock ping. MS is the page's own clock value: up to 15 digits, with an optional minus |
| `t` | room | `MS T`: the pong, to the sender only: the ping as it was sent, then the server's wall clock in ms |

A SEAT is from 0 to 149: a room seats 16 (0 to 15), a zone up to 150 (#13275). Version 1 said 0 to 15; a version 2 reader reads every version 1 frame, so pages moved first. A reader takes a frame whole or not at all: a
missing or extra field, or a SEAT out of range, drops the frame. A page
uses what it needs of N, T and G.

Check:

```sh
coax/web/tool.sh coax/web/room/relay-profile.coax proto/verse/room-relay-v2.json
```

The list of an Everglade realm's rooms (`GET /api/playtest/rooms/SLUG`) is
JSON, not a relay frame. It is not in this profile; the room reports step
(E8) owns it.
