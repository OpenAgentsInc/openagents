---
spec_format_version: "0.1"
title: "Fixture: OpenAgents Extended Spec"
artifact_type: "hypothesis"
spec_revision: 2
author: "OpenAgents"
created_at: "2026-07-08T00:00:00Z"
updated_at: "2026-07-08T12:00:00Z"
linked_github_repo: "OpenAgentsInc/openagents"
custom_sections:
  - id: "custom-owner-gates"
    label: "Owner Gates"
    after: "success_metrics"
  - id: "custom-receipts"
    label: "Receipts"
    after: "custom-owner-gates"
  - id: "custom-promise-links"
    label: "Promise Links"
    after: "custom-receipts"
tool_metadata:
  openagents_epic: "8593"
  openagents_assurance_level: "hosted"
  openagents_marginal_cost_class: "subscription"
---

## Problem

Fixture problem statement with enough words to avoid the thin-section warning
in the reference validator behavior.

## Hypothesis

If this fixture parses, the OpenAgents extension surface round-trips through
our validator without violating the upstream standard.

## Scope

```productspec-scope
in:
  - custom sections
  - flat tool_metadata
out:
  - nested tool_metadata
cut:
  - upstream parser dependency
```

## Acceptance Criteria

- Every conformance fixture keeps its documented verdict.

```productspec-ai-evals
- id: fixture_eval
  type: deterministic
  input_set: fixtures/openagents/valid-extended.product-spec.md
  evaluator: automated_test
  pass_threshold: 1
  checks:
    - validator accepts this fixture
    - tool_metadata strips cleanly on export
```

## Success Metrics

```productspec-success-metrics
- id: fixture_metric
  metric: fixture_validation_pass_rate
  target: "= 100%"
  window: every test run
  segment: product-spec package tests
  source: bun_test
```

## Owner Gates

- None; this is a test fixture.

## Receipts

- The package test suite is the receipt.

## Promise Links

- None; fixtures never feed the promise registry.
