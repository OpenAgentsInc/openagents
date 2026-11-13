# Everglade II guard and monsters v2

The path guard and the monsters are rows of the
[Everglade session v6](everglade-session-v6.md) wire (kinds 14 and 13).
This profile pins what each did, or had done to it: two entries of the `D`
frame. Profile:
[`everglade2-combat-v2.json`](../../proto/verse/everglade2-combat-v2.json).
Codec: `everglade/server/creature-wire.coax`; the room writes these entries
with it and the page reads them with it.

| Entry | Fields |
| --- | --- |
| `e guard` | `TICK KIND SEAT AMOUNT`: a blow. KIND 1 a seat's at him, 2 his at SEAT, 4 his critical hit, 5 his arrow, 6 its critical hit. AMOUNT 0 is a miss |
| `e mon` | `TICK KIND M SEAT AMOUNT [FX FZ TX TZ FLY_MS]`: KIND 1 its blow at SEAT, 2 a blow at it, 3 an arrow loosed (with the five flight fields, centimetres and ms), 4 it fell |

TICK is the room tick the event is for. It may be ahead of the frame's tick:
the guard's turn is told when the rules take it, each blow at its moment.

Both entries are on the reliable event class. The room sends them to the
seats near the creature and to the seat the entry names, outside the cue
budget. On WebTransport they go out of band on the combat stream.

A page takes an entry whole or not at all: a missing, extra or out-of-range
field drops it.

Version 1 (`g2`, `g3`, `h2`, `h3` as frames of their own) ended with session
v5 (#13276).
