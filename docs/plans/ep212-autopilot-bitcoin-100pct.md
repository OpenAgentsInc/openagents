# EP212: Autopilot Learns Bitcoin (Announcement) — 100% Implementation Plan

Status: draft (active)
Last updated: 2026-02-24
Scope: Rust-first product surfaces (web + desktop + mobile), wallet executor, lightning-ops, and protocol/docs parity.

This document audits the Episode 212 announcement claims ("Autopilot Learns Bitcoin") against the current codebase and roadmap, identifies gaps, and specifies the work required to make the announcement **literally true in production** (not "works in dev").

## Authority And Constraints (Non-Negotiable)

Checked authorities (required by `AGENTS.md`):

- Invariants: `docs/plans/rust-migration-invariant-gates.md`
- ADRs:
  - `docs/adr/ADR-0001-rust-only-architecture-baseline.md`
  - `docs/adr/ADR-0002-proto-first-contract-governance.md`
  - `docs/adr/ADR-0007-spacetime-only-sync-transport-hard-mandate.md`
  - `docs/adr/ADR-0005-wallet-executor-auth-custody-receipts.md`
  - `docs/adr/ADR-0006-bounded-vercel-sse-compatibility-lane.md`

Normative constraints this plan must obey:

- Proto-first for cross-boundary contracts (`INV-01`, `ADR-0002`). JSON is an *interop view*, not contract authority.
- Authority mutations are authenticated HTTP only (`INV-02`).
- Live delivery lanes are WS-only for Spacetime (`INV-03`, `ADR-0007`). SSE exists only as a bounded presentation adapter (`ADR-0006`).
- Wallet executor is the payment signing authority. Control-plane must **not** store mnemonics or other seed material in plaintext (`ADR-0005`).
- Rust-only endstate (`ADR-0001`). Any PHP/TypeScript implementation of core product behavior is migration debt to delete once Rust parity lands.
- `.github/` workflow automation is forbidden (`INV-12`).

## Goal And Definition Of Done

**Goal:** Everything stated in the EP212 announcement is true for a normal user in staging/prod:

- Wallet send/receive works with Spark.
- L402 consumption works against arbitrary L402 endpoints (subject to policy allowlist/caps).
- L402 paywalls are self-serve and actually earn BTC (settlement to the creator's wallet, with receipts).
- Autopilot-to-Autopilot negotiation is not "vibes": it has concrete message classes, policies, and receipts.
- `openapi.json` and docs are accurate (no "fake" examples for production paths).

**Definition of Done (EP212 Parity Gate):** A single reproducible harness run produces:

1. A real Spark wallet for a user (no fake mnemonic/addresses), balance sync, send, and invoice receive.
2. An L402 fetch to a real L402 endpoint (e.g. `sats4ai.com`) resulting in a paid receipt (or approval flow).
3. A user-created paywall deployed to the hosted gateway, with at least one paid request, and a settlement credited to the paywall owner.
4. A negotiated service flow between two Autopilots that results in either:
   - a paid L402 request with receipts; or
   - a signed commerce message envelope + receipts (see `proto/openagents/runtime/v1/commerce.proto`).

This gate must be runnable locally in mock mode and in staging with real services.

## EP212 Announcement Claims (Requirements)

From the announcement text:

1. **Send/receive bitcoin instantly (via Spark).**
2. **Pay for services from any Lightning L402 endpoint.**
3. **Earn bitcoin by creating L402 paywalls for any HTTP endpoint.**
4. **Communicate with other Autopilots to negotiate services.**
5. **Single website experience**: `openagents.com` provides a familiar ChatGPT-style UI with Bitcoin as a native experience.
6. **OpenAgents API** supports:
   - autopilot creation and management
   - threaded chat
   - Lightning payments
   - L402 paywalls
   - OpenAPI docs at `/openapi.json`
7. "Coming soon" API support:
   - teams, projects
   - skills
   - third-party integrations (e.g. GitHub)
   - developer plugins
   - Bitcoin revenue sharing
   - marketplace contributions (tools/skills/plugins/data sources)

## Current Implementation Audit (Reality Check)

This section is deliberately blunt: what exists today vs what is stubbed, legacy-only, or admin-gated.

### A) Spark Wallet Send/Receive (Claim #1)

**Rust control service endpoints exist** (`apps/openagents.com/src/openapi.rs`, `apps/openagents.com/src/lib.rs`):

- `GET/POST /api/agent-payments/wallet`
- `GET /api/agent-payments/balance`
- `POST /api/agent-payments/invoice`
- `POST /api/agent-payments/pay`
- `POST /api/agent-payments/send-spark`

**But multiple endpoints are not real:**

- `agent_payments_create_invoice` returns a fake BOLT11 string (`lnbc{amount}n1{uuid}`) in `apps/openagents.com/src/lib.rs` (`async fn agent_payments_create_invoice`).
- `agent_payments_send_spark` is stubbed (returns `"completed"` with generated id) in `apps/openagents.com/src/lib.rs` (`async fn agent_payments_send_spark`).
- Wallet upsert generates fake mnemonic and synthetic addresses in `apps/openagents.com/src/lib.rs` (`async fn upsert_agent_wallet_for_user`).
- The control-plane stores the mnemonic as plaintext in `apps/openagents.com/src/domain_store.rs` (`UserSparkWalletRecord.mnemonic`) which violates the custody direction in `docs/adr/ADR-0005-wallet-executor-auth-custody-receipts.md`.

**Wallet executor exists and can do real work**:

- `apps/lightning-wallet-executor/` exposes HTTP endpoints:
  - `POST /wallets/create`
  - `POST /wallets/status`
  - `POST /wallets/create-invoice`
  - `POST /wallets/pay-bolt11`
  - `POST /wallets/send-spark`
  - plus canonical `POST /pay-bolt11` returning a deterministic `receipt` (`openagents.lightning.wallet_receipt.v1`), see `proto/openagents/lightning/v1/wallet_executor.proto` and `docs/protocol/fixtures/lightning-wallet-executor-receipt-v1.json`.

**Conclusion:** Claim #1 is **not** 100% implemented in Rust control-plane. It is partially wired (paying invoices via executor compat path), but wallet creation/import, send, and receive are not production-real and custody is not compliant.

### B) L402 Consumption Tools (Claim #2)

**Control-plane policy and strings exist**:

- Autopilot policy includes `l402RequireApproval`, `l402MaxSpendMsatsPerCall`, `l402MaxSpendMsatsPerDay`, `l402AllowedHosts` in `apps/openagents.com/src/lib.rs` and `apps/openagents.com/src/domain_store.rs`.
- L402 receipt and credential stores exist in `apps/openagents.com/src/domain_store.rs`:
  - `upsert_l402_credential`, `list_active_l402_credentials`
  - `record_l402_receipt`, `list_l402_receipts_for_user`

**But the actual tool implementations are missing in Rust**:

- `lightning_l402_fetch` and `lightning_l402_approve` are referenced as tool names in Rust, but no Rust executor exists (only strings in `apps/openagents.com/src/lib.rs` + `apps/openagents.com/src/openapi.rs`).
- The only working implementation is still in legacy Laravel/PHP:
  - `apps/openagents.com/app/AI/Tools/LightningL402FetchTool.php`
  - `apps/openagents.com/app/AI/Tools/LightningL402ApproveTool.php`
  - `apps/openagents.com/app/Lightning/L402/L402Client.php`

**Rust primitives exist but are not composed into a tool**:

- `crates/openagents-l402/` implements:
  - `WwwAuthenticateParser` (parse `WWW-Authenticate` L402 challenge)
  - `Bolt11::amount_msats` (cap enforcement)
  - `L402CredentialCache` (TTL credential caching)

**Conclusion:** Claim #2 is **legacy-only** today. We need Rust tool implementations + receipts + approval queue + policy enforcement.

### C) L402 Paywalls (Claim #3)

**Paywall CRUD endpoints exist in Rust**:

- `GET /api/l402/paywalls` is user-accessible.
- `POST/PATCH/DELETE /api/l402/paywalls...` exist but are **admin-gated** by `admin_email_gate` in `apps/openagents.com/src/lib.rs` (protected router wiring).

**Deployment workflow exists as a control-plane + CLI, but not self-serve productized:**

- `apps/lightning-ops/` is a Rust CLI/service that talks to control-plane internal endpoints:
  - `/api/internal/lightning-ops/control-plane/query`
  - `/api/internal/lightning-ops/control-plane/mutation`
  - and can run EP212 smoke flows (`smoke:ep212-full-flow`), see `apps/lightning-ops/README.md`.

**But "earn BTC" is not end-to-end demonstrated by product code**:

- Rust `l402_settlements` today is derived from **consumption receipts** (paid L402 fetches), not "creator earnings settlement".
- There is no user-facing, non-admin flow that:
  1. verifies domain control for an arbitrary upstream (to avoid open proxy abuse),
  2. deploys paywall config to the hosted gateway automatically,
  3. credits earned funds to the paywall creator's wallet with receipts.

**Conclusion:** Claim #3 is **partially present** (paywall data model + admin tooling), but not a self-serve, safe, earning loop.

### D) Autopilot-to-Autopilot Negotiation (Claim #4)

What exists:

- Basic communication primitives:
  - Public posts: `GET/POST /api/shouts`
  - Private messaging: `GET/POST /api/whispers` (see `whispers_store` in `apps/openagents.com/src/lib.rs`)

What is missing for "negotiate services":

- A concrete commerce grammar and message classes for offers/accept/receipts in product surfaces.
  - The proto contract exists: `proto/openagents/runtime/v1/commerce.proto`.
  - The end-user product does not yet expose negotiation flows or receipts as first-class surfaces.

**Conclusion:** "Communicate" exists; "negotiate services" is not yet a coherent product loop.

### E) "Single Website / ChatGPT-style UI + Bitcoin Native" (Claim #5)

The Rust control service serves HTMX/Maud UI for multiple surfaces (e.g. `/l402`, `/compute`, `/codex`).

Gaps (based on current code and EP212 requirements):

- Wallet UI must be backed by real wallet operations (today: invoice/send are stubbed).
- L402 consumption must be visible in chat as a native action:
  - approval queue UI
  - receipts viewer
  - policy controls per Autopilot
- Paywall creation must be self-serve and safe (today: admin-only).

### F) OpenAgents API + OpenAPI Docs (Claims #6/#7)

What exists:

- Rust OpenAPI generation exists: `apps/openagents.com/src/openapi.rs` and is served at `/openapi.json` by `apps/openagents.com/src/lib.rs`.
- Autopilot + threaded chat + payment routes exist.

Gaps:

- Some documented capabilities are stubbed (wallet invoice creation, spark send).
- Some "working" capabilities live only in legacy Laravel/PHP (L402 tools).
- Persistence and production semantics (idempotency, receipts, quotas, rate limits) are inconsistent across lanes.

## Required Work (Phased, Dependency-Based; Timeline-Free)

This plan is organized around “make the EP212 Parity Gate harness pass” with minimal drift.

### Phase 1: Make Wallet Operations Real And Custody-Compliant

**Objective:** Claim #1 becomes true without violating `ADR-0005`.

Work items:

1. **Define custody model for user wallets (required decision).**
   - Recommended: wallet mnemonic stored only in wallet executor custody store (or secret manager reference owned by executor). Control-plane stores only `wallet_id` and public metadata.
   - Explicitly outlaw: plaintext mnemonic in control-plane storage (`apps/openagents.com/src/domain_store.rs`).

2. **Replace fake wallet upsert with real executor-backed wallet lifecycle.**
   - Update `POST /api/agent-payments/wallet` to:
     - create wallet if absent (executor generates mnemonic)
     - import wallet if mnemonic provided (one-time ingestion)
   - Update `GET /api/agent-payments/wallet` to return:
     - spark address
     - lightning address
     - identity pubkey (if available)
     - status lifecycle from executor
   - Update `GET /api/agent-payments/balance` to call executor status and persist snapshot fields.

3. **Implement receive path: real invoice creation.**
   - Replace fake `agent_payments_create_invoice` with executor call (`/wallets/create-invoice` or a canonical non-compat endpoint).
   - Ensure invoice expiry semantics are surfaced (Spark response may not include; document behavior).

4. **Implement send path: spark transfer.**
   - Replace stub `agent_payments_send_spark` with executor call (`/wallets/send-spark`).

5. **Normalize pay-invoice path to the canonical executor API and receipts.**
   - Prefer canonical `POST /pay-bolt11` (returns `receipt`) over compat `/wallets/pay-bolt11`.
   - Ensure host allowlist/caps are enforced by executor and surfaced by control-plane.

6. **Persistence and rotation.**
   - Ensure wallet records and receipts persist across deploys (Cloud Run cold starts cannot erase state).
   - For any secret references, implement rotation runbooks aligned with `docs/adr/ADR-0005-wallet-executor-auth-custody-receipts.md` and `apps/lightning-wallet-executor/docs/KEY_ROTATION_RUNBOOK.md`.

Acceptance criteria:

- No fake `lnbc...` invoices generated by control-plane.
- No plaintext mnemonic stored in control-plane persistence.
- Wallet send/receive succeeds against Spark in staging with deterministic receipts.
- `docs/protocol/fixtures/lightning-wallet-executor-receipt-v1.json` remains valid and new receipts match contract.

### Phase 2: Implement Rust L402 Client + Tooling (Fetch/Approve) With Receipts

**Objective:** Claim #2 becomes true without PHP tooling.

Work items:

1. **Implement `lightning_l402_fetch` in Rust.**
   - Use `crates/openagents-l402` `WwwAuthenticateParser` to parse L402 challenge.
   - Implement "credential cache" semantics using control-plane store:
     - Read existing credential: `DomainStore.list_active_l402_credentials` (or a new per-user/host/scope lookup).
     - Cache new credential after payment with TTL.
   - Implement L402 request flow:
     1. Attempt with cached Authorization header.
     2. If 402, parse challenge.
     3. Enforce cap using `Bolt11::amount_msats`.
     4. Pay invoice via wallet executor (pay-after-verify semantics do not apply; this is immediate spend).
     5. Retry with Authorization header.
   - Emit a **receipt event** to `DomainStore.record_l402_receipt` for all outcomes: cached/paid/blocked/failed.

2. **Implement approval queue + `lightning_l402_approve`.**
   - When autopilot policy requires approval (`l402RequireApproval=true`), `fetch` should enqueue an intent and return a `taskId` rather than spending.
   - `approve` consumes the taskId and executes the fetch+pay flow.
   - Approval intent storage must be durable and replay-safe (idempotency keys; expiration).

3. **Implement policy enforcement in Rust.**
   - Enforce:
     - allowed hosts (autopilot-scoped)
     - per-call max spend
     - per-day max spend (requires spend ledger)
     - require approval default
   - Mirror the Laravel behavior encoded in:
     - `apps/openagents.com/app/Lightning/L402/L402PolicyEnforcer.php`
     - `apps/openagents.com/app/AI/Tools/LightningL402FetchTool.php`

4. **Integrate tool specs into Rust skill registry.**
   - Add tool specs (and/or a new tool pack if required) so runtime can expose L402 tools in the same way coding tools are exposed.

5. **Delete legacy PHP L402 tool implementations after parity.**
   - Remove:
     - `apps/openagents.com/app/AI/Tools/LightningL402FetchTool.php`
     - `apps/openagents.com/app/AI/Tools/LightningL402ApproveTool.php`
     - `apps/openagents.com/app/Lightning/L402/*` (as applicable)
   - Ensure no production code path depends on Laravel AI/Vercel protocol lanes for L402.

Acceptance criteria:

- A Rust-owned tool call to `lightning_l402_fetch` can pay and fetch from a real L402 endpoint.
- Approval flow works: fetch returns `approval_requested`, approve completes spend and fetch.
- Each fetch produces a durable L402 receipt record viewable at `/api/l402/transactions` and `/l402`.

### Phase 3: Make L402 Paywalls Self-Serve, Safe, And Profitable

**Objective:** Claim #3 becomes true for a normal user (not admin), including earnings settlement.

Work items:

1. **Unblock self-serve paywall CRUD with safety controls.**
   - Remove admin gating for `POST/PATCH/DELETE /api/l402/paywalls` *only after* the controls below exist.
   - Add required constraints for arbitrary upstream:
     - domain ownership verification (DNS or HTTP well-known challenge)
     - denylist of private IP ranges / internal network egress
     - upstream protocol restrictions
     - rate limits and quotas per owner

2. **Make deployment automatic and observable.**
   - Define the product path: paywall change -> compile -> deploy -> status -> rollback.
   - `apps/lightning-ops` already provides compile/reconcile flows against internal control-plane endpoints; productize it:
     - either run it as a service (control-plane triggers) or a scheduled reconciler
     - persist deployment intents/events per owner and surface them at `/api/l402/deployments` and `/l402`

3. **Implement earnings settlement to creator wallet.**
   - Define the settlement ledger:
     - what counts as "earned" (paid request, net of fees)
     - how often settlement executes (batching)
     - idempotency keys and receipts for payouts
   - Wire to wallet executor for creator payout (Spark send or invoice pay).
   - Emit receipts: both gateway settlement proof and wallet executor receipt.

4. **Expose paywall creation/management as tools (optional but strongly aligned with claim).**
   - Implement `lightning_l402_paywall_create/update/delete` tool pack in Rust.
   - Bind tool calls to ownership policy and require explicit confirmation for mutation.

Acceptance criteria:

- A non-admin user can create a paywall, deploy it, and see it active.
- A paid request generates a settlement record and credits the creator wallet.
- Earnings appear in UI and via API with receipts (creator can reconcile).

### Phase 4: Autopilot Negotiation Surfaces (Services, Not Just Chat)

**Objective:** Claim #4 becomes a coherent loop (negotiate services) rather than only messaging.

Work items:

1. **Define “negotiation” message classes and trust zones.**
   - Use `proto/openagents/runtime/v1/commerce.proto` as the contract grammar.
   - Decide which messages are:
     - authority mutations (signed + receipted)
     - ephemeral coordination (session-authenticated)
   - Align with the Nexus/Nostr boundary decisions in the main economy plan (`docs/plans/2026-02-23-open-agent-economy-execution-plan.md`).

2. **Implement minimum negotiation loop on the web surface.**
   - Discover/lookup another Autopilot/user.
   - Propose an offer (service + price + terms).
   - Accept and execute via either:
     - L402 protected endpoint call; or
     - marketplace order acceptance.
   - Emit receipts for the economic action.

3. **Interop (Nostr) is “Next”, but plan the boundary now.**
   - Keep EP212 minimal: internal negotiation can start via OpenAgents API.
   - Mirror only the portable event kinds via the Bridge when ready (do not drift into “everything is Nostr now” prematurely).

Acceptance criteria:

- Two accounts can complete a service negotiation that results in:
  - a paid L402 request and receipts; or
  - a `CommerceEnvelope` receipt emitted and visible in UI.

### Phase 5: Web UI And Docs Parity (Make It Obvious)

**Objective:** Claim #5 and #6 are true in a way a user can understand.

Work items:

1. **Chat surface + Bitcoin native UI**
   - Wallet panel: balance, send, receive (invoice), recent receipts.
   - L402 panel: approvals, receipts, policy view.
   - Paywalls panel: create/update, deploy status, earnings.

2. **Make `/openapi.json` and product docs accurate**
   - Eliminate examples that imply fake functionality.
   - Add “staging mock vs prod live” semantics explicitly where relevant.
   - Add docs for:
     - wallet executor integration (at a product/API level)
     - L402 tool semantics and approval model
     - paywall safety constraints and domain verification

Acceptance criteria:

- A new user can perform the EP212 parity harness steps from the web UI without privileged access.

### Phase 6: “Coming Soon” Roadmap Commitments (Make Them Real)

The announcement lists multiple “coming soon” API areas. The broader economy execution plan already contains many of these concepts; EP212 parity requires that we:

- explicitly map which are already underway vs missing,
- scope the minimal “first shipped” subset,
- and make them visible as issues/epics (without timelines).

Minimum required work to avoid “promise drift”:

1. **Teams + projects**
   - data model (org/project membership)
   - budgets per org/project
   - policy surfaces and audit

2. **Skills + plugins**
   - skill registry versioning + publishing gates
   - tool spec governance
   - sandbox + receipts required for paid tool invocations

3. **GitHub integration**
   - already present in coding tool specs (`github.primary`) but needs production hardening and receipts.

4. **Bitcoin revenue sharing**
   - define receipt-linked splits (contributors/tools/providers)
   - implement settlement batching + payout receipts
   - UI + API to inspect splits and earnings

## EP212 Parity Harness (Executable Proof)

We need one command that proves EP212 end-to-end.

Recommended shape:

- `./scripts/ep212-parity.sh --mode mock` (local, deterministic)
- `./scripts/ep212-parity.sh --mode staging` (real services, secrets required)

Inputs:

- control service base URL
- wallet executor base URL + auth token
- L402 endpoint test target(s)
- optionally a paywall upstream target + domain verification credentials

Outputs (artifacts):

- `output/ep212-parity/<requestId>/events.jsonl`
- `output/ep212-parity/<requestId>/summary.json`
- links to receipts persisted in control-plane storage

We should reuse the existing `apps/lightning-ops` smoke flows where possible:

- `cargo run --manifest-path apps/lightning-ops/Cargo.toml -- smoke:ep212-full-flow --json --mode mock`
- `cargo run --manifest-path apps/lightning-ops/Cargo.toml -- smoke:ep212-full-flow --json --mode api`

## Issue Slicing (Proposed; Create After Approval)

When turning this plan into GitHub issues, keep them small and gate-driven. Suggested epic breakdown:

- EP212-WALLET-*: custody + wallet executor-backed send/receive + receipts
- EP212-L402-*: rust tool + approval queue + receipts + legacy deletion
- EP212-PAYWALL-*: self-serve + deployment automation + earnings settlement
- EP212-NEGOTIATION-*: commerce message loop + receipts + interop boundary
- EP212-UX-*: web UI integration + docs/OpenAPI accuracy gates
- EP212-SOON-*: teams/projects/skills/plugins/revenue share minimal shipped subset

## Notes / Known Risk Areas (Do Not Skip)

1. **Custody is the highest-risk area.** If mnemonics leak into logs/db, this is a security incident. Treat “no plaintext seeds outside executor” as a hard gate.
2. **Paywalls are an SSRF/open-proxy risk.** “Any HTTP endpoint” requires domain verification + outbound egress controls.
3. **Idempotency and receipts** must be first-class (especially for money). Every spend/settlement must be replay-safe.
4. **Performance parity**: tools must not add multi-second latencies; long polls/SSE should not reappear as authority transports.
