# Stub Exceptions (d-012)

This repository enforces a no-stubs policy. Stub patterns are forbidden in production code and tests:

- `todo!()`
- `unimplemented!()`
- `panic!("not implemented")` (or variants)

## Allowed Exceptions

None.

## Notes

- Documentation and fixtures are excluded from stub scanning because they are not compiled into production binaries.
- If an exception becomes necessary, document the exact file path and justification here and update `scripts/check-stubs.sh`.
