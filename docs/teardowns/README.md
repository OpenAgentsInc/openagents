# Product Teardowns

This directory holds point-in-time, read-only teardowns of installed products
and commit-pinned open-source implementations that are close architectural
references for OpenAgents. The goal is not feature admiration or pixel copying.
Each teardown separates observed evidence from inference, then asks which
boundaries, workflows, and failure modes should change OpenAgents product
decisions.

## Current set

| Teardown | Subject | Central finding |
| --- | --- | --- |
| [ChatGPT desktop app](./2026-07-10-chatgpt-desktop-app-teardown.md) | OpenAI's ChatGPT/Codex macOS app | A closed agent host on the Owl Chromium/Electron-compat runtime, with the open Rust Codex engine, plugins, skills, computer use, remote control, and ambient screen-memory components |
| [Claude desktop app](./2026-07-10-claude-desktop-app-teardown.md) | Anthropic's Claude macOS app | Stock Electron orchestrating a live/bundled web UI, Claude Code over stdio JSON, MCP/skills, native bridges, computer use, and a hardware-isolated Cowork VM |
| [Claude Code](./2026-07-10-claude-code-teardown.md) | A commit-pinned historical source snapshot of Anthropic's terminal and SDK agent runtime | A local-first agent engine with a React TUI, bidirectional typed control stream, layered authority, durable session/task/worktree recovery, MCP/plugins, and remote supervision—alongside duplicated query ownership and substantial build-matrix complexity |
| [Codex agent runtime](./2026-07-10-codex-agent-runtime-teardown.md) | The commit-pinned open-source Codex CLI, TUI, app-server, SDK, sandbox, persistence, subagent, and remote-control system | A Rust Thread/Turn/Item engine whose TUI and rich clients share one generated app-server contract, backed by JSONL plus SQLite, explicit agent graphs, cross-platform permission enforcement, and replay-safe remote control |
| [Codex subagent rendering](./2026-07-10-codex-subagents-rendering-analysis.md) | Codex's persisted agent graph, collaboration protocol, and terminal-versus-desktop presentation | The runtime retains explicit child topology and rich events while a linear TUI flattens them; capable supervision needs a complete navigable graph plus causal inline activity |
| [Claude subagent histories](./2026-07-10-claude-subagents-rendering-analysis.md) | Claude Code's `~/.claude` sidechains, task coordination, and Workflows across versions 2.1.170–2.1.206 | Claude retains full per-agent JSONL histories but leaves ordinary topology implicit in `Agent` results; background agents, mailbox/task coordination, and scripted Workflows evolved over the same sidechain format |
| [OpenAgents subagent design](./2026-07-10-openagents-subagents-design.md) | Cross-surface child-agent projection and the first implemented Desktop interaction | Keep the complete roster and each independent child transcript while placing one exact causal child link in the parent timeline; live Runtime Gateway/Sync parity is the next authority step |
| [OpenCode desktop app](./2026-07-10-opencode-desktop-app-teardown.md) | The open-source OpenCode Electron desktop, shared Solid app, and embedded Effect server | A sandboxed local renderer uses generated HTTP/SSE/WebSocket contracts to drive an authenticated utility-process sidecar; files, Git, PTY, agents, tools, MCP, plugins, providers, and SQLite state remain server-owned |
| [OpenCode V2 architecture](./2026-07-10-opencode-v2-architecture-teardown.md) | The OpenCode 2.0 beta docs and commit-pinned V2 engine, client, service, persistence, plugin, and TUI source | V2 separates durable admission, safe-boundary promotion, process-local execution, durable replay, current projections, and volatile events while making WorkContext services and runtime generations explicit; Desktop still embeds V1 |
| [OpenCode Effect architecture](./2026-07-10-opencode-effect-architecture-teardown.md) | Commit-pinned V1 and V2 service, Layer, Schema, HTTP, persistence, concurrency, plugin, observability, and test architecture | V1 proves deep brownfield Effect adoption but exposes the cost of ambient context and runtime bridges; V2 answers with an explicit global/Location service graph, canonical Schema identities, scope-owned generations, and one request processor across network and memory transports |
| [Cursor product](./2026-07-11-cursor-product-teardown.md) | Cursor's 195/197-era baseline versus its 2.x–3.x agent-platform pivot, from the recovered `docs/re/cursor` archive, public evidence, and a read-only bundle survey of the installed 3.11.13 | Cursor validated most episode-195 demands, and the 3.11.13 bundle proves the pivot compiled in: a still-stock-Electron VS Code fork (base 1.125.0, tracked upstream) now carrying a 17-extension first-party agent layer, a deny-by-default Seatbelt sandbox binary, a local Rust code indexer, and cloud-only inference—while startup predictability, billing legibility, and model-identity transparency dropped; the open engine, portable sessions with receipts, and usage/identity truth remain unclaimed lanes |
| [Executor architecture](./2026-07-12-executor-architecture-teardown.md) | Commit-pinned Executor integration catalog, code-mode kernel, Apps/custom-tool authoring and publication pipeline, account binding, and local/cloud sandbox runtimes | Executor closes the capability-production loop: authenticated operations become caller-account-parametric handles inside an isolated authored function, and the published function re-enters the same catalog; OpenAgents should adapt that artifact/compiler/broker shape, retain its own semantic selection, policy, containment, workflow, and receipt authority, and consume Executor only as an optional external provider |
| [OpenChamber whole product](./2026-07-12-openchamber-product-teardown.md) | Commit-pinned OpenChamber v1.16.0 web, Electron, mobile, VS Code, workroom, sync, OpenCode, goals, schedules, permissions, voice, relay, and notification system | OpenChamber is the strongest open whole-product reference for a persistent coding workroom and server-owned continuation, but its event-only goal loop and memory-backed scheduler show why persisted metadata is not restart recovery; OpenAgents should harvest the interaction and protocol patterns behind stronger typed authority, leases, evidence, and receipts |
| [T3 Code](./2026-07-13-t3-code-teardown.md) | Commit-pinned pingdotgg/t3code: a provider-neutral control plane wrapping Codex, Claude Code, Cursor, Grok, and OpenCode behind an event-sourced Effect 4 core with web/desktop/mobile clients and a thin Clerk/Cloudflare relay | The closest product-shape competitor to the OpenAgents P0 supervision lane: five harnesses behind one CQRS SQLite core; one React renderer shared by web/Electron plus a separate React Native renderer over a strong shared Effect projection kernel; worktree-parallel threads, hidden-ref checkpoints, and DPoP-scoped environment access — while defaulting execution to `danger-full-access` with no containment, receipts, or session portability; adapt React/Base UI/workbench mechanics underneath an Effect Native renderer and typed hosts, while refusing both the authority posture and duplicated application/design-system topology |
| [T3 Code and OpenAgents Desktop full gap analysis](./2026-07-15-t3-code-openagents-desktop-full-gap-analysis.md) | Dated, revision-pinned implementation comparison across providers, Codex protocol coverage, orchestration, worktrees, Git/forge, remote/mobile control, workbench tools, frontend state and libraries, performance, security, platforms, updates, and release proof | T3 is much broader, while OpenAgents is narrower and stronger at typed authority, explicit queue/steer semantics, local-first identity, guarded revert, and fail-closed release integrity; close the long-lived Codex supervisor and native event plane first, then worktree/workbench depth, remote/mobile portability, platform breadth, and only then additional providers |
| [Codex app-server client support](./2026-07-15-codex-app-server-client-support-analysis.md) | Current Codex app-server versus commit-pinned T3 Code, OpenCode, and the OpenAgents Desktop workroom slice | “Codex support” hides two architectures: T3 is a schema-broad but behavior-narrow app-server adapter, while OpenCode calls the Codex Responses backend from its own engine and consumes no app-server protocol; literal OpenAgents parity starts by replacing its per-turn process with a generated, version-gated long-lived supervisor and lossless native event plane, then completes reverse RPC, account/policy control, lifecycle/repair, ecosystem, host-utility, and experimental-runtime families in dependency order |
| [Crabbox](./2026-07-13-crabbox-teardown.md) | Commit-pinned openclaw/crabbox: a Go CLI plus optional Cloudflare Durable Object or Node/PostgreSQL coordinator that leases runners across 77 provider adapters, syncs the working-tree diff, runs commands over a direct SSH data plane, and records durable run evidence | The first audited product on the execution-infrastructure seam under the agent era: an honest lease lifecycle (ownership-proof cleanup, reserved-vs-estimated cost, fail-closed expiry), evidence verbs (`attach`/`events`/`logs`/`results`), a credential-destination provenance lattice, and release gates that blocked their own v0.37.0 — but the coordinator holds raw provider credentials, containment is a provider attribute, and the signed run receipt prints `trust=self-signed`; adapt the lifecycle and evidence discipline into the receipted broker-only placement system, never adopt it as a second control plane |
| [Grok Build](./2026-07-15-grok-build-teardown.md) | Commit-pinned xai-org/grok-build terminal, ACP runtime, shared leader, sessions, tools, subagents, worktrees, permissions, sandbox, telemetry, updater, and PTY verification system | The strongest open terminal-host reference in the set: full-screen and native-scrollback renderers, headless and ACP clients, and a process-shared leader all drive one durable local agent platform with deep PTY/race testing; adapt its host, reconnect, rewind, worktree, observability, and coordinated-update mechanics, while rejecting permissive containment defaults, ambient IPC trust, unbounded hot paths, ambiguous durability, and unsigned public release proof |
| [OpenAgents adaptation analysis](./2026-07-10-openagents-product-adaptation-analysis.md) | Cross-teardown synthesis | Combine OpenCode's thin host and V2 durable-admission/scoped-service model, Codex's generated protocol and repairable graph, Claude Code's recovery ergonomics, Grok Build's terminal-host/shared-leader/reconnect discipline, OpenChamber's persistent workroom and cross-surface attention model, and Executor's durable authored-capability loop behind the hardened Effect Native boundary; keep authority typed, isolation explicit, updates signed, and renderer/runtime privilege separate |

## Evidence convention

The dated teardowns use these labels where applicable:

- **`[bundle]`** — observed in the installed signed application bundle
- **`[runtime]`** — observed in live process, UI, network-listener, or
  names-only filesystem state
- **`[source]`** — observed in a commit-pinned source snapshot or reference tree
- **`[test]`** — encoded in source tests, benchmarks, or CI verification
- **`[public]`** — corroborated by a linked public source
- **`[history]`** — supported by commit history or a checked-in decision record
- **`[vision]`** — stated as intended direction but not necessarily present in
  the audited implementation
- **`[schema]`** — encoded directly in a typed settings, event, or wire schema
- **`[inferred]`** — reasoned from multiple observations rather than directly
  asserted by one artifact
- **`[limitation]`** — a boundary on what the available evidence can prove

Bundle metadata and compiled public strings can reveal architecture, but they
do not prove that every dormant feature is enabled or that a remote service
behaves as its client suggests. Runtime observations are snapshots, not ongoing
monitoring. Private credentials, conversation contents, and user-data payloads
do not belong in these documents.

## How to use these documents

Treat the teardowns as design evidence, not current OpenAgents status. The
authorities for implementation state and sequencing remain:

- [Sol master roadmap](../sol/MASTER_ROADMAP.md)
- [OpenAgents Desktop guarantees](../../apps/openagents-desktop/GUARANTEES.md)
- [OpenAgents Desktop README](../../apps/openagents-desktop/README.md)
- current code, tests, receipts, issues, and runtime evidence

When a teardown lesson becomes a product requirement, move it into the owning
typed contract, roadmap gate, issue, and verification surface. Do not leave a
load-bearing decision only in competitive analysis.

## Derived product pathways

- [Remote-first portable coding sessions](../sol/2026-07-11-remote-first-portable-coding-sessions-pathway.md)
  promotes the local/remote protocol, stable-runtime identity, isolation, and
  mobile-control lessons into a concrete gap analysis and ordered pathway for
  host-to-host session movement, owner or managed cloud targets, brokered
  capabilities, an any-host mobile directory, and session-neutral
  conversational voice. The dated teardowns remain evidence; the Sol roadmap
  and executable behavior contracts own the requirement. The
  [Cursor teardown](./2026-07-11-cursor-product-teardown.md) adds market
  corroboration: Cursor's CLI cloud handoff and mobile Remote Control prove
  demand for exactly this seam while stopping short of host-portable identity,
  authority, and receipts.

## Transcript-driven product calibration

- [Episode 248](../transcripts/248.md) turns predictable recent-work discovery
  into an executable release promise: shell and metadata first, stable names
  and ordering, no blank startup or permanent loading state, and real-host
  proof. The later loss-accounted v2 contract strengthens the rolling-24-hour
  entry point into recent-first bounded disclosure plus explicit paging
  with no age ceiling; it does not silently keep the old limit.
- [Episode 249](../transcripts/249.md) turns persisted agent topology into an
  operating model: a complete roster, causal inline child activity, direct
  access to independent child transcripts, and fast pointer/keyboard paths
  through the same typed intents. Historical rendering is landed; equivalent
  live Sync supervision and portable graph preservation remain roadmap work.

These transcripts calibrate product behavior; they do not replace teardown
evidence, typed authority, executable contracts, or receipts.
