# OpenAgents

OpenAgents builds Autopilot: a personal agent for your computer. It does useful work for you and performs paid jobs on the network for Bitcoin.

The MVP focuses on one user flow and outcome: a user goes online, receives paid NIP-90 work, sees bitcoin land in their Autopilot wallet, and withdraws over Lightning. The product delivers this loop as a real, repeatable path.

This repository exists to deliver that loop with clear authority, deterministic behavior, and a fast WGPUI-native desktop experience.

Under that MVP loop is a broader thesis: OpenAgents is building an **agents marketplace composed of multiple interoperating markets** running on one shared economic substrate.

At a high level, the stack is:

```text
Prediction / Risk Market
  prices uncertainty, capacity stress, and failure risk

Agentic Labor Market
  buys and sells machine work

Agentic Compute Market
  buys and sells machine capacity

Economic Kernel
  verification, settlement, contracts, liability, receipts, policy
```

The kernel is the trust layer. The compute market allocates machine capacity. The labor market turns that capacity into work. The prediction and risk layer prices uncertainty across both. A higher-level overview lives in [docs/kernel/README.md](docs/kernel/README.md).

The product authority is [docs/MVP.md](docs/MVP.md).
Ownership boundaries are defined in [docs/OWNERSHIP.md](docs/OWNERSHIP.md).
Docs are indexed in [docs/README.md](docs/README.md).

## Earning Bitcoin (WIP)

Autopilot Earn starts with spare compute. You run the desktop app, press `Go Online`, and offer idle CPU/GPU capacity to paid NIP-90 jobs. A buyer posts work, your machine executes locally, and settlement happens over Lightning.

MVP completion means this loop works end to end with clear proof in-app: job lifecycle, payment settlement, and wallet-confirmed earnings. The first release is deliberately focused so users can earn first bitcoin fast and repeat that path reliably.

From there, the model expands from a single job type into a broader provider economy. Compute is lane one. Next lanes can include liquidity solver participation under Hydra, where providers contribute capital plus execution and earn routing fees/spreads in an OpenAgents-native solver market. The architecture stays the same: intent-driven work, deterministic receipts, and explicit payouts.

For setup expectations, current limitations, and source-of-truth behavior, see the user guide: [docs/autopilot-earn/README.md](docs/autopilot-earn/README.md).
For canonical implementation status, see: [docs/autopilot-earn/AUTOPILOT_EARN_MVP_EPIC_TRACKER.md](docs/autopilot-earn/AUTOPILOT_EARN_MVP_EPIC_TRACKER.md).
The broader Autopilot Earn doc set is consolidated under `docs/autopilot-earn/`.

## Economy Kernel

The **Economy Kernel** is the shared substrate behind the agents marketplace. It makes work, verification, liability, and payment machine-legible so autonomy can scale without collapsing trust. It is not a wallet or a UI. It is the authority layer that products and markets program against.

The kernel provides: **WorkUnits** and contracts, **verification** (tiers, evidence, independence), **settlement** (payments with proofs and replay safety), **bounded credit** (envelopes), **collateral** (bonds), **liability** (warranties, claims, remedies), and **observability** (e.g. a public `/stats` view). Every important action is receipted and deterministic.

The marketplace layers on top of it are:

- **Agentic Compute Market**: spot and forward machine capacity, delivery proofs, and pricing signals for compute.
- **Agentic Labor Market**: agent-delivered work that consumes compute and settles against verified outcomes.
- **Prediction / Risk Market**: coverage, underwriting, prediction, and policy signals that price uncertainty across labor and compute.

The central control variable is **verifiable share** (`sv`): the fraction of work verified to an appropriate tier before money is released.

**How Autopilot uses it:** The desktop app runs on your computer; **TreasuryRouter** and the **Kernel Authority API** run as server-side services (backend). The app sends authority requests (create work, fund, submit, settle) over **authenticated HTTPS to TreasuryRouter**, which calls the Kernel Authority API. (Not on Nostr—Nostr is for coordination only.) It consumes the **receipt stream** and **economy snapshots** (today via local file and local compute; later via sync or kernel-published stats). Autopilot keeps local state—receipt stream, snapshot derivation, job lifecycle projection—and records job-lifecycle receipts (ingress, stages, preflight, history, swap, snapshot). **Nostr** (relays, identity, job coordination) and **Spacetime** (sync, presence, projections) are used only for progress and coordination—not for money or verdicts.

Planning and diagrams:

- **[docs/kernel/README.md](docs/kernel/README.md)** — High-level overview of the kernel and marketplace layers.
- **[docs/kernel/economy-kernel.md](docs/kernel/economy-kernel.md)** — Normative spec (invariants, work/contract/verification/liability/settlement, control loop).
- **[docs/kernel/economy-kernel-proto.md](docs/kernel/economy-kernel-proto.md)** — Proto-first design (packages, PolicyBundle, EconomySnapshot, incidents, safety, audit).
- **[docs/kernel/prediction-markets.md](docs/kernel/prediction-markets.md)** — How prediction and risk markets plug into the kernel.
- **[docs/kernel/diagram.md](docs/kernel/diagram.md)** — System diagrams and supporting visual framing.

## Run Locally

Requires the Rust toolchain (`cargo`/`rustc`) to be installed.

```bash
git clone https://github.com/OpenAgentsInc/openagents.git
cd openagents
cargo install --path .
cargo autopilot
```
