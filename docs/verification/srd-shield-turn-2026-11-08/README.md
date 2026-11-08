# Shield retires on the caster's actual next turn start

Source: pinned SRD 5.2.1 pp.161–162, SHA-256
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.
The Shield text gives +5 AC through the triggering attack until the caster's
next turn starts. A saved seat already advances `turn_at` before opening its
turn. Adding another beat when it casts Shield kept the spell active through
that next start. Clock-only expiry also retired it without the logical hook.
Both behaviors are reproduced by retained failing regressions.

`shield-lifetime.coax` retains `shield_active`, supplies the flat bonus, and
retires it through `rs_start_turn`. The spell starts it only after existing
access/components/Reaction/slot admission and payment. Clock passage, another
actor's turn and a borrowed Reaction frame do not retire it. The obsolete
`shield_until` forecast is removed from the creature and all runtime consumers;
AC and the Lab SHIELD label observe the same retained state. No new wire or
persistent save field is introduced.

Older fixtures that inspected a forecast now inspect actual retained state.
The original spell test resets through the explicit retirement helper and
asserts the actual turn hook, rather than asserting clock expiry. Its hit,
slot, AC and Graze assertions are preserved. This supersedes earlier receipts'
Shield-duration forecast checks; those did not establish actual next-start
retirement. True-owner slot/Quickened policy remains unchanged.

## Evidence

Full core and saved-seat entries each pass 201 checks. Opportunity composition
passes 210. The new lifetime gate kills 8/8 mutants, zero survivors/no exemptions.
Actual native SHIELD-TURN-START covers both strict/action profiles: a real
opportunity attack triggers paid Shield, blocks the hit, survives clock-only
advance and retires at the saved seat's actual next start. Ordinary/typed
opportunity native fixtures exercise their existing budget cases with retained
state. Complete checked Lab Wasm build passes in 38.5 seconds, 2,537,063 bytes,
without new work-budget overrides. Existing compiler-source and near-limit
advisories remain; the temporary output name has no registered size budget.
Tools-check/diff check pass.

```
coax/bin/coax test aethyr/rules/srd/shield-turn-checks.coax --library
coax/bin/coax test apps/openagents-web/shield-turn-checks.coax --library
coax/bin/coax mutate aethyr/rules/srd/shield-turn-checks.coax --library --unit aethyr/rules/srd/shield-lifetime.coax --jobs 4
coax/bin/coax test aethyr/rules/srd/opportunity-attack-checks.coax --library
coax/web/tool.sh apps/openagents-web/shield-turn-native.coax
coax/web/tool.sh aethyr/rules/srd/opportunity-attack-native.coax
coax/web/tool.sh aethyr/rules/srd/stat-opportunity-native.coax
coax/web/wasm.sh apps/openagents-web/fivelab.coax apps/openagents-web/fivelab-face.c /tmp/srd-shield-turn-fivelab.wasm apps/openagents-web/everglade-step.coax aethyr/sticks.coax
coax/web/tool.sh tools-check
```

## Remaining scope

This qualifies Shield's AC lifecycle at the existing actual turn boundary.
It does not establish complete Magic Missile targeting/immunity, Dispel Magic,
other spell-specific interactions or full Shield/source-catalog coverage.
Other numeric until-next-turn features, missed-turn/frame-stall scheduling,
repeat/death-save ordering and complete world timing remain separate active
work. A turn hook must actually be dispatched; replacing it with elapsed wall
time is not this policy. No issue closes on this fix.
