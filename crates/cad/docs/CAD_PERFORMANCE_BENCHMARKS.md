# CAD Performance Benchmark Suite

This benchmark suite provides deterministic CAD demo budget checks tied to Gate
A/B/E acceptance criteria in `crates/cad/docs/PLAN.md`.

## Scope

- Rebuild latency budget tracking
- Mesh generation latency budget tracking
- Hit-test latency budget tracking
- FPS estimate tracking
- Memory budget compliance tracking

## Implementation

- Benchmark test location:
  - `apps/autopilot-desktop/src/input/reducers/cad.rs`
  - tests:
    - `cad_performance_benchmark_suite_maps_gate_a_b_e_thresholds`
    - `cad_performance_benchmark_suite_outputs_non_empty_metrics`
- Deterministic benchmark snapshot fixture:
  - `apps/autopilot-desktop/tests/goldens/cad_performance_benchmark_snapshot.json`
- CI command:
  - `scripts/cad/perf-benchmark-ci.sh`

## Gate Mapping

Gate A:

- rebuild budget: `<= 80ms`

Gate B:

- mesh generation budget: `<= 30ms`
- hit-test budget: `<= 5ms`
- viewport FPS estimate: `>= 55`

Gate E:

- memory budget estimate: `< 800MB`

## Metric Inputs

- Rebuild latency from deterministic CAD rebuild receipts.
- Mesh/hit-test/memory/FPS metrics computed from deterministic mesh payload
  complexity and benchmark formulas.
- Canonical source script:
  - `apps/autopilot-desktop/tests/scripts/cad_demo_canonical_script.json`

## Run

```bash
scripts/cad/perf-benchmark-ci.sh
```

Regenerate snapshot intentionally:

```bash
CAD_UPDATE_GOLDENS=1 cargo test -p autopilot-desktop cad_performance_benchmark_suite_maps_gate_a_b_e_thresholds --quiet
```
