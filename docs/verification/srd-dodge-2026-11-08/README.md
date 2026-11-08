# Dodge ends on actual turn start or a lost benefit

2026-11-08, baseline `73a82fb5d4`, finally rebased onto `45069b20ee`. Partial
[#13039](https://github.com/OpenAgentsInc/openagents/issues/13039) and
[#13040](https://github.com/OpenAgentsInc/openagents/issues/13040) progress.
The containing commit and [source hashes](source-hashes.txt) identify the tree.

Source: pinned [SRD 5.2.1](https://media.dndbeyond.com/compendium-images/srd/5.2/SRD_CC_v5.2.1.pdf),
Dodge p.10, plus Grappled/Restrained, Hypnotic Pattern and Steady Aim's Speed
rules. The pinned source digest remains
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.

## Behavior and ownership

`dodge.coax` owns the normal Dodge Action, beginning/ending its benefit and
checking whether it is still live. Patient Defense uses that same benefit.
Attack rolls against a dodging creature gain Disadvantage; its Dexterity saves
gain Advantage. Other edge sources still cancel normally.

- The internal creature field `dodge_until: f32` is replaced by `dodging: bool`.
  The creature's actual next `rs_start_turn` retires it. Clock advancement,
  another creature's turn and the end of its own current turn do not.
- Admitted condition insertion and zero-HP damage reconcile Dodge immediately.
  Incapacitated and its four encompassing conditions, Grappled, Restrained and
  Dazed end it. Removing the condition does not resume the benefit.
- Current walking Speed zero ends Dodge, including tracked Slow/halving, and
  own-turn Steady Aim ends it. A Slow mastery hit and Stunning Strike's halving
  reconcile immediately. Other direct raw speed/state writes must call the
  reconciliation boundary when they change the state; attack/save queries also
  reconcile before granting the benefit.
- Command Halt forbids movement without changing Speed; it does not itself end
  Dodge. Spending all movement and another actor's anchored turn are also not
  Speed zero. Blinded and a positive one-foot Speed do not end Dodge by themselves.
- A normal admitted Dodge spends one Action, no Focus or Bonus Action. A
  capable zero-Speed creature can take the Action but receives no benefit.
  Refused or immune effects do not end an existing Dodge.

The Lab's status display reads the shared flag. No network schema/event identity
or persistent character-save field changes. This **does change internal
RsCreature layout**; consumers must compile against the same shared definition.
No protocol field named `dodge_until` exists. Searches find no remaining code
references to that old field in the engine, apps, Verse or room source.

The preceding death-save authority gains a small self-contained entry check.
Without it, entry suites that load battle but lack death-turn scenarios fail
authority execution coverage. The death-save product procedures are unchanged.
The same entry-check convention is used in the new Dodge authority.

## Evidence

- [Red](red.log): three new regressions fail on the baseline, reproducing
  nonretired Dodge after immobilization, wall-clock expiry and Speed-zero
  benefits. The immune/refusal control passes. The older Patient Defense
  regression also needed an actual next turn after clock advancement; it now
  checks that the benefit holds before that turn and ends at its start.
- [Core](core.log): all 229 checks pass on the combined tree, including inherited
  lifecycle and upstream concentration checks. The rebase preserves upstream
  teardown/continuation and effect insertion changes, as well as both ledger
  records. New component/concentration dependencies justify the combined checks.
- [Dodge mutation](mutation.log): 47 mutants killed, zero survivors. Seven
  initial survivors prompted positive-Speed/Blinded controls, exact event
  assertions, first-observation return checks and a one-creature borrowed
  storage boundary. The exact Dodge unit/compiler bytes survive the rebase;
  that passing gate is reused. [Updated lifecycle](lifecycle-mutation.log): all
  37 mutants killed after upstream centralized its filtering and added target
  validation; the root now includes the corresponding concentration fixtures.
  Its earlier [50-mutant result](lifecycle-pre-rebase-mutation.log) covers the
  pre-rebase implementation only. No equivalent exemptions in any gate.
- [Native](native.json), [text](native.log): all ten root scenarios pass on
  ordinary emitted checked C at O0/O2. They include actual Action/Patient Defense,
  Slow mastery, turn lifecycle, refusal, borrowed storage and condition removal.
- [Hotbar](hotbar.log): 193 checks pass, including a saved-seat Patient Defense
  that spends Focus/Bonus Action and loses Dodge to Restrained even when freed
  before another roll. [Components](components.log): 208 checks pass.
  [Experimental](experimental.log): 14 selected combat checks pass out of 714;
  evaluator integration only, not a rendered browser/phone receipt.
- [Tools check](tools.log) passes.

The pre-rebase original-source ABI-2 ASan/UBSan attempt cannot emit the selected
borrowed-storage fixture: [E0413](native-check-limit.log) rejects assigning a local
array slice into `b.c`. [Full record](native-check-attempt.json.gz) retains the
passing 180-check baseline and the emission refusal. It was not retried after
rebase because the same borrowed assignment remains. No ABI-2 artifact or sanitizer
execution occurred, and neither is claimed. The compiler was not weakened or
changed; the ordinary native runner above executes the actual fixture.

Commands (180-second checks/tools, 600-second mutation/native limits):

```sh
coax/bin/coax test aethyr/rules/srd/dodge-checks.coax --library
coax/bin/coax mutate aethyr/rules/srd/dodge-checks.coax --library --unit aethyr/rules/srd/dodge.coax --jobs 4
coax/bin/coax mutate aethyr/rules/srd/dodge-checks.coax --library --unit aethyr/rules/srd/effect-lifecycle.coax --jobs 4
python3 coax/tools/native_checks.py aethyr/rules/srd/dodge-checks.coax --out /tmp/oa-dodge-native-final.json
python3 coax/tools/native_check_execution.py aethyr/rules/srd/dodge-checks.coax --verification-abi 2 --only 'Dodge retirement supports' --cc 'cc -fsanitize=address,undefined -fno-omit-frame-pointer -fno-pie -no-pie' --keep /tmp/oa-dodge-abi2 --out /tmp/oa-dodge-abi2.json
coax/bin/coax test apps/openagents-web/fivebar-test.coax --library
coax/bin/coax test aethyr/rules/srd/component-checks.coax --library
coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental combat:'
coax/web/tool.sh tools-check
```

Both issues remain open for their other condition, check, action, movement,
visibility, trigger and recovery requirements. General movement modes, raw
snapshot reconciliation, all terrain/area transitions, source-profile completion
and new player controls for the ordinary Dodge Action remain separate work.
Current Patient Defense exposes the shared benefit through its existing bar;
this change does not qualify every other clause of that feature. Admitted
indices, coherent borrowed creature storage and tracked state remain caller
preconditions. No owner action is required.

This work includes material from the System Reference Document 5.2.1 ("SRD 5.2.1") by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative Commons Attribution 4.0 International License, available at https://creativecommons.org/licenses/by/4.0/legalcode.
