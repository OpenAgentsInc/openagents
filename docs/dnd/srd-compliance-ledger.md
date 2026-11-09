# SRD 5.2.1 compliance ledger

Status: milestone 2 spell source slice, 2026-11-07; requirement rows refreshed
2026-11-08 from the dated evidence below. **Not fully compliant.** This ledger
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
Wild Shape for the level 1–6 Druid is in `wild-shape.coax` (#13044). Core rules retain
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
| CORE-CHECKS | D20 Tests, proficiency pp.6-9 | Partial | Shared saving throws across combat executors; ability checks from explicit skill/tool training, expertise and Passive Perception (`ability-checks`, `check-training`); Help assistance ([checks](../verification/srd-ability-checks-2026-11-08/README.md), [saves](../verification/srd-saving-throws-2026-11-08/README.md), [Help](../verification/srd-help-2026-11-08/README.md)) | Geometry, full grant/build legality and remaining class/order bonuses, #13039/#13044 |
| CORE-ACTIONS | Actions/combat pp.9-18 | Partial | `RsTurn` budgets; shared Dash, Disengage, Dodge, Help; opportunity triggers/attacks with reach and departure order; typed stat-block melee attacks; reaction slots and frames on the true turn; Ready (one attack, a move or a held spell, answered in the turn owner's order), Search, Study, Influence and Utilize, Hide by concealment, and Grapple and Shove as Unarmed Strike options with escape and dragging ([actions and movement](../verification/srd-actions-movement-2026-11-08/README.md), [Dash](../verification/srd-dash-2026-11-08/README.md), [Disengage](../verification/srd-disengage-2026-11-08/README.md), [opportunity](../verification/srd-opportunity-attack-2026-11-08/README.md), [strike reactions](../verification/srd-strike-reactions-2026-11-08/README.md)) | Experimental slots for Search, Study, Influence, Utilize and readied moves or spells; world triggers beyond a creature's deed, #13046 |
| CORE-MOVEMENT | Movement and Position pp.14-15 | Partial | Grid movement with current-Speed walking credit, difficult terrain, crawling/Prone standing, pushes; physical controller uses the same rates ([movement credit](../verification/srd-movement-credit-2026-11-08/README.md), [physical scale](../verification/srd-physical-scale-2026-11-08/README.md)); strict continuous walking pays the turn's feet with Difficult Terrain, crawling and Dash, and the walker can't overspend ([world-profile evidence](../verification/srd-world-profile-2026-11-08/README.md)); climbing and swimming surfaces cost 5 extra feet without that Speed on the grid and in continuous walking, Long and High Jumps with run-ups, Step of the Wind and Jumper, and a grappler drags at 5 extra feet ([actions and movement](../verification/srd-actions-movement-2026-11-08/README.md)) | Sizes and geometry contracts; Experimental places no climbing or swimming surface; Shove's push is grid-only |
| CORE-VISIBILITY | Playing the Game and Rules Glossary | Partial | Selected condition-based attack modifiers; you-can-see spell choices refuse unseen creatures and points before costs ([evidence](../verification/srd-casting-workflows-2026-11-08/README.md)); Half, Three-Quarters and Total Cover as world facts and from creatures between, Lightly and Heavily Obscured squares, unseen attackers and targets, Hide by concealment and Search by Perception ([actions and movement](../verification/srd-actions-movement-2026-11-08/README.md)) | Worlds don't yet place light or obscurement; monsters don't hide; an area's cover is read from the caster |
| CORE-CONDITIONS | Rules Glossary pp.176-191 | Partial | All 15 named bits; shared condition permissions (movement/action/initiative), attack/save/critical/damage outcomes, cumulative Exhaustion levels, incapacitation interrupting sustained effects ([permissions](../verification/srd-condition-permissions-2026-11-08/README.md), [rolls](../verification/srd-condition-rolls-2026-11-08/README.md), [Exhaustion](../verification/srd-exhaustion-2026-11-08/README.md)) | Charmed's spell/stat-attack refusal, Frightened line of sight/approach and held items remain; owners and slice clauses are checked ([knockout and owners](../verification/srd-knockout-conditions-2026-11-08/README.md)) |
| CORE-HP | Damage/healing/death pp.15-18 | Partial | Shared hit points, simultaneous save damage, death saves with bonuses, first aid stabilization and timed stable recovery ([HP](../verification/srd-hit-points-2026-11-08/README.md), [death saves](../verification/srd-death-saves-2026-11-08/README.md), [stabilization](../verification/srd-stabilization-2026-11-08/README.md)) | Knockout, Healer's Kit and source-example interaction fixtures done ([evidence](../verification/srd-knockout-conditions-2026-11-08/README.md)); HP-maximum-0 death has no slice producer |
| CORE-REST | Rules Glossary, class recovery rules | Partial | Short Rest with chosen Hit Dice and class recovery; Short/Long Rest integer clock boundaries and scheduling, ordered damage/casting/Initiative causes and terminal cleanup, once-per-completion benefits and owned natural sleep; queued recovery choices applied in the controller's order when a Short Rest ends with nobody there to choose ([actions and movement](../verification/srd-actions-movement-2026-11-08/README.md)); Experimental settled controls and admitted activity ([controls](../verification/srd-rest-controls-2026-11-08/README.md), [Short Rest timing](../verification/srd-short-rest-time-2026-11-08/README.md)) | Everglade II/room controls and activity; rejoin persistence, source loss/nutrition producers, inventory and sleep presentation, #13040/#13046 ([remaining contract](long-rest-implementation.md)) |
| CAST-ACCESS | Classes and Gaining Spells pp.104-105 | Partial | Shared class eligibility/grants plus canonical cantrip/prepared selections, class limits and Wizard spellbook membership; CAST-CLASS and CAST-PREPARED pass ([selection evidence](../verification/srd-spell-selection-2026-11-08/README.md)) ; every level 1-6 preset grant is checked against its SRD feature ([provenance](../verification/srd-casting-workflows-2026-11-08/README.md)) | Origin (background/species) grants and legal rest/level-up/copying workflows, #13044 |
| CAST-COMPONENTS | Casting Spells pp.104-106 | Partial | V/S/M, price and consumption flags for all 339 records; ordinary, Reaction and empty-target casts check speech, hands, focus or pouch and exact material receipts before costs ([evidence](../verification/srd-components-2026-11-08/README.md)) | Equipment/held-object state, target-dependent quantities, feature waivers and world controls, #13044/#13045; Rituals and their Concentration/turn workflow are shared engine rules ([evidence](../verification/srd-casting-workflows-2026-11-08/README.md)) with no world control yet |
| CAST-TARGETS | Casting Spells pp.105-106 | Partial | Shared target/Self policy plus explicit creature recipients, base-one upcasts and per-ray assignments with whole-list preflight; CAST-CHOSEN-TARGETS passes; sight-required choices refuse unseen creatures/points; attack cantrips take objects by the object AC/HP/immunity/threshold rules; Experimental's hotbar sends target, option and slot-level choices ([evidence](../verification/srd-casting-workflows-2026-11-08/README.md)); Total Cover refuses a targeted spell and each chosen target, and an area skips a creature behind it ([actions and movement](../verification/srd-actions-movement-2026-11-08/README.md)) | World-placed objects and Everglade II request adoption remain under #13046 |
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
| CONTENT-MONSTERS | Monsters p.254 onward, Animals p.344 onward | Partial | Every block the worlds field (#13034): Imp, Quasit, Goblin Warrior, Ogre, Hill Giant, Wolf, Zombie, Skeleton and the npcs.coax NPCs, with a checked used-monster manifest; siege mixes aren't source catalog entries | All other stat blocks, and unused parts of the used ones (Fly, senses, Invisibility, Shape-Shift, Scare, Hide), M2/M5 |
| TIME-ADAPTERS | Spec one ruleset two clocks | Partial | Declared `RS_STRICT_TIMING` and `RS_ACTION_TIMING`, sealed at first use; durations such as Shield and Dodge retire on the actual turn; start/end-of-turn triggers, "until the end of your next turn", round/minute/hour durations and every executable spell's timed duration checked in both profiles ([timing profiles](timing-profiles.md), [evidence](../verification/srd-timing-2026-11-08/README.md), [world-profile evidence](../verification/srd-world-profile-2026-11-08/README.md)) | Explicit turn controller, an owner's chooser for simultaneous effects (fixed engine order today), M6 |
| WORLD-PROFILES | Spec World tweaks | Partial | One `rs_set_world` declaration per world (`RS_STRICT_WORLD`, `RS_ACTION_WORLD`, `RS_EVERGLADE_WORLD`) carrying timing, area-origin, press-budget and walking choices, all or nothing, with every deviation named; shared rules read the applied fields; Everglade II's named social targeting ([world-profile evidence](../verification/srd-world-profile-2026-11-08/README.md), [area-origin evidence](../verification/srd-area-origin-2026-11-08/README.md), [timing profiles](timing-profiles.md)) | Profile format/text parser and editor, remaining material/grant/damage tweaks, M6 |
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
coax/web/tool.sh aethyr/rules/srd/compliance-check.coax --active
coax/web/tool.sh aethyr/rules/srd/compliance-check.coax --inventory
```

The third command is intentionally strict: it currently exits **1** because
the parked Polymorph outcome is unmet. The fourth runs and prints the same
witnesses, marks the probes `compliance-policy.coax` declares parked (outside the
active #13030 slice; today only SPELL-POLYMORPH), and exits **1** only when an
active witness fails, so a new regression is not hidden behind the known one.
It refuses (exit 2) when every probe is parked. Parking is not completion and
`--active` passing is not compliance; the strict command remains the record.
The measured count is tallied from the reports, so adding a probe needs only its
`sc_report` line.

For the active spell slice (#13043), `coax/web/tool.sh aethyr/rules/srd/spell-coverage.coax`
counts, per base class table, the cantrips and 1st-3rd level spells that have
an executable effect program mapped to their canonical entry (`--list` prints
one row per spell; `--max-level N` widens it). Baseline when the report landed: 43 of
the 183 distinct spells at levels 0-3 had an executor. After the
[slice spells](../verification/srd-slice-spells-2026-11-08/README.md) (Inflict
Wounds, Acid Splash, Magic Missile, Mass Healing Word) and [Shatter](../verification/srd-shatter-2026-11-08/README.md):
48 of 183, 135 source text only. An executor is not qualification:
it does not show that every mechanical clause of the spell is implemented. These are executable regression predicates,
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
Later the same day the three build blockers were resolved: sliders and the
level-17 Warlock in [Lab build](../verification/srd-lab-build-2026-11-08/README.md),
Siege tactics in [Siege build](../verification/srd-siege-build-2026-11-08/README.md);
subsequent evidence records complete checked Lab Wasm builds. They no longer
block #13046, which stays open for its other acceptance items.

### General Help action, 2026-11-08

The shared engine now admits both Help benefits: an actual skill/tool proficiency
assists another ally's next matching check, and a five-foot distraction assists
another ally's next attack against that enemy. Benefits join ordinary Advantage
before cancellation, consume on the matching check/attack, and expire at the
helper's actual next turn rather than a clock estimate. Current task/geometry
exclusions are per helper; first aid forwards them. Full storage refuses before
Action spending. Saved-seat adapters preserve the running actor's turn.
[Evidence](../verification/srd-help-2026-11-08/README.md): 297 composed core checks,
219 hotbar checks, 21 root native O0/O2 scenarios; Help/resolver/clock/first-aid
gates kill 165/87/37/112 mutants, all zero survivors. World controls/geometry,
remaining condition/action rules, Healer's Kit, knockout and rests remain active
#13039/#13040/#13046 work. This does not close those issues.

### Displayed class-kit access/component audit (2026-11-08)

All displayed spells in all 12 class presets at levels 1–6 pass shared
access/preparation and component availability checks; every displayed kit is
nonempty. Twelve class slices and their complete 200-check library composition
pass. [Evidence](../verification/srd-kit-admission-2026-11-08/README.md).
This verifies fresh preset supply for displayed spell choices, not effect
execution, complete SRD choice coverage, nonspell feature behavior, physical
inventory acquisition or world state projection. No issue closes on this audit.

### Enabled class-kit spell manifest (2026-11-08)

The slice's enabled spell manifest is the 24 spells the twelve class kits
enable at levels 1–6: 23 shown on the bars plus the Sorcerer and Wizard's
Shield Reaction (`apps/openagents-web/class-kit-spells-checks.coax`, held
equal to the kits). All 24 are executable and each has an SRD resolution check
(`kit-cantrips-checks`, `kit-spells-checks`, `kit-spells-high-checks`), its
canonical metadata, its own sound and its own look. The checks found and fixed
Hypnotic Pattern dazing on a successful save, Entangle restraining its own
caster, Divine Smite ignoring Fiends, Spirit Guardians' first-turn save, and
eleven wrong source pages. 24 of 24 manifest spells checked; 0 left.
[Evidence](../verification/srd-kit-spells-2026-11-08/README.md). Spells the
presets prepare but no kit shows remain outside the manifest.

### Room-library baseline restored (2026-11-08)

The original 1,400-frame horde cast-distribution check now passes at its
unchanged traversal budget after replacing a whole-creature copy with a view
in the raid's presence query. All assertions and AI decisions remain. The
complete room library passes 325/325 checks. Together with the preceding
leave-is-not-death fix this clears both retained room-library baseline failures.
[Evidence](../verification/srd-room-traversal-2026-11-08/README.md). This is
integration/build evidence, not full SRD or #13046 world qualification.

### Actual-turn strike features and sight (2026-11-08)

On the published opportunity-frame/movement APIs, Frenzy now observes the true
turn owner, and Stunning Strike admits represented Monk weapons as well as
Unarmed Strike, with the same retained-slot reservation before Reaction/Focus
spending. A successful save can reduce Speed enough to interrupt departure
without refunding the attack. Shared attack edges include Blinded/Invisible
sight; Dodge requires seeing the attacker while retaining Dexterity-save
Advantage. [Evidence](../verification/srd-strike-reactions-2026-11-08/README.md):
282 rebased core, 232 hotbar and six actual O0/O2 native scenarios pass;
four core scopes kill 31/55/6/47 mutants, zero survivors/no exemptions. The
ordinary checked Lab Wasm build passes without budget overrides. Full legal
equipment/grants, special senses/geometry, class/recovery and physical/network
world migration remain open; no issue closes on this progress.

### Physical walking rate and bounded collision integration (2026-11-08)

Shared current walking Speed and additive crawling/difficult-terrain costs now
reach Experimental's real player controller and staged AI, including rates above
one. An opt-in bounded subdivision driver retains the original fixed clock,
interpolation, queued-input refusal and collision travel limits; default exports
and replay remain unchanged. [Evidence](../verification/srd-physical-scale-2026-11-08/README.md):
316 walker and 200 SRD-rate checks pass; three core gates kill 25/25/15 mutants,
zero survivors/no exemptions. World cases exercise Monk Speed, halving, crawling
and actual slower/faster AI movement. Strict continuous feet-per-turn payment,
special speeds/jumps, complete effect grants and world-zone/timing qualification
remain open. Continuous action tuning is explicitly documented as a house
profile, not strict movement compliance; no issue closes from this slice.

### Shield retires on actual turn start (2026-11-08)

Shield's +5 AC now uses admitted retained state retired by the caster's actual
next turn start. This fixes an extra-beat saved-seat lifetime and avoids
clock-only retirement; borrowed reaction frames do not consume it. AC and Lab
presentation agree. [Evidence](../verification/srd-shield-turn-2026-11-08/README.md):
201 core and 201 saved-seat checks, 210 opportunity checks, real strict/action
native cast/retirement and the checked Lab Wasm build pass; 8 lifetime mutants
killed, zero survivors/no exemptions. This supersedes earlier duration-forecast
checks, not the remaining spell interactions or global timing qualification.

### Rest activity producers (2026-11-08)

Continuing Short Rest recovery at `9352b75109`, accepted grid/player/AI walking
and requested admitted jumps now interrupt retained rest or its completed
choice window. Denied attempts/passive displacement preserve it; seeded Hit
Dice and recovery history are unchanged. [Evidence](../verification/srd-rest-activity-2026-11-08/README.md):
260 composed rest/activity checks and focused world/controller checks pass;
rest-state mutation kills 95, zero survivors/no exemptions. Long Rest,
rest-start controls/settled-downtime admission, network/other interruption
producers and world reconnect qualification remain open.

### Long Rest resource-bank layer (2026-11-08)

The trusted completed-rest bank operation now restores represented class banks
and all spent Hit Dice, resets daily recovery flags and cooldown bookkeeping,
and clears Temporary Hit Points. Remaining created slots have explicit
provenance; unrelated excess banks are preserved and malformed bank state
refuses before partial mutation. [Evidence](../verification/srd-long-rest-resources-2026-11-08/README.md):
264 composed checks, 104 killed mutants/zero survivors, actual native class
matrix and the checked Lab Wasm build pass. This is not a rest admission or
completion API: sleep/activity/interruption/cooldown, HP/ability restoration,
Exhaustion and world controls remain required. The [remaining contract](long-rest-implementation.md)
records the implementation sequence. No issue closes on this layer.

### Class and subclass features, levels 1–6 (#13044)

`aethyr/rules/srd/class-features.coax` lists every class-table and subclass
feature the twelve presets gain at levels 1–6, with its printed page, and holds
each to the built preset at every level: present from its level, absent before.
Each feature then resolves against its text with fixed dice in
`class-<class>-checks.coax`.

| Class (subclass) | Features checked | Status |
| --- | --- | --- |
| Barbarian (Berserker) | Rage, Unarmored Defense, Weapon Mastery (2, 3 at 4), Danger Sense, Reckless Attack, Primal Knowledge, Frenzy, ASI, Extra Attack, Fast Movement, Mindless Rage | Done |
| Fighter (Champion) | Fighting Style (Defense), Second Wind (2, 3 at 4), Weapon Mastery (3, 4 at 4), Action Surge, Tactical Mind, Improved Critical, Remarkable Athlete, ASI at 4 and 6, Extra Attack, Tactical Shift | Done |
| Rogue (Thief) | Expertise, Sneak Attack, Thieves' Cant, Weapon Mastery (Vex, Nick), Cunning Action (Dash, Disengage, Hide), Steady Aim, Fast Hands, Second-Story Work (climb Speed), ASI, Cunning Strike (Trip, Withdraw), Uncanny Dodge; the Hide action and Fast Hands in `hide.coax` | Done; Second-Story Work's jump needs a jump model, and Hide is not on the Experimental bar until the world supplies concealment |
| Monk (Open Hand) | Martial Arts, Unarmored Defense, Monk's Focus (Flurry, Patient Defense, Step of the Wind), Unarmored Movement, Uncanny Metabolism, Deflect Attacks, Open Hand Technique, ASI, Slow Fall, Extra Attack, Stunning Strike, Empowered Strikes, Wholeness of Body; `monk-features.coax` | Done; Step of the Wind's doubled jump needs a jump model |
| Paladin (Devotion) | Lay On Hands, Spellcasting, Weapon Mastery (Sap, Slow), Fighting Style (Blessed Warrior: Sacred Flame, now on the level 2 bar), Paladin's Smite, Channel Divinity (Divine Sense, in `paladin-features.coax`), Oath of Devotion Spells (Shield of Faith), Sacred Weapon, ASI, Extra Attack, Faithful Steed (its use), Aura of Protection | Done; Blessed Warrior's second cantrip, Protection from Evil and Good, Aid, Zone of Truth and Find Steed aren't spell records yet |
| Ranger (Hunter) | Spellcasting, Favored Enemy, Weapon Mastery (Vex, Slow), Deft Explorer (Survival Expertise), Fighting Style (Archery), Hunter's Lore (in `ranger-features.coax`), Hunter's Prey (Colossus Slayer), ASI, Extra Attack, Roving | Done; Deft Explorer's languages are the world's |
| Cleric (Life) | Spellcasting, Divine Order (Protector), Channel Divinity (Divine Spark, in `cleric-features.coax`, and Turn Undead), Life Domain Spells (Bless, Cure Wounds; Mass Healing Word at 5), Disciple of Life, Preserve Life, ASI, Sear Undead, Blessed Healer (now told as its own feature) | Done; Aid, Lesser Restoration and Revivify aren't spell records yet, and the battle has no weapon or armor training model for Protector |
| Bard (Lore) | Bardic Inspiration (d6, d8 at 5), Spellcasting, Expertise, Jack of All Trades, Bonus Proficiencies, Cutting Words, ASI, Font of Inspiration, Magical Discoveries (Fireball, Spirit Guardians) | Done |
| Wizard (Evoker) | Spellcasting (a book of six level 1 spells and two a level), Ritual Adept, Arcane Recovery, Scholar (Arcana Expertise), Evocation Savant (two free Evocation spells at 3, one more at 5), Potent Cantrip, ASI, Memorize Spell (in `wizard-features.coax`), Sculpt Spells | Done; Ritual Adept casts a spellbook Ritual through `ritual-cast.coax` (#13038) |
| Warlock (Fiend) | Eldritch Invocations (Armor of Shadows; Agonizing and Repelling Blast at 2; Fiendish Vigor and Eldritch Mind at 5), Pact Magic, Magical Cunning (in `warlock-features.coax`), Dark One's Blessing, Fiend Spells, ASI, Dark One's Own Luck | Done |
| Sorcerer (Draconic) | Spellcasting, Innate Sorcery, Font of Magic (both conversions, in `sorcerer-features.coax`; a created slot vanishes on a Long Rest), Metamagic (Quickened, Seeking), Draconic Resilience, Draconic Spells (Command), ASI, Sorcerous Restoration, Elemental Affinity (Fire) | Done |
| Druid (Land, polar) | Spellcasting (a third cantrip at 4), Druidic (Speak with Animals always prepared), Primal Order (Warden), Wild Shape (in `wild-shape.coax`: a Bonus Action into a known Beast, Temporary Hit Points of its level, the Beast's Strength, Dexterity, Constitution, AC, Speed, size, traits and attacks, no spellcasting, half its level in hours, ending on Incapacitated, death, re-use or a Bonus Action; four forms at 2 and six at 4: Rat, Riding Horse, Spider, Wolf, Black Bear, Warhorse, with Cat available as a Long Rest replacement), Wild Companion (shared Fey familiar executor), Circle of the Land Spells, Land's Aid, ASI, Wild Resurgence, Natural Recovery; the kit's Wild Shape and Attack slots from level 2 | Done; Wild Companion has shared execution and player controls in both Everglade worlds; the Warden's Martial weapon and Medium armor training has no model; a form's Opportunity Attacks use an Unarmed Strike; the Warhorse's charge and the Beasts' Swim and the Rat's Agile aren't run |

### Used-monster stat blocks (2026-11-08)

Every monster the worlds field now fights as its SRD 5.2.1 block. Experimental's
imp, gremlin, Puglin Brute, Imp Fiend and Puglin Warlord are the Imp, Goblin
Warrior, Ogre, Quasit and Hill Giant. Everglade 5E's Conjure Animals wolves and
Finger of Death's risen are the Wolf and the Zombie. The raid and watch use the
npcs.coax blocks, and Everglade II fields no monsters. The shared engine now
runs size and Large reach, Pack Tactics, Magic Resistance, Nimble Escape,
ranged and thrown stat attacks, second damage, the Goblin's Advantage rider,
Prone and Poisoned riders, and Multiattack. Six authority scopes kill
54/204/85/45/70/86 mutants with zero survivors and no exemptions.
[Evidence](../verification/srd-used-monsters-2026-11-08/README.md) lists the
block parts no world uses and the house mappings that remain.

### Knockout, Healer's Kit and condition owners (2026-11-08)

A melee attack that would drop a creature to 0 can now knock it out instead
(SRD 5.2.1 pp.17, 185): it stays at 1 HP, Unconscious and Prone, under its own
`EF_KNOCKOUT` record, and starts a Short Rest. The condition ends when that rest
finishes, on any healing or on successful first aid; massive damage, Monster Death
and Undead Fortitude don't apply. A Healer's Kit (p.97) stabilizes with no check.
A Charmed creature can't use the Attack action on its charmer, a Grappled one has
Disadvantage against anyone but its grappler, and a Petrified one is immune to
Poisoned. Every condition's owner in the slice is tabled and checked, and the
source's own HP, Temporary HP, massive damage, death-save and damage-order examples
run as fixtures. Experimental has a KNOCK OUT toggle under REST. Eight authority
scopes kill every mutant, no exemptions.
[Evidence and what's left](../verification/srd-knockout-conditions-2026-11-08/README.md).

### Rituals, sight-required targets, objects and hotbar choices (2026-11-08, #13038)

A prepared Ritual spell (or, for a Wizard, one in the spellbook) can now be cast
as a Ritual: ten more minutes of Magic actions under Concentration, then its
normal action at slot 0, spending no slot (SRD 5.2.1 pp.78, 104-105, 179, 188).
A missed turn, failed Concentration, incapacitation, Rage or a new Concentration
spell fails it with nothing spent. Spells that choose "a creature (or point) you
can see" refuse an unseen choice before costs; Vicious Mockery also accepts a
heard creature. Both lists are compared with every executable description.
`ritual.coax`, `cast-sight.coax` and the changed `concentration.coax` kill
25/50/89 mutants, zero survivors.
Eldritch Blast, Fire Bolt, Produce Flame's hurl and Starry Wisp can now attack
an object, which uses its substance's AC, its size's Hit Points, Poison and
Psychic Immunity and any damage threshold; Fire Bolt sets an unattended
flammable object burning (pp.177-180). Every level 1-6 preset's always-prepared
spells and components are checked against their sources. Experimental's hotbar
marks each spell's slot level and option on its icon and sends the player's
target. `object-target.coax` kills 45/45 mutants.
[Evidence](../verification/srd-casting-workflows-2026-11-08/README.md).

### World profiles, paid continuous walking and turn ordering (2026-11-08, #13045)

Each world now declares its rules deviations in one `rs_set_world` call
(`world-profile.coax`): timing, area origin, press budget and walking, applied
all or nothing before the clock starts and named by `rs_world_deviations`.
Experimental, the 5E room and Everglade II use `RS_EVERGLADE_WORLD`; single 5E
play, the MMO and the Lab's action mode use `RS_ACTION_WORLD`; removing the
profile gives strict results. Under strict walking (`walk-budget.coax`) a
physical walker pays each foot from the turn's movement, two in Difficult
Terrain or crawling and three for both, plus Dash, and is cut off where
movement runs out (pp.14, 179, 180, 188). Turn boundaries, "until the end of
your next turn", round/minute/hour durations and simultaneous end-of-turn saves
are checked in strict and action timing; every executable spell's timed
duration matches the pinned source. The engine orders simultaneous effects
itself (p.187 gives the choice to the turn's owner). Both new authorities kill
every mutant (60 and 80), no exemptions.
[Evidence and limits](../verification/srd-world-profile-2026-11-08/README.md).

### Actions, cover, visibility, grappling and movement modes (2026-11-08, #13040)

Ready holds one attack, a move up to Speed or a spell cast when readied and held
by Concentration; several readied answers fire in the turn owner's order
(pp.184, 188). Search, Study, Influence and Utilize spend the action on the
checks the glossary names. Cover is a world fact or a creature between: Half
and Three-Quarters add 2 and 5 to AC and Dexterity saves, and Total Cover
refuses attacks, stat-block attacks and targeted spells and spares a creature
from an area (p.15). Heavily Obscured squares make attackers and targets
unseen, and Hide needs concealment from every enemy. Grapple and Shove replace
an attack of the Attack action, with escape, lapse and dragging. Climbing and
swimming cost 5 extra feet without that Speed; Long and High Jumps follow the
glossary. Queued Short Rest choices apply in the controller's order when nobody
is there to choose. Experimental's bars end with HIDE, GRAPPLE, SHOVE and READY.
The sixteen new or changed authorities kill all 2,268 mutants, zero survivors.
[Evidence and limits](../verification/srd-actions-movement-2026-11-08/README.md).

### Experimental controls for Rituals, Ready, Shove and the skill actions (2026-11-08, #13030)

Experimental's bars now play the engine-only actions on their icons, with no
banners. READY's bottom strip picks ATTACK, MOVE or SPELL (the kit's first
action spell at a creature, cast and held when readied); the first monster that
moves sets it off, and a readied move carries you straight away from it up to
your Speed (p.186). SHOVE's strip picks PRONE or PUSH, and a push moves the
monster 5 feet through the world, stopping at a wall (p.181). One SEARCH slot
takes Search (Perception, DC 15), Study (the target's Area of Knowledge, p.188:
Religion for Fiends, Arcana for Fey, History for Giants; a pass shows its AC)
and Influence (Intimidation, Hesitant and Hostile; a monster that complies
stands down until harmed, and a failure waits 24 hours); its frame flashes
green or red with the result. A class with a prepared ritual spell (the Druid's
Speak with Animals) gets a RITUAL slot: begun with a press, kept going
each turn, its ten minutes filling the icon's edge, then cast with no slot
(pp.104-105). Checks: five new Experimental checks; 91 Experimental, 448
fivebar and 414 class-kit checks pass.

### Area origin cover, Charmed and Frightened limits, knockouts in Reactions (2026-11-08, #13030)

An area cast at a point (a Sphere, Cube or square at range) reads its cover
from that point, not the caster: the world states cover from
`RS_COVER_ORIGIN`, and Experimental draws it from the walls between the point
and each monster (p.15). Cones, Lines, Self cubes and Emanations still read it
from the caster. A Charmed creature can't target its charmer with a spell or a
stat-block attack, though an area still reaches it (p.177). Frightened gives
Disadvantage on attacks and ability checks only while a living source is in
line of sight, and refuses a willing step closer to it; a raw bit with no
source keeps the old always-in-sight reading (p.182). A knockout an Opportunity
Attack deals starts its Short Rest when the Reaction ends (p.17). Checks:
actions 416, condition-roll 427, knockout 419.

### Experimental placed objects (2026-11-08, #13030)

Three independently tracked supply crates near spawn now use the shared
object spell executor for compatible aimed bolts without a creature target.
Wood AC/HP, typed damage, ignition and six-second burning remain engine rules;
destruction removes the instance and movement collision. Existing baked props
remain scenery. The current registered crate mesh is reused without a pack
change, with a Coax verifier binding its captured geometry range. See
[the receipt](../verification/srd-placed-objects-2026-11-08/README.md) for
provenance, focused verification and remaining world limitations.

### Everglade II Healer's Kit (#13030)

The room gives each new seat one kit and preserves its charges through class
changes and grace rejoins. The hotbar's Healer's Kit control requests first
aid for the closest dying ally within 5 feet. The room checks the forest's
movement colliders and calls the shared Utilize action in the rescuer's saved
turn. A successful use leaves the ally Stable at 0 HP and reports the remaining
charges. The room regression puts a second player at 0 HP through shared damage
and exercises range, obstructed contact, replay refusal and success. Page
checks exercise the normal click, reply matching and disconnect handling.

### Experimental Fireball origin cover regression (#13030)

A level 5 Wizard casts Fireball past the dungeon's north wall through the
Experimental hotbar adapter. An enemy visible from the blast's origin loses
HP even though the caster cannot see it. A second enemy within the blast's
radius keeps its HP because the wall is between it and the origin. The test
fails when the adapter publishes caster cover in place of origin cover.

### Experimental Strength-based jumps (#13030)

Space makes a High Jump with no directional input and a Long Jump with
directional input. Experimental uses the saved turn's shared jump score,
running-start and Step of the Wind rules. A running start requires 10 feet of
accepted grounded movement. The controller uses the shared reach and available
movement to shape the jump. The shared engine charges the planned distance on
an admitted takeoff. The ordinary walker resolves physical contact and landing.
The jump adapter keeps the action world's existing movement budget. Its
standing and running distances can therefore differ from a strict six-second
turn.

Checks cover a standing and running start, Strength, the current budget cap,
movement payment on takeoff, Prone refusal and actual fixed-step trajectories.
All nine selected Experimental checks and 51 existing combat checks passed.
In muted headless WebGPU at 1280×900, Space raised the level 1 Wizard by
275 mm and the Barbarian by 862 mm. Space with W moved them 1,219 mm and
2,438 mm respectively from a standing start. The Lab Wasm built at
2,644,940 bytes against the 2,645,000-byte cap. The earlier core movement
mutation receipts still apply.

The shipped Experimental terrain supplies no climb or swim surfaces. The
shared engine covers those modes for worlds that supply the terrain facts.

### Wild Shape replacements at Long Rest (#13030)

A completed Long Rest grants one known-form exchange to a Druid with Wild
Shape. The Cat from SRD 5.2.1 p.346 gives both the four-form and six-form
presets a legal alternative. The known mask filters choices by class and
level. Failed attempts preserve the choice and the mask. The Experimental
rest panel chooses an outgoing and incoming form and applies the exchange.
The Wild Shape hotbar badge cycles the known forms. The exchange selects the
new form immediately, and reselecting the same Druid preset keeps the mask.

The shared fixture completes a Long Rest, exchanges a form, uses the Cat's
physical stats and Jumper trait, leaves the form, and exchanges back after a
second rest. The Experimental fixture clicks the actual rest controls and
uses the hotbar executor. Changed core authority units killed 58 known-form,
126 transformation, and 37 rest-completion mutants with zero survivors.
The fivebar, class-kit and selected Experimental checks passed.

Lab Wasm measures 2,647,552 bytes. Its cap rose by 5,000 bytes to 2,650,000
for the Cat record, known-form state and controls. The swap control is on the
Experimental page. Everglade II shares the rules state and Long Rest hook.
In muted headless WebGPU, the level 2 Druid cycled the badge from Wolf to
Rat and used the slot. The page recorded one use, zero refusals, and showed
the selected Rat and two Temporary Hit Points.

### Shared familiar actors and Wild Companion (#13030)

The shared Wild Companion entry spends the Druid's Magic action and the
chosen spell slot or Wild Shape use. It retains Verbal and Somatic checks
and omits Material components. A legal unoccupied square within two grid
steps admits one Fey familiar per owner. The catalog and Experimental
picker represent the eleven named forms, including Bat, Cat, Frog, Hawk,
Lizard, Octopus, Owl, Rat, Raven, Spider and Weasel. Recasting with a
different form changes the ally in place, retaining its current HP up to
the new maximum and its initiative.
The actor leaves the grid at 0 HP or when its owner completes a Long Rest.
A familiar can take non-attack actions. Shared weapon, stat-block, Unarmed
Strike, Ready Attack and Opportunity Attack paths refuse its attacks.

The Wizard's prepared cast takes one hour, while Ritual Adept can cast it
from the spellbook in seventy minutes. Both require Concentration and a
Magic action each turn. On successful completion both consume incense, and
the prepared cast also spends its selected slot. Each named form can be
Celestial, Fey or Fiend. The regular familiar persists across Long Rests.
A Magic action dismisses the familiar temporarily or permanently. Another
Magic action returns a temporarily dismissed familiar within 30 feet.
The shared casting path lets a familiar within 100 feet deliver Cure Wounds,
Chill Touch and Inflict Wounds with its Reaction. Touch reach and cover use
the familiar's position. The spell's resource costs and statistics come from
the owner. The shared command path admits telepathic orders within 100 feet
or audible orders through a world fact. It routes steps through ordinary
movement and Opportunity Attack resolution. A Bonus Action borrows sight
and hearing until the owner's next turn. Casting sight checks can use either
viewpoint. The represented forms expose their source darkvision range for
world adapters. Familiar gear and the Everglade II controls remain open.

The native witness passed the spell-slot path, attack refusal and Long Rest
retirement. Scoped mutation runs killed 1,221 mutants with zero survivors
across familiar, Long Rest completion and the five changed attack admission
units. The Lab Wasm built at 2,648,233 bytes within its 2,650,000-byte cap.

The timed casting slice passed 427 workflow checks and the native completion
witness. The familiar and cast-request authority units killed 343 mutants
with zero survivors. The generated spell index matches the pinned catalog,
and Lab Wasm built at 2,648,987 bytes against the 2,650,000-byte cap.

The dismissal lifecycle passed a native witness and 375 familiar authority
mutants with zero survivors. Experimental Wasm remained at 2,648,987 bytes.

Touch delivery passed the casting target, workflow and class-kit suites and
a native witness. The touch admission unit killed 25 mutants with zero
survivors. Experimental Wasm built at 2,647,369 bytes.

The shared familiar command and senses changes passed 434 casting workflow
checks and a native witness. The three changed authority units killed 146
mutants with zero survivors. Experimental Wasm built at 2,647,739 bytes.

Experimental now exposes Wild Companion at Druid level 2 and Find Familiar
as a Wizard spellbook ritual. The hotbar cycles the eleven named forms. The
Druid can spend a Wild Shape use or a level 1 spell slot. The Wizard has one
incense supply per visit, tracked across class changes. The familiar has a
world position, a projected marker and an independent turn seat. The hotbar
fixture covers summoning, turn refresh, projection, ritual continuation and
payment. All 51 existing Experimental combat checks passed. Lab Wasm is
2,653,366 bytes against the revised 2,655,000-byte cap.

Experimental's Wizard preset prepares Find Familiar in place of Mage Armor
without changing the cantrip or preparation counts. Its top-right badge
switches between the seventy-minute ritual and the one-hour prepared cast.
The player can choose a Fey, Celestial or Fiend from the bottom-right badge.
The hotbar's top-left badge cycles Cast,
Pocket or Recall, Free, and Step. Pocketing and recall preserve HP and
initiative. Step uses the familiar's movement, and a hostile Opportunity
Attack can stop its departure. The owner chooses the direction by facing
it. Experimental's Touch badge switches between Hand and Fam. Fam
delivers Chill Touch to the clicked foe using the familiar's reach, cover,
and Reaction. It uses the owner's saved action and spell resources.
Sense spends the owner's Bonus Action to move the camera to the familiar's
eye and spatial combat audio to its position. In the dungeon, the form's
darkvision lights nearby darkness in gray. Additional command choices,
animal models, and Everglade II integration still need work.

Three familiar world fixtures and all 51 existing Experimental combat
checks passed. A muted headless WebGPU playthrough ran Summon, Pocket,
Recall, Step and Free from the hotbar. Lab Wasm is 2,655,903 bytes against
a revised 2,657,500-byte cap.

The Experimental Touch fixture exercised the badge and hotbar press against
a clicked creature beyond the Wizard's own reach. Missing or distant
targets and a spent familiar Reaction refused without taking another
action. The 51 existing combat checks passed. Lab Wasm built at 2,659,260
bytes against a revised 2,661,000-byte cap.

The prepared-cast fixture covers levels 1–6, a canceled cast with no
payment, and completion with one slot and one incense supply spent. It
also checks that changing the badge during the cast preserves the pending
cost. The 51 existing Experimental combat checks passed, and muted
headless WebGPU verified starting a prepared Celestial cast. Lab Wasm is
2,660,451 bytes against the unchanged 2,661,000-byte cap.

The familiar view fixture covers camera origin, darkvision range, combat
cue panning, Blinded and Deafened fallback, and turn expiry. Five familiar
fixtures, 51 combat checks and 179 shader checks passed. Both affected WGSL
modules match their Coax sources. The site-wide checker found no stale
modules and could not lower two unchanged compute modules with the local
compiler. Muted headless WebGPU showed the familiar's low viewpoint.
Lab Wasm built at 2,660,961 bytes against the 2,661,000-byte cap.

The new stat rows come from SRD pp. 345, 348–349, 355, 357–359 and 364.
They preserve armor class, hit points, ability scores, the printed attack,
movement and special-sense distances. Frog and Octopus use the shared Swim
Speed where terrain supplies swimming. Darkvision comes from the catalog.
Flight, Bat blindsight behavior, remaining creature traits and skill
bonuses, and other eligible CR 0 Beasts still need work.

The named-form fixture covers summoning and reforming all eleven, including
the attack restriction and resetting Swim Speed on a change of form. All
478 monster checks passed. Familiar and senses mutation gates killed all
431 mutants. The Owl picker passed a world fixture and a muted headless
WebGPU click. Lab Wasm is 2,662,702 bytes against a 2,664,500-byte cap.

Everglade II's level 2+ Druid Companion control selects a named form and a
Wild Shape use or level 1 slot. The room derives the destination, asks its
collision world for clearance, and resolves the request through the
shared saved turn. The reserved familiar actor has its own turn and
expires with the owner's Long Rest. The first room control reports
success or refusal to the owner. The room now replicates visible familiar
positions and Hit Points, and the page shows the form name and health at
that position. A block or the end of the familiar removes its marker.
Animal models and the remaining room actions are tracked below.

Nine focused room checks and 21 Everglade II page checks passed. The
protocol fixture covers obstruction, replay, the saved action, and Long
Rest expiry. The page fixture verifies the selected form and payment and
ignores a stale reply. Everglade II Wasm built at 704,767 bytes.

The replication fixture checks owner and peer delivery, block filtering and
Long Rest removal. The client fixture checks replacement, stale and
malformed records, and marker drawing and removal. All 22 focused
Everglade II page checks passed. Wasm built at 705,466 bytes.

Everglade II now offers both Wizard Find Familiar casting modes and the
three spirit choices. The room continues the cast with the owner's saved
Magic action, derives a clear position on completion and publishes the
familiar through the room snapshot. The preset provides one incense
supply per seat visit. The room fixtures cover the ritual, the paid cast,
and blocked completion preserving both incense and the spell slot. The
page fixture verifies the mode and spirit request and the room's status
messages.

Everglade II now exposes Pocket, Recall and Release to Druids and Wizards.
The room checks ownership and the saved Magic action. Pocket and recall
preserve Hit Points, and an obstructed recall or replay spends no action.
Release retires the familiar. The room fixture exercises the full sequence
and the page fixture verifies request encoding and matching acknowledgments.

Everglade II's Step button now asks the room to move the familiar one
square in the direction the owner faces. The room checks clearance from
the familiar's position and uses its saved turn and movement budget.
The room checks hostile NPC sight separately from path clearance and
resolves an available Opportunity Attack before departure. An attack that
destroys the familiar stops the step before it spends movement.

Everglade II extends its combat grid south through the distant clearing.
The room converts placement, casts, NPCs, effects, collision queries and
familiar snapshots at the world boundary. The distant-clearing regression
checks summoning, Step, collision endpoints and replicated coordinates
near −470 m. The original Everglade grid keeps its existing origin.

Everglade II now offers Druids familiar-delivered Cure Wounds. The
control selects a nearby wounded player, and the room checks contact,
visibility permissions and reach before using the shared touch executor.
The Druid's Magic action and first-level slot and the familiar's Reaction
are spent on success. The owner hears the existing Cure Wounds cue at
the familiar after the room confirms the cast. Everglade II's social
profile still treats offensive casts as untargeted.

Everglade II's Sense control spends the owner's saved Bonus Action and
uses the shared familiar link. The room reports the borrowed viewpoint
and hearing state to the owner. The camera uses the familiar's position,
and spatial spell cues use it when the familiar can hear. The camera
returns when the link ends or the owner's next turn starts. The outdoor
view uses its existing lighting. Special-sense lighting and animal
presentation remain separate work.

The eleven named familiar forms now have original low-poly geometry in
Coax. Both world pack builders use the same mesh writer and stable form
order. The geometry check verifies distinct bounded meshes for every form.
Both scene renderers now draw the admitted catalog model at the familiar's
world position and hide it in its owner's borrowed viewpoint. The draw
fixtures cover Experimental's Owl, room replication at the distant
clearing, viewpoint hiding and removal. The shipped packs now include all eleven
models. Visual verification in both worlds remains open.

The two shipped familiar packs were updated using the Coax pack builder's
`--append-familiars INPUT.pack OUTPUT.pack` mode. Existing mesh and index
prefixes and all other asset payloads match byte for byte. The catalog has
one index range per form and records the new vertex base. Everglade II's
range-addressable pack also round-tripped to the updated image.

### Epic #13030 acceptance audit in progress

Reviewed against main at `03bda2786f`. All nine active child issues are
closed as completed. The epic's five acceptance items need their own
evidence at the levels 1–6 scope.

| Acceptance | Evidence retained | Outstanding verification or implementation |
| --- | --- | --- |
| Legal presets, features, and supported spells for all 12 classes | [Kit admission](../verification/srd-kit-admission-2026-11-08/README.md) covers all 72 class and level combinations. [Kit spell manifest](../verification/srd-kit-spells-2026-11-08/README.md) binds displayed spells to the pinned source and checks their effects and sound identities. | The class feature table above still lists missing preset mechanics and features, including Faithful Steed's spell, several always-prepared spells, and some equipment and beast traits. Verify these against current code and the slice before marking the item complete. |
| Core casting, conditions, damage, death, actions and reactions | [Casting targets](../verification/srd-casting-target-2026-11-08/README.md), [effect capacity](../verification/srd-effect-storage-2026-11-08/README.md), [zone capacity](../verification/srd-zone-storage-2026-11-08/README.md), [conditions and knockout](../verification/srd-knockout-conditions-2026-11-08/README.md), and [actions and movement](../verification/srd-actions-movement-2026-11-08/README.md). | Confirm the combined world paths exercise the applicable core outcomes and retain the latest authority receipts for changed rules. |
| Named used monsters, bestiary and dungeon | [Used-monster evidence](../verification/srd-used-monsters-2026-11-08/README.md) lists the consumers, source stat blocks, focused checks and mutation counts. The eleven familiar model ranges are admitted by both shipped packs. | Verify actual familiar draws through both player-facing worlds. The familiar catalog section above records remaining traits and movement modes. |
| Shared execution and declared world profiles | [World profile](../verification/srd-world-profile-2026-11-08/README.md), [Everglade II cast adapter](../verification/srd-e2-shared-cast-2026-11-08/README.md), and [Everglade II level selector](../verification/srd-e2-levels-2026-11-08/README.md). | Complete current browser and room verification, including model delivery and the remaining sense presentation. Scope the evidence to Experimental combat and Everglade II's declared social profile. |
| Retained focused evidence and zero-survivor core receipts | The verification directory contains the policy, native and mutation evidence grouped by rule. | Tie final receipts to the accepted source revisions and list the slice limitations that remain. |
