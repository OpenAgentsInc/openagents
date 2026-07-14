---
spec_format_version: "0.1"
title: "Agent Run ingest fixture"
artifact_type: "prd"
spec_revision: 3
author: "OpenAgents"
created_at: "2026-07-13T00:00:00Z"
updated_at: "2026-07-13T00:00:00Z"
---

## Problem

An agent's own completion report must remain distinguishable from independent proof.

## Hypothesis

If self-reports are typed at the lowest proof rung, downstream assurance cannot mistake them for observations.

## Scope

In: Agent Run 0.1 ingestion and ProductSpec identity cross-checks.

## Acceptance Criteria

```productspec-acceptance-criteria
- id: AC-1
  criterion: Ingest preserves a checked item as a claimed status only.
- id: AC-2
  criterion: Ingest preserves a not-checked claim without converting it to observation.
```

```productspec-ai-evals
- id: EVAL-1
  type: contains
  cases:
    - input: "producer"
      expected: "claimant"
  evaluator: deterministic
  pass_threshold: 1
```

## Success Metrics

```productspec-success-metrics
- id: SM-1
  metric: observation_promotions_from_self_report
  target: "0"
  window: every ingest
```
