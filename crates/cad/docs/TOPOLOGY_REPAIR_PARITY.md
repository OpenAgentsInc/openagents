# Topology Repair Parity

Issue coverage: `VCAD-PARITY-035`

## Purpose

Lock deterministic topology-repair parity contracts for post-boolean and post-finishing operation flows.

## Implemented Topology Repair Contract Layer

- Added deterministic repair substrate in `crates/cad/src/topology_repair.rs`:
  - `TopologyRepairRequest`
  - `TopologyRepairResult`
  - `TopologyRepairOperation` (`boolean`, `finishing`)
  - `TopologyDefectCounts`
  - `TopologyRepairAction`
  - `repair_topology_after_operation`
- Contract behavior:
  - no-defect requests return `no_repair_needed` with candidate hash passthrough.
  - deterministic action selection/reduction for boolean/finishing repair paths.
  - finishing/boolean unresolved critical defects can fallback to source geometry (`fallback_kept_source`) when allowed.
  - fallback warning code is stable: `CAD-WARN-NON-MANIFOLD`.
- Added deterministic parity lane:
  - `crates/cad/src/parity/topology_repair_parity.rs`
  - `crates/cad/src/bin/parity-topology-repair.rs`
  - `crates/cad/tests/parity_topology_repair.rs`
  - `scripts/cad/parity-topology-repair-ci.sh`
  - `crates/cad/parity/topology_repair_parity_manifest.json`

## Contracts Locked

- post-op repair receipts are deterministic for identical requests.
- no-defect and repaired/fallback statuses are stable and explicit.
- unresolved critical defects emit deterministic source-fallback behavior when configured.
- fallback warning code remains stable for downstream diagnostics/UI.
- invalid repair requests emit stable CAD primitive validation errors.

## Parity Artifact

- `crates/cad/parity/topology_repair_parity_manifest.json`

Generation/check commands:

```bash
cargo run -p openagents-cad --bin parity-topology-repair
scripts/cad/parity-topology-repair-ci.sh
```
