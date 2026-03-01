# CAD Finishing Ops (Fillet/Chamfer/Shell)

This document defines the Wave 2 production finishing-operation layer replacing
placeholder-only behavior for:

- fillet
- chamfer
- shell

Implementation:

- `crates/cad/src/finishing_ops.rs`

## Operation Keys

- `fillet.v2`
- `chamfer.v2`
- `shell.v1`

## Failure Classification

- `FINISHING_INVALID_INPUT`
- `FINISHING_TOPOLOGY_RISK`
- `FINISHING_ZERO_THICKNESS_RISK`
- `FINISHING_KERNEL_REJECTED`

Each classification includes deterministic remediation text.

## Fallback Policy

When `allow_fallback=true` and operation risk thresholds are exceeded:

- Result status: `fallback_kept_source`
- Source geometry hash is preserved
- Explicit `fallback_message` is emitted
- Structured warning is emitted (`CAD-WARN-FILLET-FAILED`) with classification metadata

When `allow_fallback=false`:

- Evaluation fails with classified `CadError::EvalFailed`
- Error reason includes classification code + remediation guidance

## Feature Graph Integration

Each op supports:

- `to_feature_node()`
- `from_feature_node(...)`

Node params are canonicalized for deterministic parsing (sorted refs, explicit
fallback flag).

## Test Coverage

- Safe-path fillet apply
- Classified fillet fallback and hard-failure paths
- Chamfer node round-trip and deterministic apply path
- Shell fallback classification and message coverage
