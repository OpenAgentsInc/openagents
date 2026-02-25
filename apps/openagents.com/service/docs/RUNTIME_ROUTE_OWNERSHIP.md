# Runtime Route Ownership

Status: active  
Last updated: 2026-02-25

This document is the canonical control/runtime route ownership matrix used for boundary hardening.

Authoritative code contracts:

- `apps/openagents.com/service/src/runtime_ownership.rs`
- `apps/runtime/src/route_ownership.rs`

Policy:

1. Each `/api/runtime/*` method/path pair has exactly one canonical owner.
2. Each `/internal/v1/*` method/path pair has exactly one canonical owner.
3. Runtime-owned `/api/runtime/*` lanes are compatibility proxies to runtime authority APIs.
4. Runtime internal authority lanes are canonical runtime-owned (`runtime_authority`).
5. If runtime service is not configured, runtime-owned `/api/runtime/*` routes fail with `503 service_unavailable`.

Compatibility lane sunset status (Phase 5 signoff):

1. Legacy chat aliases and `/api/v1/control/*` compatibility/admin lanes emit sunset metadata (`x-oa-compat-sunset-date: 2026-06-30`) and migration doc headers.
2. Runtime-driver compatibility labels are retired; only `control_service` and `runtime_service` are accepted.

## `/api/runtime/*` Ownership Matrix

| Method | Path | Owner | Delivery | Migration Status |
|---|---|---|---|---|
| POST | `/api/runtime/tools/execute` | `control_service` | `in_process` | `control_native` |
| GET | `/api/runtime/skills/tool-specs` | `control_service` | `in_process` | `control_native` |
| POST | `/api/runtime/skills/tool-specs` | `control_service` | `in_process` | `control_native` |
| GET | `/api/runtime/skills/skill-specs` | `control_service` | `in_process` | `control_native` |
| POST | `/api/runtime/skills/skill-specs` | `control_service` | `in_process` | `control_native` |
| POST | `/api/runtime/skills/skill-specs/:skill_id/:version/publish` | `control_service` | `in_process` | `control_native` |
| GET | `/api/runtime/skills/releases/:skill_id/:version` | `control_service` | `in_process` | `control_native` |
| GET | `/api/runtime/threads` | `control_service` | `in_process` | `control_native` |
| GET | `/api/runtime/threads/:thread_id/messages` | `control_service` | `in_process` | `control_native` |
| POST | `/api/runtime/threads/:thread_id/messages` | `control_service` | `in_process` | `control_native` |
| GET | `/api/runtime/codex/workers` | `control_service` | `in_process` | `control_native` |
| POST | `/api/runtime/codex/workers` | `control_service` | `in_process` | `control_native` |
| GET | `/api/runtime/codex/workers/:worker_id` | `control_service` | `in_process` | `control_native` |
| GET | `/api/runtime/codex/workers/:worker_id/stream` | `control_service` | `in_process` | `control_native` |
| POST | `/api/runtime/codex/workers/:worker_id/events` | `control_service` | `in_process` | `control_native` |
| POST | `/api/runtime/codex/workers/:worker_id/stop` | `control_service` | `in_process` | `control_native` |
| POST | `/api/runtime/codex/workers/:worker_id/requests` | `control_service` | `in_process` | `control_native` |
| GET | `/api/runtime/workers` | `runtime_service` | `runtime_proxy` | `runtime_authority_proxy` |
| POST | `/api/runtime/workers` | `runtime_service` | `runtime_proxy` | `runtime_authority_proxy` |
| GET | `/api/runtime/workers/:worker_id` | `runtime_service` | `runtime_proxy` | `runtime_authority_proxy` |
| POST | `/api/runtime/workers/:worker_id/heartbeat` | `runtime_service` | `runtime_proxy` | `runtime_authority_proxy` |
| POST | `/api/runtime/workers/:worker_id/status` | `runtime_service` | `runtime_proxy` | `runtime_authority_proxy` |

## `/internal/v1/*` Ownership Matrix

All rows below are runtime-owned internal authority lanes.

| Method | Path | Owner | Delivery | Migration Status |
|---|---|---|---|---|
| GET | `/internal/v1/openapi.json` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| POST | `/internal/v1/comms/delivery-events` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| POST | `/internal/v1/runs` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| GET | `/internal/v1/runs/:run_id` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| POST | `/internal/v1/runs/:run_id/events` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| GET | `/internal/v1/runs/:run_id/receipt` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| GET | `/internal/v1/runs/:run_id/replay` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| GET | `/internal/v1/spacetime/sync/metrics` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| GET | `/internal/v1/projectors/checkpoints/:run_id` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| GET | `/internal/v1/projectors/drift` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| GET | `/internal/v1/projectors/run-summary/:run_id` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| GET | `/internal/v1/workers` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| POST | `/internal/v1/workers` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| GET | `/internal/v1/workers/:worker_id` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| POST | `/internal/v1/workers/:worker_id/heartbeat` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| POST | `/internal/v1/workers/:worker_id/status` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| GET | `/internal/v1/workers/:worker_id/checkpoint` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| GET | `/internal/v1/marketplace/catalog/providers` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| GET | `/internal/v1/marketplace/catalog/job-types` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| GET | `/internal/v1/marketplace/telemetry/compute` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| POST | `/internal/v1/marketplace/route/provider` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| POST | `/internal/v1/marketplace/compute/quote/sandbox-run` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| POST | `/internal/v1/marketplace/router/compute/select` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| POST | `/internal/v1/hydra/routing/score` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| POST | `/internal/v1/hydra/fx/rfq` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| POST | `/internal/v1/hydra/fx/quote` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| POST | `/internal/v1/hydra/fx/select` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| POST | `/internal/v1/hydra/fx/settle` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| GET | `/internal/v1/hydra/fx/rfq/:rfq_id` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| GET | `/internal/v1/hydra/risk/health` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| GET | `/internal/v1/hydra/observability` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| POST | `/internal/v1/marketplace/dispatch/sandbox-run` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| POST | `/internal/v1/verifications/sandbox-run` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| POST | `/internal/v1/verifications/repo-index` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| GET | `/internal/v1/treasury/compute/summary` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| POST | `/internal/v1/treasury/compute/reconcile` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| POST | `/internal/v1/treasury/compute/settle/sandbox-run` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| POST | `/internal/v1/liquidity/quote_pay` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| POST | `/internal/v1/credit/intent` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| POST | `/internal/v1/credit/offer` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| POST | `/internal/v1/credit/envelope` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| POST | `/internal/v1/credit/settle` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| GET | `/internal/v1/credit/health` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| GET | `/internal/v1/credit/agents/:agent_id/exposure` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| GET | `/internal/v1/liquidity/status` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| POST | `/internal/v1/liquidity/pay` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| POST | `/internal/v1/pools/:pool_id/admin/create` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| POST | `/internal/v1/pools/:pool_id/deposit_quote` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| POST | `/internal/v1/pools/:pool_id/deposits/:deposit_id/confirm` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| POST | `/internal/v1/pools/:pool_id/withdraw_request` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| GET | `/internal/v1/pools/:pool_id/status` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| GET | `/internal/v1/pools/:pool_id/snapshots/latest` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |
| GET | `/internal/v1/fraud/incidents` | `runtime_service` | `runtime_authority` | `runtime_authority_canonical` |

## Verification

1. Control-side ownership map tests:
   - `runtime_ownership_contract_is_unambiguous`
   - `runtime_ownership_contract_covers_known_runtime_routes`
   - `runtime_service_routes_are_runtime_proxy_delivery`
2. Runtime internal ownership tests:
   - `internal_route_ownership_contract_is_unambiguous`
   - `internal_route_ownership_contract_covers_runtime_router_contract`
3. Control status exposes `/api/runtime/*` ownership at:
   - `GET /api/v1/control/status` -> `data.runtimeRouteOwnership`
