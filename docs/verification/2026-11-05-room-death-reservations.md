# Raid death reservations

Issue #12841, tested part. The previous 32-result queue discarded deaths when
more creatures fell together. The new regression fills all 96 physical creature
slots, kills them in one step, and fails on that implementation.

Each slot now reserves one result. A pending result prevents another raid
creature from taking that slot, so reinforcement cannot clear its contribution
bits before XP is applied. The room settles results before reinforcement,
awards each once and releases its slot reservation. A dead source stays for
retry if recording cannot reserve space. Explicit event clear settles the
pending results first. This bound follows the physical slots and their reuse
fence rather than assuming a larger arbitrary queue will be sufficient.

The regression verifies all 96 distinct outcomes, no duplicate on another
cleanup, refusal of new raid admission while those slots are pending, full XP
to the contributing seat, no XP to the other seat, consumption exactly once,
and admission after release. The independent outcome-slot authority kills
13/13 mutants, with zero survivors and no equivalents. The full room suite
passes 609/609. Raw results are in `room-deaths-2026-11-05/`.

The release worker is 1,025,631 / 1,060,000 bytes, SHA-256
`708a192fcee5069402317b857cdc75f72ef08f21ebb827e8f3ab6281bed460f9`.
Its generated-source manifest records the exact six unit keys and compiler
`cdaceb89e69c8d523e41242a940af34774e8b6b4f945998c2d3bf38486c357fb`
(the existing 0.48.0 image). Sources include `3e6a1e1984` and this patch, including
the concurrent `f9c2037818` fire-ownership change already in that base. Later
concurrent native-audio/socket-host commits do not alter these unit sources.
This is a room component/compiler profile, not a graphics-compiler admission.
The native room links these same generated units with strict float flags;
its defaults/startup check passes, and its SHA-256 is
`682c794700a6db58fc5be2414d22a80b4067baa8be05277b2dd80e98de8813ab`.
Unchanged unit objects are reused only after both generated C and header
compare byte-for-byte. The changed current world unit is freshly compiled.

This does not close the downstream world-strike and presentation-pool work or
the integrated pets/destruction/outage qualification. No production service,
database, deployment or visible browser is involved.

Run in an isolated worktree, with each command bounded separately:

```sh
nice -n 15 env COAX_JOBS=1 coax/bin/coax mutate aethyr/session/outcome-slot.coax --jobs 1
nice -n 15 env COAX_JOBS=1 COAX_CHECK_JOBS=1 coax/bin/coax test coax/web/room/room.coax
nice -n 15 env COAX_JOBS=1 COAX_CHECK_JOBS=1 coax/web/tool.sh coax/web/room/worker.coax /tmp/everglade-room.wasm
```
