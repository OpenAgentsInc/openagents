# Zone rules and future fifth-edition profiles

Wizard Woods now runs the original Ruins of Atlantis real-time combat schedule
through [`verse-atlantis`](../../crates/verse-atlantis/). The supported profile
is `atlantis.wizard-woods.v1`: continuous monster movement, NPC casting,
projectile collision, mana, cooldowns, damage, and destructible voxel ruins.
The phone hotbar maps to the original three abilities. There are no combat
rounds, movement budgets, or **End turn** button.

This supersedes the earlier `srd-5.1-encounter-v1` demonstration. That new
turn-based implementation did not reproduce the requested source game and is
removed from the current runtime. Earlier verification receipts remain historical
evidence for the earlier build; they do not verify this replacement.

## Current gameplay and its boundary

[The source parity audit](atlantis-source-parity.md) records original tuning and
behavior. The player starts with 100 HP and 20 mana. Firebolt costs no mana;
Magic Missile costs 2 and emits three homing darts; Fireball costs 5 and damages
an area. Monsters and NPC casters continue acting while the foreground surface
updates. The host pauses the simulation with its surface lifecycle and returns
to the plaza without requiring a win.

The forest is local-only. Its authoritative state is one in-process source ECS
world, not the Nostr plaza's shared pose state. A downloaded asset is data; it
cannot replace the compiled game code. This profile is the source game's tuning,
not a complete implementation of Dungeons & Dragons Fifth Edition. Its original
SRD 5.2.1 references do not change that claim.

## Fifth edition remains a separate planned profile

A creator-selectable fifth-edition ruleset is still intended. It should be added
as an explicit, independently versioned alternative with a documented SRD
edition and executable coverage fixtures. Do not silently label the current
real-time fireball simulator as fifth-edition conformance, and do not replace
an authored world's rules with a different profile under the same ID.

Before publishing a profile, specify character statistics, initiative and
turns, action economy, movement and range, attacks and saves, spell resources,
conditions, death, and unsupported actions. Tests must distinguish natural
attack rolls from ordinary checks and saves, verify critical dice and advantage,
and reject unsupported actions without partially mutating state. Full character
creation, a complete spell catalog, equipment, rests, and multiplayer authority
remain separate work. A limited subset must say exactly what it supports.

## Creator-selected profiles

The intended authored-world contract separates these choices:

| Field | Purpose |
| --- | --- |
| Scene definition | Exact geometry, bounds, arrivals, portals, and object placement. |
| Asset manifest | Hashes, byte lengths, formats, decode budgets, provenance, and license notices. Assets load only when the destination is admitted. |
| Presentation profile | World-specific colors, lighting, fog, and effects. The global plaza keeps its amber Coder appearance. |
| Physics profile | Units, reference frame, collision and motion semantics, numerical limits, and supported integration behavior. |
| Rules profile | Exact supported rules ID, revision, coverage, configuration, and fixtures. The current forest ID is `atlantis.wizard-woods.v1`. |
| Authority profile | Local-only behavior or an explicitly admitted shared authority, with actor ownership and accepted command semantics. |

A creator may eventually select a supported rules profile without adopting the
forest's appearance. A scene may also select ordinary exploration without a
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

## L1 construction station: design only

The future Lagrange-point construction station uses the same zone lifecycle
and on-demand assets, with a different physics profile and no required combat
rules. This change does not build that station or its simulation.

Its initial design must select which bodies define L1, a reference epoch,
coordinate frame, and unit system. A generic zero-gravity scene is not an L1
model. A later orbital profile should state its gravitational approximation,
integrator, fixed time step, numerical precision, and station-keeping behavior.
The displayed educational model must identify its approximations.

Construction adds rigid bodies with mass, center of mass, inertia, joints,
attachment points, and bounded collision shapes. A ship assembly needs a
persistent identity, component revisions, ownership, and saved constraints.
Dragging a beam, attaching it, applying thrust, and undoing a change are typed
operations. Faster construction playback and faster physics time are distinct
controls; neither should change solver stability without an explicit limit.

Start future work with a local assembly sandbox and validated conservation and
constraint tests. Then add save/load and a declared orbital approximation.
Shared visits and collaborative assembly follow only with admitted edit
authority, conflict behavior, and authoritative snapshots. This lets a parent
and child learn from the same scene before it becomes a public construction
world, without claiming realism from appearance alone.

## Sources and attribution

The forest's source license, original notices, exact revision, and modification
ledger are retained in [`crates/verse-atlantis`](../../crates/verse-atlantis/).
The [source audit](atlantis-source-parity.md) identifies the active mechanics
rather than inferring them from the source's SRD files or design documents.

The earlier SRD 5.1 demo's [attribution](SRD-5.1-NOTICE.md) remains with its
historical documentation and verification records. It does not describe the
current forest's combat engine. Any future fifth-edition profile must pin its
chosen SRD edition, preserve its attribution, and state its tested coverage.
