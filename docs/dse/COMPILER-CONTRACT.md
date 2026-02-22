# Compiler Contract (Policy Bundle Manifests)

Defines required properties for compiled policy bundles.

## Bundle Immutability

1. Bundle content is immutable once created.
2. Promotion/rollback is done by selecting a different bundle id.

## Minimum Manifest Fields

- `policy_bundle_id`
- `created_at`
- `rollout_state`
- `compiler.name`
- `compiler.version`
- `artifacts.signatures[]`
- `artifacts.modules[]`
- `artifacts.metrics[]`

Optional but recommended:

- `inputs.datasets[]`
- `inputs.code_hash`

## Attribution Requirements

Session artifacts should record bundle provenance:

- `RECEIPT.json.policy_bundle_id`
- `REPLAY.jsonl` `SessionStart.policy_bundle_id`

See `docs/execution/ARTIFACTS.md` and `docs/execution/REPLAY.md`.
