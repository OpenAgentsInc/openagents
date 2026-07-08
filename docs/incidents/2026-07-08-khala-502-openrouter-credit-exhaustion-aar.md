# AAR: Khala completions 502 — OpenRouter platform credit exhaustion shadowed healthy lanes (+ main unbuildable after Tassadar prune)

Status: fix landed on `main` and deployed; canary recovery evidence at the
bottom. Written during the incident by the responding agent.

## Impact

- `POST https://openagents.com/api/v1/chat/completions` returned
  `502 {"error":"provider_error","reason":"openrouter rejected request (402)"}`
  to every authenticated request.
- Canary (`scripts/khala-canary.sh`) last recorded `state=up` at
  **2026-07-05T16:02:20Z**. From then through 2026-07-08 the canary logged
  `down` continuously: 502s on 2026-07-05, 500s through the 2026-07-06
  Cloudflare→GCP cutover window, then 500→502 on 2026-07-07, and solid 502s
  on 2026-07-08 (~848 consecutive down ticks that day).
- Unauthenticated probes returned 401 (edge + router alive), so the outage was
  upstream-lane-shaped, not a dead service.

## Root cause (the 502s)

1. The prod Khala conversational plan is an ordered overflow chain that puts
   the OpenRouter lane FIRST
   (`KHALA_CONVERSATIONAL_ADAPTER_PLAN` in
   `apps/openagents.com/workers/api/src/inference/model-router.ts`:
   OpenRouter → Vertex Gemini → Fireworks → Hydralisk GLM).
2. The platform OpenRouter account ran out of credits:
   `GET /api/v1/credits` returned `total_credits: 481.20`,
   `total_usage: 481.399…` — every request on that lane failed **402**.
3. `classifyStatus` in
   `apps/openagents.com/workers/api/src/inference/openrouter-adapter.ts`
   classified 402 as `request_rejected, retryable: false`, and
   `dispatchWithOverflow` surfaces non-retryable failures immediately — so the
   chain never overflowed to the healthy Vertex Gemini / Fireworks lanes
   (whose keys were configured and live on the service).
4. This exact failure mode was already KNOWN and documented — in the deploy
   script itself. `scripts/deploy-cloudrun.sh` (AC-1, #8503) deliberately
   omits `OPENROUTER_API_KEY` on staging because a
   "valid key, zero OpenRouter balance -> non-retryable 402" would "shadow the
   working Fireworks lane". Production kept the key and the landmine armed.

## Fix (deployed)

`openrouter-adapter.ts`: a 402 on the **platform** key is now classified
`quota_exhausted, retryable: true` — the lane is unviable, not the request —
so dispatch overflows to the next lane in the plan. A BYOK caller-key 402
(`request.callerProviderKey`) still surfaces non-retryable: only the caller
can top up their key, and overflowing would silently move their traffic onto
platform-paid lanes. Covered on both the buffered and SSE stream paths, with
tests in `openrouter-adapter.test.ts`.

## Secondary blocker found during the fix: main was unbuildable

The Tassadar prune (`e1fbd1c185`, 2026-07-08 ~21:43Z) deleted
`packages/tassadar-executor/` and removed the
`"@openagentsinc/tassadar-executor": "workspace:*"` dependency from
`apps/openagents.com/workers/api/package.json`, but left **13 runtime source
files** in `workers/api/src` importing the package (artanis-*, tassadar-*,
kernel-optimization-*). The prune added ambient type shims
(`archived-tassadar-modules.d.ts`) so `tsc` passed, but
`bun build src/cloudrun/server.ts` failed with
`Could not resolve: "@openagentsinc/tassadar-executor"` — meaning **no
monolith deploy could ship from main** until repaired. (The currently-serving
revision predated the prune, so serving was unaffected; deploys were not.)

Repair in the same change: restored `packages/tassadar-executor/` verbatim
from the pre-prune commit, re-added the workspace dependency, and removed the
now-stale `@openagentsinc/tassadar-executor*` ambient declares. A future
re-prune must rewrite the 13 importing files (or archive them) BEFORE deleting
the package — verify with the cloudrun bundle build, not just typecheck.

## Outstanding (not fixed here)

- **Owner-gated:** the OpenRouter account balance is still ~0. The gateway now
  serves by overflowing past the dead lane; topping up (or removing the
  OpenRouter lane from the prod plan / dropping the prod
  `OPENROUTER_API_KEY` like staging) is an owner money/posture decision.
  Recorded in `NEEDS_OWNER.md`.
- **Separate fault — FIXED (see "Connection-pool fix" below):**
  `openagents-monolith` logged a continuous flood of
  `Exceeded maximum of 100 connections per instance
  "openagentsgemini:us-central1:khala-sync-pg"` (Cloud SQL connector
  per-instance cap) since at least 2026-07-07T15:32Z (correlates with the
  GCP/Cloud SQL migration cutover, epic #8515), with intermittent 500/503s on
  DB-heavy routes (pylon heartbeat, forum, khala-sync runtime-intents). Every
  Khala Sync Postgres seam (the Postgres D1 adapter path via
  `khala-sync-domain-writes-database.ts` /
  `khala-code-product-state-store.ts`, plus the raw sync-engine routes, forum
  serving, auth-KV, and the durable inference stream) opened a NEW
  `postgres.js` client (`max: 1`) per statement and `end()`ed it; under
  `--concurrency 80` × up-to-4 instances that bursts past the 100-conn
  per-instance connector cap. Now converted to a shared, memoized connection
  pool — see below.

## Connection-pool fix (2026-07-08)

Root cause confirmed by reading the code (not just the diagnosis above): all 10
`postgres.js` construction sites in `apps/openagents.com/workers/api/src`
followed the same `postgres(connectionString, { max: 1 })`-per-acquire +
`end()`-after-each-op pattern. That discipline is correct under Cloudflare
Workers + Hyperdrive (isolate-per-request; Hyperdrive pools upstream) but is
pathological on the long-lived Cloud Run Bun process: one raw Cloud SQL
connection per statement.

Fix: a new shared helper `khala-sync-postgres-pool.ts`
(`acquireSharedPostgresClient`) that, on the server runtime, constructs ONE
pool-backed `postgres.js` client per `(connectionString, variant)` at first use
and reuses it across every request and statement; the returned `end()` is a
no-op so callers' existing per-op teardown never tears the shared pool down.
Idle connections are released by `idle_timeout: 20`. On Cloudflare Workers
(auto-detected via `navigator.userAgent`) the legacy fresh-`max:1`-client +
real-teardown path is preserved unchanged. `variant` keys the cache so the D1
adapter's int8→Number `types` parser never shares a connection with the
raw-string sync-engine client. Pool size is `KHALA_SYNC_PG_POOL_MAX`
(default 10); with ≤4 active variants against the single prod DSN that is
≤40 conns/instance, comfortably under the 100 cap, and ≤160 across 4 instances,
well under the db-custom-8-53248 instance's `max_connections`.

All 10 sites now delegate to the helper: `khala-sync-push-routes.ts`,
`khala-sync-log-routes.ts`, `khala-sync-cvr-routes.ts`,
`khala-sync-scope-auth.ts`, `khala-sync-bootstrap-routes.ts`,
`khala-sync-db-smoke-routes.ts`, `khala-code-product-state-store.ts`,
`khala-sync-domain-writes-database.ts`, `forum/forum-postgres-serving.ts`,
`inference/durable-inference-stream-backend.ts` (auth-KV and the money/billing
paths inherit the fix through `defaultMakeKhalaSyncSqlClient`).

Tests: `khala-sync-postgres-pool.test.ts` pins reuse (one client per
`(dsn, variant)`), the no-op `end()`, `max` injection, variant/DSN isolation,
and the Workers fresh-client fallback. `typecheck:cloudrun` clean; all 10
affected suites + the adapter contract test green (230 tests).

Fix commit: `1f1d93d433` on `main`.

Deploy (2026-07-08 ~23:2x–23:28Z): staging revision
`openagents-monolith-staging-00024-5tz` (healthz 200; 100 concurrent DB-backed
requests all 200; zero connection errors), then production revision
`openagents-monolith-00045-87l` serving 100%.

Verification under live prod traffic: `khala-tokens-served`, `product-promises`,
and repeated 60–80-concurrent bursts all 200; healthz green. The
`Exceeded maximum of 100 connections per instance` flood STOPPED at the
revision cut — the last occurrence was 2026-07-08T23:27:52Z on the draining old
revision `00044-hmj`; the new revision `00045-87l` logged ZERO connection
errors across the following minutes of sustained traffic.

Pool sizing knob: `KHALA_SYNC_PG_POOL_MAX` (default 10). Raise it (redeploy) if
a future load profile needs more concurrent connections per variant — keep
`variants × poolMax × maxInstances` under both the 100-conn/instance connector
cap and `khala-sync-pg`'s `max_connections`.

## Timeline (UTC)

- 2026-07-05 16:02 — last canary `up`.
- 2026-07-05 16:03 → 2026-07-06 — 502s begin; CF→GCP cutover window (500s).
- 2026-07-07 15:32 — first Cloud SQL "Exceeded maximum of 100 connections"
  flood visible in monolith logs (rev 00026).
- 2026-07-08 18:53 — rev `openagents-monolith-00043-g2p` deploys (pre-prune);
  502s continue (OpenRouter 402, not a bad revision — rollback was NOT the fix).
- 2026-07-08 21:43 — Tassadar prune `e1fbd1c185` lands on main; monolith
  becomes unbuildable (latent deploy blocker).
- 2026-07-08 22:13+ — RED-ALERT investigation begins; root cause isolated via
  direct authenticated probe (502 body named the OpenRouter 402) and
  `openrouter.ai/api/v1/credits`.
- 2026-07-08 ~23:0x — fix committed to main, staging deploy + smoke, prod
  deploy, canary green (evidence below).

## Recovery evidence

Fix commit `9bf6be5191` on `main`; staging revision
`openagents-monolith-staging-00023-88t` (healthz 200), production revision
`openagents-monolith-00044-hmj` serving 100% from 2026-07-08 ~22:59Z.

Immediately after the prod rollout, `POST /api/v1/chat/completions` returned
200 with `"supply_lane":"fireworks"` (`deepseek-v4-flash`) — the request
overflowed past the dead OpenRouter lane exactly as intended. The streaming
path also returned 200 with SSE deltas.

Three consecutive official canary runs (`scripts/khala-canary.sh`, exit 0,
public counter movement required and observed):

```text
{"ts":"2026-07-08T23:02:25Z","kind":"canary","state":"up","http":200,"counterDelta":842,...,"detail":"ok"}
{"ts":"2026-07-08T23:02:39Z","kind":"canary","state":"up","http":200,"counterDelta":842,...,"detail":"ok"}
{"ts":"2026-07-08T23:02:46Z","kind":"canary","state":"up","http":200,"counterDelta":842,...,"detail":"ok"}
```

## Lessons

- A "cheapest lane first" overflow chain is only as available as its FIRST
  lane's billing state when hard-fail statuses don't overflow. Classify
  payment/quota exhaustion on PLATFORM keys as lane-unviable everywhere a lane
  can lead a plan.
- If a known landmine is worth a warning comment in the deploy script, it is
  worth fixing in the router. The staging posture (omit the key) silently
  diverged from prod (keep the key) and prod kept the outage.
- Type shims make `tsc` lie about deletability. Any package prune must be
  proven with the runtime bundle build of every consumer before landing.
