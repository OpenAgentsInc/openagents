# Room regression stays within the existing traversal budget

Integration optimization on 2a3a5b6728, cached compiler 0.52.0. The retained
red is the full-room horde failure in srd-room-leave-2026-11-08: traversal
134217729 exceeds 134217728, with whole-creature copies in `e5rd_up` the
largest contributor (31,209,115 visits before the run aborted).

`e5rd_up` now borrows the creature's one-element view and reads only dead,
HP and x. No value is changed or retained through that view. Alive/present,
seat-live, raid/flee and townsfolk predicates are identical. No AI, fixture,
assertion, frame count, clock, compiler or budget changes.

The exact original 1,400-frame horde fixture passes at 110,552,287 traversal
visits, below the unchanged 134,217,728 limit. It retains every movement,
holding, cast-distribution and burst-density assertion. The whole room library
now passes **325 of 325 checks**. Tools-check and diff check pass. No core
policy changed, so prior mutation receipts are reused instead of introducing
a blanket integration mutation gate.

```
coax/bin/coax test apps/openagents-web/everglade5e-room-rules.coax --library --only 'the horde acts apart'
coax/bin/coax test apps/openagents-web/everglade5e-room-rules.coax --library
```

Passing this suite does not prove all SRD rules or complete #13046 world
qualification. It clears the last of the two previously retained room-library
baseline failures. Actual browser/play and the remaining class/spell/monster,
profile and state-interface scope remain open.

## Full native room host build

The ordinary build also exits 0, including the entire room-server rules graph,
linked headless Everglade world/controller/grid/river/policy units, native C
compilation/linking, math-symbol admission and the real binary's startup check:

```
coax/web/room/build.sh /tmp/srd-room-traversal-server
```

Output: `room: start check: 0 fields off their defaults, the same from any memory`
and `built /tmp/srd-room-traversal-server`. Complete build log and binary/source/
compiler hashes retained. The command finished within the 15-minute limit;
no compiler/check-budget/fast-edit override was used. Headroom notes remain
in the log. This is a successful actual native host build/startup check, not
actual TCP clients, browser rendering or complete gameplay qualification.
