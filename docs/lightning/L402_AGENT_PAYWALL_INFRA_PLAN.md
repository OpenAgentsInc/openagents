# OpenAgents-Hosted L402 Infrastructure for Agent-Owned Paywalls

Date: 2026-02-11
Status: Draft design note

## Why this note

Goal: describe a realistic path for OpenAgents to run its own L402 infrastructure so agents can create and operate their own paid API paywalls, while preserving current repo constraints (Worker control plane, typed tool contracts, deterministic receipts).

This note is architecture and sequencing guidance, not an implementation commitment.

## Inputs reviewed

- `docs/lightning/LIGHTNING_AGENT_TOOLS.md`
- `docs/lightning/SETUP_LOG.md`
- `docs/lightning/LIGHTNING_DEEP_INTEGRATION_ROADMAP.md`
- `docs/lightning/LIGHTNING_LABS_VS_BREEZ_SPARK_COMPARISON.md`
- `docs/lightning/212-demo-plan.md`
- `packages/lightning-effect/README.md`
- `packages/lightning-effect/src/contracts/*`
- `packages/lightning-effect/src/services/*`
- `packages/lightning-effect/src/l402/*`
- `packages/lightning-effect/src/layers/*`
- `packages/lightning-effect/src/adapters/*`
- `packages/lightning-effect/src/errors/*`
- `packages/lightning-effect/test/*`
- `docs/GLOSSARY.md`
- `docs/adr/ADR-0007-tool-execution-contract.md`
- `docs/adr/ADR-0013-receipt-schema-payment-proofs.md`

## Current baseline (important)

What already exists:

1. Canonical terminology and receipt proof semantics for Lightning/L402 (`lightning_preimage`).
2. `packages/lightning-effect` includes buyer-side primitives:
   - L402 challenge parsing and auth header construction.
   - spend policy checks (allowlist, blocklist, max spend).
   - credential cache abstraction.
   - invoice payer abstraction and demo adapter.
   - live L402 authorization orchestration layer.
3. Clear runtime contract rules: tools need schemas, deterministic failures, replay/receipt emission.

What does not exist yet:

1. Multi-tenant seller/paywall control plane.
2. OpenAgents-hosted L402 gateway plane for agent-owned endpoints.
3. Invoice issuance + reconciliation services wired into OpenAgents data models for seller flows.
4. Agent tools for creating/managing paywalls.

## Target outcome

Any agent can register a paywall definition and receive a stable OpenAgents paywalled URL (or route binding), where:

1. Buyers get `402 Payment Required` with a valid L402 challenge.
2. Buyers can pay and retry using standard L402 semantics.
3. Access is granted only after valid proof.
4. Seller and buyer activity is recorded with typed receipts and deterministic traces.
5. Policy controls (price caps, allowlists, kill switches) are enforced before side effects.

## Proposed architecture

### 1) Control Plane (OpenAgents Worker + Convex + Autopilot)

Responsibilities:

1. Paywall lifecycle APIs (create, update, pause, delete, list).
2. Policy definitions (pricing, domain/path scope, quotas, spending caps).
3. Ownership and auth mapping (user/agent -> paywall resources).
4. Audit events and receipt references.

Suggested API surface:

- `POST /api/lightning/paywalls`
- `PATCH /api/lightning/paywalls/:id`
- `POST /api/lightning/paywalls/:id/pause`
- `POST /api/lightning/paywalls/:id/resume`
- `GET /api/lightning/paywalls/:id`
- `GET /api/lightning/paywalls`

Convex entities to add (illustrative):

- `l402Paywalls`
- `l402PaywallPolicies`
- `l402PaywallRoutes`
- `l402GatewayEvents`
- `l402Settlements`
- `l402Payouts`

### 2) Gateway Plane (L402 edge, OpenAgents-operated)

Responsibilities:

1. Intercept protected route traffic.
2. Emit L402 challenges (`WWW-Authenticate`) when authorization is missing.
3. Validate `Authorization: L402 ...` on retry.
4. Proxy authorized requests to upstream targets.
5. Emit gateway events with request correlation IDs and paywall IDs.

Implementation options:

1. Aperture-first: run Aperture ourselves and treat it as the primary gateway.
2. Hybrid: Aperture for challenge/payment checks, OpenAgents service for tenancy/policy orchestration.
3. Custom gateway later only if Aperture limits become material.

Recommendation: option 2 is the best launch path.

### 3) Settlement Plane (Lightning execution + key isolation)

Responsibilities:

1. Invoice creation and monitoring.
2. Payment settlement confirmation and proof handling.
3. Macaroon minting/rotation strategy by role.
4. Revenue attribution per paywall and owner.

Security baseline:

1. Watch-only execution host + remote signer default for production.
2. Scoped macaroons:
   - gateway: invoice-only
   - buyer agents: pay-only
   - monitoring: read-only
3. Explicit rotation and revocation runbooks.

### 4) Product Plane (agent UX and tooling)

Responsibilities:

1. Agent-facing tools to create and control paywalls.
2. Chat-visible states for paywall events and settlements.
3. Admin/operator views for failures, denials, spend spikes, and payout state.

## Aperture deployment deep dive (from upstream code)

This section is based on direct review of `/Users/christopherdavid/code/aperture`.

### What Aperture is operationally

1. A single Go binary (`cmd/aperture/main.go`) that runs an HTTP reverse proxy and L402 auth flow.
2. No first-party Helm chart, Kubernetes manifests, docker-compose, or systemd units in repo.
3. Config is file-driven (`aperture.yaml`) with CLI override precedence.

Implication: we will own deployment packaging and day-2 ops.

### Build and packaging realities

1. Upstream `go.mod` requires modern Go (`go 1.24.9`, toolchain `go1.24.11`).
2. Upstream Dockerfile builds from `golang:1.24.9-alpine`, clones the Aperture repo, and checks out a branch/tag/commit (`ARG checkout`).
3. There is a `replace` directive in `go.mod`, so pinned source builds are safer than ad hoc `go install ...@latest` flows.

Suggested OpenAgents packaging approach:

1. Build from a pinned commit SHA in CI.
2. Publish our own immutable container image.
3. Avoid “clone during docker build” in production pipeline; copy vendored source or pinned artifact instead.

### Runtime config and required dependencies

Minimum required to run:

1. `listenaddr` must be set.
2. Auth backend must be valid unless explicitly disabled.
3. Service list must be configured in ordered match priority (`hostregexp` + optional `pathregexp`).
4. Database backend must be set deliberately.

Important upstream behaviors:

1. `auth` defaults to effectively “on” when not explicitly set.
2. Service `price: 0` becomes default `1 sat`.
3. Service matching is first-match wins; config order is a real routing policy.
4. Dynamic pricing requires an external gRPC pricer service; Aperture only provides the client and protobufs.
5. Code default backend is `etcd` (sample config comments mention sqlite), so we should always set `dbbackend` explicitly.

### TLS and edge topology options

Aperture supports three practical modes:

1. Native TLS with cert/key files.
2. Native ACME/autocert (`autocert: true`) with HTTP challenge listener on port 80.
3. `insecure: true` mode (h2c/no TLS) behind an external TLS terminator.

Note: README text and runtime code differ on ACME support; runtime code includes `autocert` support, and code behavior should be treated as canonical.

Recommended for OpenAgents:

1. Terminate TLS at ingress/LB.
2. Run Aperture on private network in `insecure: true` mode only if network boundary is trusted.
3. Keep gRPC/HTTP2 requirements in mind when choosing ingress.

### Data backend choices and scale implications

Aperture supports `sqlite`, `postgres`, `etcd`, with meaningful differences:

1. `sqlite`: simplest, local file, not a good horizontal scale backend.
2. `postgres`: best candidate for multi-instance production.
3. `etcd`: supports secret/onion storage path, but LNC mode is explicitly blocked with etcd in code.

Recommendation:

1. Dev/staging: sqlite acceptable.
2. Production multi-tenant: postgres.
3. Avoid etcd for initial OpenAgents seller rollout unless we need Tor/onion-specific behaviors there.

### LND/LNC connectivity choices

Aperture can challenge via:

1. Direct lnd connection (`lndhost`, `tlspath`, `macdir`).
2. LNC passphrase/mailbox flow.

### Planned provider: Voltage-hosted LND via API

OpenAgents should treat Voltage as the default seller-side LND host for initial rollout.

Key operational facts to reflect in our deployment:

1. Voltage exposes full LND API surface (gRPC + REST) per node.
2. Base URL is node-specific and comes from the Voltage node dashboard.
3. API ports are:
   - gRPC: `10009`
   - REST: `8080`
4. API auth uses macaroon material from the node dashboard (admin macaroon download path in Voltage UI).

How this maps to Aperture runtime:

1. Use Aperture direct-lnd mode, set:
   - `authenticator.lndhost` to Voltage endpoint + gRPC port.
   - `authenticator.tlspath` to the local TLS cert file for that node.
   - `authenticator.macdir` to local directory containing macaroon files.
2. Because Aperture’s lnd client path expects `invoice.macaroon` by default, create a local macaroon layout that satisfies that file expectation.
3. If Voltage only provides admin macaroon initially, treat that as bootstrap only; move to least-privilege invoice-only macaroon as soon as node policy permits.

Notable details:

1. Direct mode uses `invoice.macaroon` by default.
2. `strictverify: true` adds invoice-state loading/subscription overhead at startup.
3. `invoicebatchsize` matters at scale with large invoice history.

Recommendation:

1. Start with direct lnd against Voltage.
2. Use least-privilege invoice macaroon (or short-lived admin fallback during bootstrap only).
3. Add LNC mode later only if we need that operational pattern.

### Rate limits, freebies, and multi-instance behavior

Upstream behavior is mostly in-memory:

1. Rate limiting uses per-instance token buckets.
2. Freebie counters are in-memory per instance.
3. Unauthenticated client keying is IP-mask based (`/24` IPv4, `/48` IPv6).
4. Authenticated keying uses L402 token ID.

Operational implication:

1. Horizontal replicas do not share freebie/rate-limit state.
2. IP-based behavior is less reliable behind proxies/CDNs.
3. For OpenAgents paywalls, rely primarily on authenticated token-based limits and explicit policy controls.

### Security caveats to account for in design

From current upstream code paths:

1. Backend proxy transport sets `InsecureSkipVerify: true` for upstream TLS connections.
2. Hashmail stream auth contains a TODO path that currently returns success.
3. Blocklist is static exact-IP list, not CIDR policy engine.
4. No built-in admin API for config mutation; dynamic service updates require in-process calls (`UpdateServices`) or process-level config rollout.

OpenAgents mitigation stance:

1. Keep Aperture and upstream services on private network boundaries.
2. Disable hashmail unless explicitly needed.
3. Use control-plane policy enforcement before Aperture routing.
4. Treat Aperture config rollout as code/declarative state with audited change history.

## Suggested hosting models for OpenAgents

### Model 1: Single-node managed VM (fastest)

1. One Aperture instance + sqlite + direct lnd to a single Voltage node.
2. Best for internal dogfood and early staging.
3. Lowest complexity, lowest resilience.

### Model 2: Production baseline (recommended initial prod)

1. 2+ Aperture instances behind L7 load balancer.
2. Shared postgres.
3. Direct lnd connectivity from each Aperture instance to the same Voltage node API endpoint.
4. External TLS termination at ingress; internal h2c or private TLS.

### Model 3: Hardened infra variant

1. Same as Model 2.
2. Keep Voltage for early operations, then graduate to dedicated lnd/watch-only + remote-signer topology when sovereignty requirements exceed hosted-node constraints.
3. Tighter network segmentation and secret-manager-backed credential rotation.

## What deployment entails for OpenAgents (concrete work)

1. Build pipeline:
   - pin Aperture commit.
   - produce signed image artifact.
2. Config compiler:
   - derive ordered `services` list from paywall control-plane state.
   - render deterministic config artifacts.
3. Rollout mechanism:
   - restart-based config reload initially, or
   - in-process controller/fork that calls `UpdateServices`.
4. Secret and credential handling:
   - Voltage node base URL and API port mapping (`10009` gRPC, `8080` REST).
   - lnd cert + macaroon material downloaded from Voltage and stored in expected local filesystem layout for Aperture.
   - invoice-only macaroon target; admin macaroon only as controlled bootstrap path.
   - DB credentials.
   - TLS cert assets (if not edge-terminated).
5. Observability:
   - Prometheus scrape endpoint.
   - request correlation across ingress -> Aperture -> upstream.
   - alerting on 402 spikes, 5xx rate, challenge issuance failures.
6. Reliability runbooks:
   - DB outage behavior.
   - Voltage API endpoint connectivity loss.
   - macaroon rotation/expiry and stale credential recovery.
   - invoice verification lag (strict verify mode).
   - emergency pause/kill switch for all paywalls.
7. Security hardening:
   - private network for Aperture/upstreams.
   - disable unused features (`hashmail`, Tor) by default.
   - regular rotation and audit of macaroon/cert material.

## Mapping Aperture service config to agent-created paywalls

For each agent paywall, OpenAgents control plane should emit one service rule with:

1. `name`: stable paywall ID (`paywall_<id>`).
2. `hostregexp` and `pathregexp`: precise route scope.
3. `address` + `protocol`: upstream target.
4. `auth`: `on` by default.
5. `price` or `dynamicprice`.
6. `timeout` and optional caveat constraints.
7. optional `ratelimits` and auth whitelist/skip patterns.

This makes Aperture the enforcement edge while OpenAgents remains the policy authority.

## Proposed `lightning-effect` evolution for seller infrastructure

Current package is already a good base. For agent-owned paywalls, add seller-side contracts/services without breaking buyer-side interfaces.

### Contracts to add

1. `PaywallDefinition`
2. `PaywallPricingPolicy` (fixed msats first, dynamic pricing later)
3. `PaywallRouteBinding`
4. `L402ChallengeIssueRequest` / `L402ChallengeIssueResult`
5. `L402AuthorizationVerificationResult`
6. `SettlementRecord`
7. `PayoutInstruction`

### Services to add

1. `InvoiceCreatorService`
2. `ChallengeIssuerService`
3. `CredentialVerifierService`
4. `PaywallRegistryService`
5. `SettlementRecorderService`
6. `PayoutService` (optional in MVP if payouts are manual)

### Layers/adapters to add

1. `ApertureGatewayAdapterLayer` (seller path)
2. `InvoiceCreatorLndLayer`
3. `SettlementListenerLayer`
4. deterministic demo/test adapters for all seller-side services

## Agent tool contracts for paywall creation

Add schema-first tools in `apps/autopilot-worker/src/tools.ts`:

1. `lightning_paywall_create`
2. `lightning_paywall_update`
3. `lightning_paywall_pause`
4. `lightning_paywall_resume`
5. `lightning_paywall_get`
6. `lightning_paywall_list`

Each tool must:

1. Validate against JSON schema before execution.
2. Emit deterministic tool receipts (`params_hash`, `output_hash`, `latency_ms`, side effects).
3. Emit explicit deny/failure reasons for policy blocks.

## End-to-end flow (agent creates paywall)

1. Agent calls `lightning_paywall_create` with route, upstream target, and pricing policy.
2. Control plane validates owner permissions and policy constraints.
3. Gateway route binding is created/updated.
4. Agent receives paywalled URL and policy snapshot.
5. Buyer calls URL and receives L402 challenge.
6. Buyer pays and retries with L402 auth header.
7. Gateway verifies auth and proxies request.
8. Settlement is recorded with receipt proof references.
9. Seller agent sees revenue event and activity log entry.

## Data and receipt mapping

For each successful paid request:

1. Record payment metadata: `rail`, `asset_id`, `amount_msats`.
2. Record proof as typed payment proof (`lightning_preimage`) per ADR-0013.
3. Link to request/thread/run IDs for replayability.

For denied/blocked requests:

1. Record explicit reason (`over_cap`, `domain_blocked`, `paywall_paused`, `invalid_auth`, `expired_invoice`).
2. Record whether a payment side effect occurred (`false` for preflight policy denials).

## Launch sequencing

### Phase A: Internal seller MVP (2-3 weeks)

1. Stand up OpenAgents-operated gateway (Aperture-first).
2. Add one internal paywalled route.
3. Confirm full challenge/pay/verify loop and settlement logging.

Exit: one stable paid endpoint with auditable receipts.

### Phase B: Agent self-serve paywalls (2-4 weeks)

1. Add paywall CRUD APIs + Convex models.
2. Add paywall tool contracts in Autopilot worker.
3. Add policy validation and kill switch controls.

Exit: selected agents can create/pause/resume paywalls from tools.

### Phase C: Harden for multi-tenant operations (3-5 weeks)

1. Add quotas, abuse controls, and route isolation guarantees.
2. Add settlement reconciliation jobs and payout workflow.
3. Add alerting for anomalies and gateway failures.

Exit: multi-tenant reliability and incident runbooks in place.

## Operational requirements before broad rollout

1. Source-pinned build/deploy path for `lnget`/Aperture/lnd-related tooling (avoid fragile `go install ...@latest` assumptions noted in setup logs).
2. Structured metrics:
   - challenge rate
   - payment success/failure
   - median paid amount
   - policy denial rate
   - per-paywall revenue
3. Structured logs with request IDs and paywall IDs.
4. Secret handling:
   - no raw preimages in plaintext logs
   - scoped macaroons only
   - rotation cadence and break-glass procedure

## Key risks

1. Multi-tenant misconfiguration could leak route access.
2. Settlement mismatch could break seller trust and accounting.
3. Overly permissive autonomous configuration could create abusive or unsafe paywalls.
4. Gateway/runtime split could produce observability gaps without strict request correlation.

## Mitigations

1. Default-deny routing and explicit ownership checks for every paywall mutation.
2. Contract tests and replay fixtures for challenge/pay/verify and denial paths.
3. Strong policy defaults (caps, allowlists, paused-by-default for new paywalls).
4. Correlated telemetry across Worker, gateway, and settlement systems.

## Recommendation

Launch with an OpenAgents-operated Aperture-first gateway plus a strict control plane in Worker/Convex, then progressively move more seller logic into `packages/lightning-effect` as reusable interfaces. This gets agent-created paywalls live quickly without violating current runtime boundaries, and keeps a clear path to fully sovereign infrastructure later.
