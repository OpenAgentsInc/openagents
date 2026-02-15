# EP212 Phase 8: Production Rehearsal Log (2026-02-15)

Scope: prove EP212 “recording readiness” end-to-end against:

1. Third-party L402 seller: `https://sats4ai.com/api/l402/text-generation`
2. OpenAgents L402 gateway (paid): `https://l402.openagents.com/ep212/premium-signal`
3. OpenAgents L402 gateway (over-cap policy block): `https://l402.openagents.com/ep212/expensive-signal`

This is the acceptance gate for issue `#1633`.

## 1. Preconditions Verified

1. Wallet executor is deployed (Cloud Run, Spark mode) and healthy:
   - `GET /status` returns `ready=true`
2. L402 gateway is live:
   - `curl -i https://l402.openagents.com/ep212/premium-signal` returns `402` with `WWW-Authenticate: L402 ...`
3. Seller side is backed by self-hosted GCP `oa-lnd` (no Voltage dependency):
   - see `docs/lightning/status/20260215-ep212-aperture-gcp-lnd-cutover-log.md`

## 2. Funding The Buyer Wallet (Spark)

The deployed wallet executor balance was initially too low to run a paid rehearsal.

Funding approach used:

1. Generated a 5,000 sat BOLT11 invoice for the Spark wallet (using the same mnemonic + Spark API key as the deployed executor).
2. Paid it from `oa-lnd` using `lncli payinvoice --force`.
3. Waited for the Spark SDK to sync and for `/status` to reflect the updated balance.

Post-funding confirmation:

- `GET /status` shows `balanceSats ≈ 5000` and `lifecycle=connected`

## 3. Live Smoke Run (Non-interactive)

Command (no secrets):

```bash
cd apps/lightning-ops
OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL="https://<cloud-run-wallet-executor-url>" \
OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN="<bearer>" \
OA_LIGHTNING_OPS_EP212_SATS4AI_URL="https://sats4ai.com/api/l402/text-generation" \
OA_LIGHTNING_OPS_EP212_ROUTE_A_URL="https://l402.openagents.com/ep212/premium-signal" \
OA_LIGHTNING_OPS_EP212_ROUTE_B_URL="https://l402.openagents.com/ep212/expensive-signal" \
OA_LIGHTNING_OPS_EP212_MAX_SPEND_MSATS=100000 \
npm run smoke:ep212-full-flow -- --json --mode live --artifact-dir ../../output/lightning-ops/ep212-live
```

Artifacts:

- `output/lightning-ops/ep212-live/summary.json`
- `output/lightning-ops/ep212-live/events.jsonl`

Observed results (from `summary.json`):

- sats4ai:
  - paid success (`firstAmountMsats=42000` => 42 sats)
  - second request required payment again (`cacheHit=false`)
- OpenAgents paid route A:
  - paid success (`paidAmountMsats=10000` => 10 sats)
- OpenAgents route B:
  - blocked pre-payment with `amount_over_cap` (`quotedAmountMsats=250000` vs cap `100000`)
  - payer call count unchanged across the block

Note on caching:

- The harness supports both behaviors:
  - cache hit (no second payment) when a seller accepts reuse of the credential, and
  - a second paid attempt when a seller requires pay-per-request.
- In this run, `sats4ai.com` required a second payment for the repeated request. For the episode narrative, we can still demonstrate “no second payment” cache behavior using our own gateway route(s), where we control semantics.

## 4. UI Rehearsal Notes

The Worker secrets needed for `openagents.com` to pay via the wallet executor are present on the production `autopilot-web` Worker (verified via `wrangler secret list`):

- `OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL`
- `OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN`
- `OA_LIGHTNING_L402_ALLOWED_HOSTS`

Next step for a human operator: run the scripted chat flow from `docs/plans/active/lightning/212-demo-plan.md` on `openagents.com` and confirm:

1. approval prompt appears before paying
2. after approval, payment succeeds and premium payload is summarized
3. wallet + transaction panes show paid/blocked state and receipt references

