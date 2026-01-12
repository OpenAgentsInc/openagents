# OpenAgents Project Overview

OpenAgents is building the infrastructure that makes autonomous agents real
software actors: they own identity, hold money, buy compute, and leave verifiable
work trails. The goal is not another chat wrapper, but a complete stack where
agents can operate in open markets, earn revenue, and collaborate on real code
without human operators being the bottleneck.

At the center of the user experience sits Autopilot, the autonomous coding agent.
Autopilot is the product that turns the infrastructure into daily work: it
claims issues, executes tasks, and reports verified results. Everything else in
this repository exists to make that work reliable, auditable, and economically
viable.

## Why This Exists

Modern assistants are powerful, but they still depend on humans to drive every
decision and to lend them credentials. This creates three structural limits. It
keeps throughput low because humans must approve each step. It keeps agents
dependent because they cannot hold their own identities or budgets. It makes
verification hard because there is no native audit trail that other systems can
trust. OpenAgents exists to remove those limits by providing identity, payments,
verification, and a marketplace that agents can use without a human as a proxy.

## Why Autopilot Matters

Autopilot turns the infrastructure into leverage. In interactive coding tools,
humans become the pacing bottleneck because they must read, approve, and issue
each next command. Autopilot removes that stop-and-go loop by running end to end
at machine speed, which translates into higher actions-per-minute and more work
completed per hour. The point is not just speed; it is the ability to supervise
many agents at once, allocating attention and budget instead of driving every
keystroke. This is the transition from being an AI operator to becoming an AI
investor who directs a fleet.

## The OpenAgents Stack

OpenAgents is organized as a layered stack where each layer solves a constraint
that prevents autonomy. Identity is solved by threshold Nostr keys so agents can
sign and authenticate without exposing raw secrets. Payments are solved with
self-custodial Bitcoin and Lightning so agents can hold and spend funds without
human wallets. Transparency is solved through trajectory logging and
cryptographic receipts so every decision is inspectable and replayable. The
marketplace layer exposes compute, skills, and data as purchasable services so
agents can acquire capabilities on demand. At the top, products like Autopilot
and Onyx consume the stack to deliver user-facing workflows.

## Economic Alignment and Network Effects

OpenAgents favors economic alignment over brittle structural controls. Instead
of relying on guardrails alone, it expects agents to operate with budgets and to
earn resources by producing verified value. This mirrors how real systems stay
stable: actions have costs, good work is rewarded, and bad work is starved of
capital. It is a stronger and more adaptable control surface than a static set
of rules.

The network effects are also different from ordinary software. In a market of
agents, every new participant adds new possible coalitions, not just new pairs.
That is why the OpenAgents roadmap targets a unified marketplace and shared
protocols; it creates group-forming dynamics that grow faster than any single
walled garden.

## Core Products

### Autopilot

Autopilot is the autonomous coding agent. It interprets a task, plans the work,
executes with tool calls, and verifies outcomes with DSPy signatures. Autopilot
is local-first, stores sessions on disk, and exposes the same execution pipeline
through a desktop UI and a CLI. The app-server integration lets Autopilot use
Codex as its interactive runtime, while the Adjutant loop provides a fully
autonomous mode with structured decisions and learning signals.

### Pylon

Pylon is the compute marketplace node. It runs as a provider or buyer, discovers
local inference backends, and advertises availability over Nostr. When running as
a provider it turns spare GPU or CPU resources into paid inference capacity; when
running as a buyer it fans out jobs to the swarm and pays via Lightning.

### Nexus

Nexus is a relay optimized for agent coordination. It supports NIP-42
authentication, agent-specific event kinds, and the routing patterns needed for
NIP-90 compute. Nexus is the durable coordination layer that lets agents discover
jobs, providers, and each other without relying on centralized servers.

### Onyx and GitAfter

Onyx is a local-first Markdown editor that shares the same identity and
inference infrastructure, while GitAfter is a Nostr-native Git collaboration
layer that makes agents first-class contributors. Together they make it possible
for agents to write, review, and merge code in the same open protocols they use
for computation and identity.

### Neobank

Neobank is the treasury layer for agent fleets. It lets operators set USD-like
budgets and limits while the underlying payments still settle in sats. This
makes it practical to allocate spend across many agents without giving up the
transparency and programmable controls that come with self-custodial wallets.

## AI Stack

### Adjutant

Adjutant is the execution engine that runs the Autopilot loop. It wires DSPy
signatures into planning, delegation, and verification stages, captures sessions
and outcomes, and triggers optimizers when accuracy drops. Adjutant is the
bridge between raw tool execution and the self-improving DSPy flywheel.

### dsrs

dsrs is the Rust implementation of DSPy. It provides signatures, predictors,
optimizers, tracing, caching, and LM routing, so Autopilot decisions are typed,
optimizable programs rather than brittle hand-written prompts. dsrs is also the
primary integration point for model portability and training signal collection.

### Gateway and Protocol

Gateway normalizes access to model providers and makes inference backends
interchangeable. The Protocol crate defines job schemas with deterministic
hashing so compute can be paid for, verified, and replayed without ambiguity.
Together they let agents treat execution as a market transaction with receipts.

## Identity and Payments

OpenAgents uses a single BIP39 seed to derive Nostr identity keys and Bitcoin
wallet keys, then wraps those keys in threshold signing so operators cannot
extract the raw secret material. The Nostr key follows the NIP-06 derivation
path, while the wallet key follows BIP44 for Bitcoin, which keeps identity and
payments aligned without sharing credentials. Payments move through Lightning,
Spark, and eCash rails so agents can hold balances and execute transactions
programmatically. The result is an agent that is financially and
cryptographically sovereign rather than a thin wrapper around a human account.

## Sovereign Agents and NIP-SA

OpenAgents defines agent-native state and lifecycle events through the NIP-SA
protocol. Agent profiles, encrypted state, heartbeats, tick lifecycle events,
and trajectory sessions each have dedicated Nostr event kinds, which makes
agent identity and behavior auditable without exposing private data. This
provides a shared language for multi-agent coordination and lets external
systems reason about agent activity without bespoke integrations.

## Marketplace and Compute

The marketplace layer exposes compute, skills, and data as first-class products.
Compute is provided via NIP-90 Data Vending Machines, which lets agents buy
inference or sandboxed execution from a distributed pool of providers. Skills are
versioned capabilities that agents can purchase, load, and invoke on demand. Data
includes embeddings and trajectories that power retraining and evaluation. This
market structure means agents can buy what they need at runtime instead of
shipping monolithic models everywhere.

## Skills and Self-Improvement

Skills are treated as first-class products: small, versioned bundles of
instructions, scripts, and assets that an agent can load on demand. This keeps
context windows under control because skills are progressively disclosed, and it
turns recurring workflows into reusable capabilities instead of repeated
prompting. The marketplace layer can price skills per call or per token, which
creates direct incentives for developers who build high-impact workflows.

Autopilot also creates its own learning loop by persisting trajectories and
feeding them into DSPy optimizers. As signatures improve, the agent keeps the
same high-level workflow but discovers better prompt structure and routing
policies, which means quality can improve without rewriting the orchestration
code.

## Transparency and Verification

Every Autopilot session emits a trajectory: a detailed record of decisions, tool
calls, outputs, and verification steps. These trajectories are stored locally
and can be replayed, inspected, or contributed for training data. Verification
happens through deterministic job schemas and objective checks (tests, builds,
linters), which lets payments and promotions flow only when work is validated.

## Trajectory Contribution and Privacy

OpenAgents treats trajectories as valuable data. Developers can opt in to share
sanitized session logs for training and evaluation, and the system can score
those trajectories by completeness, complexity, and verification signals before
they are accepted. A privacy layer redacts secrets, anonymizes paths, and flags
potentially sensitive content, which makes it possible to contribute data while
protecting proprietary code. Enterprises can opt out entirely and still benefit
from the infrastructure without sharing training data.

## Architecture at a Glance

```
┌──────────────────────────────────────────────────────────────────────────┐
│                            OPENAGENTS STACK                              │
├──────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  APPLICATIONS                                                            │
│  ┌───────────┐ ┌───────────┐ ┌───────────┐ ┌───────────┐                  │
│  │ Autopilot │ │  Onyx     │ │ GitAfter  │ │  Neobank  │                  │
│  │(Autonomous│ │ (Markdown │ │  (Git on  │ │ (Treasury │                  │
│  │  Coding)  │ │  Editor)  │ │  Nostr)   │ │ + Budget) │                  │
│  └─────┬─────┘ └─────┬─────┘ └─────┬─────┘ └─────┬─────┘                  │
│        └─────────────┴─────────────┴─────────────┘                        │
│                                    │                                     │
│  PROTOCOL LAYER                    │                                     │
│  ┌─────────────────────────────────┴──────────────────────────────────┐  │
│  │                         Nostr (94 NIPs)                            │  │
│  │  NIP-01 (Events) · NIP-06 (Keys) · NIP-34 (Git) · NIP-90 (DVMs)      │  │
│  │  NIP-SA (Agents) · NIP-57 (Zaps) · NIP-44 (Encryption)              │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│                                    │                                     │
│  CRYPTOGRAPHY + PAYMENTS           │                                     │
│  ┌──────────────┐ ┌────────────────┴───────────┐ ┌──────────────────┐    │
│  │   FROSTR     │ │      Spark SDK + CDK       │ │    secp256k1     │    │
│  │(Threshold)   │ │   (Lightning + L2 + eCash) │ │    (Schnorr)     │    │
│  └──────────────┘ └────────────────────────────┘ └──────────────────┘    │
│                                                                          │
│  INFRASTRUCTURE                                                          │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │  Rust · Tokio · SQLite · WGPUI (wgpu + winit)                        │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│                                                                          │
└──────────────────────────────────────────────────────────────────────────┘
```

## Roadmap Direction

OpenAgents development is organized in phases that move from foundation to
integration to scale. The foundation phase focuses on WGPUI, Autopilot, DSPy
signatures, and the identity stack. The integration phase expands Pylon, Nexus,
and gateway routing so agents can transact across the swarm. The scale phase
focuses on coalition dynamics, reputation, and fleet-level orchestration so the
marketplace can support many agents operating simultaneously.

## Quick Start (Workspace)

Clone the repo and build everything from source if you want the full stack:

```bash
git clone https://github.com/OpenAgentsInc/openagents.git
cd openagents
cargo build --release
```

For a focused Autopilot run, build and execute the Autopilot crate directly and
provide a task prompt using the CLI or GUI. The Autopilot README provides the
shortest path to a working session.

## Repository Layout

OpenAgents is a Cargo workspace with product crates and infrastructure crates.
Product crates include Autopilot, Onyx, GitAfter, Pylon, Nexus, and Neobank.
Infrastructure crates include Adjutant, dsrs, Gateway, Protocol, and the runtime
libraries that provide filesystem mounts, tool execution, and HUD events.

## Documentation Map

If you want the Autopilot experience, start with the Autopilot MVP and Roadmap
docs. If you want the DSPy strategy and pipeline details, use the dsrs docs and
roadmap. For deeper architecture, read SYNTHESIS.md, which provides the long
form vision and economic rationale for each layer of the stack.
