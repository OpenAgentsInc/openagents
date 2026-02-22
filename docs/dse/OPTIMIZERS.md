# Optimizers (Bundle Output and Storage Expectations)

Optimizers produce candidate bundles and selection evidence.

## Required Bundle Layout

```text
${OPENAGENTS_HOME}/policies/{policy_bundle_id}/
  manifest.json
  signatures/
  modules/
  metrics/
```

## Recommended Additions

- `datasets/`
- `scorecards/`
- `notes/`

## Optimizer Trace Requirements

Record at minimum:

- optimizer id/version
- candidate ids and scores
- baseline comparison

Purpose: attribution, debugging, and rollback clarity.
