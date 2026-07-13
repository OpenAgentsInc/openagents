---
spec_format_version: "0.1"
title: "AI Quote Search"
artifact_type: "prd"
spec_revision: 1
author: "ProductSpec"
created_at: "2026-07-06T00:00:00Z"
updated_at: "2026-07-06T00:00:00Z"
---

## Problem

Researchers lose time finding exact quotes in long video transcripts.

## Hypothesis

If quote search returns cited transcript passages, researchers will trust the transcript as a source.

## Scope

In: transcript search, timestamp citations, and quote copy.

## Acceptance Criteria

```productspec-acceptance-criteria
- id: AC-1
  criterion: User can search a transcript by phrase.
- id: AC-2
  criterion: User can copy a timestamped quote.
```

```productspec-ai-evals
- id: EVAL-1
  type: llm_judge
  cases:
    - input: "Representative input for this eval."
      expected: "Expected behavior for this eval."
  evaluator: llm
  pass_threshold: 0.85
```

## Success Metrics

```productspec-success-metrics
- id: SM-1
  metric: weekly_active_researchers_copying_timestamped_quote
  target: ">= 40%"
  window: weekly
```
