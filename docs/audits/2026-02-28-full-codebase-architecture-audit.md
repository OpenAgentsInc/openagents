# 2026-02-28 Full Codebase Architecture Audit (Rust Refactor + Best-Practices)

## Scope

- `apps/autopilot-desktop`
- `crates/codex-client`
- `crates/nostr/*`
- `crates/spark`
- `crates/wgpui*`
- lint/build/test guardrails

## Method

- Reviewed against:
  - `docs/MVP.md`
  - `docs/OWNERSHIP.md`
- Ran:
  - `cargo fmt --all -- --check`
  - `cargo check --workspace`
  - `cargo check --workspace --tests --message-format=json`
  - `cargo clippy --workspace --lib --message-format=json -- -W clippy::all`
  - `cargo clippy -p autopilot-desktop --bin codex-live-harness -- -W clippy::all`
  - `scripts/lint/ownership-boundary-check.sh`
  - `scripts/lint/workspace-dependency-drift-check.sh`
  - `scripts/skills/validate_registry.sh`
  - `scripts/lint/strict-production-hardening-check.sh`
  - `scripts/lint/clippy-regression-check.sh`
  - `scripts/lint/clippy-warning-budget-check.sh`
  - `cargo test --workspace --quiet` (timed out)
  - `cargo test -p autopilot-desktop codex_lane::tests::server_request_command_approval_round_trip -- --nocapture` (timed out)
- Trend baseline:
  - `docs/audits/2026-03-01-full-codebase-architecture-audit.md`

## Ownership-Boundary Drift

- Changes since prior audit:
  - No new crate boundary drift detected.
- Violations found:
  - None (`scripts/lint/ownership-boundary-check.sh` passed).
- Remediation actions:
  - Keep boundary check mandatory in lint gate.

## Largest-File Trend

- Current Rust footprint:
  - 419 Rust files
  - 169,839 LOC across `apps/` + `crates/`
- Delta vs prior monthly audit:
  - 149,192 -> 169,839 (`+20,647 LOC`)
- Top concentration files (current):
  - `apps/autopilot-desktop/src/codex_lane.rs` (5,285)
  - `apps/autopilot-desktop/src/app_state.rs` (5,155)
  - `apps/autopilot-desktop/src/input.rs` (4,298)
  - `apps/autopilot-desktop/src/pane_system.rs` (3,500)
  - `apps/autopilot-desktop/src/pane_renderer.rs` (2,700)
- Notable deltas vs prior monthly audit:
  - `app_state.rs`: 3,439 -> 5,155 (`+1,716`)
  - `input.rs`: 2,525 -> 4,298 (`+1,773`)
  - `pane_system.rs`: 2,341 -> 3,500 (`+1,159`)
  - `pane_renderer.rs`: 2,452 -> 2,700 (`+248`)
- Concentration risks:
  - Runtime/state/input/render logic is re-converging into very large files, increasing merge-conflict, regression, and review risk.

## Dead-Code Warning Trend

- Current warning set:
  - `cargo check --workspace --tests --message-format=json` reported `0` warnings total, including `0` `dead_code` warnings.
- Delta vs prior monthly audit:
  - Maintained at zero dead-code warnings.
- Highest-priority removals/wiring:
  - `#[allow(dead_code)]` remains in active modules (e.g. `apps/autopilot-desktop/src/app_state.rs`, `apps/autopilot-desktop/src/codex_lane.rs`, `apps/autopilot-desktop/src/state/alerts_recovery.rs`) and should continue to be burned down with ownership tickets.

## Lint-Gate Trend

- `cargo fmt --all -- --check`: pass.
- `cargo check --workspace`: pass.
- `clippy-regression-check`: fail.
  - Ownership/dependency/skills/debt-allowlist gates pass.
  - Failure is in strict production hardening lane for `autopilot-desktop`.
- `strict-production-hardening-check`: fail.
  - `autopilot-desktop` currently violates `-D clippy::print_stderr` via many `eprintln!` calls in production paths (`input.rs`, `codex_lane.rs`, `input/reducers/codex.rs`, `app_state.rs`).
- `clippy-warning-budget-check`: fail.
  - Fails before budget assertion due clippy compile errors in autopilot binaries.
- Library-only clippy signal:
  - 19 warnings total (`nostr` 11, `wgpui` 7, `nostr-client` 1).
  - Highest warning concentration: `crates/nostr/core/src/nip60.rs` (7), `crates/wgpui/src/components/organisms/thread_entry.rs` (3), `crates/nostr/core/src/nip34.rs` (2).

## Testability Trend

- `cargo test --workspace --quiet` did not complete within 300s.
- A targeted test also timed out:
  - `codex_lane::tests::server_request_command_approval_round_trip` in `apps/autopilot-desktop/src/codex_lane.rs`.
- This indicates regression risk in deterministic test execution for lane/threaded integration tests.

## Findings (Ranked)

### 1) Critical: `autopilot-desktop` has re-consolidated into multiple mega-modules

Evidence:
- `apps/autopilot-desktop/src/codex_lane.rs` (5,285)
- `apps/autopilot-desktop/src/app_state.rs` (5,155)
- `apps/autopilot-desktop/src/input.rs` (4,298)
- `PaneKind` and top-level app state live in the same file as broad pane state definitions and tests (`app_state.rs`).

Impact:
- Slower reviews, high coupling, difficult localized testing, and high probability of cross-feature regressions.

### 2) Critical: Strict production lint contract is broken by logging strategy

Evidence:
- `scripts/lint/strict-production-hardening-check.sh` fails with 39+ hard errors on `eprintln!` in production bin paths.
- `codex-live-harness` clippy lane reports 61 `println!` errors under workspace lint policy.

Impact:
- The declared hardening posture is not enforceable, and CI/lint policy is currently contradictory to runtime diagnostics implementation.

### 3) High: Integration test reliability is degraded (hanging lane test)

Evidence:
- `cargo test --workspace --quiet` timed out.
- `codex_lane::tests::server_request_command_approval_round_trip` timed out repeatedly.

Impact:
- Slower development loop and reduced confidence in protocol/runtime changes.

### 4) High: Codex transcript assembly still relies on brittle string heuristics

Evidence:
- Message assembly logic in `apps/autopilot-desktop/src/app_state.rs` manipulates ad-hoc `"Reasoning:\n"` and `"\n\nAnswer:\n"` delimiters.
- Reducer pipeline has duplicate suppression and fallback transcript probing heuristics in `apps/autopilot-desktop/src/input/reducers/codex.rs`.

Impact:
- Susceptible to duplicated/misordered rendering and hard-to-debug edge cases as app-server event shapes evolve.

### 5) High: Codex lane surface is too manual and drift-prone

Evidence:
- Large manual command/notification mapping in `apps/autopilot-desktop/src/codex_lane.rs` and `apps/autopilot-desktop/src/input/reducers/codex.rs`.
- Large API type monolith in `crates/codex-client/src/types.rs` (1,590 LOC).

Impact:
- High maintenance overhead and protocol drift risk whenever app-server methods evolve.

### 6) Medium-High: Concurrency model relies on polling + blocking channels in UI-adjacent runtime lanes

Evidence:
- `std::sync::mpsc` + `recv_timeout` and fixed poll intervals in `runtime_lanes.rs` / `codex_lane.rs`.
- Lane control/update loops are thread-based and periodically polled by UI state pump.

Impact:
- Harder deterministic testing, awkward shutdown/error semantics, and avoidable latency/CPU churn.

### 7) Medium: WGPUI remains very large for MVP lane, with debt concentrated under allowlists

Evidence:
- `crates/wgpui`: 93,443 LOC total (`src` 75,769 + `examples` 17,322).
- 31 `#[allow(...)]` annotations in `wgpui`; 83 entries in clippy debt allowlist across workspace.

Impact:
- Slower broad refactors and persistent warning debt that obscures true regressions.

### 8) Medium: Inline tests are heavily co-located with production code in large protocol/runtime files

Evidence:
- `#[cfg(test)]` blocks embedded inside large production files (`app_state.rs`, `input.rs`, `codex_lane.rs`, multiple `nostr/core` modules).

Impact:
- Harder navigation and higher accidental coupling between test fixtures and production internals.

## Recommendations

## P0 (Immediate: restore enforceable quality posture)

1. Replace production `eprintln!`/`println!` usage with a unified logging facade (`tracing` + structured fields + lane/category filters).
2. Add explicit binary-level lint policy exceptions only where intentional (e.g., CLI harness output), and document them.
3. Make strict hardening lane pass on `autopilot-desktop` before additional feature work.
4. Triage and fix hanging codex lane test; add bounded timeouts and deterministic shutdown guards for lane test fixtures.

## P1 (Near-term: modularity + testability)

1. Split `autopilot-desktop` mega-files into domain modules:
   - `app_state`: chat/codex/jobs/wallet/simulations/settings submodules
   - `input`: dispatch/core actions/domain reducers
   - `codex_lane`: transport/session/command-router/notification-normalizer/test-fixtures
2. Extract codex transcript state to a structured model (`reasoning`, `answer`, `events`, `status`) and remove string-delimiter heuristics.
3. Separate heavy integration tests from production modules into dedicated test modules/files with reusable harness utilities.

## P2 (Mid-term: protocol resilience + maintainability)

1. Split `codex-client/src/types.rs` into method-group modules (`thread`, `turn`, `account`, `mcp`, `skills`, etc.).
2. Introduce generated or schema-driven method/notification mapping where possible to reduce manual drift in codex lane.
3. Reduce clippy warning hotspots (`nip60`, `thread_entry`, `nip34`) and lower budgets in lockstep.
4. Continue WGPUI decomposition with clear MVP/non-MVP boundaries for examples/storybook/testing surfaces.

## P3 (Process and guardrails)

1. Add a dedicated audit command that runs all required metrics and emits a machine-readable snapshot.
2. Track module-size budgets (soft fail/warn) for key files (`codex_lane.rs`, `app_state.rs`, `input.rs`, `pane_system.rs`).
3. Require per-PR evidence for any new `#[allow(...)]` in active runtime paths, with expiry ticket reference.

## Proposed Refactor Sequence

1. Logging hardening pass (`eprintln!/println!` cleanup + strict lane green)
2. Stabilize codex lane test harness (fix hanging approval round-trip test)
3. `app_state` decomposition
4. `input` decomposition
5. `codex_lane` decomposition + codex-client type split
6. Transcript model rework (remove string delimiters)
7. Warning-budget burn-down and allowlist reduction

## Validation Artifacts (This Audit Run)

- Ownership boundary: pass
- Workspace dependency drift: pass
- Skills registry validation: pass
- `cargo fmt --check`: pass
- `cargo check --workspace`: pass
- `cargo check --workspace --tests --message-format=json`: 0 warnings
- Strict hardening: fail (`autopilot-desktop` print_* lint violations)
- Clippy regression check: fail (strict lane failure)
- Clippy warning budget check: fail (clippy compile errors in autopilot bins)
- Workspace tests: timeout due hanging codex lane approval test
