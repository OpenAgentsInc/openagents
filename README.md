# OpenAgents

OpenAgents is reliable software for doing coding work and managing fleets of
agents from Desktop and mobile without forking identity, state, authority, or
receipts.

The immediate priority is a daily-driver coding and fleet loop: start or resume
work, supervise named Codex and Claude capacity, steer and approve bounded
actions, continue the same authoritative conversation across devices, and
receive independently checkable outcomes. Product claims remain gated by
receipts rather than roadmap prose.

The canonical implementation roadmap is
[`docs/sol/MASTER_ROADMAP.md`](docs/sol/MASTER_ROADMAP.md). It owns product
scope, sequencing, issue state, dependency gates, and acceptance criteria.

## Product direction

There are two active product clients over one authority loop:

| Surface | Product role | Implementation home |
| --- | --- | --- |
| **OpenAgents Desktop** | Full coding workbench and fleet cockpit: conversations, projects, files, Git, terminal, runtimes, approvals, diagnostics, and receipts | [`apps/openagents-desktop`](apps/openagents-desktop/README.md) |
| **OpenAgents mobile** | Compact coding and fleet control: cross-device continuation, remote workrooms, review, safe writeback, approvals, and handoff | `apps/openagents-mobile` |

`apps/openagents.com` remains the supported public, authentication, API,
Khala Sync, Forum, product-promise, payout, and operational surface. It is not
a third active client or a broad product-expansion lane during the current
Desktop/mobile program.

Earlier Khala Code clients are frozen extraction sources, not foundations to
rename in place. Useful capabilities are ported into the active apps over the
shared contracts, and superseded product surfaces are retired. The mobile
identity is locked to the product name `OpenAgents`, iOS bundle identifier and
Android application ID `com.openagents.app`, and the existing OpenAgents icon.
The Desktop host is Electron.

## Priority zero: reliable Desktop/mobile coding and fleet control

```text
typed user intent
  -> policy and approval boundary
  -> authoritative conversation, FleetRun, or workroom command
  -> Pylon owner-local execution or managed Agent Computer
  -> Codex / Claude execution
  -> evidence, durable outcome, and receipt
  -> Khala Sync
  -> the same refs, versions, and state on Desktop and mobile
```

The repository already contains a hardened Electron/Effect Native Desktop
shell, a React Native/Expo mobile shell, native credential custody, host-owned
Khala Sync SQLite stores, canonical confirmed conversation continuity, a
tokenless Runtime Gateway, typed FleetRun and control authority, isolated
provider accounts, and receipt/evidence contracts.

The shortest path is to project and mutate that existing authority through
Khala Sync. The active work is to bind real streamed provider execution into
the canonical conversation, complete the Desktop workbench and visible fleet
cockpit, bring remote-workroom coding and fleet control to mobile, prove
interruption and restart recovery, and pass physical-device, packaging, and
owner-dogfood gates.

The current implementation ledger, cross-device reliability gates R0–R7, and
Desktop D0–D6 dependency graph live in the
[`Sol master roadmap`](docs/sol/MASTER_ROADMAP.md). Supporting current-state
evidence lives in [`docs/terra/CURRENT_STATE.md`](docs/terra/CURRENT_STATE.md)
and [`docs/terra/MOBILE_PARITY.md`](docs/terra/MOBILE_PARITY.md).

## Authority and engine room

- **Khala** provides persona-neutral inference, routing, durable streams, and
  cross-device Sync.
- **Pylon** owns connected provider accounts and owner-local execution. Each
  account lives in an isolated home; automatic work never falls back to an
  ambient default provider home.
- **Agent Computers** provide optional managed execution in isolated
  Firecracker microVMs. Cloud capacity is additive to the local path.
- **Blueprint** makes plans, work state, evidence, and memory legible without
  copying raw private worker events into public or renderer-visible state.
- **Receipts** are completion truth: usage, lifecycle state, verification, and
  closeout must reconcile before a claim turns green.

Typed services retain authority. A model may propose an intent, but owner
scope, budget, account choice, approval, payment, mutation, and public claim
state are enforced outside the model. Local caches and optimistic UI never
become execution authority, and a transport timeout is not success.

## Why Effect Native

The active clients use one typed Effect Native component and intent system
with thin swappable renderers. React Native, Expo, Electron, DOM, Monaco,
terminal, canvas, and native modules are hosts or foreign surfaces rather than
separate application architectures.

That is more than UI reuse: a run, approval, refusal, command outcome, or
receipt must mean the same thing on Desktop and mobile. Shared gaps are fixed
upstream in Effect Native instead of with local one-off primitives.

See [`docs/effect-native/README.md`](docs/effect-native/README.md) and
[`docs/sol/2026-07-09-effect-native-strategic-importance.md`](docs/sol/2026-07-09-effect-native-strategic-importance.md).

## Working in this repository

This is a Bun, Effect, and Effect Schema monorepo. Important current paths:

- `apps/openagents-desktop` — Electron host, tokenless Runtime Gateway,
  coding workbench, and fleet cockpit;
- `apps/openagents-mobile` — React Native/Expo host for mobile coding,
  continuity, and fleet control;
- `apps/openagents.com` — public web host, authentication and Worker
  authority, Sync service, APIs, payments, receipts, Forum, and public
  projections;
- `apps/pylon` — account custody, local execution, presence, assignments, and
  contributor node;
- `packages/khala-fleet-intents` and related fleet packages — typed run,
  worker, policy, control, and projection contracts;
- `packages/khala-sync-client` and `packages/khala-sync-server` — shared
  cross-device projection, mutation, replay, and convergence paths;
- `packages/cloud-contract` plus the bounded Cloud Rust crates — managed Agent
  Computer contracts and systems infrastructure;
- `clients/khala-mobile`, `clients/khala-ios/Khala`, and
  `clients/khala-code-desktop` — frozen parity and extraction sources;
- `docs/sol` — canonical roadmap and implementation analysis;
- `docs/teardowns` — desktop product teardowns and adaptation lessons;
- `docs/promises` — product-promise records and evidence gates;
- `docs/transcripts` — preserved historical transcript archive.

Install dependencies and run scoped checks for the area being changed:

```sh
bun install --frozen-lockfile
bun run test:openagents-desktop
bun run test:openagents-mobile
bun run test:openagents.com
bun run test:pylon
bun run test:khala-cli
```

Read [`AGENTS.md`](AGENTS.md) before working, and read
[`INVARIANTS.md`](INVARIANTS.md) before changing authority, routing, payment,
projection, or public-claim surfaces. Deployment and release procedures are
indexed by [`docs/DEPLOYMENT.md`](docs/DEPLOYMENT.md), and installation entry
points live in [`INSTALL.md`](INSTALL.md).

For public agent onboarding, start at
[openagents.com/AGENTS.md](https://openagents.com/AGENTS.md). Never expose
provider tokens, local auth homes, raw worker events, wallet material, private
prompts, or private repository content in public reports.
