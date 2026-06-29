# Artanis RLM Composition Architecture

> Status: design + first implementation slice for issue #6654, 2026-06-28.
> This is operator architecture, not public product copy. It does not widen
> spend, deploy, payout, merge, or destructive authority.

## Problem

The #6651 truncation fix raised `max_tokens` and continued after a `length`
finish. That is a useful safety net, but it still treats Artanis as one bounded
model call whose answer happens to be stitched after the fact.

Artanis needs the opposite model: long answers are a Recursive Language Model
run. The operator decomposes the owner request into typed subqueries, answers
those subqueries through Khala, records evidence packets, and composes the final
answer from those packets. No single completion is the architectural limit.

## Shape

The first implementation slice lives in
`apps/openagents.com/workers/api/src/artanis-operator.ts`.

The conductor is:

1. `Run.Init`: detect long-form owner asks such as architecture, design,
   report, roadmap, strategy, detailed plan, or long write-up requests.
2. `SubQuery.Submit`: ask Khala for compact JSON describing 2-4 typed
   subqueries under `program_signature.artanis.operator.rlm.compose.v1`.
3. `SubQuery.Return`: execute each subquery as a separate
   `openagents/khala` call with the normal Artanis persona and grounded context.
4. `Run.Done`: compose the final owner answer from the subquery evidence
   packets in one final Khala call.

Each returned turn carries an `rlmTrace` with:

- `programRef`: `program.artanis.operator.rlm.compose.v1`
- `signatureRef`: `program_signature.artanis.operator.rlm.compose.v1`
- `decomposition`: typed subquery records
- `evidence`: subquery answers plus evidence refs and served model IDs
- `compositionInstruction`
- `used`

## Implemented Closure Slice

Issue #6654 is satisfied by the current operator path at the reasoning layer:

- Long-form owner turns are detected by `shouldUseArtanisRlmComposition`.
- The operator asks Khala for a typed decomposition instead of answering the
  owner directly.
- Each subquery is executed as its own `openagents/khala` request.
- The final owner answer is composed from the returned evidence packets.
- The returned `rlmTrace` records the program ref, Blueprint signature ref,
  decomposition, evidence packets, served model ids, and composition
  instruction.
- The #6651 continue-on-length loop still wraps planner, subquery, and
  composition calls, so single-completion limits are a local fallback rather
  than the architecture.

The regression coverage is
`apps/openagents.com/workers/api/src/artanis-operator.test.ts`, specifically the
`#6654 Artanis RLM composition` tests. The focused verification command is:

```sh
bun run --cwd apps/openagents.com/workers/api test -- src/artanis-operator.test.ts
```

## FRLM Projection Primitives

The broader federated conductor primitives already exist in
`apps/pylon/src/frlm-conductor-execution.ts` and are covered by
`apps/pylon/tests/frlm-conductor-execution.test.ts`.

Those primitives model the historical FRLM conductor shape without granting new
runtime authority:

- `planFrlmConductorExecution` projects recursive fanout, linear fallback,
  budget/depth blockers, evidence refs, and execution-plan refs.
- `scheduleFrlmConductor` computes public-safe recursive batches and local
  fallback steps from Pylon slot refs.
- `composeFrlmRecursiveResponse` deterministically composes completed subquery
  response segments.
- `emitFrlmRlmStepTrace` emits public-safe RLM step traces with redacted content
  and evidence refs.

The operator implementation above is the owner-chat reasoning slice; the Pylon
FRLM module is the projection/scheduling slice. A later integration can wire the
two by persisting the operator `rlmTrace` as FRLM evidence rows and then letting
the conductor schedule recursive Pylon/Codex fanout under its existing budget,
slot, and fallback blockers.

## Blueprint Grounding

This is intentionally aligned with `autonomous-ops-v1`:

- The subqueries are typed records, not keyword-routed intents.
- Every subquery carries `signatureRef` and `evidenceRefs`.
- The final answer composes from evidence packets rather than presenting a raw
  model continuation as the whole run.
- Signature 6 still runs after composition, so runnable commands, paths,
  scripts, and API endpoints remain gated by actual lookups or marked
  speculative.

## Authority

The RLM conductor only reasons and composes. It does not execute risky actions.
Spend, payout, deploy, merge, destructive changes, and Pylon job dispatch remain
behind the existing Artanis approval-gate and gated-tool boundaries.

The #6651 continue-on-length loop remains in place for each individual planner,
subquery, or composition completion. It is now the fallback inside a larger RLM
run, not the primary architecture for long answers.

The current owner-chat slice deliberately suppresses operator tools while
running the RLM decomposition pass. That keeps this issue closed at the
composition layer without accidentally widening authority. Tool-backed recursive
execution should be promoted only after it carries explicit FRLM conductor rows,
BudgetPolicy refs, Pylon slot refs, and approval-gate evidence for any
state-changing action.
