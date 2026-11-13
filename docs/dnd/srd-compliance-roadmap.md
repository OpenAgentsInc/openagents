# SRD 5.2.1 compliance roadmap

Status: reordered by owner direction, 2026-11-08. The governing specification is
[README.md](README.md). Full compliance requires complete source content,
faithful execution of every mechanical rule, and independent evidence for both.
The existing combat engine is a foundation, not a qualified implementation of
the entire SRD. World-specific changes remain supported through declared profiles.

## Active delivery Everglade levels 1 to 6

The current epic is [#13030](https://github.com/OpenAgentsInc/openagents/issues/13030).
Its bounded player-facing slice supersedes the full-SRD ordering below. The
339-description/875-class-entry source catalog at `2c3ce41059` is available;
the existing executor supports only a subset. Catalog presence does not make
every spell castable.

1. **Combat foundations now:** [casting #13038](https://github.com/OpenAgentsInc/openagents/issues/13038),
   [conditions/damage/death #13039](https://github.com/OpenAgentsInc/openagents/issues/13039),
   [actions/movement/reactions #13040](https://github.com/OpenAgentsInc/openagents/issues/13040),
   with [transactional capacity #13041](https://github.com/OpenAgentsInc/openagents/issues/13041).
   Integrate checks with Experimental/Everglade II as each change lands.
2. **Playable levels 1–6:** [spells #13043](https://github.com/OpenAgentsInc/openagents/issues/13043)
   is limited to cantrips and spells through 3rd level for the 12 class loadouts;
   [features #13044](https://github.com/OpenAgentsInc/openagents/issues/13044)
   covers class/subclass features through level 6. Match the level selector and
   per-spell sound work. Pull in only needed origin-feat/equipment/grant data.
   [Profiles #13045](https://github.com/OpenAgentsInc/openagents/issues/13045)
   supplies only the timing/override boundary these worlds need.
3. **Used creatures:** [#13034](https://github.com/OpenAgentsInc/openagents/issues/13034)
   inventories Everglade's actual bestiary/dungeon monsters, roughly CR 5 and
   below. Higher-CR exceptions require a named existing consumer; the full
   monster catalog is not a prerequisite.
4. **Everglade migration immediately next:** [#13046](https://github.com/OpenAgentsInc/openagents/issues/13046)
   moves Experimental/Everglade II onto the shared slice. It does not wait for
   old milestone 7, all worlds, or Hold the Stair.

Close the epic after its active issues and world acceptance are met. Existing
higher-level play is not silently removed, but qualifying levels 7–20, spell
levels 4–9 and the complete SRD is outside this delivery.

## Lean verification for the active slice

Owner direction, 2026-11-08: zero-survivor mutation gates apply to the **core
rules engine and core admission/resource/timing policies**. Do not mark mutants
equivalent to get those core gates green. Catalogs, import tooling, presentation
and world integration use focused source/generation/boundary/interaction/native
or headless-muted checks appropriate to the change; a blanket mutation gate or
full qualification framework is not required for every non-core piece. Keep
commands under 15 minutes and reuse unchanged passing evidence by commit.
This replaces the earlier blanket mutation requirement for this SRD work.

## Parked follow-ups

These issues remain open outside the active epic's acceptance; parking is not
completion. Full class source import [#13031](https://github.com/OpenAgentsInc/openagents/issues/13031)
is paused except for a needed narrow source prerequisite. Full origins/feats
[#13032](https://github.com/OpenAgentsInc/openagents/issues/13032), equipment
[#13033](https://github.com/OpenAgentsInc/openagents/issues/13033), magic items
[#13035](https://github.com/OpenAgentsInc/openagents/issues/13035), toolbox
[#13036](https://github.com/OpenAgentsInc/openagents/issues/13036), whole-SRD
traceability [#13037](https://github.com/OpenAgentsInc/openagents/issues/13037),
Polymorph/Shapechange [#13042](https://github.com/OpenAgentsInc/openagents/issues/13042),
general voice/agent/presentation interfaces
[#13047](https://github.com/OpenAgentsInc/openagents/issues/13047), maker authoring
[#13048](https://github.com/OpenAgentsInc/openagents/issues/13048) and full qualification
[#13049](https://github.com/OpenAgentsInc/openagents/issues/13049) are parked.
Multiclassing, full creation UI and cross-world progression are also nonblocking.
Wild Shape needed by a class through level 6 belongs to the active feature slice;
it does not require completing 4th-level Polymorph first.

## Long term reference

The eight milestones below describe the eventual full system. Their original
numbering is retained for existing documentation; they are not the active
sequence or a requirement to finish all catalogs before Everglade can ship.

## Scope and completion criteria

Cover every chapter listed in the specification: Playing the Game, Rules
Glossary, Character Creation, Character Origins, Classes, Feats, Equipment,
Spells, Gameplay Toolbox, Magic Items, Monsters and Animals. Include the
specification's time adapters, world profiles, presentation and maker interfaces.
Content presence, executable mechanics, and verification are separate states.
An empty program or an approximation cannot qualify as an implemented spell.
Only genuinely free-form text may use the narrative handoff.

Completion means: a pinned source and reconciled inventories; all mechanical
clauses mapped to code and independent passing fixtures; no unreported world
deviations; no silent loss of accepted effects; and reproducible per-world and
whole-system reports. Passing selected probes never implies whole-SRD compliance.

## Milestone 1 Establish the compliance ledger

Pin the owner-local SRD PDF's SHA-256 and retain its attribution. Assign stable
IDs to requirements and eventually every source entry and mechanical clause.
Record source section/page, implementation, verification evidence, status and
next milestone. Distinguish missing, partial, implemented and verified. Record
unreconciled inventories explicitly rather than using an approximate spell count
as a denominator. Separate canonical spells, helper actions and house content.

Add executable regression probes for casting admission, Polymorph and full
effect/zone buffers. Keep known failures visible through a strict nonzero
qualification command; never encode a bug as the desired rule or call expected
failures a compliance pass. Add ordinary checks for already-correct boundaries
and the reporting predicates. Update the specification's historical status and
link the ledger and evidence. This milestone diagnoses gaps; it does not implement
the missing character model or transformation system.

Exit: every specification area has a ledger entry and an explicit next step;
source identity is pinned or explicitly blocked; the requested probes produce
reviewable outcomes, including current failures. The ledger remains incomplete
at the individual-source-entry level until milestone 2 reconciles the PDF.

## Milestone 2 Build the canonical data pipeline

Implement the PDF-to-records tool in Coax (`rules/srd/import.coax`) and
check in generated `data/` tables. Expand schemas for stable IDs, full text,
source references, school/class lists, structured casting times, rituals,
components/costs/consumption, targets, areas and durations. Import spells and
class lists first, then classes, monsters, origins, feats, equipment, magic items,
conditions and the toolbox. Retain helper actions under their parent IDs.
Keep independent source inventories separate from implementation tables.

Exit: deterministic regeneration; counts and IDs reconciled against the pinned
PDF; no broken references or missing required text; each source entry represented
in the ledger. PDF extraction is not semantic qualification: review parsed fields
and write expected results from the source before implementing mechanics.

## Milestone 3 Complete the strict rules foundation

Represent legal character state and checks: skills, proficiency/expertise,
Advantage/Disadvantage, all condition behavior including Exhaustion, healing,
death, rests, movement, visibility, cover and hiding. Add action/reaction triggers
and explicit target requests. Casting must validate preparation/knowledge,
features, slots, components, timing, target legality and storage before spending.
Define world geometry queries without moving rule decisions into presentation.

Replace silent effect replacement and lost zones with explicit bounded admission
or adequate owned storage. Refusal must preserve resources, dice, concentration,
events and existing effects. Define same-effect stacking and replacement rules.

Distinguish malformed/unavailable requests from a cast at an invalid target:
SRD p.106 explicitly spends a used spell slot even if the target cannot be
affected. Such a cast must not be mistaken for an atomic request refusal.

Exit: source-derived fixtures for primitives and boundaries; refused requests
make no changes; valid casts at invalid targets spend as specified; accepted
effects are retained completely; identical
inputs and seeds replay identically. Admission/policy modules use `#authority;`
and zero-survivor mutation gates.

## Milestone 4 Complete effect programs and every spell

Extend the interpreter with transformation, controlled creatures, general
modifiers, movable/triggered zones, terrain construction, light, senses and typed
narrative callbacks. Preserve source-specific targeting, optional choices,
multiple rays/targets, higher-level casting and turn triggers. Replace the
Polymorph approximation and empty programs. Qualify the Druid list first, then
all lists; derive actual counts from milestone 2, not the old estimated counts.

Deliver vertical slices: transformation with Polymorph, Wild Shape and beast
records; zones with movement/start/end triggers; summoning with control and
lifecycle. Each slice includes independent fixtures before implementation.

Exit: every spell's mechanical clauses execute; base/upcast/cantrip tiers,
saves, immunity, concentration, duration, repeat triggers and invalid requests
have fixtures. Every narrative clause is listed and justified. No mechanics are
hidden behind narrative, default visuals or world-specific handlers.

## Milestone 5 Complete characters and other content

Implement legal levels 1–20 and every SRD subclass, class choices, spell
preparation, feature uses/recovery, multiclassing, species, backgrounds, feats,
ability choices, XP and progression. Existing presets become consumers of the
same model. Implement all equipment, masteries, magic item attunement/charges,
monster abilities, traps, poisons, diseases and environmental effects.

Exit: legal builds validate and illegal combinations reject; full class tables
and feature behavior are tested; each mechanical catalog entry has executable
behavior and source-derived evidence. This can advance beside milestone 4 once
the shared prerequisites from milestones 2 and 3 exist.

## Milestone 6 Separate clocks and declared world profiles

Provide an explicit initiative-driven turn-based controller and the six-second
real-time adapter. Keep the rules independent of frame timing. Put faster action
beats, cooldown recharge, unrestricted casting, material-cost waivers, grants,
object damage changes and house content in validated profiles. Apply profiles in
one shared place, with strict SRD behavior as the default.

Exit: removing a profile restores strict results; matched logical-turn scenarios
agree across clocks where intended; every behavioral deviation is reported and
tested. Test ordering, repeats, death saves, reactions and expiration, not only
elapsed damage totals.

## Milestone 7 Migrate worlds and shared surfaces

Hold the Stair and Experimental's hostiles roll the engine's dice (`rules/srd/rolls.coax`, #13269); only v1 Everglade still loads `everglade/legacy/v1/srd5*.coax`. Move Hold the Stair's numbers to the shared records.
Remove duplicated rules/numbers from Everglade and other adapters. Preserve their
play through explicit profiles and keep art, sound and physical presentation in
the worlds. Generate cards, spellbook, accessible text and index content from
canonical records. Provide default presentation and voice/agent lookup/casting
by stable ID. Add ruleset/profile configuration to the maker and edit model.

Exit: representative worlds use canonical mechanics; presentation does not
decide outcomes; reports list available spells, custom/default looks and all
overrides; applicable surfaces carry the exact SRD attribution.

## Milestone 8 Qualify the release

Produce catalog, mechanical-clause, spell, narrative and per-world reports with
source/build identities. Run deterministic replay, interaction scenarios and
mutation checks in bounded shards. Test overlap, simultaneous triggers,
replacement forms, reaction windows, rests, interrupts and capacity exhaustion.
Golden answers come from the pinned SRD, never implementation output alone.

Exit: zero missing required content or unimplemented mechanics, zero unexplained
deviations, every qualification fixture passing, and core rules authority mutations
with zero survivors. Use focused risk-appropriate checks for non-core work.
Preserve qualified evidence by commit; never repeat an
unchanged passing check merely for confirmation.

## Execution and product decisions

Use isolated worktrees; write checks before code; implement tools and logic in
Coax. Run focused checks and bounded shards (each command at most 15 minutes).
Run `coax/web/tool.sh tools-check` before pushing. Rebase onto `origin/main`,
commit only owned changes with the model's Co-Authored-By line, push `HEAD:main`
and retire the worktree. Store actual owner actions in `NEEDS_OWNER.md`.

Character creation UI, persistent cross-world XP and table-mode release timing
remain product decisions. They do not block a complete engine character model,
presets or deterministic turn controller. No cross-world persistence policy is
implied by compliance work. No calendar estimate is assigned until source
inventories and the first interpreter slices establish the workload.

## Current work

The active work is the Everglade levels 1–6 slice above, starting with casting
and core combat. Full class catalog work is parked. Milestone 1 established the
baseline ledger and strict witnesses. Milestone 2
now has a Coax importer and canonical source catalog for all 339 spell
descriptions and the eight base class tables (875 memberships), with nine
preserved source discrepancies. See [the data pipeline](../../rules/srd/data/README.md)
and [the ledger](srd-compliance-ledger.md). Other content catalogs, mechanical
target/area interpretation and executable effect completion remain open; imported
metadata does not close milestones 3-8.
