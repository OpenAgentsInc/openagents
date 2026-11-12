# Everglade II guard and monsters v1

The room tells each page where the path guard and the monsters are, and
each blow, alongside the Everglade session v5 transport. Profile:
[`everglade2-combat-v1.json`](../../proto/verse/everglade2-combat-v1.json).
Codec: `everglade/server/creature-wire.coax`; the room writes these frames
with it and the page reads them with it.

| Frame | Fields |
| --- | --- |
| `g2` | `TICK LIFE STATE X Z YAW HP MAX SWING FOE`: the guard. X and Z in integer millimetres, YAW in thousandths of a radian, SWING the tick of his last blow or -1, FOE a seat or -1 |
| `g3` | `TICK KIND SEAT AMOUNT`: a blow. KIND 1 a seat's at him, 2 his at SEAT, 4 his critical hit, 5 his arrow, 6 its critical hit. AMOUNT 0 is a miss |
| `h2` | `TICK COUNT [M LIFE KIND STATE X Z YAW HP MAX ACT ACT_TICK FOE]...`: the monsters within view. X and Z in integer centimetres, YAW in thousandths of a radian, ACT 1 a blow and 2 a shot, told from ACT_TICK |
| `h3` | `TICK KIND M SEAT AMOUNT [FX FZ TX TZ FLY_MS]`: KIND 1 its blow at SEAT, 2 a blow at it, 3 an arrow loosed (with the five flight fields, centimetres and ms), 4 it fell |

`g2` and `h2` are whole snapshots. The room sends one each tick it changes
and once a second when it doesn't, on the WebTransport datagram channel;
`g3` and `h3` go on the combat stream. A monster an `h2` leaves out is out
of view. An `h2` is at most 12 rows and fits the room's 1,700 bytes.

A page takes a frame whole or not at all: a missing, extra or
out-of-range field drops it, and a snapshot no newer than the last is
ignored.
