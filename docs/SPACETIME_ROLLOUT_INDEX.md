# Spacetime Rollout Index

This is the canonical operator/documentation index for Spacetime rollout phases, acceptance criteria, and verification commands.

## Program Status

As of 2026-03-05, the audit-driven Spacetime reintegration program backlog (`#2916` through `#2930`) is complete and merged on `main`.

## Source Audits

- `docs/audits/2026-03-04-earn-device-online-state-spacetime-audit.md`
- `docs/audits/2026-03-04-spacetime-broader-vision-gap-audit.md`

## Phase Model (Current vs Target)

| Phase | Status | Semantics | Verification |
| --- | --- | --- | --- |
| Phase 1: mirror/proxy sync discipline | Current | Desktop enforces sync token/target contract, runs replay-safe local apply/checkpoints, and uses Spacetime-shaped local presence/projection state for panes. | `cargo test -p autopilot-desktop sync_bootstrap`; `cargo test -p autopilot-desktop sync_apply`; `cargo test -p autopilot-desktop sync_lifecycle` |
| Phase 2: live remote Spacetime authority for ADR-approved domains | Target | Presence/checkpoints/projections are backed by live Spacetime subscriptions/reducers with replay/idempotency and parity gates. | `scripts/spacetime/parity-chaos-gate.sh`; `scripts/spacetime/maincloud-handshake-smoke.sh`; contract checks |

## Canonical Runbooks

- Ops procedures: `docs/SPACETIME_SYNC_OPS_RUNBOOK.md`
- Release gates and artifacts: `docs/SPACETIME_SYNC_RELEASE_GATES.md`
- Authority boundaries: `docs/adr/ADR-0001-spacetime-domain-authority-matrix.md`
- Presence counting/TTL policy: `docs/adr/ADR-0002-provider-presence-cardinality-and-ttl-policy.md`

## Required Verification Commands

1. `scripts/spacetime/verify-autopilot-sync-contract.sh`
2. `scripts/spacetime/parity-chaos-gate.sh`
3. `scripts/spacetime/maincloud-handshake-smoke.sh --db "$OA_SPACETIME_DEV_DATABASE"`

## Acceptance Criteria

- Docs must identify current phase and target phase without ambiguity.
- Pane source badges/labels must remain truthful to current phase semantics.
- Release evidence must include parity/chaos artifacts and handshake smoke artifacts.

## PM Rollout Note

Project management follows the same rollout discipline:

- Current PM work is Phase 1 only: stream-shaped local projections, shared checkpoints, and truthful `source: stream.pm.*` or `source: spacetime.sync.lifecycle` labels where applicable.
- `stream.pm.projects.v1` project records, scoped defaults, and project planning context may exist locally in Phase 1, but they remain replay-safe local PM projection truth until a later ADR-approved live cutover.
- `stream.pm.teams.v1` team records and team-scoped planning defaults may exist locally in Phase 1, but they remain replay-safe local PM projection truth until a later ADR-approved live cutover.
- Live remote PM reducers/subscriptions are Phase 2 target behavior only for domains explicitly promoted by `docs/adr/ADR-0001-spacetime-domain-authority-matrix.md`.
- Wallet, payout, trust, and policy truth remain outside PM even when PM later references those systems.
