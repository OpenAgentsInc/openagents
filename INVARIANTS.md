# Invariants

## Invariants by Context

The repository's invariants are partitioned by context (#13845) so agents and humans can read the exact invariants covering the area they are editing without loading a monolith.

| Context File | Invariants | Scope |
| --- | ---: | --- |
| [bitcoin/INVARIANTS.md](bitcoin/INVARIANTS.md) | 11 | Bitcoin protocol, explorer, P2P, chainstate, script, PSBT |
| [pay/INVARIANTS.md](pay/INVARIANTS.md) | 2 | Payments, Spark wallet, Lightning, LNURL, tips, voice-meter |
| [nostr/INVARIANTS.md](nostr/INVARIANTS.md) | 5 | Nostr relays, NIPs, events, signing, NIP-44 |
| [psionic/INVARIANTS.md](psionic/INVARIANTS.md) | 82 | Psionic ML, tensors, kernels, models, serving, Pylon, robotics, ASR |
| [everglade/INVARIANTS.md](everglade/INVARIANTS.md) | 63 | Everglade realm, SRD 5.2.1 combat, NPCs, mounts, world edits |
| [metaverse-engine/INVARIANTS.md](metaverse-engine/INVARIANTS.md) | 37 | Metaverse Engine, render, physics, audio, anim, editor, scripts, net, session |
| [coax/web/api/INVARIANTS.md](coax/web/api/INVARIANTS.md) | 2 | HTTP API, auth, NIP-98, Postgres schema/migrations, room-session API |
| [apps/INVARIANTS.md](apps/INVARIANTS.md) | 4 | Desktop, iOS, web, CLI, Coder, components, Metaverse OS, Linux/Vulkan host contract |
| [coax/INVARIANTS.md](coax/INVARIANTS.md) | 100 | Coax language, compiler, runtime, #authority, stdlib, coax/web host |
| [gamebuilder/INVARIANTS.md](gamebuilder/INVARIANTS.md) | 4 | GameBuilder 3D creation core, command table, CLI/MCP, play loop, sync, publish |
| [music/INVARIANTS.md](music/INVARIANTS.md) | 4 | Music Studio core, drift-free tempo map, SMF codec, programmatic remix, real-time transport |
| [docs/INVARIANTS-CROSS-CUTTING.md](docs/INVARIANTS-CROSS-CUTTING.md) | 8 | Cross-cutting & general repository invariants |

Add new invariants to the matching context file above (or create a new `<context>/INVARIANTS.md` and link it here).
