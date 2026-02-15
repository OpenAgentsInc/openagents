# EP212: Aperture -> GCP LND Cutover Log (2026-02-15)

This log captures the work required to run the EP212 L402 gateway at `l402.openagents.com` with **Aperture on Cloud Run** and **LND on a self-managed GCE VM** (no Voltage dependency for the gateway’s LND backend).

## Objective

- Serve EP212 demo routes via Aperture at `https://l402.openagents.com/...`.
- Back invoice minting with our own LND instance on GCP.
- Ensure invoices are payable even when the LND node uses **private channels** (route hints).
- Ensure the gateway works behind Cloudflare (no duplicate `Authorization` headers to upstream).
- Provide deterministic, automated verification via `apps/lightning-ops`.

## What Changed (Repo)

### 1. Aperture image patched (route-hints + header forwarding)

Files:
- `docs/lightning/deploy/Dockerfile.aperture`
- `docs/lightning/deploy/aperture-private-invoices.patch`

Patch behavior:
1. **Private invoices**: invoice creation is forced to `private=true` so LND embeds **route hints** when the node’s channels are private. Without route hints, external payers can fail with `NO_PATH_FOUND` even when a private channel exists.
2. **Single Authorization forwarded upstream**: Aperture is adjusted to forward a *single* `Authorization` value to upstream (the `L402 ...` value). Cloudflare can reject requests when multiple `Authorization` headers are present (e.g. both LSAT and L402), producing `400` responses.

### 2. L402 challenge parsing now infers amount from invoice when missing

Files:
- `packages/lightning-effect/src/l402/parseChallenge.ts`
- `packages/lightning-effect/test/l402-parser.test.ts`

Reason:
- Aperture’s challenge header does not necessarily include `amount_msats`.
- Our buyer policy needs `quotedAmountMsats` to enforce caps and block expensive routes *before* payment.
- We now infer msats from the BOLT11 HRP (e.g. `lnbc10u...`) when the header omits `amount_msats`.

### 3. EP212 routes smoke uses colon auth for `l402.openagents.com`

Files:
- `apps/lightning-ops/src/programs/smokeEp212Routes.ts`

Reason:
- Our EP212 Aperture gateway expects `Authorization: L402 <macaroon>:<preimage>` (colon form).
- The smoke runner now pins `authorizationHeaderStrategyByHost["l402.openagents.com"]="macaroon_preimage_colon"`.

## What Changed (GCP / Runtime)

### Topology

- **Cloud Run**: `l402-aperture`
  - Reads config from Secret Manager: `l402-aperture-config`
  - Reaches LND over **VPC connector** + internal firewall to the VM
- **GCE VM**: `oa-lnd` (mainnet LND)
  - gRPC `:10009` (used by Aperture invoice service)
  - REST `:8080` (optional)

### Secret Manager inputs

These secrets are mounted into Cloud Run:
- `l402-gcp-lnd-tls-cert`
- `l402-gcp-lnd-invoice-macaroon`
- `l402-aperture-config`

### EP212 route pricing (temporary)

The under-cap demo route is currently configured to **10 sats** (10,000 msats) to support rehearsal with a low funded Spark wallet executor.

Config template:
- `docs/lightning/scripts/aperture-gcp-config-postgres.yaml`

If we want to restore the original value (e.g. 70 sats), update the template, create a new Secret Manager version for `l402-aperture-config`, then redeploy Cloud Run (see runbook).

## Verification

### 1. Confirm the gateway returns a 402 challenge

```bash
curl -i https://l402.openagents.com/ep212/premium-signal | rg -n \"HTTP/|www-authenticate\"
```

Expected:
- HTTP `402`
- `WWW-Authenticate: L402 ... invoice=\"...\" macaroon=\"...\"` present

### 2. Confirm route hints are present in the invoice (required for private channels)

This is the critical check that prevents `NO_PATH_FOUND` for external payers.

```bash
# 1) Fetch the invoice from the challenge header (manual copy is fine for ops checks)
curl -i https://l402.openagents.com/ep212/premium-signal | rg -n \"www-authenticate\"

# 2) Decode on the LND VM
gcloud compute ssh oa-lnd --zone us-central1-a --tunnel-through-iap --command \\
  \"sudo lncli --lnddir=/var/lib/lnd --network=mainnet decodepayreq --pay_req '<PASTE_BOLT11>'\"
```

Expected in decoded invoice JSON:
- `destination` matches the `oa-lnd` node pubkey
- `route_hints` is **non-empty**
- amount corresponds to config (currently 10 sats for the under-cap route)

### 3. Run automated live smoke (paid success + over-cap block)

```bash
cd apps/lightning-ops

export OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL=\"$(gcloud run services describe l402-wallet-executor --region us-central1 --project openagentsgemini --format='value(status.url)')\"
export OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN=\"$(gcloud secrets versions access latest --secret=l402-wallet-executor-auth-token --project openagentsgemini)\"

npm run smoke:ep212-routes -- --json --mode live
```

Expected:
- Route A: `402 -> paid -> 200`, with `paidAmountMsats=10000` (when configured to 10 sats)
- Route B: `402 -> blocked`, with `quotedAmountMsats` above cap, and **no payer calls** after the block

## Notes / Next

- This cutover is sufficient for EP212 buyer-demo rehearsal and for programmatic verification (`smoke:ep212-routes`).
- Next operational risks for the episode are liquidity and wallet funding (separate from this cutover log).
- For redeploy or config changes, follow:
  - `docs/lightning/runbooks/L402_APERTURE_DEPLOY_RUNBOOK.md`
