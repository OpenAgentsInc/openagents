# Inbox Autopilot IPC Contract (App <-> Daemon)

This document is the source of truth for local IPC between the macOS app and the local Rust daemon.

## Transport

- Base URL (default): `http://127.0.0.1:8787`
- Local-only: daemon binds loopback only.
- Content type: JSON unless noted.
- Streaming: Server-Sent Events (SSE) via `GET /events/stream`.

## App/Daemon Session Auth

### Session bootstrap

1. App calls `POST /session` with optional client name.
2. Daemon returns a short-lived `session_token`.
3. App stores this token in Keychain.

### Request authentication

All authenticated endpoints require:

- Header `x-session-token: <session_token>`
- Header `x-nonce: <unique_nonce_per_request>`

Validation rules:

- Session token must exist and be unexpired.
- Nonce is one-time-use per session (replay protection).

If token is expired/invalid, daemon responds `401`; app should call `POST /session` and retry.

## Endpoints

### Health/session

- `GET /health`
- `POST /session`

### Auth

- `GET /auth/gmail/url?redirect_uri=<uri>&code_challenge=<optional>`
- `POST /auth/gmail`
- `GET /auth/gmail/status`
- `POST /auth/chatgpt`
- `GET /auth/chatgpt/status`

### Sync

- `POST /sync/backfill`
- `POST /sync/now`

### Threads/drafts

- `GET /threads?search=<optional>&limit=<optional>`
- `GET /threads/:id`
- `POST /threads/:id/generate-draft`
- `GET /threads/:id/draft`
- `POST /threads/:id/approve-send`
- `GET /threads/:id/audit`
- `GET /threads/:id/events`
- `POST /threads/:id/export-audit`

- `GET /drafts?status=pending&limit=<optional>`
- `POST /drafts/:id/needs-human`

### Events/settings

- `GET /events?thread_id=<optional>&limit=<optional>`
- `GET /events/stream` (SSE)
- `GET /templates/mine?limit=<optional>`
- `GET /quality/draft-edit-rate?limit_per_category=<optional>&threshold=<optional>`
- `GET /settings`
- `PUT /settings`
- `POST /settings/delete-corpus`
- `POST /settings/factory-reset`

## Core Payloads

### `POST /session`

Request:

```json
{ "client_name": "Inbox Autopilot macOS" }
```

Response:

```json
{ "session_token": "...", "expires_at": "2026-02-19T20:00:00Z" }
```

### `POST /auth/gmail`

Request:

```json
{
  "code": "<oauth_code>",
  "redirect_uri": "inboxautopilot://oauth/callback",
  "code_verifier": null
}
```

Response: `204 No Content`

### `POST /auth/chatgpt`

Request:

```json
{ "api_key": "sk-..." }
```

Response: `204 No Content`

### `POST /sync/backfill`

Request:

```json
{ "days": 90 }
```

Response:

```json
{ "imported_threads": 42, "imported_messages": 187 }
```

### `GET /threads`

Response:

```json
{
  "threads": [
    {
      "id": "...",
      "subject": "...",
      "snippet": "...",
      "from_address": "...",
      "category": "scheduling",
      "risk": "low",
      "policy": "send_with_approval",
      "last_message_at": "2026-02-19T19:00:00Z",
      "has_pending_draft": true
    }
  ]
}
```

### `GET /threads/:id`

Response:

```json
{
  "thread": { "id": "...", "subject": "..." },
  "messages": [
    {
      "id": "...",
      "thread_id": "...",
      "sender": "...",
      "recipient": "...",
      "body": "...",
      "snippet": "...",
      "inbound": true,
      "sent_at": "2026-02-19T19:00:00Z"
    }
  ],
  "draft": {
    "id": "...",
    "thread_id": "...",
    "body": "...",
    "status": "pending",
    "source_summary": "...",
    "model_used": "gpt-4o-mini",
    "created_at": "2026-02-19T19:01:00Z",
    "updated_at": "2026-02-19T19:01:00Z"
  }
}
```

### `POST /threads/:id/approve-send`

Response:

```json
{ "draft_id": "...", "gmail_message_id": "..." }
```

### `GET /templates/mine?limit=12`

Response:

```json
{
  "suggestions": [
    {
      "id": "...",
      "category": "scheduling",
      "template_text": "Thanks for reaching out ...",
      "occurrences": 4
    }
  ]
}
```

### `GET /quality/draft-edit-rate?limit_per_category=200&threshold=0.35`

Response:

```json
{
  "generated_at": "2026-02-20T22:00:00Z",
  "threshold": 0.35,
  "target_rate": 0.6,
  "total_samples": 24,
  "total_minimal_edit": 15,
  "total_minimal_edit_rate": 0.625,
  "target_met": true,
  "categories": [
    {
      "category": "scheduling",
      "samples": 12,
      "minimal_edit_count": 8,
      "minimal_edit_rate": 0.6667,
      "average_edit_ratio": 0.31
    }
  ],
  "samples": [
    {
      "thread_id": "...",
      "category": "scheduling",
      "edit_ratio": 0.24,
      "minimal_edit": true,
      "draft_word_count": 56,
      "sent_word_count": 59
    }
  ]
}
```

### `GET /settings`

Response:

```json
{
  "privacy_mode": "hybrid",
  "backfill_days": 90,
  "allowed_recipient_domains": ["example.com"],
  "attachment_storage_mode": "metadata",
  "signature": "Thanks,\nChris",
  "template_scheduling": "...",
  "template_report_delivery": "...",
  "sync_interval_seconds": 60
}
```

### `PUT /settings`

Request:

```json
{
  "privacy_mode": "local_only",
  "backfill_days": 90,
  "allowed_recipient_domains": [],
  "attachment_storage_mode": "metadata",
  "signature": null,
  "template_scheduling": null,
  "template_report_delivery": null,
  "sync_interval_seconds": 60
}
```

Response: same shape as `GET /settings`.

## SSE Event Stream

Endpoint: `GET /events/stream`

- Content type: `text/event-stream`
- Each event `data:` payload is JSON `EventRecord`.

Event types currently emitted:

- `daemon_started`
- `auth_gmail_connected`
- `auth_chatgpt_connected`
- `sync_backfill_completed`
- `sync_incremental_completed`
- `classification_completed`
- `policy_evaluated`
- `draft_created`
- `draft_marked_needs_human`
- `approval_granted`
- `email_sent`
- `settings_updated`
- `sync_incremental_failed`
- `local_corpus_deleted`
- `factory_reset_completed`
- `draft_quality_evaluated`

## Error format

Non-2xx responses use:

```json
{ "error": "human-readable message" }
```
