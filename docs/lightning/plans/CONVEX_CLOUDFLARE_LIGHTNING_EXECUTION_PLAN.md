# Convex/Cloudflare-First Lightning Execution Plan

Status: Draft  
Date: 2026-02-12  
Owner: OpenAgents

## 1. Objective

Move to a **Convex/Cloudflare-first** Lightning architecture where:

1. `apps/web` Worker + Convex remain the control plane and product surface.
2. Agent wallets are managed through OpenAgents control-plane records (not ad hoc env-only wiring).
3. Payment execution is in Convex/Cloudflare **if technically viable**.
4. If not viable, use one minimal dedicated runtime (Cloud Run) with strict boundaries.

This plan covers buyer-side L402 payments (Spark-first), while staying compatible with hosted L402 seller infra (Aperture + Voltage).

## 2. Current State (Implemented)

1. `apps/web` Worker executes `lightning_l402_fetch` and `lightning_l402_approve`.
2. Worker L402 execution calls remote wallet executor when configured:
   - `apps/web/src/effuse-host/lightningL402Executor.ts`
   - Env: `OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL`, optional auth token.
3. Spark execution service exists:
   - `apps/lightning-wallet-executor`
   - Uses `@breeztech/breez-sdk-spark/nodejs`
   - Loads mnemonic from env or GCP Secret Manager (`src/runtime/secrets.ts`).
4. Convex stores tasks/events/presence/paywall state, but **does not currently store Spark seed material**.

## 3. Why It Is Separate Today

Payment execution ended up separate because the current Spark path is built around a Node runtime + SDK boundary:

1. Spark integration package in use is Node-targeted (`@breeztech/breez-sdk-spark/nodejs`).
2. Existing gateway initialization currently uses local storage path semantics (`withDefaultStorage(...)` in executor).
3. Cloudflare Worker runtime is not the same environment as Node process services.
4. Convex functions are ideal for control-plane and durable workflow state, but are not currently the place where this Spark SDK runtime is proven in this codebase.

So the existing implementation chose a dedicated payment runtime and kept Worker/Convex as orchestration.

## 4. Architecture Decision (Target)

Adopt a two-step decision:

1. **Try Convex/Cloudflare-native payment execution first** via a constrained technical spike.
2. If spike fails any hard gate, keep payment runtime separate (Cloud Run), but keep everything else in Convex/Cloudflare.

Hard gates for Convex/Cloudflare-native Spark execution:

1. Can run Spark payment path without unsupported runtime dependencies.
2. Can handle secret custody safely (no plaintext seed in logs/storage).
3. Can meet latency/reliability targets for interactive chat usage.
4. Can pass deterministic integration tests for paid/cached/blocked flows.

If any gate fails, we lock in the minimal external runtime.

## 5. Recommended End-State (Pragmatic)

Even if payment runtime remains separate, the architecture is still Convex/Cloudflare-first:

1. **Convex (source of truth)**
   - Wallet metadata, policy, and encrypted key envelope records.
   - Task lifecycle and payment receipts.
   - UI projection rows for wallet balance/status/last payment.
2. **Cloudflare Worker (`apps/web`)**
   - Tool schema validation, allowlist/cap policy preflight, approval UX.
   - L402 challenge/response orchestration via `@openagentsinc/lightning-effect`.
   - Calls payment runtime through a narrow API only when invoice pay is needed.
3. **Payment runtime (only if required)**
   - One service that does Spark `payBolt11`.
   - No product logic; only wallet execution and status.
   - No direct user-facing APIs.

## 6. Wallet Custody Model for Convex-First

Goal: manage wallet state in Convex without making Convex the plaintext key vault.

### 6.1 Data model additions (Convex)

Add tables (or equivalent modules) under `apps/web/convex/lightning/`:

1. `lightningWallets`
   - `walletId`, `ownerId`, `backend` (`spark`), `network`, `status`.
2. `lightningWalletPolicies`
   - spend caps, allowlist/blocklist, kill switch, rotation policy.
3. `lightningWalletKeyEnvelopes`
   - encrypted mnemonic ciphertext,
   - wrapped DEK/KMS reference,
   - key version metadata,
   - rotation timestamps.
4. `lightningWalletSnapshots`
   - latest balance, connected status, last payment time/id, health info.

### 6.2 Encryption boundary

1. Generate/import mnemonic via secure server path.
2. Encrypt mnemonic before persistence.
3. Store ciphertext + key envelope metadata in Convex.
4. Decrypt only inside approved payment runtime boundary.

Convex remains durable source of truth; plaintext seed never persists there.

## 7. Execution Topologies

## Topology A (Preferred if viable): Convex/Cloudflare-native payment

1. Worker or Convex action performs Spark payment directly.
2. No separate payment runtime.
3. Requires runtime compatibility proof with Spark SDK and custody controls.

Risk: high implementation uncertainty.

## Topology B (Fallback, likely): Minimal Cloud Run payment runtime

1. Worker performs full L402 orchestration and calls runtime only for invoice payment.
2. Runtime retrieves/decrypts wallet envelope and executes Spark payment.
3. Worker completes retry with macaroon/preimage and records receipts in Convex.

This is closest to what exists, but upgrades custody from env-secrets to Convex-managed envelopes.

## 8. Phased Implementation Plan

## Phase P0: Decision spike (Convex/Cloudflare-native feasibility)

Build a small non-production spike that attempts Spark invoice payment from Convex/Cloudflare path directly.

Acceptance:

1. One test invoice payment succeeds in controlled environment.
2. No unsupported runtime behavior.
3. Secret handling approach validated.

Exit:

1. If pass: proceed with Topology A.
2. If fail: commit ADR and proceed with Topology B.

## Phase P1: Convex wallet control-plane + custody records

Implement wallet tables, policies, and envelope schema.

Acceptance:

1. Wallet create/import/disable/rotate APIs.
2. Policy enforcement primitives in Convex layer.
3. Deterministic migration scripts + tests.

## Phase P2: Worker/payment contracts (Effect-first)

Unify wallet execution contract in `packages/lightning-effect` + `apps/web`:

1. Typed `WalletExecutionService` contract.
2. Standard error taxonomy (`policy_denied`, `insufficient_funds`, `backend_unavailable`, etc.).
3. Deterministic receipt fields for all outcomes.

Acceptance:

1. Existing L402 tool path compiles against new contract.
2. Unit tests cover happy + deny + failure paths.

## Phase P3A: Implement Topology A (if feasible)

Wire direct Spark execution in Convex/Cloudflare path with secure envelope handling.

Acceptance:

1. End-to-end paid/cached/blocked in chat without external executor.
2. All tests and smoke harness pass.

## Phase P3B: Implement Topology B (fallback)

Harden `apps/lightning-wallet-executor` as minimal runtime:

1. Remove env-only seed dependency for production path.
2. Add wallet lookup/decrypt flow based on Convex wallet envelope records.
3. Add strict auth from Worker -> runtime, idempotency, and audit fields.

Acceptance:

1. Runtime has no product logic, only wallet execution.
2. Worker/Convex remain source of truth for policy and receipts.
3. Full flow tests pass.

## Phase P4: UI and operator surfaces

In `apps/web` panes and chat states, show:

1. wallet online/offline,
2. balance/snapshot,
3. payment/cached/blocked reason and proof refs,
4. backend mode (`convex_native` vs `runtime`).

Acceptance:

1. Storybook covers all payment states.
2. Demo path is legible without log tailing.

## Phase P5: E2E + CI gates

Add deterministic full-flow harness for:

1. paid success,
2. cache hit no second payment,
3. over-cap block pre-payment,
4. endpoint allowlist block.

Acceptance:

1. Single non-interactive command for local/CI.
2. Production rehearsal runbook updated.

## 9. Migration from Current Setup

Current setup can keep running while migration happens.

1. Keep existing `OA_LIGHTNING_WALLET_EXECUTOR_*` as compatibility mode.
2. Introduce new wallet source mode:
   - `wallet_source=convex_envelope` (target),
   - `wallet_source=legacy_env` (temporary).
3. Backfill agent wallet(s) into Convex envelope model.
4. Flip Worker to prefer Convex wallet mode once healthy.
5. Remove legacy env-seed mode after stabilization.

## 10. Security and Compliance Requirements

1. No mnemonic in browser or client logs.
2. No plaintext mnemonic in Convex records.
3. Worker/runtime logs redact invoice/preimage/authorization headers.
4. Approval and policy decisions are persisted with typed reasons.
5. Every payment attempt has request correlation ID and deterministic receipt record.

## 11. Testing Strategy

1. Package tests (`packages/lightning-effect`):
   - parser/policy/cache/retry contracts.
2. Worker integration (`apps/web/tests/worker`):
   - tool approval, policy blocks, payload previews, receipt shape.
3. Runtime integration (`apps/lightning-wallet-executor/test`) if Topology B:
   - auth, allowlist, cap, payment success/fail mappings.
4. Full-flow smoke (`apps/lightning-ops`):
   - deterministic mock and live modes.

## 12. Ordered GitHub Issue Set (Proposed)

1. Phase P0: Spark in Convex/Cloudflare feasibility spike + ADR decision.
2. Phase P1: Convex wallet schema + lifecycle APIs + envelope records.
3. Phase P2: Unified Effect wallet execution contract + Worker integration.
4. Phase P3A: Convex-native Spark execution path (if feasible).
5. Phase P3B: Cloud Run minimal runtime with Convex envelope decryption (fallback path).
6. Phase P4: Web panes + chat state projection for wallet/payment execution mode.
7. Phase P5: Deterministic full-flow tests and CI gate.

## 13. Recommendation

Proceed immediately with **P0** and do not assume Convex-native Spark execution will work until proven.

Practical default:

1. Keep orchestration in Convex/Cloudflare.
2. Keep payment runtime as one minimal service only if runtime constraints require it.
3. Move wallet custody model to Convex-managed encrypted envelopes so architecture remains centrally managed by OpenAgents control plane.
