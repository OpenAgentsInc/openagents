# 2026-03-01 Full Codebase Architecture Audit

## Scope

- `apps/autopilot-desktop`
- `crates/nostr/*`
- `crates/spark`
- `crates/wgpui*`
- lint/build guardrails

## Method

- Review against:
  - `docs/MVP.md`
  - `docs/OWNERSHIP.md`
- Run:
  - `cargo check --workspace --tests`
  - `scripts/lint/ownership-boundary-check.sh`
  - `scripts/lint/clippy-regression-check.sh`
  - `cargo fmt --all --check`
- Trend baseline:
  - prior monthly architecture audit (`docs/audits/2026-02-27-full-codebase-architecture-audit.md`)
- Audit execution date:
  - 2026-02-27 UTC (for March 2026 cadence)

## Ownership-Boundary Drift

- Changes since prior audit:
  - No new crate ownership drift detected in active MVP surfaces.
  - `scripts/lint/ownership-boundary-check.sh` remains green.
- Violations found:
  - None in current run.
- Remediation actions:
  - Keep new strict lane and lint governance gates in `scripts/lint/clippy-regression-check.sh` as mandatory pre-merge posture.

## Largest-File Trend

- Top file sizes (current):
  - `apps/autopilot-desktop/src/app_state.rs` (3,439)
  - `crates/wgpui/examples/storybook/sections/products.rs` (2,901)
  - `crates/wgpui/examples/storybook/sections/flows.rs` (2,654)
  - `apps/autopilot-desktop/src/input.rs` (2,525)
  - `apps/autopilot-desktop/src/pane_renderer.rs` (2,452)
  - `apps/autopilot-desktop/src/pane_system.rs` (2,341)
- Delta vs previous monthly architecture audit:
  - Total Rust LOC (apps + crates): `133,556 -> 149,192` (`+15,636`)
  - `app_state.rs`: `3,404 -> 3,439` (`+35`)
  - `input.rs`: `3,133 -> 2,525` (`-608`)
  - `pane_renderer.rs`: `2,427 -> 2,452` (`+25`)
  - `pane_system.rs`: `2,220 -> 2,341` (`+121`)
- Concentration risks:
  - Autopilot state/input/render surfaces are still high-concentration maintenance hotspots.
  - Storybook section files are now among the largest files in-repo and should be decomposed for reviewability.

## Dead-Code Warning Trend

- Current warning set:
  - `cargo check --workspace --tests --message-format=json` reports `0` dead-code warnings.
- Delta vs previous monthly architecture audit:
  - Prior audit reported dead/legacy runtime-path warnings in active app modules.
  - Current run shows dead-code warnings reduced to zero.
- Highest-priority removals/wiring:
  - Remaining targeted `#[allow(dead_code)]` annotations still exist in:
    - `apps/autopilot-desktop/src/app_state.rs`
    - `apps/autopilot-desktop/src/state/operations.rs`
    - `crates/nostr/core/src/nip65.rs`

## Lint-Gate Trend

- `clippy-regression-check`: pass.
  - Includes ownership, dependency drift, debt-allowlist validation, touched-file gate, strict production hardening lanes, warning-budget check, and baseline lanes.
- `fmt --check`: pass.
- Any lane blockers:
  - None in current run.
- Trend delta from previous monthly architecture audit:
  - Prior audit: `clippy-regression-check` failed; `fmt --check` failed.
  - Current audit: both pass.
  - Baseline warning envelope tightened from `244/445/288` to `22/57/38` (`lib/tests/examples`).

## Findings

1. High: `autopilot-desktop` still has large concentrated runtime files (`app_state.rs`, `input.rs`, `pane_renderer.rs`, `pane_system.rs`).
2. Medium-High: WGPUI storybook sections (`products.rs`, `flows.rs`) are oversized and should be split for maintainability.
3. Medium: Targeted dead-code allowances remain in active app/protocol files and should continue to be burned down.
4. Medium: Newly introduced warning budgets are effective, but budgeted files still carry persistent warnings that should be reduced.

## Recommendations

1. Continue decomposing `autopilot-desktop` state/input ownership into domain modules with no behavior changes.
2. Remove or runtime-wire remaining dead-code allowances in active app + Nostr files.
3. Burn down warning-budget files and lower budgets in lockstep as warnings are removed.
4. Split large WGPUI storybook section files (`products.rs`, `flows.rs`) into smaller modules.

## Follow-On Issues

- #2399 `[cleanup][P1] Continue autopilot-desktop state/input decomposition`
- #2400 `[cleanup][P2] Remove remaining dead-code allowances in active app + Nostr files`
- #2401 `[cleanup][P2] Burn down strict-warning budget files and lower budgets`
- #2402 `[cleanup][P3] Split large WGPUI storybook section files`
- #2403 `[cleanup][P3] Monthly architecture hygiene audit - 2026-04` (next-month scheduling rule)
