# CAD Units, Tolerances, and Robustness Policy

Related issue: [#2456](https://github.com/OpenAgentsInc/openagents/issues/2456)

## Canonical Units

- Canonical CAD unit: `mm`
- Public CAD parameter inputs and displayed dimensions are millimeter-based by default.

Code reference:

- `crates/cad/src/policy.rs`
- `CANONICAL_UNIT`

## Tolerance Defaults

- Baseline tolerance constant: `BASE_TOLERANCE_MM = 1e-3`
- Minimum accepted positive primitive dimension: `MIN_POSITIVE_DIMENSION_MM = BASE_TOLERANCE_MM`
- Default modeling policy mode: `ModelingPolicy::Tolerant`

Code reference:

- `crates/cad/src/policy.rs`
- `resolve_tolerance_mm()`

## Robustness Rules

1. Primitive dimensions must be greater than minimum tolerance.
2. Boolean path tolerance must be positive.
3. Invalid policy/primitive values must return explicit errors.
4. No silent fallback to undefined tolerances.

Code reference:

- Primitive validation: `crates/cad/src/primitives.rs`
- Eval tolerance usage: `crates/cad/src/eval.rs`
- Boolean tolerance usage: `crates/cad/src/boolean.rs`

## Failure Surfacing

- Invalid primitive input returns `CadError::InvalidPrimitive`.
- Invalid tolerance/policy returns `CadError::InvalidPolicy`.
- Unimplemented boolean execution remains explicit via `CadError::NotImplemented`.
