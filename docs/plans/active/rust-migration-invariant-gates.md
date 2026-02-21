# Rust Migration Invariant Gates (OA-RUST-003)

Status: Active
Last updated: 2026-02-21
Owner lane: `owner:contracts-docs` (interim DRI `@AtlantisPleb`)
Source issue: `OA-RUST-003`

## Purpose

Convert Rust-endstate architecture invariants into explicit PR and release gates that can be applied consistently during migration work.

Source architecture: `docs/ARCHITECTURE-RUST.md`

## Non-Negotiable Invariants and Verification Mapping

| ID | Invariant | Applies to | Gate evidence | Verification command(s) |
| --- | --- | --- | --- | --- |
| `INV-01` | Proto-first for cross-process/client-server contracts | Control service, runtime, Khala, clients | Contract changes made in `proto/` first; generated outputs updated | `buf lint`; `buf breaking --against ...`; `./scripts/verify-proto-generate.sh` |
| `INV-02` | Authority mutations are HTTP API only (no command RPC over WS) | Control service, runtime, clients | Mutation path documented and implemented via HTTP APIs | `rg -n \"command|mutation\" docs/ARCHITECTURE-RUST.md docs/sync`; endpoint/integration test evidence |
| `INV-03` | Khala live sync is WS-only (no new SSE/poll lanes) | Khala + clients | Subscription/live updates use WS; fallback lanes not introduced | `rg -n \"SSE|EventSource|poll\" apps docs/sync --glob '!**/node_modules/**'` |
| `INV-04` | Control/runtime authority plane boundaries are preserved | Control service, runtime, DB | No cross-plane writes; cross-plane reads only through APIs; no cross-plane SQL joins in production code | DB migration review + contract checks (`mix runtime.contract.check` until runtime Rust cutover) |
| `INV-05` | No implicit in-memory coupling across control/runtime/Khala in production | Service boundaries | Cross-service calls are explicit network/proto contracts | Architecture review against `docs/ARCHITECTURE-RUST.md`; service import/dependency review |
| `INV-06` | Khala is projection/replay delivery only (not authority write path) | Runtime + Khala | Khala writes only sync metadata; authority writes remain runtime/control | Runtime/Khala integration tests and schema ownership review |
| `INV-07` | Logical ordering by `(topic, seq)` and client idempotent apply | Khala + clients | `seq` oracle and duplicate discard behavior documented/tested | Khala tests + client replay tests (`OA-RUST-029`, `OA-RUST-043`, `OA-RUST-089` gates) |
| `INV-08` | WorkOS is auth identity provider; control-plane owns authz/session/device revocation | Auth flows | WorkOS auth and control-plane session authority split is preserved | Auth integration tests + flow review (`OA-RUST-078`, `OA-RUST-079`, `OA-RUST-080`) |
| `INV-09` | Runtime deploy always followed by migrate job | Runtime ops | Deploy evidence includes migration execution | `apps/runtime/deploy/cloudrun/deploy-runtime-and-migrate.sh` |
| `INV-10` | Legacy surfaces are deleted only after mapped parity gates | Migration program | Deletion links to OA-RUST dependencies and inventory gates | `docs/plans/active/rust-migration-legacy-dependency-inventory.md` + roadmap dependency checks |

## Mandatory PR Checklist (Migration Work)

All migration PRs/issues must explicitly address:

1. Invariant impact:
   - `INV-*` IDs affected by this change.
2. Contract impact:
   - if cross-boundary contract changed, confirm `proto/` update first (`INV-01`).
3. Transport impact:
   - confirm no WS command RPC and no SSE/poll lane additions (`INV-02`, `INV-03`).
4. Authority boundaries:
   - confirm no cross-plane SQL join/write behavior (`INV-04`).
5. Service boundaries:
   - confirm no implicit in-memory coupling (`INV-05`).
6. Operations:
   - runtime deploy/migration implications documented (`INV-09`).
7. Deletion/parity:
   - if deleting legacy lane, mapped parity gate issue(s) referenced (`INV-10`).

## Release Gate Checklist (Before Promotion)

1. `INV-01` contract checks pass (buf lint/breaking/generate).
2. Runtime contract and replay checks pass for touched surfaces.
3. No unresolved invariant violations remain in PR review comments.
4. Runtime deployments include migration-job evidence (`INV-09`).
5. Any legacy deletion/change references parity dependencies and owner signoff (`INV-10`).

## Reviewer Guidance

1. Require explicit `INV-*` mapping in PR description.
2. Mark PR as blocked if any non-negotiable invariant is violated or unaddressed.
3. Escalate unresolved boundary violations to architecture owner before merge.
4. For docs-only PRs, reviewers must mark each non-applicable invariant as `N/A` with a short reason.

## Escalation Rules for Violations

1. `INV-01` to `INV-06` violation -> block merge immediately.
2. `INV-07` to `INV-08` violation -> block merge unless owner-approved mitigation + follow-up issue exists.
3. `INV-09` to `INV-10` violation -> block release promotion until remediation is complete.

## Dry-Run Against Recent Migration Changes

Dry-run sample set (direct commits on `main` in lieu of PRs):

| Change | Result | Notes |
| --- | --- | --- |
| `93e9f5d7f` (`OA-RUST-001`) | Pass | Program/doc workflow update; invariants documented, no service boundary changes |
| `5c1870fc0` (`OA-RUST-002`) | Pass | Inventory/docs only; no contract or runtime boundary mutation |
| `4b6478db0` (architecture guardrails update) | Pass | Invariant source doc strengthened; no runtime/control transport mutation |

## Canonical References

1. `docs/ARCHITECTURE-RUST.md`
2. `docs/ARCHITECTURE-RUST-ROADMAP.md`
3. `docs/plans/active/rust-migration-execution-control-plane.md`
4. `docs/plans/active/rust-migration-legacy-dependency-inventory.md`
