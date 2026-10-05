# Destructible buildings

Status: research and specification, October 4, 2026. Nothing here is
implemented by this document; a standalone phase D1 demo runs with `verse
--demolition` ([`demolition/`](../../crates/verse/src/zones/everglade/demolition/mod.rs)),
and Everglade's own town breaks under the same rules
([Everglade's town](#everglades-town)).
The owner asked: "If I have a huge sledgehammer
and I wanna start attacking buildings and damaging them, and if they fall
apart, to start crumbling and leaning or whatever — whatever is needed and
missing from the engine. Write a specification for that."

This page records what the engine already has, specifies a design for
destructible buildings in Verse, lists the gaps with effort estimates, and
orders the work from a one-cottage demo to shared, persistent destruction.
Everglade is the first target because its buildings are the only ones in
Verse assembled from modular pieces.

## Summary

- Most of the mechanism exists in one place: Wall of Stone. Its rig in
  [`wall_of_stone/rig.rs`](../../crates/verse-world/src/wall_of_stone/rig.rs)
  already models load-bearing panels as dynamic granite cuboids welded along
  shared edges, with per-panel hit points and armor class, joints that break
  when the solver holds them at a force or torque limit for `BREAK_STEPS`
  steps, debris that despawns after `DEBRIS_LIFETIME` (20 s), and
  checkpointed state. A collapsing bridge in its tests is a collapsing
  building in miniature.
- Everglade's buildings are already structural elements. `layout::house` and
  `layout::hall` place Medieval Village MegaKit pieces on a 2 m grid: 2 m by
  3.12 m wall sections, corner posts, an 8 m by 10 m roof, gables, and a
  chimney. A wall section is a natural breakable piece, and grid adjacency
  gives the support graph without hand authoring.
- Everything around the simulation is static. Placements merge into 8 m
  render cells per material and upload once, light is baked per vertex at
  load, far shadow cascades cache static casters, the player collides with
  axis-aligned footprints, navigation plans over those footprints, and the
  hosted social profile carries no physics world.
- The recommended representation is **kit pieces as structural nodes, each
  with pre-fractured chunks compiled into the pack**. Runtime Voronoi
  fracture and voxels are not recommended for the first versions.
- The smallest demo (phase D1) is one cottage, a sledgehammer on the hotbar,
  and wall sections that crack, break into chunks, and topple, in a local
  game. Structural collapse with leaning (D2), every building within device
  budgets (D3), and multiplayer (D4) follow.

## What exists

### Shared physics crate

[`crates/physics`](../../crates/physics/src/lib.rs) is the owned rigid-body
solver. All state is `f64` (`glam::DVec3`, `DQuat`), ordered by ID, and
serializable.

| Capability | Where | Fit for destruction |
| --- | --- | --- |
| Bodies: dynamic, static, kinematic | `body.rs`, `BodyKind` | Ready. |
| Shapes: sphere, capsule, cuboid; several colliders per body | `collision.rs`, `Shape`, `Collider::at` | Boxes are enough for wall chunks. A multi-collider body is an informal compound. No convex hull, mesh, or cylinder shape in the rigid world. |
| Sequential-impulse solver with warm starting, elliptic Coulomb cone, torsional friction, restitution | `contact.rs`, `SolverSettings` (20 iterations) | Ready; tall heavy stacks need measurement. |
| Contact pairs: capsule–capsule, capsule–box, box–box with clipped manifolds | `collision.rs`, `contact()`, `box_box` | Ready for box chunks. |
| Broadphase | `World::detect` (`collision.rs:638`) | **Missing.** Detection loops over every collider pair with a bounding-sphere rejection. |
| Joints: point, weld, tether; soft springs; force and torque limits that report `saturated` | `joint.rs`, `JointKind`, `Joint::limited` | Limits cap force but **never break**. Wall of Stone breaks joints in caller code. No hinge, slider, cone, or motor. |
| Sleep with union-find islands, wake on touch | `world.rs`, `settle`, `wake_touched` | Ready; an island sleeps only when it rests on something static. |
| Body removal | `World::remove_body` (`world.rs:263`) | **Leaks.** A removed body becomes static with filters `NONE`; its ID is never reused and it stays in the collider loop. Debris churn grows the world. |
| Continuous collision | Speculative contact margin that grows with speed; `continuous::sphere_capsule` | Adequate for falling chunks; no general rotating time of impact. |
| Restorable state and traces | `World` serde (`VERSION = 2`), `trace.rs`, `ledger.rs` | Ready. A restored world continues bit for bit (`a_restored_world_continues_bit_for_bit`). |
| Capsule character over triangle meshes | `character.rs`, `queries.rs` (`Mesh` BVH, `Scene`, `SceneSnapshot` capped at 4,096 colliders and 16,384 triangles) | Separate from the rigid `World`: the character never pushes or is pushed by a dynamic body. |
| Multilayer walkable navigation with dynamic blockers | `walkable.rs`, `Navigation`, `Blockers` | Used by the chamber, not Everglade. |
| Fracture, damage, debris pooling | — | **None** in the crate. |

The [Genesis port roadmap](../physics/2026-09-27-genesis-port-roadmap.md)
landed fixed steps, momentum ledgers, contact manifolds, welds, sleep,
sensors, and calibration in GP-0 to GP-8. It deferred a broadphase "until
GP-6 measurements ask for one", and its step budget is 1 ms per step on the
slowest supported phone. The [engine roadmap](engine/roadmap.md#physics-architecture)
states that "fluids, full cloth, and destruction are independent
capabilities, not features inferred from particle rendering", and audit item
[V14](../audits/2026-10-04-verse-engine-audit.md#v14-physics-acceleration-is-incomplete-at-the-scene-level)
asks for a deterministic broadphase and generic rigid continuous collision
only when a workload requires them. Destruction is that workload.

### Spells that break things

[`SpellWorld`](../../crates/verse-world/src/spells/mod.rs) holds a
`physics::World` of dynamic props beside the chamber's kinematic characters.
It steps at 120 Hz, keeps seeded `dice::Dice` and a momentum `Ledger`, and is
saved with every `Game` checkpoint. Its spells already show the building
blocks:

- **Wall of Stone** ([`wall_of_stone/`](../../crates/verse-world/src/wall_of_stone/mod.rs)):
  `PANEL_AC` 15 and `HP_PER_INCH` 30, so hit points scale with thickness.
  `DamageType::harms_panels` makes panels immune to poison and psychic damage,
  as the SRD makes objects. `rig::Wall::raise` welds panels to each other
  with limits proportional to seam length (`SEAM_FORCE` 120,000 N/m,
  `SEAM_TORQUE` 100,000 N·m/m) and pins them to stone with a soft point joint
  that carries no bending (`FOOTING_FORCE` 20,000 N). `Wall::after_step`
  breaks any joint held at a limit for six steps, so "collapse comes from the
  solver rather than from a rule". `Wall::damage` and `Wall::destroy` turn a
  panel at 0 HP into 4 to 8 debris boxes on `DEBRIS_GRIDS`. `Wall::stress`
  reports per-seam load for display. Damage enters through
  `spells/wall_of_stone.rs::hit` (fixed amounts per projectile) and `struck`
  (from `combat.rs` for hostile flights).
- **Meteor Swarm** ([`meteor_swarm.rs`](../../crates/verse-world/src/meteor_swarm.rs)):
  `Unattended { hp, flammable }` objects take full damage once, `shatter`
  into `DEBRIS_CHUNKS` (8) boxes at 0 HP with a blast impulse, and burn. It
  does not damage terrain or wall panels.
- **Telekinesis** grabs objects with two soft joints and can throw them.
  **Black Tentacles** builds chains of capsules with ball joints and
  force-limited grabs.

[`play/multiplayer.rs`](../../crates/verse-world/src/play/multiplayer.rs)
refuses catalog spells from additional networked players ("This catalog
spell needs a shared-caster adapter"), so all of this is single-caster today.

### Voxel ruins

The Ruins zone's destructible ruin is retained Ruins of Atlantis source, not
an engine feature. `voxel_proxy::VoxelGrid` holds a 32×16×32 grid of 0.5 m
voxels with 8³ chunks; projectiles and explosions push `CarveRequest`s,
`carve_sphere` clears a sphere (Fireball carves 2 m), and
`destructible_remesh_budgeted` greedy-meshes a bounded number of dirty chunks
per tick. The debris the source computes is discarded (`let _out = …` in
`systems/destructible.rs`), so no debris bodies exist. On the Verse side,
`Ruins::build_dynamic` (`zones/ruins.rs:239`) re-emits every ruin chunk as
per-frame triangles with no caching and sets `cache_far_shadows: false`.
Nothing about the ruin is networked; [the parity audit](ruins-source-parity.md)
also records a double-applied grid origin. This path is evidence that carving
works, not a design to extend: voxels at 0.5 m look nothing like the kit's
plaster and timber, and the remesh would have to rebuild textured surfaces.

### Everglade buildings

| Item | Where | Behavior |
| --- | --- | --- |
| Pieces | [`layout.rs`](../../crates/verse/src/zones/everglade/layout.rs), `Piece`, `wall()` | `Plain`, `Base`, `Timber`, `Round`, `Flat`, and `Door` map to `village/Wall_Plaster_*` sections, with separate glass, shutter, and door-frame models. `WALL_TOP` is 3.12 m. |
| Buildings | `hall()`, `house()`, `roof()`, `lane()` | The workshop hall (`HALL`, 16 m × 10 m, 26 wall sections), the Stoop Lane cottage (`COTTAGE`, 8 m × 10 m, 18 sections), the reading room (`READING_ROOM`), the open pavilion (`PAVILION`, six posts), and the fenced strongroom. A house adds four `Corner_Exterior_Wood` posts, one `Roof_RoundTiles_8x10`, two `Roof_Front_Brick8` gables, and an optional chimney. |
| Placement | `Placement` | Model name, ground position, lift, yaw, scale, and a `Collision` kind (`None`, `Bounds`, `Core`, `Opening`). Roofs, corners, and gables have `Collision::None`. |
| Solids | [`solids.rs`](../../crates/verse/src/zones/everglade/solids.rs), [`social/solids.rs`](../../crates/verse-world/src/social/solids.rs) | Each colliding placement becomes an axis-aligned `Footprint` with a top height; round-tile roofs become analytic gabled `Roof` surfaces. `Solids::set_spell_blocks` already replaces a set of spell-raised blocks at runtime. |
| Controller | [`social/controller.rs`](../../crates/verse-world/src/social/controller.rs) | A 0.45 m circle pushed out of each footprint along the shallowest axis; `STEP` 0.35 m step-up. Everglade's Wall of Stone draws a turned panel as "a close row of small" posts because "the controller collides with axis-aligned boxes" ([`everglade/spells.rs`](../../crates/verse/src/zones/everglade/spells.rs)). |
| Pack | [`everglade_pack/`](../../crates/verse/src/zones/everglade_pack/format.rs) | Custom `VTP2` binary: base-color PNGs, materials, static models with node transforms applied, one skinned character. 250,000-triangle and 16 MiB limits. Pinned by `PACK_SHA256`; `content_digest()` is a hosted instance's content identity. Compiled by `compile.rs` from glTF under `assets/verse/everglade/`. |
| Render | [`pbr/textured.rs`](../../crates/verse-pbr/src/pbr/textured.rs) | `merge()` groups placements into `CELL` (8 m) cells per material: one indexed draw per cell, no instancing, uploaded once. |
| Light | [`pbr/textured_bake.rs`](../../crates/verse-pbr/src/pbr/textured_bake.rs), `Everglade::bake_light` | At load, each static vertex stores sky visibility and one sun bounce; an L1 probe grid with 3 m cells shades characters. Keyed by pack digest; a browser bakes 192 items per frame. |
| Shadows | `pbr/gpu.rs`, `Everglade` key | Two cascades on Low and Medium, three on High, 2048². With `cache_far_shadows: true`, cascades after the first redraw only when the static caster identity changes. |
| Particles | `zones/everglade/studio.rs`, `Particles` | CPU quads in vertex-color faces. No GPU particles. |
| Navigation | [`social/nav.rs`](../../crates/verse-world/src/social/nav.rs) | Search on a 2 m grid with exact clearance against the same footprints, `MAX_BLOCKERS` 2,048. Studio seats walk with `social::seats::Walker`. |

### Combat, equipment, and authority

- The [combat model](combat-model.md) is MMO play with SRD dice behind the
  scenes: a 1.0 s global cooldown, attack rolls against AC, damage dice, and
  resistances as multipliers. Clients send intents, never results.
- Equipment has a `MainHand` slot with socket 6
  ([`service/equipment.rs`](../../crates/verse-world/src/service/equipment.rs)).
  No melee weapon, swing ability, or swing animation exists. `combat.rs`
  enemy damage is fixed integers, and its only object target is a Wall of
  Stone panel.
- [Networking](networking.md): the chamber host is authoritative at 30 Hz with
  four 120 Hz physics substeps, replicates acknowledged deltas within a 64 m
  presentation radius and 80 m collision radius
  ([V05](../audits/2026-10-04-verse-engine-audit.md#v05-spatial-replication-bounds-steady-traffic-in-retained-fixtures)),
  and serves Everglade as a hosted social profile with movement, seats, and
  realm transfer but no combat and no `SpellWorld`. NIP-MV `33301` entity
  states and pose frames carry plaza presence only and cannot carry
  authoritative physics.
- Audio: [`audio::Mixer`](../../crates/verse-engine/src/audio.rs) has bounded
  spatial voices; it has no cue banks, buses, or voice priorities
  ([V23](../audits/2026-10-04-verse-engine-audit.md#v23-audio-needs-a-content-and-lifecycle-layer)).

## Design

### Goals and non-goals

Goals:

- A player swings a sledgehammer at any destructible building. Each hit
  rolls behind the scenes, shows a number, cracks the piece, and eventually
  breaks it into chunks that fall and settle.
- Structure responds: an unsupported roof drops, a wall that lost its
  neighbors leans and then falls, and a building whose supports are gone
  collapses. The solver produces the motion; rules only decide damage.
- The result is the same for every viewer in a shared instance and survives a
  late join.
- Desktop, browser, and phone builds run the same rules within declared
  budgets.

Non-goals for this specification: destructible terrain, fluids or fire
spread, cloth, finite-element bending, arbitrary creator buildings, and
destruction in the plaza (amber line geometry has no kit pieces).

### Representation

| Option | How it works | Verdict |
| --- | --- | --- |
| Kit pieces as structural elements | Each placed wall section, post, roof segment, gable, and chimney is a node with hit points and a mass; it breaks whole. | Required as the structural layer. Matches the 2 m grid and the existing `Placement` data. Coarse alone: a section vanishing in one step looks wrong. |
| Pre-fractured chunks per piece | At pack time, each destructible model is cut into 4 to 12 chunks with interior faces and a box collider each. At 0 HP the piece is replaced by its chunks. | **Recommended**, layered on kit pieces. Deterministic, cheap at runtime, and identical on every device. The same choice Wall of Stone (`DEBRIS_GRIDS`) and Meteor Swarm (`shatter`) made with plain boxes. |
| Voxels | Buildings as voxel grids carved by spheres, remeshed per chunk. | Not recommended. Kit textures don't survive voxelization, remeshing textured surfaces is new work, the Ruins path discards its debris, and voxel state is large to replicate. |
| Runtime fracture (Voronoi at the impact point) | Cut the mesh at hit time around the impact. | Deferred. Needs robust mesh clipping of non-watertight kit meshes, costs milliseconds on phones, and makes every chunk a function of floating-point geometry that each client must reproduce. Choosing among a few pre-fractured patterns by impact position gives most of the look. |

So a building has three levels: the **building** (an activation and
replication unit), its **pieces** (structural nodes with hit points), and
each piece's **chunks** (debris bodies that exist only after the piece
breaks).

Pieces that the layout places as dressing (window glass, shutters, door
frames, vines, borders) attach to a host piece and break with it. Glass gets
its own low hit points so it shatters first.

The single `Roof_RoundTiles_8x10` model is one 8 m by 10 m slab. A roof that
drops as one slab reads as a lid falling off. The pack compiler splits each
roof into four segments (two slopes by two halves along the ridge), welded at
the ridge and the midline, so a roof can hinge at the ridge and sag into a
breach.

### Structural model

Two mechanisms work together. A support graph decides **whether** something
is held up; welded joints in the solver decide **how** it moves.

**Support graph.** Build it at zone load from placements and grid adjacency:

- Nodes: structural pieces. Each records mass, material, and a center.
- Ground anchors: pieces whose base rests on the terrain or a floor slab.
- Edges: a **seam** between two pieces that share a grid edge (wall to wall
  along a vertical edge, wall to corner post, wall top to roof segment
  bearing, roof segment to roof segment, gable to wall top, chimney to roof).
  Each seam records its length and a capacity in force and bending moment,
  following Wall of Stone's per-meter rule.

After every break, run a connectivity check: a breadth-first search from the
anchors over intact seams. Any component with no anchor is unsupported and
becomes dynamic. This check is cheap (tens of nodes per building), exact, and
the same on every machine, so it can be authoritative even where the solver
is not bit-identical.

**Activation.** An undamaged building is static: its pieces are static
bodies (or not in the physics world at all) and its colliders are the
existing footprints and roof surfaces. The first hit **activates** the
building: its pieces become dynamic bodies welded along their seams and
pinned to their anchors, the rig `Wall::raise` already builds. An activated
building whose islands sleep for a set time with no damage in progress
**deactivates**: its surviving pieces freeze in their current, possibly
leaning, poses as static bodies, and its settled chunks become static rubble.
Activation bounds solver cost to the buildings someone is hitting.

**Leaning and sagging.** Today a saturated joint either holds or breaks. Two
additions give the in-between states the owner asked for:

1. **Plastic yield.** When a weld saturates its torque limit, move its rest
   orientation (`JointKind::Weld { relative }`) toward the current relative
   orientation by the excess, instead of counting toward a break. The piece
   stays where it was pushed: a wall leans and stays leaned. Each seam
   accumulates yielded angle and breaks past a limit (for example 25°). A
   seam's force limit still breaks it directly.
2. **Hinges.** A seam that lost its partner on one side becomes a hinge
   along the shared edge: two point joints at the edge's ends, or a new
   `JointKind::Hinge { axis }` with an angular limit. A wall section whose
   side neighbors broke then rotates about its base edge and topples, and a
   roof segment rotates about its ridge.

Sag is yield under sustained load: a roof segment whose bearing wall broke
hangs from the ridge hinge and yields until its free edge rests on rubble or
the floor.

Stability notes. Heavy pieces in tall stacks with 20 solver iterations need
measurement; Wall of Stone's `SEAM_GAP` (5 mm) between welded colliders keeps
contacts from fighting welds and applies here. A soft footing
(`FOOTING_FREQUENCY`) lets pins and contacts share load. A building's
pieces filter collisions with their seam neighbors until the seam breaks.

### Damage model

Pieces use the SRD object rules the combat model already applies to Wall of
Stone panels: an armor class by material, hit points by size and
resilience, immunity to poison and psychic damage, and resistance or
vulnerability as multipliers. A 2 m by 3.12 m wall section is a Large
object; a post or door is Medium.

Proposed starting values, tunable per zone:

| Piece | Material AC | Hit points | Notes |
| --- | --- | --- | --- |
| Plaster or timber wall section | 15 (wood) | 27 (Large, resilient, 5d10 average) | The common case. |
| Brick-based wall section, gable, chimney | 17 (stone) | 27 | Damage threshold 5 is optional. |
| Corner post, door leaf | 15 | 18 (Medium, resilient) | |
| Roof segment | 15 | 27 | |
| Window glass | 13 (glass) | 2 | Vulnerable to bludgeoning. |

**The sledgehammer.** A two-handed maul in the `MainHand` slot: SRD maul
damage 2d6 bludgeoning plus the Strength modifier, with a **Wrecking**
property that doubles damage against objects and structures, as the SRD's
Siege Monster trait does. With a +3 modifier a hit averages 10, or 20 against
a structure, so a plaster wall section breaks in about two hits. The ability
is a melee action on the 1.0 s global cooldown with a 0.6 s wind-up. The
attack roll (d20 plus attack bonus against the piece's AC) and damage are
rolled by the authority. The client sends an intent with an aim; the
authority picks the struck piece by a ray or short capsule sweep from the
character's hands along the swing and validates reach (2 m) and line of
sight.

**Impulse.** Damage decides breaking; impulse decides how things move. A
real 10 kg head at 10 m/s carries about 100 N·s, which barely moves a
1,500 kg wall section, and that is correct for an intact wall. Apply the
impulse at the hit point to the struck piece when its building is active,
and, when the hit breaks the piece, to the chunks nearest the hit point
scaled by a named gameplay multiplier so debris flies away from the swing.
Record it as a named external term in the `Ledger`.

**Damage states.** Each piece has four visual states by remaining hit points:
intact (above 75%), cracked (above 40%), broken through (above 0%), and
destroyed (replaced by chunks). Window glass shatters at the first hit.
Cracks are a per-piece shader value, not decals (see [Rendering](#rendering)).

**Other sources.** The same `damage(piece, amount, type, point, impulse)`
entry point serves the hammer, Fireball, Meteor Swarm (which would add
pieces to its `Unattended` list), Thunderwave, and debris that lands on
another piece above an impulse threshold.

**Debris on characters.** A gameplay chunk that strikes a character above an
impulse threshold deals bludgeoning damage from a dice table by mass and
speed (1d6 per band), behind the scenes, and may apply the combat model's
knocked-down debuff for 1.5 s on a failed Dexterity save. Wall of Stone's
`creatures` module already rolls a Dexterity save for a creature a wall would
enclose.

### Physics needs

| Need | Approach |
| --- | --- |
| Many dynamic bodies | Two debris tiers. **Gameplay chunks** are large (above about 0.3 m), simulated by the authority, collide with characters, and are replicated. **Cosmetic chunks** are small, simulated only on each client from a replicated seed, don't collide with characters, and are never replicated. |
| Broadphase | A deterministic sort-and-sweep or uniform grid in `World::detect`, with stable pair order. Activation keeps static building pieces out of it. |
| Shapes | Boxes per chunk, fitted at pack time as oriented boxes. A chunk that a box fits poorly gets two or three boxes on one body. Convex hulls are not needed for the first phases. |
| Joints | Breaking moves into the crate: `Joint::breaks_after(steps)`, reported as an event, generalizing Wall of Stone's `BREAK_STEPS`. Add plastic yield and a hinge. |
| Body lifetime | Free removed bodies and reuse slots with a generation, so debris churn doesn't grow the collider loop. Pool chunk bodies per tier. |
| Sleeping | Already present. Debris sleeps on the ground because it rests on a static body. |
| Debris lifetime | Gameplay chunks settle and become static rubble when their building deactivates; rubble above a per-zone count merges or fades oldest first. Cosmetic chunks fade after `DEBRIS_LIFETIME` (20 s), as Wall of Stone's do. |
| Continuous collision | The speculative margin covers falling chunks against floors. Hammer hits are queries, not bodies. Chunks against the character use a capsule sweep from the character side. |

### Character controller

Everglade walks a circle against axis-aligned footprints. That cannot
represent a leaning wall, a tilted roof segment, or a rubble pile, and it
can't be pushed by debris.

- Move Everglade's walking onto `physics::character::Character`, the capsule
  controller the chamber uses, over a query `Scene`. Static terrain and
  intact building pieces compile once into mesh colliders. Active pieces and
  gameplay chunks enter the `Scene` as moving box colliders each tick. This
  is the engine roadmap's VE-2 controller work applied to Everglade; the
  `SNAPSHOT_COLLIDERS` (4,096) and `SNAPSHOT_TRIANGLES` (16,384) caps bound
  it.
- Keep `Solids` and footprints until that move lands, and keep them as the
  navigation input. Before the move, a broken piece removes its footprint
  and settled rubble adds conservative footprints through the same path
  `Solids::set_spell_blocks` uses.
- Two-way coupling: when the capsule's sweep hits a dynamic chunk, apply an
  impulse to the chunk from the character's mass and speed (75 kg, `Size::creature_mass`)
  and slow the character. When a moving chunk overlaps the capsule, push the
  character with `add_velocity` and apply debris damage.
- Walking on rubble uses the capsule's step-up (0.35 m) and slope limit.
  Settled rubble is static, so standing on it is ordinary ground.
- Depenetration: when a collapse puts a character inside a falling piece,
  `Character::recover` pushes it out; if recovery fails, teleport to the
  nearest safe point the navigation reports.

### Rendering

| Concern | Change |
| --- | --- |
| Merged cells | Destructible buildings leave `TexturedScene::merge`. Each building gets its own batch with one index range per piece, so hiding a broken piece rewrites that building's small index buffer. Undamaged buildings still draw in one call each. |
| Moving pieces and chunks | Add an instanced path: a mesh handle plus a per-instance transform and damage value, as instanced vertex attributes, which GLES 3.0 supports without storage buffers. Ruins' rebuild-every-triangle-every-frame approach does not scale. |
| Baked light | Per-vertex light in surviving pieces goes stale when a neighbor breaks (an interior suddenly sees the sky). Rebake vertices and probes in a box around each structural change, on the worker thread natively and by frame budget in a browser, keyed by pack digest plus the building's destruction revision. Moving pieces and chunks use the probe grid, as characters do, because per-vertex bakes don't move with them. |
| Shadows | Bump the far-cascade static caster identity on each structural change (an event, not every frame). Moving pieces and chunks draw in the first cascade only. |
| Cracks | A per-instance damage value drives a procedural crack mask in the textured shader, sampled in object space so it stays on the piece, and darkens the vertex light near cracks. Damage states need no new textures. |
| Interior faces | Chunk interior faces use a per-material interior color or the material's texture with planar coordinates (plaster core, brick core, raw timber). |
| Dust and fragments | A bounded CPU particle emitter (the studio's `Particles` quads) per break and per heavy landing, with seeded emission so viewers see similar dust. |
| Level of detail | Chunks beyond about 40 m draw as their box. Cosmetic chunks beyond the near cascade are culled. |

### Navigation and the studio

- Each structural change raises a navigation revision. `social::nav::plan`
  replans routes that cross the changed area. Rubble footprints count toward
  `MAX_BLOCKERS` (2,048), which bounds how much rubble the zone keeps.
- Studio seats walk to stations inside the workshop hall. A collapsed hall
  would strand seats or route them into rubble, and the boards and monitors
  are drawn on hall geometry. See [Agent Studio](#agent-studio).

### Audio

Cues for a hit (by material), a crack, glass breaking, a piece breaking, a
heavy landing, and a collapse rumble, played through `audio::Mixer` as
spatial voices. Impacts come from contact reports above an impulse
threshold, limited per building per frame, with a voice priority for the
hammer hit so it never drops. This depends on the cue and priority layer in
audit item V23; the sounds themselves are new content.

### Networking and authority

**Decision: host-authoritative structure with replicated events and
snapshots; no deterministic lockstep.** Physics is `f64` and deterministic
on one build and machine, but the audit
([V27](../audits/2026-10-04-verse-engine-audit.md#v27-replay-guarantees-need-a-declared-execution-profile))
does not promise cross-architecture equality, and browsers, phones, and
desktops will all join one instance.

- **Authority.** The world host owns building state. It runs the support
  graph, the activated rigs, and gameplay chunks inside the instance's
  physics world at 120 Hz. The hosted social profile in
  `verse_world::play::social` gains a `SpellWorld`-style physics world and the
  hammer ability; `play/multiplayer.rs` needs the shared-caster adapter it
  already names for any player to swing.
- **Reliable events.** `PieceDamaged { building, piece, hp }`,
  `PieceBroken { building, piece, pattern, seed }`,
  `SeamBroken { building, seam }`, and `BuildingSettled { building, revision }`
  go through the ordered event stream. Clients spawn cosmetic chunks from
  `pattern` and `seed` without further traffic.
- **Snapshots.** Awake active pieces and gameplay chunks join the existing
  replication delta inside the 64 m presentation radius: an ID, a position
  (three `f32`), and a compressed rotation, about 20 bytes each. Cap
  replicated bodies per viewer at 64, nearest first, at 10 Hz in the outer
  band. 64 bodies at 30 Hz is about 38 KiB/s per viewer during a collapse,
  falling to zero when everything sleeps. Clients interpolate between
  snapshots as they do for actors.
- **Late joiners.** The baseline carries, per building: its destruction
  revision, a bitset of broken pieces, hit points of damaged pieces, poses of
  leaning pieces, and settled rubble poses. A settled cottage is a few
  hundred bytes. Settled cosmetic debris is not replicated; a late joiner
  doesn't see it.
- **Prediction.** The client plays the swing animation, sound, and a dust
  puff at once, and shows numbers and breaks only when the authority's event
  arrives. Predicting breaks would produce rubble the authority might not
  agree with.
- **Local play.** Without a world host, the local game runs the same rules in
  process, as Everglade's spells do today.
- **Nostr.** NIP-MV carries none of this. A public world event may say an
  instance allows destruction.

**Persistence and repair.** A building's state is part of the instance
checkpoint, saved with the content digest that names its pack. Options for
restoring buildings, for the owner to choose:

- **Regrowth:** a destroyed building rebuilds piece by piece after a delay
  (for example 10 minutes) once no player is within 30 m, bottom-up, so
  nobody sees a wall appear in midair.
- **Repair:** a player repairs pieces with a tool or a spell, which could be
  a quest.
- **Reset:** a local zone resets on entry; a hosted instance resets on
  restart.

### Performance budgets

Proposed budgets to measure against, not measurements. They extend the
existing step budgets: 1 ms per physics step on the slowest phone (Genesis
roadmap) and a worst step under 5 ms in the plaza ball test
([mobile](mobile.md)).

| Budget | Desktop | Browser (WebGPU or WebGL2) | Phone |
| --- | --- | --- | --- |
| Active buildings at once | 4 | 2 | 2 |
| Awake gameplay bodies | 256 | 128 | 96 |
| Cosmetic chunks alive | 512 | 128 | 64 |
| Physics per 120 Hz step, p99 | 1.0 ms | 2.0 ms | 1.0 ms |
| Dust particles alive | 2,000 | 800 | 500 |
| Local rebake after a break | 250 ms on a worker | 2 s at frame budget | 1 s on a worker |
| Extra triangles in the pack for chunks | 150,000 | same pack | same pack |

When a budget is reached, degrade in this order: cosmetic chunks first, then
dust, then rebake quality. Gameplay chunks never drop; the oldest settled
rubble merges into static geometry instead.

The browser runs the simulation on the main thread; the same `f64` solver
compiles to wasm. A browser joining a hosted instance runs no authority
physics, only cosmetic chunks.

### Asset pipeline

The Everglade compiler (`everglade_pack/compile.rs`) gains a fracture step
for every model the manifest marks destructible:

1. **Slab fracture.** Wall sections, gables, and roof segments are nearly
   planar. Cut each model's outline in its plane into 4 to 12 Voronoi cells
   from a fixed seed, clip window and door openings out of the cells, and
   extrude each cell through the slab. Front and back faces keep the original
   UV mapping by planar projection; edge faces use the interior material.
   This needs only 2D polygon clipping, not a mesh boolean on non-watertight
   kit geometry.
2. **Patterns.** Two or three fracture patterns per model, with cells
   concentrated near different points, so the authority picks the pattern
   nearest the impact.
3. **Colliders.** One oriented box per chunk, fitted to its cell, with mass
   from volume and density.
4. **Structure.** Material, density, and seam sockets come from the kit's
   2 m grid, so adjacency is derived from placements and not hand authored.
5. **Format.** `VTP2` becomes `VTP3` with a fracture section per model:
   chunk meshes, boxes, masses, pattern IDs, and material class. Limits rise
   to fit the chunk budget above. A rebuilt pack has a new `PACK_SHA256` and
   so a new `content_digest()`, which already admits hosted instances.

Before the compiler step lands, phase D1 can cut a section into a 3 by 2 grid
of boxes at load, as `DEBRIS_GRIDS` does, cropping the original mesh by
planes.

### Agent Studio

The workshop hall is where the [Agent Studio](agent-studio.md) runs: seats
walk to desks, and the Task Wall and monitors are drawn on its geometry.
Destroying it mid-task would strand seats and hide the panels people use to
approve work.

Recommendation: protect the hall, the strongroom, the boards, and every
station's furniture by default. A protected piece reacts to a hit with
sparks, a sound, and "Protected", and takes no damage. Destructible by
default: the Stoop Lane cottage, the reading room, the pavilion, fences,
crates, and the wagon. A per-building `destructible` flag in the layout makes
this data. A host may also declare an instance-wide **safe zone** radius
around the studio in which no destruction applies.

## What exists versus what's missing

Effort is engineering time for one experienced contributor, including tests:
S is up to 2 days, M is 3 to 7 days, and L is 2 to 4 weeks.

| Item | Exists | Missing | Effort |
| --- | --- | --- | --- |
| Breakable joints | Limits with `saturated` (`joint.rs`); break-after-steps in `wall_of_stone::rig` | Break as a crate feature with events | S |
| Plastic yield and hinges | Point and weld joints, soft springs | Yielding weld rest orientation; hinge joint with angular limit | M |
| Broadphase | Bounding-sphere rejection over all pairs | Deterministic sort-and-sweep or grid | M |
| Body removal and pooling | `remove_body` leaves a static husk | Slot reuse with generations; chunk pools per tier | M |
| Compound and hull shapes | Several cuboid colliders per body | Convex hulls (not needed until runtime fracture) | L, deferred |
| Support graph and activation | Wall of Stone rig for one spell | Generic graph, connectivity check, activate and deactivate | M |
| Damage model and object rules | Panel AC and HP, `harms_panels`, seeded dice | Piece materials, states, a shared damage entry point | M |
| Sledgehammer | `MainHand` slot and socket | Maul item, melee ability, hit query, swing animation clip, hammer model | M |
| Debris on characters | Wall of Stone Dexterity save | Impulse-to-damage table, knockdown | S |
| Everglade capsule controller | `physics::character` in the chamber | Everglade walking over a `Scene` with moving colliders; two-way coupling | L |
| Per-building render batches | Merged 8 m cells | Batches with per-piece index ranges | M |
| Instanced dynamic meshes | Per-frame triangle stream; one skinned figure | Instanced transforms and damage values | M |
| Crack shader | Textured shader, per-vertex light | Procedural crack mask from damage value | M |
| Local light rebake | Whole-zone bake at load | Region rebake and probe update on change | M |
| Shadow cache invalidation | Static caster identity key | Bump on structural change | S |
| Dust | CPU quad particles in the studio | Seeded emitters with budgets | S |
| Audio | `audio::Mixer` spatial voices | Cues, priorities, impact limiting, sound content | M |
| Navigation updates | Grid planner over footprints | Revision-driven replanning, rubble blockers | S |
| Pack fracture | `VTP2` compiler from glTF | Slab fracture, patterns, boxes, `VTP3` | L |
| Studio protection | — | Per-building flag, safe zone, feedback | S |
| Hosted authority | Social profile without physics; chamber `SpellWorld` | Physics world in the social profile, shared-caster adapter, events, body replication, baseline | L |
| Persistence and regrowth | Instance checkpoints | Building state in checkpoints, regrowth or repair | M |
| Measurement | `StepStats`, ball and Lagrange step tests | A collapse benchmark on desktop, browser, and phone | M |

## Delivery plan

Each phase is its own issue with targeted tests. Generic mechanisms land in
`crates/physics`; building rules in `crates/verse-world` beside `spells`; the
Everglade adapter and rendering in `crates/verse`; fracture in the pack
compiler.

### D0: Headless cottage rig

Build the Stoop Lane cottage from `layout::lane` placements as a rig in a
`verse-world` test, reusing `wall_of_stone::rig`'s welds, limits, and break
rule. Add breakable joints to the physics crate.

Accept: scripted hits destroy two adjacent south wall sections and the roof
stays up; destroying every section of the west and east walls, which carry
the roof's eaves, drops the roof; a checkpoint restored mid-collapse continues bit for bit; the momentum
ledger balances with the hammer impulse as a named term.

### D1: One cottage, a sledgehammer, pieces that break and topple

The demo. Local Everglade only.

- The maul on the hotbar, its melee ability, attack and damage rolls, and
  floating numbers.
- Cottage wall sections, posts, glass, and the door take damage, show crack
  states, and break into load-time grid chunks that fall and settle.
- The cottage leaves the merged cells for its own batch; moving chunks draw
  through the instanced path; dust and placeholder sounds play.
- Broken pieces remove their footprints; chunks don't collide with the
  player yet. No rebake: accept stale light on the cottage for the demo.

Accept: a capture shows a wall section cracking over two hits and breaking
into chunks that topple outward from the swing. Desktop and phone frame times
are recorded during a break.

**Meteor Swarm in the yard.** The owner asked for a spell that destroys
buildings. `2` on the yard's hotbar aims Meteor Swarm: a pulsing ring of
fire 6 m in radius follows the cursor on the ground, up to 36 m from the
player; a click or tap casts it, and right click, `Esc`, or `2` cancels. It
is a level 9 spell under the [combat model](combat-model.md): 75 of 100
mana, a 90 s cooldown, and a 2.5 s cast bar that moving interrupts. Six
meteors fall through the circle, and each explosion deals the SRD's 20d6
Fire and 20d6 Bludgeoning, rolled once per cast, to every piece within 4 m,
falling off with distance; what breaks is thrown outward at up to 26 m/s
for 1.6 s past the yard's 10 m/s cap, and the support graph brings down
what the blast left unsupported. `R` rebuilds the cottages and refills the
mana. `crates/verse/src/zones/everglade/demolition/meteor.rs` holds the
tuning.

### Everglade's town

The owner asked: "Add destructibility and Meteor Swarm to the current
Everglade, same no-cooldown, no-mana." The yard's rules, kit pieces, chunks,
sledgehammer, and spell now act on Everglade's town
([`demolition/town.rs`](../../crates/verse/src/zones/everglade/demolition/town.rs)),
through the shared [`kit.rs`](../../crates/verse/src/zones/everglade/demolition/kit.rs),
[`hammer.rs`](../../crates/verse/src/zones/everglade/demolition/hammer.rs),
and [`site.rs`](../../crates/verse/src/zones/everglade/demolition/site.rs).

- **Controls.** Key 6 on Everglade's hotbar aims Meteor Swarm (a click or a
  tap casts it, right click or `Esc` cancels) and key 7 swings the
  sledgehammer. Both cost nothing and have no cooldown; the 2.5 s cast bar
  and the targeting circle stay. `R` restores every building.
- **Buildings.** Kit wall sections that meet side by side, at a corner, or
  one over another make one building. Its posts, roof spans, gables,
  chimney, glass, shutters, door frames, and door leaves map onto the
  yard's pieces with a story and a roof span, so upper stories need the
  section under them and a wide roof's inner eaves rest on the south and
  north walls. Generated whole-model buildings, the open pavilion and
  bandshell, the arch, the boards, and the furniture are not kit walls and
  stay whole.
- **The studio.** A building over the workshop hall, the strongroom, or a
  station's standing point is protected: a swing there floats "Protected",
  and a meteor bursts on it without harm.
- **Laziness.** A building stays in the merged static cells until a swing or
  a blast reaches it. It is then raised into the rules as static bodies, and
  only its damaged, loose, or broken pieces leave the static cells, by
  rewriting their placements' index ranges to degenerate triangles
  (`TexturedScene::edits`); their chunks draw in the frame's figure. The
  blockers and roof surfaces of a raised building come from its standing
  pieces.
- **Budgets.** At most four raised buildings and 220 chunks on desktop, and
  two and 96 in a browser or on a phone. A raised building that took no
  damage goes back after 2 s; a damaged one regrows whole after 60 s of rest
  with the player 30 m away.

### D2: Structural collapse and leaning

- Support graph, activation and deactivation, plastic yield, and hinges.
- Roof split into four segments in the pack; gables and chimney as pieces.
- Everglade walking on the capsule controller with moving colliders; rubble
  is walkable; debris strikes characters.
- Local rebake, shadow invalidation, and navigation replanning.

Accept: knocking out one long wall makes the roof sag into the gap and hang
from the ridge; removing a corner makes the adjoining wall lean, hold, then
fall when hit again; the player climbs the rubble; studio seats replan around
it.

### D3: Every destructible building within budget

- Slab fracture and patterns in the compiler; `VTP3`; the reading room and
  pavilion become destructible.
- Broadphase, body pooling, and debris tiers with degradation.
- Studio protection and the safe zone.
- A collapse benchmark recorded on desktop, in a browser on WebGPU and
  WebGL2, and on a phone.

Accept: collapsing two buildings at once stays inside the budget table on
each platform, or the table is revised from measurements.

### D4: Multiplayer

- Physics in the hosted social profile, the shared-caster adapter, reliable
  structural events, gameplay body replication, and the baseline for late
  joiners.

Accept: two players and a spectator on different platforms see the same
pieces break, the same pieces lean, and rubble in the same place; a player who
joins after a collapse sees the settled result; per-viewer traffic during a
collapse is recorded against the 38 KiB/s estimate.

### D5: Persistence and repair

- Building state in instance checkpoints, and the regrowth or repair rule the
  owner chooses.

Accept: a hosted instance restarts with its damage intact; a regrown building
returns to its pinned layout exactly.

Runtime fracture at the impact point, convex hulls, and destructible
furniture are later proposals once D3's measurements show headroom.

## Open questions for the owner

1. Which buildings may players destroy? This document proposes everything
   except the workshop hall, the strongroom, the boards, and studio
   furniture.
2. In a shared instance, may any admitted player destroy buildings, or only
   the instance owner or players with a new host right? Destruction is an
   easy grief.
3. How do buildings come back: regrowth on a timer, player repair, or reset
   on restart?
4. Is the sledgehammer an item anyone can equip, a class or quest reward, or a
   demo tool on the hotbar? Should spells (Fireball, Meteor Swarm,
   Thunderwave) damage buildings too?
5. Should numbers stay SRD-faithful (about two hits per wall section with
   the Wrecking property), or be tuned toward spectacle?
6. Are phones in scope for D1, or is D1 a desktop demo with phones joining at
   D3?
7. Should furniture and props inside buildings break or fall with them, or
   stay intact on the floor?
