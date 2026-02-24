# 2026-02-24 Rust Codebase Refactor Audit (Wave 2)

Status: completed audit snapshot (2026-02-24)

## Scope

- Workspace root: `Cargo.toml` (52 workspace members)
- Rust surface scanned: 1,291 `.rs` files
- Production-focused filter for structural metrics:
  - include `apps/**.rs`, `crates/**.rs`
  - exclude `target/`, `tests/`, `examples/`
- Verification executed against clean `HEAD` snapshot (not dirty working tree)

## Preflight Authority Check

Mandatory baseline authorities checked before this audit pass:

- `docs/adr/INDEX.md`
- `docs/plans/rust-migration-invariant-gates.md`

## Summary

1. Several recommendations from `docs/audits/2026-02-24-rust-codebase-refactor-audit.md` are implemented (compile baseline restored, test config builder, runtime typed client extraction, partial monolith decomposition).
2. Prior recommendations are **not fully closed** at clean `HEAD`; key gaps remain: production `todo!` in DSRS, unsafe-boundary hardening incompletely applied, runtime monolith/test concentration, and low test density in selected large crates.
3. New P0 regression: `web-shell` lane fails at clean `HEAD` due missing committed modules/constants and compile errors in the wasm path.
4. Clippy lane exists and is wired, but is red at clean `HEAD` due a runtime lint error plus large warning volume.

## Prior Audit Recommendation Closure

Reference source: `docs/audits/2026-02-24-rust-codebase-refactor-audit.md`

| Prior recommendation | Status | Evidence (clean `HEAD`) | Notes |
|---|---|---|---|
| P0-1 Restore `openagents-control-service` compile baseline with shared config test fixture | Implemented | `apps/openagents.com/service/src/config.rs:846` (`Config::for_tests`), widespread call-site adoption in `apps/openagents.com/service/src/tests.rs:42`, `apps/openagents.com/service/src/domain_store.rs:3301`, `apps/openagents.com/service/src/route_split.rs:601`, `apps/openagents.com/service/src/sync_token.rs:368`, `apps/openagents.com/service/src/khala_token.rs:254`; `./scripts/local-ci.sh workspace-compile` passes | Compile drift that previously blocked workspace is resolved |
| P0-2 Enforce lint policy via clippy lane | Partially implemented | Lane exists in `scripts/local-ci.sh:374`, `scripts/local-ci.sh:562`; docs in `docs/LOCAL_CI.md:39`; `./scripts/local-ci.sh clippy-rust` runs | Lane is present but red at `HEAD` (`apps/runtime/src/server.rs:3102` absurd comparison lint) |
| P0-3 Remove production `todo!` stubs | Not implemented | `crates/dsrs/src/core/module.rs:58`, `crates/dsrs/src/core/module.rs:64`, `crates/dsrs/src/core/lm/mod.rs:356`, `crates/dsrs/src/core/lm/mod.rs:508` | Still live in production paths |
| P1-4 Break monolith files by domain boundaries | Partially implemented | `apps/openagents.com/service/src/lib.rs` reduced to 16,656 LOC (from 29,336) with modules like `apps/openagents.com/service/src/auth.rs`, `apps/openagents.com/service/src/runtime_admin.rs`, `apps/openagents.com/service/src/stats.rs`, `apps/openagents.com/service/src/web_maud.rs`; `apps/openagents.com/web-shell/src/wasm/*.rs` exists | `apps/runtime/src/server.rs` remains 10,681 LOC; decomposition incomplete |
| P1-5 Isolate `unsafe` to narrow boundaries + SAFETY contracts | Partially implemented | Unsafe remains concentrated: `crates/wgpui` 125, `crates/openagents-client-core` 59 (91.54% of total 201); SAFETY comments sparse (7 total, mostly in `crates/openagents-client-core/src/ffi.rs`) | Boundary hardening incomplete, especially in `crates/wgpui/src/platform.rs` |
| P1-6 Replace ad-hoc runtime HTTP JSON helpers with typed client crate | Implemented | `crates/openagents-runtime-client/src/lib.rs` present and integrated via `apps/openagents.com/service/src/lib.rs:32`, `apps/openagents.com/service/src/lib.rs:13252`, `apps/openagents.com/service/src/stats.rs:378` | Extraction landed and wired |
| P1-7 Reduce inline mega-test modules in production files | Partially implemented | `apps/openagents.com/service/src/tests.rs` externalized (11,248 LOC), `crates/autopilot_ui/src/tests.rs` extracted; runtime still has inline giant test block in `apps/runtime/src/server.rs:5320` | Better than prior, still high concentration |
| P2-8 Address web-shell memory growth regression and stabilize perf guardrails | Partially implemented, currently blocked by build failure | Last committed perf snapshot passes (`apps/openagents.com/web-shell/perf/latest.json`, soak RSS growth 39,424 KB under 51,200 KB budget), but lane fails now | `./scripts/local-ci.sh web-shell` fails at clean `HEAD` before perf assertions |
| P2-9 Raise targeted test depth in large low-density crates | Partially implemented | Densities remain low: `crates/openagents-cli` 0.28, `crates/autopilot_ui` 1.16, `apps/openagents.com/web-shell` 1.43, `crates/runtime` 1.27, `crates/autopilot` 1.43 test markers per kLOC | Gap still open |

## Evidence Snapshot (Wave 2)

### Production-focused metrics

- Production files: `1,147` (was `1,110`, `+37`)
- Production LOC: `500,025` (was `481,987`, `+18,038`)
- `.unwrap(`: `2,238` (was `2,211`, `+27`)
- `.expect(`: `536` (was `482`, `+54`)
- `panic!(`: `125` (was `125`, `+0`)
- `todo!(`: `4` (was `4`, `+0`)
- `unsafe`: `201` (was `201`, `+0`)
- Panic surface (`unwrap + expect + panic`): `2,899` (was `2,818`, `+81`)
- `#[allow(...)]` occurrences (workspace-wide): `236` (unchanged)

### Top production LOC concentration

1. `crates/wgpui` (`86,143` LOC)
2. `crates/nostr/core` (`60,265` LOC)
3. `crates/autopilot` (`51,695` LOC)
4. `apps/openagents.com/service` (`50,116` LOC)
5. `apps/runtime` (`38,006` LOC)
6. `crates/dsrs` (`27,094` LOC)

### Panic-surface concentration (production)

Top 10 crates account for `82.27%` of total panic surface (`2,899`):

1. `crates/nostr/core` (`1,013`)
2. `crates/nostr/client` (`229`)
3. `crates/wgpui` (`223`)
4. `crates/dsrs` (`209`)
5. `apps/openagents.com/service` (`182`)
6. `crates/issues` (`175`)
7. `crates/rlm` (`117`)
8. `crates/pylon` (`86`)
9. `crates/compute` (`82`)
10. `crates/autopilot` (`69`)

### Unsafe concentration (production)

- Total `unsafe`: `201`
- Top two crates are still `91.54%` of all unsafe:
  - `crates/wgpui` (`125`)
  - `crates/openagents-client-core` (`59`)

### Largest files (current)

1. `apps/openagents.com/service/src/lib.rs` (`16,656` LOC)
2. `apps/openagents.com/service/src/tests.rs` (`11,248` LOC)
3. `apps/runtime/src/server.rs` (`10,681` LOC)
4. `crates/autopilot_ui/src/lib.rs` (`8,151` LOC)
5. `apps/autopilot-desktop/src/main.rs` (`7,495` LOC)
6. `apps/openagents.com/web-shell/src/lib.rs` (`5,977` LOC)
7. `crates/wgpui/src/platform.rs` (`5,953` LOC)

## Verification Runs (clean `HEAD`)

1. `./scripts/local-ci.sh workspace-compile`
- Result: pass
- Note: `openagents-control-service` emits dead-code warnings in Gmail response models (`apps/openagents.com/service/src/lib.rs:872`, `apps/openagents.com/service/src/lib.rs:893`)

2. `./scripts/local-ci.sh web-shell`
- Result: fail
- Blocking failures include missing wasm modules declared by `apps/openagents.com/web-shell/src/lib.rs:6` and `apps/openagents.com/web-shell/src/lib.rs:8` (`wasm_constants`, `wasm_state` not present at clean `HEAD`), plus unresolved wasm symbols in `apps/openagents.com/web-shell/src/wasm/dom.rs:12`

3. `./scripts/local-ci.sh clippy-rust`
- Result: fail
- Blocking lint error at `apps/runtime/src/server.rs:3102` with `const PRICE_INTEGRITY_TOLERANCE_MSATS: u64 = 0` defined at `apps/runtime/src/server.rs:4263`
- Additional non-blocking clippy warning volume remains high in `apps/runtime`

4. `cargo test -p openagents-client-core`
- Result: pass (`82` tests)

5. `cargo test -p openagents-runtime-service`
- Result: pass (`132` tests in primary suite)

## Wave-2 Findings and Recommendations

## P0 (Immediate)

### 1) Restore clean `web-shell` lane at `HEAD`

Problem:

- The required wasm modules are declared but absent at committed `HEAD`, and wasm compile fails before perf checks.

Evidence:

- `apps/openagents.com/web-shell/src/lib.rs:6`
- `apps/openagents.com/web-shell/src/lib.rs:8`
- `apps/openagents.com/web-shell/src/wasm/dom.rs:12`

Recommendation:

1. Commit the missing wasm module files and constant/state split as an atomic changeset.
2. Ensure `./scripts/local-ci.sh web-shell` is green from clean checkout before merge.
3. Add a pre-merge gate that requires this lane on all changes touching `apps/openagents.com/web-shell/`.

### 2) Fix clippy blocking error in runtime and hold red-line on new warnings

Problem:

- Clippy lane exists but is not passing at clean `HEAD`.

Evidence:

- `apps/runtime/src/server.rs:3102`
- `apps/runtime/src/server.rs:4263`

Recommendation:

1. Replace the current absurd-extreme comparison pattern with explicit equality/inequality semantics for zero tolerance.
2. Keep clippy lane required for runtime/control/web-shell critical crates.
3. Burn down repeated clippy classes in `apps/runtime` (`manual_clamp`, `collapsible_if`, `too_many_arguments`, redundant closures) with focused follow-up PRs.

### 3) Eliminate remaining production `todo!` stubs in DSRS

Problem:

- `todo!` remains in production trait/messaging paths.

Evidence:

- `crates/dsrs/src/core/module.rs:58`
- `crates/dsrs/src/core/module.rs:64`
- `crates/dsrs/src/core/lm/mod.rs:356`
- `crates/dsrs/src/core/lm/mod.rs:508`

Recommendation:

1. Replace each with typed errors and explicit unsupported-capability codes.
2. Add tests that assert non-panicking error behavior for image/unsupported branches.

## P1 (High-Impact)

### 4) Complete runtime monolith decomposition

Problem:

- `apps/runtime/src/server.rs` remains 10,681 LOC and still embeds large test modules.

Recommendation:

1. Split transport handlers, orchestration/settlement logic, and provider integrity/quarantine into separate modules.
2. Move integration-like tests from inline `mod tests` into crate `tests/` suites with shared fixtures.

### 5) Finish unsafe-boundary isolation in WGPUI and FFI

Problem:

- Unsafe concentration remains unchanged and SAFETY contracts are sparse.

Recommendation:

1. Create explicit `raw` boundary modules around unsafe graphics/FFI calls.
2. Require `SAFETY:` comments on every unsafe block/function.
3. Add negative-path tests for pointer validity and ownership transfer in FFI surfaces.

### 6) Arrest panic-surface growth in high-volume crates

Problem:

- Panic surface increased (`+81`) relative to prior audit.

Recommendation:

1. Enforce a no-net-growth panic policy for production paths.
2. Start reduction in top offenders: `crates/nostr/core`, `crates/nostr/client`, `crates/dsrs`, `apps/openagents.com/service`, `crates/issues`.

## P2 (Hardening)

### 7) Improve test depth in large low-density crates

Priority targets (test markers per kLOC):

- `crates/openagents-cli` (`0.28`)
- `crates/autopilot_ui` (`1.16`)
- `crates/runtime` (`1.27`)
- `apps/openagents.com/web-shell` (`1.43`)
- `crates/autopilot` (`1.43`)

Recommendation:

1. Add deterministic scenario/reducer tests on panic-hot or high-churn modules first.
2. Require each major refactor PR in these crates to include net-new focused tests.

### 8) Trim `#[allow(...)]` debt in large crates

Problem:

- Workspace-wide `#[allow(...)]` count remains 236; concentration is highest in `crates/autopilot` and `crates/nostr/core`.

Recommendation:

1. Classify allows by category (`dead_code`, `clippy::*`, `unused*`).
2. Remove broad allows in production paths and replace with targeted, justified local exceptions.

## Proposed Execution Order (Wave 2)

1. P0-1: restore clean web-shell lane at `HEAD`.
2. P0-2: resolve runtime clippy blocker and bring clippy lane green.
3. P0-3: remove DSRS production `todo!` stubs.
4. P1-4/P1-5: runtime split and unsafe boundary isolation.
5. P1-6 + P2-7/P2-8: panic-surface reduction, test-density uplift, allow-attribute debt reduction.
