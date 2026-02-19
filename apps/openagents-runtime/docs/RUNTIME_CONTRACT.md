# OpenAgents Runtime Internal Contract (`/internal/v1/*`)

This document defines the implemented internal control-plane contract between Laravel and `openagents-runtime`.

## Conventions

- Base path: `/internal/v1`
- Content type: `application/json`
- Streaming content type: `text/event-stream`
- Authentication header: `X-OA-RUNTIME-SIGNATURE` (required on all `/internal/v1/*` routes)
- Trace headers: `traceparent`, `tracestate`, `x-request-id`

## Entity IDs

- `run_id`: stable identifier of an execution run
- `thread_id`: logical conversation/session identifier
- `frame_id`: idempotency key for frame ingestion
- `seq`: monotonic run-local stream sequence (integer > 0)

## Error Envelope

Non-SSE endpoints return JSON errors:

```json
{
  "error": {
    "code": "invalid_request",
    "message": "thread_id is required",
    "details": ["optional machine-readable validation details"]
  }
}
```

Standard error codes:

- `unauthorized`
- `forbidden`
- `invalid_request`
- `not_found`
- `conflict`
- `stale_cursor`
- `internal_error`

## Cursor Semantics

`GET /internal/v1/runs/{run_id}/stream` supports cursor resume via either:

- `Last-Event-ID: <seq>`
- query parameter `?cursor=<seq>`

Resolution rules:

1. If both are present and differ, return `400 invalid_request`.
2. If both are absent, stream starts at current latest durable seq and waits for tail events.
3. If cursor points to retained history, stream resumes from `seq + 1`.
4. If cursor is older than retention floor, return `410 stale_cursor`.

SSE event `id:` always equals the run event `seq`.

## Stream Tail Window (`tail_ms`)

`GET /internal/v1/runs/{run_id}/stream` accepts optional `tail_ms` query parameter.

- `tail_ms` must be a positive integer.
- Runtime keeps the stream tail open for up to `tail_ms` waiting for wakeups/events.
- Invalid `tail_ms` returns `400 invalid_request`.

## Idempotency (`frame_id`)

`POST /internal/v1/runs/{run_id}/frames` is idempotent by `frame_id` per run.

- First request persists the frame and returns `202 accepted`.
- Duplicate request with same `frame_id` and equivalent payload/type returns `200 accepted` with `idempotentReplay=true`.
- Payload/type mismatch for existing `frame_id` returns `409 conflict`.

## Cancellation Semantics

`POST /internal/v1/runs/{run_id}/cancel` appends a durable `run.cancel_requested` event and is idempotent.

- First cancel request returns `202` with `idempotentReplay=false`.
- Duplicate cancel request returns `200` with `idempotentReplay=true`.
- Runtime stops starting new work after cancel is observed.
- Best-effort cancel is issued for in-flight tool work.
- Late model/tool results cannot reopen a terminal canceled run.

## Implemented endpoints

### `GET /internal/v1/health`

Health probe endpoint.

Success (`200`):

```json
{
  "status": "ok",
  "service": "openagents-runtime",
  "version": "0.1.0"
}
```

### `POST /internal/v1/comms/delivery-events`

Ingests normalized provider webhook delivery events into runtime canonical storage.

Required fields:

- `event_id`
- `provider`
- `delivery_state`
- `payload`

Request:

```json
{
  "event_id": "resend_evt_123",
  "provider": "resend",
  "delivery_state": "delivered",
  "message_id": "email_abc",
  "integration_id": "resend.primary",
  "recipient": "user@example.com",
  "occurred_at": "2026-02-19T18:00:00Z",
  "reason": null,
  "payload": {
    "rawType": "email.delivered"
  }
}
```

### `POST /internal/v1/tools/execute`

Dispatches runtime tool-pack operations through a single internal endpoint. `coding.v1` is implemented.

Required fields:

- `tool_pack`
- `request`

Optional fields:

- `manifest` (direct integration manifest object)
- `manifest_ref` (registry lookup reference with `tool_id` or `integration_id`)
- `mode` (`execute` default, `replay`)
- `policy` (authorization/budget/write approval context)
- `run_id`
- `thread_id`
- `user_id`

Required principal header:

- `x-oa-user-id`

Request:

```json
{
  "tool_pack": "coding.v1",
  "mode": "execute",
  "run_id": "run_123",
  "thread_id": "thread_abc",
  "manifest_ref": {
    "integration_id": "github.primary"
  },
  "request": {
    "integration_id": "github.primary",
    "operation": "get_issue",
    "repository": "OpenAgentsInc/openagents",
    "issue_number": 1747,
    "run_id": "run_123",
    "tool_call_id": "tool_call_001"
  },
  "policy": {
    "authorization_id": "auth_abc",
    "authorization_mode": "delegated_budget",
    "budget": {
      "max_total_sats": 5000
    }
  }
}
```

`manifest_ref` resolution reads from the runtime skill/tool registry:

- DB-published ToolSpec records
- built-in catalog entries (`github.primary`, `resend.primary`)

If both `manifest` and `manifest_ref` are provided, explicit `manifest` takes precedence.

Success (`200`):

```json
{
  "data": {
    "state": "succeeded",
    "decision": "allowed",
    "reason_code": "policy_allowed.default",
    "receipt": {
      "receipt_id": "coding_abc123",
      "replay_hash": "..."
    }
  }
}
```

Validation failure (`422`):

```json
{
  "error": {
    "code": "invalid_request",
    "message": "tool invocation validation failed",
    "details": ["machine-readable error details"]
  }
}
```

Accepted (`202`) for first ingest:

```json
{
  "eventId": "resend_evt_123",
  "status": "accepted",
  "idempotentReplay": false
}
```

Idempotent replay (`200`) for duplicate payload:

```json
{
  "eventId": "resend_evt_123",
  "status": "accepted",
  "idempotentReplay": true
}
```

Conflict (`409`) when `event_id` already exists with a different payload:

```json
{
  "error": {
    "code": "conflict",
    "message": "event_id payload mismatch for existing webhook event"
  }
}
```

### `GET /internal/v1/runs/{run_id}/snapshot`

Returns latest run snapshot for a run/thread principal.

Required query params:

- `thread_id`

Required principal header (at least one):

- `x-oa-user-id`
- `x-oa-guest-scope`

Success (`200`):

```json
{
  "runId": "run_123",
  "threadId": "thread_abc",
  "status": "unknown",
  "latestSeq": 12,
  "updatedAt": "2026-02-18T12:30:00Z"
}
```

### `GET /internal/v1/skills/tool-specs`

Lists ToolSpec definitions from registry storage plus built-ins.

Success (`200`):

```json
{
  "data": [
    {
      "tool_id": "github.primary",
      "version": 1,
      "tool_pack": "coding.v1",
      "state": "published"
    }
  ]
}
```

### `POST /internal/v1/skills/tool-specs`

Upserts a ToolSpec JSON definition.

Required principal header:

- `x-oa-user-id`

Request:

```json
{
  "state": "validated",
  "tool_spec": {
    "tool_id": "github.custom",
    "version": 1,
    "tool_pack": "coding.v1",
    "name": "GitHub Custom",
    "description": "Custom coding skill tool",
    "execution_kind": "http",
    "integration_manifest": {
      "manifest_version": "coding.integration.v1",
      "integration_id": "github.custom",
      "provider": "github",
      "status": "active",
      "tool_pack": "coding.v1",
      "capabilities": ["get_issue", "get_pull_request"],
      "secrets_ref": {"provider": "laravel", "key_id": "intsec_github_custom"},
      "policy": {"write_operations_mode": "enforce", "max_requests_per_minute": 120}
    },
    "commercial": {"pricing_model": "free", "currency": "BTC_SATS"}
  }
}
```

### `GET /internal/v1/skills/skill-specs`

Lists SkillSpec definitions from registry storage plus built-ins.

### `POST /internal/v1/skills/skill-specs`

Upserts a SkillSpec JSON definition.

Required principal header:

- `x-oa-user-id`

Request:

```json
{
  "state": "validated",
  "skill_spec": {
    "skill_id": "github-coding-custom",
    "version": 1,
    "name": "GitHub Coding Custom",
    "description": "Custom coding workflow skill",
    "instructions_markdown": "Use coding tools for issue workflows.",
    "allowed_tools": [{"tool_id": "github.custom", "version": 1}],
    "compatibility": {"runtime": "openagents-runtime"},
    "commercial": {"pricing_model": "free", "currency": "BTC_SATS"}
  }
}
```

### `POST /internal/v1/skills/skill-specs/{skill_id}/{version}/publish`

Compiles SkillSpec into an immutable Agent Skills bundle artifact and publishes SkillRelease.

Required principal header:

- `x-oa-user-id`

Success (`201`):

```json
{
  "data": {
    "release_id": "skillrel_abc123",
    "skill_id": "github-coding-custom",
    "version": 1,
    "bundle_hash": "f6d7...",
    "published_at": "2026-02-19T23:11:00Z"
  }
}
```

### `GET /internal/v1/skills/releases/{skill_id}/{version}`

Fetches immutable SkillRelease artifact including compiled bundle and compatibility report.

### `GET /internal/v1/codex/workers`

Lists principal-owned Codex workers for admin/observability surfaces.

Required principal header (at least one):

- `x-oa-user-id`
- `x-oa-guest-scope`

Optional query params:

- `status` (`starting|running|stopping|stopped|failed`)
- `workspace_ref` (exact match filter)
- `limit` (`1..200`, default `50`)

Success (`200`):

```json
{
  "data": [
    {
      "worker_id": "codexw_12345",
      "status": "running",
      "latest_seq": 17,
      "workspace_ref": "workspace://demo",
      "convex_projection": {
        "status": "in_sync",
        "lag_events": 0,
        "last_runtime_seq": 17,
        "last_projected_at": "2026-02-19T22:11:00Z"
      }
    }
  ]
}
```

### `POST /internal/v1/codex/workers`

Creates or reattaches a remote Codex worker session for the authenticated principal.

Current implementation note (2026-02-19): `in_memory` is the only implemented adapter. The desktop-backed runtime sync path is tracked in `docs/codex/unified-runtime-desktop-plan.md`.

Required principal header (at least one):

- `x-oa-user-id`
- `x-oa-guest-scope`

Request (optional):

```json
{
  "worker_id": "codexw_12345",
  "workspace_ref": "gs://tenant-workspaces/u_42/project-a",
  "codex_home_ref": "gs://tenant-codex-home/u_42",
  "adapter": "in_memory",
  "metadata": {"tenant_id": "u_42"}
}
```

### `GET /internal/v1/codex/workers/{worker_id}/snapshot`

Returns durable worker status and latest stream sequence.

### `POST /internal/v1/codex/workers/{worker_id}/requests`

Submits a JSON-RPC style request envelope to the worker process.

Request:

```json
{
  "request": {
    "request_id": "req_001",
    "method": "thread/start",
    "params": {"prompt": "Audit this PR"}
  }
}
```

### `GET /internal/v1/codex/workers/{worker_id}/stream`

Streams worker event log as SSE with the same cursor semantics as run streams.

- Supports `cursor`, `Last-Event-ID`, and `tail_ms`.
- SSE `id:` field equals worker event `seq`.

### `POST /internal/v1/codex/workers/{worker_id}/stop`

Requests graceful worker shutdown and appends a durable `worker.stopped` event.

## Convex Projection Boundary

Convex (self-hosted or cloud) may be used as a reactive sync layer for
web/mobile/desktop, but it is not part of runtime source-of-truth semantics.

Normative rules:

1. Runtime/Postgres remains authoritative for run/codex events and policy/spend state.
2. Convex stores projection/read-model data only.
3. Runtime is the single writer for Convex projection docs.
4. Drift repair is replay from runtime event history, not mutation in Convex.

See `docs/CONVEX_SYNC.md` and
`docs/plans/active/convex-self-hosting-runtime-sync-plan.md`.

### `POST /internal/v1/runs/{run_id}/frames`

Append an input frame to a run.

Required fields:

- `thread_id`
- `frame_id`
- `type`
- `payload`

Request:

```json
{
  "thread_id": "thread_abc",
  "frame_id": "frm_001",
  "type": "user_message",
  "payload": {
    "text": "hi"
  }
}
```

Success (`202`):

```json
{
  "runId": "run_123",
  "frameId": "frm_001",
  "status": "accepted",
  "idempotentReplay": false
}
```

Duplicate success (`200`):

```json
{
  "runId": "run_123",
  "frameId": "frm_001",
  "status": "accepted",
  "idempotentReplay": true
}
```

### `GET /internal/v1/runs/{run_id}/stream`

Location-independent stream from durable run events.

Required query params:

- `thread_id`

Optional query params:

- `cursor`
- `tail_ms`

SSE output example:

```text
event: message
id: 42
data: {"type":"text-delta","delta":"Hello","runId":"run_123"}

```

Terminal flows can emit `data: [DONE]`.

### `POST /internal/v1/runs/{run_id}/cancel`

Request durable cancellation for a run.

Required fields:

- `thread_id`

Request:

```json
{
  "thread_id": "thread_abc",
  "reason": "user requested stop"
}
```

Accepted (`202`) for first cancel:

```json
{
  "runId": "run_123",
  "status": "canceling",
  "cancelRequested": true,
  "idempotentReplay": false
}
```

Idempotent replay (`200`) for duplicate cancel:

```json
{
  "runId": "run_123",
  "status": "canceling",
  "cancelRequested": true,
  "idempotentReplay": true
}
```

## Ownership and Principal Enforcement

Runtime validates run/thread ownership from DB records. Runtime does not trust request payload `user_id` claims.

## Tracing Contract

Runtime accepts and propagates:

- `traceparent`
- `tracestate`
- `x-request-id`

Standard runtime span names:

- `runtime.ingest`
- `runtime.infer`
- `runtime.tool`
- `runtime.persist`
- `runtime.stream`

## Contract change log

### 2026-02-18

- Clarified that signed internal auth is mandatory on all `/internal/v1/*` routes.
- Aligned request field names to implemented snake_case params (`thread_id`, `frame_id`).
- Added implemented stream `tail_ms` behavior.
- Removed unimplemented `/runs` start and `/runs/{run_id}/cancel` operations from the active contract.

### 2026-02-19

- Added implemented `POST /internal/v1/runs/{run_id}/cancel` with durable and idempotent cancel semantics.
- Clarified runtime cancel behavior for stop-new-work, best-effort in-flight tool cancellation, and late-result handling.
- Added implemented `POST /internal/v1/tools/execute` for internal tool-pack dispatch (`coding.v1` first).

## Backward Compatibility Rule

Contract changes must be additive or versioned. Breaking changes to `/internal/v1/*` require explicit migration notes and Laravel adapter updates.
