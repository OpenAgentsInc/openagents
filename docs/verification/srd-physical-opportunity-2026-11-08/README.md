# Experimental player departure resolves monster opportunities

SRD 5.2.1 p.15; pinned PDF SHA-256
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.

The shared opportunity query now accepts an explicit trusted measured-feet
metric alongside the existing grid metric. Physical callers supply before/after
distances rather than manufacturing cell transitions. The same actor bounds,
capability, Reaction, sight, resource provenance, teleport, Disengage and Addle
rules still apply. Current creature positions remain in the logical battle;
measured mode does not pretend its destination is a grid square.

Experimental's `lab_keep_body` runs between actual controller advance and body
publication. It uses the controller's real pre-step body, not the interpolated
render position. `xc_depart` projects that origin into shared combat, computes
terrain-adjusted three-dimensional feet distances, asks the shared query, and
executes selected declared monster attacks through `rs_stat_opportunity`.
Ordinary sight combines the world dungeon sight query with shared Blinded and
Invisible state. A borrowed player seat preserves actual slot/Quickened and
other turn marks; no press refill is applied. Reaction availability comes from
the shared monster, independently of its Action seat.

Accepted damage follows the existing contact result's hit, stop and number
presentation and shared vitals. It is retained if the player falls. Movement
permission is checked before departure and after attacks: lethal damage keeps
the prior horizontal body position/velocity without undoing any accepted
Reaction. Nonlethal damage permits the proposed move. Vertical gravity is not
frozen by a horizontal movement refusal. The C face transports the proposed
and admitted bodies; decisions remain Coax. Existing camera constraint behavior
is preserved. Teleport placement publishes its new origin separately; only
subsequent controller locomotion is considered a departure.

## Verification

Missing API/field regressions preceded implementation. Shared metric entry:
201 checks pass. Updated trigger gate kills 95/95 mutants, zero survivors/no
exemptions. Physical adapter regressions pass for lethal and nonlethal departure,
retained saved budgets, hit/number presentation, Disengage, Invisible, Blinded,
Addle and actual controller origin versus stale render position. The separate
teleport regression passes. Existing selected Experimental combat: 31 pass.

Complete checked Lab Wasm build passes on final product sources in 35.0 seconds,
2,526,127 bytes, without new work-budget overrides. This also compiles the actual
C host body-admission call. The temporary output name has no registered size
budget; no shipped target was added. Existing compiler-source and near-limit
advisories remain. Tools-check/diff check pass.

```
coax/bin/coax test aethyr/rules/srd/measured-opportunity-checks.coax --library
coax/bin/coax mutate aethyr/rules/srd/measured-opportunity-checks.coax --library --unit aethyr/rules/srd/opportunity-trigger.coax --jobs 4
coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental physical opportunity'
coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental teleport'
coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental combat'
coax/web/wasm.sh apps/openagents-web/fivelab.coax apps/openagents-web/fivelab-face.c /tmp/srd-physical-opportunity-fivelab.wasm apps/openagents-web/everglade-step.coax aethyr/sticks.coax
coax/web/tool.sh apps/openagents-web/experimental-opportunity-native.coax
coax/web/tool.sh tools-check
```

## Remaining scope

This wires monster Reactions to **player controller locomotion**, not player
Reactions to monster AI movement. Monster movement still needs its own before-
departure boundary. Forced displacement and Action/Bonus/Reaction movement need
explicit provenance if added to this controller. Special senses/obscuration,
complete source on-hit programs, actual per-creature visibility beyond the
current world sight query, optional reactor follow-up movement and broad timing
qualification remain. Shared strike defense distances still use the world's
existing projected grid; this receipt qualifies the new departure measurement,
not every physical distance-dependent attack rule. Continuous movement cost/
Speed profiles and complete used-monster source reconciliation remain separate
active work. E2's social no-bystander-harm profile is preserved. No issue closure.

Actual native execution prints PASS PHYSICAL-OPPORTUNITY for all four player
Shield slot-spent/Quickened combinations through the physical adapter, with
correct slot payment, HP and restored frame/budget state. The initial native
fixture had AC 14 against the fixed Guard-derived roll of 13, so no Shield
could trigger; correcting fixture AC to 13 made the intended interaction real.
It did not require a product policy change.
