# Metrics (Evaluation and Optimization)

Metrics score signature/module outputs for selection and rollout.

## Metric IDs

Use stable ids:

- `oa.metric.<domain>.<name>.v<major>`

## Metric Classes

1. Proxy metrics: cheap structural/policy checks.
2. Truth metrics: expensive correctness or verification checks.

## Governance

Metric changes that affect rollout decisions must include verification evidence and compatibility notes.
