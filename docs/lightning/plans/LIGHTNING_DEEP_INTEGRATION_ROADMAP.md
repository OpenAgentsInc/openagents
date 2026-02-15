# OpenAgents Deep Lightning Integration Blueprint

Status: Draft recommendations
Date: 2026-02-11
Scope: Web (`apps/web`), worker (`apps/autopilot-worker`), mobile (`apps/mobile`), desktop (`apps/desktop`), shared packages (`packages/*`)

## 1. Executive Summary

OpenAgents should implement Lightning in three parallel tracks:

1. Lightning as a buyer capability (agents pay L402 APIs).
2. Lightning as a seller capability (OpenAgents endpoints can be L402-gated).
3. Lightning as a product wallet capability (user and agent wallet UX across web/mobile/desktop).

The key architectural conclusion from the current codebase:

- `apps/web` and `apps/autopilot-worker` run on Cloudflare Workers, which is not the right place to run shell-native binaries like `lnget`, `lnd`, or `aperture`.
- The right path is a hybrid:
  - Protocol-native Effect L402/payment library in `packages/lightning-effect` for Worker-safe flows.
  - Optional native Lightning host on desktop (Electron) for sovereign operation, local node control, and remote-signer setups.
  - A clean service boundary so web/mobile can use hosted or user-managed wallets without hard-coding one custody model.

If this is executed well, OpenAgents can support:

- Instant machine payments for agent workflows.
- Verifiable payment proofs in existing receipt semantics (`lightning_preimage`).
- Least-privilege key management with scoped macaroons and remote signer.
- Progressive custody options (hosted, bring-your-own node, sovereign desktop).

## 2. Inputs Reviewed

This blueprint is based on:

- `docs/lightning/reference/LIGHTNING_AGENT_TOOLS.md`
- `docs/lightning/status/SETUP_LOG.md`
- `apps/web/` architecture (Worker host + Convex + Effect services)
- `apps/autopilot-worker/` architecture (tool contracts, DSE signatures, AI receipts)
- `apps/mobile/` architecture (React Native app with auth + Convex client)
- `packages/dse`, `packages/effuse`, `packages/effuse-test`, and related shared package boundaries
- `docs/GLOSSARY.md`, `docs/PROJECT_OVERVIEW.md`, `docs/ROADMAP.md`
- `docs/adr/ADR-0007-tool-execution-contract.md`
- `docs/adr/ADR-0013-receipt-schema-payment-proofs.md`

## 3. Current State Assessment

## 3.1 What exists today

- Lightning terminology and L402 concepts are documented in `docs/GLOSSARY.md`.
- Initial Lightning integration planning exists in `docs/lightning/reference/LIGHTNING_AGENT_TOOLS.md`.
- Practical setup/ops findings exist in `docs/lightning/status/SETUP_LOG.md`.
- Receipt semantics already support `payment_proof.type = "lightning_preimage"` (ADR-0013).
- Web and worker code already have durable receipt infrastructure (Convex `receipts` table + worker AI receipts).
- Tool contracts are centralized and schema-driven (`apps/autopilot-worker/src/tools.ts`).

## 3.2 What does not exist yet

- No seller paywall control plane (`l402Paywalls` / route + pricing lifecycle) in Convex yet.
- No OpenAgents-hosted Aperture deployment/reconciliation path wired into app runtime yet.
- No full wallet send/receive product UX in web routes/pages yet (current web state is L402 task orchestration + observability panes).
- No Lightning UX in `apps/mobile` (mobile app is currently mostly auth/demo shell).
- No sovereign desktop node/remote-signer host yet (current `apps/desktop` is an early executor shell with Effuse panes).
- No seller-side contracts/services in `packages/lightning-effect` yet (current package focuses on buyer-side L402 flow primitives).

## 3.3 Key constraints from the current stack

- Cloudflare Worker runtime is ideal for HTTP orchestration and policy, not local binary execution.
- `lnget` and `aperture` skill install scripts currently fail on `go install ...@latest` because of `replace` directives (documented in `SETUP_LOG.md`), so source builds are currently required.
- Remote signer + watch-only flow is operationally validated in Docker and should be treated as the default high-security model.
- Existing tool execution rules require schema validation and deterministic receipts (ADR-0007), so Lightning actions need first-class typed contracts.

## 4. Strategic Positioning

OpenAgents should not choose a single wallet/custody model too early. It should support three modes under one API:

1. Managed mode (fastest onboarding for web/mobile users).
2. Connected mode (user links external node/wallet via LNC or equivalent).
3. Sovereign mode (desktop-hosted node + optional remote signer).

This avoids rework and allows enterprise-grade and consumer-grade usage from one architecture.

## 5. Recommended Target Architecture

## 5.1 Architecture layers

Control plane:

- Policy, budgets, auth, telemetry, receipts, and routing decisions.
- Lives in `apps/web` Worker + Convex + shared Effect packages.

Payment execution plane:

- Actually pays invoices, obtains preimages, mints/uses scoped credentials.
- Can be:
  - Hosted Lightning executor service (for web/mobile managed mode).
  - Local desktop Lightning host (Electron app, native mode).
  - Future external provider adapters.

UI plane:

- Web and mobile show balances, activity, send/receive, caps, and policy state.
- Desktop adds sovereign controls (node health, signer linkage, channel ops, macaroon bakery).

## 5.2 Core technical principle

Never couple business logic to one transport.

- L402 logic should be protocol-native in Effect services and schemas.
- Shell tooling (`lnget`, `lnd`, `aperture`) should be adapters behind interfaces, not the only execution path.

## 5.3 Recommended component map

1. `packages/lightning-effect` (active, canonical)
   - Effect-first shared package intended for OpenAgents and external consumers.
   - Modules:
   - `contracts` (Effect `Schema`: `Rail`, `AssetId`, `PaymentProof`, `L402Challenge`, `L402Credential`, `InvoicePaymentResult`, policy types)
   - `services` (`InvoicePayer`, `InvoiceCreator`, `L402Client`, `BudgetPolicy`, `CredentialStore`, `NodeReadApi`)
   - `l402` (challenge parse/serialize, retry loop, credential cache policies)
   - `adapters` (lnd/LNC/desktop bridge/hosted executor; optional shell adapters)
   - `receipts` helpers mapping to ADR-0013 payment proof semantics

2. `apps/web` additions
   - Worker endpoints for Lightning control APIs.
   - Effect services for wallet, L402 fetch, pricing preview, policy.
   - Convex tables for wallets, payments, credentials, limits, audit trails.

3. `apps/mobile` additions
   - Wallet views and actions using the same API contracts.
   - Device-safe credential handling and policy controls.

4. `apps/desktop` (active, expanding)
   - Native Lightning host expansion:
   - local `lnget`, optional local `lnd` watch-only, remote signer wiring.
   - secure local keystore and IPC bridge to UI.

## 5.4 Open-Source Package Strategy (`lightning-effect`)

Goals:

- Keep all Lightning business logic in Effect and reusable outside this monorepo.
- Let external apps adopt the same contracts and service interfaces without importing OpenAgents app code.

Requirements:

1. Public API surface documented and semver-stable.
2. Separate core interfaces from OpenAgents-specific adapters.
3. Publish-ready package metadata and examples (Node, Worker, React Native, Electron).
4. `Layer`-driven architecture so downstream users can provide their own wallet adapters.
5. Align architecture and naming conventions with `~/code/nostr-effect` where possible to reduce cognitive overhead across libraries.

## 6. Product Surface Design

## 6.1 Web (`apps/web`)

Recommended role:

- Primary orchestration UI and policy control.
- Not primary key custody host for sovereign users.

Recommended capabilities:

- Wallet dashboard:
  - balance, spend velocity, limits, recent payments, invoice history.
- L402 action history:
  - endpoint, amount, status, preimage proof reference.
- Policy UI:
  - per-agent and per-tool budgets.
  - max spend per request.
  - domain allowlist/denylist.
- Seller controls:
  - route -> price policy -> Aperture integration state.

Recommended implementation boundaries:

- Add `LightningApiService` in `apps/web/src/effect/`.
- Add route(s) in `apps/web/src/effuse-app/routes.ts` for wallet/payments/admin.
- Add corresponding Effuse pages in `apps/web/src/effuse-pages/`.
- Keep secrets server-side only (`apps/web/src/effuse-host/env.ts`).

## 6.2 Mobile (`apps/mobile`)

Recommended role:

- User wallet and notifications surface.
- Safe constrained payment approvals and budget management.

Recommended capabilities:

- Read-only defaults:
  - balances, history, incoming payments, L402 activity.
- Optional controlled send/pay:
  - enforced by server-side policy.
- Push notification hooks:
  - invoice paid, budget threshold crossed, failed payment.

Implementation notes from current state:

- Existing app is currently auth + demo nav shell.
- Introduce wallet features after shared API contracts are stable.
- Use secure device storage for short-lived auth/session, not long-lived private keys in phase 1.

## 6.3 Desktop (`apps/desktop`, early implementation)

Recommended role:

- Sovereign operations host.
- Place where local wallet/node tooling can run safely.

Capabilities to prioritize:

- Node mode manager:
  - watch-only + remote signer (default).
  - standalone testnet/regtest mode.
- Credential bakery UI:
  - generate pay-only, invoice-only, read-only macaroons with expiry and caps.
- Native L402 tooling:
  - local `lnget` execution.
  - optional local Aperture for dev/seller simulation.
- Local policy enforcement:
  - hard stop limits independent of server.

Security note:

- Desktop should be the only surface that can directly touch sensitive Lightning execution if user chooses sovereign mode.

## 7. Security Architecture (Required)

## 7.1 Default key model

- Default production recommendation:
  - watch-only node on execution host.
  - signer isolated on separate machine or hardened process.
- Treat direct key-on-agent mode as dev/test only.

## 7.2 Credential model

- Use least-privilege macaroons by role:
  - buyer agents: pay-only.
  - seller services: invoice-only.
  - monitoring tools: read-only.
  - signer RPC: signer-only.
- Each macaroon should include:
  - explicit scope.
  - expiry.
  - optional spend cap.
  - revocation plan.

## 7.3 Budget guardrails

Enforce at multiple layers:

1. Request-level max cost.
2. Session-level and daily spend caps.
3. Agent-level budget allocations.
4. Emergency kill switch.

## 7.4 Secret handling

- No macaroons or private keys in browser local storage.
- No signer credentials in mobile bundles.
- Worker and Convex only hold minimum required secrets for managed mode.
- Desktop secrets stored in OS keychain/secure enclave-backed storage where available.

## 8. Data Model Recommendations (Convex + Receipts)

## 8.1 Extend Convex schema

Add tables (suggested names):

- `walletAccounts`
  - accountId, ownerId, mode (`managed|connected|sovereign`), status, createdAtMs, updatedAtMs
- `walletPolicies`
  - policyId, ownerId, maxPerRequestMsats, maxDailyMsats, allowlist, denylist, updatedAtMs
- `walletPayments`
  - paymentId, accountId, rail, asset_id, amount_msats, direction, status, invoiceHash, preimageHashRef, endpoint, createdAtMs
- `walletInvoices`
  - invoiceId, accountId, amount_msats, status, paymentHash, expiresAtMs, metadata, createdAtMs
- `walletCredentials`
  - credentialId, accountId, kind (`l402_token|macaroon_ref`), scope, expiresAtMs, status
- `walletAuditEvents`
  - eventId, actorId, eventType, payload, createdAtMs

## 8.2 Extend receipts usage

Current `receipts.kind` values in `apps/web/convex/schema.ts` are:

- `model`
- `tool`
- `dse.predict`

Recommendation:

- Add payment-aware kinds, for example:
  - `payment.l402`
  - `payment.invoice`
  - `payment.refund` (future)

Each payment receipt entry should carry:

- `rail`
- `asset_id`
- `amount_msats`
- `payment_proof` (`lightning_preimage`)
- related request/run/thread identifiers
- deterministic hashes for request params and response

This keeps ADR-0013 fully respected while making payment analytics first-class.

## 8.3 Chat/event model integration

Current chat parts already support extensible unknown part types.
Add explicit typed parts for payment visibility:

- `payment.start`
- `payment.quote`
- `payment.sent`
- `payment.failed`
- `payment.refund` (future)

This gives users transparent payment traces inside agent sessions.

## 9. Tooling and Contracts Roadmap

## 9.1 New tool contracts in `apps/autopilot-worker/src/tools.ts`

Add first-class tools with strict schemas:

- `lightning_pay_invoice`
  - Input: invoice, maxCostMsats, policyRef
  - Output: paymentId, amountMsats, preimageRef, routeHints

- `lightning_l402_fetch`
  - Input: url, method, headers, body, maxCostMsats, cachePolicy
  - Output: status, headers, body, paidAmountMsats, paymentProofRef

- `lightning_create_invoice`
  - Input: amountMsats, memo, expirySec, metadata
  - Output: invoice, paymentHash, expiresAtMs

- `lightning_get_balance`
  - Input: accountRef
  - Output: confirmedMsats, pendingMsats

- `lightning_list_activity`
  - Input: filters, page
  - Output: payments/invoices list

- `lightning_policy_set_limits`
  - Input: policy fields
  - Output: policy snapshot

## 9.2 Runtime enforcement requirements

Per ADR-0007, each tool must provide:

- JSON schema validation before execution.
- deterministic timeout behavior.
- receipt emission with hashes and side effects.
- replay-safe result shape.

## 9.3 DSE signature opportunities

Add signatures for policy and routing decisions:

- `SelectPaymentRoute`
- `AssessPaymentRisk`
- `DecidePaymentApprovalMode`
- `ClassifyL402EndpointTrustTier`
- `RecommendBudgetAdjustment`

These should output confidence and support counterfactual logging.

## 10. L402 Integration Plan

## 10.1 Buyer flow (OpenAgents pays)

Flow:

1. Request target API.
2. If 402 + L402 challenge:
3. Parse invoice + macaroon.
4. Verify policy and cost cap.
5. Pay invoice via execution adapter.
6. Obtain preimage.
7. Retry with `Authorization: L402 macaroon:preimage`.
8. Cache token according to policy.
9. Emit payment receipt entry.

## 10.2 Seller flow (OpenAgents gets paid)

Flow:

1. Endpoint is placed behind Aperture (or equivalent L402 gateway).
2. Pricing policy defined per route/operation.
3. Buyer receives 402 challenge.
4. Buyer pays and retries.
5. Backend receives authorized request.
6. OpenAgents records invoice/payment reconciliation and event receipts.

## 10.3 Token caching policy

Cache keys:

- domain
- path scope (optional stricter mode)
- credential scope
- expiry

Never cache indefinitely.
Respect server revocation/challenge refresh.

## 11. API Surface Recommendations

## 11.1 Worker endpoints (`apps/web/src/effuse-host/worker.ts`)

Add namespace:

- `/api/lightning/*`

Suggested endpoints:

- `POST /api/lightning/pay-invoice`
- `POST /api/lightning/l402/fetch`
- `POST /api/lightning/create-invoice`
- `GET /api/lightning/balance`
- `GET /api/lightning/activity`
- `POST /api/lightning/policy`
- `POST /api/lightning/credentials/bake` (managed mode only, limited)

## 11.2 Service abstraction in web Effect layer

Add service:

- `apps/web/src/effect/lightning.ts`

Pattern should match existing service style (`contracts.ts`, `chat.ts`, etc):

- typed error classes
- typed request/response contracts
- telemetry events namespace (for example `lightning.api.*`)
- server/client context-aware fetch behavior

## 11.3 Desktop bridge API (for future Electron)

Define local IPC or localhost-only API for:

- `payInvoice`
- `createInvoice`
- `getNodeStatus`
- `bakeMacaroon`
- `listChannels`
- `listPayments`

Web app can optionally connect to this bridge when desktop is linked.

## 12. Implementation by Repository Area

## 12.1 `apps/autopilot-worker`

Add:

- Lightning tool contracts and handlers.
- payment receipt emission helpers.
- optional read-only node diagnostic tools first (low risk).

Do not:

- hard-couple worker handlers to shell execution.

## 12.2 `apps/web`

Add:

- Lightning API handlers in worker host.
- Effect services and atoms for wallet state.
- new Effuse pages for wallet and payment policy.
- telemetry hooks and trace correlation for payment events.

## 12.3 `apps/mobile`

Add:

- wallet screens and navigation entries.
- payment history and invoice actions.
- policy confirmation UX for risky outbound payments.

## 12.4 `packages/*`

Add shared packages:

- `packages/lightning-effect`

Optional later:

- `packages/lightning-ui` for reusable wallet UI fragments.

## 13. Testing and Verification Plan

## 13.1 Unit tests

- L402 header parser and serializer.
- budget and policy enforcement edge cases.
- token cache invalidation behavior.
- receipt serialization and hash determinism.

## 13.2 Integration tests

- End-to-end L402 challenge/pay/retry against controlled test endpoint.
- payment proof mapping into receipt records.
- policy rejection scenarios (over budget, domain blocked).

## 13.3 Environment tests

- Regtest and testnet harnesses for deterministic payment tests.
- Desktop host integration tests for local node + remote signer wiring.

## 13.4 Web E2E tests (`packages/effuse-test`)

Add scenarios:

- user triggers paid API call from agent.
- UI shows payment state transitions.
- receipts visible in admin traces.

## 13.5 Failure testing

Simulate:

- invoice expired.
- insufficient balance.
- preimage retrieval failure.
- gateway timeout.
- duplicate payment request retry/idempotency behavior.

## 14. Observability and Operations

## 14.1 Metrics

Track:

- payment attempts/success/fail rates
- average paid amount
- 402 frequency per endpoint
- retry counts
- preimage retrieval failures
- policy denials
- per-agent spend over time

## 14.2 Logs

Every payment path should include:

- request correlation id
- wallet account id
- endpoint or operation id
- policy id
- outcome and reason

Never log raw secrets or raw preimages in plain text logs.

## 14.3 Alerts

Set alerts for:

- sudden spend spikes
- repeated preimage retrieval failures
- gateway challenge anomalies
- sustained payment failure rate above threshold

## 15. Phased Delivery Roadmap

## Phase 0: Design lock and contracts (1-2 weeks)

Deliverables:

- finalized shared schemas/services (`lightning-effect`)
- ADR for Lightning integration architecture
- threat model + custody mode definitions

Exit criteria:

- schema review approved
- security review approved

## Phase 1: Buyer path MVP in managed mode (2-4 weeks)

Deliverables:

- L402 client library in Effect (`packages/lightning-effect/l402`)
- worker API endpoint for controlled L402 fetch
- payment receipts with `lightning_preimage` proof mapping
- minimal web wallet activity page

Exit criteria:

- deterministic paid request flow in staging
- receipts visible and auditable

## Phase 2: Policy and budget controls (2-3 weeks)

Deliverables:

- per-agent spend limits
- per-request max cost guards
- domain allowlist/denylist
- operator kill switch

Exit criteria:

- policy enforcement test matrix passes

## Phase 3: Seller path MVP (2-4 weeks)

Deliverables:

- aperture-backed paid endpoint runbook and integration
- pricing policy and route mapping
- payment reconciliation records

Exit criteria:

- one production-safe paid endpoint online in staging

## Phase 4: Mobile wallet surface (2-4 weeks)

Deliverables:

- mobile wallet dashboard
- receive/send controlled flows
- payment activity and alerts

Exit criteria:

- parity on core wallet read operations with web

## Phase 5: Electron sovereign host (4-8 weeks)

Deliverables:

- Expand existing `apps/desktop` shell into a sovereign Lightning host module
- local node status + remote signer support
- local `lnget` adapter
- desktop-to-web linking flow

Exit criteria:

- sovereign desktop mode can pay L402 API and return receipt proof to OpenAgents control plane

## Phase 6: Advanced optimization and marketplace alignment

Deliverables:

- agent-native commerce templates
- dynamic pricing hooks
- smarter policy/routing signatures in DSE

Exit criteria:

- stable multi-agent buyer/seller loop with telemetry and budget safeguards

## 16. Priority Backlog (Actionable)

P0:

1. Add Lightning integration ADR for current TS/Effect stack.
2. Define shared payment schemas and receipt contract mappings.
3. Expand `packages/lightning-effect` from buyer-only contracts into seller-side service interfaces.
4. Implement seller paywall control-plane records in Convex schema and APIs.
5. Add paywall/settlement UI surfaces in web alongside current L402 wallet/transactions panes.

P1:

1. Add policy and cap enforcement.
2. Add typed Lightning tool contracts in `apps/autopilot-worker`.
3. Add E2E tests for paid request flow.
4. Add mobile wallet read-only screens.

P2:

1. Add seller/Aperture runbooks and integration support.
2. Add desktop sovereign host with remote signer workflow.
3. Add richer analytics, risk scoring, and anomaly alerts.

## 17. Risks and Mitigations

Risk: Worker runtime cannot execute native tooling.
Mitigation: Protocol-native TS client + external/native adapter boundary.

Risk: Upstream skill scripts for `lnget`/`aperture` are currently fragile due to Go module layout.
Mitigation: Pin source-build runbooks and CI checks for deterministic binaries.

Risk: Custody/security complexity can slow product delivery.
Mitigation: staged custody modes with strict defaults and explicit opt-in for sovereign mode.

Risk: Receipt drift between payment paths.
Mitigation: central shared schema package + contract tests + ADR alignment checks.

Risk: Overly permissive autonomous spend behavior.
Mitigation: default-deny policy, hard caps, and approval thresholds for higher risk actions.

## 18. Immediate Next Steps (First 7 Days)

1. Create an ADR: "Lightning integration architecture for web/mobile/desktop TS stack."
2. Extend `packages/lightning-effect` with seller-side schema/service drafts and test fixtures.
3. Add Convex schema draft tables for paywalls/routes/pricing/settlements (feature-gated).
4. Add paywall CRUD + status API placeholders in web worker host.
5. Add one end-to-end staging runbook for Voltage + Aperture + OpenAgents control-plane integration.
6. Add desktop-to-web account-linking protocol notes for the existing `apps/desktop` app.
7. Add docs runbook for local regtest + Docker-based integration testing.

## 19. Definition of Done for "Deep Integration"

OpenAgents has "deep Lightning integration" only when all are true:

1. Agents can pay L402 endpoints with policy-safe caps and receipt proofs.
2. OpenAgents can expose at least one paid endpoint behind L402 with reconciliation.
3. Web and mobile users can inspect balances, spend, and payment outcomes.
4. Desktop sovereign mode supports remote signer architecture and scoped credentials.
5. Every payment action is traceable via deterministic receipts and telemetry.
6. Integration is tested in automated unit, integration, and E2E suites.

## 20. Final Recommendation

Pursue a hybrid architecture immediately:

- Shared Effect L402/payment contract stack via `packages/lightning-effect` for control-plane portability and external reuse.
- Managed execution for fast web/mobile rollout.
- Desktop native host for sovereign and advanced operational workflows.

This approach matches the current OpenAgents architecture, preserves security boundaries, and gives a realistic path from MVP payments to full agent commerce infrastructure.
