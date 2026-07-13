---
spec_format_version: "0.1"
title: "With Provisional Success Metric"
artifact_type: "prd"
author: "ProductSpec"
created_at: "2026-07-09T00:00:00Z"
updated_at: "2026-07-09T00:00:00Z"
---

## Problem

Teams need to name the post-launch outcome before the baseline is fully known.

## Hypothesis

If the spec preserves provisional targets explicitly, teams avoid recording guesses as committed intent.

## Scope

```productspec-scope
in:
  - provisional success metric target status
out:
  - analytics instrumentation
cut:
  - automated baseline calculation
```

## Acceptance Criteria

```productspec-acceptance-criteria
- id: AC-1
  criterion: A provisional target names the owner responsible for committing the target.
```

## Success Metrics

```productspec-success-metrics
- id: SM-1
  metric: baseline_calibrated_metric_rate
  target: tbd
  target_status: provisional
  target_owner: Data lead
  window: within 14 days after launch
```
