# Background Agents (Open-Inspect) teardown — 2026-08-24

Read-only architecture and product audit of the public `ColeMurray/background-agents` repository, also published as Open-Inspect. The local reference clone lives at `~/work/projects/repos/background-agents`. Nothing in the tree was modified, built, or executed. No Open-Inspect account or cloud resources were provisioned. The MIT license permits study but not blind code reuse.

Episode calibration: this audit focuses on the infrastructure and session model, not on the agent model or UI details. It compares Open-Inspect to the Phoenix-era `openagents.com` repository and to the current `openagents` Node/Effect monorepo.

Evidence labels (per [README](./README.md)):

- **`[source]`** — tracked source, docs, manifests, or config at the commit.
- **`[history]`** — Git history at or before the audited commit.
- **`[public]`** — corroborated by a linked public source.
- **`[inferred]`** — reasoned from several observations.
- **`[limitation]`** — a boundary on what this audit can prove.

## TL;DR

Open-Inspect is a single-tenant, Cloudflare-native background coding-agent service. A Next.js web client and three Hono bot Workers feed a Cloudflare Workers control plane; each session lives in its own Durable Object with SQLite state and a WebSocket hub; a Modal Python data plane runs the agent in a Debian container with OpenCode, git, browser automation, and a WebSocket bridge. [source] [docs]

The five most important findings:

1. **The session is the durable unit, and it is pinned to a Cloudflare Durable Object.** SQLite in the DO holds messages, events, participants, artifacts, and the sandbox reference. This is a strong per-session isolation model, but it makes multi-region presence, long-lived replay, and cross-session search dependent on Cloudflare's DO fabric. The Phoenix `openagents.com` application keeps product truth in PostgreSQL and uses LiveView assigns and PubSub as projections; the `openagents` monorepo is moving toward Cloud SQL Postgres and Cloud Storage as the durable authority. [source] [docs]
2. **Modal owns the sandbox, not the control plane.** Image builds, sandbox lifecycle, snapshots, and the in-sandbox agent runtime are delegated to Modal. The control plane treats the data plane as a provider: create, start, snapshot, terminate, delete. This is the same provider boundary the `openagents` `managed-sandbox-contract` describes, but Open-Inspect has one concrete provider (`modal-infra`) while OpenAgents has not yet pinned a single background-sandbox provider. [source] [docs]
3. **The single-tenant security model is explicit and narrow.** All users share one GitHub App; the App's installation scope defines repo access. PR attribution uses the user's OAuth token when available, otherwise falls back to the bot. This matches an internal-tools assumption, not a multi-tenant SaaS. OpenAgents' `openagents.com` Phoenix application treats repository access as a per-account, server-side authorization decision. [source] [docs]
4. **WebSocket streaming is the primary real-time contract.** Clients join a session WebSocket and receive events as the agent runs. This is a clean streaming model, but it couples clients to the DO hibernation lifecycle and to Cloudflare's edge routing. The Phoenix product uses LiveView WebSockets with PostgreSQL-backed event journals; the `openagents` monorepo's sync contracts use a separate sync schema and worker layer. [source] [docs]
5. **The repository is a fast-moving, pre-1.0 monorepo with heavy Cloudflare and Modal coupling.** The version is `0.1.0` and the Terraform, D1 migrations, and Modal image build paths are the real deployment authority. It is a useful pattern donor for a single-tenant background-agent lane, not a drop-in subsystem for the multi-tenant OpenAgents product. [source] [history]

## 1. Snapshot, provenance, and limitations

### 1.1 Exact source identity

| Field | Value |
| --- | --- |
| Public repository | `https://github.com/ColeMurray/background-agents` |
| Local clone | `~/work/projects/repos/background-agents` |
| Audited commit | `703c34109054198b0b07dfec3b7b5700c854e22a` |
| Commit time | 2026-08-23 22:48:54 -0700 |
| Commit subject | `ci: skip checks for documentation-only changes (#1590)` |
| License | MIT |
| Product name | Open-Inspect |
| Version | `0.1.0` (`package.json`) |
| Languages | TypeScript (Node 22+), Python 3.12 |
| Package manager | npm workspaces |
| Primary runtimes | Cloudflare Workers + Durable Objects, Modal, Next.js |

[source]

### 1.2 What was probed and what was not

This audit read tracked source, checked-in docs, `package.json`, `terraform/`, and the `packages/control-plane`, `packages/web`, and `packages/modal-infra` READMEs. It did not execute the web app, control plane, bot Workers, or Modal sandbox. It did not create an Open-Inspect deployment or cloud account. Claims about runtime behavior, performance, and cost are inferred from source and docs, not measured. [limitation]

## 2. Architecture

Open-Inspect is three tiers joined by WebSockets.

```text
Clients (Next.js web, Slack, GitHub, Linear, webhooks)
        |
        v
Control Plane (Cloudflare Workers + Durable Objects)
  - API gateway
  - One Durable Object per session
    - SQLite (messages, events, participants, artifacts, sandbox ref)
    - WebSocket hub with hibernation
    - Event stream
    - GitHub integration
  - D1 (session index, repo metadata, environments, encrypted secrets)
        |
        v
Data Plane (Modal)
  - Debian + Node 22 + Python 3.12 + uv
  - OpenCode CLI
  - agent-browser + headless Chrome
  - Supervisor / bridge WebSocket back to control plane
```

[source] [docs]

### 2.1 Control plane

The control plane is `packages/control-plane`, a Cloudflare Worker. Each session is a Durable Object. The DO stores:

- session state
- participants
- messages
- events
- artifacts
- sandbox reference
- `ws_mapping` for WebSocket clients

D1 stores the session index, repository metadata, environments, automations, image builds, and encrypted secrets. The API includes `POST /sessions`, `POST /sessions/:id/prompt`, `POST /sessions/:id/pr`, `POST /sessions/:id/scm-credentials`, and `WebSocket /sessions/:id/ws`. [source] [docs]

### 2.2 Data plane

The data plane is `packages/modal-infra`, a Python/Modal application. It builds a base image with Debian, git, build-essential, Node 22, pnpm, Bun, Python 3.12 with uv, the OpenCode CLI, and `agent-browser` with headless Chrome. A sandbox runs a supervisor (`entrypoint.py`), an OpenCode server, and a bridge (`bridge.py`) that connects back to the control-plane WebSocket. [source] [docs]

### 2.3 Clients

- `packages/web`: Next.js 16 + React 19, Vercel or Cloudflare Workers (OpenNext).
- `packages/slack-bot`, `packages/github-bot`, `packages/linear-bot`: Hono Workers that create sessions from external events.
- `packages/shared`: shared types for the TypeScript packages.

[source]

## 3. Comparison to OpenAgents

### 3.1 Durable authority

Open-Inspect: SQLite in a Cloudflare Durable Object is the session's durable store. D1 is the relational index. The session is the primary durable boundary.

Phoenix `openagents.com`: PostgreSQL is the durable authority for product, authorization, work, and deployment state. LiveView socket assigns, PubSub messages, and BEAM processes are projections.

`openagents` Node/Effect monorepo: Cloud SQL Postgres and Cloud Storage are the durable authority. The `sync-schema`/`sync-client`/`sync-worker` packages project bounded subsets to clients. The `managed-sandbox-contract` defines a runtime-neutral durable sandbox lifecycle but does not itself choose a provider.

Implication: Open-Inspect's DO-per-session model is a strong fit for ephemeral, edge-coupled background work. OpenAgents' Postgres-first model is a better fit for cross-session search, long-lived work graphs, and multi-tenant authorization. [inferred]

### 3.2 Compute and sandbox model

Open-Inspect: Modal is the concrete sandbox provider. The control plane calls Modal for create, start, snapshot, terminate, and delete. Modal images, secrets, and snapshots are the data plane's deployment authority.

Phoenix `openagents.com`: BEAM processes and connected computers are the execution surface. There is no equivalent cloud sandbox provider in the Phoenix architecture.

`openagents` monorepo: `managed-sandbox-contract` and `cloud-contract` define a provider-neutral sandbox lifecycle (identity, lease, budget, command, event, receipt, artifact). Pylon is an earning-capable node, not a background coding sandbox. No single provider like Modal is pinned today.

Implication: Open-Inspect is a complete, concrete sandbox integration. OpenAgents has the contract layer but needs a provider adapter to match the Modal integration. Harvest the control-plane/provider split, not the Modal-specific code. [source] [docs]

### 3.3 Tenancy and authorization

Open-Inspect: explicitly single-tenant. One GitHub App handles all git operations; the App's installation scope defines which repositories are reachable. The control plane does not validate per-user repo access before creating a session. Users may sign in with GitHub or Google; PRs use the user's OAuth token when available, otherwise fall back to the bot. [source] [docs]

Phoenix `openagents.com`: multi-tenant. GitHub token storage is per-account and encrypted; repository access is a server-side authorization decision. The forge is the canonical Git remote and GitHub is a mirror.

`openagents` monorepo: OpenAuth, Google Cloud, and the OpenAgents identity model. Provider-account and repository grants are typed and server-side.

Implication: Do not import Open-Inspect's single-tenant auth into OpenAgents' multi-tenant surface. The GitHub App token brokering and git credential helper are useful pattern donors for an internal/team lane, not the public product. [source] [inferred]

### 3.4 Real-time and client contract

Open-Inspect: clients open a WebSocket to the session DO and receive a stream of events as the agent runs. This is the entire real-time contract. Web, Slack, GitHub, and Linear all create or consume through the same control plane.

Phoenix `openagents.com`: LiveView keeps browser state over a WebSocket, but durable event journals in PostgreSQL are the authoritative conversation record. API clients observe the same journal.

`openagents` monorepo: Effect-native sync schema with separate sync client and worker; the browser and worker share typed schemas, not a raw WebSocket. Cloud Run and Cloud Storage are the transport/storage authorities.

Implication: Open-Inspect's WebSocket-per-session is a simple, low-latency choice for a small number of concurrent sessions. OpenAgents' typed sync and Postgres journals are a better fit for durable, searchable, multi-device history. [inferred]

### 3.5 Web framework

Open-Inspect: Next.js 16 / React 19, with an option to deploy to Cloudflare Workers via OpenNext or to Vercel. The control plane and bot Workers are Hono on Cloudflare Workers.

Phoenix `openagents.com`: Phoenix 1.8, LiveView, HEEx, Tailwind, Basecoat. Elixir/OTP is the runtime and BEAM is the execution cluster.

`openagents` monorepo: TanStack Start, Effect, and Effect Native for retained surfaces; Node 24 on Cloud Run; pnpm workspaces. The old Foldkit/Tailwind app is deleted.

Implication: There is no web-framework overlap worth copying. Open-Inspect's web client is a standard Next.js dashboard; OpenAgents has moved past that toward Effect Native. [inferred]

## 4. What to harvest and what to reject

### 4.1 Harvest

- **Concrete background-agent deployment as a provider.** The control-plane/data-plane split and the use of a cloud sandbox provider (Modal) as a black box is a useful reference for the `managed-sandbox-contract` provider adapter work. [inferred]
- **GitHub App token brokering.** The `scm-credentials` endpoint and the in-sandbox git credential helper pattern are a clean way to issue short-lived tokens without storing long-lived secrets in the sandbox. [source]
- **Environment-as-a-named-workspace.** The concept of a saved, reusable repository set with secrets and optional prebuilt images is a useful product primitive. [source] [docs]
- **Single-session WebSocket hub with hibernation.** For a small, single-tenant deployment, the per-session DO + SQLite + WebSocket hub is a compact real-time architecture. [source] [docs]
- **Provider-neutral scheduling of image builds.** The control plane schedules image builds and the data plane only executes short-lived create/start/snapshot/terminate/delete operations. [source] [docs]

### 4.2 Reject

- **Single-tenant auth as the default.** The shared GitHub App and the absence of per-user repo access validation are incompatible with OpenAgents' multi-tenant, account-scoped data-rights model. [source] [docs]
- **Modal as the only authority.** Modal is a vendor-specific data plane. OpenAgents' `cloud-contract` and `managed-sandbox-contract` are designed to allow multiple providers; do not hard-code Modal semantics into product code. [source] [inferred]
- **Cloudflare Durable Objects as the durable product store.** DO SQLite is a session cache, not the cross-session authority. OpenAgents' PostgreSQL receipts and Cloud SQL remain the durable source. [inferred]
- **Unbounded shared GitHub App scope.** The product's security model depends on limiting the App's installation to trusted repos. OpenAgents must enforce per-account, server-side repo grants. [source] [docs]

## 5. Positioning

Open-Inspect is the most complete public single-tenant background coding-agent system in the local reference set. It is a pattern donor, not a code donor: the MIT license permits copying, but the Cloudflare + Modal + Vercel stack is not OpenAgents' chosen runtime. The useful OpenAgents adaptation is a bounded background-sandbox provider adapter and a single-session real-time contract for internal or team-facing lanes, not a wholesale merge of the control plane or auth model.

[source] [inferred] [limitation]
