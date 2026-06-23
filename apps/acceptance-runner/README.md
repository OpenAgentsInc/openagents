# @openagentsinc/acceptance-runner

The out-of-Worker headless **Khala acceptance-verdict runner** (EPIC #6017).

It is the gating piece for the live auto-settlement loop: **no runner → no executed
verdicts → no verified outcomes → no payouts.** A Cloudflare Worker cannot run chromium,
so this long-running Bun service runs the real Playwright acceptance suite OUT of the
Worker: it leases a khala-code acceptance job from the gateway, runs the suite against the
produced artifact in a real headless chromium, and POSTs the `AcceptanceVerdict` back to
the authenticated callback so the receipt backfills `verified:true`.

It **orchestrates** the canonical acceptance harness
(`apps/openagents.com/workers/api/src/inference/acceptance-runner/`) — it forks nothing.

## INERT by default

The service reads a fail-closed config from env and **refuses to start** without:

| env | meaning |
|-----|---------|
| `ACCEPTANCE_VERDICT_CALLBACK_URL` | the Worker verdict callback (POST target) |
| `ACCEPTANCE_JOB_LEASE_URL` | the Worker job-lease endpoint (GET) |
| `ACCEPTANCE_JOB_ACK_URL` | the Worker job-ack endpoint (POST) |
| `ACCEPTANCE_VERDICT_CALLBACK_TOKEN` | the shared runner bearer token (never logged) |

Optional tuning: `ACCEPTANCE_POLL_INTERVAL_MS`, `ACCEPTANCE_IDLE_BACKOFF_MS`,
`ACCEPTANCE_NAV_TIMEOUT_MS`.

## Run

```sh
# one-shot: run a local artifact through the real suite (local proof / manual replay)
bunx playwright install chromium
bun run src/run-once.ts ../../scripts/khala-demo/artifacts/khala-crossy-road-northstar-passing.v1.html

# one-shot with a live callback (POSTs the verdict back)
ACCEPTANCE_VERDICT_CALLBACK_TOKEN=<tok> \
  bun run src/run-once.ts <artifact.html> --request-id <id> \
  --callback-url https://openagents.com/v1/inference/acceptance-verdicts

# the long-running daemon (reads the env table above)
bun run src/service.ts

# tests (browser-free daemon wiring + real-chromium end-to-end proof)
bun test src/daemon.test.ts
bun test src/e2e-local-proof.test.ts
```

## Deploy

See **`docs/DEPLOY.md`** for the full chain diagram, host assessment (recommended: our
GCE), the concrete image/build/run steps, and the exact owner flips to go live.
