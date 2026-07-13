---
spec_format_version: "9.9"
title: "Unsupported Version"
artifact_type: "prd"
author: "ProductSpec"
created_at: "2026-07-04T00:00:00Z"
updated_at: "2026-07-04T00:00:00Z"
---

## Problem

This document declares an unsupported version.

## Hypothesis

If the version is unsupported, validation should fail.

## Scope

In: unsupported-version conformance.

## Acceptance Criteria

```productspec-acceptance-criteria
- id: AC-1
  criterion: Validator returns unsupported_version.
```

## Success Metrics

```productspec-success-metrics
- id: SM-1
  metric: invalid_fixture_validation_failure
  target: "100%"
  window: conformance run
```
