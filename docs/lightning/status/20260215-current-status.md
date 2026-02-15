# OpenAgents Lightning Status Snapshot (2026-02-15)

Status: **EP212-ready buyer + seller path implemented and deployed** (Cloudflare Worker + Convex control plane + Cloud Run wallet executor + Cloud Run Aperture + GCP LND + GCP Bitcoin Core).

This is a “what exists right now” snapshot (code + deployed infra) so operators can run the EP212 demo without guessing.

Related logs/runbooks:

- `docs/lightning/status/20260215-ep212-phase8-production-rehearsal-log.md` (live paid rehearsal + artifacts)
- `docs/lightning/status/20260215-ep212-aperture-gcp-lnd-cutover-log.md` (Aperture → GCP LND cutover + route-hints patch)
- `docs/lightning/runbooks/EP212_L402_BUYER_REHEARSAL_RUNBOOK.md` (demo rehearsal script)
- `docs/lightning/runbooks/L402_WALLET_EXECUTOR_DEPLOY_RUNBOOK.md` (wallet executor deploy)
- `docs/lightning/runbooks/L402_APERTURE_DEPLOY_RUNBOOK.md` (Aperture deploy/config)
- `docs/plans/active/lightning/212-demo-plan.md` (episode narrative + UI beats)

## 0) One-Paragraph System Summary

When a user in the `openagents.com` Autopilot chat requests a paid resource, Autopilot runs the `lightning_l402_fetch` tool inside the `apps/web` Cloudflare Worker. That tool creates a Convex “Lightning task”, does the L402 handshake (request → 402 → parse `WWW-Authenticate`), enforces spend policy/allowlist, then pays the BOLT11 invoice by calling a **separately deployed Cloud Run wallet executor** (`apps/lightning-wallet-executor`) which owns a Spark wallet (Breez Spark SDK). After payment, the Worker retries the HTTP request with `Authorization: L402 <macaroon>:<preimage>` (host-specific auth strategy), stores bounded response metadata (preview + sha256) in the task transition, and renders payment state cards + L402 panes in the `openagents.com` UI.

Seller side (our own L402 endpoints) is served by **Aperture on Cloud Run** (`l402-aperture`) behind `https://l402.openagents.com`, backed by **our self-hosted LND VM** (`oa-lnd`) which uses **our Bitcoin Core VM** (`oa-bitcoind`) as chain backend.

## 1) What Runs Where (Current)

| Layer | Where | What it does | Source of truth |
| --- | --- | --- | --- |
| Autopilot host runtime | Cloudflare Worker (`apps/web`) | Runs Autopilot + implements `lightning_l402_fetch` + performs L402 buy flow | Code in `apps/web/src/effuse-host/*` |
| Control plane state | Convex (`apps/web/convex/lightning/*`) | Stores Lightning tasks + transition receipts + hosted paywall state | Convex schema + mutations |
| Buyer wallet execution | Cloud Run (`l402-wallet-executor`) | Owns Spark wallet seed + pays BOLT11 invoices + returns preimage | `apps/lightning-wallet-executor` |
| Seller paywall proxy | Cloud Run (`l402-aperture`) + Cloudflare DNS `l402.openagents.com` | Issues L402 challenges + validates payment, then proxies upstream | Aperture config secret + patched image |
| Seller Lightning node | GCE VM `oa-lnd` | Invoice minting + Lightning connectivity | `lncli`/systemd on VM |
| Seller chain backend | GCE VM `oa-bitcoind` | Full Bitcoin node | `bitcoin-cli`/systemd on VM |
| Ops harness | Local CLI (`apps/lightning-ops`) | Deterministic smokes for EP212 routes + external sellers | Smoke programs + artifacts |

## 2) Deployed Infra (GCP)

GCP project: `openagentsgemini`  
Region/zone: `us-central1` / `us-central1-a`

### 2.1 Cloud Run

Services:

1. `l402-aperture`
2. `l402-wallet-executor`

Discover URLs:

```bash
gcloud run services list --region us-central1 --project openagentsgemini
gcloud run services describe l402-aperture --region us-central1 --project openagentsgemini --format='value(status.url)'
gcloud run services describe l402-wallet-executor --region us-central1 --project openagentsgemini --format='value(status.url)'
```

### 2.2 Compute Engine (VMs)

Instances (as of 2026-02-15):

- `oa-bitcoind` (machine type `n2-standard-8`, internal IP `10.42.0.2`, no external IP)
- `oa-lnd` (machine type `e2-standard-4`, internal IP `10.42.0.3`, external IP present)

List:

```bash
gcloud compute instances list --project openagentsgemini
```

## 3) Node Health (Bitcoin Core + LND)

### 3.1 `oa-bitcoind` status

Systemd:

```bash
gcloud compute ssh oa-bitcoind --zone us-central1-a --tunnel-through-iap --command \
  "sudo systemctl status bitcoind --no-pager"
```

Sync progress (authoritative):

```bash
gcloud compute ssh oa-bitcoind --zone us-central1-a --tunnel-through-iap --command \
  "sudo -u bitcoin bitcoin-cli -conf=/etc/bitcoin/bitcoin.conf -datadir=/var/lib/bitcoin getblockchaininfo"
```

Observed snapshot (2026-02-15):

- `chain=main`
- `blocks == headers` (fully synced)

### 3.2 `oa-lnd` status

Systemd:

```bash
gcloud compute ssh oa-lnd --zone us-central1-a --tunnel-through-iap --command \
  "sudo systemctl status lnd --no-pager"
```

Node info:

```bash
gcloud compute ssh oa-lnd --zone us-central1-a --tunnel-through-iap --command \
  "sudo lncli --lnddir=/var/lib/lnd --network=mainnet getinfo"
```

Wallet + channel balances:

```bash
gcloud compute ssh oa-lnd --zone us-central1-a --tunnel-through-iap --command \
  "sudo lncli --lnddir=/var/lib/lnd --network=mainnet walletbalance"

gcloud compute ssh oa-lnd --zone us-central1-a --tunnel-through-iap --command \
  "sudo lncli --lnddir=/var/lib/lnd --network=mainnet channelbalance"
```

Liquidity snapshot (2026-02-15):

- One active channel (bootstrap) opened in `docs/lightning/status/20260214-ep212-liquidity-bootstrap-log.md`
- This is intentionally minimal liquidity for EP212 rehearsal, not a routing-node setup

## 4) Seller: L402 Gateway (`l402.openagents.com`) via Aperture

Public gateway URL (Cloudflare in front of Cloud Run):

- `https://l402.openagents.com`

EP212 demo routes (current):

1. Under-cap paid route: `https://l402.openagents.com/ep212/premium-signal` (configured ~10 sats in the current rehearsal config)
2. Over-cap route: `https://l402.openagents.com/ep212/expensive-signal` (configured ~2500 sats to force a policy block)

Quick check (expects `402` + `WWW-Authenticate` containing `L402 ... invoice="..." macaroon="..."`):

```bash
curl -i https://l402.openagents.com/ep212/premium-signal | rg -n "HTTP/|www-authenticate"
curl -i https://l402.openagents.com/ep212/expensive-signal | rg -n "HTTP/|www-authenticate"
```

Critical implementation details (already done):

1. **Private invoices w/ route hints** (required when `oa-lnd` uses private channels):
   - Patched Aperture image forces `private=true` when minting invoices so payers can find a route. See `docs/lightning/status/20260215-ep212-aperture-gcp-lnd-cutover-log.md`.
2. **Authorization header sanity** (Cloudflare 400 protection):
   - Patched Aperture forwards a single `Authorization` upstream to avoid Cloudflare rejecting duplicated auth headers.
3. Amount inference:
   - Our L402 buyer infers amount from the invoice HRP when `amount_msats` is omitted. See `packages/lightning-effect/src/l402/parseChallenge.ts`.

## 5) Buyer: Cloud Run Wallet Executor (Spark)

Service: `l402-wallet-executor`  
Code: `apps/lightning-wallet-executor`

What it does:

- Owns a Spark wallet seed (mnemonic stored in GCP Secret Manager).
- Exposes HTTP:
  - `GET /healthz` (unauthenticated health)
  - `GET /status` (bearer auth if configured)
  - `POST /pay-bolt11` (bearer auth if configured)
- Enforces:
  - allowed hosts (where invoices are allowed to be paid)
  - request cap and rolling window cap (msats)

How Autopilot calls it:

- `apps/web` Worker reads:
  - `OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL`
  - `OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN`
  - `OA_LIGHTNING_WALLET_EXECUTOR_TIMEOUT_MS`
  - `OA_LIGHTNING_L402_ALLOWED_HOSTS`
- Worker uses `apps/web/src/effuse-host/lightningL402Executor.ts` to pay invoices through the wallet executor.

Operator status check (do not paste tokens into logs):

```bash
WALLET_URL="$(gcloud run services describe l402-wallet-executor --region us-central1 --project openagentsgemini --format='value(status.url)')"
AUTH_TOKEN="$(gcloud secrets versions access latest --secret=l402-wallet-executor-auth-token --project openagentsgemini)"
curl -sS -H "Authorization: Bearer ${AUTH_TOKEN}" "${WALLET_URL}/status"
```

Observed snapshot (2026-02-15):

- Spark wallet is `ready=true` and has a non-zero balance (funded for rehearsal). See `docs/lightning/status/20260215-ep212-phase8-production-rehearsal-log.md`.

## 6) openagents.com UI + Autopilot Tooling

### 6.1 Tool surface (real production Autopilot UI)

- `lightning_l402_fetch` (create task, enforce approval gating, execute L402 buy via wallet executor)
- `lightning_l402_approve` (explicit user approval step; transitions the task)

Implementation:

- Worker tool host: `apps/web/src/effuse-host/autopilot.ts`
- L402 execution: `apps/web/src/effuse-host/lightningL402Executor.ts`
- Worker router: `apps/web/src/effuse-host/worker.ts`

### 6.2 Chat rendering + panes

What the user sees:

1. A payment state card in chat (“intent”, “sent”, “cached”, “blocked”, “failed”) with proof refs and bounded payload previews.
2. L402 panes in the UI:
   - Wallet summary (derived from chat metadata)
   - Recent attempts / transaction list
   - Hosted paywalls / settlements / deployments panes (for the hosted seller track)

Key code:

- Payment card parsing: `apps/web/src/effuse-app/controllers/autopilotChatParts.ts`
- Pane controller: `apps/web/src/effuse-app/controllers/home/openChatPaneController.ts`

Storybook coverage:

- `apps/web/src/storybook/stories/lightning.ts` (payment cards + L402 panes states)

Important current limitation:

- The “wallet summary” pane currently summarizes attempts/spend from tool metadata; it does **not** call the wallet executor `/status` to show live balance. (This is sufficient for EP212 narrative if you show the payments + proof refs, but it is not a live balance UI.)

## 7) Programmatic Verification (No UI)

### 7.1 EP212 routes smoke (our gateway)

This verifies:

1. `l402.openagents.com` returns 402 challenges
2. under-cap route pays and returns 200
3. over-cap route is blocked **before payment**

```bash
cd apps/lightning-ops

export OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL="$(gcloud run services describe l402-wallet-executor --region us-central1 --project openagentsgemini --format='value(status.url)')"
export OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN="$(gcloud secrets versions access latest --secret=l402-wallet-executor-auth-token --project openagentsgemini)"

npm run smoke:ep212-routes -- --json --mode live
```

### 7.2 EP212 full-flow smoke (external seller + our gateway)

This verifies:

1. third-party L402 seller payment (e.g. sats4ai)
2. our paid route
3. over-cap policy block

```bash
cd apps/lightning-ops

OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL="$(gcloud run services describe l402-wallet-executor --region us-central1 --project openagentsgemini --format='value(status.url)')" \
OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN="$(gcloud secrets versions access latest --secret=l402-wallet-executor-auth-token --project openagentsgemini)" \
OA_LIGHTNING_OPS_EP212_SATS4AI_URL="https://sats4ai.com/api/l402/text-generation" \
OA_LIGHTNING_OPS_EP212_ROUTE_A_URL="https://l402.openagents.com/ep212/premium-signal" \
OA_LIGHTNING_OPS_EP212_ROUTE_B_URL="https://l402.openagents.com/ep212/expensive-signal" \
OA_LIGHTNING_OPS_EP212_MAX_SPEND_MSATS=100000 \
npm run smoke:ep212-full-flow -- --json --mode live --artifact-dir ../../output/lightning-ops/ep212-live
```

Artifacts:

- `output/lightning-ops/ep212-live/summary.json`
- `output/lightning-ops/ep212-live/events.jsonl`

Note on caching:

- Some third-party L402 sellers appear to require pay-per-request even for repeated identical requests; do not assume a cache-hit narrative unless the seller supports it. For EP212, we can demonstrate cache semantics using our own gateway where we control behavior.

## 8) How To Verify In The Deployed UI (openagents.com)

This is the “recording readiness” UI flow:

1. Open `https://openagents.com` and start an Autopilot chat.
2. Ask for an EP212 paid resource (per `docs/plans/active/lightning/212-demo-plan.md`).
3. Confirm the UI shows an approval prompt before payment.
4. Approve; confirm the payment card transitions to paid/cached, and Autopilot summarizes the bounded premium payload preview.
5. Open the L402 panes:
   - Wallet pane should show increased attempts/spend counts
   - Transactions pane should show the latest paid attempt with `proofReference`

Operational prerequisites:

- `autopilot-web` Worker must have secrets set (verify via `wrangler secret list --name autopilot-web`):
  - `OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL`
  - `OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN`
  - `OA_LIGHTNING_L402_ALLOWED_HOSTS`
  - `OA_LIGHTNING_WALLET_EXECUTOR_TIMEOUT_MS`

## 9) Known Gaps / Follow-Ups (Not Blocking EP212 Smoke)

1. UI live balance:
   - Add a Worker-proxied endpoint that calls wallet executor `/status` and display `balanceSats` + `lifecycle` in the wallet pane.
2. Cache demo semantics:
   - Ensure the demo’s “second call is cached” is shown using an endpoint that actually supports credential reuse, or explicitly narrate pay-per-request behavior when it doesn’t.
3. Liquidity hardening:
   - Current LND channel/liquidity is minimal and oriented around “EP212 works once”, not long-term reliability.

