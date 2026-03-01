# CAD Model Validity Checks

Core validity checks live in `crates/cad/src/validity.rs`.

## Warning Code Contract

Stable warning codes:

- `CAD-WARN-NON-MANIFOLD`
- `CAD-WARN-SELF-INTERSECTION`
- `CAD-WARN-ZERO-THICKNESS`
- `CAD-WARN-SLIVER-FACE`
- `CAD-WARN-FILLET-FAILED`

Severity contract:

- `info`
- `warning`
- `critical`

## Receipt Contract

`run_model_validity_checks(...)` returns `CadWarningReceipt` with:

- deterministic warning ordering
- `severity_counts`
- per-warning remediation hints
- semantic refs and deep-link metadata (`cad://feature/<feature>/entity/<entity>`)

## Fixture Coverage

Fixture used for deterministic class coverage:

- `crates/cad/tests/goldens/model_validity_fixture_all_warning_classes.json`

## Verification

- `cargo test -p openagents-cad`
