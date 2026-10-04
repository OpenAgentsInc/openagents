# Verse world

This portable crate owns chamber authority, typed commands, controller admission,
combat state, fixed-tick movement/collision admission, encounter AI, utility
spells, life-fenced respawns, and serialized events and checkpoints. It has no
GPU, socket, platform, credential, or retained Ruins dependency. The native
chamber reads its snapshots and cinematic projection. Human and controller
requests share admission. A trusted adapter supplies controller identity;
this crate does not authenticate network connections.

The `verse-chamber-owned-v1` rules profile independently implements retained
chamber behavior; it imports no vendor source. Firebolt deals 8 damage, each of
three magic missiles deals 4, and fireball deals 15 in a visible 6.096-meter radius
with three 6-damage burn ticks. Living characters regenerate one mana per second. Swept projectiles stop at static box cover.
Shared authority runs at 30 Hz in the native adapter; `tick` also retains bounded
fixture steps. Checkpoints rebuild static collision from the scene profile.

Transactional saves, multiplayer replication, the complete capsule/mesh movement
controller, and authoring tools remain on the
[engine roadmap](../../docs/verse/engine/roadmap.md).
