# Wizard Woods source audit and parity

Issue [#9730](https://github.com/OpenAgentsInc/openagents/issues/9730) replaces the
newly invented turn-based forest demo with the retained Ruins of Atlantis
real-time game. The relevant source revision is
`daeb5d0895270159ec8c18341b4adb84bf7a4346`. This audit describes code inspected on
September 27, 2026, rather than inferring executable behavior from design notes.

## Which original game path

The source has several engines. Its latest native root executable starts the
separate Bevy dragon-and-pumpkin slice. The requested wizard, zombie, and fireball
world is the retained `platform_winit` Wizard Woods path, also reached by the
web entry point. That path runs `server_core::zones::boot_with_zone` and
`ServerState::step_authoritative`.

The separate `sim_core` SRD engine is not the Wizard Woods real-time runtime.
The files under `ecs/schedule/schedule_stages/` are scaffolding; the executable
schedule is in `server_core/src/ecs/schedule.rs`. Older helpers in
`systems/npc.rs` have different tuning and must not replace that schedule.

The port retains ten source crates: `server_core`, the original `client_core`
movement controller, and eight shared dependencies: `core_materials`,
`core_units`, `voxel_proxy`, `data_runtime`, `voxel_mesh`, `ecs_core`,
`collision_static`, and `net_core`. The portable adapter is
[`verse-atlantis`](../../crates/verse-atlantis/). It supplies host inputs and
bounded snapshots while the retained ECS owns combat. It does not import the
source's native window, web host, private service, or authentication system.

The source hashes are recorded in the crate's
[provenance manifest](../../crates/verse-atlantis/provenance.json).
[`LICENSE`](../../crates/verse-atlantis/LICENSE) and
[`NOTICE`](../../crates/verse-atlantis/NOTICE) preserve Apache-2.0 and the
source's notices. Changed files and platform adaptations must remain listed in
the crate's modification record. The original notice names SRD 5.2.1; this does
not make the real-time schedule a complete fifth-edition rules implementation.

## Original scene, without invented placements

The source terrain loader prefers workspace `data/zones/wizard_woods/` over
its older crate-local copy. The exact active JSON is retained under
[`data/wizard_woods`](../../crates/verse-atlantis/data/wizard_woods/) with
[file hashes](../../crates/verse-atlantis/data/wizard_woods/provenance.json).

| Source value | Meaning |
| --- | --- |
| Grid size 129 | 16,641 heights, ordered as `z * size + x`. |
| Extent 150 meters | Half-extent: X and Z each span −150 through +150 meters. |
| Seed 1337 | Recorded generation seed; loading uses the retained heights. |
| Center height −3.3028286 meters | Initial player ground height at X=0, Z=0. |
| Heights −3.7887688 through 2.0772564 meters | Actual snapshot range. |
| Tree count 0 | The active manifest and tree JSON both specify no trees. |
| Packed static instances and colliders 0 | The source pack does not add hidden tree placements. |
| Initial time fraction 0.95, paused | Original sky configuration, not an assertion that Verse reproduces its sky renderer. |

An older crate-local tree JSON contains 350 copies of the same transform. It is
not the active workspace scene, and copying it would reproduce a placement bug
rather than a forest. The previous Verse ring of trees and flat arena were new
authoring. They must not be described as original terrain.

[`scene::Terrain`](../../crates/verse-atlantis/src/scene.rs) adapts the source
bilinear height sampler and finite-difference normals. The JSON is parsed once
from compile-time data, without filesystem discovery. The caller can retrieve
vertices and normals for the exact grid. Finite out-of-bounds samples clamp to
the grid edge; nonfinite coordinates use the center sample. Those defensive
input rules are explicit adapter behavior. Three focused tests check source
values, all 16,641 mesh vertices against the sampler, normals, edge handling,
and a bilinear cell midpoint.

The source zone boot creates these actors. Spawn-safety functions can move a
requested position to avoid the player or another actor.

| Actor | Requested source placement and initial state |
| --- | --- |
| Player wizard | Center, facing +Z; 100 HP, 20 mana, 1 mana/second regeneration, radius 0.7 meters. |
| Eight zombies | Radius 15 meters, 20 HP each. |
| Twelve zombies | Radius 30 meters, 25 HP each. |
| Fifteen zombies | Radius 45 meters, 30 HP each. |
| Four NPC wizards | Radius 8 meters, 100 HP, 30 mana, 0.5 mana/second regeneration. |
| Nivita | Requested at the origin, then pushed outside the player safety bubble; 225 HP from the midpoint of the retained 200–250 range. |
| Death Knight | Requested at X=60, Z=0; 400 HP, 40 mana, 0.3 mana/second regeneration. |

NPC simulation transforms initially use Y=0.6. The original renderer projects
NPC models onto terrain independently; that placeholder is not the model's
world-space ground height. Player movement and terrain projection must use the
same retained heightfield on every host.

The original destructible ruin is a voxel shell at X=[−8,8], Z=[4,20], Y=0.
It has no floor or roof and has two-voxel-thick side walls. Its 32×16×32 grid
uses 0.5-meter voxels, so occupied geometry extends 8 meters high, while its
original broad-phase AABB declares a 6-meter maximum. That inconsistency is in
the retained source. Render original mesh deltas rather than hiding it behind
an unrelated decorative cube. Carving and collider refresh use the original
bounded remesh schedule.

## Original controls and abilities

The active renderer binds keys **1**, **2**, and **3** to Firebolt, Magic Missile,
and Fireball. Q and E strafe. The generic LMB/Q/E/R action map elsewhere in the
source is not this runtime's active ability mapping. The mobile hotbar maps
those three actions into the shared Rust command path; it does not add a second
mobile combat implementation.

Source key input aims along the character's facing: `[sin(yaw), 0, cos(yaw)]`.
The command origin is terrain height +1.4 meters, offset 0.25 meters forward;
projectile ingestion applies a further 0.35-meter forward offset. The source
also has animation-driven command emission and client-only effects. The new
host sends one request per admitted input and renders the authoritative
snapshot instead of duplicating that legacy command path.

| Ability | Mana | Spell cooldown | Global cooldown | Original projectile behavior |
| --- | --- | --- | --- | --- |
| Firebolt | 0 | 0.30 s | 0.30 s | Speed 40 m/s, lifetime 1.5 s, direct damage 10, arming delay 0.08 s. |
| Magic Missile | 2 | 1.50 s | 0.30 s | Three darts, speed 28 m/s, lifetime 1 s, damage 7 each, arming delay 0.08 s, initial spread −8°/0°/+8°. |
| Fireball | 5 | 2.00 s | 0.50 s | Speed 30 m/s, lifetime 1.5 s, damage 28, nominal area radius 6 m, arming delay 0.10 s, voxel carve radius 2 m. |

Projectile tuning comes from the retained `projectiles.toml` with original
fallbacks. The file has no `MagicMissile` entry; its source fallback supplies
those values. The source has no `archetypes.toml`, so actor tuning uses the
original compiled defaults rather than an invented file.

Magic Missile chooses up to three distinct nearby hostile targets and retains
the source homing behavior, including turn rate 3.5 and reacquisition. Hits apply
a 0.7 movement multiplier for 2 seconds. Fireball can explode on contact,
proximity after 0.20 seconds, or lifetime expiry. Area tests add 0.25 meters and
the target's collision radius to the nominal radius. Collisions and damage
use XZ geometry, not a complete three-dimensional projectile physics solver.

## Original movement and host input mapping

`verse-atlantis::Controller` and `MovementInput` re-export the original
`client_core::controller::PlayerController` and input type. The forest retains
one controller across frames, including its jump velocity and grounded state;
it does not reconstruct a controller from each rendered pose.

| Source controller value | Value |
| --- | --- |
| Ordinary forward run speed | 6.4008 m/s |
| Walk-mode speed | 2.2860 m/s; the source mode exists, but the phone does not expose a walk toggle. |
| Backpedal speed | 4.1148 m/s |
| Sprint multiplier | 1.3; retained in the controller, without adding a phone sprint toggle. |
| Keyboard turn rate | 180 degrees/second |
| Gravity | 9.81 m/s² |
| Initial jump velocity | 4.6 m/s |

The shared host maps touch movement, keyboard movement, and double-tap jump
into those inputs. A/D with right-button mouselook becomes strafe, matching the
source frontend's input resolution. Without mouselook, A/D turns. The source
controller's forward basis is `[sin(yaw), 0, cos(yaw)]`. Ground queries before
and after movement use the retained terrain. The host bounds the player to
±149.55 meters inside the 150-meter half-extent. The amber plaza continues to
use its own controller and flat ground.

## Continuous simulation and AI

The source platform passes elapsed frame time clamped to [0,0.1] seconds to the
schedule. The schedule is real-time; it has no encounter rounds or **End turn**
command. The adapter and native surface lifecycle bound elapsed input and do
not simulate a large background interval on resume.

The schedule applies inputs, cooldowns and regeneration, NPC cast decisions,
cast admission, projectile creation, spatial indexing, timed effects, hostile
movement, separation, melee, homing, projectile integration and collisions,
voxel destruction, area damage, faction changes, damage, and cleanup. Ordering
matters: changing it can alter a hit or a cooldown without changing any tuning.

Zombies move at 2 meters/second, acquire within 25 meters, and use 5-damage
melee with a 0.6-second cooldown. The Death Knight moves at 2.2 meters/second,
acquires within 40 meters, and uses 18-damage melee with a 0.9-second cooldown.
Nivita's active spawn code sets 2.6 meters/second, range 35 meters, and 12-damage
melee with a 0.8-second cooldown. Her much richer configuration is retained
metadata; the schedule does not thereby implement every listed spell or
legendary action. Ordinary NPC wizards have no movement component and cast
from their positions.

NPC casters choose a nearest hostile. They favor Fireball for a target cluster
at 12–25 meters, Magic Missile within 10 meters, and Firebolt farther away,
restricted by their spellbook. The chosen spell still passes mana and cooldown
gates. Death Knight knows Fireball and Magic Missile; ordinary wizards know
all three. The AI does not currently perform a line-of-sight query or navigate
around arbitrary authored obstacles.

## Source defects and declared differences

Reuse does not justify concealing inherited defects or silently claiming exact
behavior after repairs. The inspected source contains these concrete issues:

- Melee's target list includes the Wizards faction but omits the player faction,
  although hostile movement chases both. A correction that lets monsters injure
  the player must be recorded and tested explicitly.
- Cast admission starts cooldown timers before checking sufficient mana. A
  rejected low-mana request can therefore consume cooldown time.
- Burning computes `floor(damage_per_second * dt)` without a fractional
  accumulator. With burn DPS 6 and normal dt at most 0.1 seconds, each tick
  applies zero burn damage. Initial Fireball damage still applies.
- Fireball assigns the burning condition to nearby actors without the direct
  damage hostility filter. This matters if the burn accumulator is repaired.
- NPC spell selection chooses one spell before checking its cooldown and mana;
  a blocked first choice does not automatically try another ready spell.
- The original renderer's R handler resets presentation state, but the retained
  command protocol has no authoritative respawn operation. A local restart
  should rebuild the simulation explicitly, not revive only its displayed HP.
- NPC visuals are projected onto terrain, but the source server keeps their
  placeholder Y=0.6 and replicates NPC projectile heights unchanged. Actor
  collision is XZ-only, so visible missile height and a hit can disagree.
- The chunk mesher includes the voxel-grid origin in vertices, then replication
  applies the same origin again. For the default ruin, that adds an extra
  [−8,0,4] translation. Its rendered surface and announced collision volume can
  disagree until that coordinate-space defect is corrected explicitly.
- Several retained tests refer to old private schedule helpers and immediate
  despawning. Their existence does not mean they still compile or pass against
  the current schedule. Focused current tests must state exactly what ran.

At zero HP, the current source schedules the actor for removal after 2 seconds.
The host must disable player actions while dead and always permit returning to
the plaza. Reentering the zone starts a new local session. This is not persistent
character progression or a multiplayer resurrection protocol.

## Implemented integration corrections

These changes are in the portable adapter or Verse host. They do not rewrite
the retained source ECS schedule:

- `Simulation::cast` refuses a missing or defeated player before queuing a
  request. The original schedule does not check whether its caster is alive;
  this guard prevents a stale UI action from casting during the two-second
  death cleanup interval.
- At zero player HP, the forest stops player movement, removes the living
  player model, disables abilities, and displays **Defeated · return to Plaza**.
  The return control remains available. Returning and entering again creates a
  fresh simulation; there is no invented authoritative respawn message.
- Forest camera clearance uses the heightfield at the camera's XZ position
  plus 0.4 meters. It no longer inherits the plaza's absolute Y=0.4 floor,
  which was wrong over the source's negative terrain heights. Applying
  clearance preserves the requested look angle.
- The forest builds its dynamic geometry once per simulation update and
  retains that mesh for rendering and portal depth checks. Portal checks no
  longer repeatedly transform the full actor population. The renderer still
  copies/uploads dynamic geometry; this is not an instancing claim or a
  measured physical-device frame-rate claim.
- The shared input adapter explicitly maps A/D with mouselook to strafe.
  Passing those keys only as turning inputs would cause the source controller
  to ignore them while mouselook is active.

At initial population, actor geometry alone contains about 538,593 vertices,
or 14.4 MiB with Verse's 28-byte vertex representation. Source ruins and effects
add to that amount. The retained cache removes repeated transformation work
from one frame's portal and HUD queries, while the existing 32-MiB dynamic GPU
buffer limit still applies. Release observations must establish actual native
behavior rather than infer it from this byte count.

The integration tests cover simultaneous movement and a hotbar cast, source
strafe mapping, negative-terrain camera clearance, defeated-player movement,
and return. The adapter's source-height projectile test verifies that
its XZ collision path can damage an actor with the original Y=0.6 placeholder.
Native build and device results belong in the release verification records.

## Verification and remaining fidelity work

Source inclusion is stronger than rewriting similar constants, but it does not
prove every adapter is correct. Keep verification at three levels:

1. **Source identity:** compare the retained source and config hashes, enumerate
   every changed file, preserve notices, and identify any corrected source bug.
2. **Simulation conformance:** compare boot actor counts and tuning; test a
   queued cast through the actual schedule; verify moving enemies, projectile
   damage, cooldown rejection and recovery, mana and regeneration, death,
   hostile target selection, and a Fireball carving the original ruin.
3. **Host behavior:** enter through the actual portal, move and cast with the
   bottom GPU hotbar, observe advancing simulation and enemy positions, and
   check durable cast/projectile/hit counters. A screenshot of a static zombie
   is not evidence that it chases or takes damage. Return and reenter, cancel
   loading, and exercise background/resume.

The tests should record known source quirks separately from desired fixes. Do
not turn a new gameplay expectation into a passing “parity” assertion by
changing the expected source result. A source bug fix needs its own before/after
case and an update to the modification ledger.

The mobile renderer still uses the reviewed geometry pack's sampled poses and
vertex colors. It does not reproduce every original shader, texture, skinning
transition, sky effect, or actor-specific model. Those are presentation limits,
not permission to substitute turn-based rules for the original combat.
The geometry pack's [asset-specific provenance limits](../../assets/verse/forest/README.md#source-notices-and-provenance-limits)
remain unresolved and distinct from the source code's Apache license.

Nostr currently connects the plaza, not this local combat world. Shared forest
combat needs a separately admitted authority and command protocol; a signed
avatar pose is not a verified hit. Creator-selected fifth-edition rules and
L1 construction physics remain [separate planned profiles](zone-rules.md).
