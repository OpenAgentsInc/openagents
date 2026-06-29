# Staging Environment Setup (openagents.com Worker)

Date: 2026-06-19

A fully data-isolated staging environment for the `openagents.com` Cloudflare
Worker. Staging runs as a separate Worker script (`openagents-staging`) on
`workers.dev`, with its own D1 database, KV namespace, R2 bucket, and queues, so
the whole system can be exercised against staging data without touching
production.

## Staging URL

- <https://openagents-staging.openagents.workers.dev>

## Worker

- Script name: `openagents-staging`
- `workers_dev: true`, `preview_urls: true`, no custom routes.
- Config: the `env.staging` block of
  `apps/openagents.com/workers/api/wrangler.jsonc`. Production (the top-level
  config, Worker `openagents-autopilot`) is unchanged.

## Isolated resources (staging-only)

Each of these is a brand-new resource created with a `-staging` suffix. None of
them is shared with production.

| Kind  | Binding                   | Staging resource                              | ID                                       |
| ----- | ------------------------- | --------------------------------------------- | ---------------------------------------- |
| D1    | `OPENAGENTS_DB`           | `openagents-autopilot-staging`                | `b30c43fa-fa39-4e56-8b95-f90d5b58292d`   |
| KV    | `AUTH_STORAGE`            | `AUTH_STORAGE_STAGING`                         | `586eb41c0d69448a96ffabaa5fc9bafd`       |
| R2    | `ARTIFACTS`               | `openagents-autopilot-artifacts-staging`      | (bucket name)                            |
| Queue | `RUNNER_EVENTS`           | `openagents-autopilot-runner-events-staging`  | `78656cb9bbbe420789202f4de53c11f3`       |
| Queue | `ADJUTANT_ENRICHMENT_QUEUE` | `openagents-adjutant-enrichment-jobs-staging` | `1e7550cca2b747c9899650c898c55f2e`     |

The `ADJUTANT_ENRICHMENT_QUEUE` consumer is also wired to the staging queue.

Binding *names* are intentionally identical to production (e.g. `OPENAGENTS_DB`,
`AUTH_STORAGE`, `ARTIFACTS`) because the Worker code reads those binding names;
only the underlying resources differ.

Durable Object bindings are left as-is — DO storage is per-script, so the
`openagents-staging` script already gets its own isolated DO namespace
(staging only binds `SYNC_ROOM`; MDK container DOs are not bound on staging).

### Staging vars

- `INFERENCE_GATEWAY_ENABLED: "true"` (added so inference can be tested on
  staging).
- `ARTANIS_SCHEDULED_RUNNER_ENABLED: "false"`, `SHC_DISPATCH_MODE:
  "unconfigured"`, `GITHUB_CLIENT_SECRET: "staging-unconfigured"` (kept).
- Cron triggers are empty on staging.

## D1 migrations

All migrations `0001`–`0210` (including `0210_inference_free_tier.sql`) are
applied to `openagents-autopilot-staging --remote`.

One-time seed note: the historical seed migration
`0005_core_team_owner_and_ben.sql` runs an unconditional
`UPDATE teams SET owner_user_id = 'github:14167547'`, which fails the
`teams.owner_user_id → users(id)` FK on a fresh DB. To let the historical
migration batch apply cleanly on an empty staging DB, a single placeholder owner
user row (`github:14167547`, `staging-owner`) was seeded before re-running the
apply. This is staging-only seed data; the shared migration files and production
are untouched.

To (re)apply migrations later:

```sh
cd apps/openagents.com/workers/api
bun run migrate:staging
# == wrangler d1 migrations apply openagents-autopilot-staging --env staging --remote
```

## Deploy

```sh
cd apps/openagents.com/workers/api
bun run deploy:staging
# == check:deploy → migrate staging D1 → build:web → wrangler deploy --env staging
```

Or directly:

```sh
cd apps/openagents.com && bun run build:web
cd workers/api && wrangler deploy --env staging --assets ../../apps/web/dist
```

## Secrets

Staging has its own secret namespace (separate from production). Set staging
secrets with:

```sh
cd apps/openagents.com/workers/api
wrangler secret put <NAME> --env staging
```

### Bitcoin treasury is intentionally UNCONFIGURED on staging

The real Bitcoin treasury secrets (`MDK_*` mnemonic / treasury / service tokens)
are deliberately **not** set on staging. The staging Worker does not bind the
MDK container Durable Objects, so MDK containers stay unconfigured / test on
staging. Do not copy any production `MDK_*` secret value onto staging.

### Owner must set (for full testing) — Stripe TEST keys

Use Stripe **test-mode** keys/values only:

- `STRIPE_API_KEY`
- `STRIPE_WEBHOOK_SIGNING_SECRET`
- `STRIPE_CREDIT_PACKAGES_JSON`
- `STRIPE_CHECKOUT_SUCCESS_URL` (point at the staging URL)
- `STRIPE_CHECKOUT_CANCEL_URL` (point at the staging URL)

### Operator (agent) will set — inference

- `VERTEX_SA_KEY`
- `FIREWORKS_API_KEY`
- `GEMINI_API_KEY`

## Verification (2026-06-19)

- `https://openagents-staging.openagents.workers.dev/` → HTTP 200.
- `https://openagents-staging.openagents.workers.dev/api/public/product-promises`
  → HTTP 200 with valid JSON.
- Staging D1 isolation confirmed: `SELECT count(*) FROM users` returns `1` (the
  seeded placeholder owner only), vs production's many users.
- `wrangler d1 execute openagents-autopilot-staging --env staging --remote
  --command "SELECT name FROM sqlite_master WHERE type='table' LIMIT 5"` returns
  staging-owned tables.
- `bun run check:deploy` GREEN (production config still valid).
