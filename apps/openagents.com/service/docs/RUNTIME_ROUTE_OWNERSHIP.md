# Runtime Route Ownership

Status: active  
Last updated: 2026-02-25

This document is the control-service runtime route ownership map used for boundary hardening.

Authoritative code contract:

- `apps/openagents.com/service/src/runtime_ownership.rs`

Policy:

1. Each `/api/runtime/*` method/path pair has exactly one canonical owner.
2. `control_service` owns codex worker/session + skill/tool registry lanes currently implemented in control.
3. `runtime_service` owns runtime worker authority lanes and is reached via internal runtime proxy client.
4. If runtime service is not configured, runtime-owned routes fail with `503 service_unavailable`.

## Ownership Table

| Method | Path | Owner |
|---|---|---|
| POST | `/api/runtime/tools/execute` | `control_service` |
| GET | `/api/runtime/skills/tool-specs` | `control_service` |
| POST | `/api/runtime/skills/tool-specs` | `control_service` |
| GET | `/api/runtime/skills/skill-specs` | `control_service` |
| POST | `/api/runtime/skills/skill-specs` | `control_service` |
| POST | `/api/runtime/skills/skill-specs/:skill_id/:version/publish` | `control_service` |
| GET | `/api/runtime/skills/releases/:skill_id/:version` | `control_service` |
| GET | `/api/runtime/threads` | `control_service` |
| GET | `/api/runtime/threads/:thread_id/messages` | `control_service` |
| POST | `/api/runtime/threads/:thread_id/messages` | `control_service` |
| GET | `/api/runtime/codex/workers` | `control_service` |
| POST | `/api/runtime/codex/workers` | `control_service` |
| GET | `/api/runtime/codex/workers/:worker_id` | `control_service` |
| GET | `/api/runtime/codex/workers/:worker_id/stream` | `control_service` |
| POST | `/api/runtime/codex/workers/:worker_id/events` | `control_service` |
| POST | `/api/runtime/codex/workers/:worker_id/stop` | `control_service` |
| POST | `/api/runtime/codex/workers/:worker_id/requests` | `control_service` |
| GET | `/api/runtime/workers` | `runtime_service` |
| POST | `/api/runtime/workers` | `runtime_service` |
| GET | `/api/runtime/workers/:worker_id` | `runtime_service` |
| POST | `/api/runtime/workers/:worker_id/heartbeat` | `runtime_service` |
| POST | `/api/runtime/workers/:worker_id/status` | `runtime_service` |

## Verification

1. Unit tests validate ownership map uniqueness and coverage:
   - `runtime_ownership_contract_is_unambiguous`
   - `runtime_ownership_contract_covers_known_runtime_routes`
2. Control status exposes the ownership map at:
   - `GET /api/v1/control/status` -> `data.runtimeRouteOwnership`
