# Ruins simulation for Verse

`verse-ruins` runs the original Ruins of Atlantis Wizard Woods combat and
movement code inside Verse. The three hotbar controls replace the source's keys
1–3: Firebolt, Magic missile, and Fireball. Monsters move, choose targets, and cast
while the player moves. There is no turn-based encounter controller in this crate.

## Source and scope

The source is the public Apache-2.0
[Ruins of Atlantis repository](https://github.com/OpenAgentsInc/ruinsofatlantis/tree/daeb5d0895270159ec8c18341b4adb84bf7a4346),
pinned at `daeb5d0895270159ec8c18341b4adb84bf7a4346`. The retained forest path is
`platform_winit` → `server_core::zones::boot_with_zone("wizard_woods")` → the
`server_core` schedule. That source revision's default native launcher points to
a separate Bevy dragon scene; this port does not use that scene.

Ten source crates live under `vendor/crates`: `server_core`, `client_core`,
`ecs_core`, `net_core`, `data_runtime`, `core_materials`, `core_units`,
`voxel_proxy`, `voxel_mesh`, and `collision_static`. Their source and retained tests
carry the original [license](LICENSE) and [notice](NOTICE). The
[provenance manifest](provenance.json) records every original path and SHA-256,
the retained SHA-256, and each adaptation. Gameplay algorithms and the controller
remain source copies. The adapted data loaders embed pinned configuration, so an
installed phone uses the same values without a developer's working directory.
The original archetype fallback remains in use because this source revision has
no `archetypes.toml`. Unused server telemetry dependencies are omitted.

The exact active terrain snapshot is under `data/wizard_woods`, with a separate
[scene provenance manifest](data/wizard_woods/provenance.json). It has no tree
instances. Its terrain spans ±150 meters and uses a 129 × 129 height grid.
The separately downloaded Ruins model pack is owned by Verse's asset loader;
this crate does not fetch models, own a renderer, or connect to a relay.

The authored `chamber_spells` module reimplements Misty Step, Thunderwave, Web,
Grease, and Light from the sibling SRD 5.2.1 guide. It debits the retained mana
pool and changes the same actor health and positions. It adds deterministic
local control durations and cooldowns without changing the vendored forest
schedule. See `docs/wow/episodes.md` for the real-time adaptations and omissions;
SRD attribution is retained in `NOTICE`.

## Host boundary

`Simulation::new` spawns the source player and Wizard Woods population.
`tick` mirrors the host's player pose and advances the original schedule.
`cast` queues one request; the original spellbook, stun, cooldown, and mana gates
decide whether to accept it. Hosts supply the source frontend's origin and aim:
terrain height plus 1.4 meters, offset 0.25 meters along the player's horizontal
facing. Source ingestion adds its own 0.35-meter offset. A touch produces one
request, without also replaying the source frontend's legacy animation callback.

`Controller` and `MovementInput` re-export the original `client_core` types.
Keep the controller across frames to retain its jump velocity and ground state.
Its run speed is 6.4008 m/s, sprint multiplier is 1.3, gravity is 9.81 m/s², and
jump velocity is 4.6 m/s. The Verse plaza uses its own controller.

`Snapshot` carries player resources, three ability states, actors, projectiles,
source impact events, and counters. `casts` counts queued valid input requests,
including requests later refused by the source. `projectiles` counts observed
new projectiles from all casters. `hits` counts original impact effects; it is
not a damage, kill, or successful-player-hit count. Effect kinds are 0 for
Firebolt, 1 for Fireball, and 2 for Magic missile; not every source damage path
emits an effect.

`ruins` exposes the original destructible chunk meshes outside mobile JSON.
`ruin_revision` changes when source mesh deltas arrive. `solid_at` reads the live
voxel occupancy using the source mesh transform. It is a host query, not a new
movement solver. The source's capsule-versus-OBB collision function is a stub;
this crate does not claim the original slide solver provides solid ruin walls.

## Retained behavior and limits

The source boots 35 zombies, four NPC wizards, Nivita, the Death Knight, and the
player. It retains pursuit, separation, NPC spell selection, homing missiles,
Fireball area damage, mana regeneration, status durations, death, and voxel
carving. A reset creates a fresh simulation; it is a host restart, not a retained
source server respawn command.

Several source quirks remain visible. NPC collision Y remains 0.6 while their
rendered models follow terrain; actor projectile collision checks use XZ, so
player casts can still hit them. Source NPC missiles can appear above or below
the rendered caster. Ruin mesh creation applies its grid origin and proxy origin
both; meshes and occupancy retain that placement. Melee selects wizard NPCs,
not the player faction. Insufficient mana can still start a cooldown. Source
burn damage rounds each short frame down to zero, and its area status assignment
is broader than its hostile direct-damage assignment. These are not silently
replaced with new game rules. See the [Verse source-parity audit](../../docs/verse/ruins-source-parity.md)
for the original paths and integration decisions.

The adapter rejects non-finite positions, invalid frame deltas, invalid cast
directions, defeated-player requests, and more than 16 pending player requests.
The defeated-player refusal closes an original input-gate omission without
changing the retained schedule. It does not add AI calls,
network transport, benchmark runs, or persistence. Snapshot counters describe
observations; they do not establish multiplayer authority.

## Verification

Run `python3 crates/verse-ruins/verify-source.py` to check all retained source
hashes and declared adaptations. Add `--source-root /path/to/ruinsofatlantis` to
compare the originals too. Run `cargo test -p verse-ruins` for adapter, terrain,
movement, autonomous combat, cooldown, damage, death, and mesh checks. The retained
`server_core` and `client_core` tests provide source-level coverage; upstream
ignored tests remain ignored and do not count as passing evidence.
