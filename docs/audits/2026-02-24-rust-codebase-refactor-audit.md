# 2026-02-24 Rust Codebase Audit and Refactor Recommendations

Status: completed audit snapshot (2026-02-24)

## Scope

- Workspace root: `Cargo.toml` (52 workspace members)
- Rust surface scanned: 1,261 `.rs` files
- Static metrics generated for all Rust crates/apps
- Build/test/perf lanes sampled on critical services and shared crates

## Preflight Authority Check

Mandatory baseline authorities read before audit work:

- `docs/adr/INDEX.md`
- `docs/plans/rust-migration-invariant-gates.md`

Additional architecture/ownership references used to constrain recommendations:

- `docs/core/ARCHITECTURE.md`
- `docs/core/ARCHITECTURE.md`
- `docs/core/PROJECT_OVERVIEW.md`
- `docs/core/ROADMAP.md`
- `docs/core/LOCAL_CI.md`
- `docs/adr/ADR-0001-rust-only-architecture-baseline.md`
- `docs/adr/ADR-0002-proto-first-contract-governance.md`
- `docs/adr/ADR-0003-khala-ws-only-replay-transport.md`
- `docs/adr/ADR-0008-bounded-vercel-sse-compatibility-lane.md`
- `apps/autopilot-ios/docs/wgpui-codex-ownership-boundaries.md`
- `apps/autopilot-ios/docs/codex-wgpui-parity-gates.md`
- `apps/autopilot-ios/docs/rust-client-core-integration.md`

## Executive Summary

1. The Rust workspace is large and active (52 members, ~482k production LOC), and most major crates compile cleanly.
2. `openagents-control-service` currently blocks workspace compile due a test-fixture drift (`Config` gained `liquidity_stats_pool_ids`, multiple test constructors were not updated).
3. Refactor priority should target monolithic file concentration, panic-surface reduction in production paths, and isolation of `unsafe` FFI/rendering boundaries.
4. Existing lint intent in `Cargo.toml` is strict, but enforcement is incomplete because local CI does not run a workspace clippy lane.
5. Web-shell guardrails passed functionally, but perf signoff failed memory growth budget (`rss_growth_kb`) in latest run.

## Evidence Snapshot

### Workspace-level metrics (production-focused filter)

- Production files: `1110`
- Production LOC: `481,987`
- `.unwrap(`: `2,211`
- `.expect(`: `482`
- `panic!(`: `125`
- `todo!(`: `4`
- `unsafe`: `201`

### Top crate concentration by production LOC

1. `crates/wgpui` (`85,869` LOC)
2. `crates/nostr/core` (`60,048` LOC)
3. `crates/autopilot` (`51,637` LOC)
4. `apps/openagents.com/service` (`48,300` LOC)
5. `apps/runtime` (`27,200` LOC)
6. `crates/dsrs` (`27,094` LOC)

### Panic-surface concentration (production)

Top 10 crates account for `82.15%` of panic surface (`unwrap + expect + panic`):

1. `crates/nostr/core` (`1004`)
2. `crates/wgpui` (`223`)
3. `crates/dsrs` (`209`)
4. `crates/nostr/client` (`203`)
5. `apps/openagents.com/service` (`166`)
6. `crates/issues` (`156`)
7. `crates/rlm` (`117`)
8. `crates/pylon` (`86`)
9. `crates/compute` (`82`)
10. `crates/autopilot` (`69`)

### Unsafe concentration (production)

`91.54%` of `unsafe` occurrences are concentrated in:

1. `crates/wgpui` (`125`)
2. `crates/openagents-client-core` (`59`)

## Verification Runs

### Build and checks executed

1. `./scripts/local-ci.sh workspace-compile`
- Result: **failed**
- Blocking errors: missing `liquidity_stats_pool_ids` in `Config` test constructors in:
  - `apps/openagents.com/service/src/domain_store.rs:2902`
  - `apps/openagents.com/service/src/khala_token.rs:254`
  - `apps/openagents.com/service/src/route_split.rs:602`
  - `apps/openagents.com/service/src/sync_token.rs:369`
  - `apps/openagents.com/service/src/lib.rs:18286`

2. `cargo check -p openagents-control-service --all-targets`
- Result: **failed** (same root cause)

3. `cargo check -p openagents-runtime-service --all-targets`
- Result: passed

4. `cargo check -p openagents-web-shell --all-targets`
- Result: passed

5. `cargo check -p openagents-client-core --all-targets`
- Result: passed

6. `cargo check -p wgpui --all-targets`
- Result: passed

7. `cargo check -p autopilot-desktop --all-targets`
- Result: passed

8. `cargo test -p openagents-client-core`
- Result: passed (`82` tests)

9. `cargo test -p openagents-runtime-service`
- Result: passed (`129` tests in primary unit suite)

10. `./scripts/local-ci.sh web-shell`
- Result: **failed perf budget**
- Failing budget: `rss_growth_kb > 51200`
- Evidence: observed in local-ci run output during this audit (`rss_growth_kb` growth reported as `113,696` KB)

## Key Findings and Refactor Recommendations

## P0 (Immediate)

### 1. Restore `openagents-control-service` compile baseline

Evidence:

- Workspace compile is red due test fixture drift in multiple files (above).
- `Config` has a large field set and many duplicated hand-written test constructors.

Recommendation:

1. Add a single test fixture builder (`Config::for_tests()` or `TestConfigBuilder`) in `apps/openagents.com/service/src/config.rs`.
2. Replace per-module raw `Config { ... }` constructors with the shared builder.
3. Keep minimal per-test overrides local (only changed fields).

Expected impact:

- Eliminates recurring compile breaks when config fields evolve.
- Reduces test maintenance churn and cross-file drift.

### 2. Enforce lint policy that already exists in workspace config

Evidence:

- Workspace lint config denies `unwrap_used`, `expect_used`, `panic`, `todo`, and `allow_attributes` (`Cargo.toml`), but `scripts/local-ci.sh` has no clippy lane.
- Production code still contains high panic surface (`2,818` occurrences) and many `#[allow(...)]` attributes (`236` occurrences workspace-wide).

Recommendation:

1. Add a phased `cargo clippy` lane in `scripts/local-ci.sh` (start with critical crates: control-service, runtime-service, client-core, web-shell, wgpui).
2. Fail on changed files first, then progressively widen coverage.
3. Explicitly scope exceptions to test/bench/example targets only.

Expected impact:

- Aligns enforcement with declared engineering policy.
- Prevents additional panic-surface growth in production paths.

### 3. Remove production `todo!` stubs from live paths

Evidence:

- `crates/dsrs/src/core/module.rs:58`
- `crates/dsrs/src/core/module.rs:64`
- `crates/dsrs/src/core/lm/mod.rs:356`
- `crates/dsrs/src/core/lm/mod.rs:508`

Recommendation:

1. Replace `todo!` branches with typed errors (`Result::Err`) and explicit unsupported-capability codes.
2. Gate unsupported capabilities behind feature flags if needed.

Expected impact:

- Removes panic-at-runtime failure mode in production flows.
- Matches invariant expectation of no TODO-only stubs in production paths.

## P1 (High-Impact Refactors)

### 4. Break monolithic files by domain boundaries

Evidence (largest files):

- `apps/openagents.com/service/src/lib.rs` (`29,336` LOC, `654` functions)
- `apps/runtime/src/server.rs` (`10,247` LOC, `173` functions)
- `apps/openagents.com/web-shell/src/lib.rs` (`8,361` LOC, `213` functions)
- `crates/autopilot_ui/src/lib.rs` (`12,071` LOC, `262` functions)
- `crates/wgpui/src/platform.rs` (`5,953` LOC)

Recommendation:

1. `openagents-control-service`: split by API domain (`auth`, `sync`, `runtime-admin`, `stats`, `render`), keep route wiring thin.
2. `runtime-service`: split `server.rs` into transport handlers vs domain services.
3. `web-shell`: move `thread_local` global state and handler registration into composable modules (`state`, `dom`, `network`, `routing`, `lifecycle`).
4. `autopilot_ui`: separate layout/constants/state transitions/renderers.

Expected impact:

- Lower merge conflict rate and review scope.
- Reduces accidental coupling and brace/scope regressions in giant files.

### 5. Isolate `unsafe` to narrow FFI/rendering boundaries

Evidence:

- `crates/wgpui/src/platform.rs` contains the dominant unsafe surface.
- `crates/openagents-client-core/src/ffi.rs` contains a dense extern boundary with raw-pointer handling.

Recommendation:

1. Introduce explicit unsafe boundary modules (`ffi/raw.rs`, `platform/raw_surface.rs`) with safe wrappers.
2. Require `SAFETY:` contract comments on each unsafe block/function.
3. Add targeted tests for null pointers, ownership transfer, and double-free prevention on FFI paths.

Expected impact:

- Contains unsafety blast radius.
- Eases auditing and iOS parity verification.

### 6. Replace ad-hoc runtime HTTP JSON fetch helpers with a typed client crate

Evidence:

- Control service uses hand-built runtime URLs and decoding in `apps/openagents.com/service/src/lib.rs:9427`, `:9441`, `:9766`.

Recommendation:

1. Extract runtime internal API access to a dedicated crate (e.g. `crates/openagents-runtime-client`).
2. Centralize URL construction, retries/timeouts, and typed decode logic.
3. Align DTOs with proto-governed contracts where boundaries are cross-process.

Expected impact:

- Fewer copy-paste transport bugs.
- Better alignment with ADR-0002 proto-first governance and boundary clarity.

### 7. Reduce inline mega-test modules in production files

Evidence:

- Many largest files embed large `mod tests` sections (`service/lib.rs`, `runtime/server.rs`, `wgpui/platform.rs`, `autopilot_ui/lib.rs`).

Recommendation:

1. Move integration-like suites into crate-level `tests/` where practical.
2. Keep only focused unit tests near small modules.
3. Share fixtures/builders across modules to avoid repeated full-struct construction.

Expected impact:

- Improves compile times and incremental edit loops.
- Reduces production file churn from test-only changes.

## P2 (Operational Hardening)

### 8. Address web-shell memory growth regression before tightening perf budgets

Evidence:

- Latest signoff failed `rss_growth_kb` budget (`113,696` KB vs `51,200` KB budget).

Recommendation:

1. Add leak-focused instrumentation around DOM/event handler lifecycle and `thread_local` state caches.
2. Audit long-lived closure registries and cache eviction paths in `apps/openagents.com/web-shell/src/lib.rs`.
3. Run perf signoff in CI-adjacent scheduled lane and track trend line, not just single snapshots.

Expected impact:

- Stabilizes web-shell long-session behavior and avoids memory regressions.

### 9. Raise targeted test depth in large low-density crates

Signal (test markers per 1k LOC, large crates):

- `crates/openagents-cli`: `0.28`
- `crates/autopilot_ui`: `1.07`
- `apps/openagents.com/web-shell`: `1.20`
- `crates/runtime`: `1.32`
- `crates/autopilot`: `1.36`

Recommendation:

1. Add deterministic scenario tests around state reducers and protocol adapters.
2. Prioritize panic-hot modules first (`nostr`, `issues`, `compute`, `service`, `wgpui`).

Expected impact:

- Higher confidence during refactors and migration sequencing.

## ADR/Invariant Alignment Notes

Recommendations above preserve:

1. Proto-first contract authority (`INV-01`, ADR-0002).
2. HTTP command authority + WS-only Khala live transport (`INV-02`, `INV-03`, ADR-0003/0008).
3. Control/runtime/Khala authority isolation (`INV-04`, `INV-05`, `INV-06`).
4. Replay/idempotency and migration discipline (`INV-07`, `INV-09`, `INV-10`).
5. iOS Rust/WGPUI product authority boundaries (`INV-11`).

## Suggested Execution Order

1. P0-1: Unblock control-service compile and convert to shared test config builder.
2. P0-2: Add clippy lane (changed-files mode) and lock panic/todo growth.
3. P1-4: Split service/runtime/web-shell monoliths into boundary modules.
4. P1-5: Isolate unsafe boundaries and add focused safety tests.
5. P2-8/P2-9: Stabilize web-shell memory budget and expand tests in large low-density crates.
