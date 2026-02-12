# OpenAgents L402 Agent Paywall Infrastructure Plan (Voltage + GCP)

- Date: 2026-02-11
- Status: Implemented baseline complete (Phase 1-5 delivered as of 2026-02-12)
- Owner: Lightning / Autopilot product + runtime

## 1) Purpose and Decision Summary

This plan defines how OpenAgents will run its own multi-tenant L402 paywall infrastructure so agents can create, manage, and monetize paid endpoints through OpenAgents-managed rails.

Core decisions:

1. **Seller infrastructure is OpenAgents-hosted** (not only third-party seller endpoints).
2. **Voltage-hosted LND is the initial Lightning backend** for fast production delivery.
3. **All OpenAgents custom logic remains TypeScript + Effect** in this repo; only upstream Go components (Aperture and optional pricer/controller) run as isolated infra services.
4. **Any Go-hosted workloads run on Google Cloud**.
5. **Control plane authority remains in `apps/web` + Convex**, with typed contracts and replayable receipts enforced by existing runtime standards.

This is sequencing and architecture guidance for full implementation, not marketing copy.

## 1.1) Local-Track Handoff Contract (from #1605)

Hosted rollout is explicitly downstream of the local-node execution baseline. Before staging gateway rollout is considered ready, hosted phases must preserve contract compatibility with outputs from the local Neutrino track:

1. Task lifecycle parity:
   - `queued/approved/running/paid/cached/blocked/failed/completed`
2. Proof + receipt parity:
   - `paymentProofRef` (preimage reference) available on successful paid path
   - deterministic deny reason on blocked path (for example budget/policy)
3. Correlation parity:
   - request/task/user identifiers preserved end-to-end
   - artifact-friendly IDs for non-interactive agent verification
4. Execution-path labeling:
   - explicit `executionPath` (`local-node` or `hosted-node`) in telemetry/events

Sequencing implication:

1. `#1597` and `#1598` may proceed in parallel with local track.
2. `#1599` is gated by local baseline validation (`#1614`) and synchronization phase (`#1615`).
3. `#1604` must gate on both hosted flow health and local-node regression parity.

## 1.2) Epic #1595 Completion Snapshot (2026-02-12)

Hosted-L402 implementation phases tracked under this epic are complete:

1. `#1596` closed: hosted paywall contracts/services (`packages/lightning-effect`)
2. `#1597` closed: control-plane schema + lifecycle APIs (`apps/web` + Convex)
3. `#1598` closed: deterministic Aperture config compiler (`apps/lightning-ops`)
4. `#1599` closed: staging deploy/reconcile workflow (Aperture + Voltage)
5. `#1600` closed: settlement ingestion + proof correlation
6. `#1601` closed: credential security/rotation/emergency controls
7. `#1602` closed: Autopilot paywall tool contracts + receipt-safe execution
8. `#1603` closed: hosted operations panes/admin views
9. `#1604` closed: dual-path full-flow tests + CI gating

Operational gate now in-repo:

1. CI workflow: `.github/workflows/l402-hosted-flow.yml`
2. Hosted full-flow smoke command: `cd apps/lightning-ops && npm run smoke:full-flow -- --json`
3. Hosted E2E spec/tag:
   - id: `apps-web.hosted-l402.full-flow`
   - tag: `l402-hosted`

Artifact and parity expectations now enforced:

1. hosted flow artifacts: `events.jsonl` and `summary.json`
2. local-node parity source: `output/l402-local-node-smoke-artifact.json`
3. required parity keys: `executionPath`, `requestId`, `taskId`, `paymentProofRef`

**Operational runbook (deploy, secrets, how to use and edit):** `docs/lightning/runbooks/L402_APERTURE_DEPLOY_RUNBOOK.md`. Single reference for the live Aperture gateway on GCP; no sensitive values in the repo.

## 2) Repo-Aware Baseline

The plan is grounded in the current codebase, not a greenfield architecture.

### Existing components we should reuse

1. `apps/web/convex/lightning/tasks.ts`
   - Already provides a typed Lightning task queue lifecycle (`queued/approved/running/paid/cached/blocked/failed/completed`) with transition controls and event logs.
   - This is a strong base for control-plane operations and deterministic task traces.
2. `apps/web/src/effect/lightning.ts`
   - Already provides Effect-based Lightning API client wiring from web runtime to Convex functions.
3. `apps/autopilot-worker/src/tools.ts`
   - Already includes `lightning_l402_fetch` schema contracts and output semantics for buyer-side L402 fetch orchestration.
4. `packages/lightning-effect`
   - Already has buyer-side L402 challenge parsing, auth header building, spend policy checks, credential cache, and invoice payment abstractions.
   - Already structured as reusable package for external consumers.
5. `apps/desktop`
   - Existing executor model for off-web action execution.
   - Useful for sovereign or user-local buyer flows, but not required to host seller infra.
6. `apps/web/src/effuse-app/controllers/home/openChatPaneController.ts`
   - Wallet + transaction L402 panes already exist.
   - We should extend these for seller/paywall operations rather than inventing a new UI framework.
7. ADR contract/receipt standards:
   - `docs/adr/ADR-0007-tool-execution-contract.md`
   - `docs/adr/ADR-0013-receipt-schema-payment-proofs.md`

### Gaps we must close

1. No seller-side paywall resource model in Convex.
2. No OpenAgents-managed Aperture deployment + config compiler.
3. No settlement ingestion pipeline mapped to paywall ownership/revenue.
4. No agent tool contract set for paywall CRUD.
5. No robust seller observability/ops runbooks integrated with request IDs and replay semantics.

## 3) Target State

Any agent can create an OpenAgents paywall and receive a stable endpoint (or route binding) that:

1. issues standards-compliant `402 Payment Required` + L402 challenge;
2. accepts valid `Authorization: L402 ...` retries;
3. gates upstream access by paywall policy;
4. records payment proof metadata (`lightning_preimage`) and deterministic receipts;
5. supports pause/resume/kill-switch and spend/abuse controls.

## 4) Architecture (Concrete Plan)

### 4.1 Planes and responsibilities

### A) Control plane (`apps/web` + Convex + Worker APIs)

Responsibilities:

1. Paywall lifecycle APIs (create, update, pause, resume, archive).
2. Ownership mapping (`subject -> paywall`).
3. Pricing + policy authoring (fixed price first, dynamic later).
4. Route binding policy and conflict checks.
5. Audit logs and deployment intent state.

### B) Gateway plane (Aperture, Go, on GCP)

Responsibilities:

1. Enforce L402 challenge/auth handshake.
2. Proxy authorized traffic to upstream seller endpoint.
3. Emit gateway logs/metrics tied to request/paywall IDs.
4. Provide deterministic config-driven behavior from control plane.

### C) Settlement plane (LND + reconciliation services)

Responsibilities:

1. Invoice issuance and state tracking.
2. Settlement confirmation and reconciliation.
3. Paywall revenue attribution and payout eligibility.
4. Credential and macaroon hygiene for least privilege.

### D) Product plane (Autopilot tools + Web panes)

Responsibilities:

1. Agent tool access to paywall CRUD and status.
2. Operator/user UI for paywall status, payment events, and error state.
3. Explainability hooks in chat metadata/receipts.

### 4.2 Runtime topology

```text
[Buyer client/agent]
      |
      v
[OpenAgents Paywall URL]
      |
      v
[GCP HTTPS LB] --> [Aperture (Cloud Run, Go)]
                         |
                         +--> [Voltage LND (gRPC 10009)]  (challenge/invoice auth path)
                         |
                         +--> [Seller upstream endpoint]
                         |
                         +--> [Gateway logs/metrics]

[OpenAgents Control Plane]
  - apps/web Worker API routes
  - Convex models/functions
  - apps/lightning-ops (new, TS+Effect service on Cloud Run)
      |
      +--> compile/deploy aperture config
      +--> ingest settlements
      +--> write deterministic events + receipts
```

### 4.3 Go placement policy

All Go services live on Google Cloud:

1. Aperture binary (mandatory, upstream Go component).
2. Optional dynamic pricing gRPC service if implemented in Go.
3. Optional minimal control shim if Aperture runtime update APIs require in-process Go integration.

All OpenAgents business logic (policy, models, orchestration, receipts) remains TypeScript + Effect in this repo.

## 5) Voltage-Specific Operating Model

Voltage is the initial Lightning provider for seller infrastructure.

### 5.1 Node setup by environment

1. Provision distinct Voltage nodes per environment:
   - dev/test
   - staging
   - production
2. Capture node metadata in secure config:
   - `LND_HOST`
   - `LND_GRPC_PORT=10009`
   - `LND_REST_PORT=8080`
   - node alias / pubkey
3. Export and store in secret manager:
   - TLS cert
   - bootstrap admin macaroon

### 5.2 Macaroon hardening plan

1. Bootstrap with admin macaroon only long enough to bake scoped creds.
2. Bake invoice-only macaroon for gateway challenge flow.
3. Bake read-only macaroon for monitoring and settlement readers.
4. Remove admin macaroon from runtime workloads once scoped credentials are validated.
5. Rotate scoped macaroons on explicit cadence and on incident.

### 5.3 Connectivity and failure strategy

1. Aperture connects to Voltage via direct lnd mode first.
2. Monitor gRPC health and auth failures continuously.
3. On Voltage outage:
   - fail challenge issuance predictably;
   - surface typed error reason in gateway telemetry and control-plane status;
   - allow emergency pause of all paywalls.

## 5.4) Hosted vs Local Role Split (Operator Clarity)

Hosted infrastructure is the seller/paywall control and gateway path. Local-node mode remains a buyer/executor path for user-owned wallets and local deterministic testing.

1. Hosted path (`hosted-node`):
   - OpenAgents-managed paywall URLs
   - Aperture + Voltage challenge/proxy flow
2. Local path (`local-node`):
   - desktop LND Neutrino execution via `lnd-effect` + `lightning-effect`
   - smoke/CI regression path that must continue to pass as hosted stack evolves

These paths are complementary and must stay schema-compatible at the artifact/correlation layer.

## 6) Aperture on Google Cloud (Detailed)

This section assumes Aperture remains the enforcement edge for MVP and early production.

### 6.1 Build and artifact pipeline

1. Pin Aperture git SHA in infra config.
2. Build immutable image in CI (Cloud Build or GitHub Actions + Artifact Registry).
3. Attach SBOM and provenance metadata.
4. Tag images with:
   - upstream SHA
   - OpenAgents rollout ID
   - date stamp
5. Never deploy floating tags (`latest`) in production.

### 6.2 Deployment target

Recommended baseline: Cloud Run + Cloud SQL (Postgres).

1. Cloud Run service `l402-aperture` (canonical URL: **https://l402.openagents.com**; see `docs/lightning/runbooks/L402_APERTURE_DEPLOY_RUNBOOK.md`):
   - min instances >= 2 for prod
   - max instances tuned by expected challenge throughput
   - private ingress + HTTPS LB fronting
2. Cloud SQL Postgres:
   - primary + backup policy
   - connection via Cloud SQL connector
3. Secret Manager for:
   - `invoice.macaroon`
   - TLS cert
   - DB credentials
   - any dynamic pricer auth secrets

### 6.3 Config strategy

Aperture config is declarative and ordered. We should treat it as compiled output.

1. Control plane stores canonical paywall policies in Convex.
2. `apps/lightning-ops` compiles that state to deterministic `aperture.yaml`.
3. Compiler emits:
   - config hash
   - ordered rule list
   - validation diagnostics
4. Rollout updates Cloud Run revision with new config artifact.
5. Rollback target is previous config hash + image digest.

### 6.4 Security constraints and mitigations

Known caveats in upstream behavior require explicit controls:

1. Upstream proxy transport TLS verification behavior should be isolated by network policy; prefer private upstream networks/VPC.
2. Disable unused Aperture features unless explicitly required.
3. Avoid relying only on Aperture’s in-memory freebie/rate limits for multi-tenant policy; enforce quotas in control plane too.
4. Keep route matching strict and deterministic; first-match ordering must be compiler-owned and test-covered.

### 6.5 Optional dynamic pricing service (Go on GCP)

If/when dynamic pricing is needed:

1. Implement pricer gRPC service in Go.
2. Run on Cloud Run (internal only), reachable by Aperture over private network.
3. Keep pricer stateless; pricing inputs come from control-plane-managed policy documents.
4. Enforce upper/lower price bounds in both pricer and control-plane validation.

## 7) Data Model and API Expansion in `apps/web`

### 7.1 Convex schema additions (proposed)

Add tables with explicit ownership and replay anchors:

1. `l402Paywalls`
   - `paywallId`, `ownerId`, `status`, `createdAtMs`, `updatedAtMs`
   - `name`, `description`
2. `l402PaywallPolicies`
   - `paywallId`, `pricingMode`, `fixedAmountMsats`
   - `maxPerRequestMsats`, `allowedHosts`, `blockedHosts`
   - `quotaPerMinute`, `quotaPerDay`, `killSwitch`
3. `l402PaywallRoutes`
   - `paywallId`, `hostPattern`, `pathPattern`
   - `upstreamUrl`, `protocol`, `timeoutMs`, `priority`
4. `l402GatewayDeployments`
   - `deploymentId`, `configHash`, `imageDigest`, `status`
   - `appliedAtMs`, `rolledBackFrom`, `diagnostics`
5. `l402GatewayEvents`
   - request-level gateway event records with `oa_req` and paywall references
6. `l402Invoices`
   - invoice metadata + paywall linkage + state (`open/settled/canceled/expired`)
7. `l402Settlements`
   - settlement records including payment proof references per ADR-0013
8. `l402Payouts`
   - payout instruction and status lifecycle (even if manual in MVP)

Add indexes by:

1. `paywallId`
2. `ownerId`
3. `(status, updatedAtMs)`
4. `requestId`
5. `(configHash, appliedAtMs)`

### 7.2 Worker/API surface

Implement in `apps/web/src/effuse-host/worker.ts` + routed handlers:

1. `POST /api/lightning/paywalls`
2. `PATCH /api/lightning/paywalls/:id`
3. `POST /api/lightning/paywalls/:id/pause`
4. `POST /api/lightning/paywalls/:id/resume`
5. `GET /api/lightning/paywalls/:id`
6. `GET /api/lightning/paywalls`
7. `GET /api/lightning/paywalls/:id/settlements`
8. `GET /api/lightning/paywalls/:id/deployments`

Rules:

1. subject auth required on all mutation routes.
2. explicit ownership checks before state transitions.
3. deterministic error codes for policy denials and invalid transitions.

### 7.3 Reuse of existing task/event model

Extend (do not replace) existing patterns in `apps/web/convex/lightning/tasks.ts`:

1. Keep status transition discipline for action workflows.
2. Emit typed events for paywall lifecycle transitions similar to current L402 fetch transitions.
3. Use same actor semantics where possible (`web_worker`, `system`, etc.).

## 8) New Service: `apps/lightning-ops` (TypeScript + Effect)

Introduce a dedicated service for long-running operational jobs that are not a fit for request-bound Worker execution.

### 8.1 Why this service is needed

1. Aperture config compilation/deployment is asynchronous and should be audited.
2. LND settlement subscriptions are long-lived streams.
3. Replayable operational events require stable process-level orchestration.

### 8.2 Responsibilities

1. Poll/control paywall desired state from Convex.
2. Compile Aperture config deterministically.
3. Apply and verify gateway deployments.
4. Subscribe to invoice updates and write settlement records.
5. Emit structured logs with request/deployment IDs.

### 8.3 Suggested structure

1. `apps/lightning-ops/src/config/*`
2. `apps/lightning-ops/src/aperture/*`
3. `apps/lightning-ops/src/settlement/*`
4. `apps/lightning-ops/src/telemetry/*`
5. `apps/lightning-ops/src/main.ts`

All modules implemented with Effect services/layers and schema-typed payloads.

## 9) `packages/lightning-effect` Expansion (Seller Side)

Keep package reusable for OpenAgents and external consumers.

### 9.1 New contracts

1. `PaywallDefinition`
2. `PaywallPolicy`
3. `RouteBinding`
4. `ChallengeIssueRequest/Result`
5. `AuthorizationVerificationResult`
6. `SettlementRecord`
7. `PayoutInstruction`
8. `GatewayDeploymentSnapshot`

### 9.2 New services

1. `PaywallRegistryService`
2. `GatewayConfigCompilerService`
3. `InvoiceIssuerService`
4. `SettlementIngestService`
5. `SellerPolicyService`

### 9.3 Adapter/layer additions

1. `ApertureConfigCompilerLayer`
2. `InvoiceIssuerLndLayer`
3. `SettlementIngestLndLayer`
4. deterministic in-memory test layers for all seller interfaces

### 9.4 Package consumer guarantees

1. No dependency on `apps/*`.
2. Explicit subpath exports for seller contracts/services.
3. Versioned schema evolution notes in README.

## 10) Autopilot Tool Contract Expansion

Add tools in `apps/autopilot-worker/src/tools.ts`:

1. `lightning_paywall_create`
2. `lightning_paywall_update`
3. `lightning_paywall_pause`
4. `lightning_paywall_resume`
5. `lightning_paywall_get`
6. `lightning_paywall_list`
7. `lightning_paywall_settlement_list`

Execution requirements:

1. Schema validate input/output.
2. Deterministic status/error mapping.
3. Receipt emission with `params_hash`, `output_hash`, `latency_ms`, side-effect markers.
4. Counterfactual-ready logging where tool decisions replace legacy behavior.

## 11) Web UX and Pane Integration (`apps/web`)

We already have L402 wallet + transactions panes. Extend them rather than creating a second pane framework.

### 11.1 Existing pane anchors

File: `apps/web/src/effuse-app/controllers/home/openChatPaneController.ts`

Already includes:

1. wallet summary pane (`L402 Wallet Summary`)
2. transactions pane (`Recent L402 Attempts`)
3. payment detail pane

### 11.2 Seller additions

Add panes for:

1. `L402 Paywalls`
   - paywall name/status/price/route
   - pause/resume controls
2. `L402 Settlements`
   - recent settled invoices
   - proof reference and amount
3. `L402 Deployments`
   - current gateway config hash
   - last deploy status and rollback action

Data should come from new Worker/Convex read APIs with request correlation IDs.

## 12) End-to-End Flows

### 12.1 Seller onboarding flow

1. Agent/user calls `lightning_paywall_create`.
2. Control plane validates ownership and policy bounds.
3. Convex writes paywall + route + policy (initially paused or active per policy).
4. `apps/lightning-ops` compiles and deploys new Aperture config.
5. Deployment event stored with config hash.
6. UI reflects active paywall endpoint and health status.

### 12.2 Buyer request flow

1. Buyer requests paywalled endpoint.
2. Aperture emits `402` challenge with invoice + macaroon.
3. Buyer pays (via lnget or other L402-capable client) and retries with auth header.
4. Aperture verifies proof and proxies upstream request.
5. Gateway + settlement events are written and linked to paywall.
6. Seller pane shows new settlement/revenue event.

### 12.3 Failure flow

1. Voltage/LND unavailable -> challenge issuance fails.
2. Gateway returns deterministic failure response.
3. Control plane marks paywall health degraded.
4. Alert fires, operator runbook initiated.
5. Optional global pause toggle disables all paywalls until recovery.

## 13) Verification and Testing Matrix

### 13.1 Unit and contract tests

1. `packages/lightning-effect`
   - schema/contract tests for new seller types
   - deterministic compile tests for rule ordering
   - challenge/authorization edge cases
2. `packages/lnd-effect`
   - contract and adapter tests for LND RPC integration used by local executor path
   - typed error mapping and deterministic transport behavior
3. `apps/web`
   - Convex mutation/query tests for paywall lifecycle
   - API authorization and ownership tests
4. `apps/autopilot-worker`
   - tool schema tests
   - deterministic tool output tests

### 13.2 Integration tests

1. Staging Aperture + staging Voltage node + staging control plane.
2. Scenario matrix:
   - successful paid request
   - invalid auth retry
   - paused paywall
   - over-cap policy deny
   - stale/expired invoice
3. Local-node parity checks:
   - local desktop executor success path remains green
   - local budget/policy deny path remains deterministic
   - artifact schema parity for correlation IDs across `local-node` and `hosted-node`

### 13.3 E2E smoke tests

1. `apps/web` hosted prod-style smoke using existing E2E harness patterns.
2. `apps/desktop` local-node smoke run (non-interactive JSON artifact output).
3. Include request correlation assertions (`x-oa-request-id`, `oa_req=<id>` style linkage where relevant).
4. Ensure artifacts saved for replay/audit.

### 13.4 Mandatory local validation before merge

1. `cd packages/lnd-effect && npm run typecheck && npm test`
2. `cd packages/lightning-effect && npm run typecheck && npm test`
3. `cd apps/desktop && npm run typecheck && npm test`
4. `cd apps/desktop && npm run test:l402-local-node-smoke -- --json`
5. `cd apps/autopilot-worker && npm run typecheck && npm test`
6. `cd apps/web && npm run lint && npm test`

Use app deploy scripts for production deploy operations (avoid raw `npx convex deploy` path).

## 14) Operations, Security, and Runbooks

### 14.1 Secrets and key material

1. Secrets in Secret Manager only.
2. No raw preimages in logs.
3. Scoped macaroons per workload.
4. Rotation runbook with tested rollback.

### 14.2 Observability

Track at minimum:

1. challenge issuance rate
2. paid conversion rate
3. failed auth rate
4. settlement lag
5. payout queue depth
6. per-paywall revenue and error rate

### 14.3 Incident classes

1. LND unreachable
2. Aperture config rollout failure
3. Settlement mismatch
4. abusive route/policy misconfiguration
5. DB or config corruption

Each class needs:

1. detection signal
2. immediate mitigation steps
3. rollback path
4. post-incident audit fields

## 15) Ordered Implementation Steps (Logical Execution Plan)

This is the recommended sequence for full implementation.

1. Confirm ADR updates for hosted paywall topology + authority boundaries.
2. Add Convex schema and Worker API stubs for paywall resources.
3. Create `apps/lightning-ops` skeleton (Effect runtime, config, telemetry).
4. Implement deterministic Aperture config compiler in `packages/lightning-effect`.
5. Wire `apps/lightning-ops` to read paywall state and produce config artifacts.
6. Stand up GCP staging infra (Cloud Run + Cloud SQL + Secret Manager + Artifact Registry).
7. Provision staging Voltage node and scoped macaroons.
8. Deploy Aperture staging with compiled static config.
9. Add deploy/reconcile loop from `apps/lightning-ops` to staging Aperture.
10. Add settlement ingest pipeline from Voltage to Convex.
11. Expand Autopilot tool contracts for paywall CRUD/status.
12. Add web seller panes and status views in existing pane system.
13. Run full staging E2E challenge/pay/settle tests.
14. Add production runbooks and alerting.
15. Canary deploy production with a single internal paywall.
16. Expand to selected user-facing paywalls.

## 16) Milestone Gates and Exit Criteria

### M0: Infrastructure ready

1. GCP baseline services deployed.
2. Voltage staging node live with scoped macaroons.
3. Aperture reachable and healthy.

### M1: Single paywall end-to-end

1. Can create one paywall in control plane.
2. Buyer receives `402`, pays, retries, gets resource.
3. Settlement and proof reference recorded.

### M2: Agent self-serve beta

1. Paywall CRUD tools available in Autopilot.
2. Ownership and policy checks enforced.
3. UI panes expose paywall + settlement state.

### M3: Multi-tenant production readiness

1. deterministic deployment/rollback in place
2. SLO-backed telemetry and alerts active
3. incident runbooks tested
4. payout accounting path validated

## 17) Risks and Mitigations

1. **Rule ordering mistakes in Aperture config**
   - Mitigation: deterministic compiler + golden tests + config hash rollback.
2. **Voltage availability or API regressions**
   - Mitigation: environment isolation, health checks, degraded mode + global pause.
3. **Policy abuse or unsafe autonomous configuration**
   - Mitigation: strict defaults, caps, explicit allowlists, owner auth on all mutations.
4. **Settlement accounting drift**
   - Mitigation: reconciliation jobs + anomaly alerts + immutable settlement events.
5. **Operational complexity growth**
   - Mitigation: separate ops service (`apps/lightning-ops`), explicit runbooks, staged rollout.

## 18) Recommendation

Proceed with an Aperture-first gateway on Google Cloud, backed by Voltage-hosted LND, while keeping all OpenAgents policy/orchestration code in TypeScript + Effect and rooted in existing `apps/web` + Convex models.

This path delivers a practical production rollout for agent-owned paywalls without blocking on full node sovereignty, while preserving a clean migration path to later remote-signer/self-hosted node topologies.
