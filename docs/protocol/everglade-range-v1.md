# Everglade Archery Range v1

The Archery Range stands on Everglade's land, on the city's clearing east of the
path (`everglade/range/layout.coax`, #13507). Its arrows are Everglade's: the room
flies every one (`everglade/server/arrows.coax`), and an arrow that sticks in one
of the range's seven faces ends as any stuck arrow does, its `d` frame's BODY
the face ([`everglade-projectiles-v1.md`](everglade-projectiles-v1.md)). What is
the range's own is its tally: the room keeps one for each seat
(`everglade/range/score.coax` `RgScore`) and tells the archer alone what each of
its arrows on the range scored. The page keeps its copy by the same rule.

Profile: [`everglade2-range-v1.json`](../../proto/verse/everglade2-range-v1.json).
Codec: `everglade/server/range-wire.coax`, which the room and the page both load.
Check: `coax/web/tool.sh coax/web/room/range-profile.coax proto/verse/everglade2-range-v1.json`.

| Frame | Fields |
| --- | --- |
| `E SEQ 23 0` | page to room: clear your tally. Answered with `e range 0 0 0 0 -1` |
| `e range ID MODE CM POINTS FACE` | an e entry of D, told to the archer alone: arrow ID landed. MODE 0 on foot, 1 from horseback, 2 from dragonback; CM how far from the hand it struck, in centimetres (at most 100000); POINTS 0 a miss, 1 to 10 the ring; FACE the face, -1 none. ID 0: the tally was cleared, the other fields 0 and FACE -1 |

An arrow loosed while its archer stands on the range's ground is counted when it
lands, in a face or not. An arrow in a face is counted wherever it was loosed
from. A frame with a field missing, extra or out of these bounds is refused
whole.

A page from before the range has no reader for `e range` and drops it, and never
says kind 23, so this needs no previous-wire branch
(`everglade/server/wire-prev.coax`).

The tally is the seat's, kept while the seat is held. Keeping it on the
character's record (`everglade/model/character.coax`) needs a new record version
and its migration: a follow-up.
