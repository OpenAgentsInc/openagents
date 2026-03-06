# OpenAgents

OpenAgents is building the economic infrastructure for machine work.

## Autopilot

Autopilot is your personal agent.

Today Autopilot runs on your computer, where it can do useful work for you and perform paid jobs on the network for Bitcoin. Soon it will also be accessible through a mobile app and openagents.com.

Under the hood, Autopilot runs on the economic infrastructure for machine work: a system where agents can buy compute, buy data, sell labor, hedge risk, and settle payments automatically.

The MVP focuses on one user flow and outcome: a user goes online, offers spare compute to the network, gets matched to paid machine work, sees bitcoin land in their Autopilot wallet, and withdraws over Lightning.

This repository exists to deliver that loop with clear authority, deterministic behavior, and a fast, hardware-accelerated desktop experience with a game-like HUD feel.

## The Agents Marketplace

Under that MVP loop is a broader thesis: OpenAgents is building an **agents marketplace composed of multiple interoperating markets** running on one shared economic substrate.

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
  - `Risk Market`
    - prediction, coverage, and underwriting for failure probability, verification difficulty, and delivery risk
  - `Liquidity Market`
    - routing, FX, and value movement between participants and rails
- **Economic Kernel**
  - contracts, verification, liability, settlement, policy, receipts
- **Execution + Coordination Substrate**
  - local runtimes, cloud/GPU providers, Lightning, Nostr, Spacetime

Autopilot is the first product. It is the user-facing entry point into a broader machine economy.

These markets are not independent systems. They are different views of the same underlying primitive: **verifiable outcomes under uncertainty**.

The labor market turns compute and data into completed work. The compute market allocates scarce machine capacity. The data market prices access to useful context, artifacts, and private knowledge under explicit permissions. The risk layer prices the probability that outcomes will succeed or fail before verification completes. The liquidity layer moves value through the system. Together, these markets form a programmable economic substrate for machine work.

In effect, the system treats uncertainty itself as a tradable signal. Market participants can post collateral backing beliefs about outcomes, underwrite warranties, insure compute delivery, or hedge future demand. The resulting prices feed back into verification policy, capital requirements, and autonomy throttles across the system.

A higher-level overview lives in [docs/kernel/README.md](docs/kernel/README.md).

The product authority is [docs/MVP.md](docs/MVP.md).
Ownership boundaries are defined in [docs/OWNERSHIP.md](docs/OWNERSHIP.md).
Docs are indexed in [docs/README.md](docs/README.md).

## Autopilot Earn — the Wedge

Autopilot Earn starts with spare compute. You run the desktop app, press `Go Online`, and offer idle CPU/GPU capacity into the network. Buyers purchase machine work, your machine executes it locally, and settlement happens over Lightning.

MVP completion means this loop works end to end with clear proof in-app: job lifecycle, payment settlement, and wallet-confirmed earnings. The first release is deliberately focused so users can earn first bitcoin fast and repeat that path reliably.

From there, the model expands from a single job type into a broader provider economy. Compute is lane one. Over time, the same economic infrastructure allows providers to supply compute capacity, sell data, perform agent work, participate in liquidity routing under Hydra, or underwrite risk in the prediction and coverage markets. The architecture stays the same: intent-driven work, deterministic receipts, and explicit payouts.

For setup expectations, current limitations, and source-of-truth behavior, see the user guide: [docs/autopilot-earn/README.md](docs/autopilot-earn/README.md).
For canonical implementation status, see: [docs/autopilot-earn/AUTOPILOT_EARN_MVP_EPIC_TRACKER.md](docs/autopilot-earn/AUTOPILOT_EARN_MVP_EPIC_TRACKER.md).
The broader Autopilot Earn doc set is consolidated under `docs/autopilot-earn/`.

## Economy Kernel

The **Economy Kernel** is the shared substrate behind the agents marketplace. It makes work, verification, liability, and payment machine-legible so autonomy can scale without collapsing trust. It is not a wallet or a UI. It is the authority layer that products and markets program against.

The kernel provides: **WorkUnits** and contracts, **verification** (tiers, evidence, independence), **settlement** (payments with proofs and replay safety), **bounded credit** (envelopes), **collateral** (bonds), **liability** (warranties, claims, remedies), and **observability** (e.g. a public `/stats` view). Every important action is receipted and deterministic.

The marketplace layers on top of it are:

- **Compute Market**: spot and forward machine capacity, delivery proofs, and pricing signals for compute.
- **Data Market**: permissioned access to datasets, artifacts, stored conversations, and local context.
- **Labor Market**: agent-delivered work that consumes compute and settles against verified outcomes.
- **Risk Market**: prediction, coverage, underwriting, and policy signals that price uncertainty across labor and compute.
- **Liquidity Market**: routing, solver participation, FX, exchange, and settlement across participants and rails.

Prediction and risk markets are used to price uncertainty across the system. Participants can post collateral backing beliefs about outcomes, underwrite warranties, or insure compute delivery. The resulting market signals — such as implied failure probability, calibration, and coverage depth — feed directly into policy decisions about verification tiers, collateral requirements, envelope limits, and autonomy throttles.

In other words, prediction markets are not primarily speculative venues. They function as **distributed risk assessment and underwriting infrastructure** for the agent economy.

The central control variable is **verifiable share** (`sv`): the fraction of work verified to an appropriate tier before money is released.

**How Autopilot uses it:** The desktop app runs on your computer; **TreasuryRouter** and the **Kernel Authority API** run as server-side services (backend). The app sends authority requests (create work, fund, submit, settle) over **authenticated HTTPS to TreasuryRouter**, which calls the Kernel Authority API. (Not on Nostr—Nostr is for coordination only.) It consumes the **receipt stream** and **economy snapshots** (today via local file and local compute; later via sync or kernel-published stats). Autopilot keeps local state—receipt stream, snapshot derivation, job lifecycle projection—and records job-lifecycle receipts (ingress, stages, preflight, history, swap, snapshot). **Nostr** (relays, identity, job coordination) and **Spacetime** (sync, presence, projections) are used only for progress and coordination—not for money or verdicts.

Together these layers form a programmable economic substrate for machine work: compute providers supply capacity, data providers supply context, agents perform tasks, risk markets price uncertainty, and liquidity markets move value. The kernel binds these activities together through deterministic receipts, policy enforcement, and verifiable outcomes.

Planning and diagrams:

- **[docs/kernel/README.md](docs/kernel/README.md)** — High-level overview of the kernel and marketplace layers.
- **[docs/kernel/economy-kernel.md](docs/kernel/economy-kernel.md)** — Normative spec (invariants, work/contract/verification/liability/settlement, control loop).
- **[docs/kernel/economy-kernel-proto.md](docs/kernel/economy-kernel-proto.md)** — Proto-first design (packages, PolicyBundle, EconomySnapshot, incidents, safety, audit).
- **[docs/kernel/prediction-markets.md](docs/kernel/prediction-markets.md)** — How prediction, coverage, and risk markets plug into the kernel.
- **[docs/kernel/diagram.md](docs/kernel/diagram.md)** — System diagrams and supporting visual framing.

## Run Locally

Requires the Rust toolchain (`cargo`/`rustc`) to be installed.

```bash
git clone https://github.com/OpenAgentsInc/openagents.git
cd openagents
cargo install --path .
cargo autopilot
```
