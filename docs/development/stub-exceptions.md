# Stub Exceptions (d-012)

This repository enforces a no-stubs policy. Stub patterns are forbidden in production code and tests:

- `todo!()`
- `unimplemented!()`
- `panic!("not implemented")` (or variants)

## Allowed Exceptions

### External Code (dsrs)

The `crates/dsrs/` and `crates/dsrs-macros/` directories are excluded from stub scanning because they are external code (DSPy Rust implementation) that we've integrated wholesale. These contain `todo!()` patterns for unimplemented features that don't affect our core functionality.

## Notes

- Documentation and fixtures are excluded from stub scanning because they are not compiled into production binaries.
- If an exception becomes necessary, document the exact file path and justification here and update `scripts/check-stubs.sh`.
