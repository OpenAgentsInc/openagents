# Nexus Operator Hardening and Backup/Restore Validation

Date: March 6, 2026
Issue: `#3051`

## Scope

Finish the first real operator-hardening pass for the durable Nexus service:

- explicit relay limit profile
- explicit client IP handling behind Cloudflare
- explicit backup and restore procedures
- an actual backup/restore drill against staging

## What shipped

### Relay operator profile

Committed config:

- `apps/nexus-relay/deploy/upstream-config.toml`

Deployed by:

- `scripts/deploy/nexus/03-configure-and-start.sh`

Current intentional settings:

- `remote_ip_header = "cf-connecting-ip"`
- `reject_future_seconds = 1800`
- `messages_per_sec = 50`
- `subscriptions_per_min = 60`
- `max_blocking_threads = 8`
- `max_event_bytes = 131072`
- `max_ws_message_bytes = 131072`
- `max_ws_frame_bytes = 131072`
- `broadcast_buffer = 4096`
- `event_persist_buffer = 1024`
- `limit_scrapers = true`
- `nip42_auth = true`

### Maintenance scripts

Added:

- `scripts/deploy/nexus/07-backup-relay-data.sh`
- `scripts/deploy/nexus/08-restore-relay-data.sh`

These use:

- `sqlite3 .backup` for a consistent SQLite copy
- a receipt-log copy when present
- a local operator archive destination outside the repo

## Deployment validation

The hardened profile was redeployed to:

- production VM: `nexus-mainnet-1`
- staging VM: `nexus-staging-1`

Production remained healthy after the rollout:

- `https://nexus.openagents.com/healthz` returned the durable relay health payload
- `https://nexus.openagents.com/api/stats` still reported `receipt_persistence_enabled = true`

## Staging backup/restore drill

Validation lane:

- VM: `nexus-staging-1`
- access path: local IAP tunnel to `ws://127.0.0.1:18080/`

### Pre-restore proof

Using an exact-id query (required under `limit_scrapers = true`), staging returned the known sample event:

- event id: `f3ce6798d70e358213ebbeba4886bbdfacf1ecfd4f65ee5323ef5f404de32b86`

### Backup

Created with:

```bash
NEXUS_VM='nexus-staging-1' \
NEXUS_BACKUP_LOCAL_DIR='/tmp/nexus-backups' \
scripts/deploy/nexus/07-backup-relay-data.sh
```

Created archive:

- `/tmp/nexus-backups/nexus-backup-nexus-staging-1-20260306-223302.tar.gz`

Archive contents:

- `nostr.db`
- `nexus-control-receipts.jsonl`
- `metadata.json`

### Restore

Restored with:

```bash
NEXUS_VM='nexus-staging-1' \
NEXUS_BACKUP_ARCHIVE='/tmp/nexus-backups/nexus-backup-nexus-staging-1-20260306-223302.tar.gz' \
scripts/deploy/nexus/08-restore-relay-data.sh
```

### Post-restore proof

After restore:

- staging `healthz` returned healthy durable-relay status
- exact-id replay returned the same sample event
- replay completed with `EOSE`

This confirms the backup archive is restorable and preserves relay event history plus receipt-side state.

## Operational conclusions

The current durable Nexus service now has:

- an explicit relay policy/limits profile
- real backup and restore scripts
- one validated restore drill against staging

What remains intentionally deferred:

- deeper abuse controls beyond the current rate/shape limits
- automatic TTL retention pruning
- more automated or off-host backup rotation
