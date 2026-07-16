# OpenAgents

## The Agent IDE

**OpenAgents ships The Agent IDE: the integrated development environment for
building software with agents.** It is the place where developers start work,
direct agents, inspect what they are doing, resolve blockers, review repository
effects, and return to durable sessions.

An agent engine can reason, call tools, and change code. That execution loop is
necessary, but it is not the whole product. OpenAgents owns the working
environment around the loop: conversations, project context, agent and
subagent topology, controls, history, review, recovery, and evidence.

> Models are workers. OpenAgents keeps the work.

The Agent IDE is also OpenAgents' human front door to the broader open agent
network being bootstrapped. The IDE gives a person one coherent cockpit for
intent, authority, supervision, and review; the network direction is intended
to connect that cockpit to a growing ecology of agents, models, tools, compute,
and contributors beyond any one lab.

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

## Protocol interoperability

Agent Client Protocol (ACP) client support is an explicit, bounded architecture
with a peer-specific Desktop control surface. It lets
OpenAgents control Grok through `grok agent stdio`, Cursor through `agent acp`,
and other compatible coding agents over bidirectional JSON-RPC/stdio at the
typed runtime boundary.

The adapter starts from current stable wire version 1 and the current schema
artifact, then capability-gates optional methods and keeps provider-specific
authentication, launch, and extensions in thin profiles. ACP does not replace
OpenAgents Thread/Turn/Item/Work Unit, Runtime Interaction, authority,
evidence, or Receipt models, and protocol support alone does not make every
registry agent a shipped provider. This direction is client-only: controlling
external coding agents is the requirement.

The source audit and ordered implementation gates are in the
[T3 Code Agent Client Protocol implementation teardown](docs/teardowns/2026-07-16-t3-code-agent-client-protocol-implementation-teardown.md);
the pinned protocol authority, bounded stdio transport, and hermetic 23-method
[conformance package](packages/agent-client-protocol-conformance/README.md) are
now implemented. The shared [runtime bridge contract](docs/adr/2026-07-16-agent-client-runtime-bridge.md)
adds private native evidence, canonical projection, false-by-default reverse
authority, interactions, and refs-only receipts. The shared [session runtime](docs/adr/2026-07-16-agent-client-session-runtime.md)
adds single-flight startup, capability-gated lifecycle calls, serialized prompts,
accepted-frame draining, replay/live fencing, idempotent cancellation, and bounded
recovery. The Grok and Cursor peer composers now add exact executable admission,
real-workspace binding, advertised authentication, provider-isolated extensions,
and digest-bound feature gates. Cursor's composer launches only a resolved
`cursor-agent` installation through `agent acp`, pins and rechecks its complete
flat installation closure, negotiates `cursor_login`, and
keeps model discovery behind its versioned extension decoder. Diagnostic live
probes and source-derived fixtures remain explicitly non-release evidence. A
checked opt-in production runner now reproduces a redacted Grok/Cursor candidate
receipt in disposable repositories; it has no claim authority. The checked
[pinned live-binary matrix](packages/agent-client-protocol-conformance/compatibility/release-matrix.json)
and [human proof ledger](docs/qa/2026-07-16-acp10-release-proof/README.md) now
independently gate the two peers; their current verdict is experimental because
required live scenarios remain unresolved. Execution is tracked in
[#8887 — Full Agent Client Protocol integration for Grok and Cursor](https://github.com/OpenAgentsInc/openagents/issues/8887);
the wider reference set remains indexed in
[Product Teardowns](docs/teardowns/README.md).

Desktop Settings now schema-decodes a main-owned Grok/Cursor projection with
probe-verified executable identity, validated alternate executable selection,
advertised authentication state, session/cancellation/recovery state, and
stable-versus-extension configuration provenance. Grok uses the existing local
cached-token session for headless ACP by default—no API key is required. An
intentionally supplied `XAI_API_KEY` remains an optional peer-advertised path;
Cursor exposes only its advertised `cursor_login` flow. The
support artifact is constructed in main from a closed refs-only schema and
omits executable paths, environment, auth payloads, prompts, files, terminal
content, and native events. Desktop derives its label from trusted-profile
admission evidence; the pinned matrix remains the release authority and is not
loaded by the renderer. The checked Grok and Cursor builds remain visibly
experimental, and evidence for one can never promote the other.

## The front door to an open agent network

The long-term OpenAgents thesis is not one better assistant. It is an open,
group-forming network where independently owned agents and people can discover
one another, assemble temporary teams around a goal, divide work, verify the
result, and recombine for the next task without one model vendor owning the
graph.

The Agent IDE makes that network useful and legible to a human. It is where an
operator supplies goals, repositories, judgment, and boundaries; sees which
agents and tools did what; intervenes when needed; and decides whether the
result counts. In the other direction, the network can make the IDE more
capable than any single agent: broader specialists, tools, compute, review, and
eventually reusable capabilities can meet behind one durable workroom.

The bootstrap loop is bidirectional: the IDE creates real demand and
verifiable outcomes; the network can return a wider supply of agents, tools,
capacity, and reusable capabilities.

The atomic unit is an **accepted outcome**, not a token, model response, or
claim of completion. Work must be scoped, executed, checked, and recorded with
evidence before unfamiliar participants can compose without trusting one
another blindly. Over time, that same verification layer can let value flow
back to the people who contribute useful skills, compute, data, review, and
verification.

Any future multiplayer participation must remain opt-in and owner-controlled.
Independent workrooms would share nothing by default; participation would
require explicit grants, bounded authority, visibility rules, and evidence
that other participants can inspect without receiving private prompts,
credentials, or repository data.

“OpenAgents” therefore names both the product and the larger project:

| Layer        | Meaning                                                                                                                                                                        |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Product**  | The Agent IDE: OpenAgents Desktop today and the retained OpenAgents mobile companion                                                                                           |
| **Platform** | The typed runtime, workspace capabilities, Effect Native, receipts, and broader retained Sync, Pylon, and Agent Computer substrate; not all of it is required by the local MVP |
| **Network**  | The intended open coordination topology being bootstrapped behind the Agent IDE: agents, tools, compute, people, verification, and accepted outcomes                           |
| **Lab**      | Research, evaluation, and open-model work that improves the available workers while keeping the Agent IDE neutral among them                                                   |

This network is a direction being bootstrapped, not a claim that every routing,
market, settlement, distributed-training, or multiplayer path ships in the
current product. The accepted Desktop MVP proves the local control surface
first; broader paths retain their own owner decisions and evidence gates.

The project thesis begins in
[`Episode 200: The Agent Network`](docs/transcripts/200.md), while
[`Episode 237`](docs/transcripts/237.md) connects the human cockpit to the
network and names the accepted outcome as its economic unit. The
[`transcript theme guide`](docs/transcripts/README.md) traces how that network
thesis narrowed into the current Desktop product without disappearing.

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
