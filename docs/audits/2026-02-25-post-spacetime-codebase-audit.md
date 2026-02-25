# 2026-02-25 Post-Spacetime Full Codebase Audit

Status: comprehensive post-refactor snapshot  
Date: 2026-02-25  
Owner: repo audit (Codex)

## Scope

Requested audit scope:

1. Review current `docs/audits/*` and `docs/plans/*`.
2. Review latest GitHub issue stream (last ~50 issues; audited with 60 most recent).
3. Audit full current codebase for remaining cleanup, improvement, and upgrade opportunities after Spacetime refactor closure.

This audit is code-first: when docs and code conflict, code is treated as implemented truth and docs are marked as drift.

## Mandatory Preflight Authority Check

Reviewed before analysis and before edits:

1. `docs/adr/INDEX.md`
2. `docs/plans/rust-migration-invariant-gates.md`
3. `docs/core/PROJECT_OVERVIEW.md`
4. `docs/core/ARCHITECTURE.md`
5. `docs/core/ROADMAP.md`

Constraints applied:

1. Spacetime-only retained live sync transport is mandatory (`ADR-0010`, `INV-03`).
2. Proto-first, authority isolation, replay/idempotency invariants remain hard gates (`INV-01`, `INV-04..INV-07`).
3. `.github/workflows` automation is prohibited in-repo (`INV-12`).
4. Compatibility lanes are migration debt and sunset-scoped, not growth surfaces.

## Evidence Reviewed

## Documentation surfaces

Reviewed active audit and plan indices and current documents under:

1. `docs/audits/*`
2. `docs/plans/*`
3. `docs/core/*` canonical architecture/ownership/roadmap docs
4. `docs/sync/*` and `docs/protocol/*` active sync/protocol docs

## GitHub issue stream (last ~50+)

Command evidence: `gh issue list --state all --limit 60 --search "sort:updated-desc"`

Result:

1. 60 most recent issues are all closed.
2. Issue range: `#2235` through `#2294`.
3. Composition:
   - `34` issues in `OA-SPACETIME-*` (`#2235..#2268`)
   - `26` issues in `OA-SPACETIME-TOTAL-*` (`#2269..#2294`)
4. Open issues currently: `0` (`gh issue list --state open`).

Interpretation: the Spacetime migration/refactor train closed aggressively and recently; this audit focuses on post-closure residue and non-transport cleanup debt.

## Runtime verification evidence executed

1. `./scripts/spacetime/verify-spacetime-only-symbols.sh` -> pass
2. `./scripts/spacetime/runtime-desktop-e2e.sh` -> pass
3. `cargo check -p openagents-proto -p autopilot-spacetime -p autopilot-desktop -p openagents-runtime-service -p openagents-control-service` -> pass

## Codebase snapshot metrics (current main)

1. Tracked files: `13,490`
2. Workspace packages: `52`
3. Tracked Rust files: `1,296`
4. Rust LOC total (tracked): `553,871`
   - `apps/*`: `116,121`
   - `crates/*`: `437,369`
5. Largest Rust files:
   - `apps/openagents.com/service/src/lib.rs` -> `18,044` LOC
   - `apps/openagents.com/service/src/tests.rs` -> `11,944` LOC
   - `apps/runtime/src/server/tests.rs` -> `6,868` LOC
   - `apps/autopilot-desktop/src/main.rs` -> `6,748` LOC
   - `apps/runtime/src/server.rs` -> `6,684` LOC
6. Router complexity proxies:
   - `apps/openagents.com/service/src` `.route(` occurrences: `115`
   - `apps/runtime/src` `.route(` occurrences: `83`
7. Compatibility debt proxy:
   - `route_split|runtime_routing` references in control service source: `223`

## Findings (severity ordered)

## Critical

1. Active docs still cite superseded sync ADRs in canonical/active files.
   - Active references found to `ADR-0009` and `ADR-0003` in:
     - `docs/core/ARCHITECTURE.md`
     - `docs/plans/spacetimedb-full-integration.md`
     - `docs/sync/SPACETIME_CUTOVER_ACCEPTANCE_AND_ROLLBACK.md`
     - `docs/plans/ep212-autopilot-bitcoin-100pct.md`
     - `docs/plans/2026-02-23-open-agent-economy-execution-plan.md`
   - This conflicts with current authority index where `ADR-0010` is accepted and supersedes prior sync transport ADR posture.

2. Repository still carries a large tracked legacy/vendor payload under `apps/openagents.com`.
   - `apps/openagents.com/vendor`: `11,272` tracked files.
   - `apps/openagents.com/storage`: `200` tracked files, including parity/cutover artifacts and generated framework views.
   - Largest tracked file in repo is vendored binary `apps/openagents.com/vendor/laravel/pint/builds/pint` (~15.6 MB).
   - Literal `.github/workflows` files exist inside tracked vendor subtree (`10` files), creating policy ambiguity with `INV-12`.
   - Effect: architecture clarity, review signal quality, and repository hygiene remain weak despite Spacetime transport closure.

3. Control service compile warning debt is very high and dominated by dead code.
   - `cargo check -p openagents-control-service` reports `148` warnings.
   - Breakdown from warning stream:
     - `function_* never used`: `119`
     - `constant_* never used`: `9`
     - `struct_* never constructed`: `14`
     - `field_* never read`: `2`
     - `unused import(s)`: `5`
   - Most warnings cluster around web/render/stats/compatibility routing residue in the control monolith.

## High

4. Monolith concentration remains a major maintainability/regression risk.
   - `apps/openagents.com/service/src/lib.rs` (`18k LOC`)
   - `apps/runtime/src/server.rs` (`6.7k LOC`)
   - `apps/autopilot-desktop/src/main.rs` (`6.7k LOC`)
   - These are still the dominant blast-radius files after Spacetime closure.

5. Compatibility lane complexity remains high even after sunset headers/signoff.
   - `route_split` / `runtime_routing` code remains deeply present in control service (`223` source references).
   - Sunset metadata is wired, but compatibility surface still imposes substantial complexity tax.

6. Tracked generated parity/cutover artifacts in `apps/openagents.com/storage` remain as active repo payload.
   - Includes historical parity/cutover snapshots and generated assets (`*.json`, `*.jsonl`, `*.headers`, `*.body`, framework compiled view outputs).
   - These artifacts include stale legacy/Khala strings and increase false-positive audit noise.

7. Dependency/version skew remains non-trivial across workspace.
   - Duplicate major/version families include:
     - `axum` (`0.7.9`, `0.8.8`)
     - `tower` (`0.4.13`, `0.5.2`)
     - `which` (`6.0.3`, `7.0.3`, `8.0.0`)
     - `thiserror` (`1.0.69`, `2.0.17`)
     - plus many additional multi-version crates in `cargo tree --workspace -d`.
   - Not immediately broken, but this raises compile cost, binary size, and update complexity.

## Medium

8. `unwrap/expect` usage remains high in non-test production paths.
   - Across `apps/*` + `crates/*` non-test/non-bench/non-example paths: `2,772` occurrences.
   - Notable service hotspots include:
     - `apps/openagents.com/service/src/domain_store.rs` (`65`)
     - `apps/openagents.com/service/src/auth.rs` (`19`)
     - `apps/runtime/src/liquidity_pool/service.rs` (`12`)
     - `apps/runtime/src/fx/service.rs` (`11`)
   - Recommendation is targeted reduction in authority and ingestion paths first.

9. Backlog hygiene gap: no currently open issues after closure train.
   - `gh issue list --state open` returns empty.
   - Given remaining architectural/code hygiene debt, this creates execution risk unless a fresh backlog is seeded immediately.

## Positive signals (what is working)

1. Spacetime-only transport regression guards pass on retained surfaces.
2. Runtime-to-desktop Spacetime E2E suite passes.
3. Retained source scan (`apps/runtime/src`, `apps/autopilot-desktop/src`, `apps/openagents.com/service/src`, key sync crates/proto) does not show active `khala|convex` symbol usage.
4. Core package checks for retained services/clients compile successfully.

## Recommended cleanup / upgrade sequence

## Phase 0 (immediate, highest leverage)

1. Reconcile active doc authority to `ADR-0010` in canonical/active docs.
2. Decide and execute one policy for `apps/openagents.com/vendor` and `apps/openagents.com/storage`:
   - archive/remove tracked artifacts that are not retained authority code, or
   - explicitly classify retained subsets with ownership and invariant exceptions.
3. Seed a new issue backlog for post-Spacetime cleanup (no open issues currently).

## Phase 1 (near-term)

1. Eliminate control service warning debt to near-zero, starting with dead code prune in `lib.rs`, `render.rs`, `stats.rs`, `auth_routes.rs`.
2. Decompose control/runtime/desktop monolith files into domain modules with explicit ownership boundaries.
3. Continue compatibility-lane retirement before sunset date (`2026-06-30`) with measurable route deletion targets.

## Phase 2 (upgrade hardening)

1. Consolidate duplicate dependency stacks where practical (`axum`, `tower`, `which`, `thiserror`, related trees).
2. Reduce `unwrap/expect` in authority-sensitive paths and replace with typed error propagation.
3. Move generated parity/cutover snapshots to archival locations outside active repo surface.

## Bottom line

Spacetime refactor closure is real and test-backed on retained runtime/desktop/control transport behavior. The biggest remaining risks are no longer sync transport correctness; they are repository hygiene, documentation authority drift, control-surface dead code/monolith complexity, and missing follow-on execution backlog.
