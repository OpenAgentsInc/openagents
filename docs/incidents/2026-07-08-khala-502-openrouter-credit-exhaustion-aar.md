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

## Lane dropped (owner decision 2026-07-09)

The owner resolved the outstanding money/posture question by **dropping
OpenRouter as a platform Khala lane entirely** ("drop the lane … we are going to
use Gemma 4 via our gcloud primarily"). Even with the #8565 402→overflow fix,
every request still paid a dead-lane ~0.5s 402 hop because OpenRouter still LED
the plan. Changes (this pass):

- `model-router.ts`: `OPENROUTER_KHALA_FALLBACK_ADAPTER_ID` removed from every
  prod Khala plan (`KHALA_CONVERSATIONAL_*`, `*_FIREWORKS_DEEPSEEK_*`,
  `*_AGENT_TOOL_*`, `*_STRONG_CODING_*`, `*_PAID_FAILOVER_*`). The conversational
  plan now LEADS with our own Google Cloud (Vertex Gemini) lane, then Fireworks,
  then the owned GLM lane. Tool-bearing Khala still leads with the owned GLM
  lane. Router/dispatch/chat-completions tests updated to match.
- `deploy-cloudrun.sh`: `OPENROUTER_API_KEY` dropped from the **production**
  `--set-secrets` set (staging already omitted it), so the platform lane cannot
  silently re-lead even if a plan is later mis-edited.
- `openrouter-adapter.ts`: deprecation note added to the header. The adapter code
  and tests are KEPT (physical removal to backroom can follow later) and it stays
  REGISTERED (`index.ts`) for the **BYOK caller-key path only** — that path forces
  `[OPENROUTER_KHALA_FALLBACK_ADAPTER_ID]` and supplies the caller's OWN key per
  request, never the platform key. `index.ts registerOpenRouterAdapter` now
  registers the adapter even with no platform key so removing the prod secret does
  not break BYOK.

**Gemma 4 specifically is NOT yet the primary** — the current gcloud lane serves
`gemini-3.5-flash` (Vertex `aiplatform` publishers/google endpoint, SA-token).
A true Gemma 4 Khala lane is a FOLLOW-UP: it needs a new adapter (or a
`google_inference` variant of the vertex lane) targeting the **Generative
Language API** `POST https://generativelanguage.googleapis.com/v1beta/models/gemma-4-31b-it:generateContent?key=GEMINI_API_KEY`
(the exact path `apps/sarah/src/services/google-inference.ts` proved live under
#8594), filtering Gemma's `thought:true` scratchpad parts, mapping
`thoughtsTokenCount` into reasoning tokens, and with NO tool-calling (Gemma has
none, so tool-bearing Khala must stay on the GLM/Fireworks lanes). Until that
lands, "gcloud primarily" = the Vertex Gemini lane leads.

### Lane-drop deploy + verification (2026-07-09)

Commit on `main`: `cdd18401ae`. Deployed via the sanctioned monolith path
(`apps/openagents.com/workers/api/scripts/deploy-cloudrun.sh`, automation SA):
staging revision `openagents-monolith-staging-00027-6tz` (healthz 200), then
production revision `openagents-monolith-00049-jpc` (healthz 200). Prod service
env confirmed to NO LONGER carry `OPENROUTER_API_KEY` (Vertex/Fireworks/Gemini
keys retained).

Verification (prod, authenticated `POST /api/v1/chat/completions`,
`openagents/khala`):

- Normal requests (`max_tokens` ≥ 64) now serve on `worker=vertex-gemini`
  (our gcloud lane) with `fallback_reason=null`, ~1.5s — OpenRouter is no longer
  attempted, so the ~0.5s dead-lane 402 hop is gone. Baseline pre-deploy was
  `worker=fireworks` (overflowing past the exhausted OpenRouter lead), ~2.1–5.8s.
- Very small requests (`max_tokens=8`, e.g. the canary) still land on
  `worker=fireworks` — but now via a single `empty_assistant_content` overflow
  off the Vertex primary (a thinking-model artifact when the token budget is too
  small to emit text), NOT the old double hop (OpenRouter 402 → Vertex → …).
  Minor follow-up: consider a min-output-token floor for the Khala Vertex lane so
  tiny budgets still emit text.
- Official canary (`scripts/khala-canary.sh`) run twice post-deploy:
  `state=up, http=200, counterDelta=842` then `1684` (exit 0 both).

## Gemma 4 lane landed — primary conversational Khala lane (2026-07-09 follow-up)

The follow-up the "Lane dropped" note called for is now built: a real **Gemma 4**
lane on our own gcloud, leading the conversational Khala plan (owner directive
2026-07-09: "we are going to use Gemma 4 via our gcloud primarily"). "gcloud
primarily" now = Gemma 4, with the Vertex Gemini lane as the next overflow.

### Adapter (`inference/gemma4-adapter.ts`, `GEMMA4_ADAPTER_ID = 'google-gemma4'`)

- **Target = Generative Language API**, the exact path #8594 proved live in
  `apps/sarah/src/services/google-inference.ts`:
  `POST https://generativelanguage.googleapis.com/v1beta/models/gemma-4-31b-it:generateContent?key=GEMINI_API_KEY`
  (and `:streamGenerateContent?alt=sse` for the incremental path). This is NOT the
  Vertex `aiplatform` publishers/google endpoint the `vertex-gemini` lane uses —
  it is the API-key-in-query path, so the adapter NEVER surfaces the request URL
  in an error/log (the key rides the URL). Implements the full adapter seam:
  buffered `complete`, buffered `stream`, and true pass-through `streamSse`.
- **Key reused, none minted.** The adapter reads the existing `GEMINI_API_KEY`
  Worker secret — the SAME secret sarah's google-inference service uses
  (`openagents-gemini-api-key:latest`, already in `deploy-cloudrun.sh` on BOTH
  staging and prod). No new credential, no owner credential action. The key is
  resolved LAZILY per call (the registry is constructed at module load before env
  is captured — same pattern as the Vertex lanes' `tokenProvider`); with no key
  the adapter is INERT (typed non-retryable error), and the gateway route stays
  flag-gated regardless.
- **Thought filtering.** Gemma is a thinking model: candidates carry scratchpad
  parts flagged `thought: true` before the answer. Buffered `complete` drops them
  from user-visible content; streaming routes them to the separate `reasoningDelta`
  channel (clients hide it) and keeps them out of `contentDelta`. Either way thoughts
  never appear in user-visible output.
- **Exact token accounting.** `usageMetadata.thoughtsTokenCount` maps verbatim to
  a new optional `InferenceUsage.reasoningTokens`, which the served-tokens recorder
  writes into `token_usage_events.reasoning_tokens` (was hard-coded `0`). Mapped
  only when the field is a real finite number — never invented. Google's
  `totalTokenCount` already includes thoughts, so it is trusted as the total (a
  breakdown dimension, not an addend). Other lanes leave `reasoningTokens`
  undefined -> `0`, unchanged.
- **NO-TOOLS guard (airtight, two layers).** Gemma has no tool calling. (1) The
  router keeps `google-gemma4` out of every tool plan:
  `selectAdapterPlanForKhalaToolRequest` now filters it from the base plan, so a
  tool-bearing Khala request uses the GLM-led agent-tool plan and never lists
  Gemma. (2) Defense-in-depth: the adapter itself refuses any request carrying
  tools/functions or prior tool-call/tool-result messages with a RETRYABLE
  `tool_calls_unsupported` error, so even a mis-routed tool request overflows to a
  tool-capable lane (Vertex Gemini / Fireworks / GLM) instead of silently dropping
  the tools. The conversational plan is the only plan Gemma leads.
- **Tiny-budget guard (the AAR caveat above).** The "min-output-token floor for the
  Khala Vertex lane" minor follow-up is implemented for Gemma: thoughts draw from
  the output budget, so a tiny `max_tokens` (the canary's 8) would be entirely
  consumed by thoughts and emit zero visible text -> `empty_assistant_content`
  overflow off the primary. The adapter floors the effective `maxOutputTokens` to
  `GEMMA4_DEFAULT_MIN_OUTPUT_TOKENS` (512) so tiny requests keep headroom for a
  visible answer and the canary stays on the Gemma primary. Budgets already above
  the floor are untouched.
- **Failure classification** matches the #8565 overflow pattern: 402
  `quota_exhausted` / 429 `rate_limited` / 503 `service_overloaded` / 5xx
  `upstream_error` are all retryable (lane unviable -> overflow); a 4xx rejection
  surfaces non-retryable.

### Plan ordering — before / after

- Conversational (`KHALA_CONVERSATIONAL_ADAPTER_PLAN`):
  before `[vertex-gemini, fireworks, hydralisk-glm]`
  → after `[google-gemma4, vertex-gemini, fireworks, hydralisk-glm]`.
- Agent-tool (`KHALA_AGENT_TOOL_ADAPTER_PLAN`), strong-coding, deepseek-backing,
  paid-failover: UNCHANGED (still GLM-led for tools). Gemma is excluded from all of
  them by the no-tools guard.

### Tests

New `gemma4-adapter.test.ts` (17 tests: endpoint/key-in-query, system hoist +
role map, thought filtering, exact `thoughtsTokenCount -> reasoningTokens`,
min-output floor, no-tools refusal (declared tools + tool messages), full failure
classification, key-not-leaked, inert-without-key, streamSse thought→reasoning
split + receipt-first terminal usage, buffered stream). Router plan-order tests
updated for the Gemma lead + a new assertion that the tool plan excludes
`google-gemma4`. Full inference suite green: **1839 passing** (was 1822; +17).
`typecheck:cloudrun` clean for the changed files; the cloudrun server bundle
builds (the real deploy gate).

### Deploy + verification (2026-07-09)

Commit on `main`: `PENDING-COMMIT`. Deployed via the sanctioned monolith path
(`apps/openagents.com/workers/api/scripts/deploy-cloudrun.sh`, automation SA):
staging revision `PENDING`, then production revision `PENDING`. Rollback target if
the canary goes red post-deploy: `openagents-monolith-00049-jpc`.

Verification (prod, authenticated `POST /api/v1/chat/completions`,
`openagents/khala`): `PENDING`.

## Outstanding (superseded by "Lane dropped" above)

- ~~**Owner-gated:** the OpenRouter account balance is still ~0…~~ Resolved: the
  platform lane is dropped rather than topped up.
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
