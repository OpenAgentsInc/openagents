# Zone rules and future fifth-edition profiles

Each loaded zone selects a closed, host-supported rules profile. The Ruins
zone, which ran the original Ruins of Atlantis Wizard Woods real-time combat
under the `ruins.wizard-woods.v1` profile, was removed on 2026-10-05 with the
retained `verse-ruins` source. Combat now lives in the zones that follow the
[combat model](combat-model.md), such as the [druid demo](druid-demo.md).

## The combat model

Every zone uses the [combat model](combat-model.md) (owner, 2026-10-04):
MMO play in real time with mana, cooldowns, and timed debuffs, and SRD dice,
attack rolls, and saving throws rolled behind the scenes. There is no
separate turn-based fifth-edition profile.

## Creator-selected profiles

The intended authored-world contract separates these choices:

| Field | Purpose |
| --- | --- |
| Scene definition | Exact geometry, bounds, arrivals, portals, and object placement. |
| Asset manifest | Hashes, byte lengths, formats, decode budgets, provenance, and license notices. Assets load only when the destination is admitted. |
| Presentation profile | World-specific colors, lighting, fog, and effects. The global plaza keeps its amber Coder appearance. |
| Physics profile | Units, reference frame, collision and motion semantics, numerical limits, and supported integration behavior. |
| Rules profile | Exact supported rules ID, revision, coverage, configuration, and fixtures. |
| Authority profile | Local-only behavior or an explicitly admitted shared authority, with actor ownership and accepted command semantics. |

A creator may eventually select a supported rules profile without adopting
another zone's appearance. A scene may also select ordinary exploration without a
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

The removed Ruins zone's [source audit](ruins-source-parity.md) is retained as
history; its retained source, license, and modification ledger remain in the
repository history before 2026-10-05.

The earlier SRD 5.1 demo's [attribution](SRD-5.1-NOTICE.md) remains with its
historical documentation and verification records. Any future fifth-edition
profile must pin its chosen SRD edition, preserve its attribution, and state
its tested coverage.
