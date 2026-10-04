# Verse world

This portable crate owns typed command envelopes and controller admission for
Verse worlds. It has no GPU, socket, platform, credential, or retained Ruins
dependency. The local chamber submits player and controller ability requests
through the same boundary. A trusted adapter supplies controller identity;
this crate does not authenticate network connections.

Game-rule extraction, durable state, multiplayer replication, and movement
command integration remain on the [engine roadmap](../../docs/verse/engine/roadmap.md).
