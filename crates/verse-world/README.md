# Verse world

This portable crate owns chamber authority, typed commands, controller admission,
combat state, fixed-tick movement/collision admission, encounter AI, utility
spells, life-fenced respawns, and serialized events and checkpoints. It has no
GPU, socket, platform, credential, or retained Ruins dependency. The native
chamber reads its snapshots and cinematic projection. Human and controller
requests share admission. A trusted adapter supplies controller identity;
this crate does not authenticate network connections.

The `verse-chamber-owned-v6` rules profile independently implements retained
chamber behavior; it imports no vendor source. Firebolt deals 8 damage, each of
three magic missiles deals 4, and fireball deals 15 in a visible 6.096-meter radius
with three 6-damage burn ticks. Living characters regenerate one mana per second.
Spell projectiles stop at static box cover and sweep against relative actor motion
in 120 Hz slices. Actual character substep paths survive checkpoint replay;
admitted teleports skip intermediate space.
Shared authority runs at 30 Hz in the native adapter; `tick` also retains bounded
fixture steps. Checkpoints rebuild static collision and compiled walkable
navigation from the scene profile. Grounded capsules admit slopes, stairs, jumps, and moving platform
poses; NPC routes fence dynamic blocker generations.

Life-bound kinematic player and NPC bodies retain 60-second corpse collision and
navigation masks across checkpoints. Removal and respawn fence exact generations.
Live-body contact integration, prop body ownership, unified clock ownership,
bow and hostile projectile integration, transactional saves, multiplayer
replication, and authoring tools remain on the
[engine roadmap](../../docs/verse/engine/roadmap.md).
