# Spark + L402 Desktop Integration Work Log (2026-02-12)

Status: Implemented and verified
Owner: Codex session
Scope: `apps/desktop`, `packages/lightning-effect`, `docs/lightning`

## 1) Goal

Implement a Spark-backed L402 payment path in the Electron desktop app using:

1. NIP-06 mnemonic generation from `nostr-effect`
2. Breez Spark SDK (`@breeztech/breez-sdk-spark`) as the wallet/payment backend
3. Effect-first service boundaries so the same contracts remain reusable
4. Full test coverage for Spark-paid L402 flow without regressing existing LND path

This work is aligned with the repo strategy from:

1. `docs/lightning/reference/LIGHTNING_LABS_VS_BREEZ_SPARK_COMPARISON.md`
2. `docs/plans/active/lightning/LND_NEUTRINO_ELECTRON_IMPLEMENTATION_PLAN.md`

The resulting model is dual-path:

1. Spark path for nodeless desktop payment execution (when connected)
2. Existing LND deterministic path retained as fallback and for existing local-node flows/tests

## 2) Major Changes

## 2.1 Shared package: Spark adapter in `lightning-effect`

Added reusable Spark payment service + adapter in `packages/lightning-effect`:

1. `src/services/sparkPayment.ts`
   - New `SparkPaymentService` contract:
     - `payBolt11(request) -> InvoicePaymentResult`
   - Uses existing typed Lightning errors (`PaymentFailedError`, `PaymentTimeoutError`, `PaymentMissingPreimageError`)

2. `src/adapters/invoicePayerSpark.ts`
   - New `makeInvoicePayerSparkLayer()`
   - Bridges `SparkPaymentService` into existing `InvoicePayerService`

3. `src/layers/invoicePayerSpark.ts`
   - New spark live layer helper

4. Export updates:
   - `src/services/index.ts`
   - `src/adapters/index.ts`
   - `src/layers/index.ts`

Added tests:

1. `test/invoice-payer-spark.contract.test.ts`
   - Verifies pass-through success and typed failures
2. `test/l402-client-spark.integration.test.ts`
   - Verifies full L402 challenge -> pay -> retry -> cache flow using Spark adapter

## 2.2 Desktop main process: Spark wallet manager

Added Spark manager and IPC wiring:

1. `apps/desktop/src/main/sparkWalletManager.ts`
   - Effect service `SparkWalletManagerService`
   - Uses `DesktopSecureStorageService` for mnemonic persistence
   - Uses NIP-06 from `nostr-effect`:
     - `generateSeedWords`
     - `validateWords`
   - Connects Breez SDK via `SdkBuilder`
   - Exposes:
     - `bootstrap()`
     - `refresh()`
     - `snapshot()`
     - `payInvoice()`
     - `disconnect()`
   - Returns L402-compatible payment result:
     - `paymentId`
     - `amountMsats`
     - `preimageHex`
     - `paidAtMs`

2. `apps/desktop/src/main/sparkWalletIpc.ts`
   - Added spark IPC channels

3. `apps/desktop/src/main/lndRuntimeRuntime.ts`
   - Runtime layer now includes Spark manager layer + config
   - Added `defaultSparkWalletManagerConfig(...)`

4. `apps/desktop/src/main.ts`
   - Registers spark IPC handlers
   - Bootstraps spark wallet on app startup
   - Disconnects spark wallet on shutdown

## 2.3 Renderer/effect wiring + panes

Added renderer bridge + app state integration:

1. `apps/desktop/src/preload.ts`
   - Exposes `openAgentsDesktop.sparkWallet` bridge:
     - `snapshot`
     - `bootstrap`
     - `refresh`
     - `payInvoice`
     - `disconnect`

2. `apps/desktop/src/global.d.ts`
   - Added typed spark bridge and payload types

3. `apps/desktop/src/effect/sparkWalletGateway.ts`
   - New Effect gateway service for renderer-safe Spark operations
   - Normalizes snapshots and invoice payment results
   - Maps invalid results to typed payment errors

4. `apps/desktop/src/effect/model.ts`
   - Added `spark` runtime section in desktop state

5. `apps/desktop/src/effect/app.ts`
   - Bootstraps/refreshes spark state during app bootstrap + auth flow
   - Refreshes spark state on executor tick

6. `apps/desktop/src/effect/layer.ts`
   - Includes `SparkWalletGatewayLive`
   - Provides spark dependency to `L402ExecutorLive`

7. `apps/desktop/src/effect/paneModels.ts`
   - Wallet availability now treats connected Spark as ready

8. `apps/desktop/src/renderer.ts`
   - Overview and wallet panes now render Spark lifecycle/balance/error/status fields

## 2.4 L402 executor behavior

Updated executor to select backend at runtime:

1. `apps/desktop/src/effect/l402Executor.ts`
   - Uses Spark when spark wallet lifecycle is `connected`
   - Falls back to existing deterministic LND adapter otherwise
   - Adds `paymentBackend` to execution result:
     - `spark`
     - `lnd_deterministic`

2. `apps/desktop/src/effect/executorLoop.ts`
   - Persists `paymentBackend` into task transition metadata

## 3) Payment Flow (Spark path)

For an L402-protected request in desktop executor:

1. Request hits seller endpoint
2. Seller returns `402` with `WWW-Authenticate: L402 ...`
3. `lightning-effect` parses challenge
4. Spend policy validates quote/cap
5. Executor’s Spark invoice payer calls main-process spark manager `payInvoice`
6. Spark manager:
   - prepares bolt11 send with Breez SDK
   - sends payment (with completion timeout)
   - extracts lightning preimage from payment details
7. Credential cached and retry request sent with `Authorization: L402 ...`
8. Success metadata includes proof reference + payment backend

## 4) Configuration

Desktop Spark manager config is controlled by env (with defaults):

1. `OA_DESKTOP_SPARK_NETWORK` (`mainnet` or `regtest`, default `regtest`)
2. `OA_DESKTOP_SPARK_API_KEY` (or `OA_DESKTOP_BREEZ_API_KEY`)
3. `OA_DESKTOP_SPARK_MNEMONIC_PASSPHRASE` (optional)
4. `OA_DESKTOP_SPARK_AUTO_GENERATE_MNEMONIC` (`0` disables auto-generation)
5. `OA_DESKTOP_SPARK_PAYMENT_TIMEOUT_SECS` (default `45`)

Mnemonic storage:

1. Generated/validated via NIP-06
2. Stored through existing secure storage service (`DesktopSecureStorageService`)
3. Not exposed to renderer state or pane models

## 5) Tests Added/Updated

New desktop tests:

1. `apps/desktop/tests/sparkWalletManager.test.ts`
   - bootstrap + mnemonic generation + connection behavior
   - bolt11 pay result compatibility (`preimageHex` etc.)
   - renderer-safe projection (no mnemonic leakage)
2. `apps/desktop/tests/l402ExecutorSpark.test.ts`
   - full Spark-paid L402 flow with cache reuse

New lightning-effect tests:

1. `packages/lightning-effect/test/invoice-payer-spark.contract.test.ts`
2. `packages/lightning-effect/test/l402-client-spark.integration.test.ts`

Updated desktop tests:

1. `apps/desktop/tests/executorLoop.test.ts`
   - Added `paymentBackend` to mocked executor outcomes

## 6) Verification Run

Commands run and passing:

1. `cd packages/lightning-effect && npm run typecheck && npm test`
2. `cd apps/desktop && npm run typecheck`
3. `cd apps/desktop && npm run lint` (no errors; existing pre-existing warnings remain)
4. `cd apps/desktop && npm test`

Result summary:

1. `packages/lightning-effect`: typecheck pass, tests pass
2. `apps/desktop`: typecheck pass, lint pass (0 errors), tests pass

## 7) Notes and Follow-ups

1. Spark is now first-class in desktop execution path, but still requires Breez API key configuration.
2. LND path remains intact and acts as a deterministic fallback for offline/spark-unavailable cases.
3. The new adapter/service boundaries keep backend choice at the edge while preserving one L402 contract model.
