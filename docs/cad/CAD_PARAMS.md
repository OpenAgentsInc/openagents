# CAD Parameter Store

This document defines the Wave 1 CAD parameter store contract.

## Location

- `crates/cad/src/params.rs`

## Types

- `ScalarUnit`
  - `mm`
  - `deg`
  - `unitless`
- `ScalarValue { value, unit }`
- `ParameterStore { values: BTreeMap<String, ScalarValue> }`

## Validation Rules

- Parameter names:
  - must be non-empty
  - must start with ascii letter or `_`
  - may contain ascii alphanumeric, `_`, `.`, `-`
- Scalar values must be finite (`NaN`/`inf` rejected).
- Unit parsing rejects unsupported tokens.
- Required-lookup API enforces expected unit matching.

## Reviewer Verification

- `cargo test -p openagents-cad`
