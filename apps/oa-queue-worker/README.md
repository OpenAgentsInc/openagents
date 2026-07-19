# oa-queue-worker

CFG-7 (#8522, epic #8515): the Cloud Run pump that replaced the four
Cloudflare Queues.

## How the queue path works now

1. **Producers** (the `openagents.com` app) enqueue with a single INSERT into
   `oa_infra_jobs` over the same Postgres connection the app already uses for
   Khala Sync — see
   `apps/openagents.com/workers/api/src/oa-job-queue-producer.ts`.
2. **This service** leases due jobs per topic with the oa-infra Postgres
   JobQueue (`FOR UPDATE SKIP LOCKED`, `packages/oa-infra`), POSTs each job to
   the app's admin-bearer internal route `POST /api/internal/queue/deliver`,
   acks on 2xx, nacks otherwise (delayed retry, `max_attempts = 4` mirrors the
   retired wrangler `max_retries: 3`, exhausted jobs land in the dead-letter
   state for `deadLetters()` inspection/replay).
3. **The delivery route** runs the original queue-consumer logic unchanged
   (`dispatchOaQueueMessage` in `workers/api/src/index.ts`) — the handlers
   need that app runtime's bindings (D1, the EVENT_LEDGER_OWNER Durable
   Object, provider secrets), which is why the pump delivers over HTTP
   instead of hosting the handlers itself. When CFG-9 moves the app off
   workerd, only `OA_QUEUE_DELIVERY_URL` changes.

Topics (`src/topics.ts`) mirror the retired queue names and batch sizes:

| topic                                       | batch | delivery  |
| ------------------------------------------- | ----- | --------- |
| `openagents-adjutant-enrichment-jobs`       | 1     | http      |
| `openagents-event-ledger-ingest`            | 1     | http      |
| `openagents-pylon-codex-raw-event-metadata` | 25    | http      |
| `oa-queue-worker-smoke`                     | 10    | ack-local |

The retired `openagents-autopilot-runner-events` queue had no producers and
no consumer. The lane was deleted, not ported. The `oa-queue-worker-smoke`
topic acks locally so operators can prove the live lease/ack loop with one
INSERT and zero app dependency.

## Run / test

```sh
pnpm --dir apps/oa-queue-worker run test
pnpm --dir apps/oa-queue-worker run typecheck

OA_INFRA_DATABASE_URL=postgres://... \
OA_QUEUE_DELIVERY_TOKEN=... \
pnpm --dir apps/oa-queue-worker run serve
```

## Deploy

```sh
pnpm --dir apps/oa-queue-worker run deploy   # scripts/deploy-cloudrun.sh
```

Cloud Run service `oa-queue-worker`, project `openagentsgemini`, region
`us-central1`, `min-instances=1` (a scaled-to-zero pump delivers nothing).
Secrets ride GCP Secret Manager (`oa-queue-worker-database-url`,
`oa-queue-worker-delivery-token`). See the deploy script header.

The jobs table ships with oa-infra: apply
`packages/oa-infra/migrations/` with
`node --import tsx packages/oa-infra/scripts/migrate.ts --database-url <direct-url>`
(its own `oa_infra_migrations` ledger, deliberately separate from
`khala_sync_migrations`).
