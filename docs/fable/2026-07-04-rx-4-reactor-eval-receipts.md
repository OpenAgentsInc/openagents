# RX-4 Reactor Eval Receipts

Date: 2026-07-04

Issue: [#8274](https://github.com/OpenAgentsInc/openagents/issues/8274)

## Landed

`packages/reactor-contracts` now carries the Reactor-side eval receipt boundary:

- `openagents.reactor.eval_harness_profile.v1`
- `openagents.reactor.model_eval_receipt.v1`
- `openagents.reactor.eval_coverage_matrix.v1`
- `openagents.reactor.capability_copy_eval_decision.v1`

The harness profile is owned by Psionic and names four task classes:
`drafting`, `extraction`, `rag_over_corpus`, and `agent_tool_use`.

The seed fixture receipts cover the acceptance minimum: two models by two task
classes.

| Model | Task class | Target label |
| --- | --- | --- |
| `model.openai.gpt_oss.open_family` | `drafting` | `rx3_served_model` |
| `model.openai.gpt_oss.open_family` | `extraction` | `rx3_served_model` |
| `model.meta.llama.open_family` | `drafting` | `hosted_equivalent_large_model` |
| `model.meta.llama.open_family` | `extraction` | `hosted_equivalent_large_model` |

The RX-2 seed catalog now cites those measured receipt refs in the relevant
model `evalRefs`. Other catalog models still have no eval refs.

## Honesty Rules

The coverage matrix expands every catalog model across all four task classes.
Measured cells carry a receipt ref and score. Unrun cells carry
`measurementState: "not_measured"`, `receiptRef: null`, `score: null`, and a
`blocker.reactor.eval.not_measured` blocker. No unrun cell is represented as a
blank or as measured `0`.

Capability-copy decisions only return measured eval refs. A requested model/task
claim with no measured receipt returns `blocked_not_measured` and names the
missing task/model blocker.

## Boundaries

This is Reactor-side harness/receipt integration only. It does not implement or
move Psionic runtime execution machinery, deploy a Reactor node, authorize
customer serving, prove data custody, approve public capability copy, or green
any Reactor product promise.

## Verification

Source coverage:

- `packages/reactor-contracts/src/index.ts`
- `packages/reactor-contracts/src/index.test.ts`

Commands:

```sh
bun run --cwd packages/reactor-contracts test
bun run --cwd packages/reactor-contracts typecheck
```
