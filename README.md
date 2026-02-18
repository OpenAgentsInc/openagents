# OpenAgents

OpenAgents is the **operating system for the AI agent economy**. We ship three apps—**web** at [openagents.com](https://openagents.com) (**`apps/openagents.com/`**), **mobile** in **`apps/mobile/`**, and **desktop** in progress—and the platform beneath: **runtime** (identity, transport, payments, treasury on permissionless protocols), **reputation** (trajectory logging, proofs), and a **marketplace** for skills and compute.

**Release quality:** Web is **alpha** (live at [openagents.com](https://openagents.com), Laravel app in **`apps/openagents.com/`**). Mobile and desktop are **prerelease**.

If you're looking for the philosophy / "why open", start with **[MANIFESTO.md](./docs/MANIFESTO.md)**.

## What’s possible with Autopilot (evolving agent)

Autopilot is an agent you can chat with in plain language. Right now you can sign in on the web, start a conversation, and use it for the capabilities we’ve shipped today (e.g. analyzing code you paste, suggesting refactors, generating configs). What it can do is not fixed: **the system is built to evolve**.

New behaviors are added as **signatures**—versioned, measurable steps with clear inputs and outputs—and can be compiled, tested, and rolled out like software. Over time, the Autopilot "network" will grow through a **marketplace** of signatures and tools: as we and others add capabilities (new tools, new skills), those become available to your Autopilot. So if you ask for something it can't do yet, the answer is "not yet"—we're building toward a world where user demand and new signatures continuously expand what's possible.

**To try it now:** go to [openagents.com](https://openagents.com), sign in, and start chatting. Your Autopilot will remember your preferences and the thread; you can use it for coding help, Blueprint updates, and whatever we've enabled so far—with more coming as the ecosystem grows.

## Apps architecture

```mermaid
flowchart LR
  subgraph user["User-facing"]
    web["openagents.com<br/>(Laravel + Inertia + React)<br/>Web UI, chat, API, control plane"]
    mobile["mobile<br/>React Native / Expo<br/>Prerelease"]
    desktop["desktop<br/>Electron + Effuse<br/>Lightning wallet execution, L402"]
  end

  subgraph platform["Platform services"]
    ops["lightning-ops<br/>L402 gateway ops, Convex, aperture"]
    executor["lightning-wallet-executor<br/>Spark wallet HTTP (pay-bolt11)"]
  end

  subgraph infra["Bitcoin / Lightning nodes"]
    bitcoind["bitcoind<br/>Bitcoin Core full node<br/>GCP VM"]
    lnd["lnd<br/>Lightning node · chain = bitcoind<br/>GCP VM"]
  end

  subgraph planned["Planned"]
    runtime["openagents-runtime<br/>Elixir/BEAM agent runtime<br/>GCP GKE · stream-from-log · DS-Elixir"]
  end

  web <--> desktop
  web --> ops
  desktop --> executor
  ops --> lnd
  lnd --> bitcoind
  web -.->|"orchestration (future)"| runtime

  style runtime stroke-dasharray: 5 5
  style planned stroke-dasharray: 5 5
  style mobile stroke-dasharray: 5 5
  style desktop stroke-dasharray: 5 5
```

- **openagents.com** (`apps/openagents.com/`): Core web app — Laravel 12, Inertia, React; chat, auth, API; production deploy to Cloud Run.
- **mobile** (`apps/mobile/`): React Native (Ignite/Expo) mobile surface — prerelease.
- **desktop** (`apps/desktop/`): Electron shell for Lightning (EP212); wallet/payment execution; Convex + OpenAgents API.
- **lightning-ops** (`apps/lightning-ops/`): Effect service that turns “what we sell and at what price” into the config that powers our L402 paywall. It reads route and policy state from Convex, compiles deterministic Aperture config (used by the gateway at l402.openagents.com), validates routes and security, and writes deployment intent back to Convex. So: Convex is the source of truth; lightning-ops is the compiler and validator that keeps the live gateway in sync.
- **lightning-wallet-executor** (`apps/lightning-wallet-executor/`): HTTP service that pays Lightning invoices on behalf of agents. When Autopilot or another service needs to pay a bolt (e.g. to call a paid API like sats4ai or our own L402 route), this service holds the sats (via Breez Spark wallet) and exposes `POST /pay-bolt11`: you send an invoice and limits, it pays within policy (allowed hosts, max amount). So: the “agent-owned” wallet in the cloud that enables buyer-side L402 and paid tool calls.
- **Bitcoin / Lightning nodes**: Our own full Bitcoin node (`bitcoind`) and Lightning node (`lnd`) on GCP VMs. LND uses bitcoind as its chain backend. The L402 gateway (Aperture) uses LND to verify incoming payments; desktop and other flows can use LND for outbound payments. Setup and ops: [docs/plans/active/lightning/GCP_BITCOIND_LND_2VM_PLAN.md](docs/plans/active/lightning/GCP_BITCOIND_LND_2VM_PLAN.md); see also [docs/lightning/README.md](docs/lightning/README.md).
- **openagents-runtime** (planned): Elixir agent runtime; see [docs/plans/active/elixir-agent-runtime-gcp-implementation-plan.md](docs/plans/active/elixir-agent-runtime-gcp-implementation-plan.md).

**Vision and architecture:** [docs/SYNTHESIS.md](docs/SYNTHESIS.md) (“OpenAgents: The Agentic OS”) is the north-star spec: what OpenAgents is (the OS for the AI agent economy), core primitives (identity, transport, payments, treasury, FX), the wedge→platform path (Autopilot → trajectory/issue moat → Neobank → skills/compute marketplace → Exchange), and a status-tagged stack. It defers to [SYNTHESIS_EXECUTION.md](docs/SYNTHESIS_EXECUTION.md) for what’s wired today and to [GLOSSARY.md](docs/GLOSSARY.md) and [PROTOCOL_SURFACE.md](docs/protocol/PROTOCOL_SURFACE.md) for terminology and protocol details.

## Quick Start (Web)

```bash
cd apps/openagents.com
composer install && npm install
composer run dev
```

See [apps/openagents.com/README.md](apps/openagents.com/README.md) for full dev and deploy instructions.

## Mobile

The mobile app lives in **`apps/mobile/`**.

## Documentation

Start with:

* **Web app**: [apps/openagents.com/](apps/openagents.com/)
* **Mobile app**: [apps/mobile/](apps/mobile/)
* **Vision / architecture (north-star spec)**: [docs/SYNTHESIS.md](docs/SYNTHESIS.md)
* **Docs index (everything else)**: [docs/README.md](docs/README.md)
* **Repo map / ownership**: [docs/PROJECT_OVERVIEW.md](docs/PROJECT_OVERVIEW.md)
* **Agent contract / contribution norms**: [AGENTS.md](./AGENTS.md)
