# @openagentsinc/khala-capture

The Khala Sync **capture daemon** as an always-on Cloud Run service (#8554).

This is the live-tail delivery pipe. It tails the `khala_sync_changelog`
(LISTEN `khala_sync_changelog_append` wake + short poll fallback) over a
DIRECT Cloud SQL **session** connection and pushes ordered whole-version-group
batches to the LiveHub `/append` route, which fans DeltaFrames out to
subscribed WebSocket clients. Checkpoints advance only on a hub 2xx; delivery
is at-least-once (the hub dedupes by version).

The capture logic itself lives in
[`@openagentsinc/khala-sync-server`](../../packages/khala-sync-server)
(`src/capture.ts`, `startCaptureDaemon`). This app is just the Cloud Run
entrypoint (`src/server.ts`) + deploy shape.

## Why this exists

Capture used to run under launchd on the owner's Mac over a direct DB IP.
**CFG-14 closed the Cloud SQL public ingress**, freezing that path (checkpoints
stopped advancing → no live updates reached mobile). This service replaces it:
it connects through the **Cloud SQL Auth Connector** unix socket
(`--add-cloudsql-instances`, mounted under `/cloudsql/<instance>`) and stays
resident, so live delivery no longer depends on the Mac.

## Deploy

```sh
export CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config
bash scripts/deploy-cloudrun.sh prod        # or staging
```

See [`docs/khala-sync/RUNBOOK.md`](../../docs/khala-sync/RUNBOOK.md) "Capture
daemon operation" for the liveness query and recovery procedure, and
[`docs/DEPLOYMENT.md`](../../docs/DEPLOYMENT.md) for the Cloud SQL connector
posture.

## Deploy shape (deliberate)

- `min=max=1` — a **singleton** daemon. LISTEN/NOTIFY needs one persistent
  session; a second instance only double-pushes (the hub dedupes by version).
- `--no-cpu-throttling` — the daemon loop, LISTEN connection, and poll timer
  must run **between** HTTP requests.
- `--add-cloudsql-instances openagentsgemini:us-central1:khala-sync-pg` —
  mounts the connector socket; `PGHOST` points at `/cloudsql/<instance>`.

## Env

| Var | Purpose |
| --- | --- |
| `PGHOST` | Connector socket dir (`/cloudsql/<instance>`) — selects socket mode |
| `PGUSER` | `khala_capture` (SELECT changelog/checkpoints + UPDATE checkpoints) |
| `PGPASSWORD` | Secret Manager `khala-sync-capture-password` |
| `PGDATABASE` | `khala_sync_prod` / `khala_sync_staging` |
| `KHALA_SYNC_HUB_APPEND_URL` | LiveHub `/append` URL |
| `KHALA_SYNC_HUB_TOKEN` | Secret Manager `khala-live-hub-token` (shared bearer) |
| `PORT` | Cloud Run health port (default 8080) |

Secrets are read from the environment (Secret Manager mounts) and never logged.

## Health

`GET /health` → `{ ok, service, listener, uptimeSeconds }`. Real liveness is
the checkpoints table advancing (RUNBOOK liveness query), not this endpoint.
