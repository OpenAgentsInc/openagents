# Wallet Executor Auth and Key Custody

Status: Active  
Last updated: 2026-02-21  
Owner lane: `owner:infra`

## Purpose

Define the wallet-executor trust boundary with control-plane services, the custody location for payment secrets, and the canonical payment receipt contract.

## Auth Channel (Control Plane -> Wallet Executor)

Normative behavior:

1. `apps/lightning-wallet-executor` accepts authenticated control-plane calls over HTTPS only.
2. Authentication is static bearer token based:
   - request header: `Authorization: Bearer <token>`
   - configured token: `OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN`
3. Token comparison is constant-time in server code to reduce timing side channels.
4. Spark mode is fail-closed:
   - `OA_LIGHTNING_WALLET_EXECUTOR_MODE=spark` requires `OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN`.
5. Auth metadata is visible at `GET /status`:
   - `authMode` (`disabled` or `bearer_static`)
   - `authEnforced` (boolean)
   - `authTokenVersion` (positive integer)

## Key Custody

Custody policy:

1. Spark API key (`OA_LIGHTNING_SPARK_API_KEY`) is service secret material and must be provided via secret manager in production.
2. Wallet mnemonic custody follows provider mode:
   - local/dev: env var (`OA_LIGHTNING_WALLET_MNEMONIC`)
   - production: GCP Secret Manager (`OA_LIGHTNING_WALLET_MNEMONIC_SECRET_VERSION`)
3. Mnemonic plaintext must not be stored in runtime receipts, structured logs, or control-plane databases.
4. Control-plane services are never seed-key authorities; they only invoke executor APIs with scoped instructions.

## Receipt Canonicalization Contract

Payment execution emits `openagents.lightning.wallet_receipt.v1` receipts with deterministic hash semantics.

Canonical fields:

1. `receiptVersion`
2. `requestId`
3. `walletId`
4. `host` (lowercased)
5. `paymentId`
6. `invoiceHash` (lowercased)
7. `quotedAmountMsats`
8. `settledAmountMsats`
9. `preimageSha256`
10. `paidAtMs`
11. `rail` (`lightning`)
12. `assetId` (`BTC_LN`)

Rules:

1. Canonical JSON serialization of the ordered payload is hashed with SHA-256 (`canonicalJsonSha256`).
2. `receiptId` is derived from hash prefix: `lwr_<first24hex>`.
3. Any change to canonical fields must change `canonicalJsonSha256`.

## Non-Goals

1. This document does not define end-user auth flows (WorkOS/session UX).
2. This document does not define billing/quota monetization policy.
3. This document does not replace runtime-level global payment policy checks.

