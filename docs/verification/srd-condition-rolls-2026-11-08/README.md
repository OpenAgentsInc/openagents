# Condition consequences in shared combat rolls

2026-11-08. Partial progress on [#13039](https://github.com/OpenAgentsInc/openagents/issues/13039),
which remains open. Initial baseline `dc86ad5b8f`; final integration base
`018abe4bea`. The containing commit, [source hashes](source-hashes.txt) and
compressed native record identify the checked tree and compiler.

## Source and behavior

Source: [official SRD 5.2.1](https://media.dndbeyond.com/compendium-images/srd/5.2/SRD_CC_v5.2.1.pdf),
damage defenses p.17, Blinded p.177, Paralyzed/Petrified/Poisoned/Prone p.186,
Stunned p.189 and Unconscious p.191. Downloaded PDF SHA-256 matches the pinned
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.

`condition-rolls.coax` is the new authority for range-independent attack
modifiers, nearby condition criticals, automatic Strength/Dexterity save
failures, the shared saving-throw result and damage defenses:

- Petrified grants attack Advantage, automatically fails Strength/Dexterity
  saves and resists every damage type. Resistance from it, innate resistance
  and physical Rage resistance applies once. Immunity precedes resistance;
  vulnerability follows it, with resistance rounded down first.
- Hits against Paralyzed or Unconscious creatures within five feet become
  critical hits, including ranged weapon attacks. Stunned and Petrified do not
  confer this critical. A failed attack remains a miss.
- Spell save programs and Cunning Strike use the shared save resolver.
  Automatic failures do not roll a d20, Bless/Bane or rescue dice, and cannot
  be rescued by high ability scores, Inspiration or Own Luck. Normal saves
  retain the existing optional rescue order, including named boon dice for
  Cunning Strike. Save messages describe automatic failure, including Petrified.

The independently landed Prone implementation (`e58f409197`) remains its single
owner. Battle combines its range-dependent sources with the new condition
sources. The initial prototype's overlapping Prone implementation was removed;
this patch does not claim authorship or new qualification of Prone movement.
No protocol, creature layout or saved-state field changes.

## Evidence

- [Initial red](red.log): four regressions fail on the initial baseline,
  including two Prone cases subsequently fixed upstream. The initial Petrified
  test fails at attack Advantage before its damage-kind loop; the final loop
  uses actual damage IDs 1 through 13. [Cunning Trip red](trip-red.log) separately
  catches an automatically failing high-Dexterity creature incorrectly saving.
- [Core](core.log): 165 checks pass on the final combined tree. The damage
  matrix exercises 13 kinds × 32 defense combinations × four amounts, plus
  unrelated-type controls. The condition property independently enumerates all
  15 condition bits and all six save abilities. Distance/hit/critical boundaries,
  held resources, exact DC and actual spell/Cunning Strike paths are checked.
- [Mutation](mutation.log): all 64 mutants in `condition-rolls.coax` killed,
  zero survivors and no equivalent exemptions. Earlier surviving redundant
  Prone-forwarding sites prompted removing the duplicate implementation.
  This gate's policy bytes and compiler match the final tree; later changes
  removed only an unnecessary test budget annotation and a stale comment, then
  rebased upstream timing work. The complete final evaluator/native checks ran
  after that rebase. Unchanged passing policy gates are reused. Upstream Prone's
  own 62-mutant receipt is in its [existing record](../srd-prone-2026-11-08/README.md).
- [Hotbar](hotbar.log): 174 checks pass. New cases use actual Fire Bolt and Rogue
  Attack/Cunning Trip choices through `fb_seat_use`, including normal action
  spending, critical/resisted damage and preserved victim rescue resources.
- [Native scenarios](native.json), [text](native.log): all 12 root `#test`
  scenarios pass on ordinary emitted checked C at O0/O2. This includes the full
  damage matrix, actual weapon/spell execution and Cunning Strike. Removing its
  unnecessary `#steps` annotation lets the established runner discover the
  matrix; an earlier eleven-test native run did not include it.
- [Original-source native record](native-property.json.gz),
  [summary](native-property.log): ABI 2 emits the selected pure condition
  `#forall` directly from the original source. All 15 cases complete at both
  O0/O2 under ASan/UBSan. The full record retains baseline/source validation,
  emission/artifact identities, compile commands and per-case progress. This
  sanitizer claim covers that property only; battle-heavy native scenarios
  above use the ordinary runner and are not claimed sanitizer-qualified.
- [Experimental](experimental.log): ten selected `experimental combat:` checks
  pass out of 695. This is evaluator world integration, not a rendered browser,
  Wasm, phone or whole-world qualification. [Tools check](tools.log) passes.

Commands, bounded to 180 seconds for evaluator/tools and 600 seconds for native
or mutation runs:

```sh
coax/bin/coax mutate aethyr/rules/srd/condition-roll-checks.coax --library --unit aethyr/rules/srd/condition-rolls.coax --jobs 4
coax/bin/coax test apps/openagents-web/fivebar-test.coax --library
coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental combat:'
python3 coax/tools/native_checks.py aethyr/rules/srd/condition-roll-checks.coax --out /tmp/oa-condition-native-publish.json
python3 coax/tools/native_check_execution.py aethyr/rules/srd/condition-roll-checks.coax --verification-abi 2 --only 'each condition contributes' --cc 'cc -fsanitize=address,undefined -fno-omit-frame-pointer -fno-pie -no-pie' --keep /tmp/oa-condition-abi2-publish --out /tmp/oa-condition-abi2-publish.json
coax/web/tool.sh tools-check
```

The modern native runner first runs the full evaluator entry; its validated
baseline output supplies `core.log`. The compressed record retains the complete
output and source manifest. No new harness or compiler change was needed.

## Remaining acceptance

This does not complete #13039. General ability checks, Exhaustion, sight,
Frightened/Grappled/Invisible, Dodge lifecycle, condition immunities and ownership,
Petrified Poisoned immunity/suspension, concentration ending on incapacitation,
and broader HP/recovery work remain. Not every legacy save producer delegates
to the resolver: Hellish Rebuke, Open Hand, repeat saves, death saves and others
retain their separate behavior and need source-qualified follow-up. Expanding
`rs_helpless` to include Petrified also reaches its existing callers, but does not
qualify all of those callers' roll/modifier/resource behavior. Cunning Poison
immunity and class-feature legality retain their separate outstanding work.

Callers provide admitted creature indices, valid abilities and damage IDs,
nonnegative bounded damage and tracked condition state. These helpers do not
validate arbitrary forged battle records, infer geometry/sight or implement
condition lifecycle. World controls, declared profiles and player/wire choice
adoption remain separate active requirements. No owner action is required.

This work includes material from the System Reference Document 5.2.1 ("SRD 5.2.1") by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative Commons Attribution 4.0 International License, available at https://creativecommons.org/licenses/by/4.0/legalcode.
