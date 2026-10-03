# Verse Engine delivery roadmap

Status: proposed work, October 3, 2026. This roadmap extends the
[architecture](architecture.md) and the
[#10406 authority plan](https://github.com/OpenAgentsInc/openagents/issues/10406#issuecomment-5973821300).
The [expanded issue plan](https://github.com/OpenAgentsInc/openagents/issues/10406#issuecomment-5974013251)
connects these stages to the service acceptance.
Verse Engine is the reusable engine; Verse is the metaverse built on it.
The original chamber is the first new game fixture, while Lagrange and Physics
Lab remain regression consumers. This roadmap imports no reference-engine code.

## Original scene implementation

[#10426](https://github.com/OpenAgentsInc/openagents/issues/10426) implements the
first asset-free procedural scene. The `verse_play` native example generates an
original chamber, skeletal placeholder actors, weapons, particles, and UI
sprites; it uses the bundled licensed Fira Mono font. It retains timed dialogue,
camera handoff, local combat, overhead red health bars, damage numbers, corpses,
and respawns. See the [scene instructions](../../../assets/verse/original/README.md).

[#10427](https://github.com/OpenAgentsInc/openagents/issues/10427) extracts generic
pack, skeletal animation, and cinematic contracts into `crates/verse-engine`.
The renderer and original scene consume that headless crate directly; the WoW
adapter preserves its old API through re-exports.

[#10428](https://github.com/OpenAgentsInc/openagents/issues/10428) adds continuous
axis-aligned box sweep and wall sliding queries to `physics`. The original scene
shares room solids between rendering and collision. Movement and Misty Step use
those queries; a thin obstacle cannot be crossed by a large displacement. This
is primitive box collision, not the full capsule/mesh controller of VE-2.

[#10429](https://github.com/OpenAgentsInc/openagents/issues/10429) uses the same
query for third-person camera clearance against walls, columns, and the floor,
without changing the requested orbit distance.

[#10430](https://github.com/OpenAgentsInc/openagents/issues/10430) adds bounded,
deterministic static box navigation in `physics`. Cultists route around columns,
and shared sweeps constrain their placement and control displacements. This is
a visibility graph for a flat primitive room, not a crowd solver or navmesh.

[#10431](https://github.com/OpenAgentsInc/openagents/issues/10431) applies static
visibility to targeted attacks, delayed bow/cast rechecks, directional damage,
and hostile cast/impact admission. Obstructed cultists navigate toward a firing
position. Full projectile CCD and explosion-radius occlusion remain separate
work; this slice does not replace the retained projectile solver.

This delivers the visual procedural milestone of VE-1/VE-3, not the complete
VE-0–VE-6 roadmap. Portable authority extraction, original
rules replacement, service persistence, and multiplayer remain implementation
work. #10406 and #10407 stay open for their full acceptance.

## Reuse inventory

Review baseline: OpenAgents `843c92dd7e`. The entries below were checked against
current source and retained research; existing measurement numbers are historical,
not new benchmark results. No runtime checks were run for this documentation change.

| Existing owned code | Reuse now | Boundary or remaining gap |
| --- | --- | --- |
| [`physics`](../../../crates/physics/src/lib.rs) | Rigid bodies, forces/torques, shape manifolds, contact friction/restitution, joints, sleep, fixed steps, momentum ledgers, sensors, traces, ropes, modal response, plume forces, thruster allocation. | Keep this crate as the generic physics owner. Primitive overlap/contact and ray queries do not provide a finished character controller, mesh collision, navigation, or general time-of-impact CCD. Current contact detection enumerates collider pairs. |
| [`verse-lagrange`](../../../crates/verse-lagrange/README.md) | 120 Hz integration, versioned `StationState`, tick-stamped command patterns, interpolated presentation, seeded plume events, conservation/replay fixtures. | Keep CR3BP, Coriolis/tidal fields, station keeping, orbital time warp, EVA controls, and construction rules in the zone. Do not apply these to dungeon physics. A serializable station state is not a multiplayer database or admission system. |
| [`Physics Lab`](../physics-lab.md) | Nine live scenarios, parameter knobs, procedural geometry, debug overlays, reset/capture workflow. | Use it as Verse Engine's physics inspection fixture. Retain local-only behavior until an authority adapter is implemented. |
| [`pbr`](../../../crates/verse/src/pbr/mod.rs) | Lit materials, physical exposure conventions, sky/depth passes, shadows, irradiance baking, post-processing. | Separate generic lighting/material/camera machinery from Lagrange-specific Sun/Earth/sky and material tables. Confirm capability/budget fit before sharing between desktop and mobile. |
| [`imported`](../../../crates/verse/src/imported/mod.rs) | Project-owned textured/skinned draw submission, local lights, particles/ribbons, overlays, native video capture. | Generalize data contracts. Original builds cannot depend on `verse-wow`, benilla, M2/WMO IDs, imported font/UI files, or a private asset path. Do not discard proven GPU work merely because its first consumer used imported art. |
| [`runtime`](../../../crates/verse/src/runtime.rs), [`nav`](../../../crates/verse/src/nav.rs), and [`zones/assets`](../../../crates/verse/src/zones/assets.rs) | Shared surface lifecycle, input/camera patterns, asset admission/loading budgets, bounded 2D routing. | Preserve reusable behavior; the plaza footprint planner is not a 3D dungeon navmesh. Player movement/collision belongs behind world admission. |
| [`verse-ruins`](../../../crates/verse-ruins/README.md) | Gameplay behavior, source-parity fixtures, spell/AI observations, failure cases. | Reimplement original game rules against owned contracts. Preserve retained provenance rather than silently relabeling vendored code as new engine code. |

## Physics architecture

`crates/physics` is already the owned physics foundation. The proposed
`verse-engine-physics` boundary in the architecture means an engine-facing
module over that crate and new portable queries, not a second rigid-body solver
or a mandatory crate rename. It owns entity/body/collider lifetime mapping and
query integration; it does not own Lagrange zone rules.

Use a 30 Hz command/rules tick with four 120 Hz physics substeps as the initial
chamber profile. Keep Lagrange's established 120 Hz profile. These rates are
configuration to measure, not universal requirements. Physics runs on authority;
rendering interpolates previous/current poses. Client movement prediction uses
the same admitted solver. Report dropped/capped time; do not advance cooldowns
by a different elapsed time from the simulation that produced damage.

Keep generic physics state in double precision where already implemented. Use
explicit world/local frames and deliberate conversion to GPU float coordinates.
Large-world camera-relative rendering is a separate extension; the current L1
zone's limited extent does not demonstrate a metaverse-wide precision solution.

Add portable APIs for ray, overlap, and shape sweep queries with world/instance,
collision layer, ignored entity/life, maximum distance, and bounded results.
A sweep returns time of impact and contact normal. Distinguish blocking contacts,
triggers, selection queries, and damage volumes. A query must not mutate state.
Static mesh colliders use compiled collision geometry and an acceleration
structure; dynamic colliders retain stable IDs and deterministic ordering.

The character controller needs capsule sweeps, ground detection, gravity/jump,
stairs, slopes, wall sliding, moving-platform motion, spawn depenetration, and
safe teleport endpoints. Navigation consumes compiled walkable geometry and
dynamic blockers but does not replace collision. Projectiles sweep their traveled
segment each substep so thin walls cannot be crossed between endpoints.

Declare corpse behavior: the first chamber uses animated, grounded corpses and
colliders with explicit lifetimes. Full articulated ragdolls require skeletal
joint limits, stable collision filtering, and network/replay support and are
later work. Rope cloth-like motion, modal appendages, and dynamic props can reuse
existing mechanisms where appropriate; fluids, full cloth, and destruction are
independent capabilities, not features inferred from particle rendering.

## Stages and dependency order

| Stage | Work | Depends on | Acceptance |
| --- | --- | --- | --- |
| VE-0: contracts and reuse | Extract IDs, life generations, fixed schedules, commands/events, asset handles, and presentation snapshots; preserve existing physics owner. | Architecture review. | Headless core has no GPU/platform/private-reader dependency. Lagrange and Lab retain their existing behavior through adapters. |
| VE-1: original greybox | Compile a new chamber, new placeholder rigs, UI/font/icons, collision geometry, effects, and a scene timeline. | Asset contracts from VE-0. | Launch with WoW directories absent. Asset dependency/provenance checks pass. Creation of original art proceeds alongside physics and service work. |
| VE-2: physics and movement | Generic query contracts, static mesh acceleration, sweeps, character controller, projectile CCD, navigation, entity/body lifetime synchronization. | VE-0 and VE-1 collision fixtures. | Character stops at thin walls, climbs admitted stairs, slides on walls, handles slopes/teleports, and NPCs route around obstructions. L1 conservation and Lab contact/joint tests remain valid. |
| VE-3: rendering and animation | Generalize owned PBR/imported GPU paths, original skeletons and named state transitions, local shadowed lights, effects, HUD and native capture. | VE-0 and VE-1 packs. | Original dark-room video shows torch and spell lighting, smooth combat transitions, grounded corpses, and actual-damage numbers. No source-art dependency remains. |
| VE-4: owned local game rules | Move all ten abilities, enemy AI/casts, shields/statuses, damage/death, ritual triggers and respawn into the portable authority adapter. | VE-0 and VE-2; VE-3 for visual acceptance. | Human and controller share admission. Claude starts with 300,000 HP; every cultist respawns 60 simulated seconds after its death with a new life generation. Camera/yells are programmatic events. |
| VE-5: service, saves, multiplayer | #10406: authenticated rights, instance ownership, replication, prediction/reconciliation, inventory/items, quest/progression, transactions/checkpoints and replay. | VE-0 contracts; integrate VE-2/VE-4 incrementally. | Two players and a spectator share authoritative combat; rewards survive restart and retries without duplication; reset is instance-scoped; vmangos is absent. |
| VE-6: tools and measured scale | Scene/property/timeline editors, undo/redo, validated reload, profiling, quality tiers, streaming and mobile adapters. | Start pack inspector in VE-1; extend as contracts stabilize. | Edit content without writing game-specific renderer code, recover from failed reload/device loss, and meet measured device budgets. |

VE-1 and VE-3 do not wait for VE-5. #10407's final world acceptance depends on
#10406, while its original-content work starts in VE-1. The first comparable
original-engine video lands after VE-4 using a local authority adapter; #10406
closes only after VE-5's actual service, persistence, and multiplayer checks.

Each implementation slice needs its own claimed issue and targeted tests.
This document does not create or claim those future implementation issues.
#10425 tracks this documentation update. Existing #10406/#10407 retain their
scope and remain open until their implementation acceptance passes.

## Physics research priorities

The retained [Chaos candidate study](../../research/unreal/2026-09-27-chaos-physics-candidates.md)
is a dated candidate list, not evidence that every mechanism landed. Reconcile
each candidate with current source before creating implementation work.

1. Establish stable simulation/input/solve ordering, state checkpoints, and
   divergence traces before network resimulation. Reuse Lagrange's patterns;
   do not assume a whole-world replay guarantee from a local trace.
2. Prioritize overlap/sweep queries and static geometry for the chamber.
   Add collision-pair exemptions, trigger semantics, and generation-safe query
   filtering where the tests require them.
3. Address contact persistence/warm starting and position correction that does
   not inject physical energy when fixtures reveal drift or unstable stacks.
   Treat separation impulses and restitution thresholds explicitly. Preserve
   momentum and record external field/constraint impulses in the ledger.
4. Add a scalable broadphase, manifold persistence/hysteresis, friction anchors,
   or sleep refinements based on measured chamber/prop loads. The current
   small EVA benchmark is not a large multiplayer or dungeon benchmark.
5. Add time-of-impact handling for fast dynamic bodies and articulated joint
   features only when the playable fixtures require them. Projectile sweeps
   are already required at VE-2. GPU rigid-body solving, general fluids, and
   arbitrary cloth are later capability proposals.

Use the public papers and permissive references cited in the candidate study
for our implementations. The [Genesis roadmap](../../physics/2026-09-27-genesis-port-roadmap.md)
records existing owned work and oracle limits. The
[Lagrange realism audit](../../research/unreal/2026-09-27-lagrange-realism-audit.md)
records landed lighting/material/camera work, including explicit rejected and
unadopted features. Preserve these historical statuses rather than converting
all research candidates into approved scope.

## Accessing Unreal for further study

Epic's [source-access instructions](https://www.unrealengine.com/ue-on-github/)
were checked on October 3, 2026. Access uses a verified Epic account and a
personal GitHub account: connect GitHub under Epic's Apps & Accounts, complete
account linking and OAuth authorization, accept the applicable EULA, and accept
the emailed EpicGames organization invitation within seven days. Then the
licensed account can access `EpicGames/UnrealEngine`. Epic's
[download guide](https://dev.epicgames.com/documentation/unreal-engine/downloading-source-code-in-unreal-engine?lang=en-US)
explains branches and source downloads. The Launcher supplies an installed
editor; the GitHub route supplies source for inspection and modification.

Private clone paths already exist at `~/work/UnrealEngine` and
`~/code/UnrealEngine`; existence does not establish current remote access or
source revision. This update reads retained research and public docs only,
not the private Unreal source, and makes no new access/license assertion.

Before future source reading, follow the
[repository's Unreal study policy](../../research/unreal/AGENTS.md), including
its account/data-use condition. Keep any checkout outside tracked trees and
record the exact revision studied. Study a specific engineering gap, write an
independent technique note, find its public basis, then implement our Rust
solution from that note. No Unreal code, transliterations, shaders, Starter
Content, or MetaHuman assets enter Verse Engine. Obtaining access does not make
Unreal an engine or asset dependency of this project.

## Evidence and documentation ownership

The architecture owns stable boundaries; this roadmap owns delivery order;
#10406 owns world-service acceptance. Runtime guides describe what currently
works. Keep Lagrange, Physics Lab, and the Genesis/Unreal research notes linked
as evidence rather than duplicating historical benchmark numbers.

Required checks cover pack provenance, stale handles/life IDs, collision and
query results, conservation/energy, deterministic command replay, character
movement, physics-to-render interpolation, combat events, and restart-safe
rewards. Measure CPU simulation, GPU execution, capture encoding, asset memory,
and replication separately. Use synthetic/original fixtures and temporary
hosts; existing release gates remain reserved for releases.
