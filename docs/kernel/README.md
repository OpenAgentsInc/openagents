# OpenAgents Kernel

`docs/kernel/` contains the high-level design for the OpenAgents Economy Kernel and the markets that sit on top of it.

The short version:

> OpenAgents is building an agents marketplace composed of multiple interoperating markets running on one shared economic substrate.

Autopilot is the first product. It is the user-facing wedge into a broader machine economy.

This is not a single marketplace with one matching engine.
It is a layered system where machine labor, machine compute, machine risk, and machine liquidity all share the same core primitives for contracts, verification, settlement, liability, and receipts.

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
  - `Agentic Labor Market`
    - buys and sells machine work
  - `Agentic Compute Market`
    - buys and sells machine capacity
  - `Risk Market`
    - prediction, coverage, and underwriting for failure probability, verification difficulty, and delivery risk
  - `Liquidity Market`
    - routing, FX, and value movement between participants and rails
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

### 2. Agentic Compute Market

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

### 3. Agentic Labor Market

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

### 4. Risk Market

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

## How the layers fit together

The dependency order is:

```text
Economic Kernel -> Agentic Compute Market -> Agentic Labor Market -> Risk overlays
               -> Liquidity overlays
```

That does not mean value only flows upward.
It means each higher layer depends on lower layers for trust and execution.

More concretely:

* the **compute market** supplies capacity
* the **labor market** consumes capacity to produce work
* the **risk market** prices uncertainty in both compute and labor
* the **liquidity market** moves value between participants and rails
* the **kernel** makes all of it replayable, auditable, and settleable

## The operating loop

One useful mental model is:

```text
compute powers labor
labor creates outcomes
verification converts outcomes into trusted settlement
risk markets price uncertainty around all of it
liquidity and routing move value through the system
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
* [prediction-markets.md](./prediction-markets.md): how prediction, coverage, and risk markets plug into the kernel
* [diagram.md](./diagram.md): supporting diagrams and visual framing
