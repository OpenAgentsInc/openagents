# CAD STEP Checker (Backlog 77)

This runbook documents the STEP checker utility lane for CAD export validation.

## Goal

Validate exported STEP files with machine-readable diagnostics and fail fast on:
- invalid solids
- missing shells
- non-manifold/open edges
- round-trip bbox/volume tolerance regressions

## Checker Outputs

Checker reports follow `CadStepCheckerReport` (JSON):
- `checker_version`
- `backend`
- `source`
- `passed`
- `solid_count`
- `shell_count`
- `face_count`
- `poly_loop_count`
- `non_manifold_edge_count`
- `bbox_min_mm`
- `bbox_max_mm`
- `volume_mm3`
- `diagnostics[]` with:
  - `code`
  - `severity`
  - `message`
  - `remediation_hint`
  - `count`

Stable diagnostic codes:
- `STEP_INVALID_SOLID`
- `STEP_MISSING_SHELL`
- `STEP_NON_MANIFOLD_EDGE`
- `STEP_POLY_LOOP_PARSE_FAILED`
- `STEP_POLY_LOOP_TOO_SHORT`
- `STEP_OCCT_BACKEND_UNAVAILABLE`

## Local Usage

Run fixture export + checker lane:

```bash
scripts/cad/step-checker-ci.sh
```

This script runs:
- STEP checker fixture assertions (`baseline`, `lightweight`)
- STEP round-trip tolerance fixture assertions (including near-threshold pass/fail fixtures)

Run OpenCascade backend explicitly (requires OCP/pythonocc in environment):

```bash
CAD_STEP_CHECKER_BACKEND=opencascade \
CAD_STEP_CHECKER_OCCT_PROGRAM=python3 \
CAD_STEP_CHECKER_OCCT_SCRIPT=scripts/cad/opencascade_step_checker.py \
scripts/cad/step-checker-ci.sh
```

Run checker CLI directly:

```bash
cargo run -p openagents-cad --bin step_checker -- \
  --input /path/to/model.step \
  --backend structural \
  --output /tmp/step-report.json
```

## CI Integration

`scripts/lint/strict-production-hardening-check.sh` includes a `cad-step-checker` lane that runs:

```bash
scripts/cad/step-checker-ci.sh
```

Artifacts are written to:
- `${CAD_STEP_CHECKER_ARTIFACT_DIR}` when set
- otherwise `artifacts/cad-step-checker`

If checker fails, upload that artifact directory from CI to inspect:
- `step-checker.log`
- `step-roundtrip.log`
- `summary.json`
- per-variant `*-report.json`
- per-variant `*-roundtrip-*.json`
- generated fixture `.step` files
