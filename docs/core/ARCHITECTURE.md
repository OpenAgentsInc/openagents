# OpenAgents Architecture

Status: Canonical Rust-era architecture
Last updated: 2026-02-25
Canonical architecture file: `docs/core/ARCHITECTURE.md`

## Purpose

This is the single architecture authority for OpenAgents.

It must describe both:

1. What is implemented now in `main`.
2. What is planned next in active plan docs.

If this document conflicts with code, code wins and this file must be corrected.

## Authority Stack

Primary authorities for this document:

1. `docs/adr/ADR-0001-rust-only-architecture-baseline.md`
2. `docs/adr/ADR-0002-proto-first-contract-governance.md`
3. `docs/adr/ADR-0003-khala-ws-only-replay-transport.md`
4. `docs/adr/ADR-0004-rivet-harvest-posture-and-adoption-boundaries.md`
5. `docs/adr/ADR-0005-compatibility-negotiation-and-support-window-policy.md`
6. `docs/adr/ADR-0006-wallet-executor-auth-custody-receipts.md`
7. `docs/adr/ADR-0008-bounded-vercel-sse-compatibility-lane.md`
8. `docs/plans/rust-migration-invariant-gates.md`

Active plan set reflected in this architecture:

1. `docs/plans/2026-02-23-open-agent-economy-execution-plan.md`
2. `docs/plans/hydra-liquidity-engine.md`
3. `docs/plans/aegis.md`
4. `docs/plans/ep212-autopilot-bitcoin-100pct.md`
5. `docs/plans/rust-migration-execution-control-plane.md`
6. `docs/plans/vignette-phase0-issue-to-pr.md`
7. `docs/plans/vignette-hydra-mvp2.md`
8. `docs/plans/vignette-hydra-mvp3.md`

Research/optional context (not execution authority):

1. `docs/plans/research/gitafter-bonus.md`
2. `docs/plans/research/simple-agi-economics-research.md`

## Non-Negotiable Invariants

1. Proto-first wire contracts (`INV-01`).
2. Authority mutations over authenticated HTTP only (`INV-02`).
3. Khala live transport is WS-only (`INV-03`); SSE is adapter-only per ADR-0008.
4. Control-plane and runtime authority isolation (`INV-04`, `INV-05`, `INV-06`).
5. Replay/idempotency by `(topic, seq)` (`INV-07`).
6. Service deploy isolation (`INV-08`) and migration discipline (`INV-09`).
7. Legacy removal ordering after parity evidence (`INV-10`).
8. No `.github/workflows` automation in-repo (`INV-12`).

## State Model: Implemented vs Target

### Implemented now (`main`)

1. Active app roots:
   - `apps/openagents.com/service/`
   - `apps/runtime/`
   - `apps/autopilot-desktop/`
   - `apps/lightning-ops/`
   - `apps/lightning-wallet-executor/`
2. Desktop is the primary execution surface with Codex, runtime sync/auth, NIP-90 lanes, and local Spark wallet actions.
3. Runtime internal authority APIs are active under `/internal/v1/*` (runs, workers, marketplace, treasury, verifications, liquidity/credit/Hydra, pools, fraud).
4. Control service owns auth/session/sync token issuance and also serves substantial web/API product and compatibility surfaces.
5. Hydra internal authority and FX MVP-3 lanes are implemented and covered by harnesses.
6. Aegis is mostly a planned architecture lane; no dedicated `/internal/v1/aegis/*` namespace is implemented yet.

### Target direction (active plans)

1. Liquidity-first open-agent economy with Autopilot as a guaranteed compute buyer (`2026-02-23-open-agent-economy-execution-plan.md`).
2. Two-sided marketplace loop: user can consume compute and provide compute (NIP-90 provider mode) from Autopilot surfaces.
3. Hardened Hydra economics with continued API/productization and operations maturity.
4. Aegis verification/underwriting/liability subsystem phased in behind runtime + treasury + receipts.
5. EP212 parity closure for wallet + L402 + commerce flows as production-real, not placeholder/stub behavior.

## End-to-End Topology (Current + Planned)

```mermaid
flowchart TB
  subgraph surfaces[Client Surfaces]
    web[openagents.com control + web UI lanes]
    desktop[autopilot-desktop Rust/WGPUI]
  end

  subgraph controlplane[Control Plane]
    control[openagents.com/service
Auth/session/org/device authority
Sync token issuance + control APIs]
  end

  subgraph runtimeplane[Runtime and Economy Plane]
    runtime[apps/runtime
Execution authority + marketplace + treasury]
    khala[Khala WS replay/live delivery]
    hydra[Hydra liquidity/credit/routing/FX]
    aegis[Aegis verification/underwriting (planned lane)]
  end

  subgraph settlement[Settlement and Ops]
    lwe[lightning-wallet-executor
Signing/custody/receipts]
    lops[lightning-ops
L402/paywall ops + reconcile]
    lnd[LND]
    bitcoind[bitcoind]
  end

  subgraph fabrics[Coordination and Interop]
    nexus[Nexus intra-domain fabric]
    bridge[Bridge/Gateway policy boundary]
    nostr[Nostr interop substrate]
  end

  web --> control
  desktop --> control

  control --> runtime
  control --> khala
  runtime --> khala

  runtime --> hydra
  runtime --> aegis
  hydra --> lwe
  lops --> control
  lwe --> lnd
  lnd --> bitcoind

  runtime --> nexus
  control --> nexus
  bridge --> nexus
  bridge --> nostr
```

## Service Boundaries (Implemented)

### `apps/openagents.com/service` (control-plane authority + retained web/API lanes)

Owns:

1. WorkOS-authenticated identity/session binding and revocation.
2. Org/device/session authorization state.
3. Sync token + Khala token issuance.
4. Control API entry point and route orchestration.
5. Desktop download redirect authority via `OA_DESKTOP_DOWNLOAD_URL`.

Current exposed API/web groups (implemented):

1. Distribution and service health:
   - `GET /`
   - `GET /download-desktop`
   - `GET /healthz`
   - `GET /readyz`
   - `GET /openapi.json`
2. Auth and identity:
   - `/api/auth/*`, `/api/v1/auth/*`, `/api/me`
3. Autopilot/settings/inbox/token/org lanes:
   - `/api/autopilots*`
   - `/api/settings/*` and `/settings/integrations/*`
   - `/api/inbox/*`
   - `/api/tokens*`
   - `/api/orgs/*`, `/api/policy/authorize`
4. Runtime sync + worker control lanes:
   - `/api/sync/token`, `/api/v1/sync/token`, `/api/khala/token`
   - `/api/runtime/codex/workers*`
   - `/api/runtime/threads*`
   - `/api/runtime/workers*`
   - `/api/runtime/tools/execute`
   - `/api/runtime/skills/*`
5. Payments and L402 lanes:
   - `/api/agent-payments/*`
   - `/api/payments/*`
   - `/api/l402/*`
6. Social/feed and web shell lanes still present in Rust service:
   - `/api/shouts`, `/api/whispers*`
   - HTMX/Maud pages and fragments (`/chat`, `/feed`, `/inbox`, `/compute`, `/l402`, `/admin`, etc.)
7. Compatibility/admin control lanes:
   - legacy chat aliases (`/api/chat/stream`, `/api/chats/*`)
   - route split and runtime routing controls (`/api/v1/control/*`)

Source of truth for control route inventory:

1. `apps/openagents.com/service/src/openapi.rs`
2. `apps/openagents.com/service/src/lib.rs`

Important reality:

1. Current web/control surface is broader than a pure landing-only site.
2. "Landing-only web" remains a target-state direction, not current-state fact.

### `apps/runtime` (execution authority + internal economy APIs)

Owns:

1. Runtime execution authority state (runs/events/projectors/workers).
2. Internal marketplace, verification, and treasury compute flows.
3. Hydra liquidity/credit/routing/FX authority lanes.
4. Khala delivery endpoints and replay/read models.

Current internal API groups (implemented under `/internal/v1/*`):

1. Health + OpenAPI:
   - `/healthz`, `/readyz`, `/internal/v1/openapi.json`
2. Execution and replay:
   - `/runs*`, `/khala/topics/*`, `/projectors/*`
3. Worker lifecycle:
   - `/workers*`
4. Marketplace and dispatch:
   - `/marketplace/catalog/*`
   - `/marketplace/route/provider`
   - `/marketplace/compute/quote/sandbox-run`
   - `/marketplace/router/compute/select`
   - `/marketplace/dispatch/sandbox-run`
5. Verification and treasury settlement:
   - `/verifications/sandbox-run`
   - `/verifications/repo-index`
   - `/treasury/compute/*`
6. Hydra + liquidity + credit:
   - `/hydra/routing/score`
   - `/hydra/fx/*`
   - `/hydra/risk/health`
   - `/hydra/observability`
   - `/liquidity/quote_pay`, `/liquidity/pay`, `/liquidity/status`
   - `/credit/intent`, `/credit/offer`, `/credit/envelope`, `/credit/settle`, `/credit/health`, `/credit/agents/:agent_id/exposure`
7. Pool and fraud ops:
   - `/pools/*`
   - `/fraud/incidents`

Source of truth for runtime route inventory:

1. `apps/runtime/src/server.rs`
2. `apps/runtime/docs/openapi-internal-v1.yaml`

Not implemented yet:

1. Dedicated `/internal/v1/aegis/*` namespace.

### `apps/autopilot-desktop` (primary execution/operator client)

Implemented responsibilities:

1. Runtime auth flows (`RuntimeAuthSendCode`, `RuntimeAuthVerifyCode`, status/logout).
2. Runtime Codex worker sync/control via control + Khala lanes.
3. Local identity lane via `UnifiedIdentity` (`load_or_init_identity`) persisted as local mnemonic.
4. Provider controls:
   - liquidity provider online/offline/refresh/invoice
   - DVM provider start/stop/refresh
5. NIP-90 compute consumption (`Nip90Submit`) via `nostr::nip90` + `nostr_client::dvm::DvmClient`.
6. Local Spark wallet operations via Rust Breez SDK bindings:
   - wallet refresh
   - create invoice
   - pay request
   - recent payment history

### `apps/lightning-wallet-executor` (custody/signing boundary)

Owns:

1. Wallet execution and signing APIs (`/wallets/*`, `/pay-bolt11`).
2. Canonical receipt emission aligned to `ADR-0006` + proto contracts.

### `apps/lightning-ops` (ops/reconcile and paywall operations)

Owns:

1. Operational reconciliation and paywall deployment/compile/smoke flows.
2. Internal control-plane mutation/query workflows for hosted L402 operations.

## Economy Architecture (Hydra + Aegis)

### Hydra (implemented and active)

Hydra is the active capital substrate and currently has implemented authority lanes in runtime.

Implemented now:

1. Liquidity, credit, routing/risk, and FX lanes under `/internal/v1/*`.
2. Deterministic/idempotent FX settlement path with withheld/failure handling.
3. Hydra observability surfaced into control `/stats`.
4. Harness gates:
   - `./scripts/vignette-hydra-mvp2.sh`
   - `./scripts/vignette-hydra-mvp3.sh`

Planned continuation (`docs/plans/hydra-liquidity-engine.md`):

1. Finalize public API posture (internal-only vs promoted `/v1`).
2. Deepen LLP/CEP/RRP accounting and LP lifecycle.
3. Expand FX provider strategy and operations hardening.
4. Bridge/Nostr interop for portable receipt/reputation summaries.

### Aegis (planned, with partial prerequisites already present)

Aegis remains a spec-level architecture lane, paired with Hydra.

Implemented prerequisites today:

1. Runtime verification endpoints (`/internal/v1/verifications/*`).
2. Treasury compute settle/reconcile lanes.
3. Receipt/replay infrastructure and policy hooks needed by future Aegis flows.

Planned Aegis build track (`docs/plans/aegis.md`):

1. Verification plane classification and receipting.
2. Independent checker tiers and autonomy throttle.
3. Underwriting/bond/claim/dispute lanes.
4. Ground-truth registry and synthetic practice system.
5. Interop surfaces via Bridge policy lanes.

## Compute Marketplace, Fabrics, and Trust Zones

This architecture follows the liquidity-first plan in `docs/plans/2026-02-23-open-agent-economy-execution-plan.md`.

### OpenAgents Compute posture

1. Autopilot is the guaranteed demand floor (compute buyer).
2. Provider supply can come from account-attached devices and external providers.
3. Authority writes remain HTTP + receipted.
4. High-rate coordination remains in Nexus lanes; interop/exported proofs go through Bridge to Nostr.

### Nostr vs Nexus vs Bridge

1. Nostr: interop substrate for portable ads/receipts/reputation pointers.
2. Nexus: intra-domain high-throughput coordination and streaming fabric.
3. Bridge: policy/translation boundary controlling cross-fabric mirroring.

### Trust zones

1. Zone 0: operator-domain authority services.
2. Zone 0.5: account-attached enrolled devices/providers (bounded, verifiable, disableable).
3. Zone 1: external/semi-trusted providers/agents/domains.

### Message classes

1. Class 1 authority mutations:
   - authenticated HTTP, receipted, idempotent.
2. Class 2 ephemeral coordination:
   - session-authenticated streaming/control messages, non-authority.

## Bitcoin and L402 Architecture Lane (EP212)

EP212 plan is active and architecture-relevant.

Implemented now:

1. Rust control service exposes wallet/L402/paywall routes.
2. Rust wallet executor exposes payment/send/invoice execution endpoints.
3. Rust lightning-ops provides operations and smoke tooling.

Current gaps called out by the active EP212 plan (still true in current code):

1. `agent_payments_create_invoice` synthesizes invoice strings in control service.
2. `agent_payments_send_spark` currently returns local synthetic transfer completion.
3. Control-plane wallet upsert path still stores/generated mnemonic material in control service persistence.
4. Rust-native `lightning_l402_fetch` / `lightning_l402_approve` product tool execution parity is incomplete.
5. Paywall self-serve creator earnings loop remains partial/admin-gated.

EP212 target:

1. Move wallet custody/signing reality fully into executor boundary.
2. Complete real L402 fetch/approve/settle commerce loop with receipts.
3. Make paywall creation/earning loop production-real and user-self-serve.

## Command, Sync, and Compatibility Model

1. Mutations are HTTP-only authority commands.
2. Live replay/tail is Khala WS-only.
3. Compatibility SSE lane is adapter-only over existing authority outputs.
4. Client apply path must remain idempotent by `(topic, seq)`.
5. Compatibility lanes (`legacy chat aliases`, `route split`, `runtime driver aliases`) still exist for migration and contract continuity, and are decommission targets once replacement parity is proven.

## Plan Alignment Matrix

| Plan | Architecture effect | Status in architecture |
|---|---|---|
| `docs/plans/rust-migration-execution-control-plane.md` | Keeps control-plane auth/session/sync authority centralized in Rust service | Implemented baseline, still active hardening |
| `docs/plans/2026-02-23-open-agent-economy-execution-plan.md` | Defines liquidity-first sequencing, trust zones, and compute-market posture | Partially implemented, active execution track |
| `docs/plans/hydra-liquidity-engine.md` | Defines Hydra capital substrate and phased maturity | MVP-3 lanes + harnesses implemented; more phases planned |
| `docs/plans/aegis.md` | Defines verification/underwriting substrate | Planned; prerequisites present but no dedicated namespace yet |
| `docs/plans/ep212-autopilot-bitcoin-100pct.md` | Defines wallet/L402/paywall parity closure criteria | Active gap-closure plan; not fully complete |
| `docs/plans/vignette-phase0-issue-to-pr.md` | Gate L issue->verified PR execution authority harness | Active acceptance harness lane |
| `docs/plans/vignette-hydra-mvp2.md` | Hydra routing/risk observability regression gate | Implemented and active |
| `docs/plans/vignette-hydra-mvp3.md` | Hydra FX determinism/idempotency regression gate | Implemented and active |

## Implementation Sequencing

This sequencing is dependency-driven and aligns to the active plan set.

1. Phase 0: topology truth and invariant compliance
   - Keep this architecture file aligned with code.
   - Resolve control/runtime ownership ambiguity where route ownership is mixed.
2. Phase 1: control/runtime boundary hardening
   - Keep auth/session/sync authority in control service.
   - Keep execution/economy authority in runtime internal APIs.
3. Phase 2: liquidity-first compute marketplace baseline
   - Keep Gate L (`vignette-phase0-issue-to-pr`) passing.
   - Keep NIP-90 provider/consumer loop operational for Autopilot demand + supply.
4. Phase 3: Hydra maturity path
   - Keep MVP-2 and MVP-3 harnesses green.
   - Expand API posture/economics/ops according to Hydra plan phases.
5. Phase 4: EP212 parity closure
   - Replace synthetic wallet/L402 behavior with custody-compliant executor-backed flows.
   - Complete self-serve paywall and settlement loop.
6. Phase 5: Aegis phased implementation
   - Introduce verification/underwriting namespace lanes and receipts per Aegis phases.
   - Keep authority semantics and replay/idempotency invariants intact.
7. Phase 6: compatibility lane retirement
   - Retire legacy chat aliases/route split/runtime driver aliases after parity evidence.
8. Phase 7: repository debt and invariant cleanup
   - Complete legacy web tree removal/archival (completed for tracked PHP/TS legacy lanes in OA-AUDIT `#2212`).
   - Resolve `INV-12` workflow-file conflict (completed in OA-AUDIT `#2213`).

## Repository Shape (Accurate Current State)

Active app roots:

1. `apps/autopilot-desktop/`
2. `apps/openagents.com/service/`
3. `apps/runtime/`
4. `apps/lightning-ops/`
5. `apps/lightning-wallet-executor/`

Removed app roots:

1. `apps/onyx/` (archived/removed)
2. `apps/autopilot-ios/` (removed)

Shared code roots:

1. `crates/`
2. `proto/`

Tracked migration-debt lanes still present under `apps/openagents.com/`:

1. None. Retained tracked surface is `apps/openagents.com/service/` only.

Legacy web code removal status:

1. Tracked PHP/TS legacy lanes were archived and removed in OA-AUDIT `#2212`.
2. Archive manifest: `docs/audits/2026-02-25-openagents-com-legacy-code-archive-manifest.md`.
3. Residual nested workflow files were removed in OA-AUDIT `#2213` to satisfy `INV-12`.

## Known Drift to Resolve

To keep this architecture fully truthful, these drifts are explicitly acknowledged:

1. Target-state "web landing-only" is not yet complete; control service still hosts broad web/API lanes.
2. Aegis is architected and planned but not yet implemented as a dedicated runtime namespace.
3. EP212 parity work remains open on wallet custody realism, L402 tooling parity, and paywall self-serve earnings.

## Verification Baseline

Use these as baseline architecture verification lanes:

```bash
./scripts/local-ci.sh docs
./scripts/local-ci.sh workspace-compile
./scripts/local-ci.sh proto
./scripts/local-ci.sh runtime
./scripts/vignette-phase0-issue-to-pr.sh
./scripts/vignette-hydra-mvp2.sh
./scripts/vignette-hydra-mvp3.sh
```

## Canonical Companion Docs

1. `docs/core/DEPLOYMENT_RUST_SERVICES.md`
2. `docs/core/RUST_STAGING_PROD_VALIDATION.md`
3. `docs/core/RUST_LEGACY_INFRA_DECOMMISSION.md`
4. `docs/core/ROADMAP.md`
5. `docs/core/PROJECT_OVERVIEW.md`
6. `docs/plans/hydra-liquidity-engine.md`
7. `docs/plans/aegis.md`
8. `docs/plans/ep212-autopilot-bitcoin-100pct.md`
9. `docs/plans/2026-02-23-open-agent-economy-execution-plan.md`
