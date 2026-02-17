# Agent Payments + Spark + L402 (Laravel)

## Why `invoice payer: fake` showed up

The L402 wallet page reads `config('lightning.l402.invoice_payer')`.

If `L402_INVOICE_PAYER=fake` is set in runtime env, L402 calls use a deterministic fake preimage payer for tests/dev only.

This phase changes the default to `spark_wallet` and wires real L402 invoice payment through each authenticated user's Spark wallet.

## What is implemented now

### 1) Per-user Spark wallet model

- New table: `user_spark_wallets`
- One wallet row per user (`user_id` unique)
- Stores:
  - `wallet_id`
  - encrypted `mnemonic` (Eloquent cast: `encrypted`)
  - `spark_address`, `lightning_address`, `identity_pubkey`
  - `last_balance_sats`, `status`, `last_error`, metadata

Files:

- `database/migrations/2026_02_17_000001_create_user_spark_wallets_table.php`
- `app/Models/UserSparkWallet.php`
- `app/Models/User.php` (`sparkWallet()` relation)

### 2) Spark executor client + wallet service

- `SparkExecutorClient` wraps HTTP calls to external signer/executor service.
- `UserSparkWalletService` handles:
  - ensure/import wallet
  - sync status/balance
  - create invoice
  - pay BOLT11
  - send sats to Spark address

Files:

- `app/Lightning/Spark/SparkExecutorClient.php`
- `app/Lightning/Spark/UserSparkWalletService.php`
- `app/Lightning/Spark/SparkExecutorException.php`

### 3) L402 now uses per-user Spark wallet context

- New payer: `SparkWalletInvoicePayer`
- `InvoicePayer` interface now accepts context (`userId`, spend caps, host, etc.).
- `L402Client` forwards context into payer call.
- `lightning_l402_fetch` and `lightning_l402_approve` now pass authenticated `userId` context.
- Approval tasks now persist `userId` and enforce user-match on approve.

Files:

- `app/Lightning/L402/InvoicePayers/SparkWalletInvoicePayer.php`
- `app/Lightning/L402/L402Client.php`
- `app/AI/Tools/LightningL402FetchTool.php`
- `app/AI/Tools/LightningL402ApproveTool.php`
- `app/Providers/AppServiceProvider.php`

### 4) Agent Payments API restored in Laravel

Sanctum-authenticated endpoints:

- `GET /api/v1/agent-payments/wallet`
- `POST /api/v1/agent-payments/wallet`
- `GET /api/v1/agent-payments/balance`
- `POST /api/v1/agent-payments/invoice`
- `POST /api/v1/agent-payments/pay`
- `POST /api/v1/agent-payments/send-spark`

Back-compat aliases (Episode 169 style):

- `GET /api/v1/agents/me/wallet`
- `POST /api/v1/agents/me/wallet`
- `GET /api/v1/agents/me/balance`
- `POST /api/v1/payments/invoice`
- `POST /api/v1/payments/pay`
- `POST /api/v1/payments/send-spark`

Files:

- `app/Http/Controllers/Api/AgentPaymentsController.php`
- `routes/api.php`
- OpenAPI request bodies under `app/OpenApi/RequestBodies/*Agent*`

### 5) L402 API wallet endpoint includes Spark wallet snapshot

`GET /api/v1/l402/wallet` now returns `data.sparkWallet` (if present) so L402 views/API consumers can see payer wallet identity/balance context.

## Required runtime env

Set these for real Spark-backed behavior:

- `L402_INVOICE_PAYER=spark_wallet`
- `SPARK_EXECUTOR_BASE_URL=https://<wallet-executor-host>`
- `SPARK_EXECUTOR_AUTH_TOKEN=<token-if-required>`
- `SPARK_EXECUTOR_TIMEOUT_MS=20000` (optional)
- `SPARK_AGENT_WALLET_ID_PREFIX=oa-user-` (optional)

If `SPARK_EXECUTOR_BASE_URL` is missing, wallet provisioning/payment endpoints will return Spark executor configuration errors.

## External signer/executor contract expected by Laravel

Laravel currently expects these executor routes:

- `POST /wallets/create`
- `POST /wallets/status`
- `POST /wallets/create-invoice`
- `POST /wallets/pay-bolt11`
- `POST /wallets/send-spark`

If your deployed executor only exposes legacy `/pay-bolt11` and `/status`, agent-wallet lifecycle endpoints will not work until executor is upgraded.

## Verification done in this phase

Targeted Pest coverage added/updated:

- L402 tool + client tests updated for context-aware payer interface.
- New Spark payer tests (`SparkWalletInvoicePayerTest`).
- New Agent Payments API coverage (`tests/Feature/Api/V1/AgentPaymentsApiTest.php`).
- OpenAPI generation test asserts agent-payment paths are present.

Run:

```bash
cd apps/openagents.com
php artisan test --compact
```

## What remains to be fully complete

1. Ensure deployed wallet executor implements full `/wallets/*` API above.
2. Bind production env vars so runtime is `spark_wallet` (not fake).
3. Add production smoke tests that call:
   - wallet ensure
   - invoice create
   - pay invoice (small amount)
   - L402 fetch using same wallet path
4. Add operator runbook for wallet key rotation and wallet import/recovery flow.
