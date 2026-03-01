# CAD Error Model

This document defines the `openagents-cad` error taxonomy and result conventions.

## Goals

- Keep failures explicit and deterministic.
- Map all domain failures to structured UI/activity events.
- Provide stable remediation hints for operator and developer workflows.

## Core Types

- `CadResult<T>`: crate-wide result alias (`Result<T, CadError>`).
- `CadErrorCode`: stable code enum for telemetry and UI mapping.
- `CadError`: typed failure variants (parse/eval/query/export/policy/graph/etc).
- `CadErrorEvent`: structured payload for UI surfaces and activity feed ingestion.

## Variant Set (MVP)

- `NotImplemented`
- `ParseFailed { reason }`
- `EvalFailed { reason }`
- `QueryFailed { reason }`
- `ExportFailed { format, reason }`
- `InvalidPrimitive { reason }`
- `InvalidPolicy { reason }`
- `Serialization { reason }`
- `InvalidFeatureGraph { reason }`

## Conventions

- Every error maps to a stable `CadErrorCode` via `CadError::code()`.
- Every error includes a stable remediation hint via `CadError::remediation_hint()`.
- UI/event mapping uses `CadError::to_event(operation)` and includes:
  - `code`
  - `operation`
  - `message`
  - `remediation_hint`
  - `retryable`
- No silent error downgrades are allowed in `crates/cad`.

## Retry Semantics

- Retryable by default for transient domain lanes (`EvalFailed`, `QueryFailed`, `NotImplemented` while gated in caller workflows).
- Non-retryable for schema/validation/export configuration errors unless inputs change.

## Reviewer Verification

- `cargo test -p openagents-cad`
- Confirm `error.rs` tests cover:
  - code mapping stability
  - event mapping fields
  - contract stability for core variants
