# 2026-03-01 CAD Implementation Code-Health Audit

## Scope

Audited surface:

- `crates/cad/src/*`
- `crates/cad/tests/*`
- CAD-facing contracts and validation behavior used by `apps/autopilot-desktop`

Primary objective:

- Evaluate code health, maintainability, test/lint hygiene, and short-term hardening opportunities for the current CAD implementation.

## Method

Baseline references:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`

Commands run:

- `cargo check -p openagents-cad --quiet` (pass)
- `cargo test -p openagents-cad --quiet` (pass: 170 unit tests + integration lanes)
- `cargo clippy -p openagents-cad --all-targets` (fail)
- `cargo clippy -p openagents-cad --all-targets -- -D warnings` (fail)
- `scripts/lint/ownership-boundary-check.sh` (pass)
- `cargo fmt --all --check` (fail; repo-wide formatting deltas outside CAD scope)
- structural scans:
  - module size / concentration (`wc -l crates/cad/src/*.rs`)
  - panic/expect usage (`rg panic!|expect\\(`)
  - duplication scan (`rg fnv1a64`)

## Executive Summary

The CAD domain implementation is functionally strong and deterministic-focused, with broad test coverage and clean crate boundaries. However, code-health risk is accumulating in three places:

1. lint hygiene (strict clippy not currently passable),
2. module concentration (several very large source files),
3. repeated low-level utilities / stringly-typed contracts.

No immediate architecture-boundary drift was found. Reliability signals are good at runtime tests, but maintainability and review cost will rise quickly without refactors.

## Current Health Snapshot

### What is healthy

- Crate boundary discipline is good:
  - no app/wgpui dependency leakage detected from `crates/cad`.
  - `scripts/lint/ownership-boundary-check.sh` passes.
- Runtime behavior is stable under current tests:
  - `cargo test -p openagents-cad --quiet` passes (170 tests).
- Dependency surface is intentionally small:
  - only `serde`, `serde_json`, and `thiserror` (plus proc-macro transitive deps).
- Determinism patterns are consistently applied:
  - canonical maps (`BTreeMap`), deterministic hashing, snapshot fixtures, stable ID persistence.

### Concentration metrics

Top large modules (LOC):

- `crates/cad/src/sketch.rs` (1630)
- `crates/cad/src/features.rs` (1453)
- `crates/cad/src/rack.rs` (965)
- `crates/cad/src/analysis.rs` (803)
- `crates/cad/src/sketch_feature_ops.rs` (798)
- `crates/cad/src/finishing_ops.rs` (771)
- `crates/cad/src/eval.rs` (738)

Crate-wide source size:

- `crates/cad/src/*` total: 15,783 LOC.

Test placement:

- 34 source files include inline `#[cfg(test)]` modules.
- 16 integration/golden files exist under `crates/cad/tests`.

## Findings (Ranked)

## 1) Strict clippy lane is currently non-passable for CAD

Severity: High  
Impact: blocks enforceable lint quality gates for future changes.

Evidence:

- `cargo clippy -p openagents-cad --all-targets` exits non-zero.
- lint totals from latest run:
  - warnings: 15
  - errors: 312
- dominant lint classes:
  - `expect_used`: 296
  - `panic`: 14
  - `assigning_clones`: 4
  - `too_many_arguments`: 2
  - additional style/derivable nits.

Representative references:

- argument-heavy constructors:
  - `crates/cad/src/events.rs:68`
  - `crates/cad/src/finishing_ops.rs:392`
- copy inefficiency:
  - `crates/cad/src/dispatch.rs:94`
- sort primitive style:
  - `crates/cad/src/eval.rs:183`

Notes:

- Most `expect/panic` violations are inside test modules, but because linting runs with `--all-targets`, they still fail the lane.

Improvement actions:

1. Define explicit clippy policy for test code:
   - allow `clippy::expect_used` and `clippy::panic` under `#[cfg(test)]` modules only, or
   - migrate high-noise inline tests into integration tests with test-specific lint allowances.
2. Fix non-test lint findings first (e.g., `assigning_clones`, `too_many_arguments` wrappers).
3. Add `scripts/lint/cad-clippy.sh` with scoped policy and include it in CAD CI lane.

## 2) Monolithic modules are increasing maintenance and review cost

Severity: High  
Impact: slower onboarding, higher merge conflict frequency, harder targeted testing.

Evidence:

- `sketch.rs` and `features.rs` together exceed 3K LOC.
- multiple domain concerns are mixed in single files:
  - domain model + solve logic + serialization + tests in the same unit.

Improvement actions:

1. Split by concern with stable submodules:
   - `sketch/` => `model.rs`, `constraints.rs`, `solver.rs`, `serde.rs`, `tests.rs`
   - `features/` => primitive ops vs transforms vs pattern ops vs placeholder/legacy ops
2. Move most inline tests into `crates/cad/tests/` per feature area.
3. Keep each source module under ~400 LOC target unless heavily justified.

## 3) Repeated hash utility implementation across modules

Severity: Medium  
Impact: unnecessary duplication and risk of drift in deterministic identity logic.

Evidence:

- local `fnv1a64` appears in multiple modules:
  - `crates/cad/src/features.rs:755`
  - `crates/cad/src/eval.rs:364`
  - `crates/cad/src/export.rs:291`
  - `crates/cad/src/events.rs:135`
  - `crates/cad/src/mesh.rs:338`
  - `crates/cad/src/selection.rs:314`
  - `crates/cad/src/sketch_feature_ops.rs:473`
  - `crates/cad/src/finishing_ops.rs:562`
  - `crates/cad/src/step_import.rs:106`

Improvement actions:

1. Introduce shared utility module:
   - `crates/cad/src/hash.rs` with `stable_fnv1a64`, `stable_hex_digest`.
2. Migrate all modules to centralized implementation.
3. Add one deterministic cross-module hash fixture test.

## 4) Production-path `expect` calls should be removed

Severity: Medium  
Impact: potential panic behavior in edge-paths, weaker explicit failure contract.

Evidence:

- `crates/cad/src/history.rs:148`
  - `expect("coalesce precondition requires prior entry")`
- `crates/cad/src/sketch_feature_ops.rs:237`
  - `expect("cut validate requires source feature")`

Improvement actions:

1. Replace with explicit `CadError` branches.
2. If logically unreachable, use `debug_assert!` + safe fallback error return.
3. Add targeted tests for those defensive branches.

## 5) Stringly-typed metadata/parameter payloads are pervasive

Severity: Medium  
Impact: typo risk, schema drift, and runtime-only validation failures.

Evidence:

- broad usage of `BTreeMap<String, String>` for model and feature metadata:
  - `crates/cad/src/document.rs:24`
  - `crates/cad/src/feature_graph.rs:17`
  - `crates/cad/src/format.rs:28`
  - `crates/cad/src/step_import.rs:66`
  - `crates/cad/src/history.rs:13`

Improvement actions:

1. Introduce typed constants for all canonical metadata keys.
2. Wrap high-value key groups in typed structs (e.g., import metadata, feature params subsets).
3. Enforce parsing/validation at boundary entry points only.

## 6) Formatting gate currently fails at repo level

Severity: Low  
Impact: noisy diffs and weak style enforcement.

Evidence:

- `cargo fmt --all --check` reports broad diffs across repo, including CAD and non-CAD files.

Improvement actions:

1. Run one-time formatting baseline commit across workspace.
2. Keep `fmt --check` required in CI after baseline lands.

## 7) Test placement strategy inflates source files and lint noise

Severity: Low  
Impact: larger compile units and lower signal-to-noise in production modules.

Evidence:

- 34 source files include inline test modules (`#[cfg(test)]`).

Improvement actions:

1. Move scenario/golden tests to `crates/cad/tests/*`.
2. Keep only micro unit tests inline where it improves local readability.

## Sequential Recommendations (Execution Order)

1. Create a CAD-specific lint gate and freeze policy.
   - Add `scripts/lint/cad-clippy.sh`.
   - Define policy in script comments: production code must be `-D warnings`, tests may allow `expect_used`/`panic` until migrated.
   - Done criteria:
     - script exists and is documented.
     - `scripts/lint/cad-clippy.sh` runs in CI or strict local lane.

2. Fix all non-test clippy findings first.
   - Start with currently flagged production code:
     - `crates/cad/src/dispatch.rs:94` (`clone_from` improvement)
     - `crates/cad/src/eval.rs:183` (`sort_unstable`)
     - `crates/cad/src/events.rs:68` (reduce argument count)
     - `crates/cad/src/finishing_ops.rs:392` (reduce argument count)
   - Done criteria:
     - no non-test clippy errors in `crates/cad/src`.

3. Remove production-path panic/expect usage.
   - Replace with explicit error paths in:
     - `crates/cad/src/history.rs:148`
     - `crates/cad/src/sketch_feature_ops.rs:237`
   - Done criteria:
     - no `panic!`/`expect*` in production paths under `crates/cad/src` (test modules excluded).

4. Centralize deterministic hashing utilities.
   - Add `crates/cad/src/hash.rs` with canonical `stable_fnv1a64` and `stable_hex_digest`.
   - Replace local hash implementations in:
     - `features.rs`, `eval.rs`, `export.rs`, `events.rs`, `mesh.rs`, `selection.rs`, `sketch_feature_ops.rs`, `finishing_ops.rs`, `step_import.rs`.
   - Done criteria:
     - only one FNV implementation remains in crate.
     - determinism tests continue to pass.

5. Add typed key constants for metadata and parameter map keys.
   - Introduce key modules/constants for:
     - import keys (`import.format`, `import.hash`, counts)
     - stable feature metadata keys
     - warning metadata keys.
   - Done criteria:
     - hardcoded repeated key strings are replaced by constants.

6. Add typed wrappers for highest-risk stringly payloads.
   - Begin with:
     - STEP import metadata wrapper around `CadDocument.metadata`
     - feature parameter wrapper for finishing ops and sketch feature conversion.
   - Done criteria:
     - compile-time typed APIs exist for these two surfaces.
     - boundary parsing remains explicit and validated.

7. Split `sketch.rs` into focused submodules.
   - Target module layout:
     - `sketch/model.rs`
     - `sketch/constraints.rs`
     - `sketch/solver.rs`
     - `sketch/serde.rs`
     - `sketch/tests.rs` (or integration tests where appropriate)
   - Done criteria:
     - original `sketch.rs` reduced to module wiring plus public exports.
     - solver behavior unchanged (snapshot tests green).

8. Split `features.rs` into focused submodules.
   - Target module layout:
     - `features/primitives.rs`
     - `features/transform.rs`
     - `features/pattern.rs`
     - `features/placeholder.rs`
   - Done criteria:
     - API compatibility preserved at `crate::features`.
     - golden feature hash fixtures still pass.

9. Move high-noise inline tests to integration/golden tests.
   - Prioritize large modules first:
     - sketch, features, rack, eval, tessellation.
   - Keep inline only tight unit tests with high locality.
   - Done criteria:
     - inline test footprint reduced materially.
     - clippy noise from test modules reduced.

10. Establish formatting baseline in one dedicated cleanup commit.
   - Run `cargo fmt --all` once intentionally.
   - Avoid mixing with behavior changes.
   - Done criteria:
     - `cargo fmt --all --check` passes at repo level.

11. Add CAD code-health runbook and lane expectations.
   - Create `docs/cad/CAD_CODE_HEALTH.md`:
     - lint commands
     - formatting commands
     - test lanes
     - policy for `expect/panic` usage.
   - Done criteria:
     - doc linked from `docs/cad/PLAN.md`.

12. Enforce health gates in ongoing workflow.
   - Add CAD health checks to strict lint/release scripts after baseline:
     - `scripts/lint/strict-production-hardening-check.sh`
   - Done criteria:
     - regressions fail fast in CI.
     - no recurrence of current lint debt profile.

## Concrete Work Queue (Issue-Ready)

1. `cad-lint`: Add `scripts/lint/cad-clippy.sh` and policy.
2. `cad-lint`: Fix non-test clippy findings (`dispatch`, `eval`, `events`, `finishing_ops`).
3. `cad-hardening`: Remove production `expect/panic` in history/sketch-feature paths.
4. `cad-core`: Add shared `hash.rs` and migrate all FNV usage.
5. `cad-core`: Add metadata key constants module and replace repeated string keys.
6. `cad-core`: Add typed STEP import metadata wrapper.
7. `cad-refactor`: Split `sketch.rs` into submodules.
8. `cad-refactor`: Split `features.rs` into submodules.
9. `cad-tests`: Move high-noise inline tests into `crates/cad/tests`.
10. `repo-hygiene`: One-time `cargo fmt --all` baseline commit.
11. `cad-docs`: Add `CAD_CODE_HEALTH.md` and link from plan docs.
12. `cad-ci`: Wire CAD health lanes into strict production hardening checks.

## Final Assessment

The CAD system is already strong on deterministic behavior, test coverage, and crate-boundary discipline. The next quality inflection point is not capability; it is maintainability hardening. If the lint policy, module decomposition, and utility dedup work are completed early in Wave 2, the codebase should remain fast to iterate without sacrificing rigor.
