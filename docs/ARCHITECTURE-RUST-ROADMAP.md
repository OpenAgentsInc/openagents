# OpenAgents Rust-Only Roadmap

Status: active
Last updated: 2026-02-21

This roadmap defines the Rust-only migration and hardening sequence.

## Program Goals

1. Rust-only product/runtime implementation.
2. Proto-first wire contracts.
3. WS-only Khala live transport.
4. Shared client-core behavior across web/desktop/iOS.
5. Clear control/runtime/Khala authority boundaries.

## Phase 1: Authority and Topology Lock

1. Keep `docs/ARCHITECTURE-RUST.md` as canonical architecture authority.
2. Keep WorkOS as identity/auth provider with control service as auth/session authority.
3. Enforce no cross-plane SQL joins in production code.
4. Enforce no authority mutations over Khala WebSocket lanes.

## Phase 2: Contract Governance

1. Maintain proto-first contract governance under `proto/`.
2. Keep generated Rust wire types in `crates/openagents-proto/` in sync.
3. Enforce compatibility policy for control APIs and Khala protocol versions.
4. Keep fixture contracts updated in `docs/protocol/fixtures/`.

## Phase 3: Runtime + Khala Reliability

1. Keep runtime deploy+migrate coupling mandatory.
2. Keep replay/resume/stale-cursor correctness tests green.
3. Enforce idempotent client apply rules (`seq <= last_applied` discard).
4. Maintain reconnect/replay chaos drill runbooks and execution cadence.

## Phase 4: Web Shell Completion

1. Keep `apps/openagents.com/service/` as control API + static host authority.
2. Keep `apps/openagents.com/web-shell/` as Rust/WASM WGPUI shell.
3. Remove residual legacy runtime coupling from production web paths.
4. Keep SW/build compatibility gates for client version skew control.

## Phase 5: Desktop + iOS Parity

1. Keep desktop as primary Codex operator surface.
2. Align iOS command/replay behavior with desktop via shared Rust client core.
3. Keep cross-surface contract harness current and executable.
4. Ensure identical auth/session/replay semantics across surfaces.

## Phase 6: Operations and Observability

1. Maintain golden signals for control/runtime/Khala.
2. Maintain alert coverage for replay drift, slow consumers, and auth failures.
3. Keep incident runbooks current for WS/auth/reconnect/stale-cursor failures.
4. Keep staging/prod validation matrix as release gate.

## Phase 7: Legacy Infrastructure Cutover

1. Keep staging on Rust lane with production safety controls.
2. Preserve Laravel rollback resources until final approved production cutover.
3. Run fresh DB backup + validation before destructive production deletion.
4. Remove legacy infra only after all required Rust gates pass.

## Phase 8: Docs and Governance Hygiene

1. Keep in-repo docs limited to current system and forward strategy.
2. Move stale/historical docs to backroom archive batches.
3. Keep ADR set current; supersede obsolete decisions explicitly.
4. Keep `scripts/docs-check.mjs` passing on every docs change.

## Active Gate References

- `docs/plans/active/rust-migration-invariant-gates.md`
- `docs/plans/active/rust-migration-execution-control-plane.md`
- `docs/RUST_STAGING_PROD_VALIDATION.md`
- `docs/RUST_LEGACY_INFRA_DECOMMISSION.md`

## Verification Baseline

```bash
./scripts/local-ci.sh docs
./scripts/local-ci.sh workspace-compile
./scripts/local-ci.sh proto
scripts/release/validate-rust-cutover.sh
```
