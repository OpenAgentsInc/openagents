# Compatibility Negotiation Policy (v1)

Date: 2026-02-21
Status: Active
Owner: `owner:contracts-docs`

This document defines the canonical compatibility negotiation contract across:

1. control-service APIs,
2. sync websocket join/subscribe flows (Spacetime canonical; Khala legacy during migration),
3. retained-client support windows,
4. compatibility stream aliases (`/api/chat/stream`, `/api/chats/{conversationId}/stream`).

Authoritative decision record: `docs/adr/ADR-0005-compatibility-negotiation-and-support-window-policy.md`.

## Inputs

Every client handshake supplies:

- `client_build_id` (required, monotonic timestamp-like build ID)
- `protocol_version` (required)
- `schema_version` (required)

Server compatibility window per environment:

- `min_client_build_id` (required)
- `max_client_build_id` (optional)
- `min_schema_version` (required)
- `max_schema_version` (required)
- `protocol_version` (required exact match)

## Negotiation Matrix

| Check | Condition | Result code | Upgrade required |
|---|---|---|---|
| Build ID present | `client_build_id` missing/blank | `invalid_client_build` | `true` |
| Protocol match | `client.protocol_version != server.protocol_version` | `unsupported_protocol_version` | `true` |
| Schema range | `client.schema_version < min` or `> max` | `unsupported_schema_version` | `true` |
| Min build floor | `client_build_id < min_client_build_id` | `upgrade_required` | `true` |
| Max build ceiling (if set) | `client_build_id > max_client_build_id` | `unsupported_client_build` | `true` |
| All checks pass | within range | success | `false` |

## Deterministic Error Payload

All negotiation failures must return a machine-readable payload containing active support window metadata.

Required fields:

- `code`
- `message`
- `upgrade_required`
- `min_client_build_id`
- `max_client_build_id` (nullable)
- `min_schema_version`
- `max_schema_version`
- `protocol_version`

## Control API Contract

Failure response semantics:

- HTTP status: `426 Upgrade Required` (preferred) or explicit compatibility status override per route.
- JSON envelope carries required fields above.
- Client behavior: block command/read paths requiring compatibility and prompt upgrade/reload.

Compatibility stream aliases:

- `POST /api/chat/stream`
- `POST /api/chats/{conversationId}/stream`

These routes use the same compatibility headers and deterministic `426` rejection envelope as other control APIs before endpoint-specific auth/validation runs.

Steady-state operations for these compatibility aliases are governed by:

- `apps/openagents.com/service/docs/STREAM_COMPAT_STEADY_STATE_RUNBOOK.md`

## Sync WS Contract

Failure response semantics:

- Join/subscribe failure payload uses same `code` set and required support-window fields.
- If failure occurs after socket establishment, `sync:error` must carry deterministic compatibility code.
- Client behavior: stop reconnect loops and surface upgrade-required UX until compatibility window changes.

Protocol-version policy:

- Canonical sync protocol version: `spacetime.sync.v1`
- Legacy migration alias: `khala.ws.v1`
- Unsupported examples: `khala.ws.v2`, unknown custom protocol strings

Negotiation requirements:

1. Servers must advertise one canonical protocol per environment window.
2. During migration windows, servers may accept canonical/legacy alias pairs only when explicitly configured.
3. Failure responses for protocol mismatch must always use `unsupported_protocol_version` with full support-window metadata.

## Environment Support-Window Policy

- `dev`: broad window allowed; `max_client_build_id` usually unset.
- `staging`: explicit `min_client_build_id`, optional `max_client_build_id` for rollback rehearsal.
- `prod`: explicit `min_client_build_id`; `max_client_build_id` optional only for controlled freeze windows.

Window updates are operational changes and must follow release runbook ordering.

## Release Ordering

1. Publish control-service revision with updated compatibility windows.
2. Keep compatibility window broad enough for currently active clients.
3. Observe rollout and reconnect telemetry.
4. Tighten window only after client convergence.

Rollback:

1. Deploy prior compatible control-service revision.
2. Widen compatibility window.
3. Verify deterministic negotiation outcomes.

## Reference Implementation (Policy-Level)

Shared Rust negotiation helper:

- `crates/openagents-client-core/src/compatibility.rs`

Policy tests:

- `cargo test -p openagents-client-core compatibility::`

Fixture examples:

- `docs/protocol/fixtures/compatibility-negotiation-v1.json`
