# Performance Guardrails (Issue #2314)

Date: 2026-02-26  
Scope: `crates/wgpui`, `scripts/perf`

## Summary

Added explicit performance guardrails for compile-time and render/text hot paths:

- Compile-time baseline + budget checks.
- Renderer instrumentation for draw-call and CPU timing counters.
- Microbench baseline + budget checks for scene and text operations.

## Changes

- Added renderer instrumentation in `crates/wgpui/src/renderer.rs`:
  - `RenderMetrics` (layer/instance counts, draw calls, prepare/render CPU ms)
  - `Renderer::render_metrics()` getter for regression analysis
- Added Criterion microbench suite:
  - `crates/wgpui/benches/text_scene_microbench.rs`
  - Registered in `crates/wgpui/Cargo.toml`
- Added compile budget scripts:
  - `scripts/perf/compile-baseline.sh`
  - `scripts/perf/compile-budget-check.sh`
- Added microbench budget scripts:
  - `scripts/perf/microbench-baseline.sh`
  - `scripts/perf/microbench-check.sh`
- Added wrapper:
  - `scripts/perf/perf-check.sh`
- Committed baseline artifacts:
  - `scripts/perf/compile-baseline.toml`
  - `scripts/perf/microbench-baseline.toml`

## Baselines

Compile baseline (`scripts/perf/compile-baseline.toml`):

- `BUDGET_FACTOR_PERCENT=125`
- `WGPUI_CHECK_SECONDS=2`
- `DESKTOP_CHECK_SECONDS=1`
- `WGPUI_SMOKE_TEST_SECONDS=4`

Microbench baseline (`scripts/perf/microbench-baseline.toml`):

- `MICRO_BUDGET_FACTOR_PERCENT=130`
- `SCENE_BUILD_1000_QUADS_NS=5096`
- `TEXT_MEASURE_1KB_NS=439510`
- `TEXT_LAYOUT_1KB_NS=471834`

## Verification

- `cargo check -p wgpui`
- `cargo check -p autopilot-desktop`
- `cargo check -p wgpui --benches`
- `scripts/lint/clippy-regression-check.sh`
- `scripts/perf/compile-budget-check.sh`
- `scripts/perf/microbench-check.sh`
- `scripts/perf/perf-check.sh`
