# Disengage state and Patient Defense costs

Partial #13040/#13044 work on f0e95ea900, cached compiler 0.52.0.
Pinned SRD 5.2.1 Disengage p.180, Cunning Action p.62 and Patient Defense
pp.50–51. Source PDF SHA-256 remains the ledger's pinned 8974902d…e3d87.

The new authority admits an Action Disengage, Rogue Cunning Action Disengage,
Monk level-2 free Patient Defense Disengage, and its paid Disengage-plus-Dodge
option. The ordinary Action entry is `rs_disengage`; Bonus Action choices use
`rs_disengage_use` with named RS_DISENGAGE_* modes. The existing paid
`rs_patient_defense` delegates its costs/state to the same authority and
preserves its feature event/message.

Admission validates logical actor/turn, current action permission, the correct
Action/Bonus Action budget, mode and feature prerequisites before any spending.
Paid Patient Defense spends one Focus; free Patient Defense needs none. Unknown
modes, wrong turns, unregistered backing records, incapacity and missing
features refuse transactionally. Zero Speed alone does not prevent Disengage.
The flag belongs to RsTurn, survives movement and saved-seat transitions, and
retires at the actual end of that turn. It does not renew movement/Reactions.
Dodge keeps its distinct next-turn-start lifetime.

## Evidence

The initial fixtures failed on missing APIs/state. Final full entry passes 193
checks. Policy mutation kills 79 mutants, zero survivors, no exemptions. A
consumer-local authority exercise test was subsequently added without changing
policy procedures; the passing mutation receipt is reused. The full hotbar
entry passes 221 checks; the subsequently added saved-seat isolation fixture
passes separately (222-check composition). Twenty-nine selected Experimental
combat checks pass. Actual native tool prints PASS DISENGAGE-STATE and exits 0.
Tools-check and diff check pass. Exact logs/source/compiler hashes retained.

```
coax/bin/coax test aethyr/rules/srd/disengage-checks.coax --library
coax/bin/coax mutate aethyr/rules/srd/disengage-checks.coax --library --unit aethyr/rules/srd/disengage.coax --jobs 4
coax/bin/coax test apps/openagents-web/fivebar-test.coax --library
coax/bin/coax test apps/openagents-web/fivebar-test.coax --library --only 'a saved seat retains Disengage'
coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental combat'
coax/web/tool.sh aethyr/rules/srd/disengage-native.coax
```

## Not complete yet

This establishes correct admission/payment and turn state; **opportunity attacks
are still not executed**, so the defensive consequence is not yet qualified.
The next core movement/Reaction slice must consume this flag, visibility/reach,
Addle and free-movement exceptions. Paid Step of the Wind's combined Dash/
Disengage/jump option, Tactical Shift, visible free-choice controls and complete
world/runtime qualification also remain open. No #13040/#13044 closure, complete
Monk feature qualification or new visible hotbar action is claimed. The prior
native room-host binary predates this core change and is not claimed as a new
Disengage host artifact; the new native component is the retained evidence.
