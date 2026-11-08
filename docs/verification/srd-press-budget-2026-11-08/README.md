# Explicit per-press house resource policy

Partial #13045/#13046 work on 75ce46daf0, compiler 0.52.0.

`press-budget.coax` distinguishes ordinary paid presses (no changes) from
Experimental's explicit unlimited house policy. Unlimited preparation restores
slots, ordinary limited uses, Focus and Rage to their preset maxima and clears
only Action, Bonus Action and slot-spent flags. It does not grant access,
materials, health, temporary HP, Reactions, movement, Sorcery Points, Lay On
Hands or a new logical turn. Unknown profile identities refuse without writes.
The narrow receipt captures exactly those overridden fields for rollback.
These internal helpers require one creature and one turn view supplied by a
validated caller; they are not a public wire/request parser.

Experimental replaces its local refill loop with this shared policy, takes a
receipt before preparation, and restores it plus the seat's prior open state
when shared execution refuses. A speech-blocked Healing Word reproduced the
old bug: refusal refilled an empty slot and erased spent Bonus Action and
slot-spent state. The new red/green world fixture preserves those facts and
then checks that an accepted unlimited press still works without renewing a
spent Reaction. HP and normal component/access policy stay authoritative.

## Evidence

188 complete library checks pass. The policy mutation gate kills all 51
mutants, zero survivors, no exemptions. All 52 selected Experimental checks
pass. Tools-check passes. Retained logs include both missing-module and actual
world-refusal red evidence.

```
coax/bin/coax test aethyr/rules/srd/press-budget-checks.coax --library
coax/bin/coax mutate aethyr/rules/srd/press-budget-checks.coax --library --unit aethyr/rules/srd/press-budget.coax --jobs 4
coax/bin/coax test apps/openagents-web/fivelab.coax --only experimental
```

This is not closure of #13045 or #13046. Other worlds' per-press reset paths,
profile selection/lifecycle, grants/waivers, empty-target world behavior,
resource acquisition and complete rendered/runtime qualification remain open.
The paid helper's no-op restores ordinary rule admission; there is no new
player-facing strict-mode selector in Experimental.

The ordinary checked Lab Wasm build also exits 0 in 30.4 seconds, producing
2,509,489 bytes. The exact command is the preceding Siege build command with
output `/tmp/srd-press-fivelab.wasm`; output hash and complete log are retained.
No compiler/check-budget override was used. The temporary filename has no
shipping size-budget entry, as the log reports. No browser/play claim follows.

Rebased onto c38e48ad74, retaining the independently landed Help implementation
and both invariants. The composed library passes 189 checks; the ordinary
checked Lab Wasm build and tools-check also pass. Rebased logs/hash are retained.
Unchanged press-policy mutation evidence is reused, not rerun.
