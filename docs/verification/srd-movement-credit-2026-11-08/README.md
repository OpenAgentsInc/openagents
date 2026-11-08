# Current-turn walking movement accounting

Partial #13040/#13039 progress on baseline `1c5b5f1571`, compiler 0.52.0.
Source: pinned SRD 5.2.1 p.24 movement, p.180 Dash, p.181 Exhaustion and
p.186 Prone. PDF SHA-256:
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.

## Behavior and contract

The internal turn retains its last ordinary-plus-Dash allowance and any debt
when movement already spent exceeds a reduced allowance. Repricing preserves
`remaining - debt`, so reducing and then restoring Speed cannot refill movement
already spent. Every walking Dash follows modified Speed; an Action and Bonus
Action Dash both count. Strict timing uses Speed for the ordinary grant; the
action profile keeps its proportional ordinary grant and full-Speed Dash.
Exhaustion gain/recovery, Slow/halving, tracked zero-Speed conditions and
restoration feed the same calculation. No new protocol field is published.

`rs_movement_left` explicitly reconciles a logical actor in its own turn.
Step and stand prepare a local repriced transaction and publish only after
successful movement payment; refusal preserves position and stored budget.
Dash validates and spends the correct action before binding the old allowance
and adding its grant. That initial binding matters for legacy caller-created
untracked turns. New ordinary turns initialize tracked credit and reset debt
and Dash count. Steady Aim clears residual feet as well as whole-square credit.

The Lab/siege/MMO walking loops query the reconciled allowance rather than
skip movement based on stale raw squares. Saved seats copy the complete turn,
including debt, while preserving another actor's turn. Raw movement fields
are a stored projection, not an independent authority after a Speed change;
callers should use the query before inspecting spendable movement. Existing
explicit flat credit added by other feature paths is retained as flat credit.

## Source-derived fixtures and verification

Initial fixtures precede implementation; red records the missing query. New
fixtures exercise actual steps with Speed loss, overspending, restoration,
multiple Dashes, Exhaustion recovery, action timing, temporary zero Speed,
legacy turns and native standing/refusal. An exhaustive small-range arithmetic
oracle checks all initial allowances 0..41, every spent amount and replacement
allowances 0..41, then restoration. The oracle derives remaining allowance
minus spent feet independently of the implementation's stored delta/debt.

Earlier Dash-only fixtures now establish their modified Speed before starting
the test turn, so their manually supplied residual movement has a coherent
basis. The old saved-seat and Prone assertions are corrected to include the
previously omitted ordinary-grant reduction. Assertions for costs, residual
feet, refusal and actor isolation remain. A saved-seat regression verifies
actual movement debt is retained across the adapter and restoration.

All commands are local and completed below fifteen minutes:

- `coax/bin/coax test aethyr/rules/srd/movement-credit-checks.coax --library`: 241 checks pass.
- `coax/bin/coax mutate aethyr/rules/srd/movement-credit-checks.coax --library --unit aethyr/rules/srd/movement-credit.coax --jobs 4`: 57 killed, zero survivors. Later additive fixtures leave the policy unchanged; its receipt is reused.
- Same mutation entry with `--unit aethyr/rules/srd/dash.coax`: 82 killed, zero survivors. Initial survivors exposed the missing legacy untracked-turn fixture; no exemptions.
- `coax/bin/coax test aethyr/rules/srd/prone-checks.coax --library`: 181 checks pass.
- `coax/bin/coax test apps/openagents-web/fivebar-test.coax --library`: 206 checks pass.
- `coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental combat'`: all 21 selected checks pass; no filtered authority coverage claim.
- `coax/web/tool.sh aethyr/rules/srd/movement-credit-native.coax` and the updated `dash-native.coax`: actual native MOVEMENT-CREDIT and DASH-MOVEMENT pass, exit 0. These execute golden scenarios shared with the evaluator; they are not separate independent oracles.
- `coax/web/tool.sh tools-check`: zero new tools without a reason.

The existing full compliance report's deferred Polymorph failure remains
unsuppressed in its previous receipt; it is not rerun for this walking slice.

## Remaining

#13040 and #13039 stay open. Special movement modes and switching speeds,
feature-specific immediate/free movement (Tactical Shift/Withdraw/Athlete),
all condition permissions, opportunity/Disengage windows, rests and complete
world controls remain unqualified. This accounting does not by itself make
those feature paths source-correct. It resolves the named mid-turn walking
credit gap in the preceding Dash/Exhaustion receipts, not every movement rule.
