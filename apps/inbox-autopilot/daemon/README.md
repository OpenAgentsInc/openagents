# Inbox Autopilot Daemon

Local Rust daemon for Inbox Autopilot.

Shared inbox mailbox policy/draft/audit domain logic is imported from:

- `crates/autopilot-inbox-domain`

## Run

```bash
cd daemon
cargo run
```

Default bind: `127.0.0.1:8787`

## Required env for Gmail OAuth

```bash
export GOOGLE_OAUTH_CLIENT_ID=...
export GOOGLE_OAUTH_CLIENT_SECRET=...
```

Optional:

- `INBOX_AUTOPILOT_BIND_ADDR` (default `127.0.0.1:8787`)
- `INBOX_AUTOPILOT_DATA_DIR` (default `~/.inbox-autopilot`)
- `OPENAI_BASE_URL` (default `https://api.openai.com/v1`)
- `OPENAI_MODEL` (default `gpt-4o-mini`)

## Docs

- IPC contract: `docs/ipc-contract.md`
- DB schema: `docs/db-schema.md`

## Draft quality evaluation

The daemon exposes a quality scoring endpoint used by the app Audit screen:

- `GET /quality/draft-edit-rate?limit_per_category=<optional>&threshold=<optional>`

It evaluates scheduling/report-delivery threads by comparing generated drafts against sent replies and reports minimal-edit rate vs the MVP target (`60%`).
