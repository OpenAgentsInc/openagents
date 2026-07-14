# OpenAgents

## The Agent IDE

**OpenAgents is The Agent IDE: the integrated development environment for
building software with agents.** It is the place where developers start work,
direct agents, inspect what they are doing, resolve blockers, review repository
effects, and return to durable sessions.

An agent engine can reason, call tools, and change code. That execution loop is
necessary, but it is not the whole product. OpenAgents owns the working
environment around the loop: conversations, project context, agent and
subagent topology, controls, history, review, recovery, and evidence.

> The model is a worker. OpenAgents is where the work lives.

## What makes it an Agent IDE

A conventional IDE integrates an editor, language tools, a debugger, source
control, and a runtime. The Agent IDE integrates the corresponding objects of
agent work:

| Development concern | OpenAgents responsibility                                                                  |
| ------------------- | ------------------------------------------------------------------------------------------ |
| Intent and context  | Durable conversations, repository grants, and explicit work state                          |
| Execution           | Typed agent turns, tools, plans, usage, and terminal outcomes                              |
| Multi-agent work    | Complete parent/child topology, causal activity, and independent transcripts               |
| Supervision         | Stop, steer, queue, question, approval, refusal, and honest blocker states                 |
| Review              | Bounded files, Git status and diffs, and evidence beside the conversation                  |
| Continuity          | Findable history, exact retry reconciliation, reload and restart recovery, and diagnostics |

Conversation is the default surface; repository context, the agent graph,
review, and diagnostics open around the work when needed. This is not a code
editor with a chat panel, and it is not another model provider. Many tools put
an agent inside an IDE; OpenAgents makes the agents, their work, and their
evidence the IDE.

## One product, Desktop first

The OpenAgents product identity spans Desktop and mobile. Desktop is The Agent
IDE today; mobile is the retained compact companion. The accepted first product
shape is deliberately narrower:

| App                    | Current role                                                                                                                                                                   |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **OpenAgents Desktop** | The primary local-first Agent IDE: a signed workroom around the user's ordinary logged-in Codex session, usable without an OpenAgents account                                  |
| **OpenAgents mobile**  | The retained companion built on the same product identity and typed application model; physical distribution and broader remote-coding claims remain separately evidence-gated |

Codex is the first engine. Codex owns its model, agent, and tool loop;
OpenAgents owns the durable product around it. The current Desktop MVP keeps
its visible path intentionally small—New Chat, Chat, Project Home, and
Settings—while making sessions, typed activity, child agents, bounded
repository review, controls, recovery, updates, and diagnostics coherent.

ProductSpec and AssuranceSpec remain underlying authoring and verification
tooling. They are not user-facing destinations in the MVP. Fleet, broad
provider parity, managed targets, portable host movement, and full mobile
coding are closed `not planned`; retained architecture does not restore their
product status without a new bounded owner decision and issue.

The exact current scope and proof boundary live in the
[`MVP package`](docs/mvp/README.md) and
[`Sol master roadmap`](docs/sol/MASTER_ROADMAP.md). An accepted or signed
release candidate is not the same as a published release; public availability
and capability language remain gated by the
[`promise registry`](docs/promises/README.md).

## Open at the control points

OpenAgents is designed so that a frontier model can be useful without becoming
the owner of the application, the user's identity, private knowledge, tools,
or history. The current product is Codex-first, while the architectural rule is
that workers remain behind an inspectable, typed OpenAgents boundary.

- **Local-first authority:** useful Desktop work does not require an
  OpenAgents account or hosted control plane.
- **Inspectable execution:** typed commands, agent identity, causal activity,
  explicit failure states, and bounded evidence replace assistant theater.
- **Durable user-owned work:** provider identifiers and local paths do not
  become product identity, and a transport timeout never becomes success.
- **Replaceable workers:** models, runtimes, tools, and compute can evolve
  without taking the workroom or its history with them.
- **Evidence before claims:** code, tests, packaged artifacts, live journeys,
  owner acceptance, publication, and public promises remain distinct proof
  rungs.

The longer argument for this boundary is in
[`The Case Against Anthropic`](docs/sol/the-case-against-anthropic.md). The
point is institutional rather than model-specific: intelligence should be
plural, execution inspectable, history portable, and private knowledge under
the user's control.

## Product first, broader mission around it

“OpenAgents” names both the product and the larger project. The distinction is
useful:

| Layer        | Meaning                                                                                                                                                                                |
| ------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Product**  | The Agent IDE: OpenAgents Desktop today and the retained OpenAgents mobile companion                                                                                                   |
| **Platform** | The typed runtime, workspace capabilities, Effect Native, receipts, and broader retained Sync, Pylon, and Agent Computer substrate; not all of it is required by the current local MVP |
| **Network**  | The longer-term open topology in which agents, tools, compute, and contributors can be discovered and combined without one model vendor owning the system                              |
| **Lab**      | Research, evaluation, and open-model work that improves the available workers while keeping the Agent IDE neutral among them                                                           |

The platform, network, and lab explain how the product can grow. They do not
replace the simple user-facing answer: **OpenAgents is the Agent IDE.**

## Working in this repository

This is a Node.js, pnpm, Vite Plus, Effect, and Effect Schema monorepo. The
current toolchain is pinned in [`package.json`](package.json).

- [`apps/openagents-desktop`](apps/openagents-desktop/README.md) — Electron
  host, tokenless Runtime Gateway, and the primary Agent IDE;
- [`apps/openagents-mobile`](apps/openagents-mobile/README.md) — React
  Native/Expo host for the compact OpenAgents client;
- [`apps/openagents.com`](apps/openagents.com/README.md) — public, auth, API,
  Sync, promise, receipt, health, legal, and operations surfaces;
- [`apps/pylon`](apps/pylon) — account custody and owner-local execution;
- [`packages`](packages) — shared schemas, runtime, Sync, UI, evidence, and
  infrastructure contracts;
- [`docs/mvp`](docs/mvp/README.md) — exact first-product definition and
  release-candidate evidence;
- [`docs/sol`](docs/sol/README.md) — canonical product direction, roadmap,
  decisions, and implementation evidence;
- [`docs/effect-native`](docs/effect-native/README.md) — the shared typed
  application and renderer architecture.

Use Node `24.13.1` and pnpm `11.10.0`, then run checks scoped to the area being
changed:

```sh
pnpm install --frozen-lockfile
pnpm run test:openagents-desktop
pnpm run test:openagents-mobile
pnpm run test:openagents.com
pnpm run check:fast
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
