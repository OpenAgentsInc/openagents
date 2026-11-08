# Stunned movement and condition initiative

Partial #13039/#13040 progress on `c5b5cff3dd`, compiler 0.52.0. Primary source:
pinned SRD 5.2.1 p.183 Incapacitated/Invisible, p.185 Stunned and p.186 Prone.
PDF SHA-256:
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.

## Source correction

Stunned imposes Incapacitated, automatic Strength/Dexterity saving-throw failure
and Advantage on attacks against the creature. It does not impose Speed zero.
The old movement predicate incorrectly grouped it with Paralyzed and Petrified,
preventing walking and standing. `condition-permissions.coax` now owns that
predicate: Stunned and Incapacitated permit movement, subject to independent
Speed, other conditions, Halt/Dazed carriers and the movement budget. Actions,
Bonus Actions and Reactions remain unavailable; concentration/Dodge interruption,
save consequences and attack Advantage still use the existing shared lifecycle.
Stunned does not grant an automatic critical hit.

Incapacitated also imposes Disadvantage on Initiative. The new initiative edge
handles that implication for Paralyzed/Petrified/Stunned/Unconscious too, with
ordinary cancellation against Remarkable Athlete or Invisible Advantage.
`rs_add` consumes the resolved edge and the existing Exhaustion penalty. This
uses supplied condition state; it is not a visibility/obscuration oracle.

The previous pure standing-refusal fixture now combines Stunned with Grappled
for its independent movement blocker. A new actual-retention fixture proves
Stunned alone allows crawling, standing and walking with fresh action budgets
while still refusing actions and a prepared Reaction spell without spending.

## Evidence

Tests precede implementation. The red records Stunned movement and initiative
failures. Current-state fixtures cover every ordinary condition's movement
permission, independent blockers at every effect slot, life/death boundaries,
retained Stunned lifecycle, Prone costs, automatic saves and noncritical attacks,
all initiative source combinations and cancellation. A later seeded fixture
independently takes the lower/higher of two raw faces (one after cancellation)
and checks the actual initiative value and RNG state.

All commands finished locally below fifteen minutes:

- `coax/bin/coax test aethyr/rules/srd/condition-permission-checks.coax --library`: 205 checks pass.
- `coax/bin/coax mutate aethyr/rules/srd/condition-permission-checks.coax --library --unit aethyr/rules/srd/condition-permissions.coax --jobs 4`: 17 killed, zero survivors. Positive-count equivalences in the initial edge adapter were removed by expressing the boolean cancellation directly; no mutants were exempted. The later seeded fixture leaves those procedures unchanged; the passing receipt is reused.
- `coax/bin/coax test apps/openagents-web/fivebar-test.coax --library`: 209 checks pass. The new saved-seat case stands and walks while hotbar actions refuse, preserving another actor's budget.
- `coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental combat'`: 21 selected checks pass; no filtered authority coverage claim.
- `coax/web/tool.sh aethyr/rules/srd/condition-permission-native.coax`: actual native `PASS CONDITION-PERMISSIONS`, exit 0. The native golden scenario is shared with evaluator checks, not a second independent oracle. It precedes the additive seeded initiative fixture; executable procedures are unchanged.
- `coax/web/tool.sh tools-check`: zero new tools without a reason.

The unchanged Prone payment procedures retain their earlier mutation receipt;
only a source-incorrect test case changed. The broad compliance report's deferred
Polymorph failure remains unsuppressed in earlier receipts; it is not rerun here.

## Remaining

#13039/#13040 remain open. Experimental's separate hostile-AI `held` bit currently
combines action and movement restrictions; changing it alone could also enable
an invalid attack. That adapter needs the shared permission split as part of
#13046 rather than a claim of complete world migration here. General player
controls, visibility/line of sight, Incapacitated speech surfaces, condition
ownership/expiry, other movement permissions and action/reaction windows still
need qualification. No complete condition/world/SRD qualification is claimed.
