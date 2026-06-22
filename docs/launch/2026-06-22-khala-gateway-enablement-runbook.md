# Khala Gateway Enablement — One-Shot Runbook

> **STATUS 2026-06-22: STAGING ENABLED + SERVING LIVE.** Owner refreshed wrangler
> to a Workers-write OAuth token; provider secrets (`FIREWORKS_API_KEY`,
> `VERTEX_SA_KEY`) set on `openagents-staging`, D1 migrated, Worker deployed
> (`INFERENCE_GATEWAY_ENABLED="true"`, version `c3ce41bc…`). `GET /v1/models`
> lists `khala-mini` + `khala-code`; a `gemini-3.5-flash` completion returns
> **HTTP 200** with real usage — the gateway genuinely serves. **Remaining:** a
> paid `khala-code`/`khala-mini` completion `402`s on a fresh agent
> (`insufficient_credits`) — it needs a **funded balance** (Bitcoin/Spark, the M3
> step + the guinea-pig payout lever), not a gateway change. **Prod** is still on
> a pre-#6018 build; redeploy with the same recipe (prod already has the provider
> secrets + a valid agent token). The original NEEDS-OWNER write-creds blocker
> below is **resolved**.

*2026-06-22. The exact steps to take Khala from "merged but not live" to a real
metered completion + receipt, and the first Bitcoin/Spark payout to the
guinea-pig Pylon. Everything is staged and ready; the ONLY missing piece is a
**write-scoped Cloudflare credential**.*

## State (verified 2026-06-22)

- The gateway is **deployed and live in prod** — `https://openagents.com/v1/models`
  returns the catalog — but prod runs a **pre-#6018 build**: it does **not** list
  `openagents/khala-*`. So enabling Khala = **redeploy current `main`** (which has
  khala-mini #6018 + khala-code + the `openagents` receipt block) + provider
  secrets present + a smoke. `INFERENCE_GATEWAY_ENABLED` is already `"true"` in
  both prod and `env.staging` config — no flag flip needed.
- **Blocker (hard): no write-scoped Cloudflare credential is available to agents.**
  - The wrangler OAuth login is `arcadecd@gmail.com` with **`account (read)` only**.
  - `.secrets/cloudflare-openagents.env` (`CLOUDFLARE_API_TOKEN`) returns
    **Authentication error 10000** on `workers/scripts/.../secrets` for account
    `54fac8b750a29fdda9f2fa0f0afaed90` — not authorized for Workers edit.
  - No CF Workers token found in GCP Secret Manager (`openagentsgemini`, 54 secrets).
- Everything else is in hand: provider keys (`.secrets/fireworks.env` →
  `FIREWORKS_API_KEY`; `.secrets/vertex-sa-inference.json` → `VERTEX_SA_KEY`), an
  agent token for the smoke (`.secrets/openagents-artanis-agent.env` →
  `OPENAGENTS_AGENT_TOKEN`), and the guinea-pig payout target
  (`.secrets/khala-test-payout.env`). `gcloud` is authed (project
  `openagentsgemini`) for the M6 compute lane.

## NEEDS-OWNER — provide ONE of these, then the rest is automatic

1. **Refresh wrangler to a write-scoped OAuth token** (simplest): in the prompt, run
   `! cd /Users/christopherdavid/work/openagents/apps/openagents.com/workers/api && bunx wrangler login`
   and approve Workers Scripts:Edit. Then an agent can run §"Enable on staging".
2. **OR** drop a `CLOUDFLARE_API_TOKEN` with **Workers Scripts:Edit** (account
   `54fac8b75…`) into `/Users/christopherdavid/work/.secrets/cloudflare-openagents.env`.
3. **OR** run §"Enable on staging" yourself (the commands below).

## Enable on staging (safe target; own D1/R2, separate URL)

All commands from a **clean worktree off `origin/main`** (deploy rule), with the
deploy token + account id exported:

```bash
cd /Users/christopherdavid/work/openagents && git fetch origin main \
  && git worktree add -b deploy/khala-staging /tmp/oa-deploy origin/main
cd /tmp/oa-deploy && bun install
export CLOUDFLARE_API_TOKEN="$(grep -E '^CLOUDFLARE_API_TOKEN=' /Users/christopherdavid/work/.secrets/cloudflare-openagents.env | cut -d= -f2-)"
export CLOUDFLARE_ACCOUNT_ID=54fac8b750a29fdda9f2fa0f0afaed90

# 1. provider secrets on the staging Worker (values piped, never printed)
cd apps/openagents.com/workers/api
grep -E '^FIREWORKS_API_KEY=' /Users/christopherdavid/work/.secrets/fireworks.env | cut -d= -f2- \
  | bunx wrangler secret put FIREWORKS_API_KEY --env staging
cat /Users/christopherdavid/work/.secrets/vertex-sa-inference.json \
  | bunx wrangler secret put VERTEX_SA_KEY --env staging

# 2. D1 migrations + deploy staging (UI assets are MANDATORY or you ship stale UI)
bunx wrangler d1 migrations apply openagents-autopilot-staging --remote --env staging
cd /tmp/oa-deploy && bun run build:web
cd apps/openagents.com/workers/api && bunx wrangler deploy --env staging --assets ../../apps/web/dist
```

## M0 live smoke (proves #6008 acceptance)

```bash
TOKEN="$(grep -E '^OPENAGENTS_AGENT_TOKEN=' /Users/christopherdavid/work/.secrets/openagents-artanis-agent.env | cut -d= -f2-)"
BASE=https://openagents-staging.openagents.workers.dev   # staging
curl -sS "$BASE/v1/models" | grep -o 'openagents/khala-[a-z]*' | sort -u   # expect khala-mini, khala-code
curl -sS -X POST "$BASE/v1/chat/completions" \
  -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d '{"model":"openagents/khala-code","messages":[{"role":"user","content":"build a really high quality single html file crossy road game with three.js"}]}' \
  | python3 -c 'import sys,json;d=json.load(sys.stdin);print(json.dumps(d.get("openagents",{}),indent=2));print("CHARS",len(d.get("choices",[{}])[0].get("message",{}).get("content","")))'
```

Acceptance: the response carries the `openagents` block (`requested_model`,
`served_model`, `worker`, `lane`, `verification`, `cost_msat`, `settled`) and a
real completion. `khala-code` serves on the Fireworks lane (key set above);
`khala-mini` serves on the Vertex-Gemini lane (`VERTEX_SA_KEY`). That closes M0's
live acceptance → comment + close #6008.

## First Bitcoin/Spark payout to the guinea-pig Pylon (M3 live arm)

Only after M0 serves and the Loop-Integration PR (verified-serve → settlement
sink, see EPIC #6017) is merged. **Owner-armed, tiny-capped:** set the bounded
real-settlement gate env (allowlist the Khala run, per-payout + daily caps in the
single-digit-sats range), register the guinea-pig Spark target
(`KHALA_TEST_PAYOUT_SPARK_ADDRESS`), and run one verified serve. Expect a settled
`realBitcoinMoved:true` receipt to that node. Keep it bounded — it is real money.

## Prod (after staging proves out)

Same as staging without `--env staging` (prod is already provisioned, so it's a
code update): set the secrets on the prod Worker, `wrangler d1 migrations apply
openagents-autopilot --remote`, `bun run build:web`, `wrangler deploy --assets
../../apps/web/dist`. Prod redeploy touches the live product — do it from clean
`origin/main` with the `--assets` flag (mandatory) and watch the deploy.

## Why this is honest

Agents wired the whole loop (catalog, receipt block, fabric adapter, settlement
leg) and verified it in tests, but no agent has a credential that can deploy
Cloudflare or set Worker secrets. This runbook is the precise hand-off: one
write-scoped credential (or one owner-run block) turns merged code into a live,
metered, Bitcoin-paid loop.
