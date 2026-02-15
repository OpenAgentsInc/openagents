# Optimizers (Policy Bundles and On-Disk Layout)

This doc defines the canonical on-disk expectations for **policy bundles** and optimizer-produced artifacts.

Normative references:
- `docs/adr/ADR-0015-policy-bundles.md` (policy bundle semantics)
- `docs/adr/ADR-0008-session-storage-layout.md` (base paths)

## Policy Bundle Storage

Per `docs/adr/ADR-0008-session-storage-layout.md`, bundles live under:

```text
${OPENAGENTS_HOME}/policies/
  {policy_bundle_id}/
```

## Bundle Layout (Minimum)

Each `{policy_bundle_id}` directory MUST contain:

```text
{policy_bundle_id}/
  manifest.json
  signatures/
  modules/
  metrics/
```

Recommended additional directories:
- `datasets/` (dataset references/hashes used for compilation)
- `scorecards/` (evaluation outputs)
- `notes/` (human-readable bundle notes)

## manifest.json

`manifest.json` MUST satisfy the minimum fields in:
- `docs/dse/COMPILER-CONTRACT.md`

## Optimizer Outputs

Optimizers SHOULD record:
- optimizer id + version
- candidate ids and scores
- counterfactual comparisons vs baseline

The purpose is attribution and rollback, not perfect reproducibility of optimizer internals.

