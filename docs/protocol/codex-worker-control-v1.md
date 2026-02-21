# Codex Worker Control Contract v1

Status: Frozen v1 (mobile->desktop control lane)  
Date: 2026-02-21

This document defines the request/receipt contract for controlling a desktop Codex session via runtime workers.

Related:
- `docs/protocol/codex-worker-events-v1.md`
- `apps/runtime/docs/RUNTIME_CONTRACT.md`
- `proto/openagents/codex/v1/workers.proto`
- `proto/openagents/codex/v1/events.proto`

## Scope

This contract covers:
1. Control request envelope (`/requests`) from clients.
2. Success/error receipt envelopes emitted as worker events.
3. Method allowlist for remote desktop control.
4. Idempotency and reconnect/replay semantics.

## Endpoints

Public control-plane path:
- `POST /api/runtime/codex/workers/{workerId}/requests`

Runtime internal path (proxied by control plane):
- `POST /internal/v1/codex/workers/{worker_id}/requests`

Receipts are consumed from:
- `GET /api/runtime/codex/workers/{workerId}/stream`
- `GET /api/runtime/codex/workers/{workerId}` (latest sequence / projection status)

## Request Envelope

`request` object fields:
1. `request_id` (required string, client-generated idempotency key per worker)
2. `method` (required string, must be allowlisted)
3. `params` (optional object, method-specific payload)
4. `request_version` (optional string, default `v1`)
5. `sent_at` (optional RFC3339 timestamp)
6. `source` (optional string, recommended `autopilot-ios`)

Example:

```json
{
  "request": {
    "request_id": "iosreq_8f3d0b",
    "method": "turn/start",
    "params": {
      "thread_id": "thread_123",
      "input": [{"type": "text", "text": "Continue from last step"}],
      "model": "gpt-5-codex",
      "effort": "medium"
    },
    "request_version": "v1",
    "sent_at": "2026-02-21T23:50:00Z",
    "source": "autopilot-ios"
  }
}
```

## Allowlisted Methods (v1)

Minimum required allowlist:
1. `thread/start`
2. `thread/resume`
3. `turn/start`
4. `turn/interrupt`
5. `thread/list`
6. `thread/read`

Rules:
1. Non-allowlisted methods MUST be rejected with an error receipt.
2. Each method MUST validate required params before execution.
3. Missing/invalid required params MUST produce `invalid_request` error code.

## Worker Targeting Semantics (v1)

Target resolution follows:
1. `worker_id` selects the worker lane.
2. `session_id` (if provided in request params/metadata) selects desktop app-session inside that worker.
3. `thread_id` (method-specific) selects Codex thread inside the resolved session.

Resolution priority:
1. Explicit request target (`session_id`, `thread_id`) wins.
2. Worker metadata mapping (`metadata.session_id`, `metadata.thread_id`) is next.
3. For `:shared` workers, use runtime-maintained active session/thread mapping if present.

Method requirements:
1. `thread/start`: session target required; thread id is created by server.
2. `thread/resume`: session target required, resume thread id required.
3. `turn/start`: session + thread target required.
4. `turn/interrupt`: session + thread target required; active turn id required by app-server contract.
5. `thread/list`: session target required; thread id not required.
6. `thread/read`: session + thread target required.

Stale/missing target behavior:
1. Missing required session mapping -> `worker_unavailable`.
2. Missing required thread mapping for method -> `invalid_request`.
3. Thread/session mapping mismatch or stale attachment -> `conflict`.
4. Responses SHOULD include `details.current_session_id` and/or `details.current_thread_id` when known.

Backward-compatible `:shared` worker fallback:
1. If exactly one active desktop session mapping exists, route there.
2. If zero active mappings exist, return `worker_unavailable`.
3. If multiple active mappings exist and request does not specify `session_id`, return `conflict` (ambiguous target).
4. Existing `ios/user_message` lane remains supported and routes via the current active mapping.

## Receipt Envelopes

A control request has exactly one terminal receipt keyed by `(worker_id, request_id)`:
1. Success receipt: `worker.response`
2. Error receipt: `worker.error`

### Success Receipt (`worker.response`)

Required payload fields:
1. `request_id`
2. `method`
3. `ok` (true)
4. `response` (object or scalar payload)
5. `occurred_at` (RFC3339)

Example:

```json
{
  "event_type": "worker.response",
  "payload": {
    "request_id": "iosreq_8f3d0b",
    "method": "turn/start",
    "ok": true,
    "response": {
      "turn": {"id": "turn_987"}
    },
    "occurred_at": "2026-02-21T23:50:01Z"
  }
}
```

### Error Receipt (`worker.error`)

Required payload fields:
1. `request_id`
2. `method`
3. `code`
4. `message`
5. `occurred_at` (RFC3339)

Optional fields:
1. `retryable` (bool)
2. `details` (object)

Example:

```json
{
  "event_type": "worker.error",
  "payload": {
    "request_id": "iosreq_8f3d0b",
    "method": "turn/start",
    "code": "invalid_request",
    "message": "thread_id is required",
    "retryable": false,
    "occurred_at": "2026-02-21T23:50:01Z"
  }
}
```

## Error Taxonomy (v1)

Standard codes:
1. `unauthorized`
2. `forbidden`
3. `invalid_request`
4. `unsupported_method`
5. `conflict`
6. `worker_unavailable`
7. `timeout`
8. `internal_error`

Guidance:
1. `conflict` for stopped/stale target mutation conflicts.
2. `worker_unavailable` when desktop session mapping cannot execute request.
3. `unsupported_method` for non-allowlisted methods.

## Idempotency and Replay Semantics

### Idempotency Key

The idempotency key is `(worker_id, request_id)`.

Requirements:
1. Duplicate requests with the same key MUST NOT execute side effects more than once.
2. A duplicate request MUST resolve to the same terminal receipt outcome.
3. Client retries MUST reuse the original `request_id`.

### Sequencing

1. Worker stream sequence (`seq`) is monotonic per worker.
2. Receipts are replayable by sequence cursor.
3. Consumers apply events idempotently by `(worker_id, seq)`.

### Reconnect/Resume

1. Clients persist the last applied `seq` watermark.
2. On reconnect, clients request events after that watermark.
3. If a stale cursor is returned, clients must reset to server-provided `resume_after` and continue.
4. Receipt reconciliation is request-id based; duplicate replayed terminal receipts are ignored.

## Compatibility

1. v1 is additive-only.
2. Existing field meanings cannot be repurposed.
3. New methods require allowlist extension and compatibility note before use.
