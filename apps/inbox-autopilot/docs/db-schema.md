# Inbox Autopilot Local DB Schema (SQLite)

Path defaults to `~/.inbox-autopilot/daemon.sqlite`.

## Tables

### `settings`

- `key TEXT PRIMARY KEY`
- `value TEXT NOT NULL`

Used for runtime/config settings (`privacy_mode`, `backfill_days`, `attachment_storage_mode`, `sync_interval_seconds`, templates, etc.).

### `oauth_tokens`

- `provider TEXT PRIMARY KEY` (`gmail`, `chatgpt`)
- `access_token TEXT` (encrypted)
- `refresh_token TEXT` (encrypted)
- `expires_at INTEGER`
- `scope TEXT`
- `token_type TEXT`
- `updated_at INTEGER NOT NULL`

### `threads`

- `id TEXT PRIMARY KEY`
- `gmail_thread_id TEXT NOT NULL UNIQUE`
- `subject TEXT NOT NULL`
- `snippet TEXT NOT NULL`
- `from_address TEXT NOT NULL`
- `category TEXT`
- `risk TEXT`
- `policy TEXT`
- `reason_codes TEXT` (JSON array)
- `similar_thread_ids TEXT` (JSON array)
- `external_model_used INTEGER NOT NULL DEFAULT 0`
- `last_message_at INTEGER NOT NULL`
- `updated_at INTEGER NOT NULL`

### `messages`

- `id TEXT PRIMARY KEY`
- `thread_id TEXT NOT NULL` (FK -> `threads.id`)
- `gmail_message_id TEXT NOT NULL UNIQUE`
- `sender TEXT NOT NULL`
- `recipient TEXT NOT NULL`
- `subject TEXT NOT NULL`
- `snippet TEXT NOT NULL`
- `body TEXT NOT NULL` (AES-GCM encrypted at rest)
- `inbound INTEGER NOT NULL`
- `sent_at INTEGER NOT NULL`

### `drafts`

- `id TEXT PRIMARY KEY`
- `thread_id TEXT NOT NULL` (FK -> `threads.id`)
- `body TEXT NOT NULL` (AES-GCM encrypted at rest)
- `status TEXT NOT NULL` (`pending`, `approved`, `rejected`, `needs_human`, `sent`)
- `source_summary TEXT NOT NULL`
- `model_used TEXT`
- `gmail_message_id TEXT`
- `created_at INTEGER NOT NULL`
- `updated_at INTEGER NOT NULL`

### `events`

- `id TEXT PRIMARY KEY`
- `thread_id TEXT` (nullable FK -> `threads.id`)
- `event_type TEXT NOT NULL`
- `payload_json TEXT NOT NULL`
- `created_at INTEGER NOT NULL`

Append-only event log for audit timeline and export.

## Indexes

- `idx_threads_last_message_at ON threads(last_message_at DESC)`
- `idx_messages_thread_sent ON messages(thread_id, sent_at DESC)`
- `idx_drafts_status ON drafts(status, updated_at DESC)`
- `idx_events_thread ON events(thread_id, created_at DESC)`

## Encryption/Security

- OAuth/chatgpt secrets are encrypted before storage using AES-256-GCM.
- Message and draft bodies are encrypted with the same vault key before persistence.
- Master key is generated at first run and stored locally at `~/.inbox-autopilot/master.key`.
- App session token is stored in macOS Keychain.
