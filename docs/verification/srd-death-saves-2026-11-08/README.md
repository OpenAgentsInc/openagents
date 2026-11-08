# Battle death saves include saving-throw bonuses

2026-11-08, baseline `c20f499fbb`. Partial progress on
[#13039](https://github.com/OpenAgentsInc/openagents/issues/13039), which remains
open. The containing commit and [source hashes](source-hashes.txt) identify the
implementation and compiler.

Sources: pinned [SRD 5.2.1](https://media.dndbeyond.com/compendium-images/srd/5.2/SRD_CC_v5.2.1.pdf),
death saving throws pp.17–18, Bardic Inspiration p.32, Aura of Protection p.55,
Dark One's Own Luck p.75, Bane p.112 and Bless p.113. The source SHA-256 remains
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.

## Behavior

Battle turns now use `rs_battle_death_save`, which admits only a living,
unstable creature at zero HP before sampling dice or spending resources. It
adds the best eligible nearby allied Aura and one Bless/Bane die each. Death
saves have no ability modifier or save proficiency and do not automatically
fail because the creature is Unconscious. The existing optional rescue policy
can then spend held Inspiration and Own Luck on an ordinary failed result.
The final creature copy is taken after rescue spending, so updating the death
track cannot restore a consumed resource.

`death-saves.coax` keeps the natural face separate from the modified total.
An ordinary total of ten succeeds; a modified twenty is not a natural twenty.
A natural one imposes two failures regardless of bonuses and cannot spend
useless rescue dice. A natural twenty attempts actual one-HP healing regardless
of its modified total, preserving blocked-healing and independent Unconscious
behavior. Bless/Bane dice still occur on these special natural rolls; they do
not change the special outcome. With currently supported battle modifiers,
a natural twenty's total is at least sixteen, so the rescue policy spends none.

The standalone `rs_death_save(dice, creature)` remains an **unmodified** roll
primitive for callers without a battle. Its eligibility and outcome now delegate
to the same authority; all product turn callers use the battle-aware resolver.
`rs_hurt` and `rs_heal` are unchanged. Existing result codes, event kinds, turn
cadence, protocol and creature layout remain unchanged.

## Evidence

- [Red](red.log): four new regressions fail on the baseline, reproducing missing
  boons, Aura, rescue resources and special-roll boon draws. The final fixture
  moves an unrelated third creature away from the Aura boundary squares; no
  expected rule result changes.
- [Core](core.log): all 189 checks pass, including the existing HP/death/healing
  regressions. The new property crosses every natural face, eight modified-total
  boundaries and all nine preterminal success/failure-count combinations: 1,440
  outcome combinations. A minimal admitted creature fixture avoids repeatedly
  constructing a class inventory; the initial constructor-heavy property
  exceeded its default step budget and was not given a larger budget.
- [Outcome mutation](outcome-mutation.log): all 57 mutants killed.
  [Battle roll mutation](roll-mutation.log): all 22 killed. Zero survivors and
  no equivalent exemptions in either authority; every entry check is retained.
  The earlier HP damage/healing qualification remains applicable to those
  unchanged procedures; this gate separately qualifies the extracted/extended
  death-save policy.
- [Native scenarios](native.json), [text](native.log): all eight root `#test`
  scenarios pass on ordinary emitted checked C at O0/O2. They exercise real
  turn calls, modifier eligibility, resource spending, ineligible no-ops and
  special natural-face outcomes. They make no sanitizer claim.
- [Original-source native property](native-property.json.gz),
  [summary](native-property.log): ABI 2 emits the selected outcome property from
  its original source. All 20 face cases, each with the 72 total/counter
  combinations, complete at O0/O2 under ASan/UBSan. The full record preserves
  source/baseline validation, emission/artifact hashes, compiler commands and
  progress. This sanitizer claim covers the pure outcome property only.
- [Hotbar](hotbar.log): 184 checks pass. A dying real-time saved seat receives
  Bane and uses Inspiration, preserves the other actor's turn, and does not
  roll again on the intervening short action beat.
- [Experimental](experimental.log): selected evaluator combat checks pass;
  this is not a browser, Wasm, phone or full-world receipt.
  [Tools check](tools.log) passes.

Commands (180-second checks/tools, 600-second mutation/native limits):

```sh
coax/bin/coax test aethyr/rules/srd/death-save-checks.coax --library
coax/bin/coax mutate aethyr/rules/srd/death-save-checks.coax --library --unit aethyr/rules/srd/death-saves.coax --jobs 4
coax/bin/coax mutate aethyr/rules/srd/death-save-checks.coax --library --unit aethyr/rules/srd/death-save-roll.coax --jobs 4
python3 coax/tools/native_checks.py aethyr/rules/srd/death-save-checks.coax --out /tmp/oa-death-save-native.json
python3 coax/tools/native_check_execution.py aethyr/rules/srd/death-save-checks.coax --verification-abi 2 --only 'death outcomes separate' --cc 'cc -fsanitize=address,undefined -fno-omit-frame-pointer -fno-pie -no-pie' --keep /tmp/oa-death-save-abi2 --out /tmp/oa-death-save-abi2.json
coax/bin/coax test apps/openagents-web/fivebar-test.coax --library
coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental combat:'
coax/web/tool.sh tools-check
```

Core callers supply valid creature indices, admitted natural faces and bounded
preterminal counters. No arbitrary record validation or new player choice surface
is introduced. General Advantage/Disadvantage grants on death saves, Exhaustion,
other condition clauses, stabilization/knockout/recovery workflows and wider
#13039 acceptance remain open. Current Inspiration/Own Luck use follows the
existing automatic rescue policy; explicit optional choices remain follow-up.
No owner action is required.

This work includes material from the System Reference Document 5.2.1 ("SRD 5.2.1") by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative Commons Attribution 4.0 International License, available at https://creativecommons.org/licenses/by/4.0/legalcode.
