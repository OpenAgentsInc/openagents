---
spec_format_version: "0.1"
title: "Fixture: Bad Success Metric"
artifact_type: "prd"
author: "OpenAgents"
created_at: "2026-07-08T00:00:00Z"
updated_at: "2026-07-08T00:00:00Z"
---

## Problem

This fixture exists to prove malformed success metrics are rejected properly.

## Hypothesis

If a success metric omits required fields and uses a camelCase id, the
validator must reject the document with invalid_success_metric.

## Scope

In: one malformed structured success metric block for the negative test.

## Acceptance Criteria

- The validator rejects this file.

## Success Metrics

```productspec-success-metrics
- id: BadMetricId
  metric: missing_most_fields
```
