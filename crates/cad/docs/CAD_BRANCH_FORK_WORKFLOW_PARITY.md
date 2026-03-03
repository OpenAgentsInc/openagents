# CAD Branch Fork Workflow Parity

Issue coverage: `VCAD-PARITY-118`

## Goal

Lock deterministic parity contracts for CAD Branch Fork Workflow Parity in Phase J - Full workspace parity lanes using the pinned vcad baseline.

## Contracts

- Capability scope parity is tracked for this issue ID and lane label.
- vcad source references used for this capability remain pinned and explicit.
- Generated parity manifests are deterministic across replay.

## Parity Artifacts

- vcad reference fixture:
  - `crates/cad/parity/fixtures/branch_fork_workflow_vcad_reference.json`
- Generated parity manifest:
  - `crates/cad/parity/branch_fork_workflow_parity_manifest.json`

## Validation

```bash
scripts/cad/parity-branch-fork-workflow-ci.sh
cargo run -p openagents-cad --bin parity-branch-fork-workflow
```
