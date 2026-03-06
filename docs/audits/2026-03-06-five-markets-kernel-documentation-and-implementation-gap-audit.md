# 2026-03-06 Five Markets Kernel Documentation and Implementation Gap Audit

> Historical note: This audit is a point-in-time snapshot from its date. Current product and architecture authority lives in `README.md`, `docs/MVP.md`, `docs/OWNERSHIP.md`, and `docs/kernel/`. File paths, issue states, and implementation-status claims here may be superseded by later commits.


Author: Codex  
Status: complete  
Scope: `docs/kernel/` plus current repo implementation, evaluated against the new five-market framing: Compute, Data, Labor, Liquidity, and Risk.

## Objective

Audit what still needs to happen to:

1. update the documentation so the OpenAgents marketplace is consistently described as five interlocking markets on one kernel, and
2. implement those five markets fully in code rather than only in docs, local simulation, or protocol notes.

This audit is aligned to `docs/MVP.md`. A gap against the five-market architecture is not automatically an MVP bug. The current MVP remains compute-provider-first.

## Sources Reviewed

Primary kernel docs reviewed in full:

- `docs/kernel/README.md`
- `docs/kernel/economy-kernel.md`
- `docs/kernel/economy-kernel-proto.md`
- `docs/kernel/prediction-markets.md`
- `docs/kernel/diagram.md`

Product / architecture authority:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `README.md`

Current implementation surfaces reviewed:

- `Cargo.toml`
- `apps/autopilot-desktop/src/provider_nip90_lane.rs`
- `apps/autopilot-desktop/src/starter_demand_client.rs`
- `apps/autopilot-desktop/src/economy_kernel_receipts.rs`
- `apps/autopilot-desktop/src/state/earn_kernel_receipts.rs`
- `apps/autopilot-desktop/src/state/economy_snapshot.rs`
- `apps/autopilot-desktop/src/state/provider_runtime.rs`
- `apps/autopilot-desktop/src/input/reducers/provider_ingress.rs`
- `apps/autopilot-desktop/src/input/reducers/jobs.rs`
- `apps/autopilot-desktop/src/runtime_lanes.rs`
- `apps/nexus-control/src/lib.rs`
- `apps/nexus-control/src/economy.rs`
- `crates/spark/src/wallet.rs`
- `crates/nostr/core/src/nip90/*`
- `crates/nostr/nips/AC.md`
- `crates/nostr/nips/SA.md`
- `crates/nostr/nips/SKL.md`

## Executive Verdict

1. The five-market framing is now present in the repo entry points, but it is **not yet the canonical framing of the kernel spec set**. `README.md` and `docs/kernel/README.md` use the new taxonomy; `economy-kernel.md`, `economy-kernel-proto.md`, and `diagram.md` still describe a kernel centered on labor/verification/risk plus a compute extension, with Data and Liquidity mostly implicit.
2. The current codebase does **not** implement five markets. It implements:
   - a real compute-provider MVP loop,
   - a real wallet surface,
   - a small hosted-Nexus starter-demand authority service,
   - and a substantial **desktop-local simulation** of kernel receipts, policy, incidents, and snapshots.
3. Of the five markets, only **Compute** has meaningful product implementation today. **Labor** is partial and tightly coupled to NIP-90 compute jobs. **Liquidity** is partial at the wallet/protocol/simulation level. **Data** and **Risk** are not implemented as production markets.
4. Fully implementing the five-market architecture requires a new backend and proto layer. There is still no `proto/` tree, no generated kernel proto crate, no real `TreasuryRouter`, and no real `Kernel Authority API` in this repo.
5. The right interpretation is: **Autopilot MVP is still valid, but the broader marketplace architecture is ahead of the built system**. Documentation should make that asymmetry explicit instead of reading as if all five markets already exist.

## What Is Already Aligned

These pieces are already pointed in the right direction:

- `README.md` now presents the marketplace as Compute, Data, Labor, Liquidity, and Risk on a shared kernel.
- `docs/kernel/README.md` now mirrors that five-market stack and explains the dependency order clearly.
- `apps/autopilot-desktop/src/state/earn_kernel_receipts.rs` and `apps/autopilot-desktop/src/state/economy_snapshot.rs` already prototype many kernel concepts locally:
  - receipts
  - policy bundles
  - work-unit metadata
  - incidents
  - outcome registry
  - certification
  - audit exports
  - minute snapshots
- `apps/nexus-control/src/lib.rs` and `apps/nexus-control/src/economy.rs` provide a small real authority surface today:
  - desktop session issuance
  - sync token issuance
  - starter-demand poll/ack/heartbeat/fail/complete
  - public `/stats`

Those pieces are useful seeds for a real kernel implementation. They just are not the five-market platform yet.

## 1. Documentation Gap Analysis

### 1.1 Current doc state by file

| File | Current state | Gap |
| --- | --- | --- |
| `docs/kernel/README.md` | Good five-market overview. | Needs stronger MVP-vs-platform status language and links to market-specific docs that do not exist yet. |
| `docs/kernel/economy-kernel.md` | Strong kernel + verification + compute-market spec. | Does not present Compute/Data/Labor/Liquidity/Risk as the governing market taxonomy. Data and Liquidity are not first-class market sections. |
| `docs/kernel/economy-kernel-proto.md` | Strong proto plan for kernel, risk, and compute. | No first-class proto package plan for Data or Liquidity markets. Labor is mostly implicit inside outcomes/work. |
| `docs/kernel/prediction-markets.md` | Good risk-market subdoc. | Still framed as prediction/coverage/risk only; no corresponding docs exist for Data, Labor, or Liquidity. |
| `docs/kernel/diagram.md` | Good kernel/service diagrams. | Does not show a canonical five-market stack diagram or per-market flows. |

### 1.2 Documentation gaps that still need to be fixed

#### Gap A: the normative spec does not yet treat the five markets as the primary taxonomy

`docs/kernel/economy-kernel.md` still reads as:

- kernel first,
- verification/risk first,
- compute as the main extension.

That is coherent, but it does not yet match the current marketplace framing in the README. The doc explains why compute belongs in the kernel, and why prediction/risk matters, but it never does the same normalization for:

- Data as a first-class traded input,
- Labor as a first-class market layer,
- Liquidity as a first-class market layer.

#### Gap B: the proto plan is asymmetrical

`docs/kernel/economy-kernel-proto.md` makes:

- Risk first-class via `liability_market.proto`
- Compute first-class via `compute/v1/*`

But it does not do the same for:

- Data market objects
- Liquidity market objects
- Labor market package boundaries as a market rather than only as outcomes/work objects

Today the proto plan implies:

- Labor = `outcomes_work.proto`
- Risk = `liability_market.proto`
- Compute = `compute/v1/*`
- Data = provenance/data-source refs
- Liquidity = Hydra + settlement references

That asymmetry is probably fine for an internal design draft, but it is not fine if the public architecture story is now “five interlocking markets.”

#### Gap C: there are no market-specific docs for Data, Labor, or Liquidity

Right now `docs/kernel/` has:

- one kernel overview
- one normative kernel spec
- one proto plan
- one risk-market doc
- one diagram doc

It does **not** have:

- `data-market.md`
- `labor-market.md`
- `liquidity-market.md`

That creates an obvious imbalance: Compute and Risk have real conceptual surfaces, while Data and Liquidity are only named in overview prose.

#### Gap D: the diagram doc still models systems/modules, not the five-market architecture

`docs/kernel/diagram.md` is useful, but it is still mostly:

- trust zones
- receipt graph
- state machines
- proto package dependencies
- authority vs projection

What is missing is one canonical “market architecture” diagram showing:

- Compute Market
- Data Market
- Labor Market
- Liquidity Market
- Risk Market
- and how all five terminate in kernel objects and receipts

#### Gap E: MVP vs platform status is not explicit enough in kernel docs

The docs now present the five-market architecture clearly, but they do not always tell the reader which parts are:

- implemented now,
- locally simulated,
- protocol-sketched,
- or still planned.

Without that, the docs can read as if OpenAgents already ships a five-market platform, when the repo actually ships a compute-provider MVP plus local kernel prototypes.

### 1.3 Documentation work required to fully reflect the five-market emphasis

Minimum remaining documentation work:

1. Update `docs/kernel/economy-kernel.md` so the introduction and section map explicitly name the five markets and map kernel objects/modules to them.
2. Update `docs/kernel/economy-kernel-proto.md` so it has an explicit “market package map” for all five markets, even if some packages are intentionally thin or deferred.
3. Either rename `docs/kernel/prediction-markets.md` to a broader risk-market doc or add a short wrapper section that clearly positions it as the Risk Market subdoc.
4. Add new market docs:
   - `docs/kernel/data-market.md`
   - `docs/kernel/labor-market.md`
   - `docs/kernel/liquidity-market.md`
5. Add a top-of-file five-market architecture diagram to `docs/kernel/diagram.md`.
6. Add an implementation-status legend in `docs/kernel/README.md`:
   - `implemented`
   - `local prototype`
   - `planned`

## 2. Implementation Gap Analysis

## 2.1 Platform baseline gap

Before market-by-market detail, the biggest implementation gap is structural:

- there is no `proto/` tree in the repo
- there is no generated kernel proto crate
- there is no real backend `TreasuryRouter`
- there is no real backend `Kernel Authority API`
- there is no authoritative receipt store service matching the kernel spec
- there is no authoritative snapshot service matching the proto plan

What exists instead:

- desktop-local receipt and snapshot state in `apps/autopilot-desktop`
- small hosted-Nexus control endpoints in `apps/nexus-control`
- protocol/event lanes for Nostr-facing flows

So the repo currently has **kernel-shaped client/runtime prototypes**, not a full multi-market backend.

## 2.2 Market-by-market implementation status

| Market | What exists now | What is missing for full implementation |
| --- | --- | --- |
| Compute | NIP-90 provider lane, Go Online, job inbox/active job/history, starter-demand service, Spark payout loop, local receipt/snapshot modeling. | Real compute market objects: products, lots, delivery proofs, indices, forwards/futures/options, clearing, compute policy/breakers, authoritative compute receipts. |
| Data | Only implicit pieces: provenance refs, permissioning refs, local context ideas, some dataset utilities/UI molecules. | Actual market objects and flows: data asset registry, permissions, grants, pricing, purchase flow, access enforcement, receipts, revocation, provider payout. |
| Labor | Partially present through NIP-90 jobs and local job lifecycle. | Real labor market backend: WorkUnit/Contract APIs, generic service catalog, verifier assignment, acceptance criteria management, settlement against authoritative verdicts, provider/buyer matching beyond compute jobs. |
| Liquidity | Spark wallet send/receive, NIP-AC/credit concepts in protocol docs, runtime lane prototypes, treasury/stablesats simulations. | Real liquidity market: quote/execute routing, FX RFQ, solver participation, reserve partitions, LP posture, envelope issuance/commit/settle in authority services, production routing policies. |
| Risk | Strong docs and local receipt/snapshot simulation for claims, premiums, incidents, certification, drift, safety signals. | Real risk market: underwriter accounts, coverage offers/bindings, claim settlement engine, belief/market signals, calibration tracking, concentration controls, live policy integration. |

## 2.3 Detailed implementation gaps by market

### Compute

This is the only market with real MVP product behavior today.

Built now:

- Desktop provider ingress and relay health in `apps/autopilot-desktop/src/provider_nip90_lane.rs`
- Job acceptance/execution/result handling in `apps/autopilot-desktop/src/input/reducers/jobs.rs`
- Starter-demand authority flow in `apps/nexus-control/src/lib.rs`
- Wallet payout path via `crates/spark/src/wallet.rs`

Still missing:

- `ComputeProduct`
- `CapacityLot`
- `ComputeIndex`
- `DeliveryProof`
- `CapacityInstrument`
- physical vs cash settlement semantics
- compute-market breakers and open-interest metrics
- authoritative compute market receipts

Conclusion:
Current code implements **compute-provider earning**, not a full **Compute Market**.

### Data

Built now:

- provenance/data-source/permissioning concepts in local receipt modeling
- some dataset-related utilities in `crates/cad`
- a dataset UI molecule in `crates/wgpui`

Still missing:

- marketable data asset model
- access control and permission grants
- pricing and purchase flow
- deterministic delivery format for sold conversations/artifacts/local context
- local-vault permission mediation for “buy someone else’s stored context”
- settlement and revocation receipts

Conclusion:
There is **no Data Market implementation** in the current product. There are only adjacent primitives and examples.

### Labor

Built now:

- NIP-90 request intake
- local job lifecycle projection
- basic provider execution loop
- SKL/SA/AC protocol concepts and runtime lanes

Still missing:

- general WorkUnit creation API
- general Contract creation API
- worker/verifier assignment API
- authoritative verdict finalization
- marketplace-native labor matching outside compute-provider NIP-90 jobs
- skill/service catalog with pricing and settlement

Conclusion:
Labor exists today only as a **compute-job execution lane**, not as a full **Labor Market**.

### Liquidity

Built now:

- Spark receive/send flows
- starter-demand completion uses payment pointers
- local AC/credit lane prototypes in `apps/autopilot-desktop/src/runtime_lanes.rs`
- treasury/stablesats simulations in `apps/autopilot-desktop/src/app_state_domains.rs`

Still missing:

- routing engine
- FX RFQ engine
- solver marketplace
- liquidity-provider accounting
- reserve partitions
- authoritative quote/execute receipts across rails
- real `Credit Envelope` issuance/commit/settle service

Conclusion:
There is **partial liquidity plumbing and simulation**, but not a real **Liquidity Market**.

### Risk

Built now:

- strong design docs
- local modeling of:
  - premiums
  - claims
  - incidents
  - certification
  - safety signals
  - audit exports
  - minute snapshots

Still missing:

- real underwriter identity/accounts
- coverage offer placement
- coverage binding
- claims settlement with live capital
- belief/prediction positions
- live calibration and concentration measurements from real market participation
- policy safe-harbor integration from real market data

Conclusion:
Risk is **well-modeled locally** but **not implemented as a production market**.

## 3. What Else Is Needed To Fully Update Documentation

If the goal is “docs fully reflect the five-market architecture,” the remaining work is:

1. Make the five markets explicit in the normative spec, not just the overview README.
2. Give each market a first-class conceptual document.
3. Add a market/object mapping:
   - Compute -> products, lots, indices, delivery proofs, instruments
   - Data -> assets, grants, permissions, access receipts
   - Labor -> work units, contracts, submissions, verdicts
   - Liquidity -> quotes, routes, envelopes, settlements, reserves
   - Risk -> coverage offers, bindings, claims, market signals
4. Add a status matrix showing:
   - MVP implemented now
   - local prototype
   - planned backend
5. Make sure diagrams show the five markets as a stack, not just kernel internals.

## 4. What Else Is Needed To Fully Implement The Five Markets In Code

This is the minimum serious platform build, not a doc rewrite:

### P0: shared foundation

1. Create the real `proto/openagents/**/v1` tree and generated crates.
2. Build a real backend authority baseline:
   - `TreasuryRouter`
   - `Kernel Authority API`
   - canonical receipt store
   - snapshot computation service
3. Move the local receipt/snapshot model into shared wire contracts and backend services, while preserving desktop replay safety.

### P1: Compute Market

1. Convert the current compute-provider MVP into backend-backed WorkUnit/Contract/Settlement flows.
2. Add compute-market objects:
   - products
   - lots
   - delivery proofs
   - indices
   - instrument lifecycle
3. Expose compute-market receipts and stats from the backend rather than desktop-local projection only.

### P1: Labor Market

1. Build authoritative WorkUnit and Contract services.
2. Add provider capability cataloging and matching.
3. Add verifier assignment and verdict finalization.
4. Make settlement depend on authoritative labor receipts.

### P1: Data Market

1. Define a `DataAsset` object model.
2. Build permission, grant, and revocation flows.
3. Add pricing and purchase APIs.
4. Add local vault / conversation export mediation so “buy access to local context” is explicit and safe.

### P2: Liquidity Market

1. Build quote/execute APIs for value movement.
2. Build routing and FX services.
3. Add reserve partitions and solver/LP models.
4. Turn envelope/credit concepts into actual authority services.

### P2: Risk Market

1. Build underwriter accounts and collateral tracking.
2. Build coverage offer placement and binding.
3. Build claims and remedy settlement.
4. Add market signal generation and policy integration.

## 5. Priority Recommendation

If OpenAgents wants to preserve MVP momentum and still make the five-market thesis credible, the right order is:

1. Finish the documentation alignment so the kernel docs clearly distinguish:
   - current MVP reality
   - local prototypes
   - full five-market target architecture
2. Build the shared proto + backend authority layer.
3. Graduate the current compute-provider MVP into a real backend-backed Compute + Labor foundation.
4. Add Data, Liquidity, and Risk as first-class markets after the shared kernel is real.

That sequencing matches `docs/MVP.md`. It avoids pretending the platform is already broader than the code, while still preserving the larger architecture.

## Bottom Line

OpenAgents now has a good **story** for five markets, but not yet a five-market **spec set** and definitely not a five-market **implementation**.

Today’s built system is best described as:

- a real Compute-provider MVP,
- a partial Labor lane,
- partial Liquidity plumbing,
- local Risk simulation,
- and no true Data market yet.

To make the architecture real, the next critical step is not “more README language.” It is:

> **turning the local kernel simulation into a real backend authority and proto layer, then adding first-class market objects for Compute, Data, Labor, Liquidity, and Risk.**
