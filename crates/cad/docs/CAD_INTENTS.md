# CAD Intent Schema (Wave 1)

This document defines the strict CAD intent contract used between chat adapters and CAD execution.

## Module

- `crates/cad/src/intent.rs`

## Rules

- Payload must be a JSON object with required string field `intent`.
- Intent name must be in the allow-list.
- Unknown/invented operations are rejected.
- Unknown fields are rejected per intent payload.
- Validation errors are machine-readable via `CadIntentValidationError`.

## Allowed Intents

- `CreateRackSpec`
- `GenerateVariants`
- `SetObjective`
- `AdjustParameter`
- `SetMaterial`
- `AddVentPattern`
- `Select`
- `CompareVariants`
- `Export`

## Error Contract

`CadIntentValidationError` includes:

- `code`
- `intent`
- `field`
- `message`

Stable error codes include:

- `CAD-INTENT-INVALID-JSON`
- `CAD-INTENT-INVALID-SHAPE`
- `CAD-INTENT-MISSING-INTENT`
- `CAD-INTENT-UNKNOWN-OP`
- `CAD-INTENT-INVALID-PAYLOAD`
- `CAD-INTENT-INVALID-FIELD`
- `CAD-INTENT-INVALID-RANGE`
- `CAD-INTENT-INVALID-NUMBER`
