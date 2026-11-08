# Shared saving throws across the existing combat executors

2026-11-08, baseline `399da4abbc`, rebased onto `969d260160`. The upstream change
moves an unrelated Lab Tab-target assertion before its foes die; it changes no
shared rules or selected Experimental checks. Recorded core/compiler hashes
match after rebase. Partial #13039 progress. The containing commit
and [source hashes](source-hashes.txt) identify the final source/compiler.

SRD 5.2.1 saving throws p.7, Bardic Inspiration p.32, Dark One's Own Luck p.75,
Bless/Bane, Concentration p.179 and the named spell/feature entries apply to
these saving throws. The pinned PDF and resolver's independent source fixtures
are recorded in [the preceding change](../srd-condition-rolls-2026-11-08/README.md).

Every existing ordinary saving-throw producer in `battle.coax` now delegates to
`rs_saving_throw`: Undead Fortitude, damage concentration, Hellish Rebuke,
Spirit Guardians, end-turn repeat saves, Deflect Attacks redirection, Stunning
Strike, Open Hand, Turn Undead and Land's Aid, alongside the already migrated
spell programs and Cunning Strike. They use the resolver's saved/automatic
outcome rather than reconstructing it from the numerical total. This supplies
the previously missing Bless/Bane and optional rescue behavior, save edges for
repeat saves, and no-roll automatic Strength/Dexterity failures.

Undead Fortitude refreshes its local creature copy after saving, before HP loss.
Otherwise the HP write could restore spent Inspiration/Own Luck. Tests check
both rescue resources and the retained boon record. No new rule formula,
protocol field or state layout is introduced; this is integration of the
preceding authority, whose exact policy/compiler bytes are unchanged. Its
64-mutant zero-survivor receipt is reused, not rerun. This is not a mutation
qualification of every unchanged legacy battle procedure or feature trigger.

## Evidence

- [Red](red.log): all eleven new scenarios initially fail against the baseline.
  The final redirection fixture subsequently raises the Monk's Wisdom to 16
  so both Bless and Inspiration are required, rather than Bless alone. That
  fixture correction does not change the missing-modifier failure in baseline.
- [Core](core.log): 163 checks pass, including all eleven new scenarios.
  Fixtures cover every migrated producer, exact dice counts, automatic save
  failure, held/consumed rescue state, normal feature spending, half damage and
  actual repeat-save retention. Both Strength Advantage and Dexterity
  Disadvantage repeat saves consume two d20s; automatic failure consumes none.
- [Native](native.json), [text](native.log): all eleven root scenarios pass on
  ordinary emitted checked C at O0/O2. This is not an ASan/UBSan or ABI-2 claim.
- [Casting](casting.log): 186 checks pass. [Hotbar](hotbar.log): 174 pass.
  [Experimental](experimental.log): ten selected combat checks pass out of 695;
  evaluator integration only, not browser/phone qualification.
- [Tools check](tools.log) passes; no new tool is added here.

Checks use 180-second limits; native uses 600 seconds:

```sh
coax/bin/coax test aethyr/rules/srd/saving-throw-checks.coax --library
python3 coax/tools/native_checks.py aethyr/rules/srd/saving-throw-checks.coax --out /tmp/oa-save-paths-native.json
coax/bin/coax test aethyr/rules/srd/cast-target-checks.coax --library
coax/bin/coax test apps/openagents-web/fivebar-test.coax --library
coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental combat:'
```

Death saves retain their separate natural-face/counter/healing path and do not
yet receive the general save bonuses. Optional voluntary failure/player rescue
choices, complete condition lifecycle/immunities, incapacitation concentration
loss, Exhaustion and remaining #13039 requirements are still open. This change
does not qualify spell/feature eligibility, all trigger windows, Spirit Guardians
overlap/timing, every repeat-effect lifecycle or world profiles. The existing
rescue policy auto-spends when it can help; this does not add a choice surface.
No owner action is required.

This work includes material from the System Reference Document 5.2.1 ("SRD 5.2.1") by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative Commons Attribution 4.0 International License, available at https://creativecommons.org/licenses/by/4.0/legalcode.
