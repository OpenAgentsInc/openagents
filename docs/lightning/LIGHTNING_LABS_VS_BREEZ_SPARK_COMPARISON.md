# Lightning Labs Stack vs Breez/Spark for OpenAgents

Status: Draft recommendations
Date: 2026-02-11
Scope: OpenAgents active TypeScript/Effect stack (`apps/web`, `apps/autopilot-worker`, `apps/expo`, `packages/*`) + archived Rust wallet model in backroom

## 1. Executive Verdict

Lightning Labs and Breez/Spark are not mutually exclusive for OpenAgents. They solve different layers and can be combined cleanly if we stop treating "wallet" as one monolith.

Recommended stance:

1. Use Lightning Labs stack (`lnd` + remote signer + macaroons + `lnget` + Aperture + L402) for agent commerce infrastructure, high-assurance key isolation, and seller/paywalled API flows.
2. Use Breez/Spark for nodeless end-user wallet UX where running a node is a bad fit (especially browser/mobile clients).
3. Standardize OpenAgents on one internal payment contract: `pay_invoice -> { payment_id, amount_sats, preimage }` plus typed receipts.
4. Build everything in Effect first via `packages/lightning-effect`, then add adapters for both stacks.

If we sequence this correctly, we get faster product velocity and avoid a forced migration.

## 2. What Was Reviewed

### 2.1 Active repo findings

- Lightning planning docs exist and are solid:
  - `docs/lightning/LIGHTNING_AGENT_TOOLS.md`
  - `docs/lightning/SETUP_LOG.md`
  - `docs/lightning/LIGHTNING_DEEP_INTEGRATION_ROADMAP.md`
- Receipt semantics already support Lightning proofs (`lightning_preimage`):
  - `docs/adr/ADR-0013-receipt-schema-payment-proofs.md`
- Active worker/web/mobile code currently has no real Lightning wallet runtime:
  - `apps/autopilot-worker/src/tools.ts` has no payment tool contracts.
  - `apps/web/src/` and `apps/expo/` have no active Lightning wallet implementation.

### 2.2 Archived Rust-era findings (backroom)

Archive roots reviewed:

- `~/code/backroom/openagents-rust-deprecation-2026-02-11/openagents/`
- `~/code/backroom/openagents-docs-rust-archive-2026-02-11/docs/`

- Spark wrapper was broad and feature-rich:
  - send/receive, BOLT11, LNURL, lightning address, on-chain claim/refund, token APIs, event listeners.
  - refs: `.../crates/spark/src/wallet.rs`, `.../crates/spark/docs/API.md`
- Runtime wallet abstraction dropped data needed for L402:
  - `WalletService::pay_invoice` returned only `{ payment_id, amount_sats }`.
  - refs: `.../crates/runtime/src/wallet.rs`, `.../crates/runtime/src/wallet_spark.rs`
- Spark network model was nodeless and infra-coupled:
  - Mainnet + hosted regtest path via Breez/Lightspark services.
  - ref: `.../crates/spark/docs/REGTEST.md`
- Old API layer had wallet attach/discovery but payment endpoints were effectively retired (501) after Spark API removal.

## 3. Stack Comparison: Lightning Labs vs Breez/Spark

| Dimension | Lightning Labs stack | Breez/Spark stack (archived OpenAgents model) | OpenAgents implication |
|---|---|---|---|
| Core model | Node-centric (`lnd`), L402-native tooling (`lnget`, Aperture), macaroon auth, remote signer option | Nodeless SDK model (wallet in app, no local node ops) | These are complementary, not replacements |
| L402 buyer flow | First-class via `lnget` (402 parse, pay, retry, token cache) | Possible only if wallet returns preimage and app implements L402 client logic | We need wallet contract fix either way |
| L402 seller flow | First-class via Aperture reverse proxy | Not native seller stack; no Aperture-equivalent in Spark wrapper | Seller path should use Lightning Labs stack first |
| Key isolation | Strong: remote signer splits signing from routing/ops | Key-in-app model; self-custodial but no remote signer pattern | High-value agent ops should prefer remote signer |
| Scoped credentials | Native macaroon scoping (pay-only, invoice-only, etc.) | Not macaroon-first model | Least-privilege policy is easier with Lightning Labs path |
| Browser fit | Poor for direct node/tooling (Workers/browser cannot run `lnd`/`lnget`) | Strong (WASM/mobile-friendly nodeless model) | Keep Spark/Breez-like approach for browser/mobile UX |
| Ops complexity | Higher (node lifecycle, certs, macaroons, signer topology) | Lower for app teams (SDK init + storage + API key) | Use Lightning Labs where infra complexity is justified |
| Vendor coupling | Depends on chosen node hosting/operator decisions; protocol stack is open | Breez API dependency + hosted infra assumptions | Multi-adapter architecture avoids hard lock-in |
| Agent shell integration | Excellent (CLI tools fit Codex/OpenClaw style workflows) | SDK integration stronger than shell tooling | Desktop/agent runtime can leverage both |

## 4. Can They Work Well Together?

Yes, but only with a layered architecture.

### 4.1 Where they fit together cleanly

1. Breez/Spark for user-facing wallets in browser/mobile.
2. Lightning Labs stack for L402 marketplace behavior (buy/sell APIs), node-grade operations, and key isolation.
3. Unified receipt and policy plane in OpenAgents (budgeting, spend proofs, audit trails).

### 4.2 Where they conflict

1. `lnget` currently targets Lightning backends (`lnd`/LNC/Neutrino), not Spark SDK directly.
2. Aperture is tightly aligned to Lightning Labs node/auth patterns (macaroons, invoices), not Spark SDK abstractions.
3. Worker/browser runtime cannot directly host `lnd`/`lnget`/Aperture binaries.

### 4.3 Primary technical gate (must fix)

OpenAgents payment abstraction must return payment proof material:

- Existing archived pattern was `pay_invoice -> { payment_id, amount_sats }`.
- Required pattern for L402 and receipt integrity is `pay_invoice -> { payment_id, amount_sats, preimage_hex }` (or reliable lookup that yields preimage).

Without this, neither a Spark-backed nor an LND-backed Effect runtime can be first-class for L402 auth.

## 5. Product-Surface Strategy (Web, Mobile, Desktop, Worker)

### 5.1 `apps/web` (openagents.com)

Role:

- Orchestration, policy, receipts, visibility.
- Not direct wallet binary host.

Recommendation:

- Add Worker-safe L402/policy orchestration APIs.
- Call an execution adapter (hosted wallet service or desktop bridge) instead of running local binaries.
- Keep payment proofs and spend limits as first-class UI entities.

### 5.2 `apps/expo` (React Native)

Role:

- User wallet visibility + constrained send/receive UX.

Recommendation:

- Use nodeless wallet path first (Spark-like adapter) for speed.
- Defer full node operations to desktop/backend surfaces.

### 5.3 Planned Electron desktop

Role:

- Sovereign execution host and security boundary.

Recommendation:

- Run `lnd` watch-only + remote signer path here first.
- Run `lnget` locally for buyer automation.
- Optionally run local Aperture for seller development/testing.
- Expose a narrow IPC API to web/mobile control planes.

### 5.4 `apps/autopilot-worker`

Role:

- Typed tool contracts + deterministic receipts.

Recommendation:

- Introduce payment/L402 tool contracts that are backend-agnostic.
- Keep runtime adapters pluggable so one tool can call Spark-backed payer or LND-backed payer.

## 6. Recommended Order of Operations

This is the order that minimizes rework and gets to production utility fastest.

## Phase 0: Contract and terminology hardening (immediate)

Deliverables:

1. Create `packages/lightning-effect` as the canonical shared package with:
   - Effect `Schema` contracts for payment/L402.
   - Effect services (`InvoicePayer`, `InvoiceCreator`, `L402Client`, `BudgetPolicy`).
   - adapters and live/test Layers for wallet backends.
2. Publishable package layout and API docs so external consumers can adopt `lightning-effect` directly.
3. Keep API and module conventions aligned with `nostr-effect` where practical (naming, Layer composition, error tagging, test style).
4. Add explicit docs linking payment proofs to ADR-0013 semantics.

Exit criteria:

- All future adapters target the same interface.

## Phase 1: Buyer capability in controlled environment (desktop-first)

Deliverables:

1. Electron/hosted adapter that can pay BOLT11 and return preimage.
2. L402 client helper (parse 402 challenge, pay, retry, cache).
3. Basic spend policy (`max_cost`, domain allowlist).

Exit criteria:

- One real L402 API can be paid from OpenAgents flow with receipt proof captured.

## Phase 2: Seller capability using Lightning Labs primitives

Deliverables:

1. Put one OpenAgents endpoint behind Aperture.
2. Invoice-only macaroon path for seller side.
3. Reconciliation hook into OpenAgents receipts.

Exit criteria:

- One endpoint is monetized via L402 end-to-end.

## Phase 3: Security hardening (remote signer default)

Deliverables:

1. Watch-only execution node + signer-only remote signer topology.
2. Macaroon bakery UX/policy for pay-only, invoice-only, read-only roles.
3. Budget kill switches at policy layer.

Exit criteria:

- Compromised execution host cannot extract signing keys.

## Phase 4: Spark/Breez adapter restoration for nodeless UX

Deliverables:

1. Reintroduce a Spark/Breez adapter in `packages/lightning-effect` (not Rust-only).
2. Ensure adapter returns preimage-compatible payment proofs.
3. Use this adapter for mobile/browser flows where node operation is not desired.

Exit criteria:

- Web/mobile can execute policy-safe payments without local node ops.

## Phase 5: Unified routing and policy engine

Deliverables:

1. Route decisions by policy:
   - small payments/high UX: Spark path.
   - seller ops/high assurance: LND path.
2. Common telemetry and receipts regardless of backend.

Exit criteria:

- Backend swap does not change product-level contracts.

## Phase 6: Scale and productization

Deliverables:

1. Managed mode (hosted execution plane).
2. Connected mode (external user wallet/node).
3. Sovereign mode (desktop-hosted wallet/node).

Exit criteria:

- All three modes share one policy + receipt model.

## 7. What to Keep from the Old Spark/Breez Model

Keep these design wins:

1. Unified identity orientation (agent identity and payment identity coupling logic).
2. Rich payment operation coverage (receive/send/LNURL/lightning address/history).
3. Nodeless ergonomics for user-facing apps.

Do not carry forward these limitations:

1. Payment result without preimage.
2. Wallet logic trapped in one runtime/language stack.
3. Network assumptions that treat hosted regtest as equivalent to local deterministic test infra.

## 8. Decision Rules for Stack Selection

Use Lightning Labs stack when:

1. You need L402 selling (Aperture), scoped macaroons, or remote signer security.
2. You are running long-lived agent infrastructure or desktop sovereign mode.

Use Spark/Breez-style nodeless adapter when:

1. You need browser/mobile wallet UX with minimal ops burden.
2. You do not need local node management.

Use both when:

1. You want OpenAgents to support broad user UX and deep agent commerce infrastructure simultaneously.

## 9. Risks and Mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| Adapter divergence (Spark vs LND behavior mismatch) | Inconsistent user outcomes | Enforce one shared contract + conformance tests |
| Preimage retrieval not uniformly available | Blocks L402 correctness | Make preimage a hard contract requirement before release |
| Worker runtime limitations | Tooling unusable in prod | Keep binary execution in desktop/hosted executor, not Worker |
| `lnget` / Aperture upstream install path fragility (`go install ...@latest`) | Slows CI/dev bring-up | Pin source-build workflow and containerized tool builds in internal setup scripts |
| Credential sprawl (macaroons, certs, node creds) | Security incident risk | OS keychain/secret manager, scoped macaroons, rotation runbook |
| Rebuilding too much old Rust logic blindly | Slow delivery | Port behavior, not code; implement in current Effect architecture |

## 10. Concrete Near-Term Plan (next 4-6 weeks)

1. Create `packages/lightning-effect` with shared contracts, services, and tests.
2. Implement first L402 buyer flow in controlled adapter (desktop or hosted service).
3. Add one `autopilot-worker` payment tool contract with schema validation and receipts.
4. Stand up one Aperture-gated endpoint in dev/staging.
5. Validate full proof path in receipts (`lightning_preimage`) end-to-end.
6. Start Spark/Breez adapter reintroduction only after contract and receipt parity are locked.

## 11. Bottom Line

OpenAgents should not pick a single winner between Lightning Labs and Breez/Spark.

The right architecture is:

1. Lightning Labs for infrastructure-grade agent commerce and security.
2. Breez/Spark for nodeless client wallet UX.
3. One OpenAgents contract and receipt model above both.

That gives us speed now, sovereignty later, and no forced rewrite when the desktop app ships.

## 12. Primary Source Pointers

Active repo:

- `docs/lightning/LIGHTNING_AGENT_TOOLS.md`
- `docs/lightning/SETUP_LOG.md`
- `docs/lightning/LIGHTNING_DEEP_INTEGRATION_ROADMAP.md`
- `docs/adr/ADR-0013-receipt-schema-payment-proofs.md`
- `apps/autopilot-worker/src/tools.ts`
- `docs/PROJECT_OVERVIEW.md`

Backroom archive:

- `~/code/backroom/openagents-rust-deprecation-2026-02-11/openagents/crates/spark/src/wallet.rs`
- `~/code/backroom/openagents-rust-deprecation-2026-02-11/openagents/crates/runtime/src/wallet.rs`
- `~/code/backroom/openagents-rust-deprecation-2026-02-11/openagents/crates/runtime/src/wallet_spark.rs`
- `~/code/backroom/openagents-rust-deprecation-2026-02-11/openagents/crates/spark/docs/REGTEST.md`
- `~/code/backroom/openagents-rust-deprecation-2026-02-11/openagents/apps/api/docs/agent-wallets.md`
- `~/code/backroom/openagents-docs-rust-archive-2026-02-11/docs/agent-payments/wallet-considerations.md`
