---
spec_format_version: "0.1"
title: "Malformed Applies To"
artifact_type: "prd"
spec_revision: 1
author: "ProductSpec"
created_at: "2026-07-09T00:00:00Z"
updated_at: "2026-07-09T00:00:00Z"
applies_to:
  - path: "apps/web/src/transcripts/"
    component: "transcript-search"
---

## Problem

Researchers lose time finding exact video quotes.

## Hypothesis

If transcript search returns timestamped passages, researchers will cite videos faster.

## Scope

```productspec-scope
in:
  - transcript search
out:
  - team libraries
cut:
  - speaker labels
```

## Acceptance Criteria

```productspec-acceptance-criteria
- id: AC-1
  criterion: User can search one transcript by phrase.
```

## Success Metrics

```productspec-success-metrics
- id: SM-1
  metric: copied_timestamped_quote_rate
  target: ">= 35%"
  window: within 7 days
```
