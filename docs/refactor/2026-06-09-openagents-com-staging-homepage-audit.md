# openagents.com Staging Homepage Audit

Date: 2026-06-09

## Scope

Move the current homepage implementation into the rebuilt `openagents` repo,
wire it to public-safe Pylon and Forum evidence endpoints, and deploy a new
Cloudflare Worker for staging verification before the `openagents.com` cutover.

## Homepage Implementation

- The logged-out Home model now loads:
  - `GET /api/public/pylon-stats`
  - `GET /api/forum/launch-status`
  - `GET /api/forum/tip-leaderboards?limit=10`
- The root document view now renders the logged-out Home route through the same
  Foldkit submodel path used by other public routes.
- The homepage renders endpoint manifest, token-bound task dispatch, Nostr relay
  configuration, Pylon stats, Forum stats, accounting evidence, and explicit
  claim-boundary panels.
- Money values are shown only when returned by a public endpoint. Missing
  public-safe evidence renders as `Unavailable`.

## Forum Placement Decision

Keep the live Forum surface inside `apps/openagents.com` for this staging
deployment.

Reasoning:

- The current Forum implementation shares the same Worker, D1 schema, auth
  session boundary, L402 receipt routes, Tailwind build, and public route policy
  as the product app.
- Splitting it into an independently deployed app before those contracts are
  packaged would duplicate auth/payment/database boundaries and increase
  cutover risk.
- `apps/forum` remains the extraction target and already records the
  `openagents.com/forum` mount contract.

Extraction criteria:

- Move Forum schemas into a shared package when both the Worker and standalone
  Forum app consume them.
- Move Forum UI into `apps/forum` when it has its own build entry, route base,
  visual tokens, and no hidden dependency on product page state.
- Keep `/forum` served by the `openagents.com` Worker until Cloudflare routing,
  asset build, auth, D1 access, and L402 receipts have an explicit handoff
  contract.

## Cloudflare Data Decision

Staging uses the existing Cloudflare data resources for verification:

- Existing D1 database binding: `OPENAGENTS_DB`
- Existing KV auth storage binding: `AUTH_STORAGE`
- Existing R2 artifacts binding: `ARTIFACTS`
- Existing dispatch namespace for site routes
- Queue producer bindings remain available for route compatibility

The staging Worker intentionally does not configure a cron trigger or queue
consumer. This allows real public-read verification against the current data
without a second scheduled/consumer script mutating the same operational state.
Secrets that are not needed for public-read verification are intentionally not
copied into staging. The staging config uses a dummy GitHub client secret,
omits Resend send credentials, and marks SHC dispatch as `unconfigured`.

Fresh-resource cutover remains a separate migration:

1. Create new D1, KV, R2, queues, dispatch namespace, and Durable Object
   namespaces.
2. Apply D1 migrations to the new database.
3. Export/import or replay only public-safe and required operational records.
4. Run read-only smoke checks for homepage, Forum, public Pylon stats, auth
   session, and receipt lookup.
5. Enable write routes, then scheduled and queue consumers.
6. Flip the `openagents.com` custom domains after parity evidence is recorded.

## Staging Worker

- Wrangler environment: `staging`
- Worker name: `openagents-staging`
- Workers.dev enabled for verification
- URL: `https://openagents-staging.openagents.workers.dev`
- Current deployed version after this audit: `28bf3c85-cdfe-453d-8f0e-5e81ed1c8b9c`
- No custom domain routes
- No cron trigger
- No queue consumer

## Verification

- `bun run --cwd apps/openagents.com/apps/web typecheck`
- `bun run --cwd apps/openagents.com/apps/web test`
- `bun run --cwd apps/openagents.com/workers/api typecheck`
- From `apps/openagents.com/workers/api`: `bun run test -- src/public-pylon-stats.test.ts`
- `bun run --cwd apps/openagents.com build:web`
- `bun run test`
- From `apps/openagents.com/workers/api`: `bunx wrangler deploy --env staging --dry-run`
- `bunx wrangler deploy --env staging`
- `curl -fsSI https://openagents-staging.openagents.workers.dev/`
- `curl -fsS https://openagents-staging.openagents.workers.dev/api/public/pylon-stats`
- `curl -fsS https://openagents-staging.openagents.workers.dev/api/forum/launch-status`
- `curl -fsS 'https://openagents-staging.openagents.workers.dev/api/forum/tip-leaderboards?limit=10'`
