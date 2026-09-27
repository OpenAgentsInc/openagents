# Zone rules and the fifth-edition encounter

The Atlantis forest introduces selectable rules independently of its scene,
assets, palette, and movement renderer. Its first executable rules profile is
`srd-5.1-encounter-v1`: a local, turn-based wizard-versus-zombie encounter based
on a bounded subset of the **System Reference Document 5.1**. This is the rules
work for [#9727](https://github.com/OpenAgentsInc/openagents/issues/9727).

This profile does not implement the full Dungeons & Dragons game. The
[official SRD index](https://www.dndbeyond.com/srd) distinguishes the original
fifth-edition SRD 5.1 from the revised SRD 5.2.1. The Ruins of Atlantis checkout
contains 5.2.1 documents; they are design reference material, not the rules
source for this profile. The implementation is new Rust code, not a copy of
that repository's real-time combat simulator.

## Play the encounter

Enter the forest, then select **Encounter**. The encounter places the
wizard and zombie in a clear arena and rolls initiative. If the zombie wins,
it takes its first turn before the wizard. Ties go to the player: this is the
demo's explicit choice where the SRD lets the GM decide.

On your turn, move within the remaining budget, select **Fire Bolt**, then select
**End turn** when ready. You can also end a turn without casting. Casting spends
one action; it does not automatically end your turn. **End turn** resolves one
zombie turn and returns control to the wizard. Rendering frames, waiting, and
backgrounding never make the zombie attack. **Reset** starts a fresh
encounter with new initiative rolls. Leaving the zone discards the encounter.

The player can leave the encounter through the zone's return control. Access to
the rest of the app does not depend on winning a fight.

| Participant | Statistics and supported actions |
| --- | --- |
| Wizard | Level 1; 12 HP; AC 12; initiative +2; 30-foot movement budget; Fire Bolt attack +5. The HP, AC, ability modifiers, and spell selection are authored demo values, not a generated or complete character sheet. |
| Zombie | 22 HP; AC 8; initiative −2; speed 20 feet; Slam +3, reach 5 feet, damage 1d6 + 1; Constitution +3 for Undead Fortitude. These values come from the SRD 5.1 Zombie stat block. |

Fire Bolt has a 120-foot limit and deals 1d10 fire damage. A critical hit rolls
2d10. The attack has disadvantage when the living zombie is within 5 feet.
On its turn, the zombie approaches by at most 20 feet and uses Slam if the
wizard is in reach. A critical Slam rolls 2d6 + 1; the flat modifier is added
once. The forest uses meters, with one foot equal to 0.3048 meters.

A noncritical, nonradiant hit that reduces the zombie to zero HP triggers
Undead Fortitude: a Constitution save against DC 5 plus the damage taken.
A successful save leaves it at 1 HP. Critical and radiant damage bypass this
save. Fire Bolt is the only player attack in this demo; radiant damage is a
tested kernel case, not an additional selectable spell.

## Rule boundaries

The independent kernel supports these distinctions:

- An attack roll's natural 1 misses and natural 20 hits, regardless of the
  total. The latter is a critical hit.
- An ordinary ability check or saving throw compares its modified total with
  its target. Natural 1 and natural 20 have no general attack-style override.
  Death saving throws have special rules and are not implemented here.
- Advantage chooses the higher of two d20s; disadvantage chooses the lower.
  Any simultaneous source of both cancels to a single roll. Sources do not
  accumulate extra dice.
- Critical damage doubles the number of damage dice, not the flat modifier.
- Each wizard turn admits one action and a 30-foot movement budget. Actual
  movement segments spend that budget; a route destination cannot bypass it.

This encounter deliberately omits character creation, leveling, spell slots,
other spells, equipment, bonus actions, reactions, opportunity attacks, Dash,
Disengage, cover, difficult terrain, surprise, conditions, resistances, rests,
death saves, loot, and XP awards. The zombie has no opportunity attack when the
player moves away. At zero wizard HP, the demo ends; it does not simulate death
or stabilization. This profile must not claim full fifth-edition compatibility.
Adding those behaviors changes the declared coverage and requires a new profile
revision with its own fixtures.

## Implementation and integration

[`zones/rules.rs`](../../crates/verse/src/zones/rules.rs) owns no renderer,
transport, wall clock, asset loader, credentials, or platform objects. Its
`Encounter` reducer accepts **Cast**, **End turn**, **Reset**, and bounded
movement through typed methods. `snapshot()` returns actor positions and HP,
initiative, current round and turn, action availability, remaining movement,
the most recent attack, a concise notice, and a revision.

The host places those encounter coordinates relative to a clear local arena.
While the encounter is active, the wizard stays within its 10-meter radius;
movement beyond that boundary refuses without spending the movement budget.
The host stops enforcing encounter movement after the fight finishes.
It checks static collision before admitting each movement segment. The rules
also reject movement through the zombie's occupied space. A host with
intervening geometry must use `cast_with_visibility` after checking the shared
line of sight. The convenience `cast` method assumes the declared clear arena.
The zombie's straight approach also requires that clear arena; it is not a
general pathfinder for authored combat maps.

Every die is supplied through `Dice` and checked against its declared sides.
`SeededDice` provides reproducible local demo rolls with rejection sampling.
It is not a cryptographic or multiplayer fairness mechanism. Tests supply exact
faces and check consumption, which makes initiative and damage reproducible.

Rejected commands do not spend actions, movement, or HP. A failed die result
does not partially apply a cast, NPC turn, or reset. The external dice source
may already have consumed values before returning an invalid one; the reducer
does not rewind an injected source or retry a failed command. Inputs must be
finite and bounded. The profile caps encounters at 1,000 rounds and keeps one
attack result rather than an unbounded event history.

Tests in [`zones/rules/tests.rs`](../../crates/verse/src/zones/rules/tests.rs)
cover initiative order, nonattack natural rolls, advantage, critical dice,
Undead Fortitude, range, close-range disadvantage, action economy, movement,
reset, bounded NPC turns, and transaction failures. These fixtures establish
the subset above; they do not establish a complete SRD implementation.

## Creator-selected profiles

The intended authored-world contract separates these choices:

| Field | Purpose |
| --- | --- |
| Scene definition | Exact geometry, bounds, arrivals, portals, and object placement. |
| Asset manifest | Hashes, byte lengths, formats, decode budgets, provenance, and license notices. Assets load only when the destination is admitted. |
| Presentation profile | World-specific colors, lighting, fog, and effects. The global plaza keeps its amber Coder appearance. |
| Physics profile | Units, reference frame, collision and motion semantics, numerical limits, and supported integration behavior. |
| Rules profile | Exact supported rules ID, revision, coverage, configuration, and fixtures. The first encounter ID is `srd-5.1-encounter-v1`. |
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
local encounter authoritative or synchronize its dice, HP, or turns. A future
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

The source is the [official SRD 5.1 Creative Commons PDF](https://media.dndbeyond.com/compendium-images/srd/5.1/SRD_CC_v5.1.pdf),
checked on September 27, 2026. Its SHA-256 is
`2504d2a0abb0a4d491a939be4f17910a2dde0312570ab8d208080225ccf0a1f0`.
Relevant printed pages are 77–78 (d20 tests and advantage), 90–91 (initiative,
turns, and movement), 94–97 (attacks, range, critical hits, and damage), 144
(Fire Bolt), and 356–357 (Zombie).

Keep [the SRD attribution](SRD-5.1-NOTICE.md) with distributed versions that
include these adapted rules. The implementation and demo omissions are
modifications; no SRD artwork or branding is included. Model and texture
licenses are separate from these rules and from the reference repository's
code license.
