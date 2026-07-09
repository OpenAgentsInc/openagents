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

- User can search a transcript by phrase.
- User can copy a timestamped quote.

```productspec-ai-evals
- id: quote_relevance
  type: rubric
  input_set: evals/quote-search-cases.jsonl
  evaluator: llm_judge
  pass_threshold: 0.85
  checks:
    - returned passage answers the query
    - citation links to the correct timestamp
```

## Success Metrics

- 40% of weekly active researchers copy at least one timestamped quote.
