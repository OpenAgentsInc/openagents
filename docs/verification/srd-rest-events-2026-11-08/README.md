# Initiative interruption and terminal rest retirement

Source: pinned SRD 5.2.1 Long Rest p.185, Exhaustion p.181, Unconscious
p.191, and death saves pp.17–18. PDF SHA-256:
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.
This advances #13040 and #13039; it does not complete either issue.

## Behavior

`rs_roll_initiative` admits the logical actor, refuses a dead actor or a borrowed
strike frame, interrupts that actor's synchronized rest, then uses the shared
Initiative plan and dice. `rs_add` publishes the new logical actor before this
roll. An unrelated spawn cannot interrupt the other actors. `rs_begin` only
schedules retained Initiative and does not manufacture a new roll/interruption.
A dead admitted body retires rest and consumes no Initiative die.

Rolling Initiative is an interruption, not a wake source. Natural sleep and
independent Unconscious sources remain; Incapacitated Initiative Disadvantage
still applies (including ordinary cancellation with Athlete Advantage). Damage
and admitted casting retain their existing waking behavior. Eligible recovery
is granted once, without automatically healing or renewing a Reaction. The
producer defaults to declining unconfigured optional choices.

`rs_rest_retire_dead` retires Short/Long Rest, pending fallback and natural sleep
without recovery or revival. It preserves completion timestamps, spent dice,
daily histories and unrelated condition ownership. It is called at every shared
battle terminal producer: damage, death saves, Exhaustion and dead admission.
Nonterminal Exhaustion does not interrupt rest. Earned recovery from preceding
damage remains valid when that damage then kills; non-damage death grants none.
This prevents an active dead-body journal from freezing the rest scheduler.

## Evidence

The Exhaustion-death regression failed before implementation at the retained
active-phase assertion. Final verification:

- 338/338 composed Long Rest checks, 333/333 Exhaustion-composed checks and
  319/319 hotbar checks pass (these suites overlap).
- Authority scopes: rest-events 42/42, shared rest-damage/cause 73/73,
  exhaustion-lifecycle 50/50 and death-save-roll 33/33 mutants killed;
  198 total, zero survivors/exemptions.
- The death-save gate includes stabilization-checks, retaining its existing
  Exhaustion/natural-face witnesses and a new explicit inactive-deadline
  replacement regression. The initial narrower gate missed these two cases.
- Native compiled execution passes all 72 class/level Initiative/death
  transactions and same-epoch actor-local admission. The witness accounts for
  Champion Athlete canceling sleeping Initiative Disadvantage. It observes
  unaffected other-actor rest/time, preserved turn/Reaction budgets, earned
  banks, natural-sleep ownership, death retirement and continued clock progress.
- Checked Lab Wasm passes in 41.0 seconds: 2,551,093 bytes, SHA-256
  `6104e918f9aca7fcdb7432dfdf3b0bfa32d187b50696f174b834c56b367f0128`.
  Existing compiler source-version/value-use advisories remain. The temporary
  artifact has no shipping size-budget entry; no budget was raised.
- Tools policy: 1897 tracked tools, 35 vendored, zero missing reasons.

```sh
coax/bin/coax test aethyr/rules/srd/long-rest-resources-checks.coax --library
coax/bin/coax test aethyr/rules/srd/exhaustion-checks.coax --library
coax/bin/coax test apps/openagents-web/fivebar-test.coax --library
coax/bin/coax mutate aethyr/rules/srd/long-rest-resources-checks.coax --library --unit aethyr/rules/srd/rest-events.coax --jobs 2
coax/bin/coax mutate aethyr/rules/srd/long-rest-resources-checks.coax --library --unit aethyr/rules/srd/rest-damage.coax --jobs 2
coax/bin/coax mutate aethyr/rules/srd/exhaustion-checks.coax --library --unit aethyr/rules/srd/exhaustion-lifecycle.coax --jobs 2
coax/bin/coax mutate aethyr/rules/srd/stabilization-checks.coax --library --unit aethyr/rules/srd/death-save-roll.coax --jobs 2
coax/web/tool.sh aethyr/rules/srd/rest-events-native.coax
coax/web/tool.sh tools-check
coax/web/wasm.sh apps/openagents-web/fivelab.coax apps/openagents-web/fivelab-face.c /tmp/srd-rest-events.wasm apps/openagents-web/everglade-step.coax aethyr/sticks.coax
```

## Remaining acceptance

Journal synchronization and earlier-boundary consumption remain the caller's
obligation before a causal event. The admission copy witness uses the same
battle clock; it does not qualify foreign-epoch persistence/rejoin. Automatic
rest still has no exposed world controls. Actual world activity/settled/wake
observations, queued optional recovery choices and turn-controller simultaneous
ordering, source stat/nutrition producers, complete inventory dropping, and
persistent timeline binding remain required before a complete rest flow.
