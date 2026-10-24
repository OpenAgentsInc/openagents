# Delivery milestones and acceptance

Status: proposed dependency plan. No milestone is implemented or qualified.
Unlimited engineering permits parallel work after boundaries agree; it does not
remove integration gates. Dates and staffing estimates are intentionally absent.

## Complete target and first proof

The complete target is an owned browser creation platform: editable semantic
worlds, gameplay authoring, high-quality rendering, physics/navigation, animation,
audio/UI/cinematics, multiplayer, collaboration, export, and independent releases.
The first proof establishes one full creator journey with a single-storey house.
It is evidence for the architecture, not a reduction of the complete target to a
house-placement demo. [Product](product.md) owns the capability map.

## Dependency milestones

| Milestone | Required inputs | Deliverable and exit gate |
| --- | --- | --- |
| M0, contracts and qualification fixtures | This specification | Resolve blocking experiments; lock formats/toolchain/profile manifest; create original conformance worlds; approve implementation license before distributing engine code |
| M1, editable world proof | M0 transaction/world/ABI contracts | React DOM/Effect Atom/Effect 4 fullscreen editor loads a semantic cottage; walk inside; select a wall; cut door/remove partition/add room; visible geometry, collision, navigation, and selection activate together; undo/save/reopen and frontend lifecycle/independence checks pass |
| M2, incremental engine | M1 identities and generation contract | Dependency graph/cache, coarse Rust/Wasm kernel, streaming, render graph, device recovery, profiling; stale/canceled/failed work preserves state and resource bounds |
| M3, gameplay and media | M2 and VM contract | Safe behavior authoring/debugging/hot replacement, typed state migration, animation/audio/UI/cinematics, physics interactions, terrain, and complete gameplay loop |
| M4, collaborative projects | M1 durable command protocol and M2 artifact identities | Shared branches, concurrent text, compensating undo, permissions, offline branches/merge/export, recoverable project history |
| M5, independent releases | M2 artifacts, M3 gameplay, M4 identity/durability | Signed immutable packages, channel CAS, promotion/rollback, static player, self-host/export, content-rights admission, active-session policy |
| M6, multiplayer and connected worlds | M3 runtime, M5 release pinning | Authoritative 32-player reference session, prediction/correction, interest management, portals/region handoff, quotas, moderated session access |
| M7, qualification and launch | M1–M6 | Browser/device/accessibility, security, recovery, performance and endurance gates pass on pinned profiles; limitations and supported capability matrix are public |

M0 creates independent **Haunted House** and **Medieval Town Lab** conformance
projects from original or redistributable assets, each with its own save namespace
and releases. Haunted House proves shader/behavior iteration and the Kiln adapter;
Medieval Town proves building structure, walkable interiors, and room/wall edits.
They use the same public engine contracts. Neither becomes a hard-coded editor mode.

The [Medieval Town rendering effort](references/medieval-town.md#phased-effort)
adds source/settings recovery at M0, entrance day/night calibration before full
square matching, environment/material/VFX work across M2–M3, and interactive
reference qualification at M7. Its 16 scenic views are a separate licensed
comparison set. Public conformance worlds retain original/redistributable assets.

Before its milestone starts, each subsystem freezes its remaining wire profiles
and conformance examples. In particular, M0 must specify registered component and
operation schemas, key-registry snapshots, fresh channel-resolution responses,
admission-ledger records, and capability grants. M4 additionally freezes CRDT
frontier/merge protocols; M6 freezes simulation replication and transfer messages.
The initial five JSON envelopes are not permission to improvise these boundaries.
M0 also pins compatible React/React DOM, Effect 4, and Atom React binding versions;
M1 proves [FE-01–FE-08](frontend.md#acceptance-requirements), and later engine and
qualification milestones repeat them under their integrated workloads.

Editor interactions, compiler kernels, platform services, and qualification worlds
can progress independently behind fixtures. M4 does not need to wait for every
rendering feature, but M5 cannot claim publishing works until complete artifacts,
permissions, and rollback have been integrated. No subsystem marks its consumers
qualified based only on a unit fixture.

## Required vertical acceptance stories

1. **Create a playable room.** Open a project without external inference, place
   a walkable cottage, enter through a real opening, edit a partition, place a
   table, attach an interaction, and save. Reopen offline from cached inputs.
2. **Change the running game.** Adjust a shader, introduce a syntax error, keep
   walking, repair, then change a behavior with compatible state. An incompatible
   migration remains held with a clear action; state is not silently reset.
3. **Build a second floor.** Add a storey with a valid stair/ramp/lift connection,
   landing, floor opening, headroom, and reachable rooms. All traversal outputs
   update together. Cosmetic unreachable floors do not pass this story.
4. **Let an agent help.** A bounded request adds known objects through the same
   transaction path. A larger generation job presents a preview/diff and requires
   its configured review. Prompt injection and forged capability attempts fail.
5. **Work together and recover.** Two creators edit interacting walls; one
   conflicts explicitly. One works offline, reconnects, merges, and undoes their
   change without discarding the other's work. A crash during acknowledgment
   produces one durable commit after retry.
6. **Publish and play independently.** Publish a chosen revision, open its exact
   package outside the editor, keep an older session running during a new publish,
   and roll back the channel without pretending already changed gameplay state
   has rewound. Repeat through a self-hosted adapter.
7. **Make a complete game.** A creator combines player input, animated characters,
   physical objects, spatial sound, menus/HUD, triggers, goals/failure/restart,
   scene transitions, and save-state rules. All features survive release export.
8. **Traverse a connected world.** Players cross streamed cells and authorized
   portals under loss/latency, with correct support/collision and explicit session
   ownership. Engine loading cannot expose an empty traversable hole.

## Parallel work boundaries

- Domain/world team publishes pure validators and fixtures; compiler/runtime/UI
  depend on generated contracts, not private internal structs.
- Runtime and compiler teams share generation identities, ABI, traversal profiles,
  and resource accounting before optimizing their internals.
- Editor/AI teams share command generation and uncertainty handling; neither gets
  a privileged bypass for convenience.
- Service teams share branch/release identities, idempotency, capabilities, and
  schema migration with the local host.
- Security and qualification reviewers can reject a surface that lacks evidence
  even if a visual demo appears complete.

## Exit evidence and implementation discipline

Each milestone ships code, conformance fixtures, actual checks, recorded platform
profiles, fault cases, source/rights inventory, and an accurate implementation
status entry. M7 additionally requires usability sessions with indie creators,
accessibility evaluation of the editor journey, hostile-content testing, and a
restore drill. No success criterion depends on testing on an owner's production
account or using non-redistributable art in public evidence.

Use [requirements](requirements.json) to map completed stories to owning specs.
Update a status only with integrated implementation and evidence. Unknowns in
[risks and experiments](risks-and-experiments.md) have named experiments and exit
criteria; they cannot be erased by writing a confident design paragraph.

## Deliberately separate extensions

XR, console certification, mobile-store downloadable-code policy, photorealistic
film rendering, marketplace payments, arbitrary native plugins, and cloud video
streaming are separately specified expansions. The core permits adapters for
them but does not claim they qualify with a desktop browser test. An engine
extension cannot redefine project authority or silently fork the world format.
