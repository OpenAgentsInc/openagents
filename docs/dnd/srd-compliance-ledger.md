# SRD 5.2.1 compliance ledger

Status: milestone 2 spell source slice, 2026-11-07. **Not fully compliant.** This ledger
covers every area required by [the specification](README.md) and assigns stable
requirement IDs. It is a requirements inventory, not yet an exhaustive inventory
of all the PDF's individual entries or mechanical clauses. Spells and base class
spell tables are now inventoried; the other catalogs and mechanical clauses remain
open. The [roadmap](srd-compliance-roadmap.md) defines all eight milestones.

Priority update, 2026-11-08: [#13030](https://github.com/OpenAgentsInc/openagents/issues/13030)
is now the Everglade levels 1–6 slice. Core combat (#13038–#13041) comes first,
then spells through 3rd level and class features through level 6 (#13043/#13044),
the used-monster subset (#13034), and Everglade migration (#13046). The minimal
profile boundary is #13045. Broad catalogs, multiclassing, magic items, toolbox,
general voice/agent/maker interfaces and full qualification remain parked and
open. Polymorph's recorded failure is outside the active spell-level limit;
Wild Shape needed by a level 1–6 class remains active. Core rules retain
zero-survivor mutation gates; non-core work uses focused lean checks. The full
requirements inventory below is not the active delivery sequence.

## Source identity

| Field | Pin |
| --- | --- |
| Source | System Reference Document 5.2.1, Wizards of the Coast LLC |
| Owner-local file | `~/Desktop/SRD_CC_v5.2.1.pdf` (not required at runtime) |
| Bytes | 6,031,375 |
| SHA-256 | `8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87` |
| Page references | Printed SRD page numbers; these match this PDF's page numbers |
| Source inventory | 339 descriptions and 875 base class-table entries reconciled; other catalogs unmeasured |
| Milestone 1 baseline code | `f7cab6d3a4ef98752ecb7d7cb834a90ea9af7bbc` |
| Milestone 2 baseline code | `593ceb26af8762425a7b83d8bc45b26e749f9546` |
| Evidence | [Milestone 1 verification](../verification/srd-compliance-m1-2026-11-07/README.md), [milestone 2 verification](../verification/srd-compliance-m2-2026-11-07/README.md) |

Changing the source hash is a deliberate source-version update, followed by
inventory and golden-fixture review. This pin establishes identity, not correct
extraction or implementation. The PDF itself is not copied into Git. Generated
spell records contain its licensed extracted descriptions and attribution.

This work includes material from the System Reference Document 5.2.1 ("SRD 5.2.1") by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative Commons Attribution 4.0 International License, available at https://creativecommons.org/licenses/by/4.0/legalcode.

## Status and evidence rules

- **Missing:** no complete representation/executor identified for this requirement.
- **Partial:** selected content/behavior exists; enumerated gaps prevent completion.
- **Implemented:** every enumerated mechanical clause has code, pending qualification.
- **Verified:** implemented plus source-derived passing fixtures and applicable
  authority mutation evidence, tied to a commit and the pinned source.

No whole requirement is marked verified merely because selected examples pass.
Missing evidence is recorded as **unmeasured**, never as a passing result.
Probe IDs below are permanent evidence keys, not a claim to cover an entire rule.
Future source-entry IDs use `spell/<srd-id>`, `class/<class>/<level>/<feature>`,
`monster/<srd-id>`, etc.; helper records link their parent, and house records
never enter the SRD denominator. Clause IDs extend those IDs after source review.

## Requirements inventory

Paths below are repository-relative. `engine`, `battle`, `spells`, `more` and
`npcs` mean the corresponding files under `aethyr/rules/srd/`. M2 through M8
refer to the roadmap milestones. Source sections are normative; code tests
named below are selected evidence only, not exhaustive qualification.

| Stable ID | Source/spec requirement | Status | Existing implementation and evidence | Remaining work and next milestone |
| --- | --- | --- | --- | --- |
| SRC-PIN | Spec Legal and naming; SRD p.1 | Partial | Importer enforces whole PDF size/hash and extracts an owned snapshot; attribution retained | Extend pinned provenance to remaining source catalogs, M2 |
| DATA-INVENTORY | Spec Import check | Partial | `data/inventory.tsv`: 339 stable spell IDs; 875 independent base class rows; nine source differences preserved | Reconcile monsters/classes/origins/feats/equipment/items/toolbox and mechanical clauses, M2 |
| DATA-IMPORT | Spec SRD content | Partial | Coax importer, generated spell/class tables, byte-identical native regeneration check | Extend importer beyond spells/lists, M2 |
| DATA-METADATA | Spec spell record shape | Partial | `SrdSourceSpell`: full extracted text, source spans, casting/range/components/duration metadata; canonical catalog API | Review mechanical targets/areas and executable effects; preserve source layout limits, M3/M4 |
| CORE-DICE | Playing the Game pp.5-8 | Partial | `engine` SplitMix64, checks, `dice are fair and repeatable` | Rule inventory and independent deterministic vectors, M3 |
| CORE-CHECKS | D20 Tests, proficiency pp.6-9 | Partial | Abilities/save proficiency; selected skill flags | Full skill/tool checks, expertise, passive checks and interactions, M3 |
| CORE-ACTIONS | Actions/combat pp.9-18 | Partial | `RsTurn`, budgets, attacks and selected reactions | Complete actions, triggers, ready/interrupt/order semantics, M3 |
| CORE-MOVEMENT | Movement and Position pp.14-15 | Partial | Grid movement, difficult terrain, pushes | All movement modes, sizes, reach, grapple and geometry contracts, M3 |
| CORE-VISIBILITY | Playing the Game and Rules Glossary | Partial | Selected condition-based attack modifiers | Cover, hiding, obscuration, perception and targeting integration, M3 |
| CORE-CONDITIONS | Rules Glossary pp.176-191 | Partial | All 15 named bits; selected enforced behaviors | Each condition's clauses, Exhaustion levels, stacking/lifecycle, M3 |
| CORE-HP | Damage/healing/death pp.15-18 | Partial | `rs_hurt`, `rs_heal`, `rs_death_save`; embedded checks | Full interaction and source-clause fixtures, M3 |
| CORE-REST | Rules Glossary, class recovery rules | Partial | Uses/maxima and selected recovery features | Short/long rests, Hit Dice, limits and recovery effects, M3/M5 |
| CAST-ACCESS | Classes and Gaining Spells pp.104-105 | Partial | Shared class eligibility/grants plus canonical cantrip/prepared selections, class limits and Wizard spellbook membership; CAST-CLASS and CAST-PREPARED pass ([selection evidence](../verification/srd-spell-selection-2026-11-08/README.md)) | Full source-feature/origin grant provenance and legal rest/level-up/copying workflows, #13038/#13044 |
| CAST-COMPONENTS | Casting Spells pp.104-106 | Missing | Display strings, no complete component state | V/S/M, focus/pouch, cost, consumption, rituals/long casts, M2/M3 |
| CAST-TARGETS | Casting Spells pp.105-106 | Partial | Shared target/Self policy plus explicit creature recipients, base-one upcasts and per-ray assignments with whole-list preflight; CAST-CHOSEN-TARGETS passes | Object/visibility/point workflows and world request adoption remain under #13038/#13043/#13046 |
| CAST-RESOURCES | Spell slots/casting rules pp.104-106 | Partial | Slot checks and turn budgets; unknown executable IDs and malformed cantrip slots now refuse atomically ([evidence](../verification/srd-casting-request-2026-11-08/README.md)); four admission probes pass | Preparation, full free-cast restrictions, transactional preflight, M3 |
| CAST-CONCENTRATION | Rules Glossary Concentration | Partial | Concentration saves, replacement and expiration | Full interruption, overlapping effects and duration fixtures, M3/M4 |
| FX-EXECUTOR | Spec typed effect program | Partial | Closed step set with specialized handlers | Transform, summon, sense, general modify/terrain/light/narrative, M4 |
| FX-TARGETING | Spec target step | Partial | Explicit recipient/ray API shares the existing executor; legacy convenience auto-selection remains during migration | Full filters/geometry, source effect options and world choice surfaces, #13038/#13043/#13046 |
| FX-ZONES | Spec zone triggers and shapes | Partial | Eight zones; Lines/Spheres and rectangular approximations | True shapes, enter/start/end timing, movement and multi-area spells, M4 |
| FX-CAP-EFFECT | Spec lasting effects; deterministic complete results | Partial | Six owned records; explicit refusal, whole-operation preflight and independent restraint leases; `CAP-EFFECT` passes | Full source stacking/condition qualification remains under #13039; bounded capacity is qualified separately under #13041 |
| FX-CAP-ZONE | Spec lasting areas; admitted cast results | Partial | Eight owned slots with preflight, release/reuse, expiry and independent restraint ownership; bounded-capacity #13041 is closed | Full geometry/trigger/condition qualification remains under FX-ZONES and #13039 |
| CONTENT-SPELLS | Spell Descriptions pp.107-175 | Partial | All 339 descriptions imported; eight class tables independently reconciled with nine source differences; executable subset unchanged | Implement/qualify every mechanical clause and explicitly resolve gameplay interpretations of source differences, M3/M4 |
| SPELL-POLYMORPH | Polymorph pp.153-154 | Partial | `more` uses Incapacitated; `SPELL-POLYMORPH` fails | Beast choice/CR, replaced stats, retained identity/HP/type, form temp HP, anatomy/gear and reversion, M4 |
| CONTENT-CHARACTERS | Character Creation pp.19-27 | Partial | Preset ability arrays and fixed builds | Legal choices, levels/XP/proficiency progression, independent fixtures, M5 |
| CONTENT-ORIGINS | Character Origins pp.83-86 | Missing | No complete origins catalog/model identified in shared engine | Species, backgrounds and origin choices, M2/M5 |
| CONTENT-CLASSES | Classes pp.28-82 | Partial | All 12 class constructors and selected subclass features | Complete levels 1-20, choices, preparation and multiclassing, M2/M5 |
| CONTENT-FEATS | Feats pp.87-88 | Partial | Selected feature/ASI behavior in constructors | Origin/general/style/epic catalog and legal executable choices, M2/M5 |
| CONTENT-EQUIPMENT | Equipment pp.89-103 | Partial | Selected weapons/masteries and fixed armor | Complete weapons/armor/tools/gear/mounts/services/crafting, M2/M5 |
| CONTENT-TOOLBOX | Gameplay Toolbox pp.192-203 | Missing | No complete shared catalog/executor identified | Travel, curses/contagions, environment, stress, poisons, traps and encounter rules, M2/M5 |
| CONTENT-ITEMS | Magic Items pp.204-253 | Missing | No complete shared catalog/executor identified | All items, attunement, charges, activation and effects, M2/M5 |
| CONTENT-MONSTERS | Monsters p.254 onward, Animals p.344 onward | Partial | Selected NPCs, Skeleton and Zombie; siege mixes aren't source catalog entries | All stat blocks, traits/actions/reactions and source IDs, M2/M5 |
| TIME-ADAPTERS | Spec one ruleset two clocks | Partial | Staggered initiative, 6s rounds, faster beats/recharge | Explicit turn controller and matched-logical-turn fixtures, M6 |
| WORLD-PROFILES | Spec World tweaks | Partial | `house` flags; adapter-level grants/free budgets | Profile parser/validation, shared application and strict-default isolation, M6 |
| WORLD-MIGRATION | Spec engine home and record consumers | Partial | Everglade/5E Lab load `battle`; Stair loads old combat rules | Remove duplicate rules/numbers; migrate Stair; preserve declared play, M7 |
| SURFACE-LOOKS | Spec Presentation | Partial | World effect libraries and hotbars | Default look for every spell, per-world custom/default report, M7 |
| SURFACE-RECORDS | Spec Formats | Partial | Cards and world text exist | All spellbook/cards/index/accessible/voice/agent surfaces from same records, M7 |
| SURFACE-MAKER | Spec How a world adds it | Missing | No complete all-spell ruleset/profile workflow identified | Maker spec/edit model, choices and same engine execution, M7 |
| VERIFY-GOLDENS | Spec Golden rules tests | Partial | Embedded examples and new focused witnesses | Independent fixtures per rule/spell/interaction with pinned source, M3-M8 |
| VERIFY-COVERAGE | Spec effect/per-world coverage | Missing | Focused report below explicitly refuses whole-SRD claim | Clause/narrative, tweak-isolation and per-world coverage reports, M8 |

## Focused regression witnesses

Run from the repository root:

```sh
coax/bin/coax test --library aethyr/rules/srd/compliance-probes.coax
coax/bin/coax mutate --library aethyr/rules/srd/compliance-policy.coax
coax/web/tool.sh aethyr/rules/srd/compliance-check.coax
coax/web/tool.sh aethyr/rules/srd/compliance-check.coax --inventory
```

The third command is intentionally strict: it currently exits **1** because
the parked Polymorph outcome is unmet. These are executable regression predicates,
not `#test` assertions that bugs should remain. The first command's ordinary
checks guard working boundaries and free-capacity controls. Fixes change a
predicate to PASS without deleting an expected-failure exemption.

| Probe ID | Desired outcome | Current | Scope |
| --- | --- | --- | --- |
| CAST-ACTOR | Missing/negative actor refuses without spending | PASS | Request index validity |
| CAST-SLOT | Unavailable slot refuses without spending | PASS | One selected slot boundary |
| CAST-REACTION-SLOT | Own-turn reaction casting respects the slot limit and records its cost before later casting | PASS | Shield/Rebuke budgets share ordinary casting's turn flags; another actor's flags do not leak into a reaction |
| CAST-CHOSEN-TARGETS | A chosen recipient set, including the caster, admits as one complete operation or refuses before spending | PASS | Native chosen Bless excludes an unchosen nearby creature and preserves state at full recipient storage |
| SPELL-OPTIONS | Named condition/ability choices reach execution and source deadlines | PASS | Native mixed Blindness/Deafness and Hex ability/duration; this does not qualify all condition/check consequences |
| CAST-TARGET-INDEX | Missing/negative target index refuses without spending | PASS | Malformed request, not SRD invalid-target rule |
| CAST-RANGE | Out-of-touch-range Cure Wounds refuses without spending | PASS | Range admission |
| CAST-CLASS | Unmodified preset Druid cannot cast Guiding Bolt | PASS | Class eligibility and explicit extra-list grants enforced in shared admission; separate selected-state admission is covered by CAST-PREPARED |
| CAST-PREPARED | An eligible but unselected Cure Wounds refuses without spending | PASS | Complete legal alternative loadout excludes Cure Wounds; shared readiness refuses it, including native execution |
| CAST-SELF | False Life cannot name another primary creature | PASS | Structural request refusal preserves resources; source Self range is distinguished from originating effects and later helper actions |
| CAST-INVALID-TARGET | Hold Person on a Skeleton spends its used slot without affecting it or rolling a save | PASS | Corrected shared target policy/outcome; apparent save succeeds with no invented roll, SRD p.106 and p.141 |
| SPELL-POLYMORPH | Failed save grants form temp HP, retains HP/type, does not merely Incapacitate | FAIL | Necessary witness only; current API cannot select a Beast, so even a future PASS would not qualify the full spell |
| CAP-EFFECT | Adding another effect never silently removes independent existing effects | PASS | Explicit insertion refusal preserves every existing record; native full-cast refusal preserves budgets/dice/events and freeing one record admits the complete cast |
| CAP-ZONE | Full unrelated zones refuse before resources, concentration or effects change | PASS | Preflight chooses an available or concentration-released slot before spending; accepted zones are retained and expired storage is released; the limit is an implementation capacity, not an SRD rule |

Refusal snapshots check slots, action/bonus/slot/leveled budgets, dice, event-log
counters, concentration and target HP/temp/conditions. The zone probe also checks
existing zone identity/bounds/duration. These are selected observable projections,
not a byte-for-byte proof of complete battle-state equivalence.

The 2026-11-08 [target-casting evidence](../verification/srd-casting-target-2026-11-08/README.md)
records the first core slice change: invalid-type casts pay their resources,
while request refusals preserve preflight state (including the per-cast affinity
flag). Core policy/outcome mutation gates kill all 31 selected mutants. This does
not close #13038: class/preparation/component admission and explicit target
choices remain open. The subsequent [spell-access evidence](../verification/srd-spell-access-2026-11-08/README.md)
turns CAST-CLASS to PASS: six witnesses pass and three fail. Polymorph remains a
parked higher-level failure; the two capacity failures remain active work.
The [selection change](../verification/srd-spell-selection-2026-11-08/README.md)
adds CAST-PREPARED: seven of ten witnesses now pass. The Polymorph fixture now
explicitly prepares its spell and provides a level-appropriate slot, so its
failure still diagnoses transformation rather than missing selection state.
The subsequent [Self request change](../verification/srd-self-targets-2026-11-08/README.md)
adds CAST-SELF: eight of eleven witnesses pass, with the same three failures.
The [zone admission/lifetime change](../verification/srd-zone-storage-2026-11-08/README.md)
turns CAP-ZONE to PASS: nine of eleven witnesses pass. CAP-EFFECT remains active;
Polymorph remains parked. Full effect storage and independent zone-restraint
bindings are not qualified by passing this selected zone witness.

The [effect admission and ownership change](../verification/srd-effect-storage-2026-11-08/README.md)
turns CAP-EFFECT to PASS: ten of eleven witnesses pass; only parked Polymorph
fails. Whole-operation preflight covers normal/Quickened and multi-target casts,
attack batches and requested hit riders, Turn Undead and curse transfer. Each
zone restraint owns a record; release and escape preserve other owners. Capacity
refusal precedes costs, dice and events. These bounded-storage guarantees do not
qualify every condition, duration or same-spell stacking rule.

The [reaction slot change](../verification/srd-reaction-slots-2026-11-08/README.md)
adds CAST-REACTION-SLOT: eleven of twelve witnesses pass, only parked Polymorph
fails. Own-turn Shield/Hellish Rebuke refuse a second slot or a level 1+ cast
after Quickened Spell; an accepted reaction marks slot/level history for later
casting. Off-turn reactions preserve the active actor's history. Mage's
Protective Magic daily use is distinguished from a real spell slot. This does
not qualify every reaction trigger, line of sight or NPC resource workflow.

The [effect-roll overlap change](../verification/srd-effect-rolls-2026-11-08/README.md)
applies one Bless d4 and one Bane d4 while retaining each independent owner and
deadline. One attack consumes every pending weakness, Guiding Bolt and matching
Vex rider, including a missed or canceled-edge roll; Vex for another target stays.
Actual casts, concentration release/expiry, seeded dice and the hotbar path are
checked. This is partial #13039 progress, not qualification of all same-spell
combination rules, conditions, damage or death.

The [zero-HP transition change](../verification/srd-hit-points-2026-11-08/README.md)
keeps death-save failures on damage absorbed by temporary HP, resets both save
counts on stabilization, and reports natural-20 recovery only when healing
actually restores HP. Healing and condition retirement preserve independent
Unconscious sources, and saturating healing cannot overflow. Source-boundary,
turn, native and hotbar fixtures cover these transitions; core HP/storage gates
have zero survivors. This is partial #13039 progress, not complete HP/recovery,
condition, damage-defense or concentration qualification.

The [chosen-target change](../verification/srd-chosen-targets-2026-11-08/README.md)
adds CAST-CHOSEN-TARGETS: twelve of thirteen witnesses pass, with only parked
Polymorph failing. Explicit casts choose distinct recipients, including the
caster, or assign individual rays. Source upcast counts for Bless, Bane,
Command, Hold Person and Blindness/Deafness are represented. Invalid types and
successful saves do not cancel other recipients' concentration effects, and
all requested effect storage is checked before costs. Existing worlds still
have their legacy convenience picker during migration; this is an engine API
slice, not a claim that all player/wire choice surfaces have been adopted.

The [spell-option change](../verification/srd-spell-options-2026-11-08/README.md)
adds SPELL-OPTIONS: thirteen of fourteen witnesses pass, with parked Polymorph
still failing. Each chosen recipient can select Blinded or Deafened; Hex retains
the selected ability through curse transfer and uses source upcast durations.
The actual variant is used for immunity/capacity preflight and execution.
General ability checks, condition consequences, remaining Command options and
world choice surfaces remain active work.

The [Command Halt turn change](../verification/srd-command-halt-2026-11-08/README.md)
queues Halt until the recipient's next logical turn, blocks movement, Actions
and Bonus Actions for that turn, and retires it at turn end. Reactions and
passive features remain available. COMMAND-HALT-TURN adds a native witness;
Approach, Flee, Grovel and Drop remain unqualified. This is partial #13040 and
#13043 progress and does not complete those issues.

The [Prone movement change](../verification/srd-prone-2026-11-08/README.md)
removes automatic turn-start standing. Explicit standing pays half current
Speed rounded down in feet; crawling costs ten feet per five-foot step, fifteen
in Difficult Terrain. Residual feet are retained. The creature's attacks have
Disadvantage, while attacks against it use distance, not a melee-only test.
CONDITION-PRONE adds a native witness. Movement modes, profiles and world
controls remain open under #13039/#13040/#13045/#13046.

The [timing-profile change](../verification/srd-timing-2026-11-08/README.md)
replaces bare battle beat/recharge values with validated strict/action presets.
Strict grants exact Speed in feet with no timer-based recovery. Action combat
explicitly retains its shorter beat, cooldowns and rounded movement grant,
including a standing cost based on that grant. Selection seals before play;
reset returns to strict. TIMING-PROFILE adds a native witness. General profiles,
free casting/material/grant overrides and full world migration remain open.

The [condition-roll change](../verification/srd-condition-rolls-2026-11-08/README.md)
adds Petrified attack Advantage, automatic Strength/Dexterity save failure and
all-damage resistance. Nearby ranged weapon hits against Paralyzed/Unconscious
targets become criticals. Shared spell/Cunning Strike save resolution skips all
dice and rescue resources on automatic failure; normal saves include named boon
dice and existing rescue policy. Immunity/resistance/vulnerability ordering and
nonstacking resistance are checked across all 13 damage types. The new core
authority has zero mutation survivors; native scenarios, one sanitized native
condition property and hotbar integration have scoped receipts. Prone remains
owned by the independently landed movement policy. General checks, other
conditions, lifecycle/concentration and remaining save producers keep #13039 open.

The [saving-throw integration](../verification/srd-saving-throws-2026-11-08/README.md)
moves all existing ordinary battle save producers onto that same resolver:
Fortitude, concentration damage, reaction/feature and repeat saves now receive
its modifiers, resource spending and automatic failures. Fortitude's HP update
preserves rescue resources consumed by its save. Eleven new evaluator/native
scenarios cover the migrated paths. Death saves, condition lifecycle and wider
combat acceptance remain open; #13039 is not complete.

The [component admission change](../verification/srd-components-2026-11-08/README.md)
projects canonical V/S/M, price and consumption flags for all 339 records.
Ordinary, Reaction and empty-target paths check speech, hands, eligible focus
or pouch, and exact material receipts before casting costs. Reusable materials
remain; component payment tests consume one matching set at a time. Separate
class-feature grants preserve class-focus eligibility without treating arbitrary
world grants as class spells. CAST-COMPONENTS adds a native witness. Actual
equipment/held-object state, target-dependent material quantities, feature
waivers and world control integration remain open; this does not complete
#13038, #13044, #13045 or #13046.

The [effect lifecycle change](../verification/srd-effect-lifecycle-2026-11-08/README.md)
ends concentration, Rage and the recipient's Turn Undead ownership immediately
when a retained condition incapacitates it. Immune/refused effects preserve that
state; Stunning Strike no longer ends Rage on an immune target. Zero-HP damage
also ends Rage immediately. The new authority kills all 50 mutants, and scoped
native/hotbar/component checks pass. Condition lifecycle and other #13039 clauses
remain incomplete.

The [death-save change](../verification/srd-death-saves-2026-11-08/README.md)
adds existing boon/Aura/rescue modifiers to actual battle death saves without
an ability modifier/proficiency. Natural one/twenty outcomes use the raw face,
ordinary success uses the modified total, and consumed rescue resources stay
consumed. Both new authority gates have zero survivors; original-source native
outcome properties pass under ASan/UBSan and real turn/saved-seat scenarios pass.
General death-save Advantage, Exhaustion, condition/recovery and wider #13039
acceptance remain open.

The [concentration lifecycle change](../verification/srd-concentration-2026-11-08/README.md)
interrupts concentration on accepted incapacitating effects, including Hypnotic
Pattern and Stunning Strike, rather than selected spell branches alone.
Teardown preserves other owners/records and clears deadlines; concentrating
programs cannot attach later effects after ownership ends. Free dismissal works
outside a saved seat's turn. CONCENTRATION-LIFECYCLE adds a native witness.
Raw state-writer integration, overlapping outline provenance, complete feature
lifecycles and full casting workflows remain open; #13039 stays open.

The [Dodge change](../verification/srd-dodge-2026-11-08/README.md) replaces timer
expiry with actual next-turn retirement and irreversible loss on tracked
incapacity/Speed-zero transitions. A normal Dodge Action and Patient Defense
share the benefit; roll consumers and the Lab read the shared state. Core
Dodge/lifecycle authorities have zero mutation survivors and ten native root
scenarios pass. This does not complete either #13039 or #13040; broader movement,
condition and recovery work and ordinary Dodge player controls remain open.

## Canonical source inventory

[data/inventory.tsv](../../aethyr/rules/srd/data/inventory.tsv) is the source-entry
ledger: one stable `spell/<id>` row per spell, with page span and unqualified
mechanics status. [Class rows](../../aethyr/rules/srd/data/class-lists.tsv) preserve
all eight base tables. [Discrepancies](../../aethyr/rules/srd/data/discrepancies.tsv)
preserve nine differing assertions across Flaming Sphere, Mind Spike, Phantasmal
Force and True Strike. They are exact pinned-source exceptions, not mutant
exemptions or silent corrections. [The pipeline guide](../../aethyr/rules/srd/data/README.md)
describes reproduction, schema, text fidelity limits and access from Coax.

Class-table counts: Bard 129, Cleric 109, Druid 124, Paladin 38, Ranger 48,
Sorcerer 138, Warlock 72, Wizard 217. Grants from subclasses, feats and profiles
are separate. Other source catalogs and mechanical-clause inventories remain
unmeasured. Imported metadata does not expand the executable `rs_spell` table.

## Historical runtime record inventory

[spell-records.tsv](../verification/srd-compliance-m1-2026-11-07/spell-records.tsv)
lists every currently assigned record by stable textual ID, parent, kind, text
presence, page and effect-step count. `canonical-candidate` means a top-level
record marked SRD in the code, not independent confirmation that it matches the
source. Numeric holes are excluded. The report does not count helpers as spells
or house content as SRD. It reports metadata presence, not metadata correctness
or mechanical completeness. Mapping these executable records to the canonical
source and qualifying their mechanics remain open.

## Next implementation slice

Active next: add slice casting state, actions/conditions/damage/death and
transactional storage admission, preserving the p.106 invalid-target distinction.
Then complete the 0–3rd-level spells/features through character level 6, used
monsters and shared Everglade integration. Keep these probe IDs and extend source
fixtures as prerequisites land. Full catalog expansion and Polymorph are parked.
Source access is available; no owner action is required for this work.

### Shared damage-roll progress, 2026-11-08

SRD p.16 simultaneous saving-throw damage now shares one roll per instruction
across cast, Ice Knife burst and hotbar zone-pulse recipients. Saves and defenses
remain individual; separate attacks/triggers/pulses remain independent. Roll
modifiers precede save halving. Policy mutation kills all 20 mutants; Fireball,
Enfeeblement/affinity/resistance and renewed Moonbeam pulse regressions pass.
This does not complete #13039/#13043; see [scoped evidence](../verification/srd-shared-damage-2026-11-08/README.md).

### Dash movement progress, 2026-11-08

Shared Action Dash, Cunning Action and free Step of the Wind now credit current
tracked walking Speed in exact feet, including Slow/halving, residual feet and
zero-Speed boundaries. Refusal preserves budgets/events; feature event identities
and the separate Action/Bonus Action costs remain. Policy gate kills all 78
mutants; native Dash, 198 hotbar and 21 selected Experimental checks pass.
#13040 remains open for special speeds, mid-turn credit reconciliation and other
action/reaction rules. [Evidence](../verification/srd-dash-2026-11-08/README.md).

### Exhaustion level progress, 2026-11-08

Exhaustion now has cumulative scalar state and gain/recovery APIs. D20 penalties
reach attacks, saves, initiative, Athletics escape and both death-save paths;
walking Speed subtracts five feet per level. Six levels kill without damage and
end owned concentration through the shared lifecycle. Natural criticals/death
save twenties remain special; recovery cannot revive. Five changed authority
units kill 26/49/66/27/59 mutants with zero survivors. Rest admission, mid-turn
movement-credit recalculation and producer/world migration are not complete;
#13039 remains open. [Evidence](../verification/srd-exhaustion-2026-11-08/README.md).

### Mid-turn walking credit progress, 2026-11-08

Current-turn ordinary and Dash allowances now reprice tracked walking Speed
while retaining feet already spent, including overspending debt. Speed loss,
Exhaustion recovery and restoration cannot refill a spent turn. Step/stand
payment remains transactional; grid loops and saved seats consume the shared
accounting. Core policy/Dash mutation gates kill 57/82 mutants with zero
survivors; 241 core, 181 Prone, 206 hotbar and 21 selected Experimental checks
pass, as do native movement and Dash witnesses. Special speeds, feature-specific
movement and other action/condition requirements remain; #13040/#13039 stay open.
[Evidence](../verification/srd-movement-credit-2026-11-08/README.md).

### Condition movement and initiative progress, 2026-11-08

Stunned no longer incorrectly prevents walking or standing. Shared movement
permission is distinct from action capability; independent Speed/condition
blockers still apply. Initiative now applies Incapacitated Disadvantage (including
implied conditions) and Invisible/Athlete Advantage with ordinary cancellation.
Seventeen policy mutants are killed, zero survive; 205 core, 209 hotbar and 21
selected Experimental checks and an actual native witness pass. The saved-seat
path separates movement from refused actions, but Experimental's combined
hostile-AI held flag still needs #13046 migration. #13039/#13040 remain open.
[Evidence](../verification/srd-condition-permissions-2026-11-08/README.md).

### Experimental hostile permission migration, 2026-11-08

Experimental now projects shared movement and action permissions independently
into hostile bodies. Stunned can pursue but cannot strike; Restrained stays put
without losing action permission. Pending swings cancel immediately on action
loss, including during hit stop, and cannot resume on recovery. Return-home
movement respects held/stopped/dead bodies. All 198 hostile/geometry checks,
22 selected Experimental checks and an actual native body witness pass. This
resolves the combined-held adapter gap from the preceding condition receipt,
but legacy hostile rolls/player HP and the rest of #13046 remain unqualified.
[Evidence](../verification/srd-hostile-permissions-2026-11-08/README.md).

### Ability-check training and resolution progress, 2026-11-08

Shared checks now use explicit selected skill/Expertise/tool masks, one
proficiency bonus, tool/skill Advantage, Jack/Primal Knowledge and applicable
condition edges. Sensory automatic failures and malformed requests spend no
dice/resources. Entangle escape and Initiative consume the shared plan;
Exhaustion penalties survive integration. Passive Perception is a score, and
Tactical Mind preserves its use on a failed attempt. Class-only presets name
their selections through level six. Gates kill 129/125/83 mutants with zero
survivors; native, sanitized pure-property and actual MMO AI escape checks pass.
Initiative preserves Invisible/Athlete and incapacitation sources through the
shared permission helpers; their updated 17-mutant gate also has zero survivors.
Geometry, full grant/build legality, other class/order bonuses and the remaining
#13039/#13044 requirements stay open. [Evidence](../verification/srd-ability-checks-2026-11-08/README.md).

### Typed stat-attack prerequisite, 2026-11-08

Printed melee NPC attacks now have a validated shared Action entry that supplies
explicit attack/damage modifiers to the existing strike executor. Shared reactions,
defenses, temporary HP and zero-HP concentration cleanup remain active; source
names and zero-dice damage are preserved without fake catalog weapons. Policy
mutation gates kill 80/8 mutants, zero survive. The rebased tree passes 210 core,
214 hotbar and 22 selected Experimental checks and an actual native witness.
Physical contact wiring, player HP migration and used-monster data remain;
#13034/#13046 stay open. [Evidence](../verification/srd-stat-attacks-2026-11-08/README.md).

## First aid and stable recovery — 2026-11-08

Shared Help first aid now spends an Action on the actual Wisdom (Medicine)
check, stabilizes without healing/waking, and resets both death-save counts.
New stable episodes from first aid/three battle successes arm one 1d4-hour
logical-clock recovery; admission handles standalone stable creatures and
preserves restored deadlines. Damage/death/earlier healing suppress stale
recovery; no-healing blocks actual recovery without rerolling; other Unconscious
owners remain. A saved-seat adapter preserves the other actor's turn.
Evidence: [stabilization](../verification/srd-stabilization-2026-11-08/README.md):
251 composed core checks, 216 hotbar checks, fifteen root native O0/O2 scenarios;
112 stabilization and 32 battle death-save mutants killed, zero survivors.
Healer's Kit, knockout/Short Rest, general Help, world inputs and remaining
#13039/#13040 acceptance are still open. This is not full recovery qualification.

### Experimental contact and health migration, 2026-11-08

The live physical path now emits bounded contacts and resolves shared typed NPC
attacks through saved turns, including quiet repeat saves. Actual class AC,
reactions, defenses and temporary HP reach the player. Shared hero HP is canonical;
per-cast synthetic HP/reset/transfer is removed. HUD and explicit respawn consume
that state; every landed striker receives its own hit stop. Queue mutation kills
45 mutants with zero survivors; 51 selected Experimental, 216 hotbar, focused HUD/
respawn/body checks and the native component chain pass. A real release build
fails on three wider checks, all reproduced on unmodified main `79a2ce92b3` with
the same compiler; no Wasm/browser qualification is claimed. Canonical monster
profiles, remaining overrides/world integration and the build blockers keep
#13046 open. [Evidence](../verification/srd-contact-health-2026-11-08/README.md).
