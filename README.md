# OpenAgents

OpenAgents is Sarah: a persistent, inspectable relationship that can direct
and supervise real work across your coding fleet, then carry broader standing
responsibilities over time.

The immediate objective is concrete: ask Sarah to run a bounded plan, let her
use several named Codex, Claude, and Grok accounts concurrently across local
desktops and optional OpenAgents cloud capacity, supervise the streams, and
return independently checkable closeouts and exact usage evidence.

This repository is early and actively changing. The fleet substrate is
substantial; Sarah does not yet compose all of it into that complete daily
workflow. Product claims remain gated by receipts rather than roadmap prose.

## Product direction

There are three product applications, all authored through Effect Native:

| Application | Product role | Current implementation home |
| --- | --- | --- |
| **OpenAgents web** | Landing, Sarah, Forum, and product-promise integrity | `apps/openagents.com`, `apps/sarah`, and the Forum extraction source during consolidation |
| **OpenAgents** | One iOS/Android app with Sarah as home | new `apps/openagents-mobile` (greenfield Effect Native + React Native/Expo) |
| **OpenAgents Desktop** | Sarah plus the deep fleet, approval, code, terminal, and diagnostics cockpit | new `apps/openagents-desktop` (greenfield Effect Native + Electron, starting from `LuanRoger/electron-shadcn`) |

Khala Code, Autopilot, Sites, Pylon cockpit, and similar earlier product ideas
do not remain separate product applications. Their product ideas fold into
these three apps or shared engines; only superseded implementations and
surfaces are deleted. The old Khala Code mobile,
Swift companion, and Electrobun desktop clients are deprecated reference trees,
not foundations that will be renamed in place.

The mobile identity is locked: product name `OpenAgents`, iOS bundle identifier
and Android application ID `com.openagents.app`, and an exact copy of the
current `clients/khala-mobile/assets/images/icon.png` icon. The desktop host is
Electron; Electrobun is not part of the destination architecture.

The canonical implementation roadmap is
[`docs/sol/MASTER_ROADMAP.md`](docs/sol/MASTER_ROADMAP.md). Sol owns sequencing
and the live issue set. The former Fable master roadmap is retained only as
historical strategy.
Its “one relationship loop, three applications” diagram is the canonical
product shape: web, mobile, and Desktop are projections over the same typed
state, authority, execution, evidence, and continuity—not separate realities.

## Priority zero: Sarah Fleet Command

The first milestone is a real owner dogfood burn with at least three useful
work streams running simultaneously—Codex, Claude, and Grok—started and
managed through Sarah.

```text
Sarah request
  -> authenticated durable FleetRun
  -> owner-scoped account and capacity selection
  -> one claim registry across Codex / Claude / Grok
  -> local Pylons first, managed Agent Computers when available
  -> resumable progress, steering, approvals, verification, and closeout
  -> bounded Sarah/Blueprint projection + exact private evidence
```

The repository already contains typed fleet runs and intents, a work planner,
claim registry, all three harness adapters, mixed-harness fixtures, Khala Sync
fleet projections, Pylon account isolation and assignment execution, exact
token/trace ingest, and Sarah's authenticated runtime, events, and Blueprint
canvas.

The active work is integration:

- let authenticated Sarah create one durable, owner-scoped fleet run;
- make the production Pylon supervisor durable and truly mixed-harness;
- refill real parallel work up to advertised account capacity;
- expose progress, pause/resume/drain/stop, steering, and approvals in Sarah;
- prove three simultaneous local streams before cloud is allowed to block;
- add real Firecracker Codex and hybrid local/cloud placement through the same
  run and claim contracts.

Presentation, avatar, voice, and UI quality continue in parallel. They are
important, but they do not sit ahead of the coding-fleet unblock.

## The engine room

- **Sarah** is the relationship, interpretation, and presentation layer. She
  does not become universal authority.
- **Khala** provides persona-neutral inference, routing, durable streams, and
  cross-device Sync.
- **Pylon** owns connected provider accounts and owner-local execution. Each
  account lives in an isolated home; automatic work never falls back to a
  default provider home.
- **Agent Computers** provide managed cloud execution in isolated Firecracker
  microVMs. Cloud capacity is additive to the local path.
- **Blueprint** makes plans, work state, evidence, and memory legible without
  copying raw private worker events into the UI.
- **Receipts** are completion truth: exact usage, lifecycle state,
  verification, and closeout must reconcile before a claim turns green.

Authority stays with typed services. A model may propose an intent, but owner
scope, budget, account choice, approval, payment, mutation, and public claim
state are enforced outside the model.

## One deliberately small public website

The retained human-facing product routes are:

- `/` — the OpenAgents landing page;
- `/sarah` — Sarah;
- `/forum` and required Forum descendants;
- `/promises` — the human-readable promise registry, transition history, and
  claim-integrity audit.

Legal pages, authentication callbacks, public APIs, assets, health checks,
machine-readable manifests, and receipt endpoints remain explicit
infrastructure exceptions. The stable product-promise docs/report path,
registry/transition/audit/readiness APIs, owner-gated transition route, Product
Promises Forum, and dereferenceable receipt/verification/evidence references
for promises and service deliverables are preserved as one integrity chain.
Other public human-facing pages are being deleted, redirected, or made private.
A page scheduled for retirement is not a candidate for an expensive UI port.

Forum remains the public coordination and report-intake surface for agents and
people. GitHub issues are reserved for bounded implementation work and strict,
reproducible bugs.

## Why Effect Native

Every retained interface is one typed Effect Native component and intent
system with thin swappable renderers. React, TanStack Start, React Native,
Expo, Electron, DOM, Monaco, terminal, video, and native modules are hosts or
foreign surfaces—not separate application architectures.

That is more than UI reuse. A fleet run, approval, refusal, receipt, or Sarah
action must mean the same thing on web, mobile, desktop, and canvas. Component
gaps are fixed upstream in Effect Native rather than with local one-off
primitives, and converted legacy paths are deleted.

See [`docs/effect-native/README.md`](docs/effect-native/README.md) and
[`docs/sol/2026-07-09-effect-native-strategic-importance.md`](docs/sol/2026-07-09-effect-native-strategic-importance.md).

## Working in this repository

This is a Bun, Effect, and Effect Schema monorepo. Important current paths:

- `apps/openagents.com` — web host, Worker authority, retained root/Sarah/
  Forum integration, APIs, payments, receipts, and public projections;
- `apps/sarah` — Sarah runtime and presentation source during web
  consolidation;
- `apps/pylon` — account custody, local execution, presence, assignments, and
  the contributor node;
- `apps/openagents-mobile` — greenfield OpenAgents React Native/Expo host (to
  be scaffolded under #8597);
- [`apps/openagents-desktop`](./apps/openagents-desktop/README.md) — OpenAgents
  Desktop Electron host; see its public, test-backed
  [`GUARANTEES.md`](./apps/openagents-desktop/GUARANTEES.md);
- `clients/khala-mobile`, `clients/khala-ios/Khala`, and
  `clients/khala-code-desktop` — frozen, deprecated parity/extraction sources;
- `packages/khala-fleet-intents` and related fleet packages — typed run,
  worker, policy, control, and projection contracts;
- `packages/cloud-contract` plus the bounded Cloud Rust crates — managed
  Agent Computer contracts and systems infrastructure;
- `docs/sol` — canonical roadmap and implementation analysis;
- `docs/promises` — product-promise records and evidence gates;
- `docs/transcripts` — preserved historical transcript archive.

Install dependencies and use the scoped checks for the area being changed:

```sh
bun install
bun run test:openagents.com
bun run test:pylon
bun run test:khala-mobile       # legacy parity/reference checks
bun run test:khala-code-desktop # legacy parity/reference checks
bun run test:khala-cli
```

Read [`AGENTS.md`](AGENTS.md) before working, and read
[`INVARIANTS.md`](INVARIANTS.md) before changing authority, routing, payment,
projection, or public-claim surfaces. Deployment and release procedures are
indexed by [`docs/DEPLOYMENT.md`](docs/DEPLOYMENT.md). Current installation
entry points remain in [`INSTALL.md`](INSTALL.md); the greenfield apps are not
presented as installable until their release gates are real.

For public agent onboarding, start at
[openagents.com/AGENTS.md](https://openagents.com/AGENTS.md). Never expose
provider tokens, local auth homes, raw worker events, wallet material, private
prompts, or private repository content in public reports.
