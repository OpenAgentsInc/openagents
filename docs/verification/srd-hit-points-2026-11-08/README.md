# Zero-HP damage, healing and death-save transitions

2026-11-08. Initial baseline `471f88df1e`; finally rebased onto `ca56df8c01`.
Partial progress on [#13039](https://github.com/OpenAgentsInc/openagents/issues/13039),
which remains open. The containing commit and [source hashes](source-hashes.txt)
identify the implementation and compiler.

## Source and behavior

Source: [official SRD 5.2.1](https://media.dndbeyond.com/compendium-images/srd/5.2/SRD_CC_v5.2.1.pdf),
pp.17–18, plus the existing Concentration damage-DC rule in the glossary.
The downloaded source matches the repository's pinned SHA-256
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.

- Taking positive damage while at zero HP causes one death-save failure, or
  two for a Critical Hit, even when temporary HP absorb the entire hit. It ends
  stability. The p.18 damage-at-zero maximum-HP threshold compares the damage
  taken; the separate p.17 rule for first dropping from positive HP compares
  damage left after temporary HP and ordinary HP. Tests cover both thresholds.
- A third successful death save makes the creature Stable and immediately
  resets both counts. Stable, dead and positive-HP creatures do not roll through
  the low-level death-save API. Natural 1/20 and independent success/failure
  counts retain their specified outcomes.
- A natural 20 attempts to restore one HP. If an active no-healing effect blocks
  that gain, the result is `RS_DS_BLOCKED`, not `RS_DS_UP`; HP, save counts and
  Unconscious remain. The turn log reports that healing was prevented. Successful
  recovery says it regains one HP, without claiming it can act through another
  Unconscious source. `RS_DS_NONE` is the explicit ineligible/no-roll result.
- Healing from zero removes only zero HP's Unconscious source. An independent
  recorded Unconscious condition remains until its own retirement; conversely,
  ending that record does not clear zero HP's Unconscious condition. Prone and
  unrelated conditions survive healing as before. Spell and feature healing
  append “and wakes” only when the restored creature is no longer Unconscious.
- Healing clamps the gain to remaining room before adding it, so large positive
  amounts cannot overflow. Nonpositive, full-HP, dead and no-healing cases do
  not consume or restore temporary HP. Existing damage/concentration reporting
  continues to count the whole damage taken, including absorbed damage.

No creature/battle field, public protocol or saved layout changes. Existing
death result numbers 0–4 are retained; the two new internal result codes are
handled by the shared turn path. The legacy `RS_EV_STABLE` death-save event kind
is retained for a blocked result; actual stability still comes from creature
state, and no false `RS_EV_UP` is emitted.

## Checks and evidence

- [Initial red](red.log): seven new regressions fail against the baseline.
  [Reporting red](report-red.log) separately catches the false “back up” claim
  when an independent Unconscious source remains. Review also caught the same
  reporting error in [Healing Word](hotbar-report-red.log) and
  [Lay On Hands](feature-report-red.log); both failed before their message fixes.
- [Final core checks](core.log): 161 passed. Three properties cover every
  death-save face with all preterminal success/failure counts, damage/temporary
  HP conservation and massive-damage edges, and concentration DC boundaries.
- [HP authority mutation](hp-mutation.log): 185 killed, zero survivors.
  [Effect-storage authority](storage-mutation.log): 121 killed, zero survivors.
  Every entry check is retained; no equivalent-mutant exemptions. Initial HP
  survivors prompted identity-zero, exact condition-mask and independent-Blinded
  fixtures, plus removal of a redundant zero-healing guard.
- [Hotbar checks](hotbar.log): 159 passed. The new Healing Word test runs through
  `fb_seat_use`, pays the normal slot/Bonus Action and restores a dying ally's HP
  while preserving another Unconscious source and its separate retirement.
- [Native report](native.json), [text](native.log): all 17 authored root `#test`
  scenarios pass on emitted checked C at O0 and O2 using the established
  `coax/tools/native_checks.py`. This includes real turn stabilization,
  destabilization through temporary HP, natural-one death and natural-twenty
  reporting, and Lay On Hands with/without another Unconscious source. The three
  `#forall` properties are evaluator evidence, not native.
- [Experimental combat](experimental.log): 10 selected checks pass. This is
  evaluator world integration, not a browser/phone rendering receipt.
  [Combined-tree result](experimental-rebased.log): 10 of 680 pass after the
  casting rebase, before the final healing-message-only corrections.

Mutation gates ran on the initial baseline, with the same final HP/storage unit
bytes; later turn/healing regressions and presentation fixes leave those units
unchanged. Rebase preserved
upstream explicit recipient/ray choices and subsequent per-recipient spell
options/Hex durations. Fresh complete core/hotbar checks and
native scenarios above ran on the combined tree. Unchanged passing mutation
results are reused, rather than rerun to confirm them. The earlier Experimental
result is retained separately from any combined-tree result.

Commands (checks bounded to 180 seconds, mutation/native to 600 seconds):

```sh
coax/bin/coax test aethyr/rules/srd/hit-point-checks.coax --library
coax/bin/coax mutate aethyr/rules/srd/hit-point-checks.coax --library --unit aethyr/rules/srd/hit-points.coax --jobs 4
coax/bin/coax mutate aethyr/rules/srd/hit-point-checks.coax --library --unit aethyr/rules/srd/effect-storage.coax --jobs 4
coax/bin/coax test apps/openagents-web/fivebar-test.coax --library
coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental combat:'
python3 coax/tools/native_checks.py aethyr/rules/srd/hit-point-checks.coax --out /tmp/hit-point-native.json
coax/web/tool.sh tools-check
```

The newer native-check emitter's existing `rs_room` self-borrow restriction is
recorded in the [previous overlap evidence](../srd-effect-rolls-2026-11-08/README.md).
It was not relaxed or retried here. This report uses ordinary native emission
through the established runner, and makes no sanitizer, ABI-2, Wasm or phone claim.

## Remaining acceptance

This does not complete #13039. The wider checks/conditions, damage-defense and
concentration interactions still need source-qualified implementation for the
slice. These primitives do not add the Medicine/Help stabilization action,
1d4-hour recovery of an unhealed Stable creature, the melee knockout choice,
maximum-HP reduction workflows or a general unconscious-condition provenance
model for unrecorded effects. Existing world profiles, used-monster mechanics
and migration retain their separate active tickets. Core callers supply one
creature with nonnegative HP/temp and bounded death-save counts; this is not a
general validation boundary for arbitrary forged creature records. No owner
action is required.

This work includes material from the System Reference Document 5.2.1 ("SRD 5.2.1") by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative Commons Attribution 4.0 International License, available at https://creativecommons.org/licenses/by/4.0/legalcode.
