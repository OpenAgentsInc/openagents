# 2026-03-07 Five Markets Launch Gap Analysis

> Historical note: This audit is a point-in-time snapshot as of March 7, 2026. Current product and architecture authority lives in `README.md`, `docs/MVP.md`, `docs/OWNERSHIP.md`, and `docs/kernel/`. File paths, test status, and implementation claims here may be superseded by later commits.

Author: Codex  
Status: complete  
Scope: user-provided Episode 213 launch note, `README.md`, `docs/kernel/`, and current repo implementation for the five markets: Compute, Data, Labor, Liquidity, and Risk.

## Objective

Audit what is and is not implemented for the five markets announced to launch weekly starting on:

- March 11, 2026: Compute
- March 18, 2026: Data
- March 25, 2026: Labor
- April 1, 2026: Liquidity
- April 8, 2026: Risk

This audit uses a strict distinction between:

- `productized`: visible, end-user-capable behavior in `apps/autopilot-desktop`
- `starter authority slice`: typed objects, authenticated HTTP mutations, canonical receipts, and tests in `apps/nexus-control` + `crates/openagents-kernel-core`
- `missing`: required for the launch-note behavior but not yet exposed as a real user-facing market

## Sources Reviewed

Launch framing:

- User-provided Episode 213 market announcement

Product / architecture authority:

- `README.md`
- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/kernel/README.md`
- `docs/kernel/economy-kernel.md`
- `docs/kernel/economy-kernel-proto.md`
- `docs/kernel/prediction-markets.md`
- `docs/kernel/labor-market.md`
- `docs/kernel/liquidity-market.md`
- `docs/kernel/data-market.md`
- `docs/kernel/nostr-managed-chat-contract.md`
- `docs/kernel/diagram.md`

Implementation surfaces reviewed:

- `apps/autopilot-desktop/src/provider_nip90_lane.rs`
- `apps/autopilot-desktop/src/starter_demand_client.rs`
- `apps/autopilot-desktop/src/kernel_control.rs`
- `apps/autopilot-desktop/src/labor_orchestrator.rs`
- `apps/autopilot-desktop/src/spark_wallet.rs`
- `apps/autopilot-desktop/src/state/swap_quote_adapter.rs`
- `apps/autopilot-desktop/src/state/swap_contract.rs`
- `apps/autopilot-desktop/src/panes/credit.rs`
- `apps/autopilot-desktop/src/state/earn_kernel_receipts.rs`
- `apps/autopilot-desktop/src/state/economy_snapshot.rs`
- `apps/nexus-control/src/lib.rs`
- `apps/nexus-control/src/economy.rs`
- `apps/nexus-control/src/kernel.rs`
- `crates/openagents-kernel-core/src/authority.rs`
- `crates/openagents-kernel-core/src/compute.rs`
- `crates/openagents-kernel-core/src/data.rs`
- `crates/openagents-kernel-core/src/labor.rs`
- `crates/openagents-kernel-core/src/liquidity.rs`
- `crates/openagents-kernel-core/src/risk.rs`
- `crates/openagents-kernel-core/src/receipts.rs`
- `crates/openagents-kernel-core/src/snapshots.rs`
- `proto/openagents/common/v1/common.proto`
- `proto/openagents/economy/v1/receipt.proto`
- `proto/openagents/economy/v1/snapshot.proto`
- `proto/openagents/compute/v1/compute.proto`
- `proto/openagents/labor/v1/work.proto`
- `crates/openagents-kernel-proto/README.md`

## Validation Run

Executed during this audit:

- `cargo test -p nexus-control`
- `cargo test -p autopilot-desktop --bin autopilot-desktop provider_nip90_lane::tests::desktop_earn_harness_relay_execute_publish_wallet_confirm_end_to_end`
- `cargo test -p autopilot-desktop --bin autopilot-desktop mission_control_earn_loop_wallet_confirmed_end_to_end`

Result: all passed.

## Executive Verdict

1. The repo now implements all five markets as `starter authority slices`. This is no longer just documentation. `apps/nexus-control` exposes authenticated kernel HTTP routes and passing tests for Compute, Data, Labor, Liquidity, and Risk.
2. Only `Compute` is productized in the desktop app today. The compute-provider earn loop is real: `Go Online`, receive NIP-90 work, execute it, receive wallet-confirmed sats, and withdraw over Lightning.
3. Public launch framing for `Compute` should remain "the OpenAgents Compute Market," with `inference` and `embeddings` as the first live compute product families inside that umbrella. The retained implementation is still inference-led today; embeddings are not yet productized as a distinct live family in the desktop.
4. `Labor` is partially productized, but only as supporting infrastructure around the compute lane and local Codex labor modeling. It is not yet a launched open labor market for arbitrary coding work.
5. `Data`, `Liquidity`, and `Risk` are implemented as backend/kernel state machines, receipts, and tests, but they are not yet exposed as real end-user market lanes in Autopilot.
6. The biggest remaining gap is not object modeling. It is `market productization`: seller UX, buyer UX, discovery, pricing, operational flows, durable authority storage, and wire/package coverage beyond the thin initial proto slice.

## Market Matrix

| Market | Starter authority slice | Productized in Autopilot | Proto coverage | Current verdict |
| --- | --- | --- | --- | --- |
| Compute | Yes | Yes | Partial | Closest to the March 11, 2026 launch note |
| Data | Yes | No | No | Backend-only starter slice |
| Labor | Yes | Partial | Yes | Local modeling + compute-lane support, not a full labor market |
| Liquidity | Yes | No | No | Backend-only starter slice with adjacent local wallet/credit prototypes |
| Risk | Yes | No | No | Backend-only starter slice |

## What Is Implemented Now

### Compute

Implemented:

- Real desktop earn loop in `apps/autopilot-desktop`:
  - provider runtime + `Go Online`
  - NIP-90 ingress / execution / result publishing
  - starter-demand seeding
  - wallet-confirmed payout tracking
  - withdrawal via Spark wallet
- Labor-side kernel authority wiring for the active compute-provider flow:
  - `create_work_unit`
  - `create_contract`
  - `submit_output`
  - `finalize_verdict`
- Explicit compute-market backend objects and routes in `openagents-kernel-core` + `nexus-control`:
  - `ComputeProduct`
  - `CapacityLot`
  - `CapacityInstrument`
  - `DeliveryProof`
  - `ComputeIndex`
- Snapshot and `/stats` counters for compute activity.
- Passing backend compute-market flow test plus passing desktop earn-loop tests.
- Launchable market umbrella positioning in docs can be defended as `Compute Market`, not merely a single backend feature lane.

Not yet implemented / not yet productized:

- No desktop UI for browsing or creating compute products, lots, instruments, delivery proofs, or indices.
- The user-visible compute earn loop does not currently drive those explicit compute-market objects. The desktop product flow still lands primarily in labor receipts plus wallet settlement.
- The retained desktop flow is still inference-led; embeddings are not yet surfaced as a distinct live compute product family in desktop inventory, buyer procurement, or settlement UX.
- No visible compute market orderbook, catalog, or commodity-style pricing UX.
- No full compute-market wire/package surface beyond the thin `compute.v1` proto.
- No durable kernel backing store for canonical compute receipts and objects; the kernel state inside `nexus-control` is in-memory.

Launch-note fit:

- The March 11, 2026 claim "sell your spare compute for bitcoin" is broadly supported by the current desktop product.
- The strongest accurate public framing is: "We are launching the OpenAgents Compute Market with inference and embeddings as the first live compute product families." That preserves the compute-market claim without implying raw accelerator trading is already live.
- The retained implementation today can substantiate the compute-market umbrella and a real inference-led earn path, but not a fully productized embeddings family yet.
- The stronger reading "the explicit compute market is launched as a surfaced market product" is only partially true today.

### Data

Implemented:

- Backend/kernel object model for:
  - `DataAsset`
  - `AccessGrant`
  - `PermissionPolicy`
  - `DeliveryBundle`
  - `RevocationReceipt`
- Authenticated HTTP authority routes in `nexus-control` for asset registration, grants, grant acceptance, delivery, and revocation.
- Backend data-market flow test passes.
- Some redaction/export machinery exists in desktop-local receipt export code, and NIP-SA trajectory redaction exists for sensitive event content.

Not yet implemented / not yet productized:

- No desktop data-market pane or flow for registering a local dataset or conversation history as a sellable asset.
- No user-facing redaction/anonymization workflow for "Claude Code or Codex conversations sitting on your computer."
- No provider payout flow tied to data-asset sales.
- No marketplace discovery, asset listing, pricing, search, or buyer UX.
- No first-class data-market counters in `EconomySnapshot` or `/stats`.
- No `proto/openagents/data/v1/*` package and no generated data proto crate coverage.

Launch-note fit:

- The March 18, 2026 claim "sell your spare data" is not implemented as a real user-facing market yet.
- The repo has a backend starter slice for data contracts, not a shipped desktop data market.

### Labor

Implemented:

- Backend/kernel object model for:
  - `WorkUnit`
  - `Contract`
  - `Submission`
  - `Verdict`
- Authenticated labor authority routes in `nexus-control`.
- Thin labor proto package in `proto/openagents/labor/v1/work.proto`.
- Desktop compute earn flow already uses labor authority mutations through `kernel_control.rs`.
- Desktop-local Codex labor modeling in `labor_orchestrator.rs`:
  - explicit run classification
  - provenance bundles
  - submission assembly
  - deterministic verifier path
  - local verdict and claim/remedy modeling

Not yet implemented / not yet productized:

- No external labor-market intake path that actually lets the user sell arbitrary coding work overnight through the Autopilot UI.
- `CodexRunClassification::LaborMarket` is present, but economically meaningful Codex runs are still labeled as `projected / non-authoritative` in the desktop-local labor model.
- No labor-market buyer catalog, assignment, escrow, SLA surface, dispute console, or general coding-task marketplace UX.
- In practice, current inbound paid demand is still the compute-provider NIP-90 lane, not a general labor market.
- No first-class labor inventory / volume counters in `/stats`; labor observability is mostly generic receipt and verification metrics.

Launch-note fit:

- The March 25, 2026 claim "sell autonomous labor" is only partially implemented.
- The repo supports labor semantics and local Codex labor records, but not a launched open labor market for arbitrary coding contracts.

### Liquidity

Implemented:

- Backend/kernel object model for:
  - `Quote`
  - `RoutePlan`
  - `Envelope`
  - `SettlementIntent`
  - `ReservePartition`
- Authenticated liquidity authority routes in `nexus-control`.
- Snapshot and `/stats` counters for liquidity activity.
- Passing backend liquidity-market flow test.
- Adjacent local desktop prototypes exist:
  - Blink / Stablesats quote and execution adapters
  - local treasury conversion flows
  - local credit-desk / AC envelope panes and runtime lane

Not yet implemented / not yet productized:

- The desktop does not call the kernel liquidity routes today.
- No LP onboarding flow, no channel-management UX, no automatic Lightning rebalance lane, and no Hydra solver market UX.
- The existing credit desk is a local runtime/Nostr AC tool surface, not the kernel liquidity market described in the README and kernel docs.
- No `proto/openagents/liquidity/v1/*` package and no generated liquidity proto coverage.
- No explicit capital opt-in or capital-at-risk UX matching the April 1, 2026 launch note.

Launch-note fit:

- The April 1, 2026 claim "provide liquidity for yield" is not productized yet.
- What exists now is a backend liquidity authority slice plus adjacent local wallet/credit prototypes.

### Risk

Implemented:

- Backend/kernel object model for:
  - `CoverageOffer`
  - `CoverageBinding`
  - `PredictionPosition`
  - `RiskClaim`
  - `RiskSignal`
- Authenticated risk authority routes in `nexus-control`.
- Snapshot and `/stats` counters for:
  - coverage offers
  - coverage bindings
  - prediction positions
  - claims
  - risk signals
  - implied fail probability
  - calibration score
  - coverage concentration
- Passing backend risk-market flow test.
- Desktop-local receipt and snapshot code already carries a large amount of adjacent policy / incident / export / redaction machinery.

Not yet implemented / not yet productized:

- No desktop workflow for staking collateral, posting a bond, underwriting a contract, or buying/selling coverage as a user-facing product.
- No user-facing "verification and performance bond" flow for Autopilot users.
- No live integration where kernel risk signals actively gate or reshape the desktop provider experience in a visible way.
- No `proto/openagents/risk/v1/*` package and no generated risk proto coverage.
- No underwriter account system, market-depth UX, or capital-management UX.

Launch-note fit:

- The April 8, 2026 claim "underwrite verification and performance bonds" is not yet productized.
- The repo contains the backend starter slice, not the launched user-facing risk market.

## Cross-Cutting Gaps

### 1. Productization is compute-first; the other four lanes are backend-first

The repo now clearly supports the five-market architecture at the authority level, but only Compute has a real end-user flow in Autopilot.

### 2. Proto coverage is still asymmetrical

`openagents-kernel-proto` currently covers:

- `common`
- `economy`
- `compute`
- `labor`

It explicitly defers:

- `data`
- `liquidity`
- `risk`

That is enough for the current compute-provider MVP, but not enough to call the full five-market wire layer complete.

### 3. Desktop integration is narrow

`apps/autopilot-desktop/src/kernel_control.rs` currently wires only the labor-side authority mutations plus snapshot/receipt projection streams.

There is no desktop integration today for:

- data-market mutations
- compute-market object mutations
- liquidity-market mutations
- risk-market mutations

### 4. Canonical kernel state is not yet durable

`apps/nexus-control/src/kernel.rs` uses in-memory state and an in-memory receipt store for canonical kernel objects and receipts.

Implication:

- the starter authority slice is real and tested,
- but it is not yet a durable production authority substrate for multi-market launch claims.

### 5. Observability is still uneven across the five markets

`EconomySnapshot` and `/stats` have explicit counters for:

- Compute
- Liquidity
- Risk

But not comparable first-class counters for:

- Data assets / grants / deliveries / revocations
- Labor work units / contracts / submissions / verdicts

That asymmetry matters if the public story is "five markets launching one per week."

## Launch Readiness By Date

| Launch date | Market | Audit verdict |
| --- | --- | --- |
| March 11, 2026 | Compute | `Ready enough for the honest compute-provider story.` The desktop earn loop is real. The explicit compute-market object model is still backend-only. |
| March 18, 2026 | Data | `Not ready as announced.` Backend slice exists, but no user-facing data packaging, redaction, sale, or payout flow. |
| March 25, 2026 | Labor | `Not ready as announced.` Labor semantics exist, but not a shipped open labor marketplace for coding work. |
| April 1, 2026 | Liquidity | `Not ready as announced.` Backend slice exists, but no LP/channel-management or Hydra solver product flow. |
| April 8, 2026 | Risk | `Not ready as announced.` Backend slice exists, but no end-user bond / underwriting / coverage product. |

## Recommended Messaging Adjustment

If you want the Episode 213 announcement to be strictly truthful against the current repo as of March 7, 2026:

1. Describe Compute as the first fully productized lane in Autopilot 0.1.
2. Describe Data, Labor, Liquidity, and Risk as `kernel-backed starter market slices` or `early authority/API previews`, not fully launched end-user markets.
3. Avoid copy that implies users can already:
   - package and sell conversations safely,
   - let Codex accept external coding contracts overnight,
   - manage Lightning channels for yield from the desktop,
   - or post verification/performance bonds from the app.

## Highest-Priority Follow-On Work

1. Wire desktop flows to the non-compute market mutations, starting with the exact launch lane you intend to expose next.
2. Add durable storage for kernel objects and canonical receipts in `nexus-control` or the next authority service layer.
3. Add `data`, `liquidity`, and `risk` proto packages before claiming full five-market protocol readiness.
4. Add first-class observability for Data and Labor in `EconomySnapshot` and `/stats`.
5. For Data specifically, build the missing redaction/anonymization and local-asset packaging flow before any public launch claim.
6. For Labor specifically, turn the current local Codex labor modeling into an authoritative external contract flow rather than a projected local binding.
