# Spacetime Sync Roadmap (Canonical Lane)

Date: 2026-02-25
Status: Active
Owner lanes: Runtime, Desktop, Control, Protocol, Infra
Authority ADRs: `docs/adr/ADR-0007-spacetime-only-sync-transport-hard-mandate.md`, `docs/adr/ADR-0002-proto-first-contract-governance.md`

## Program Goal

Operate one retained sync/replay lane with deterministic replay, idempotent apply, and production-safe rollout posture.

Primary retained client surface:

- `apps/autopilot-desktop`

## Non-Negotiable Constraints

1. Runtime/Postgres remains authority for execution state.
2. Sync transport is delivery/projection only (no authority writes).
3. Retained live sync doctrine is Spacetime transport semantics.
4. Proto contracts remain schema authority.
5. Commands are HTTP authority operations; sync delivery remains non-authority.
6. Client apply remains idempotent and ordered by `(stream_id, seq)`.

## Current Baseline (2026-02-25)

Completed baseline capabilities:

1. Runtime publishes retained sync events through Spacetime publisher path.
2. Runtime exposes sync observability at `/internal/v1/spacetime/sync/metrics`.
3. Retired runtime Spacetime internal endpoints return deterministic `404` and are no longer active authority lanes.
4. Control service issues scoped sync claims through canonical `POST /api/sync/token` endpoint.

Remaining closure work:

1. Remove remaining Spacetime-named compatibility references in docs/tooling when touched.
2. Keep replay/resume/chaos harnesses green across runtime/shared-client/desktop.
3. Complete final migration audit and residual debt closure.

## Workstreams

### SYNC-001: Contract and Negotiation Stability

Status: Active

Done when:

- `spacetime.sync.v1` compatibility window remains deterministic.
- protocol/schema/build-window failures remain machine-readable and consistent.

Verification:

- `cargo test -p openagents-client-core compatibility::tests -- --nocapture`

### SYNC-002: Runtime Delivery Correctness

Status: Active

Done when:

- publish metrics and retired-route guards remain green.
- runtime delivery signals remain within rollout SLO gates.

Verification:

- `cargo test -p openagents-runtime-service spacetime_publisher::tests::http_publish_failure_queues_outbox_for_retry -- --nocapture`
- `cargo test -p openagents-runtime-service retired_spacetime_routes_return_not_found -- --nocapture`

### SYNC-003: Cross-Surface Replay/Resume Reliability

Status: Active

Done when:

- shared-client and desktop replay/resume lanes remain deterministic under reconnect/backoff churn.

Verification:

- `./scripts/spacetime/replay-resume-parity-harness.sh`

### SYNC-004: Chaos and Promotion Discipline

Status: Active

Done when:

- deterministic chaos drill suite is green for canary/prod promotions.
- artifacts are attached for go/no-go decisions.

Verification:

- `./scripts/spacetime/run-chaos-drills.sh`

## Rollout Gates

Do not advance cohort promotion unless all are green:

1. sync auth/stream error rates remain within SLO.
2. replay bootstrap latency remains in budget.
3. reconnect storms remain bounded.
4. duplicate delivery stays deterministic and idempotent.
5. desktop sync UX remains stable with no out-of-order regressions.

## References

- `docs/sync/RUNTIME_CODEX_CUTOVER_RUNBOOK.md`
- `docs/sync/SPACETIME_PARITY_HARNESS.md`
- `docs/sync/SPACETIME_CHAOS_DRILLS.md`
- `docs/core/ARCHITECTURE.md`
