# OpenAgents Runtime Internal Contract (`/internal/v1/*`)

This document defines the internal control-plane contract between Laravel and `openagents-runtime`.

## Conventions

- Base path: `/internal/v1`
- Content type: `application/json`
- Streaming content type: `text/event-stream`
- Authentication header: `X-OA-RUNTIME-SIGNATURE`
- Trace headers: `traceparent`, `tracestate`, `x-request-id`

## Entity IDs

- `runId`: stable identifier of an execution run
- `threadId`: logical conversation/session identifier
- `frameId`: idempotency key for frame ingestion
- `seq`: monotonic run-local stream sequence (integer > 0)

## Error Envelope

Non-SSE endpoints return JSON errors:

```json
{
  "error": {
    "code": "invalid_request",
    "message": "frameId is required",
    "details": {
      "field": "frameId"
    }
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

`GET /internal/v1/runs/{runId}/stream` supports cursor resume via either:

- `Last-Event-ID: <seq>`
- query parameter `?cursor=<seq>`

Resolution rules:

1. If both are present and differ, return `400 invalid_request`.
2. If both are absent, stream starts from latest position + new events.
3. If cursor points to retained history, stream resumes from `seq + 1`.
4. If cursor is older than retention floor, return `410 stale_cursor`.

SSE event `id:` always equals the run event `seq`.

## Idempotency (`frameId`)

`POST /internal/v1/runs/{runId}/frames` is idempotent by `frameId` per run.

- First request persists the frame and appends corresponding run event(s).
- Duplicate requests with the same `frameId` return success with canonical persisted result.
- Payload mismatches for an existing `frameId` return `409 conflict`.

## Endpoints

### `POST /internal/v1/runs`

Create or start a run execution.

Request:

```json
{
  "runId": "run_123",
  "threadId": "thread_abc",
  "authorizationRef": {
    "autopilotId": "ap_9",
    "mode": "delegated_budget"
  },
  "metadata": {
    "source": "chat"
  }
}
```

Success (`202`):

```json
{
  "runId": "run_123",
  "status": "accepted"
}
```

### `POST /internal/v1/runs/{runId}/frames`

Append an input frame to a run.

Request:

```json
{
  "frameId": "frm_001",
  "type": "user_message",
  "payload": {
    "text": "hi"
  },
  "occurredAt": "2026-02-18T12:00:00Z"
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

### `GET /internal/v1/runs/{runId}/stream`

Location-independent stream from durable run events.

SSE output example:

```text
event: run.event
id: 42
data: {"type":"text-delta","delta":"Hello"}

```

Terminal output includes mapped finish events and `[DONE]`.

### `POST /internal/v1/runs/{runId}/cancel`

Request cancellation for a run.

Request:

```json
{
  "reason": "user_requested"
}
```

Success (`202`):

```json
{
  "runId": "run_123",
  "status": "cancel_requested"
}
```

### `GET /internal/v1/runs/{runId}/snapshot`

Returns latest durable run snapshot for control-plane inspection.

Success (`200`):

```json
{
  "runId": "run_123",
  "threadId": "thread_abc",
  "status": "running",
  "latestSeq": 128,
  "updatedAt": "2026-02-18T12:30:00Z"
}
```

## Ownership and Principal Enforcement

Runtime must validate run/thread ownership from DB records. Runtime does not trust request payload `userId` claims.

## Backward Compatibility Rule

All contract changes must be additive or versioned under a new path. Breaking changes to `/internal/v1/*` require explicit migration plan.
