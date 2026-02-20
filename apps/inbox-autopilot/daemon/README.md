# Inbox Autopilot Daemon

Local Rust daemon for Inbox Autopilot.

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
