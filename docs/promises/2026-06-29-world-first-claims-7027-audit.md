# World-First Claims Audit for #7027

Date: 2026-06-29

Issue: [#7027](https://github.com/OpenAgentsInc/openagents/issues/7027)

## Scope

This audit covers the four active world-first / largest-force promise records:

- `claims.world_first_ai_training_paid_bitcoin.v1`
- `claims.world_first_public_llm_computer_training_run.v1`
- `claims.pursued_world_first_largest_agentic_sales_force.v1`
- `claims.pursued_world_first_largest_sales_force.v1`

## Decision

No promise is ready to turn green.

The LLM-computer claim has a focused evidence pack and definition, but it still
needs an owner-signed receipt-first transition before public copy can present it
as achieved. The Bitcoin-paid-training claim still needs its own
dereferenceable qualified evidence bundle and the same owner-signed transition.
The two largest-force claims remain explicit pursuits, not achieved records.

## Blockers Kept Explicit

| Promise | State | Why it stays non-green |
| --- | --- | --- |
| `claims.world_first_ai_training_paid_bitcoin.v1` | red | Missing a qualified evidence pack tying the narrowed wording to receipts; missing owner-signed upgrade. |
| `claims.world_first_public_llm_computer_training_run.v1` | red | Evidence pack exists, but owner-signed upgrade is still missing. Public copy must keep Percepta credit, executor-construction wording, and no-gradient-descent caveats. |
| `claims.pursued_world_first_largest_agentic_sales_force.v1` | planned | No real, sized, independently countable agentic sales force exists. |
| `claims.pursued_world_first_largest_sales_force.v1` | planned | The named roughly seven-million-agent bar is unmet, and the comparison figure is not OpenAgents-verified. |

## Copy Gate

Public copy must not use bare "world first", "has the largest sales force",
"largest sales force in the world", "world record achieved", or equivalent
unqualified achievement language for these records.

Allowed copy is limited to the registry's qualified wording:

- Claim 1 may be discussed only with the full Bitcoin + replay-verified training
  compute + own consumer devices qualifiers, and only as red pending receipt
  upgrade.
- Claim 2 may be discussed only as a public/open-contributor LLM-computer
  training-run claim with Percepta credit, executor-construction semantics, and
  no gradient-descent model-training overclaim.
- The largest-force records may be discussed only as clearly labeled pursuits or
  aspirations.

## Follow-Up Required

To close the remaining gates, prepare one owner-signed transition request per
claim that is actually ready. Each request must cite dereferenceable receipt
refs, the prior-art review, exact qualifier language, and the relevant refuse
list. Until those receipts exist, the machine-readable registry must keep the
records red/planned and keep the blockers listed.
