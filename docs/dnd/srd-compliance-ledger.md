# SRD 5.2.1 compliance ledger

Status: milestone 1 baseline, 2026-11-07. **Not fully compliant.** This ledger
covers every area required by [the specification](README.md) and assigns stable
requirement IDs. It is a requirements inventory, not yet an exhaustive inventory
of the PDF's individual entries or mechanical clauses. That reconciliation is
milestone 2. The [roadmap](srd-compliance-roadmap.md) defines all eight milestones.

## Source identity

| Field | Pin |
| --- | --- |
| Source | System Reference Document 5.2.1, Wizards of the Coast LLC |
| Owner-local file | `~/Desktop/SRD_CC_v5.2.1.pdf` (not required at runtime) |
| Bytes | 6,031,375 |
| SHA-256 | `8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87` |
| Page references | Printed SRD page numbers; these match this PDF's page numbers |
| Source inventory | Unreconciled; no total spell/monster/item compliance denominator yet |
| Baseline code | `f7cab6d3a4ef98752ecb7d7cb834a90ea9af7bbc` |
| Evidence | [Milestone 1 verification](../verification/srd-compliance-m1-2026-11-07/README.md) |

Changing the source hash is a deliberate source-version update, followed by
inventory and golden-fixture review. This pin establishes identity, not correct
extraction or implementation. No PDF or extracted full text is copied into Git.

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
| SRC-PIN | Spec Legal and naming; SRD p.1 | Partial | Hash and attribution above | Tool-enforced identity and intentional version updates, M2 |
| DATA-INVENTORY | Spec Import check | Missing | Runtime record inventory linked below | Independent PDF lists, stable source IDs, exact reconciliation, M2 |
| DATA-IMPORT | Spec SRD content | Missing | Handwritten `spells`, `more`, `npcs` | Coax importer, generated tables, deterministic regeneration, M2 |
| DATA-METADATA | Spec spell record shape | Partial | `RsSpell` fields; later records have empty text/components and page 0 | Structured complete metadata, full text, source references, M2 |
| CORE-DICE | Playing the Game pp.5-8 | Partial | `engine` SplitMix64, checks, `dice are fair and repeatable` | Rule inventory and independent deterministic vectors, M3 |
| CORE-CHECKS | D20 Tests, proficiency pp.6-9 | Partial | Abilities/save proficiency; selected skill flags | Full skill/tool checks, expertise, passive checks and interactions, M3 |
| CORE-ACTIONS | Actions/combat pp.9-18 | Partial | `RsTurn`, budgets, attacks and selected reactions | Complete actions, triggers, ready/interrupt/order semantics, M3 |
| CORE-MOVEMENT | Movement and Position pp.14-15 | Partial | Grid movement, difficult terrain, pushes | All movement modes, sizes, reach, grapple and geometry contracts, M3 |
| CORE-VISIBILITY | Playing the Game and Rules Glossary | Partial | Selected condition-based attack modifiers | Cover, hiding, obscuration, perception and targeting integration, M3 |
| CORE-CONDITIONS | Rules Glossary pp.176-191 | Partial | All 15 named bits; selected enforced behaviors | Each condition's clauses, Exhaustion levels, stacking/lifecycle, M3 |
| CORE-HP | Damage/healing/death pp.15-18 | Partial | `rs_hurt`, `rs_heal`, `rs_death_save`; embedded checks | Full interaction and source-clause fixtures, M3 |
| CORE-REST | Rules Glossary, class recovery rules | Partial | Uses/maxima and selected recovery features | Short/long rests, Hit Dice, limits and recovery effects, M3/M5 |
| CAST-ACCESS | Classes and Gaining Spells pp.104-105 | Partial | `rs_can_cast_as` checks budget/slots; `CAST-CLASS` fails | Legal known/prepared lists, feature grants and strict admission, M3/M5 |
| CAST-COMPONENTS | Casting Spells pp.104-106 | Missing | Display strings, no complete component state | V/S/M, focus/pouch, cost, consumption, rituals/long casts, M2/M3 |
| CAST-TARGETS | Casting Spells pp.105-106 | Partial | Range/type checks; `CAST-INVALID-TARGET` fails | Explicit selections; separate malformed refusal from spent invalid-target cast, M3 |
| CAST-RESOURCES | Spell slots/casting rules pp.104-106 | Partial | Slot checks and turn budgets; four admission probes pass | Preparation, full free-cast restrictions, transactional preflight, M3 |
| CAST-CONCENTRATION | Rules Glossary Concentration | Partial | Concentration saves, replacement and expiration | Full interruption, overlapping effects and duration fixtures, M3/M4 |
| FX-EXECUTOR | Spec typed effect program | Partial | Closed step set with specialized handlers | Transform, summon, sense, general modify/terrain/light/narrative, M4 |
| FX-TARGETING | Spec target step | Partial | Single target plus automatic nearest-side selection; same target per ray | Explicit legal multiple targets, filters and geometry, M3/M4 |
| FX-ZONES | Spec zone triggers and shapes | Partial | Eight zones; Lines/Spheres and rectangular approximations | True shapes, enter/start/end timing, movement and multi-area spells, M4 |
| FX-CAP-EFFECT | Spec lasting effects; deterministic complete results | Partial | Six effect slots; `CAP-EFFECT` fails | No silent replacement/loss, explicit capacity outcome and stacking, M3 |
| FX-CAP-ZONE | Spec lasting areas; admitted cast results | Partial | Eight zones; `CAP-ZONE` fails | Reserve before spending, retain existing concentration/effects on refusal, M3 |
| CONTENT-SPELLS | Spell Descriptions pp.107-175 | Partial | Handwritten candidates/helpers; selected execution tests | Reconciled all-spell inventory, complete text and every mechanical clause, M2/M4 |
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
five desired outcomes are unmet. These are executable regression predicates,
not `#test` assertions that bugs should remain. The first command's ordinary
checks guard working boundaries and free-capacity controls. Fixes change a
predicate to PASS without deleting an expected-failure exemption.

| Probe ID | Desired outcome | Baseline | Scope |
| --- | --- | --- | --- |
| CAST-ACTOR | Missing/negative actor refuses without spending | PASS | Request index validity |
| CAST-SLOT | Unavailable slot refuses without spending | PASS | One selected slot boundary |
| CAST-TARGET-INDEX | Missing/negative target index refuses without spending | PASS | Malformed request, not SRD invalid-target rule |
| CAST-RANGE | Out-of-touch-range Cure Wounds refuses without spending | PASS | Range admission |
| CAST-CLASS | Unmodified preset Druid cannot cast Guiding Bolt | FAIL | Missing strict class/preparation admission; no feature grant or caller whitelist is supplied |
| CAST-INVALID-TARGET | Hold Person on a Skeleton spends its used slot without affecting it or rolling a save | FAIL | SRD p.106 Invalid Targets and p.141 Hold Person |
| SPELL-POLYMORPH | Failed save grants form temp HP, retains HP/type, does not merely Incapacitate | FAIL | Necessary witness only; current API cannot select a Beast, so even a future PASS would not qualify the full spell |
| CAP-EFFECT | Adding another effect never silently removes independent existing effects | FAIL | Direct storage boundary, not an alternative SRD stacking rule |
| CAP-ZONE | Full unrelated zones refuse before resources, concentration or effects change | FAIL | Engine capacity contract; no SRD claim about a maximum number of zones |

Refusal snapshots check slots, action/bonus/slot/leveled budgets, dice, event-log
counters, concentration and target HP/temp/conditions. The zone probe also checks
existing zone identity/bounds/duration. These are selected observable projections,
not a byte-for-byte proof of complete battle-state equivalence.

## Runtime record inventory

[spell-records.tsv](../verification/srd-compliance-m1-2026-11-07/spell-records.tsv)
lists every currently assigned record by stable textual ID, parent, kind, text
presence, page and effect-step count. `canonical-candidate` means a top-level
record marked SRD in the code, not independent confirmation that it matches the
source. Numeric holes are excluded. The report does not count helpers as spells
or house content as SRD. It reports metadata presence, not metadata correctness
or mechanical completeness. Source-level reconciliation remains open.

## Next implementation slice

M2: reconcile the pinned source inventory and generate structured metadata.
M3: add legal casting state and transactional storage admission, preserving the
p.106 invalid-target distinction. M4: introduce chosen-form transformations and
replace Polymorph's approximation. Keep these probe IDs and extend their source
fixtures as each prerequisite lands. Source access is available; no owner action
is required for this milestone.
