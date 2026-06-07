# Nexus/Omega Cloudflare Boundary

Date: 2026-05-19
Updated: 2026-06-07

This is the production boundary for Nexus v0.2, Pylon v0.2, and any
Cloudflare-hosted OpenAgents web surface.

## End State

The current Pylon v0.2 release direction is MDK-default. Omega on Cloudflare is
the active product/payment control plane for checkout, Site commerce,
agent-visible receipts, public-safe proof, and MDK sidecar routing. Pylon wraps
MoneyDevKit `agent-wallet` locally for the normal provider wallet runtime.

Native LDK remains an explicit lower-level regression and hardening lane. The
old GCP-hosted native Nexus/LDK deployment is historical production context; it
is not the default release gate for the MDK-default Pylon v0.2 path unless a
task explicitly changes that native lane.

## Hosting Boundary

Cloudflare owns the current Omega/MDK control plane:

- `openagents.com`, Autopilot/Sites/Forum product surfaces, auth, and product
  navigation.
- Omega Worker routes for Site commerce, checkout intent creation, clean
  checkout returns, webhook verification, receipts, entitlements, public-safe
  proof, and agent-facing APIs.
- D1 ledgers for checkout intents, reconciliation events, receipts,
  entitlements, dispatch proposals, public projections, and idempotency state.
- R2 or equivalent object storage for proof bundles and generated artifacts.
- Durable Objects, Queues, and Workflows for per-run coordination,
  idempotency, retries, and long-running state transitions when they are added.
- Cloudflare Containers for Node/native MDK sidecars that cannot run inside a
  plain Worker.

Pylons own local execution and wallet edge state:

- local provider runtime;
- local MDK `agent-wallet` home;
- local compute/inference/training work;
- local receipts and operator-facing lifecycle state.

The old native Nexus/LDK lane owns only explicit native regression scope:

- native LDK channel telemetry and direct LDK hardening;
- old `nexus.openagents.com` GCP deployment diagnostics;
- transition reports for historical accepted-work payout proofs.

Cloudflare Workers must not store or log:

- raw wallet recovery phrases or mnemonic material in logs, public docs, D1
  public projection, issue comments, or browser responses;
- raw invoices beyond transient server-side request/response handling;
- raw payment hashes, preimages, checkout client secrets, or payout
  destination strings;
- private channel state, raw LDK node keys, or bitcoind RPC credentials;
- unrestricted treasury spend authority exposed to browser or unauthenticated
  routes.

Cloudflare Worker secrets may hold app-scoped MDK credentials for the current
MDK checkout sidecar, including the secrets required by the MDK hosted
platform. Those values are server-side authority and must stay in Cloudflare
secret storage or local ignored secret files only. They must not be projected
to users, generated Sites, Forum posts, public docs, or GitHub comments.

## Route Review

Read-only or redacted surfaces:

- `GET /v1/treasury/status`
  - Public treasury status read model.
  - Returns active provider, LDK readiness, balances, latency metrics, and
    operator actions.
  - Does not return raw payout targets, raw invoices, raw payment ids, API keys,
    seeds, operation rows, or private channel state.
- `GET /v1/treasury/operations/{operation_id}`
  - Redacted single-operation status.
  - Returns hashes and safe metadata only.
  - Intended for following funding-target latency and provider-operation
    lifecycle without exposing payment material.
- `GET /v1/treasury/projections`
- `GET /api/treasury/projections`
  - Authenticated projection surfaces for admin/operator visualization.
  - Accept the normal admin bearer token or treasury integration token.
  - Cannot initiate payments, channel changes, peer changes, or wallet refreshes.

Current MDK/Site commerce surfaces:

- `POST /api/mdk`
  - Worker route to the Cloudflare Container MDK sidecar.
  - Live only when route secrets and MDK app credentials are configured.
  - May create or inspect MDK checkout state through the sidecar.
  - Must not expose raw access tokens, mnemonic material, invoices,
    payment hashes, preimages, or checkout client secrets to browser callers.
- `POST /api/sites/:siteId/commerce/checkout-intents`
  - First-party checkout-intent authority.
  - Validates catalog, price, clean return paths, customer-data refs, and
    idempotency before contacting an MDK-compatible provider.
- `GET /api/sites/:siteId/commerce/checkout-returns/...`
  - Clean return state projection.
  - Reads durable checkout, receipt, and entitlement state without trusting
    checkout query strings as payment proof.
- `POST /api/sites/:siteId/commerce/mdk/webhooks`
  - Verifies exactly the configured webhook source before mutating state.
  - Writes replay-safe reconciliation, receipt, and entitlement records.

Historical native Nexus receive-material surface:

- `POST /v1/treasury/funding-target`
  - Creates an LDK receive target or BOLT11 invoice for funding Nexus.
  - Writes a durable `funding_invoice_creation` operation before the provider
    call and updates it to `completed` or `failed`.
  - It is not payment proof, spend authority, a channel command, or custody
    transfer. The response redacts provider payment ids as hashes.

Admin write surfaces:

- `POST /v1/admin/treasury/refresh`
- `POST /api/admin/treasury/refresh`
- `POST /v1/admin/treasury/operations`
- `POST /api/admin/treasury/operations`

These require the Nexus admin bearer token. Write commands also require an
idempotency key and record a durable `lightning_admin_command` operation row.
The operation row stores hashes of sensitive command inputs and safe metadata,
not raw invoices, API keys, seeds, private channel state, or bitcoind secrets.

## Cloudflare Facade Rules

If Omega or another Cloudflare Worker exposes Nexus/MDK functionality:

1. Read-only visualization routes may expose only redacted status/projection
   data.
2. Admin or payout-control routes must require WorkOS/API-token or agent-grant
   authorization before mutating state.
3. Admin writes, checkout creation, payout intents, and webhook mutations must
   be idempotent and receipt-able.
4. Worker logs and persisted state may include request ids, actor ids,
   operation ids, checkout refs, receipt ids, and hashed target ids only.
5. Worker state must not project raw MDK credentials, raw invoices, preimages,
   raw payment hashes, raw payout destinations, private channel material, or
   bitcoind credentials.
6. Browser code must never instantiate the production wallet runtime or sign
   Lightning payments.
7. Containers may host Node/native MDK code behind a binding; that does not
   grant generated Sites or public browser JavaScript direct payment authority.

## Verification

Use these checks before claiming the boundary is intact:

```bash
bash scripts/deploy/nexus/test-custody-boundary-guards.sh
cargo test -p nexus-control admin_treasury_operations -- --nocapture
```

For the MDK-default release path, also verify the Omega Cloudflare evidence:

```text
openagents.com Worker -> MDK_SIDECAR -> Cloudflare Container -> MDK platform
```

Required public-safe evidence is a signed sidecar health/ping, a real
bitcoin-denominated checkout, a local MDK wallet payment, provider checkout
status reaching a paid state, and a payer-wallet balance delta. Do not include
raw invoices, preimages, payment hashes, access tokens, mnemonics, or checkout
client secrets in public reports.

For live verification, use an admin token from the local ignored secrets file
and check that unauthorized admin calls fail, authorized read/admin calls return
redacted operation ids, and no response contains raw LDK custody material.
