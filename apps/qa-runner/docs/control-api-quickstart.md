# QA Control API — curl quick-start (#6196)

Drive the **full autonomous-QA / eval flow over HTTP** — submit → run → fetch
artifacts + verdict + `/pro` link — instead of the CLI. This is the
"do everything via API" surface a third party (or `executor.sh`'s CI) can wire
into.

## Why a daemon (not a Cloudflare Worker)

The runner drives a **real Chrome via Playwright**, which cannot run inside a
Cloudflare Worker isolate. So the control API is a **qa-runner HTTP daemon** that
runs on a machine **with Chrome**. It runs the existing runner/evals engine
**in-process**, async, with an **in-memory job store**. The Worker side (`/pro`)
only *dereferences* the public-safe artifacts this daemon produces.

## Start the daemon

```bash
cd apps/qa-runner

# Mock-only (deterministic; no Chrome, no network, no spend) — what tests + this
# quick-start use. A Khala agent token allowlist is REQUIRED (fail closed).
QA_CONTROL_PORT=8799 \
QA_CONTROL_STORE_DIR=./runs/control \
QA_CONTROL_TOKENS="raynor:tok_demo_secret" \
  bun run api
# -> {"kind":"qa_control_api","message":"qa-runner control API listening","url":"http://127.0.0.1:8799",...}
```

Environment:

| Var | Default | Meaning |
|---|---|---|
| `QA_CONTROL_PORT` | `8787` | listen port |
| `QA_CONTROL_HOSTNAME` | `127.0.0.1` | bind host |
| `QA_CONTROL_STORE_DIR` | `./runs/control` | artifact store root (one subdir per job) |
| `QA_CONTROL_PRO_BASE_URL` | `https://openagents.com` | base for `/pro` links |
| `QA_CONTROL_TOKENS` | *(empty → fail closed)* | comma-separated `agent:token` allowlist |
| `QA_CONTROL_ARM_REAL` | *(unset)* | `1` to allow real (network/spend) runs |
| `QA_CONTROL_TOKEN_BUDGET` | `0` | default per-run token cap for real runs |

**No fake green:** with an empty token allowlist every request is rejected
(401); a real run is refused (403 `not_armed`) unless `QA_CONTROL_ARM_REAL=1`;
the mock path is honestly marked `spendCapable: false` / `decisionGrade: false`.

## Auth

Every endpoint except `GET /healthz` requires a **Khala agent bearer token**:

```bash
-H "Authorization: Bearer tok_demo_secret"
```

A missing/invalid token returns `401` in an OpenAI-style error envelope:

```bash
curl -s -X POST http://127.0.0.1:8799/runs
# 401
# { "error": { "message": "missing bearer token", "type": "invalid_request_error", "code": "invalid_api_key" } }
```

## Submit a run → poll → fetch artifacts (purely via curl)

```bash
B=http://127.0.0.1:8799
T=tok_demo_secret

# 1) submit
RID=$(curl -s -X POST $B/runs \
  -H "Authorization: Bearer $T" -H "content-type: application/json" \
  -d '{"scenario":"login-regression","commitments":["own-infra"]}' \
  | grep -o '"id": "[^"]*"' | head -1 | sed 's/.*"id": "//;s/"//')
echo "run id = $RID"

# 2) poll status until succeeded/failed
curl -s $B/runs/$RID -H "Authorization: Bearer $T"
# { "object": "qa_control.run", "status": "succeeded", "mode": "mock", "receipt": {...}, ... }

# 3) fetch artifacts: video, committed test ref, result.json (incl. additive
#    `verify` verdict + `receipt` if present), and the /pro/runs/:id link
curl -s $B/runs/$RID/artifacts -H "Authorization: Bearer $T"
```

Artifacts response (abridged):

```jsonc
{
  "object": "qa_control.run_artifacts",
  "proUrl": "https://openagents.com/pro/runs/<id>",   // the shareable /pro link
  "video": "session.webm",
  "videoFormat": "webm",
  "trace": "trace.zip",
  "screenshots": ["00-login-page.png"],
  "committedTest": null,            // the distiller's e2e test ref, when emitted
  "result": { "status": "pass", "...": "...", "receipt": { "...": "..." } },
  "verify": null,                   // additive verify verdict (peer lane) if present:
                                    //   "CONFIRMED" | "REFUTED" | "INCONCLUSIVE"
  "receipt": {                      // additive run receipt (already landed)
    "schemaVersion": "openagents.qa_runner.receipt.v1",
    "verificationClass": "exact_trace_replay",
    "resultPath": "result.json"
  },
  "jobReceipt": { "mode": "mock", "spendCapable": false, "tokenBudget": 0, "tokensSpent": 0 }
}
```

`verify` and `receipt` are **read-only passthrough**: this API reads them off
`result.json` if present (another lane owns `verify`; `receipt` already landed)
and never defines or mutates them. `verify: null` means a verdict was not
written — honest, not fabricated.

## Submit an eval (≥ 2 variants) → fetch the comparison

```bash
EID=$(curl -s -X POST $B/evals \
  -H "Authorization: Bearer $T" -H "content-type: application/json" \
  -d '{
    "title": "Login: baseline vs candidate",
    "variants": [
      { "id": "baseline",  "scenario": "login-regression" },
      { "id": "candidate", "scenario": "login-regression-wrong", "note": "regressed" }
    ]
  }' | grep -o '"id": "[^"]*"' | head -1 | sed 's/.*"id": "//;s/"//')

curl -s $B/evals/$EID -H "Authorization: Bearer $T"
# { "object": "qa_control.eval_comparison",
#   "proUrl": "https://openagents.com/pro/evals/<id>",
#   "comparison": { "variants": [ {passRate:1,...}, {passRate:0,...} ],
#                   "deltas": [...], "decisionGrade": false } }
```

## Submit a QA Swarm run → fetch the projection

One API call starts the hosted-run composition: qa-runner control fanout,
FleetRun-style caps, the nightly-matrix projection vocabulary, and a
`/qa/{runRef}` share URL. The fixture tier is no-spend by default; GCE Tier-2
and CF Browser Rendering live tiers are represented as skip-safe tier rows
unless the daemon is armed and a live runner receipt is attached.

```bash
SID=$(curl -s -X POST $B/swarm-runs \
  -H "Authorization: Bearer $T" -H "content-type: application/json" \
  -d '{
    "target": "https://openagents.com",
    "targetName": "openagents.com",
    "maxWorkers": 2,
    "maxRuns": 1
  }' | grep -o '"id": "[^"]*"' | head -1 | sed 's/.*"id": "//;s/"//')

curl -s $B/swarm-runs/$SID -H "Authorization: Bearer $T"
# { "object": "qa_control.swarm_run_artifacts",
#   "qaShareUrl": "https://openagents.com/qa/qa-run.swarm.openagents.com.<id>",
#   "swarm": {
#     "projection": { "schemaVersion": "openagents.qa_swarm.run_projection.v1", ... },
#     "tiers": [
#       { "backend": "fixture", "status": "passed", ... },
#       { "backend": "gce-tier-2", "status": "skipped", ... },
#       { "backend": "cf-browser-rendering", "status": "skipped", ... }
#     ]
#   } }
```

## Real (gated) runs

A `real` run drives **real Chrome against the live Target** and is **owner-gated**:

```bash
# refused unless the daemon is armed:
curl -s -X POST $B/runs -H "Authorization: Bearer $T" \
  -d '{"real":true,"target":"https://openagents.com"}'
# 403 { "error": { "code": "not_armed", ... } }

# arm it (a machine WITH Chrome; `bun run playwright:install` once):
QA_CONTROL_ARM_REAL=1 QA_CONTROL_TOKEN_BUDGET=20000 \
QA_CONTROL_TOKENS="raynor:tok_demo_secret" bun run api
```

## Endpoints

| Method | Path | Auth | Purpose |
|---|---|---|---|
| `GET` | `/healthz` | none | liveness |
| `POST` | `/runs` | bearer | submit a run → `202` `{ id, status, mode, receipt }` |
| `GET` | `/runs/:id` | bearer | run status |
| `GET` | `/runs/:id/artifacts` | bearer | video + committed test + `result.json` (+ `verify`/`receipt`) + `/pro` link |
| `POST` | `/evals` | bearer | submit a ≥2-variant comparison → `202` |
| `GET` | `/evals/:id` | bearer | comparison + `/pro/evals/:id` link |
| `POST` | `/swarm-runs` | bearer | submit a QA Swarm hosted-run composition → `202` |
| `GET` | `/swarm-runs/:id` | bearer | QA Swarm projection + `/qa/{runRef}` share URL + tier statuses |

OpenAI-compatible shapes where they fit: errors use the
`{ error: { message, type, code } }` envelope; submit/status responses carry a
stable `object` discriminator. The inference itself (khala-openrouter) is already
OpenAI-compatible; this surface mirrors that envelope so existing clients' error
handling works.

## Tests (mocks, no spend)

```bash
cd apps/qa-runner
bun test src/control-auth.test.ts src/artifacts.test.ts src/control.test.ts src/api-server.test.ts
# or the whole app: bun run test
```
