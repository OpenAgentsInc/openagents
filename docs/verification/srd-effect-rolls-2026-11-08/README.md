# Shared effect rolls and overlapping sources

2026-11-08. Baseline `acee72340d`. Partial progress on
[#13039](https://github.com/OpenAgentsInc/openagents/issues/13039).
The commit containing this record and [source hashes](source-hashes.txt) identify
the delivered change. The issue remains open for the rest of its active scope.

## Behavior and source

SRD 5.2.1 p.106 says repeated castings of the same spell do not combine their
benefits, explicitly including Bless. Bless (p.113) and Bane (p.112) now each
roll one d4 per attack/save, regardless of record count. Bless is rolled before
Bane to keep deterministic replay independent of physical storage order. The
records retain their original owners and deadlines: ending one concentration
or expiring one cast leaves any other active casting in force.

Sap and Vex (p.90), Guiding Bolt (p.138), and Vicious Mockery (pp.171–172)
name the next applicable attack roll. That roll now retires every matching
pending record, even when the attack misses or opposing sources cancel.
Vex records for another target remain pending. A source does not extend its
benefit across several rolls merely because multiple records were retained.

Source: the [official SRD PDF](https://media.dndbeyond.com/compendium-images/srd/5.2/SRD_CC_v5.2.1.pdf),
downloaded and checked against the existing pinned SHA-256
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.
No source catalog or public protocol layout changes.

## Evidence

- [Initial regressions](red.log): all five new checks fail on the baseline,
  including the slot/multiplicity property at `bits = 3`. The fixture resets
  its dice count after initiative; this is the corrected red run.
- [Complete evaluator entry](core.log): 137 checks pass, including two
  64-case properties. Tests include two real Bless casts, ending either
  concentration, staggered expiry, full pools, unrelated effects, missed
  attacks and stable seeded dice.
- [Core authority mutation](mutation.log): `effect-rolls.coax`, every entry
  check retained, 39 killed, zero survivors and no equivalence exemptions.
  This qualifies the new pending-record/boon policy, not every unchanged
  legacy battle procedure. Initial survivors prompted a broader replay-seed
  property and removal of an irrelevant sentinel argument.
- [Hotbar integration](hotbar.log): 145 checks pass. The added case drives
  `fb_seat_use`: three Bless records give one d4, every weakness record is
  consumed by the missed attack, and the action is spent normally.
- [Experimental combat](experimental.log): 10 of 671 checks selected by
  `--only 'experimental combat:'`, all pass. This is evaluator integration,
  not a rendering-page or phone receipt.
- [Native report](native.json): all seven authored `#test` bodies pass on
  emitted checked C at both O0 and O2 using the existing `native_checks.py`.
  Its scope excludes the two `#forall` properties; those run in the evaluator.
  [Native text result](native.log).

After these executions, the two source-reference header comments were corrected
to Bane p.112 and Vicious Mockery pp.171–172. No program or check body changed;
the native report retains its original source hash, and `source-hashes.txt`
records the final files. Unchanged passing checks are reused.

The newer original-source native-check emitter was also attempted with ABI 2.
Its complete evaluator baseline passes, but emission refuses the existing
`rs_room` self-borrow (`battle.coax`, `b[0].c = b[0].own[..]`) with E0413.
[Retained diagnostic](native-emission-refusal.log). No emitted native assertion
ran through that path; the fallback report above does not claim ABI 2 or
sanitizer qualification. The compiler's borrow restriction was not relaxed.

Commands (each bounded to 180 seconds for checks or 600 seconds for mutation/native):

```sh
coax/bin/coax test aethyr/rules/srd/effect-roll-checks.coax --library
coax/bin/coax mutate aethyr/rules/srd/effect-roll-checks.coax --library --unit aethyr/rules/srd/effect-rolls.coax --jobs 4
coax/bin/coax test apps/openagents-web/fivebar-test.coax --library
coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental combat:'
python3 coax/tools/native_checks.py aethyr/rules/srd/effect-roll-checks.coax --out /tmp/effect-roll-native.json
coax/web/tool.sh tools-check
```

## Remaining scope

Rebased onto `a38714980b`, preserving the new `reaction-slot.coax` load and
policy alongside this change. Fresh combined-tree checks pass
[140 core checks](core-rebased.log), [148 hotbar checks](hotbar-rebased.log)
and [10 of 674 Experimental checks](experimental-rebased.log).
[Tools-check](tools-rebased.log) also passes. The unchanged effect-roll authority
reuses its 39/39 mutation result; the native report above describes the earlier
baseline, before this upstream reaction integration. The final source hashes
include the rebased battle and reaction policy. These results do not claim a
new native qualification of the upstream change.

This change does not implement a general potency/recency selector for arbitrary
same-named effects. Hex/Mark overlap, other conditions and damage/healing/death,
visibility and action timing retain their separate active acceptance. Storage
admission remains the bounded policy delivered in #13041; this change does not
relax its conservative reservation. Whole SRD, browser, Wasm, phone and full
world qualification are not claimed. No owner action is required.

This work includes material from the System Reference Document 5.2.1 ("SRD 5.2.1") by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative Commons Attribution 4.0 International License, available at https://creativecommons.org/licenses/by/4.0/legalcode.
