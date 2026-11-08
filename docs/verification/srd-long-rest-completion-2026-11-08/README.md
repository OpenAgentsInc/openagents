# Long Rest body benefits and pending-boundary completion

Base: `263b47aeff`. Sources: pinned SRD 5.2.1 p185 (Long Rest and
Malnutrition), p181 (Dehydration); pinned PDF identity is recorded in
`docs/dnd/long-rest-implementation.md`.

## Behavior

`long-rest-benefits.coax` preflights body and represented resource banks before
mutating either. Explicit `hp_max_lost` and six `score_lost` counters retain
reductions that their source authorizes a Long Rest to remove. Reduction
admission reports both acceptance and actual loss; requests saturate at zero
without negative stats or invented loss. Restoration adds the retained loss to
the current value rather than replacing it with an old snapshot, preserving
later legitimate increases. Arithmetic handles exact signed 64-bit capacity
and refuses overflow before mutation.

HP recovery uses the shared healer, including independent Unconscious owners
and healing prohibitions. Resources use the existing bank primitive, retaining
its source recharge, Hit Dice and Temporary Hit Point behavior. Body recovery
removes one unlocked Exhaustion level. Separate food/water cause counts prevent
removing those levels before their source's full daily nourishment requirement
is met. Cause validation prevents negative/overflowing floors; generic
Exhaustion recovery now observes those same locks.

`long-rest-completion.coax` consumes only an owned pending journal boundary.
It applies those benefits, retires natural sleep, closes the old Short Rest
choice window and acknowledges the journal's *actual* completion timestamp.
Subsequent applications refuse; Turn/Action/Bonus Action/Reaction budgets are
not renewed. Failed late body/bank validation preserves sleep, history and
resources. Other Unconscious sources remain after waking.

## Verification

- Final composed `long-rest-resources-checks.coax --library`: 285 checks pass,
  including journal and timing checks now composed through the creature owner.
- New body-benefit unit: 127 mutants killed, zero survivors/exemptions.
- Changed Exhaustion unit: 42 mutants killed, zero survivors/exemptions.
- New pending-completion unit: 33 mutants killed, zero survivors/exemptions.
  The Exhaustion gate passed before the subsequent private journal field and
  completion bridge were composed; that product unit remained unchanged. The
  final body gate and 285-check composition cover the merged layout. The unchanged
  journal's 259-mutant receipt remains `srd-long-rest-state-2026-11-08`.
- Final native entry `long-rest-benefits-native.coax` passes 72 scenarios, all
  twelve classes at levels1–6. It executes sleep, journal advancement, pending
  completion and replay refusal; checks restoration of every score plus
  unrelated increases, full HP/Hit Dice/banks, locked Exhaustion, independent
  conditions, preserved Reaction, actual completion history and atomic refusal.
- `coax/web/tool.sh tools-check`: 1897 tracked tools, 35 vendored, no missing
  reasons. `git diff --check` passes.

Mutation commands use `coax/bin/coax mutate
 aethyr/rules/srd/long-rest-resources-checks.coax --library --unit UNIT --jobs 3`
(`--jobs 4` for benefits and completion). The checks were written before their implementation
and sharpened against survivors; no equivalent exemptions were declared.

Source SHA256:

- benefits: `7c1f0245e0dd3cbbc312bff15946e4fa0dac2b7646dbd4f11fd7458fc93cccbe`
- completion: `6407d2df839a6fb05b033af6280b849aedd3bbd2daa502e6d725fc3f6ed997cf`
- Exhaustion: `a97b0559a9b72bd2eed8fc8ae11521c4dea8b10ad86d7dafea5ab0bb5eeb80fb`
- native entry: `8b75691008a333e8a4d03247f5db5822126d810fc107ab57905eaed071949d3f`

## Remaining integration and limits

This is private trusted engine state, not an admission decoder for arbitrary
client-supplied reductions, locks or journal phases. There is no world rest
control/scheduler caller yet. The next required work is actual sleep/activity
transitions, authoritative clock conversion and before-event synchronization,
then controls and rejoin persistence. Repeated calls to the lower-level benefit
primitive are not themselves separate admitted rests; the journal consumer
provides the once-per-completion boundary.

Only source-authorized restorable losses enter these counters. Source-specific
monster/spell producers still need to use them and handle their own lethal,
conditional-restoration and derived-stat consequences. No current attack was
silently converted into Life Drain or an ability-drain effect. Food/water daily
consumption/hazard producers and release of the corresponding cause locks also
remain world rules work. No generic nourishment flag resets those counts during
rest. Unrepresented class features and general held-inventory dropping remain
separate acceptance items. Issues #13040/#13044/#13045/#13046 stay open.

The final full checked Lab Wasm composition passes:

```
coax/web/wasm.sh apps/openagents-web/fivelab.coax apps/openagents-web/fivelab-face.c /tmp/srd-long-rest-completion.wasm apps/openagents-web/everglade-step.coax aethyr/sticks.coax
```

35.5 seconds, 2,539,741 bytes; SHA256:
`13a3c8d602d815d6c3099e3885dc8fc284975e21e8e22fb9dbad5b0e419d6b2c`.
The existing compiler source-version and near-limit compile-time work/value
advisories remain (value slots 86%); no work budget was raised. The temporary
artifact name has no shipping size budget; no product budget was added for it.
This proves checked composition, not an exposed world rest flow: uncalled
engine primitives can be eliminated from the shipped Wasm artifact.
