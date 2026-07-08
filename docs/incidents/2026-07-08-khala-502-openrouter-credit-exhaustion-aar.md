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
- **Separate ongoing fault:** `openagents-monolith` logs a continuous flood of
  `Exceeded maximum of 100 connections per instance
  "openagentsgemini:us-central1:khala-sync-pg"` (Cloud SQL connector
  per-instance cap) since at least 2026-07-07T15:32Z, with intermittent
  500/503s on DB-heavy routes (pylon heartbeat, forum, khala-sync
  runtime-intents). The Postgres D1 adapter
  (`postgres-d1-adapter.ts` / `khala-sync-domain-writes-database.ts`) opens a
  NEW `postgres.js` client (max:1) per statement; under `containerConcurrency:
  80` that bursts past the 100-conn connector cap. Needs a shared pool or a
  per-instance connection budget — separate lane, filed as follow-up.

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

See the canary lines following the prod deploy in
`~/work/.khala-heartbeat/canary.jsonl` (state=up, http=200, non-zero token
delta) — appended by the on-host canary loop after the fix went live.

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
