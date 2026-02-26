# Rust Refactor Audit (WGPUI + Desktop)

Date: 2026-02-26  
Scope: Entire current Rust codebase in this pruned repo (`apps/autopilot-desktop`, `crates/wgpui`)  
Method: Static scan + lint/test baselines (`rg`, LOC/scripts, `cargo metadata`, `cargo clippy --workspace --all-targets`, `cargo test --workspace --all-targets`)

## Executive Summary

The repo is healthy and build-stable, but structurally over-expanded for current MVP scope. The biggest risks are:

1. Monolithic modules that make iteration/error isolation expensive.
2. Very large API surface with weak lane boundaries (runtime vs demo vs test tooling).
3. Dependency and feature footprint larger than required for current desktop MVP.
4. Lint policy drift (workspace policy exists, but strictness is not uniformly enforced).
5. Documentation drift in `wgpui` docs/README vs actual file structure and product scope.

## Snapshot (Current State)

- Rust files: `284`
- Rust LOC: `127,966`
- `wgpui` LOC (src+examples+benches): `103,410`
- `autopilot-desktop` LOC: `814`
- Workspace members: `2`
- Reachable packages via `cargo metadata`: `500` (`498` third-party + `2` workspace)
- Test baseline: `1000 passed`, `0 failed` (`cargo test --workspace --all-targets`)
- Clippy baseline: `97 warnings` total (`8` lib, `32` lib-test, `57` examples)

Largest hotspots by file size:

- `crates/wgpui/src/platform.rs` (`~6,016` LOC, `142` fns)
- `crates/wgpui/src/components/live_editor/mod.rs` (`~3,129` LOC, `119` fns)
- `apps/autopilot-desktop/src/main.rs` (`~814` LOC, pane/hotbar/render/input all in one file)

Largest hotspots by subsystem:

- `crates/wgpui/src/components/*` (`~49,316` LOC)
- `crates/wgpui/examples/storybook/*` (`~11,425` LOC)
- `crates/wgpui/src/testing/*` (`~5,536` LOC)

## Key Findings

### 1) Module Monolith Risk

`platform.rs`, `live_editor/mod.rs`, and desktop `main.rs` are doing too many jobs each (platform bootstrap + render loop + IO + state + interaction logic). This increases bug coupling and makes targeted testing harder.

### 2) API Surface Is Too Broad for Current MVP

`wgpui` currently exports a very large top-level surface (`~3702 pub items` across `src`). This is convenient short-term but expensive for maintenance and semver discipline.

### 3) Framework Lanes Exist but Are Not Consumed

`wgpui` includes `app/`, `element/`, and `window/` abstractions, but current desktop app/examples mostly bypass them with direct winit/wgpu loops. Keeping both paradigms long-term adds cognitive and maintenance overhead.

### 4) Feature/Dependency Footprint Is Larger Than Active Usage

`wgpui` defaults to `web`, while the active product lane here is desktop MVP. Optional lanes (`audio`, test framework, large demo surfaces) are mixed into normal development flow and dependency graph more than necessary.

### 5) Lint Policy Enforcement Gap

Workspace-level strict clippy policy exists, but strict rules (panic/unwrap/expect/no stdout) are not uniformly enforced in all crates/targets. This causes policy drift and allows debt to accumulate in examples/tests.

### 6) Documentation Drift

`crates/wgpui/README.md` and some docs still describe older broader architecture assumptions and stale file references. This is now misleading in the pruned MVP repo.

## Recommended Refactor Series

## Phase 0 (Immediate, Low Risk)

1. Split desktop app entrypoint by concern.
   - Target: `apps/autopilot-desktop/src/main.rs`
   - Extract to modules: `app_state.rs`, `input.rs`, `pane_system.rs`, `render.rs`, `hotbar.rs`.
   - Outcome: easier iteration on pane UX without touching GPU bootstrap each change.

2. Create clippy debt baseline file and gate regressions.
   - Keep existing warnings as baseline, fail CI on net-new warnings.
   - Include separate lanes: `lib`, `lib tests`, `examples`.

3. Refresh doc authority in `wgpui`.
   - Update `crates/wgpui/README.md` + `crates/wgpui/docs/*.md` to MVP-relevant scope and current file paths.

## Phase 1 (High Impact)

1. Decompose `platform.rs` into lane modules.
   - Suggested structure:
     - `platform/mod.rs` (trait + shared config)
     - `platform/web.rs`
     - `platform/desktop.rs`
     - `platform/ios.rs`
   - Keep compile-time `cfg` in small wrappers instead of one 6k file.

2. Decompose `live_editor`.
   - Target: `crates/wgpui/src/components/live_editor/mod.rs`
   - Extract:
     - `state.rs`
     - `editing.rs`
     - `vim.rs`
     - `layout_paint.rs`
     - `input_handlers.rs`
   - Add focused tests per submodule.

3. Narrow `wgpui` public API.
   - Introduce stable prelude(s): `prelude::core`, `prelude::desktop`.
   - Stop re-exporting large niche surfaces at crate root by default.
   - Keep explicit paths for advanced/experimental modules.

## Phase 2 (Scope & Build Efficiency)

1. Feature-gate heavyweight lanes.
   - `storybook`, advanced demo surfaces, and large testing DSL should compile only when explicitly requested.
   - Keep default developer loop closer to product app requirements.

2. Dependency gating cleanup.
   - Make optional dependencies truly optional by feature (for example audio-only/network-only branches).
   - Re-evaluate default `wgpui` feature set for MVP desktop-first development.

3. Evaluate framework lane (`app/element/window`) decision.
   - Either:
     - Adopt it in `autopilot-desktop` (single canonical UI model), or
     - Move it behind `experimental-framework` feature and reduce default surface.

## Phase 3 (Quality & Safety Hardening)

1. Adopt `lints.workspace = true` consistently where appropriate and resolve violations lane-by-lane.
2. Reduce `allow/expect(clippy::too_many_arguments)` by introducing config structs/builders.
3. Remove debug `eprintln!` paths or gate them behind explicit debug features.
4. Add perf guardrails:
   - compile-time tracking (`cargo check` wall-clock budget),
   - draw-call/frame instrumentation in renderer hot paths,
   - text/scene microbench baselines.

## Suggested Backlog Issues (Ready to File)

1. Split `autopilot-desktop/src/main.rs` into input/render/state/panes modules.
2. Break `wgpui/src/platform.rs` into platform-specific modules.
3. Break `wgpui/src/components/live_editor/mod.rs` into focused submodules.
4. Introduce `wgpui` prelude strategy and reduce root re-export surface.
5. Feature-gate storybook + testing DSL to non-default development lanes.
6. Align `wgpui` default feature strategy with desktop MVP workflow.
7. Add clippy warning baseline and “no net-new warnings” gate.
8. Refresh `wgpui` docs/README to current MVP authority and paths.
9. Decide fate of `app/element/window` framework lane (adopt vs isolate).
10. Convert remaining large argument-heavy render helpers to config-struct APIs.

## Exit Criteria

Refactor series is complete when all are true:

- No single source file exceeds ~2k LOC in core runtime paths.
- Desktop app main loop is split into testable modules.
- `wgpui` root exports are intentionally minimal and documented.
- Default build path matches MVP desktop needs.
- Clippy warning count trends down with enforced regression gate.
- `wgpui` docs accurately reflect the code and current repo scope.

