# Nexus Cloudflare Boundary

Date: 2026-05-19

This is the production boundary for Nexus v0.2, Pylon v0.2, and any
Cloudflare-hosted OpenAgents web surface.

## End State

Nexus and the LDK node remain the server-side payment authority. Cloudflare can
host product UI, auth, read-only projections, visualizations, rate limits, and
thin admin facades, but it must not hold LDK custody material or execute
Lightning payment signing.

## Hosting Boundary

Google Cloud owns long-running Bitcoin and Lightning infrastructure:

- `bitcoind` and Bitcoin Core RPC.
- `ldk-server` and its node identity.
- LDK seed, TLS material, API key, SQLite state, channel state, payment event
  stream, and wallet backups.
- Nexus treasury authority, payout dispatch, channel/peer admin commands, and
  durable operation rows.

Cloudflare owns web and edge coordination:

- `openagents.com`, Autopilot UI, auth, and product navigation.
- Read-only status and projection views for channels, liquidity, payments,
  Pylons, payout receipts, and degraded states.
- React/Three visualizations of the OpenAgents Lightning/Pylon state.
- Thin API/admin facades that authenticate users and delegate write operations
  to Nexus without storing custody material.

Cloudflare Workers must not store or log:

- LDK seed, node key, API key, TLS key, or wallet backup material.
- Raw invoices beyond transient request/response handling.
- Private channel state or raw payment secrets.
- Bitcoind RPC credentials.
- Any local signing authority for Nexus treasury funds.

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

Receive-material surface:

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

If Autopilot 3 or another Cloudflare Worker exposes Nexus functionality:

1. Read-only visualization routes may proxy only status/projection data that is
   already redacted by Nexus.
2. Admin routes must require WorkOS/API-token authorization before calling
   Nexus.
3. Admin writes must pass an idempotency key to Nexus and return the Nexus
   operation id to the caller.
4. Worker logs and persisted state may include request ids, actor ids, Nexus
   operation ids, and hashed target ids only.
5. Worker state must not contain LDK seed, node key, API key, TLS key, private
   channel material, raw bitcoind RPC credentials, or treasury spend authority.
6. Browser code must never instantiate the production LDK node or sign Nexus
   treasury payments.

## Verification

Use these checks before claiming the boundary is intact:

```bash
bash scripts/deploy/nexus/test-custody-boundary-guards.sh
cargo test -p nexus-control admin_treasury_operations -- --nocapture
```

For live verification, use an admin token from the local ignored secrets file
and check that unauthorized admin calls fail, authorized read/admin calls return
redacted operation ids, and no response contains raw LDK custody material.
