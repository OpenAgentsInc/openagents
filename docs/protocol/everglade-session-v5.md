# Everglade session profile v5

Version 5 adds the room's forest state to the existing entity replication
path. Its machine-readable contract is
[`everglade-session-v5.json`](../../proto/verse/everglade-session-v5.json).
Version 4 remains unchanged. All existing message fields and units retain
their meanings, except the explicitly typed tree fields below.

Rooms and pages use version 5, with oldest version 5 and build pins 1.1.1.
A version 4 page receives `r 0 version` and reloads. The separately published
legacy-wire sunset remains unchanged.

## Trees are state

Kind 26 identifies a tree record. Its entity ID is 1,162,000 plus its sparse
record slot (1–191). `CLIP` identifies the planted tree; record reuse advances
`LIFE` and starts a new interpolation history. The room publishes each occupied record,
including burnt trees after their fire goes out. The 512-entry entity table
reserves the original slots for creatures, abilities, boats and doors and a
separate 192-slot forest range. No cast loses capacity to a tree.

The JSON's `forest.fields` defines this kind's row fields. Tilt, break
height, resting angle, moving base and extra foot lift determine the same
trunk, stump and collision shape on every page. Soot, leaf loss and frost
determine its appearance. The burning bit, starter, age and lifetime let
pages draw its fire without simulating damage or spread.

Tree rows use the existing whole-tick ACK, absolute/delta baselines,
retransmission, LIFE/WARP and acknowledged removal rules. Every seat receives
the forest state, including late joiners. Rebuild removes the rows; slot
reuse advances the record's life; a pending removal completes before a new
life is sent. A planted tree can move to another record without an old
record's removal erasing it. Byte budgets remain
unchanged. Trees use the same priority and overdue-row service as other
entities; qualification checks full-forest snapshot delivery.

The room advances tree motion and fire once per world step. A page in a room
only interpolates the room's tree transforms and draws their fire. It never
spreads tree fire or advances tree damage locally.

## Check

```sh
coax/web/tool.sh apps/openagents-web/everglade-wire-profile.coax proto/verse/everglade-session-v5.json
```

The independently written tree fixture checks every forest field through
the ordinary row writer and reader. Room and page suites cover burn-down,
starter attribution, snapshots, delayed delivery, rebuild and record reuse.
