# Zone rules and future fifth-edition profiles

Wizard Woods now runs the original Ruins of Atlantis real-time combat schedule
through [`verse-ruins`](../../crates/verse-ruins/). The supported profile
is `ruins.wizard-woods.v1`: continuous monster movement, NPC casting,
projectile collision, mana, cooldowns, damage, and destructible voxel ruins.
The phone hotbar maps to the original three abilities. There are no combat
rounds, movement budgets, or **End turn** button.

This supersedes the earlier `srd-5.1-encounter-v1` demonstration. That new
turn-based implementation did not reproduce the requested source game and is
removed from the current runtime. Earlier verification receipts remain historical
evidence for the earlier build; they do not verify this replacement.

## Current gameplay and its boundary

[The source parity audit](ruins-source-parity.md) records original tuning and
behavior. The player starts with 100 HP and 20 mana. Firebolt costs no mana;
Magic Missile costs 2 and emits three homing darts; Fireball costs 5 and damages
an area. Monsters and NPC casters continue acting while the foreground surface
updates. The host pauses the simulation with its surface lifecycle and returns
to the plaza without requiring a win.

The Ruins is local-only. Its authoritative state is one in-process source ECS
world, not the Nostr plaza's shared pose state. A downloaded asset is data; it
cannot replace the compiled game code. This profile is the source game's tuning,
not a complete implementation of Dungeons & Dragons Fifth Edition. Its original
SRD 5.2.1 references do not change that claim.

## No dice: the combat model

There is no dice-based fifth-edition profile, and none is planned (owner,
2026-10-04). Every zone uses the real-time [combat model](combat-model.md):
fixed damage, mana and cooldowns, timed debuffs, and no attack rolls or saving
throws. SRD abilities are translated into that model, never run by tabletop
procedure.

## Creator-selected profiles

The intended authored-world contract separates these choices:

| Field | Purpose |
| --- | --- |
| Scene definition | Exact geometry, bounds, arrivals, portals, and object placement. |
| Asset manifest | Hashes, byte lengths, formats, decode budgets, provenance, and license notices. Assets load only when the destination is admitted. |
| Presentation profile | World-specific colors, lighting, fog, and effects. The global plaza keeps its amber Coder appearance. |
| Physics profile | Units, reference frame, collision and motion semantics, numerical limits, and supported integration behavior. |
| Rules profile | Exact supported rules ID, revision, coverage, configuration, and fixtures. The current Ruins ID is `ruins.wizard-woods.v1`. |
| Authority profile | Local-only behavior or an explicitly admitted shared authority, with actor ownership and accepted command semantics. |

A creator may eventually select a supported rules profile without adopting the
Ruins' appearance. A scene may also select ordinary exploration without a
combat profile. Unknown required profiles must refuse entry or offer an
explicitly different supported experience; silently interpreting an unknown
ID as default physics changes the world the creator declared.

These are closed host-supported implementations. Downloading a signed world
manifest does not authorize native plugins, scripts, shader programs, wallet
operations, or host commands. An exact profile identity must remain attached to
saved state and replay records. A moving version label is not a simulation pin.

NIP-MV provides world-scoped presence and entity identity. It does not make this
local game authoritative or synchronize its HP, mana, or projectiles. A future
shared encounter needs a reviewed command and result protocol with an exact
rules pin, actor grants, sequence and replay checks, disconnect recovery, and
an explicit fairness model. A signed avatar pose proves publication, not a
legal move, a hit, an item transfer, or a result accepted by other players.

## Lagrange 1: physics without combat

[Lagrange 1](lagrange-1.md) uses the same zone lifecycle with a different
physics profile and no combat rules. Its construction sandbox has three
actions: grab, carry, and latch a part into the keel jig. The orbit uses the
Sun–Earth circular restricted three-body problem with named constants, an
RK4 integrator, and linear unstable-mode station-keeping. Local motion uses the
linearized L1 field, torque-free rigid bodies, and a cold-gas pack obeying the
rocket equation. [The L1 page](lagrange-1.md#approximations) lists every
approximation.

Construction is local and resets on each visit. Save/load, component
revisions, ownership, undo, and collaborative assembly remain future work;
shared editing needs admitted edit authority, conflict behavior, and
authoritative snapshots before a public construction world.

## Everglade: exploration only

[Everglade](everglade.md) uses the same zone lifecycle with the plaza's walking
rules on a generated heightfield and no combat or construction. Its station
markers are fixed coordinates for the Agent Studio; they open nothing yet.
It is local-only and keeps no state between visits.

## Sources and attribution

The Ruins's source license, original notices, exact revision, and modification
ledger are retained in [`crates/verse-ruins`](../../crates/verse-ruins/).
The [source audit](ruins-source-parity.md) identifies the active mechanics
rather than inferring them from the source's SRD files or design documents.

The earlier SRD 5.1 demo's [attribution](SRD-5.1-NOTICE.md) remains with its
historical documentation and verification records. It does not describe the
current Ruins' combat engine. Any future fifth-edition profile must pin its
chosen SRD edition, preserve its attribution, and state its tested coverage.
