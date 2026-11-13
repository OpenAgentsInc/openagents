# Everglade projectiles v1

Arrows over a room: the page tells the room it loosed one, and the room
flies it and tells every page. Profile:
[`everglade2-combat-v2.json`](../../proto/verse/everglade2-combat-v2.json).
Codec: `metaverse-engine/net/projectile.coax`, which the page and the room both load.
Whole numbers a space apart (`metaverse-engine/net/record.coax`): positions in mm,
velocities in mm/s, strength in thousandths.

| Frame | Fields |
| --- | --- |
| `F` | `SEQ FX FY FZ TX TY TZ ST VIEW`, page to room: where it left, where it was aimed, how strongly it was drawn, and the room moment the page drew the others at (ms of the room's ticks, its low nine digits) |
| `a` | `TICK ID SEAT SEQ FX FY FZ VX VY VZ ST AGE`, room to pages: projectile ID left SEAT's hand AGE ms before TICK at this velocity. SEQ is that seat's request, so its page matches its own guess |
| `d` | `TICK ID KIND X Y Z DX DY DZ BODY`, room to pages: it ended. KIND 1 stuck at (X, Y, Z) pointing (DX, DY, DZ) in thousandths, 2 in body BODY, 3 gone. BODY is -1 when none. A KIND 1 arrow stuck in one of a realm's standing targets names it in BODY (Everglade's Archery Range: its face, 0 to 6, [`everglade-range-v1.md`](everglade-range-v1.md)); a page that does not use it draws the arrow stuck where it is |

The room flies a launch from VIEW against the bodies as they stood then,
within the past it keeps (`metaverse-engine/net/rewind.coax`: 24 ticks), so what the
archer's page showed is what the room judges. An `a`'s AGE is at most 250 ms. A frame with a
field missing, extra, farther than 2 km from the origin, or faster than
200 m/s is refused whole. The page takes each `a` and `d` once by its ID.
