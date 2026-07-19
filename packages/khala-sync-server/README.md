# @openagentsinc/khala-sync-server

Server substrate for Khala Sync. Production runs entirely on Google Cloud:

- the OpenAgents Node 24 API runs on Cloud Run.
- relational and changelog state lives in Cloud SQL Postgres.
- `khala-live-hub` runs on Cloud Run and provides bounded WebSocket fan-out.
- capture, compaction, and migrations use direct Cloud SQL connections. And
- credentials live in Secret Manager.

Cloudflare Workers, Durable Objects, D1, Hyperdrive, Queues, R2, and Wrangler
are retired. They are not runtime options, compatibility lanes, migration
requirements, or operational authorities. SHC was a limited pilot, never the
primary infrastructure, and is not accepted by current dispatch admission.

## Components

- `migrations/` defines the Cloud SQL schema.
  The schema contains per-scope counters, the transactional changelog,
  mutation idempotency, projections, and the Google-Cloud-only admission
  boundary.
- `src/push-engine.ts` runs named mutators in one Cloud SQL transaction.
- `src/read-service.ts` serves consistent bootstrap pages and resumable log
  pages with self-contained cursors.
  It requires no database session across requests.
- `src/outbox-writer.ts` allocates commit-ordered per-scope versions and
  appends changelog rows transactionally with business state.
- `src/scope-auth.ts` is the fail-closed read authorization gate over live
  Cloud SQL ownership and membership data.
- `src/capture.ts` tails the changelog through a direct Postgres connection
  and forwards ordered, replayable frames to LiveHub.
- `src/compaction.ts` advances retained windows and does not create partial-log
  gaps.
- `src/managed-sandbox-store.ts` owns generation-fenced managed-sandbox
  commands, native events, receipts, turn order, and compatibility cursors.
- projection modules publish redacted public or owner-scoped post-images from
  authoritative Cloud SQL state.

The API integration lives under
`apps/openagents.com/workers/api/`. `workers/api` is a historical directory
name and does not identify the runtime provider.

## Connection authority

The shared Cloud SQL instance is `khala-sync-pg` in project
`openagentsgemini`, region `us-central1`:

- production database: `khala_sync_prod`
- `staging` database: `khala_sync_staging`
- request role: `khala_app`
- migration role: `khala_migrate`
- capture role: `khala_capture`

Passwords, connector URLs, and service credentials must never be committed.
Cloud Run receives them from Secret Manager. Local administrative access uses
the Cloud SQL Auth Proxy and the workspace-local secret files documented by the
Khala Sync runbook.

## Runtime invariants

1. Cloud SQL is authoritative. LiveHub is a bounded cache/fan-out service.
2. A mutator's business writes, changelog append, version allocation, and
   mutation receipt commit or roll back together.
3. Read pages never split a version, and bootstrap cursors fail closed once
   compaction passes their retained window.
4. Authorization is checked live. Storage failures never become grants.
5. Public projections use explicit allowlists and refuse private material.
6. Capture delivery is at least once. Hub and client apply are idempotent.
7. The production admission lane is `cloud-gcp`. The managed runner backend is
   `gcloud_vm`. Retired pilot provenance is terminal history only.
8. Managed-sandbox command bytes enter Cloud SQL before provider effects.
9. One sandbox has at most one `pending` command and one generation with
   `acceptingWork: true`.
10. Compatibility cursors never replace or advance beyond native event truth.

## Commands

From the repository root on Node 24:

```sh
pnpm --filter @openagentsinc/khala-sync-server typecheck
pnpm --filter @openagentsinc/khala-sync-server test
pnpm --filter @openagentsinc/khala-sync-server migrate
pnpm --filter @openagentsinc/khala-sync-server compact
pnpm --filter @openagentsinc/khala-sync-server capture
```

Migration, compaction, and capture commands require a direct Cloud SQL
connection. See `docs/khala-sync/RUNBOOK.md` for the approved proxy, role, and
Secret Manager procedure.

## Retired migration code

Some source modules and tests retain D1-shaped row names so historical exports
can be decoded and audited byte-for-byte. They do not connect to Cloudflare,
are not exported as production authority, and are not invoked by deploy or
operator scripts. Git history and the private retained-provider archive are the
source of truth for the retired platform itself.
