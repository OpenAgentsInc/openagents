# Khala acceptance-verdict runner — deploy plan (EPIC #6017)

The gating piece for the live auto-settlement loop: **no runner → no executed
verdicts → no verified outcomes → no payouts.** This service is the out-of-Worker
headless executor that takes a khala-code artifact, runs the acceptance suite in a real
chromium, and POSTs a real `AcceptanceVerdict` back to the Worker so the receipt
backfills `verified:true`.

This doc is the **deploy plan only** — nothing here moves money or sets a prod secret.
Everything stays INERT until an owner sets the flags + token below.

## The full chain

```
 openagents.com Worker (CF, no chromium)            acceptance-runner (this service, node + chromium)
 ┌───────────────────────────────────────┐         ┌──────────────────────────────────────────────┐
 │ khala-code completion w/ executable    │         │  poll loop (src/daemon.ts):                    │
 │ artifact                               │         │                                                │
 │   └─ enqueueAcceptanceJob ──▶ pull queue (D1) ◀──┼── GET  /v1/inference/acceptance-jobs/lease     │
 │      (acceptance-dispatch.ts,          │  lease  │      (authenticated; 204 when idle)            │
 │       KHALA_ACCEPTANCE_DISPATCH_ENABLED)│        │   ├─ resolveArtifact(ref)  (R2-signed GET)     │
 │                                        │         │   ├─ runAcceptanceSuite()  ▶ chromium  6/6     │
 │  POST /v1/inference/acceptance-verdicts ◀────────┼── postVerdict(token)                           │
 │   └─ handleAcceptanceVerdictCallback   │ verdict │   └─ POST /v1/inference/acceptance-jobs/ack    │
 │      (auth: ACCEPTANCE_VERDICT_CALLBACK_TOKEN)   │      (delivered ▶ remove | retry ▶ re-pending) │
 │   └─ backfill receipt: unverified ▶    │         └──────────────────────────────────────────────┘
 │      test_passed / failed, verified    │
 │   └─ (settlement sink fires IFF the    │   ← a SEPARATE lane owns the money path
 │      khala-loop flag + owner gate armed)│      (khala-loop-integration.ts); untouched here
 └───────────────────────────────────────┘
```

One shared bearer token (`ACCEPTANCE_VERDICT_CALLBACK_TOKEN`) authenticates the whole
runner↔gateway channel: the lease GET, the ack POST, and the verdict POST.

## What is built vs. what is a deploy step

**Built (in this PR):**

- `apps/acceptance-runner/` — the standalone long-running daemon:
  - `src/daemon.ts` — the poll loop: lease → run the canonical harness → POST verdict →
    ack. Fail-soft, constant-motion, graceful-stoppable.
  - `src/service.ts` — the daemon entrypoint (reads fail-closed config from env; refuses
    to start without its secrets).
  - `src/run-once.ts` — one-shot mode: run a local artifact through the real suite and
    optionally POST the verdict to a live callback (local proof / manual replay).
  - `src/http-job-source.ts` + `src/job-source.ts` — the authenticated HTTP pull source.
  - `src/config.ts` — fail-closed env config.
  - `src/harness-bridge.ts` — the single import boundary into the canonical worker harness
    (`apps/openagents.com/.../acceptance-runner/harness.ts` + `runner.ts`). The service
    forks NOTHING; it only orchestrates the existing executor.
  - `Dockerfile` — Playwright base image + Bun, for Cloud Run / GCE.
- Worker pull-queue side (INERT): `acceptance-job-queue-store.ts` (D1 + in-memory),
  `acceptance-job-lease-routes.ts` (authenticated lease/ack), migration
  `0222_khala_acceptance_job_queue.sql`, and the two routes wired into `index.ts`.
- A robustness fix to `runner.ts`: the single-press advance check now settles until the
  hop animation lands (`settleUntilStable`) instead of a fixed wait shorter than the
  artifact's hop, so a good artifact reads a full tile (the committed passing artifact now
  scores a clean **6/6**). The broken fixtures still fail honestly.

**Deploy steps (owner; NOT done here):**

1. Host the runner (recommended: **our GCE**, see below).
2. Mint + set the callback token on the Worker and the runner.
3. Wire the dispatch producer to the pull queue (the one remaining gateway seam).
4. Flip `KHALA_ACCEPTANCE_DISPATCH_ENABLED=true`.

## Host options (assessment)

| Host | Fit | Notes |
|------|-----|-------|
| **Our GCE (oa-codex-control + a Compute VM)** ✅ **recommended** | Best for an always-on PULL daemon | Matches the workspace rule "autonomous/unattended execution goes on OUR Google Cloud." A pull daemon has no inbound port, so it sidesteps Cloud Run's request/port model. chromium + system deps come free from the Playwright base image. One small VM runs the loop 24/7; restart via systemd. |
| Cloud Run | Workable but awkward | Cloud Run wants an HTTP server + scales to zero; our daemon PULLS and has no inbound port. You'd add a dummy health server and a min-instance=1 (always-warm) to keep it polling, which is just a worse always-on VM. Use only if you specifically want Cloud Run ops. |
| A Pylon node | Natural long-term home | A Pylon is already a programmatic chromium-capable environment and joins the verified-work revshare flywheel (workers run QC for pay). Good once the Pylon fleet is the execution substrate; for the FIRST live verdict, our GCE is the lowest-friction owner-operated box. |

**Recommendation: our GCE.** It is the workspace's canonical home for unattended
execution, fits a no-inbound-port pull daemon cleanly, and gets chromium for free from the
Playwright image.

### Concrete GCE steps

1. **Build + push the image** (build context = repo root):
   ```
   docker build -f apps/acceptance-runner/Dockerfile -t \
     us-docker.pkg.dev/openagentsgemini/oa/acceptance-runner:0.1.0 .
   docker push us-docker.pkg.dev/openagentsgemini/oa/acceptance-runner:0.1.0
   ```
2. **Run it on the GCE box** (Container-Optimized OS or a plain VM with Docker), with the
   host secrets injected as env (never baked into the image):
   ```
   docker run -d --restart=always --name oa-acceptance-runner \
     -e ACCEPTANCE_VERDICT_CALLBACK_URL=https://openagents.com/v1/inference/acceptance-verdicts \
     -e ACCEPTANCE_JOB_LEASE_URL=https://openagents.com/v1/inference/acceptance-jobs/lease \
     -e ACCEPTANCE_JOB_ACK_URL=https://openagents.com/v1/inference/acceptance-jobs/ack \
     -e ACCEPTANCE_VERDICT_CALLBACK_TOKEN="$(cat /run/secrets/acceptance_token)" \
     us-docker.pkg.dev/openagentsgemini/oa/acceptance-runner:0.1.0
   ```
   The token should come from GCP Secret Manager (project `openagentsgemini`), not a
   literal. `docker logs -f oa-acceptance-runner` shows the structured JSON poll log.
3. Until the Worker flags are flipped, the runner just polls and gets `204` (idle). That
   is the correct inert state.

## The exact flips to go live (in order)

Everything below is **NEEDS-OWNER**. Until all are done, the loop is inert: the producer
enqueues nothing, the callback/lease routes reject everything (no token), and the runner
idles on 204s.

1. **Mint the token** (a long random secret), e.g. `openssl rand -hex 32`.
2. **Set it on the Worker** (so the callback + lease/ack routes authenticate):
   ```
   cd apps/openagents.com/workers/api
   wrangler secret put ACCEPTANCE_VERDICT_CALLBACK_TOKEN
   ```
3. **Set the SAME token on the runner host** (Secret Manager / the `-e` above).
4. **Wire the dispatch producer to the pull queue.** Today `acceptanceDispatch.queue` is
   `undefined` in `index.ts` (two sites: `/v1/chat/completions` and `/mpp/v1/...`). Replace
   `queue: undefined` with a producer backed by `makeD1AcceptanceJobQueueStore(...)` (plus
   an R2 `storeArtifact` that writes the runnable HTML to the `ARTIFACTS` bucket and
   returns a signed GET URL as the `artifactRef`). This is the one remaining gateway seam;
   it is owned by the dispatch lane and left untouched here on purpose.
5. **Apply the migration** to prod D1:
   ```
   wrangler d1 migrations apply openagents-autopilot --remote
   ```
6. **Flip the dispatch flag** (`vars` in `wrangler.jsonc` or a deploy-time var):
   `KHALA_ACCEPTANCE_DISPATCH_ENABLED=true`. (`INFERENCE_GATEWAY_ENABLED` is already
   `true`, so the callback + lease routes arm as soon as the token is set.)
7. **Deploy the Worker.** Now: a khala-code completion enqueues a job → the runner leases
   it → runs the suite → POSTs the verdict → the receipt backfills `verified:true`.

The money path stays separately gated: even with all of the above, real settlement fires
only when the KHALA loop-arming flag AND the owner real-settlement gate are BOTH armed
(owned by `khala-loop-integration.ts`, untouched here).

## Local proof (already passing in this PR)

- `bun apps/acceptance-runner/src/run-once.ts scripts/khala-demo/artifacts/khala-crossy-road-northstar-passing.v1.html`
  → **6/6 verified**, exit 0 (runs the full job path: resolve → real headless suite →
  verdict payload).
- `bun test apps/acceptance-runner/src/e2e-local-proof.test.ts` → runs the REAL headless
  suite against the committed passing artifact, POSTs the verdict through the REAL
  `handleAcceptanceVerdictCallback` route (test token, in-memory store), and asserts the
  receipt backfills `verified:true` / `test_passed`.
- `bun test apps/acceptance-runner/src/daemon.test.ts` → the poll-loop wiring
  (lease→run→post→ack, idle/error backoff), browser-free.
- Worker side: `vitest run src/inference/acceptance-job-queue.test.ts` (queue + lease
  routes) and the existing `acceptance-dispatch.test.ts` full-loop fixtures stay green.
