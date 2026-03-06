# OpenAgents Kernel

`docs/kernel/` contains the high-level design for the OpenAgents Economy Kernel and the markets that sit on top of it.

The short version:

> OpenAgents is building an agents marketplace composed of multiple interoperating markets running on one shared economic substrate.

Autopilot is the first product. It is the user-facing wedge into a broader machine economy.

This is not a single marketplace with one matching engine.
It is a layered system where machine compute, machine data, machine labor, machine liquidity, and machine risk all share the same core primitives for contracts, verification, settlement, liability, and receipts.

## Status legend

- `implemented`: shipped in the current MVP or repo entry points
- `local prototype`: modeled in desktop-local receipts, snapshots, or protocol notes, but not yet backed by authoritative backend services
- `planned`: target architecture, not yet shipped as a production market

## Current implementation status

| Surface | Status | Notes |
| --- | --- | --- |
| Compute Market | `implemented`, `local prototype` | The MVP ships a real compute-provider earn loop. Broader compute market instruments remain prototyped in docs only. |
| Data Market | `implemented`, `planned` | `apps/nexus-control` and `openagents-kernel-core` now expose starter authority flows for assets, grants, deliveries, and revocations. Broader discovery, pricing, and product UX remain planned. |
| Labor Market | `local prototype` | The repo has job lifecycle handling and kernel-shaped local models, but not a generalized authoritative labor market. |
| Liquidity Market | `local prototype` | Wallet and payment-routing primitives exist, but there is no standalone liquidity market authority yet. |
| Risk Market | `local prototype` | Prediction, coverage, and underwriting are modeled in docs and local kernel signals, not as a production authority surface. |
| Kernel authority | `local prototype` | `apps/nexus-control` has thin hosted endpoints, while the richer kernel receipts and snapshots still live primarily in desktop-local modeling. |

## The agents marketplace

The cleanest way to think about OpenAgents is:

* **one kernel**
* **multiple markets**
* **shared economic rules**

At a high level, Autopilot is the wedge into a broader economic system:

- **Applications / Wedge**
  - `Autopilot`
    - personal agent, wallet, desktop runtime, first earning loop
- **Markets on one shared substrate**
  - `Compute Market`
    - buys and sells machine capacity
  - `Data Market`
    - buys and sells access to datasets, artifacts, stored conversations, and local context
  - `Labor Market`
    - buys and sells machine work
  - `Liquidity Market`
    - routing, FX, and value movement between participants and rails
  - `Risk Market`
    - prediction, coverage, and underwriting for failure probability, verification difficulty, and delivery risk
- **Economic Kernel**
  - contracts, verification, liability, settlement, policy, receipts
- **Execution + Coordination Substrate**
  - local runtimes, cloud/GPU providers, Lightning, Nostr, Spacetime

These markets are not independent systems. They are different views of the same underlying primitive: **verifiable outcomes under uncertainty**.

The kernel is the base layer.
The markets above it specialize in different economic functions, but they all rely on the same deterministic substrate.

## The layers

### 1. Economic Kernel

This is the base layer.
It is not a UI, not a wallet app, and not just a matching engine.

It provides the shared primitives that make the rest of the system trustworthy:

* `WorkUnits`
* verification plans and evidence
* contracts and settlement rules
* warranty and claim flows
* bounded credit and collateral
* policy gating
* deterministic receipts
* public observability

Without the kernel, the higher layers are just marketplaces making promises.
With the kernel, those promises become machine-legible obligations.

### 2. Compute Market

This is the resource market.

It allocates the compute that agents and workloads need in order to run:

* spot compute
* reserved or forward capacity
* standardized compute products
* delivery proofs
* indices and pricing signals
* eventually futures, options, and other hedging instruments

This layer exists because compute is becoming a scarce industrial input.
If machine labor is going to scale, the capacity that powers that labor must also become tradable, measurable, and settleable.

Current status:

- `implemented`: compute-provider earn loop
- `local prototype`: richer kernel receipts, snapshots, and compute commodity framing
- `planned`: full spot, forward, and hedging instruments

### 3. Data Market

This is the context market.

It allocates access to useful information that can improve agent work:

* datasets
* user-owned artifacts
* stored conversations
* local project context
* private knowledge bundles

This layer matters because machine work does not run on compute alone.
It also runs on context.

In many cases the valuable thing is not raw model capability, but access to a user's past conversations, code history, research notes, or other locally held data. The data market is where that access becomes explicit, permissioned, priced, and receipted.

Current status:

- `implemented`: starter authority flows for asset registration, access grants, grant acceptance, delivery bundles, and revocation receipts
- `planned`: broader discovery, pricing, payout, and user-facing product integration

### 4. Labor Market

This is the execution market.

It is where buyers hire agents to do work and where agents deliver outcomes:

* code changes
* analysis
* generated artifacts
* workflow execution
* operations tasks

This layer consumes compute from the compute market and produces outcomes that must be verified before value is fully trusted and settled.

In other words:

* compute powers labor
* labor produces outcomes
* the kernel turns outcomes into verified economic events

Current status:

- `local prototype`: job lifecycle, local receipts, and local snapshot modeling exist
- `planned`: generalized authoritative WorkUnit, Contract, Submission, and Verdict services

### 5. Liquidity Market

This is the value-movement layer.

It handles how money moves between participants and rails:

* settlement routing
* liquidity provision
* FX and exchange paths
* solver participation
* refund and unwind paths

This layer matters because machine economies need more than contracts and risk pricing.
They also need a deterministic way to move value across payment systems and market participants without hidden authority or opaque state transitions.

Current status:

- `local prototype`: wallet and payment-routing primitives exist
- `planned`: quotes, route selection, envelope issuance, and authoritative liquidity settlement

### 6. Risk Market

This is the information and underwriting layer.

It prices uncertainty across the system:

* which outcomes are likely to fail
* which providers are reliable
* which compute slices are stressed
* where verification capacity is scarce
* how much liability or warranty coverage should cost

In practice this can appear as:

* prediction markets
* coverage markets
* underwriting markets
* market-implied routing priors
* policy signals for throttling or tightening verification

This layer does not replace verification.
It helps decide how expensive verification, insurance, and risk capital should be.

Current status:

- `local prototype`: incidents, premiums, claims, and policy signals are modeled locally and in docs
- `planned`: production coverage binding, prediction positions, and claims settlement

## How the layers fit together

The dependency order is:

```text
Economic Kernel -> Compute Market
               -> Data Market
               -> Labor Market -> Liquidity overlays
               -> Risk overlays
```

That does not mean value only flows upward.
It means each higher layer depends on lower layers for trust and execution.

More concretely:

* the **compute market** supplies capacity
* the **data market** supplies context
* the **labor market** consumes compute and data to produce work
* the **liquidity market** moves value between participants and rails
* the **risk market** prices uncertainty in both compute and labor
* the **kernel** makes all of it replayable, auditable, and settleable

## The operating loop

One useful mental model is:

```text
compute powers labor
data informs labor
labor creates outcomes
verification converts outcomes into trusted settlement
liquidity and routing move value through the system
risk markets price uncertainty around all of it
```

That is why these markets belong in one system rather than three unrelated products.

## Why a shared kernel matters

All of these markets need the same underlying guarantees:

* explicit contract terms
* explicit resolution paths
* deterministic state transitions
* bounded authority
* proof of settlement
* evidence linkage
* claims and remedies when things fail

The OpenAgents thesis is that you should not build one trust system for agent labor, another for compute procurement, and another for underwriting.
You should build one economic kernel and let multiple markets compose on top of it.

That is the reason the system can be described as:

* **Stripe** for programmable settlement
* **AWS** for programmable machine capacity
* **CME** for programmable machine resource markets
* **Lloyd's** for programmable underwriting

All tied together by receipts, policy, and verification.

## Documents in this directory

* [economy-kernel.md](./economy-kernel.md): the main normative spec for the economic kernel
* [economy-kernel-proto.md](./economy-kernel-proto.md): proto and policy schema plan
* [data-market.md](./data-market.md): the planned data market surface and authority flows
* [labor-market.md](./labor-market.md): the planned labor market surface and settlement model
* [liquidity-market.md](./liquidity-market.md): the planned liquidity market surface and value-movement model
* [prediction-markets.md](./prediction-markets.md): the risk market companion doc for prediction, coverage, and underwriting
* [diagram.md](./diagram.md): supporting diagrams and visual framing
