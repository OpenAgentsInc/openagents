# Shared ability checks and selected training

2026-11-08. Initial baseline `3009a8557b`, final integration base `287eb5b201`.
Partial #13039 and selected-training #13044 progress; neither issue is complete.
The containing commit and [source hashes](source-hashes.txt) identify the tree.

## Source and contract

Pinned [SRD 5.2.1](https://media.dndbeyond.com/compendium-images/srd/5.2/SRD_CC_v5.2.1.pdf),
SHA-256 `8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`:
checks/proficiency pp.6–9; Initiative p.13; Primal Knowledge p.29; Jack/Expertise
p.32; Lore p.35; Tactical Mind p.48; Athlete p.49; Deft Explorer p.59; Rogue
Expertise p.62; Scholar p.78; tools/variants pp.93–94; Blinded p.177;
Deafened/Exhaustion p.181; Frightened p.182; Incapacitated/Invisible p.184; Passive
Perception p.186. The public [Basic Rules glossary](https://www.dndbeyond.com/sources/dnd/br-2024/rules-glossary)
was also consulted; the pinned SRD remains the acceptance source.

`check-training.coax` declares eighteen skill and thirty-seven tool proficiencies,
retaining separate gaming-set/instrument variants. Internal `RsCreature.checks`
holds selected skill/Expertise/tool masks. Atomic replacement rejects unknown
bits and Expertise outside trained skills; it does not authorize a class grant.
Constructors now carry explicit selections, while the old Expertise count stays
for displays. This changes internal creature layout, with no protocol schema or
frozen Character Create save-key change.

`rs_ability_check` takes an ability, optional skill/tool, DC, optional Primal
Knowledge choice and authoritative task/sense/circumstance facts:

- Proficiency applies once. Skill Expertise doubles it; tools cannot add a
  second bonus. Trained skill plus trained tool grants Advantage. Jack applies
  only to an untrained skill without another proficiency bonus, not a bare
  ability/Initiative check. The requested ability supplies the modifier.
- Primal Knowledge substitutes Strength for its five skills only while the
  feature and Rage are active. Rage/Athlete, Poisoned, visible fear, Hex and
  Enfeeblement contribute applicable edges. Any Advantage/Disadvantage cancel.
- Sight/hearing-dependent checks automatically fail through Blinded/Deafened
  without dice or rescue spending. Other checks remain possible. Malformed
  requests/actor indices refuse before dice, events or resources.
- Ordinary results use the modified total, without natural-one/twenty special
  outcomes. Inspiration/Own Luck precede Tactical Mind under the existing
  automatic rescue policy. Tactical Mind's attempted d10 contributes even on a
  failure; its Second Wind use is spent only on success. The escape log reports
  that attempted total instead of discarding a failed extra die.
- Entangle escape uses this resolver. Initiative uses the shared Dexterity plan,
  condition/Athlete/Invisible edges and Exhaustion, without a skill bonus or
  failed-check rescue. Upstream initiative sources remain individually available
  through `condition-permissions.coax`, so adding Poisoned or fear cannot lose
  an Advantage that already canceled another Disadvantage. Bless/Bane/Aura
  saving-throw bonuses do not enter ability checks.
- Exhaustion reduces rolled check/Initiative totals once. Passive Perception
  uses ten plus its Wisdom (Perception) bonus and net ±5 edge, without dice or
  rescue spending. **Source interpretation:** Exhaustion reduces a roll when
  making a D20 Test; Passive Perception is a score used without making that
  check. The flat roll penalty is therefore kept separate from the check bonus
  and not subtracted from this passive score. This is an inference from
  pp.181/186, not an explicit separate ruling about that interaction.

## Deliberate class-only preset choices

| Preset / source page | Skills | Additional selection | Tools |
| --- | --- | --- | --- |
| Druid / 41 | Animal Handling, Nature | — | Herbalism Kit |
| Wizard / 77 | Arcana, History | Arcana Expertise from 2 | — |
| Warlock / 70 | Arcana, Religion | — | — |
| Barbarian / 28 | Athletics, Perception | Survival from 3 | — |
| Monk / 49 | Acrobatics, Athletics | — | Weaver's Tools |
| Bard / 31 | Acrobatics, Performance, Persuasion | Performance/Persuasion Expertise from 2; Lore adds Arcana/History/Insight from 3 | Flute, Lute, Lyre |
| Cleric / 36 | Insight, Medicine | — | — |
| Fighter / 47 | Athletics, Perception | — | — |
| Paladin / 53 | Athletics, Persuasion | — | — |
| Ranger / 57 | Perception, Stealth, Survival | Survival Expertise from 2 | — |
| Rogue / 61 | Acrobatics, Athletics, Perception, Stealth | Athletics/Stealth Expertise; Acrobatics/Perception added at 6 | Thieves' Tools |
| Sorcerer / 64 | Arcana, Persuasion | — | — |

The existing Bard level-nine Expertise count also has explicit Acrobatics/Arcana
choices and boundary fixtures; this does not qualify broader levels 7–20.
Unknown classes/monsters get no inferred training. Origins, other class/order
bonuses, monster skill import and full build/grant legality remain separate work.

## Evidence

- [Red](red.log): the Fighter, Bard and Rogue escape regressions fail before
  the fix. [Core](core.log): all 204 checks pass on the merged tree. Independent
  numeric-mask fixtures cover preset identities/boundaries; all constructors are
  checked through levels 1–6. Skill bonus ranks cover 18 skills × two levels ×
  six training combinations × two Jack states (432 combinations). Other cases
  cover every tool, malformed requests, senses, conditions, natural faces,
  Initiative, passive scores, Exhaustion and resource boundaries.
- [Training mutation](training-mutation.log): 129 killed, zero survivors.
  [Math](math-mutation.log): 125 killed. [Resolver](roll-mutation.log): 83 killed.
  [Initiative/movement permissions](permissions-mutation.log): all 17 killed.
  No equivalent exemptions. Survivors prompted removal of redundant bounds and
  positive edge counts, complete preset masks, returned-metadata assertions,
  single-bit/Initiative controls, empty-resource and independently seeded d10
  fixtures. Training policy/compiler bytes survive rebase unchanged, so that
  passing gate is reused. Math/roll units were gated after integrating upstream
  Exhaustion. After the last movement/permission rebase, changed math/permission
  units were gated again; the unchanged resolver receipt is reused. Fresh
  combined evaluator/native/world checks preserve both sides of each rebase.
- [Native](native.json), [text](native.log): all twenty-one root `#test` scenarios
  pass in ordinary emitted checked C at O0/O2; properties are not counted as
  native by this runner. [Original-source native property](native-property.json.gz),
  [summary](native-property.log): ABI 2 emits the skill-bonus property directly.
  All eighteen cases with 24 combinations each complete at O0/O2 under ASan/UBSan.
  The full record preserves source/baseline validation, artifact/compiler
  identities and progress. This receipt predates the last permission rebase;
  the selected property and `rs_check_plan` body/dependencies remain unchanged,
  so its passing result is reused. Plan-body SHA-256:
  `724385a17223178126b3dcd4d89e7df613a3be16928882ba1c156efd5d6b15fe`.
  Sanitizer scope is that pure property only.
- [Hotbar](hotbar.log): 212 checks pass. [Exhaustion](exhaustion.log): 233 pass.
  [Experimental](experimental.log): fifteen selected combat checks pass out of
  734 after the final hostile-permission integration rebase. All nine recorded
  owned source/compiler hashes remain unchanged across that rebase; the other
  passing scopes are reused. [MMO AI](mmo.log): actual Monk AI escapes DC 13 Entangle with its trained
  total of 13 and spends its Action. These are evaluator integrations, not
  rendered browser/phone or complete world qualification.
- [Tools check](tools.log) passes. No compiler change or new non-Coax tool.

Checks use 180-second limits, mutation/native 600 seconds:

```sh
coax/bin/coax test aethyr/rules/srd/ability-check-tests.coax --library
coax/bin/coax mutate aethyr/rules/srd/ability-check-tests.coax --library --unit aethyr/rules/srd/check-training.coax --jobs 4
coax/bin/coax mutate aethyr/rules/srd/ability-check-tests.coax --library --unit aethyr/rules/srd/ability-check-math.coax --jobs 4
coax/bin/coax mutate aethyr/rules/srd/ability-check-tests.coax --library --unit aethyr/rules/srd/ability-checks.coax --jobs 4
coax/bin/coax mutate aethyr/rules/srd/ability-check-tests.coax --library --unit aethyr/rules/srd/condition-permissions.coax --jobs 4
python3 coax/tools/native_checks.py aethyr/rules/srd/ability-check-tests.coax --out /tmp/oa-ability-native-final.json
python3 coax/tools/native_check_execution.py aethyr/rules/srd/ability-check-tests.coax --verification-abi 2 --only 'skill training bonus ranks' --cc 'cc -fsanitize=address,undefined -fno-omit-frame-pointer -fno-pie -no-pie' --keep /tmp/oa-ability-abi2-final --out /tmp/oa-ability-abi2-final.json
coax/bin/coax test aethyr/rules/srd/exhaustion-checks.coax --library
coax/bin/coax test apps/openagents-web/fivebar-test.coax --library
coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental combat:'
coax/bin/coax test apps/openagents-web/fivemmo-battle.coax --library --only '5E MMO: trained Monk AI'
coax/web/tool.sh tools-check
```

## Remaining acceptance and trust boundary

Task requirements, applicable usable tools, circumstance edges and fear visibility
are caller facts. The legacy open-grid Initiative/escape adapters treat fear as
visible unless Blinded; occlusion/fear-source geometry and alternate senses such
as Blindsight still need shared-world integration under #13040. The APIs do not
admit an Action, authorize training grants or prove equipment possession. They
assume coherent creature storage, legal scores/levels/resources and admitted
check contexts. Other class/order bonuses, item/grant provenance, progression/
rest/learning, higher-level features, stealth/social/tool actions, Help,
stabilization/recovery and remaining condition clauses stay open. No training
editor, new player/wire choice surface or explicit optional-rescue UI is added.
No owner action is required.

This work includes material from the System Reference Document 5.2.1 ("SRD 5.2.1") by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative Commons Attribution 4.0 International License, available at https://creativecommons.org/licenses/by/4.0/legalcode.
