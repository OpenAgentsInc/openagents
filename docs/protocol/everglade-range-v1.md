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
| `E SEQ 23 0` | page to room: clear your tally. Answered with `e range 0 0 0 0 -1`. Standing on the range's ground (`rg_lends`), you are also taken back to the line, off any mount (#13519) |
| `E SEQ 24 MODE` | page to room (#13519): shoot MODE from now. 0: get down, by riding's rule (a flier in the air lands first). 1 or 2: off whatever else you ride and onto the range's horse or dragon, stepped up to its side from anywhere on the range. Answered as riding answers, `e ride SEQ CODE MOUNT` ([riding v1](everglade-riding-v1.md)); off the range CODE is 2 (held) |
| `e tally BEST S H P G ...` | an e entry of D, told to the archer alone (#13519): your lasting tally, from your character's record ([character v2](everglade-character-v2.md)), when your seat takes it: the longest run of hits, then for each way of shooting and each band of distance (to 15, 25, 40, 60 m, past 60; `score.coax` order, way major) its shots, hits, points and golds. 61 numbers, each 0 to 9,999,999; a cell's hits at most its shots, its golds at most its hits, its points one to ten a hit, the run at most every hit |
| `e range ID MODE CM POINTS FACE` | an e entry of D, told to the archer alone: arrow ID landed. MODE 0 on foot, 1 from horseback, 2 from dragonback; CM how far from the hand it struck, in centimetres (at most 100000); POINTS 0 a miss, 1 to 10 the ring; FACE the face, -1 none. ID 0: the tally was cleared, the other fields 0 and FACE -1 |

An arrow loosed while its archer stands on the range's ground is counted when it
lands, in a face or not. An arrow in a face is counted wherever it was loosed
from. A frame with a field missing, extra or out of these bounds is refused
whole.

A page from before the range has no reader for `e range` and drops it, and never
says kind 23, so this needs no previous-wire branch
(`everglade/server/wire-prev.coax`).

The tally lasts on the character's record (#13519). Each arrow the room scores
is a claim on the record through the item ledger (`shot` and `run`,
[item operation v1](everglade-item-op-v1.md)), and RESET's clear is one too. When
a seat sits, is placed and has its snapshot, it takes the record's tally (with
any arrows it shot before the record was read) and the page hears it whole as
`e tally`. The run now and the last arrow are the seat's only.

## The Longbow it lends (#13519)

Anyone standing on the range's ground, or 2 m round it (`layout.coax`
`rg_lends`), holds the range's Longbow as their ranged weapon: the room sets it
in the rules each tick (`arrows.coax` `e2a_range_lend`) and gives the seat its
own back when it leaves. A Longbow held makes any class's kit the bow
(`class-kit.coax` `xk_kit_held`): its Attack is the Longbow's shot, aimed as the
Ranger's, and on its bar at every level (a Druid's waits for level 2 elsewhere).
The page draws and looses by the same rule. The Wizard, Bard, Sorcerer and
Warlock have no Attack to shoot with yet.

## The pane (#13519)

On `/worlds/archery-range` the range's panel is a pane the browser draws beside
the canvas (`apps/openagents-web/range-pane.coax`, the kit's dock in the mono
skin; `coax/web/host.js` "A page's own pane"). The page tells it three lines
(`egr_pane_put`): `pane HERE MODE RUN`, your last arrow as `e range`, and your
tally as `e tally`. A press comes back as message 740 with one word: `reset`,
`foot`, `horse` or `dragon`, which the page sends as kind 23 or 24.
