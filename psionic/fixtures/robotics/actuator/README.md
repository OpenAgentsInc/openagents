# Illustrative scalar servo reference

The independent `psionic/tests/robotics/actuator_oracle.py` uses Python's
`Fraction` without importing Coax/product code. It fixes three synthetic
inputs and exact discrete-state equations. `reference.json` preserves every
fraction, declared numerical tolerance and column order. The generated
`actuator-golden.coax` is compared byte for byte on every oracle run.

The oracle's `--write` creates missing files only. It refuses a changed
existing reference. These are authored numerical examples, not measured
Microduck trajectories or upstream simulator captures.

`example.out` is the exact transcript checked in debug/release. The
[contract](../../../docs/robotics/actuator-contract-rb02.md) and
[receipt](../../../../docs/verification/robotics-actuator-rb02-2026-11-15.md)
state the full scope, commands and observed results.
