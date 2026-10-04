# Verse world

This portable crate owns chamber authority, typed commands, controller admission,
combat state, fixed-tick movement/collision admission, encounter AI, utility
spells, life-fenced respawns, and serialized events and checkpoints. It has no
GPU, socket, platform, credential, or retained Ruins dependency. The native
chamber reads its snapshots and cinematic projection. Human and controller
requests share admission. A trusted adapter supplies controller identity;
this crate does not authenticate network connections.

The `verse-chamber-owned-v11` rules profile independently implements retained
chamber behavior; it imports no vendor source. Firebolt deals 8 damage, each of
three magic missiles deals 4, and fireball deals 15 in a visible 6.096-meter radius
with three 6-damage burn ticks. Living characters regenerate one mana per second.
Spell projectiles stop at static box cover and sweep against relative actor motion
in 120 Hz slices. Actual character substep paths survive checkpoint replay;
admitted teleports skip intermediate space.
The native adapter admits commands at 30 Hz. One world clock consumes bounded
120 Hz steps for movement, combat, and timers; fractional input time remains in
the accumulator. Combat elapsed time derives from the step count, and scene time
uses a retained origin. Checkpoints validate both clocks and retain pending input.
Checkpoints rebuild static collision and compiled walkable
navigation from the scene profile. Grounded capsules admit slopes, stairs, jumps, and moving platform
poses; NPC routes fence dynamic blocker generations.

Life-bound kinematic player and NPC bodies retain 60-second corpse collision and
navigation masks across checkpoints. Removal and respawn fence exact generations.
Collision-only prop bodies share the blocker life, bounds, and removal fences;
checkpoints reject missing or mismatched prop ownership. Props cannot gain actor
damage or selection masks.
Living bodies contribute exact capsule colliders to movement queries. Controllers
ignore their own life, later actors observe admitted earlier movement, and
checkpoint loading rebuilds live colliders. Navigation plans use static and prop
geometry; controllers still admit contacts with other living actors.
The bow launches non-homing arrows through the shared continuous collision path;
cover and the first actor intercept them, moving targets can evade them, and
pending arrows replay exactly.
Hostile flights retain positions, sweep against player controller trajectories
and cover, and resolve shields through the shared damage path. Checkpoints fence
source and target lives and reject inconsistent flight positions.
Transactional saves, multiplayer
replication, and authoring tools remain on the
[engine roadmap](../../docs/verse/engine/roadmap.md).
