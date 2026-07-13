---
spec_format_version: "0.1"
title: "With Fenced Heading"
artifact_type: "prd"
author: "ProductSpec"
created_at: "2026-07-09T00:00:00Z"
updated_at: "2026-07-09T00:00:00Z"
---

## Problem

Teams adopting a spec format have no in-repo example, so every team invents a different shape.

## Hypothesis

If the repo ships a starter spec, new teams copy it instead of inventing a shape.

## Scope

In: a starter file at docs/product-specs/example.product-spec.md, containing:

~~~markdown
## Problem

Who is hurting.

## Hypothesis

The causal bet.
~~~

Out: a generator CLI.

## Acceptance Criteria

```productspec-acceptance-criteria
- id: AC-1
  criterion: A new repo can copy the starter and validate it unchanged.
```

## Success Metrics

```productspec-success-metrics
- id: SM-1
  metric: share_of_new_specs_derived_from_starter
  target: ">= 50%"
  window: within a quarter of the starter landing
```
