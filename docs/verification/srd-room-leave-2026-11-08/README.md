# Leaving is not lethal damage

Partial #13046 work on 52f8380896, cached compiler 0.52.0.

The room's leave path marked its caster dead and set HP to zero. This
contradicted the existing room contract: multiplayer spec section 6.3 keeps
spawned creatures/zones when the caster leaves until the effect would end.
The new shared concentration reconciliation correctly noticed death/zero HP
and removed the pack on the next tick. The leave adapter was manufacturing
that lethal transition; the core concentration rule was correct.

`e5rr_leave` now removes active participation and places the caster off-map,
retaining its actual HP/death/concentration state. It neither heals nor revives.
The room retains this off-map owner for effect lifetime. The existing join path
replaces the owner and retires its old effects. Duration expiry, actual zero HP
or death still go through shared lifecycle handling. This is the declared room
persistence profile, not an SRD promise that spell effects survive actual death.

## Evidence

The new regression failed before the code change: leaving fabricated death/zero
HP. It now preserves HP/death state and off-map ownership, keeps the pack after
a tick, then applies actual lethal shared damage and verifies concentration and
summons end. The original Conjure Animals fixture (previously failing) now
passes, retaining every original assertion including replacement cleanup.
The existing lasting-area fixture also passes.

The actual server join/cast/replication scenario verifies both clients and a
late joiner see the pack and that leaving retains it. The separate actual room
vitals/rejoin fixture verifies HP does not refill and down/dead remain so,
including demo and Warlock class changes. Both pass. These are interpreter
protocol scenarios, not actual TCP or rendered browser play.

The full room library suite exits 1 with **324 passed, one failed** of 325:
the remaining horde asynchronous-actions traversal limit. Its unchanged-main
reproduction is retained in srd-room-press-2026-11-08; that compiler resource
failure is not hidden, skipped or given a larger budget here. Tools-check and
diff check pass. Shared core policies did not change; their earlier mutation
receipts remain applicable. This integration correction uses focused verification.

```
coax/bin/coax test apps/openagents-web/everglade5e-room-rules.coax --library
coax/bin/coax test apps/openagents-web/everglade5e-room-rules.coax --library --only 'leaving the room is not death'
coax/bin/coax test apps/openagents-web/everglade5e-room-rules.coax --library --only 'Conjure Animals brings'
coax/bin/coax test apps/openagents-web/everglade5e-room-rules.coax --library --only 'a lasting area is the room'
coax/bin/coax test coax/web/room/room.coax --library --only 'the wolves one seat conjures'
coax/bin/coax test coax/web/room/room.coax --library --only 'vitals (#12883)'
```

No complete room/world qualification or full #13046 closure is claimed. Custom
summons remain house content; this does not implement canonical Conjure Animals.
