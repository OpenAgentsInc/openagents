# Checker / Import Results (Spike Snapshot)

## Summary

Import/checker validation could not run because STEP outputs were unavailable in this spike.

## Status Table

| Check | Status | Evidence |
|---|---|---|
| VCAD kernel bootstrap | Failed | `artifacts/vcad_kernel_check.log` |
| VCAD CLI bootstrap | Failed | `artifacts/vcad_cli_help.log` |
| OpenCascade compile bootstrap | Failed | `artifacts/opencascade_check.log` |
| STEP checker run | Not executed | Depends on STEP artifacts |

## Follow-up checker contract

When STEP export is available, checker pass must include:

- non-fatal topology import,
- bounding box delta within tolerance,
- volume delta within tolerance,
- deterministic output hash stability across repeated exports.
