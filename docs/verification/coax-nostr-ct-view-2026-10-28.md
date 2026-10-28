# Compile-time view diagnostic regression

The handoff's `coax/tests/fail/ct_view.coax` returned a view into a local array.
The E0383 lifetime diagnostic correctly rejected it before the compile-time
freezer could exercise E0409. Changing the expectation would lose coverage.

The replacement returns an empty view descriptor, with no local array borrow.
The existing rule forbids freezing any view into a constant, so the first error
is again E0409. No checker, evaluator, expected code or lifetime policy changed.

Focused validation with the existing coax0 0.13.0 binary:

- `ct_view.coax` rejects with E0409 and emits no C, checked and `--release`.
- The three E0383 fixtures (local view, alias and string returns) remain refused
  in both modes: six negative cases.
- No compiler bootstrap, full suite or iOS build was needed for this fixture.

Related queue work: `coax-ct-view-diagnostic` (`#0a4f`).
