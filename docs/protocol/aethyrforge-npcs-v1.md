# Local Metaverse Engine NPC commands v1

The desktop scene tool adds `npcs` (read live NPCs), `npcs people` and
`npcs buildings` (read target catalogues), and `npc 'JSON'` (apply a command).
Typed and finalized voice Coder turns call the
same prebuilt Coax program. The object command v1 contract is unchanged.

The local-only endpoint is `/api/playtest/forge/npcs`. GET returns the current
version, authored NPCs with stable IDs and groups, their live positions/HP/orders,
and targets: named townspeople, live players, other creatures and buildings.
POST accepts [NPC command v1](../../proto/aethyrforge/npc-command-v1.schema.json).
It requires development mode and its existing admin grant; production exposes
no new command endpoint. It operates on the one active local Everglade room.

`spawn` creates 1–8 cultists, villagers or guards on checked open ground, with
one group for the batch. `order` selects one `npc` or a whole `group` and gives
`attack`, `move` or `stop`. Attack names `target_kind` and `target_id` from GET;
move supplies both coordinates in centimetres. `remove` removes that selection.
Missing, dead, self or invalid targets refuse before changing any order. Spawns
and group orders are atomic. Capacity is 32 authored NPCs and the room's free
creature slots. The tool supplies `base`; a concurrent command returns 409.

The host atomically saves the Coax-authored NPC document separately from the
object document, then commits it to the room. Its format is
[NPCs document v1](../../proto/aethyrforge/npcs-v1.schema.json), written and
read by `aethyr/forge/npc-document.coax` and checked by
`aethyr/tools/npc-document-profile.coax`. Definitions and orders survive
reopening. Combat HP and movement are playtest state and start afresh on a new
room. Spawning does not start an automatic raid or let the raid controller
replace explicit orders. The simulation uses existing SRD stat blocks, damage,
death events, animated character parts, world collision and breakable town
pieces. Building attacks damage the real building pieces through the existing
strike path. This is local editing, not public publishing.

The room refreshes its placed-object fences before floor probes and every NPC
step. Stopped NPCs are pushed clear if an object rises under them. Villager
appearances use the existing humanoid outfits, excluding the town's cat slots.
