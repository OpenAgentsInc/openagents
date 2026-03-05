# OpenAgents

OpenAgents builds Autopilot: a personal agent for your computer. It does useful work for you and performs paid jobs on the network for Bitcoin.

The MVP focuses on one user flow and outcome: a user goes online, receives paid NIP-90 work, sees bitcoin land in their Autopilot wallet, and withdraws over Lightning. The product delivers this loop as a real, repeatable path.

This repository exists to deliver that loop with clear authority, deterministic behavior, and a fast WGPUI-native desktop experience.

The product authority is [docs/MVP.md](docs/MVP.md).
Ownership boundaries are defined in [docs/OWNERSHIP.md](docs/OWNERSHIP.md).

## Earning Bitcoin (WIP)

Autopilot Earn starts with spare compute. You run the desktop app, press `Go Online`, and offer idle CPU/GPU capacity to paid NIP-90 jobs. A buyer posts work, your machine executes locally, and settlement happens over Lightning.

MVP completion means this loop works end to end with clear proof in-app: job lifecycle, payment settlement, and wallet-confirmed earnings. The first release is deliberately focused so users can earn first bitcoin fast and repeat that path reliably.

From there, the model expands from a single job type into a broader provider economy. Compute is lane one. Next lanes can include liquidity solver participation under Hydra, where providers contribute capital plus execution and earn routing fees/spreads in an OpenAgents-native solver market. The architecture stays the same: intent-driven work, deterministic receipts, and explicit payouts.

For setup expectations, current limitations, and source-of-truth behavior, see the user guide: [docs/EARN.md](docs/EARN.md).
For canonical implementation status, see: [docs/AUTOPILOT_EARN_MVP_EPIC_TRACKER.md](docs/AUTOPILOT_EARN_MVP_EPIC_TRACKER.md).

## Economy Kernel

The **Economy Kernel** is the economic operating system behind Autopilot Earn: it makes work, verification, liability, and payment machine-legible so autonomy can scale without collapsing trust. It is not a wallet or a UI—it’s the authority layer that products (Autopilot, marketplace, compute) program against.

The kernel provides: **WorkUnits** and contracts, **verification** (tiers, evidence, independence), **settlement** (payments with proofs and replay safety), **bounded credit** (envelopes), **collateral** (bonds), **liability** (warranties, claims, remedies), and **observability** (e.g. a public `/stats` view). Every important action is receipted and deterministic. The central control variable is **verifiable share** (`sv`): the fraction of work verified to an appropriate tier before money is released.

**How Autopilot uses it:** The desktop app talks to the kernel over **authenticated HTTP** for authority (create work, fund, submit, settle). It consumes the **receipt stream** and **economy snapshots** (today via local file and local compute; later via sync or kernel-published stats). Autopilot keeps local state—receipt stream, snapshot derivation, job lifecycle projection—and records job-lifecycle receipts (ingress, stages, preflight, history, swap, snapshot). **Nostr** (relays, identity, job coordination) and **Spacetime** (sync, presence, projections) are used only for progress and coordination—not for money or verdicts.

Planning and diagrams:

- **[docs/plans/economy-kernel.md](docs/plans/economy-kernel.md)** — Normative spec (invariants, work/contract/verification/liability/settlement, control loop).
- **[docs/plans/economy-kernel-proto.md](docs/plans/economy-kernel-proto.md)** — Proto-first design (packages, PolicyBundle, EconomySnapshot, incidents, safety, audit).
- **[docs/plans/diagram.md](docs/plans/diagram.md)** — System diagrams (architecture, lifecycle, receipts, state machines, control loop, Autopilot ↔ kernel).

## Run Locally

Requires the Rust toolchain (`cargo`/`rustc`) to be installed.

```bash
git clone https://github.com/OpenAgentsInc/openagents.git
cd openagents
cargo install --path .
cargo autopilot
```
