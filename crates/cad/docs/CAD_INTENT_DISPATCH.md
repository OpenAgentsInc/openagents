# CAD Intent Dispatcher

This document defines typed intent execution for CAD state mutation.

## Module

- `crates/cad/src/dispatch.rs`
- Integrated execution flow: `crates/cad/src/intent_execution.rs`

## Entry Points

- `dispatch_cad_intent(&CadIntent, &mut CadDispatchState)`
- `dispatch_cad_payload_json(&str, &mut CadDispatchState)`
- `reject_free_text_mutation(&str)`

## Guarantees

- Only schema-validated `CadIntent` variants are executable.
- Free-text state mutation is rejected.
- Dispatch produces typed command receipts (`CadDispatchReceipt`).
- State revision increments deterministically per accepted command.

## Typed Command Coverage

- `CreateRackSpec`
- `GenerateVariants`
- `SetObjective`
- `AdjustParameter`
- `SetMaterial`
- `AddVentPattern`
- `Select`
- `CompareVariants`
- `Export`
