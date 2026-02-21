# Lightning Wallet Executor Auth and Receipt Contract (v1)

Status: Active  
Last updated: 2026-02-21  
Authority proto package: `openagents.lightning.v1`

## Purpose

Define wallet-executor auth assertion and canonical payment receipt wire contracts used by control/runtime consumers.

## Proto Authority

- `proto/openagents/lightning/v1/wallet_executor.proto`

Messages:

1. `WalletExecutorAuthAssertion`
2. `WalletExecutionReceipt`

## Auth Assertion Semantics

`WalletExecutorAuthAssertion` fields:

1. `wallet_id`: executor wallet identity.
2. `auth_mode`: executor auth mode (`DISABLED` or `BEARER_STATIC`).
3. `auth_enforced`: whether bearer auth is currently required for calls.
4. `auth_token_version`: integer version of active token.
5. `token_fingerprint`: non-secret SHA-256 fingerprint of active token material.
6. `asserted_at_ms`: unix epoch millis when assertion snapshot was produced.

## Receipt Canonicalization Semantics

`WalletExecutionReceipt` is canonical settlement proof emitted by wallet executor.

Hash-critical fields (v1):

1. `receipt_version`
2. `request_id`
3. `wallet_id`
4. `host`
5. `payment_id`
6. `invoice_hash`
7. `quoted_amount_msats`
8. `settled_amount_msats`
9. `preimage_sha256`
10. `paid_at_ms`
11. `rail`
12. `asset_id`

`canonical_json_sha256` must be SHA-256 of canonical JSON encoding of the hash-critical field set.

`receipt_id` is derived from `canonical_json_sha256` prefix (`lwr_<24 hex chars>`).

## Fixture

- `docs/protocol/fixtures/lightning-wallet-executor-receipt-v1.json`

The fixture is used by proto contract tests to enforce field compatibility and payload round-trip behavior.

