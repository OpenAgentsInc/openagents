# @openagentsinc/rlm-recall-eval

Hermetic dense-recall evaluation and honesty gate over OpenAgents transcript
shapes (RLM-07, issue #9143).

This package measures dense-history usefulness and cost on OpenAgents transcript
shapes before OpenAgents enables automatic semantic escalation, depth above one,
or any long-context quality claim. It **consumes** the published
`@openagentsinc/rlm` engine, corpus builder, program and result Schemas, and the
scripted-model plan interface. It **forks no evaluation module** into an
OpenAgents package.

## What it adds

- OpenAgents-specific synthetic transcript generators for constant (O(1)),
  linear-density (O(n)), and pair-density (O(n^2)) questions at representative
  history sizes. Generation is pure and deterministic and mirrors the desktop
  history corpus shape.
- Tier runners that compare a direct admitted model, deterministic Tier D,
  admitted Tier S at semantic depth 0, one-shot ModelMap fan-out, depth 1, a
  separately admitted higher depth, a bounded-window baseline, and a
  provider-compaction baseline.
- Scoring for answer correctness, citation exactness and coverage, abstention
  and partial honesty, modeled latency, model-call distribution, token
  distribution, and cost against a versioned OpenAgents price catalog snapshot.
- Two product-admission gates with explicit pass or fail criteria for automatic
  Tier S escalation and for depth above one. Both stay disabled. Enabling either
  is a separate product admission, not an engine default.

## Commands

- `pnpm --filter @openagentsinc/rlm-recall-eval test` runs the hermetic suite
  with scripted models and deterministic fixtures. No network. No spend.
- `pnpm --filter @openagentsinc/rlm-recall-eval run eval:hermetic` writes the raw
  aggregate artifact to `docs/rlm/rlm-recall-eval-hermetic-aggregate.json`.
- `pnpm --filter @openagentsinc/rlm-recall-eval run eval:live` is the
  owner-triggered live-provider command. It refuses to run without the
  `OPENAGENTS_RLM_EVAL_LIVE=1` flag, an admitted account ref, explicit call and
  spend caps, and a bound live model module. Live output is written separately
  and marked `meta.kind = "live"`, so live and hermetic results can never be
  confused.

## Honesty rules

- Unknown usage stays unknown. A model call without exact token counts is
  excluded from cost aggregates and labeled, never priced as zero.
- Retrieval-only tiers that surface evidence without combining it into an answer
  score as partial on pair tasks, not success.
- The evidence report makes no public product claim and separates retrieval
  quality evidence from runtime conformance.

The evidence report is `docs/rlm/2026-07-21-rlm-07-dense-recall-eval.md`.
