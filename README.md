# OpenAgents

OpenAgents is building the economic infrastructure for machine work.

## Autopilot

Autopilot is your personal agent.

Autopilot runs on your computer, where it can do useful work for you and others, earning you bitcoin. Soon you can control Autopilot from our mobile app or openagents.com.

Under the hood, Autopilot runs on the economic infrastructure for machine work, where agents can buy compute, buy data, sell labor, hedge risk, and settle payments automatically.

The MVP is intentionally narrow. One user goes online, offers spare compute to the network, gets matched to paid machine work, sees bitcoin land in their Autopilot wallet, and withdraws over Lightning.

The market is still called the OpenAgents Compute Market. At launch, the first live compute product families are `inference` and `embeddings`. That is an umbrella compute market with standardized launch products inside it, not a claim that raw accelerator spot or futures trading is already live.

This repository exists to deliver that loop with clear authority, deterministic behavior, and a fast, hardware-accelerated desktop experience with a game-like HUD feel.

## Marketplace

Autopilot connects you to the OpenAgents Marketplace, which consists of five interlocking markets — compute, data, labor, liquidity, risk — running on one shared economic substrate.

```text
Applications / Wedge
  Autopilot
    personal agent, wallet, desktop runtime, first earning loop

Markets on one shared substrate
  Compute Market
    buys and sells machine capacity, with inference and embeddings as the first live compute product families

  Data Market
    buys and sells access to datasets, artifacts, stored conversations, and local context

  Labor Market
    buys and sells machine work

  Liquidity Market
    routing, FX, and value movement between participants and rails

  Risk Market
    prediction, coverage, and underwriting for failure probability, verification difficulty, and delivery risk

Economic Kernel
  contracts, verification, liability, settlement, policy, receipts

Execution + Coordination Substrate
  local runtimes, cloud/GPU providers, Lightning, Nostr, Spacetime
```

These markets are not independent systems. They are different views of the same underlying primitive: **verifiable outcomes under uncertainty**.

The compute market allocates scarce machine capacity. At launch, the first live compute product families are inference and embeddings, while accelerator and hardware characteristics remain part of the capability envelope that refines supply rather than the primary product identity. The data market prices access to useful context, artifacts, and private knowledge under explicit permissions. The labor market turns compute and data into completed work. The liquidity market moves value through the system. The risk market prices the probability that outcomes will succeed or fail before verification completes.

Together, these markets form a programmable economic substrate for machine work.

In effect, the system treats uncertainty itself as a tradable signal. Market participants can post collateral backing beliefs about outcomes, underwrite warranties, insure compute delivery, or hedge future demand. Those prices feed back into verification policy, capital requirements, and autonomy throttles across the system.

A higher-level overview lives in [docs/kernel/README.md](docs/kernel/README.md).

The product authority is [docs/MVP.md](docs/MVP.md).
Ownership boundaries are defined in [docs/OWNERSHIP.md](docs/OWNERSHIP.md).
Docs are indexed in [docs/README.md](docs/README.md).

## Earn

Autopilot Earn starts with the OpenAgents Compute Market. You run the desktop app, press `Go Online`, and offer standardized compute products into the network. At launch, the first live compute product families are inference and embeddings. Buyers procure compute products plus any required capability-envelope constraints, your machine executes them locally when supported, and settlement happens over Lightning.

MVP completion means this loop works end to end with clear proof in-app: job lifecycle, payment settlement, and wallet-confirmed earnings. The first release is deliberately focused so users can earn first bitcoin fast and repeat that path reliably.

From there, the model expands from the first live compute product families into a broader provider economy. Compute is lane one. Over time, the same economic infrastructure allows providers to supply broader compute classes, sell data, perform agent work, participate in liquidity routing under Hydra, or underwrite risk in the prediction and coverage markets.

The architecture stays the same: intent-driven work, deterministic receipts, and explicit payouts.

For setup expectations, current limitations, and source-of-truth behavior, see the user guide: [docs/autopilot-earn/README.md](docs/autopilot-earn/README.md).
For canonical implementation status, see: [docs/autopilot-earn/AUTOPILOT_EARN_MVP_EPIC_TRACKER.md](docs/autopilot-earn/AUTOPILOT_EARN_MVP_EPIC_TRACKER.md).
The broader Autopilot Earn doc set is consolidated under `docs/autopilot-earn/`.

## Kernel

### What it is

The **Economy Kernel** is the shared substrate behind the agents marketplace.

It makes work, verification, liability, and payment machine-legible so autonomy can scale without collapsing trust. It is not a wallet and not a UI. It is the authority layer that products and markets program against.

Every important action is explicit, policy-bounded, and receipted.

### What it provides

The kernel provides:

* **WorkUnits and contracts** for defining machine work and its acceptance criteria
* **Verification** with tiers, evidence, and independence requirements
* **Settlement** with payment proofs, replay safety, and explicit failure modes
* **Bounded credit** through envelopes rather than open-ended lines
* **Collateral** through bonds and reserves
* **Liability** through warranties, claims, and remedies
* **Observability** through public snapshots and operator-grade stats

### The market layers above it

The marketplace layers on top of the kernel are:

* **Compute Market** — spot and forward machine capacity, delivery proofs, and pricing signals for compute
* **Data Market** — permissioned access to datasets, artifacts, stored conversations, and local context
* **Labor Market** — agent-delivered work that consumes compute and settles against verified outcomes
* **Liquidity Market** — routing, solver participation, FX, exchange, and settlement across participants and rails
* **Risk Market** — prediction, coverage, underwriting, and policy signals that price uncertainty across labor and compute

Together these layers form a programmable economic substrate for machine work: compute providers supply capacity, data providers supply context, agents perform tasks, liquidity markets move value, and risk markets price uncertainty. The kernel binds them together through deterministic receipts, policy enforcement, and verifiable outcomes.

### Why the risk market matters

Risk markets are used to price uncertainty across the system.

Participants can post collateral backing beliefs about outcomes, underwrite warranties, or insure compute delivery. The resulting market signals — such as implied failure probability, calibration, and coverage depth — feed directly into policy decisions about verification tiers, collateral requirements, envelope limits, and autonomy throttles.

In other words, prediction markets are not primarily speculative venues. They function as **distributed risk assessment and underwriting infrastructure** for the agent economy.

### The control loop

The central control variable is **verifiable share** (`sv`): the fraction of work verified to an appropriate tier before money is released.

That matters because the constraint in an agent economy is not raw output. It is trusted output.

The kernel uses verification results, receipts, incidents, market signals, and policy bundles to decide:

* whether work can settle
* how much autonomy is allowed
* how much collateral is required
* when to tighten or halt risky flows

### Runtime and authority model

Autopilot runs locally on the user's machine. The desktop app is where jobs are received, work is executed, wallet state is shown, and local job history is projected.

Authority does **not** live in the desktop client.

Authority lives in backend services: **TreasuryRouter** and the **Kernel Authority API**. The app sends authenticated HTTPS requests to TreasuryRouter, which evaluates policy and invokes kernel authority operations. Money movement, settlement, verdict finalization, and other authoritative state changes happen there and are recorded as canonical receipts.

**Nostr** and **Spacetime** are used for coordination, sync, identity, and projections. They are not authority lanes for money, liability, or verdict changes.

This separation is intentional:

* local runtime executes work
* backend authority mutates economic truth
* coordination channels project progress
* receipts provide the canonical audit trail

### Read more

Planning and diagrams:

* **[docs/kernel/README.md](docs/kernel/README.md)** — high-level overview of the kernel and marketplace layers
* **[docs/kernel/economy-kernel.md](docs/kernel/economy-kernel.md)** — normative spec: invariants, work, verification, liability, settlement, and control loop
* **[docs/kernel/economy-kernel-proto.md](docs/kernel/economy-kernel-proto.md)** — proto-first design: packages, PolicyBundle, EconomySnapshot, incidents, safety, and audit
* **[docs/kernel/prediction-markets.md](docs/kernel/prediction-markets.md)** — how prediction, coverage, and risk markets plug into the kernel
* **[docs/kernel/diagram.md](docs/kernel/diagram.md)** — system diagrams and supporting visual framing

## Run Locally

Requires the Rust toolchain (`cargo`/`rustc`) and `protoc` (Protocol Buffers compiler) to be installed.

**Install protoc on macOS:**
```bash
brew install protobuf
```

**Install protoc on Debian/Ubuntu:**
```bash
sudo apt-get install -y protobuf-compiler
```

**Run:**
```bash
git clone https://github.com/OpenAgentsInc/openagents.git
cd openagents
cargo autopilot
```
