# Damage interrupts retained Long Rest

Scope: SRD 5.2.1 p.185 (Long Rest), p.343 (Zombie Undead Fortitude), and
concentration/damage consequences. Pinned PDF SHA-256:
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.
This advances #13040 and #13039; neither issue is complete.

The production battle damage resolver interrupts an active synchronized Long
Rest before copying HP or rolling protection saves. Earned Short Rest banks
are available to the ensuing damage Reaction; no Reaction budget is renewed.
Temporary HP absorption still interrupts and wakes natural sleep. Independent
Unconscious sources survive waking. Zero/negative damage and damage entirely
prevented by the declared house HP floor do not interrupt. Interruption-counter
exhaustion retires the unresumable period without freezing the battle clock.

This producer defaults to declining optional Hit Dice/recovery choices and
closes their window. Queued choices and the turn controller's simultaneous
ordering (p.187) remain required; this is not a universal rule that recovery
must precede same-time damage. Casting/Initiative causes and real world controls
are not enabled by this change. Callers must synchronize the journal first;
opt-in scheduled ticks do that, legacy/manual owners retain that obligation.

A regression exposed Undead Fortitude incorrectly replacing damage taken with
HP loss. The protected HP kernel now consumes actual Temporary HP and retains
full damage/concentration DC, while applying admitted protection only to HP.
Turn Undead ends and concentration rolls even when protection prevents all HP
loss. Protection saves retain consumed rescue resources before the HP snapshot.

## Verification

Before merging upstream Shatter/Exhaustion-save changes:

- Shared Long Rest suite: 324/324 checks.
- Independent saving-throw suite: 275/275 checks.
- Authority procedure scopes, all checks retained: rest-damage 61/61,
  damage-protection 38/38, hit-points 136/136, rest-scheduler 177/177,
  long-rest-state 281/281 mutants killed: 693 total, zero survivors/exemptions.
- Compiled native witness: all 72 class/level rest interruptions/resumptions,
  borrowed Reaction/slot budgets, Fortitude damage triggers including an
  independent concentration save, and capacity retirement pass.
- Checked Lab Wasm builds (2,549,597 bytes; SHA-256
  `f285be9c640b75fe09c7fc9744eaee76d3b94b44039ee59c17781ab16259c35d`).
- Tools policy check: 1897 tracked tools, 35 vendored, zero missing reasons.

Commands:

```sh
coax/bin/coax test aethyr/rules/srd/long-rest-resources-checks.coax --library
coax/bin/coax test aethyr/rules/srd/saving-throw-checks.coax --library
coax/bin/coax mutate aethyr/rules/srd/long-rest-resources-checks.coax --library --unit aethyr/rules/srd/UNIT.coax --jobs 2
coax/web/tool.sh aethyr/rules/srd/rest-damage-native.coax
coax/web/tool.sh tools-check
coax/web/wasm.sh apps/openagents-web/fivelab.coax apps/openagents-web/fivelab-face.c /tmp/srd-rest-damage.wasm apps/openagents-web/everglade-step.coax aethyr/sticks.coax
```

The mutation command is run for each named scope; passing unchanged procedures
are not rerun merely for confirmation. Merged dependency validation is recorded
below before publication.

## Merged validation

Rebased onto `d12df2f88c` (Shatter Construct save rider and Exhaustion-save
checks), retaining both invariant sections. The merged tree passes 325 shared
Long Rest checks, 278 independent saving-throw checks, the final native witness
above and tools-check. The five authority procedure scopes are unchanged by
that merge; their 693 killed-mutant results are retained. The checked Lab Wasm
build passes in 43.3 seconds, 2,550,572 bytes, SHA-256
`792813c85088c0552bb0774b1b952e5748d5b0eda7be162cce518390fd59fd2e`.
The compiler reports its existing source-version/value-use advisories. The
`/tmp` build has no shipping artifact size-budget entry; no budget was raised.
