# Compiler Contract (Policy Bundle Manifests)

This doc defines the canonical expectations for **compiled policy bundles** produced by the DSE/compiler layer.

Normative references:
- `docs/plans/archived/adr-legacy-2026-02-21/ADR-0015-policy-bundles.md` (rollout states, pin/rollback, attribution)
- `docs/plans/archived/adr-legacy-2026-02-21/ADR-0008-session-storage-layout.md` (`OPENAGENTS_HOME` layout)

## Bundle Immutability

- A bundle is immutable once created.
- Promotion/rollback is done by selecting a different bundle id as default, not by rewriting bundles in place.

## Manifest (Minimum Fields)

A policy bundle MUST contain a manifest file with:
- `policy_bundle_id` (string, required)
- `created_at` (string, ISO-8601, required)
- `rollout_state` (string, required; see ADR-0015)
- `compiler` (object, required)
  - `name` (string)
  - `version` (string)
- `inputs` (object, optional but recommended)
  - `datasets` (array of `{ id, hash }`)
  - `code_hash` (string)
- `artifacts` (object, required)
  - `signatures` (array; stable signature ids + paths)
  - `modules` (array; stable module ids + paths)
  - `metrics` (array; stable metric ids + paths)

## Attribution

Sessions MUST record the bundle id used:
- `RECEIPT.json.policy_bundle_id`
- `REPLAY.jsonl` (`SessionStart.policy_bundle_id`)

See `docs/execution/ARTIFACTS.md` and `docs/execution/REPLAY.md`.

