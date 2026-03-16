# Export And Deployment Artifact Contracts

> Status: canonical `PLIB-221` / `#3736` reference record, updated 2026-03-16
> after landing the first bounded exportable-graph and deployment-artifact
> contracts in `crates/psionic/psionic-ir/src/lib.rs` and
> `crates/psionic/psionic-compiler/src/lib.rs`.

This document records the current bounded export and deployment surface for
Psionic.

## Canonical Runner

Run the export/deployment harness from the repo root:

```bash
scripts/release/check-psionic-export-deployment-artifact-contracts.sh
```

## What Landed

`psionic-ir` now exposes:

- `Graph::exportable_graph_contract(...)`
- `FunctionalGraph::exportable_graph_contract(...)`
- `ExportableGraphContract`

`psionic-compiler` now exposes:

- `CompilerArtifacts::deployment_artifact_contract(...)`
- `DeploymentArtifactContract`
- `ExportDeploymentArtifactSemanticsReport`
- `builtin_export_deployment_artifact_semantics_report()`

## Current Honest Posture

Today Psionic has a first graph-first handoff contract, but it does **not**
claim full checkpoint migration or every external deployment format.

The bounded seeded surface now makes these seams explicit:

- export-safe functionalized graph contracts with named input/output bindings
- deployment artifact bundles over lowered execution plans
- topology-aware deployment bundles when explicit execution topology exists
- explicit refusal for opaque backend-extension graphs that are not export-safe
- explicit refusal for graph-digest mismatches between the export envelope and
  the compiled deployment artifact

## Why This Matters

This report prevents two failure modes:

- treating raw checkpoints as the only library-level handoff format
- letting export-safe graph work stay disconnected from deployment bundles and
  downstream packaging

The point of this issue is to make graph-first export and deployment contracts
machine-legible without claiming every packaging or portability workflow is
finished.
