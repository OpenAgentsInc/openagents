# What OpenAgents Should Adapt from the Desktop and Agent-Runtime Teardowns

Date: 2026-07-10

Status: product and architecture decision input

Evidence base:

- [ChatGPT / Codex desktop teardown](./2026-07-10-chatgpt-desktop-app-teardown.md)
- [Claude desktop teardown](./2026-07-10-claude-desktop-app-teardown.md)
- [Claude Code architecture teardown](./2026-07-10-claude-code-teardown.md)
- [Codex CLI and agent runtime teardown](./2026-07-10-codex-agent-runtime-teardown.md)
- [OpenCode desktop source teardown](./2026-07-10-opencode-desktop-app-teardown.md)
- [OpenCode V2 architecture teardown](./2026-07-10-opencode-v2-architecture-teardown.md)
- [OpenCode Effect architecture teardown](./2026-07-10-opencode-effect-architecture-teardown.md)
- [Executor architecture teardown](./2026-07-12-executor-architecture-teardown.md)
- [T3 Code teardown](./2026-07-13-t3-code-teardown.md)
- [Crabbox teardown](./2026-07-13-crabbox-teardown.md)
- [Grok Build teardown](./2026-07-15-grok-build-teardown.md)
- [Command Code teardown](./2026-07-16-command-code-teardown.md)
- [Factory Desktop and Droid CLI teardown](./2026-07-16-factory-desktop-cli-teardown.md)
- [Amp Code teardown](./2026-07-16-amp-code-teardown.md)
- [Open Interpreter harness-emulation teardown](./2026-07-18-open-interpreter-teardown.md)
- [Sol master roadmap](../sol/MASTER_ROADMAP.md), especially Desktop D0–D6
- [OpenAgents Desktop enforced guarantees](../../apps/openagents-desktop/GUARANTEES.md)

## Executive decision

OpenAgents should adapt the **shape** all three desktop products converge on,
while rejecting their least transparent trust choices. OpenCode exposes the
desktop/server implementation of that shape under an MIT license; Codex exposes
the local engine, rich-client protocol, durable state, sandbox, subagent graph,
and remote-control implementation under Apache-2.0. Together they replace a
large amount of bundle inference with inspectable load-bearing seams.

The convergent shape is clear:

1. a web-capable desktop shell that is not itself the agent engine;
2. a real agent engine outside the renderer;
3. a versioned query/command/event seam between UI and engine;
4. local tools, MCP, skills, plugins, and native capabilities;
5. separate isolation for untrusted code execution;
6. desktop/mobile continuity and remote steering;
7. independently updateable components; and
8. a workbench that grows beyond chat without making every conversation look
   like an IDE.

OpenAgents already chose the correct foundation: stock Electron as a hardened
host, Effect Native as the application and intent grammar, Khala Sync for
cross-device continuity, Pylon for execution, and Blueprint for legible work
state, governance, and receipts.
The teardowns strengthen that decision. OpenCode further shows that the durable
architecture is not “Electron”; it is a local/remote runtime protocol with a
desktop client. OpenCode V2 makes the next consequence explicit: accepting
input, executing it, replaying its durable facts, projecting current state, and
streaming transient UI updates are separate contracts. Claude Code makes the
engine consequence sharper: the protocol must be bidirectional, the local
executor must remain authoritative, and every surface must consume one
conversation/task state machine rather than own a parallel query loop. Codex
then demonstrates that migration in production
source: its TUI now uses the same app-server contract in-process or remotely,
and its former TUI-specific app-server flag is only a removed compatibility
no-op. The evidence does **not** justify an Owl-style runtime fork, a
live remote site as the privileged renderer, opaque sidecar sprawl, renderer-
held general runtime authority, or ambient screen recording by default.

The product-level adaptation is:

> Build one quiet OpenAgents work surface whose typed state can deepen into a
> coding workbench and Fleet cockpit; keep authority in explicit host/runtime
> services; make every engine, plugin, sandbox, and remote-control transition
> inspectable and receipt-backed.

## What the three products prove together

| Question | ChatGPT / Codex | Claude | OpenCode | OpenAgents conclusion |
| --- | --- | --- | --- | --- |
| Can Electron support a serious agentic desktop? | Yes, through an Electron-compatible first-party Chromium fork | Yes, on stock Electron with native modules and a VM | Yes, on stock Electron with an open utility-process server and shared local app | Stay on stock Electron; capability does not require a browser fork |
| Where should the agent engine live? | Rust `codex app-server` child over stdio JSON-RPC | Claude Agent SDK spawning Claude Code over stdio `stream-json` | Embedded Effect server in an Electron utility process, addressed through HTTP/SSE/WebSocket | One shared engine behind a versioned typed protocol; transport follows lifecycle and stream needs |
| How should risky code run? | Codex Seatbelt/Landlock command sandbox and policy engine | Linux guest under Apple Virtualization/Hyper-V for local Cowork | Host tools behind permission rules; renderer sandbox is strong, workload isolation is separate | Separate host authority from guest execution; explicit mounts, egress, policy, and receipts |
| How should tools extend the product? | Plugin directories, MCP manifests, skills, marketplace metadata | MCPB/DXT, MCP hosts, skills, plugins, connectors, hardware bridge | MCP, skills, npm/config plugins, custom tools, provider adapters | Support open MCP/MCPB/plugin compatibility inside a signed, isolated typed catalog |
| How should clients relate? | Remote-control/device-key infrastructure | Dispatch, remote sessions, and desktop-backed local access | The same generated server contract serves desktop, web, CLI, SDK, WSL, SSH, and remote HTTP | Desktop/mobile/web are typed command/projection clients, not pixel mirrors or credential carriers |
| How should UI and host relate? | Large local React SPA on Owl | Remote Claude.ai plus bundled Ion SPA and local shell | Local shared Solid app plus a narrow desktop `Platform` adapter | Ship a local versioned Effect Native renderer; share packages, not live privileged web deployment authority |
| How should workbench capabilities be exposed? | Renderer calls the app-server and native bridges | Web UI, CLI sidecar, native services, and VM planes | Files, Git, review, PTY, sessions, providers, and tools are server APIs; IPC stays desktop-specific | Keep workbench authority behind runtime services; keep preload small and schema-validated |
| What does the update problem become? | App, Owl, Codex, plugins, skills, computer-use services | App, live/bundled Ion, Agent SDK, CLI, extensions, VM rootfs | App bundles host/server/renderer, while plugins, MCP, skills, and remote servers still move independently | One signed component compatibility ledger with rollback proof |
| What is the trust risk? | Unsandboxed automation host plus computer use and ambient memory | Unsandboxed host plus broad native/VM/browser/file reach | Renderer receives local-server credentials; plugins run in the trusted server; host tools are not guest isolation | Make credentials, permissions, capture, execution, memory, extensions, and remote control explicit, narrow, and auditable |

## What the OpenCode source audit changes

ChatGPT and Claude establish architectural plausibility. OpenCode supplies an
inspectable implementation and therefore turns several recommendations into
specific engineering requirements:

- **Local and remote are one protocol.** The built-in sidecar, WSL, SSH proxy,
  and regular HTTP server are connection variants behind stable identities.
- **Desktop IPC is not the engine API.** IPC handles windows, pickers, storage,
  update, logs, menus, and OS integration; the generated server client handles
  files, sessions, tools, Git, PTY, providers, and permissions.
- **Streams need lifecycle semantics.** Eager subscription, connected and
  heartbeat events, disposal, reconnection, coalescing, replay cursors, and
  bounded UI scheduling are visible code, not implementation trivia.
- **Local facts deserve a database.** SQLite WAL, migrations, aggregate event
  sequences, scoped instances, and transactional project/session state replace
  transcript-shaped ad hoc persistence.
- **A renderer command catalog is useful but incomplete.** Stable IDs can unify
  palette, keybindings, slash commands, and native menus; OpenAgents must carry
  those IDs through typed authority checks and durable outcomes.
- **Migration debt is architectural evidence.** OpenCode currently carries
  legacy and next protocols, event bridges, generated clients, layouts, routes,
  and state migrations. OpenAgents should freeze its event algebra early and
  give every compatibility layer an explicit deletion gate.
- **Open extensions still require isolation.** Open source does not make an npm
  plugin safe to run inside the trusted server, nor a host shell a sandbox.

## What the OpenCode V2 source audit changes

The first OpenCode audit proved the thin Electron host and server-owned
workbench. The V2 beta turns the next-generation packages visible in that
snapshot into an explicit replacement architecture and adds requirements that
the earlier desktop analysis could not establish:

- **Durable admission precedes execution.** A prompt or synthetic input is
  recorded with an idempotent identity before an advisory wake. It becomes
  model-visible only through an atomic promotion boundary. OpenAgents command
  acceptance must survive a client disconnect even when execution never
  starts.
- **Mid-run delivery is semantic.** OpenCode V2 distinguishes a steer that
  enters at the next safe boundary from a queue item that waits until current
  work would yield. Desktop and mobile must not infer follow-up behavior from
  arrival timing or spinner state.
- **One stream is not enough.** V2 separates bounded query projections, a
  per-Session durable aggregate log with a synchronization marker, and a
  volatile live event stream that may overflow or miss disconnected events.
  Khala Sync and Runtime Gateway need the same explicit division.
- **Current activity is process-local.** Pending input, projected messages,
  durable execution history, graceful restart intent, and active process
  ownership have different authorities. OpenAgents must not persist one
  timeless “running” field.
- **Work context is a service graph.** A stored Location resolves filesystem,
  tools, permissions, agents, providers, plugins, MCP, instructions, PTY, and
  runner services. OpenAgents should strengthen this into a typed WorkContext
  bound to account, repository, placement, containment, and Blueprint
  program/action state.
- **Embedded is a transport, not a bypass.** V2's SDK uses an in-memory
  HttpClient against the same router, middleware, codecs, handlers, and errors
  as the network client. Every OpenAgents local/remote/test adapter must enter
  the same Effect request processor.
- **Mutable catalogs require generations.** V2 scopes plugin and tool
  registrations, captures the exact tool generation advertised to a model,
  and replays active catalog transforms. OpenAgents extension, provider, MCP,
  and tool updates need the same ownership plus signatures, isolation, and
  receipts.
- **Instructions can be synchronized as typed values.** V2 persists
  content-addressed value deltas and derives privileged rendering at request
  assembly. OpenAgents should synchronize verified typed values, distinguish
  unavailable from removed, encrypt sensitive bodies, and never treat a naked
  local hash as a portable Sync object.
- **Recovery is a collection of honest mechanisms.** Pending input, durable
  log, projections, compaction barriers, staged conversation/file rewind, and
  graceful managed-service restart solve different failures. V2 explicitly
  leaves hard-crash exactly-once provider/tool execution and clustered
  ownership unresolved.
- **Staged rewind belongs in the engine.** Stage, inspect, commit, and clear are
  server operations, while the docs list irreversible effects. OpenAgents
  should add conflict checks, worktree ownership, and checkpoint receipts.
- **A managed runtime is discoverable state.** Endpoint, process, version,
  readiness, registration, restart, and reconnection are modeled rather than
  hidden behind a window. Pylon needs that lifecycle while replacing the
  shared Basic secret with scoped client/device capabilities.
- **Large tool catalogs can be deferred safely.** V2's confined Code Mode
  exposes only captured tools through a budgeted searchable catalog. If
  OpenAgents adopts this, timeout, tool-call, output, spend, and authority
  limits must be mandatory and every nested call must remain receipt-visible.

V2 also supplies important negative evidence. Its shell still has host-user
authority, plugins remain trusted in-process code, subagent permissions can
widen beyond the parent, Code Mode budgets default to unbounded, some accepted
configuration is inert, hard-crash recovery is intentionally incomplete, and
the Electron desktop still embeds the V1 server. OpenAgents should adopt the
state-machine seams, not interpret “V2” as proof that all product surfaces have
completed the migration.

## What the OpenCode Effect source audit changes

The broader V2 analysis established the product state machine. The focused
Effect audit explains how to make that architecture composable and testable:

- **V1 was already deeply Effect-based.** The migration was not
  Promise-to-Effect so much as ambient-to-explicit topology. OpenCode's V1
  runtime shows that individual Effect services can still inherit the
  correctness burden of fiber references, `AsyncLocalStorage`, directory
  caches, multiple managed runtimes, and callback bridges.
- **Service scope is an architectural law.** OpenCode V2 classifies services as
  process-global or Location-scoped and rejects dependency direction that
  would let a global service capture one Location. OpenAgents should publish a
  process, WorkContext, run, request, and foreign-host scope law before adding
  more services.
- **A service graph is valuable when it enables proofs.** OpenCode's custom
  `LayerNode` representation checks dependency completeness, detects cycles,
  validates replacements, enforces scope tags, and hoists global dependencies.
  OpenAgents should begin with ordinary Layers plus architecture tests and add
  a smaller graph IR only if replacement or hoisting requires it.
- **Canonical Schema identity prevents package drift.** Public commands,
  events, projections, authority manifests, and receipts should each have one
  browser-safe Schema value reused by Core, Protocol, clients, Sync, and UI.
  Compatibility contracts belong in explicit V1 namespaces.
- **`ManagedRuntime` belongs at foreign host edges.** It is appropriate for an
  Electron/native callback host, CLI, server, or embedded SDK. It should not be
  the ordinary path by which Effect services call one another.
- **Scope is the extension lifecycle.** Plugins, tools, provider catalogs, MCP,
  and foreign hosts should be generation-owned resources whose registrations,
  fibers, and cleanup end together. A stable ID plus mutable global map is not
  sufficient.
- **Promise compatibility should adapt inward.** OpenCode's Promise plugin API
  captures Effect context and Scope, then enters the canonical Effect runtime.
  OpenAgents should keep Effect-native internals and put `async`/`await`, IPC,
  and provider-SDK bridges at named perimeter modules.
- **Interruption must survive every adapter.** Cancellation is control flow
  that owns cleanup, not an ordinary tool failure. Broad cause conversion in a
  tool, plugin, renderer bridge, or Pylon adapter must not swallow it.
- **Testing is a primary payoff.** Replace graph nodes for filesystem,
  transport, identity, policy, provider, database, and Sync; use deterministic
  time for leases, approvals, retry, debounce, reconnect, and cleanup; and test
  forbidden scope edges and cycles directly.
- **Effect does not provide containment.** Scope-owned in-process plugins are
  easier to clean up but remain trusted code. OpenAgents still needs signed
  provenance, capability narrowing, process/guest isolation, and receipts.
- **Failure policy needs more discipline than `orDie`.** User refusal,
  dependency outage, interruption, invariant defect, and optional telemetry
  failure need different contracts and recovery. OpenCode's frequent
  fatalization is cautionary evidence.
- **Framework risk is program risk.** OpenCode pins an Effect 4 beta and uses
  unstable HTTP, SQL, process, and observability modules. OpenAgents upgrades
  need protocol, resource-finalization, startup, typecheck, and replacement
  regression gates.

This changes the recommended Effect Native foundation from “typed components
over Effect” to an explicit capability-and-lifetime architecture shared by
Desktop, mobile, web, and runtime hosts.

## What the Claude Code source audit changes

The Claude desktop inspection established the host/CLI/VM split. The separate
Claude Code audit exposes the agent engine behind that split and turns several
product recommendations into hard runtime requirements:

- **The stream is a control protocol, not a transcript feed.** Initialization,
  partial model output, tools, permissions, hooks, tasks, compaction, usage,
  rewind, MCP state, cancellation, and terminal results are typed in both
  directions. Khala Sync must preserve those semantics rather than synchronize
  rendered chat rows.
- **One engine needs one conversation owner.** Claude Code's interactive and
  headless/SDK paths still have parallel lifecycle ownership. OpenAgents should
  not reproduce that fault line: Desktop, terminal, SDK, mobile, and web must
  call one scoped conversation service and project its events.
- **Execution completion and delivery are different state machines.** A child
  agent can complete while its changes remain unreviewed, uncommitted,
  unmerged, unpushed, or unaccepted. Those outcomes need distinct canonical
  states and receipts.
- **Append-only local history is valuable, but raw JSONL is not enough.** Claude
  Code's parent-linked messages, sidechains, sidecars, content replacements,
  file checkpoints, and dead-branch filtering provide excellent recovery while
  increasingly behaving like an implicit graph database. OpenAgents should
  keep an exportable append log and make an indexed typed event graph the
  authority.
- **Approval and containment are independent.** Workspace trust, tool
  visibility, deterministic safety checks, permission rules, hooks, user
  approval, and OS sandboxing answer different questions. One shield or
  “allowed” bit cannot summarize effective authority.
- **Hermetic execution is a first-class product mode.** Claude Code's minimal
  mode can suppress ambient instructions, memory, hooks, plugins, keychain
  lookup, language servers, and discovery. OpenAgents needs an equivalent
  reproducible profile that emits the complete admitted-input manifest.
- **Local recovery belongs in the engine contract.** File checkpoints,
  session fork/rewind, durable background output, conservative worktree
  retention, and resumable tasks are not terminal-only conveniences. Desktop
  and mobile should surface the same durable facts.
- **Mobile supervision requires distributed-systems semantics.** Worker epochs,
  event sequence, acknowledgements, replay, idempotent control requests,
  reconnect backoff, sleep/wake handling, and expiring permission responses
  belong in the protocol before live cross-device continuation is declared
  complete.
- **Hooks are authority-bearing middleware.** Internal OpenAgents policy should
  remain typed Effect services. External command/HTTP hooks may integrate at
  bounded lifecycle points, but they must not become an invisible root of
  product authority.
- **Build flags cannot redefine invariants.** Claude Code's broad compile-time
  feature matrix shows how capability growth can fragment semantics. Feature
  flags may select adapters and UX; they must not change task terminality,
  event durability, permission meaning, containment, or signature checks.

The highest-value Claude Code pattern is the local executor controlled by many
clients. The highest-value warning is architectural accretion: multiple query
owners, memory systems, task classes, stores, and build variants can make an
otherwise capable runtime impossible to explain as one coherent state machine.

## What the Codex source audit changes

Codex is the strongest engine-level confirmation of the direction above. Its
open Rust repository shows the target architecture operating across a TUI,
app-server, SDKs, IDE/desktop clients, local/remote transports, and multi-agent
execution:

- **One protocol can serve both embedded and remote clients.** Codex's TUI uses
  an in-process typed app-server client for efficiency and a remote client for
  other runtimes, while both paths reach the same request processor. In-process
  is a transport optimization, not a second engine API.
- **Thread → Turn → Item is a durable product vocabulary.** The app-server
  protocol describes messages, reasoning, commands, patches, tools, agents,
  compaction, review, usage, and terminal outcomes without exposing terminal
  text as state. OpenAgents should extend this with Work Unit, Authority
  Manifest, Execution Receipt, and Delivery Receipt.
- **Protocol generation and experimental gating are release features.** Rust
  types generate version-matched TypeScript and JSON Schema bundles; V2 naming,
  optionality, tagged unions, pagination, and experimental opt-in are explicit
  compatibility rules. OpenAgents needs generated desktop/mobile/SDK clients
  and stable/experimental bundles from the beginning.
- **Append evidence and indexed state can coexist.** Codex keeps inspectable
  rollout JSONL while SQLite indexes thread metadata and owns operational
  records. Recovery can repair stale/missing index rows from rollouts. This is
  the right compromise between local transparency and product-scale queries.
- **Agent topology deserves its own store.** Codex persists directional spawn
  edges, open/closed status, deterministic child lists, and breadth-first
  descendants behind a storage-neutral graph interface. OpenAgents should
  extend that edge state through delivery and acceptance.
- **Permissions can compile into cross-platform enforcement.** Named profiles,
  organization requirements, approval policy, execpolicy, macOS Seatbelt,
  Linux bubblewrap/namespaces/seccomp, Windows restricted-token paths, and a
  managed egress proxy are distinct but composable layers.
- **Remote control is a bounded replicated protocol.** Codex identifies client,
  stream incarnation, sequence, cursor, acknowledgements, and chunks; caps
  frames and reassembly; pairs controllers with short-lived artifacts; and
  exposes revocation. Khala Sync should match this rigor while remaining
  transport-neutral.
- **Context discipline can be mechanical.** Codex requires typed context
  fragments, incremental history, cache-stable inputs, hard caps, and manual
  review for unusually large injections. OpenAgents should enforce equivalent
  invariants in constructors and tests.
- **Testing the public seam is essential.** Codex tests agent behavior through
  mocked provider streams and public protocol calls, checks generated schema
  drift, snapshot-tests terminal projections, and exercises sandbox policy
  generation. Runtime Gateway fixtures should become OpenAgents' main
  integration-test seam.
- **Daemon lifecycle is product state.** Version, process, socket, readiness,
  updater, pairing, and shutdown behavior are explicitly modeled. Pylon must
  expose the same facts instead of flattening them to “online.”

Codex also sharpens what not to copy: a 125-crate, million-line workspace; 92
feature entries including removed compatibility flags; parallel V1/V2 and
legacy/new policy concepts; a hybrid store set without one obvious deletion
transaction; no general per-turn file checkpoint store; no first-class
worktree ownership lifecycle; and a code-mode sidecar that can fall back to
in-process V8 when missing. OpenAgents should take the seams and verification
habits while keeping its Effect service graph smaller and fail-closed.

## Adapt now: changes that belong in the current P0 program

### 1. Freeze the engine protocol before widening the workbench

All three products place their most valuable seam between UI and agent engine:
Codex exposes `app-server`; Claude Code exposes JSON streams through the Agent
SDK; OpenCode exposes a generated HTTP/SSE/WebSocket contract shared by local
and remote clients. OpenAgents should formalize the same seam across Desktop,
Pylon, local Codex/Claude/Grok executors, mobile/web projections, and future
engines.

The contract should include:

- protocol and engine version negotiation;
- stable thread/session/run identifiers;
- client-chosen input/command IDs with exact-retry reconciliation and conflict
  refusal;
- durable admission before advisory execution scheduling;
- explicit pending, promoted, executing, and terminal transitions;
- explicit steer-at-safe-boundary and queue-until-yield delivery;
- ordered event cursors and replay/resume;
- text, reasoning, tool, plan, todo, question, permission, approval, usage,
  error, interruption, and completion events;
- typed capability discovery rather than UI feature guessing;
- explicit cancellation, reconnect, and terminal states;
- connected, heartbeat, disposal, and stale-stream semantics;
- worker incarnation/epoch, causal parent, idempotency key, acknowledgement,
  replay window, and monotonic terminal-state rules;
- backpressure/coalescing rules and bounded replay cursors;
- separate current projections, durable per-aggregate logs, and volatile live
  events, including a replay-to-live synchronization marker;
- explicit child-task and change-integration states rather than a single
  overloaded “completed” outcome;
- redacted diagnostics and per-event provenance; and
- compatibility fixtures replayable without a live provider.

Model the public hierarchy as **Thread → Turn → Item → Work Unit/Receipt**.
Generate every client from the Effect Schema source. Local in-process calls may
substitute an in-memory transport, but they must not skip the request
processor, middleware, policy, transaction, events, or receipts. Persist the
WorkContext on the run and resolve its scoped filesystem, account, provider,
tool, extension, and containment services during execution; a caller may not
replace that context by supplying a different directory.

This is the load-bearing requirement behind Desktop D1, not an implementation
detail. The renderer should consume an Effect Schema event algebra; provider-
specific streams terminate in the host/runtime adapter.

### 2. Preserve and strengthen the existing Electron boundary

OpenAgents Desktop already enforces the right baseline: renderer sandbox,
context isolation, Node integration off, webviews off, restrictive CSP,
deny-by-default permissions/navigation/windows, and fixed validated preload
capabilities. Keep those guarantees as D1–D5 expand.

Add the concrete hardening the teardowns and OpenCode source make visible:

- verify Electron fuses in packaged tests;
- disable RunAsNode, `NODE_OPTIONS`, and CLI inspect arguments;
- require asar integrity and asar-only application loading;
- encrypt cookies and host credential stores;
- validate sender frame and origin for every IPC call;
- Effect Schema-decode every IPC request and response at runtime;
- give artifact, file-preview, terminal, and browser surfaces separate
  partitions and schemes;
- keep tokens, raw IPC, `MessagePort`, arbitrary commands, and general
  filesystem handles out of the renderer; and
- sign/notarize every executable or native module loaded after installation.

These belong in D0/D6 mechanical oracles, not a release checklist maintained by
memory.

### 3. Make host authority and execution isolation separate products

Claude's local Cowork split is the cleanest competitive boundary: the host owns
conversation, selected-folder file access, web fetch, and MCP; generated code
runs in a hardware VM. Codex uses a narrower OS command sandbox. OpenAgents
needs the same conceptual split even if implementation levels differ by task.

Define one typed execution profile contract:

- **projection only** — no writes or process execution;
- **workspace bounded** — explicit roots, bounded subprocesses, no ambient
  host access;
- **isolated guest** — VM/container/microVM with declared mounts and egress;
- **owner-local danger mode** — explicit, local, visually persistent, never
  inferred from public requests; and
- **managed cloud** — Blueprint governance, spend, topology, and receipt gates.

Every execution result should name the profile, grants, engine version,
workspace roots, egress policy, and receipt/evidence references. This connects
Desktop D3/D5 to Pylon and OpenAgents Cloud rather than creating a second local
authority universe.

Compile that profile into two related but separate records: an **authority
manifest** describing what policy and approval admitted, and an **execution
receipt** describing what containment was actually established. If promised
containment is unavailable, the profile fails closed rather than silently
degrading to host execution.

Add a hermetic profile for reproducible work. It admits only explicitly named
instructions, credentials, tools, MCP servers, plugins, directories, and
network destinations, and records their identities and hashes before the first
external side effect.

Use Codex's enforcement direction as the platform reference: named profiles
compile into filesystem roots and carveouts, network domain/method/socket
policy, executable policy, and an OS-specific plan. If the host cannot
represent the policy without weakening it, execution fails closed. A managed
egress service should default deny, make deny override allow, protect local
networks, and emit redacted decision events.

### 4. Turn the command registry into the shared product API

The Sol roadmap already requires every material action to have a stable intent
and command ID. OpenCode demonstrates the immediate product payoff: one catalog
drives palette, keybindings, slash aliases, settings, contextual commands, and
native-menu forwarding. The three teardowns show why it must go further: once
chat grows into files, terminals, plugins, computer use, remote control, and
scheduled work, renderer callbacks alone become an unreviewable authority
graph.

The registry should cover direct UI, keyboard, native menu, command palette,
mobile, and future model-proposed actions. Each entry needs:

- typed input and output schemas;
- capability and policy requirements;
- whether approval is required;
- idempotency/retry semantics;
- durable outcome projection;
- diagnostics redaction class; and
- supported host surfaces.

The assistant may propose an action, but policy and Blueprint's action and
approval gates decide whether it executes. This is the bridge between Effect
Native intents and the actual operating system/runtime.

### 5. Ship a local renderer; share source packages, not remote authority

Claude's hybrid renderer gains deployment speed but creates two web-product
copies and a privileged live-site dependency. OpenCode demonstrates the better
sharing boundary: the desktop renderer imports the same local application
package as web and adapts native behavior through a platform interface.
OpenAgents should not load `openagents.com` as the primary Desktop application.

Instead:

- ship a locally versioned Effect Native renderer;
- share domain packages, components, tokens, and projections with web/mobile;
- keep remote content inside bounded browser/artifact surfaces;
- make the installed renderer/host protocol pair reproducible; and
- require explicit migrations when server projections evolve.

This preserves cross-surface consistency without allowing a web deployment to
silently expand desktop privilege.

### 6. Adopt MCPB as an input format, not as authority

Anthropic has created a practical open packaging convention: `.mcpb` is a zip
with a manifest and local MCP server. OpenAI independently converges on plugin
manifests plus MCP and skill directories. OpenCode proves the interoperability
value of supporting stdio, Streamable HTTP, SSE, OAuth, roots, resources,
skills, custom tools, and provider plugins in one runtime. OpenAgents should
ingest these common formats rather than inventing another archive shape.

The OpenAgents catalog should add what the references do not make strong
enough:

- publisher identity and signature;
- immutable content hash and reproducible unpacking;
- declared runtime, network, filesystem, secret, browser, and UI capabilities;
- typed tool schemas and semantic selection metadata;
- provenance and license;
- organization allow/deny policy;
- staged update and rollback;
- per-session enablement; and
- a public-safe install/run receipt.

MCPB compatibility is ecosystem leverage. Execution still passes through
OpenAgents policy, sandbox, and receipt boundaries.

### 7. Build the component compatibility ledger before D6

None of the products is one immutable binary in practice. Even OpenCode, which
bundles its server with the desktop app, still admits independently moving
plugins, skills, MCP servers, remote runtimes, and native dependencies.
OpenAgents will likewise have an Electron host, renderer, Pylon/runtime engine,
provider adapters, native helpers, skills/plugins, sandbox images, and Sync
protocol.

Create one signed manifest that records:

- component identity, version, hash, signature, and source;
- minimum/maximum protocol versions;
- required host capability and OS/architecture;
- rollout channel and update policy;
- migration and rollback compatibility;
- last-known-good set; and
- verification receipts for install, launch, update, and rollback.

Squirrel versus Sparkle is secondary. The product requirement is that the
whole compatible set is knowable, recoverable, and explainable to the user.

Codex adds two concrete compatibility requirements: generate a schema bundle
from the exact running engine version, and make helper/daemon protocol
negotiation explicit. The manifest should bind host, Runtime Gateway, Pylon,
sandbox helper/image, network proxy, plugin, mobile client, and generated
protocol bundle as one tested set.

## Product-specific adaptations

### OpenAgents Desktop

Desktop should absorb the deepest lessons because it owns local capability.

Priority order aligned with D0–D6:

1. **D1:** real protocol-backed streamed sessions, eager subscribe, connected
   and heartbeat events, durable admission, steer/queue delivery, current
   projections, replayable per-thread log, volatile live updates, interrupt,
   resume, reconnect, coalescing, permissions, approvals, and usage. The local
   embedded path and a remote path must hit the same request processor, as
   OpenCode V2's memory/network clients and Codex's TUI/app-server do.
2. **D2:** project/session routes plus the central command registry, palette,
   keybindings, native menus, deep links, and restore.
3. **D3:** bounded file grants, lazy tree and content budgets, editor foreign
   host, typed Git review, replayable ticketed workspace PTY, and guest-
   execution profiles.
4. **D4:** runtime/model/MCP/skill/plugin registries, account custody,
   permissions, diagnostics, and recovery.
5. **D5:** authoritative Fleet projection and control using the same command
   outcomes mobile sees.
6. **D6:** identity freeze, fuses, signing, notarization, component manifest,
   clean install/update/rollback proof.

Conversation stays the quiet default. Files, terminal, review, browser,
computer use, and Fleet appear when active context requires them, following the
three-depth product shape already in the Sol roadmap.

### OpenAgents mobile

Mobile should adapt the reference products' remote-control value without
becoming a remote desktop. Its narrow identity/Sync continuation slice lands
with Desktop D1; it does not wait for the full Desktop workbench or Fleet
cockpit.

It should:

- show the same durable thread, run, request, approval, command, outcome, and
  receipt records as Desktop;
- start or steer work through the shared command registry;
- receive a durable admission acknowledgement before presenting a command as
  accepted, and choose explicit steer or queue behavior for active work;
- receive input-needed, completion, failure, and budget notifications;
- make device/offline/capability state explicit;
- use idempotent commands, worker epochs, ordered replay, acknowledgements, and
  monotonic terminal outcomes across disconnect and sleep/wake;
- support resumable handoff to Desktop; and
- carry no desktop tokens, raw local paths, arbitrary IPC, or hidden danger
  mode.

The correct abstraction is “control and understand the work,” not “stream the
desktop pixels and hope clicks mean success.”

### Pylon and local runtimes

Pylon should become the stable multi-engine supervisor behind Desktop rather
than a second product shell. OpenCode's built-in/WSL/SSH/HTTP connection model
is the practical reference: clients address one typed runtime identity while
the host chooses how that runtime is reached and supervised.

Adapt:

- Codex/Claude-style versioned stdio sessions;
- health and capability negotiation;
- named isolated account custody;
- typed quota/auth/rate-limit failures;
- engine download/provenance verification;
- deterministic shutdown/reconnect/reap;
- discover, authenticate, version-check, elect, and restart one compatible
  managed local service without stale PID/socket ambiguity;
- one conversation service shared by interactive, headless, SDK, and remote
  adapters;
- health, heartbeat, disposal, and stale-stream recovery;
- durable task output, file checkpoints, session fork/rewind, and
  outcome-sensitive worktree retention;
- a staged rewind transaction with inspect/commit/clear and an explicit list of
  irreversible side effects;
- local/remote transport adapters behind stable runtime identities;
- plugin/MCP/skill composition; and
- redacted replayable event receipts.

Pylon should expose a machine-readable lifecycle record containing executable
identity/version, protocol version, process generation, socket/transport,
readiness, update state, active worker epoch, and last transition. Start must be
idempotent, lifecycle mutations serialized, and stop/update/rollback bounded
and receipted.

Do not expose provider-specific sidecar flags directly to renderers or mobile.

### Khala Sync

Khala Sync is the answer to the reference products' growing cross-device surfaces.
It should synchronize durable product facts, not desktop implementation state:

- stable thread/session/run/work-unit IDs;
- event cursor and compacted transcript projection;
- active context and selected project identity where safe;
- pending requests and approvals;
- command acceptance and terminal outcome;
- worker/account/capacity health;
- artifact/evidence/receipt references; and
- explicit conflict/refetch state.

The synchronized command/event envelope must also carry an idempotency key,
worker epoch, ordered sequence, acknowledgement state, causal parent, and
expiry for authority-bearing responses. The server and clients must reject a
late progress event that attempts to reopen a terminal task.

Sync should expose three distinct client contracts: bounded current
projections, a durable replay log with a replay-to-live synchronization marker,
and a volatile coalesced event stream. Reconnection repairs from projection and
log; it never assumes the live stream retained events while a device slept.

Use bounded segmentation for large events and cap each frame, full message,
segment count, and number/age of partial assemblies. Controller pairing is
short-lived; durable device grants are listable and revocable.

Cursor positions, window geometry, raw filesystem handles, secrets, and
unredacted local logs remain local.

### OpenAgents Cloud and sandbox services

Claude's VM proves that isolation is a product feature users can understand;
Codex proves narrower OS sandboxes still matter for local latency.

OpenAgents Cloud should expose a common execution receipt across local sandbox,
microVM/container, GCE/managed workroom, and remote provider modes. The receipt
must never pretend that a formal model or UI approval is runtime authority. It
records what actually ran, where, with which grants, and what evidence resulted.

### openagents.com

The web product should remain the public and remote projection surface, not the
source of local desktop privilege.

It should adapt:

- the same typed command/outcome vocabulary;
- remote session and Fleet visibility;
- plugin/catalog discovery and provenance;
- device presence/capability state;
- human-readable receipts and diagnostics; and
- continuation links into Desktop/mobile.

It should not receive local filesystem authority merely because Desktop is
online. Every local reach must be an explicit connected capability with a
bounded request and durable outcome.

### Effect Native

The reference stacks validate the need for foreign-host components without
weakening the one-UI-substrate decision.

The OpenCode Effect audit adds a stronger foundation: Effect Native must own an
explicit application service graph, not only expose components that happen to
return Effects. Define five scopes up front:

| Scope | OpenAgents examples |
| --- | --- |
| Process | identity, encrypted storage, Khala Sync, observability, component ledger |
| WorkContext | repository, Blueprint program/action, policy, provider catalog, containment, Pylon target |
| Conversation/run | model stream, captured tool generation, child topology, budgets, settlement |
| Request/command | decoding, idempotency, approval, transaction, event, receipt |
| Foreign host/view | PTY, editor, diff, browser preview, canvas, native capture |

Every service and resource should declare its owner, allowed upstream scopes,
cache key, freshness rule, and disposal proof. A WorkContext-scoped service may
depend on process services; a process service must not capture one WorkContext.
Avoid ambient cwd or `AsyncLocalStorage` as authority.

Effect Native should define typed, lifecycle-owned hosts for:

- terminal/PTY;
- Monaco or CodeMirror editor;
- diff/review;
- browser/artifact preview;
- canvas/3D;
- native capture/computer-use status; and
- permission/approval surfaces.

Each host receives serializable configuration and emits typed intents/events.
It does not leak library-specific instances, Electron APIs, native handles, or
provider state into application code. Acquisition creates an owned Scope;
unmount, navigation, runtime replacement, and app shutdown close it and its
fibers, subscriptions, registrations, and native resources together.

The in-process Effect Native/runtime composition should substitute an owned
transport into the same generated client and request processor used over the
network. It must not become a privileged direct-service API that bypasses
middleware, WorkContext resolution, policy, events, or receipts.

Promise, Electron IPC, React Native, and native callback bridges should capture
and re-enter Effect context only in explicit perimeter modules. Cancellation
must become Effect interruption and run finalizers. Tests must be able to
replace every platform host with a Layer and use deterministic time without
starting Electron, React Native, or a browser.

OpenCode's `Platform` adapter is the useful comparison point, while its
Ghostty terminal shows the desired lifecycle shape: create/connect/replay,
resize/input, persist bounded presentation state, disconnect, and dispose. The
Effect Native version should make that lifecycle an owned Effect resource and
keep the runtime token/transport outside ordinary renderer state.

Do not copy OpenCode's custom `LayerNode` wholesale. Start with native Layers,
small explicit node metadata, and architecture tests for dependency direction,
cycles, replacement, freshness, and cleanup. Introduce a graph compiler only
when WorkContext hoisting or graph-aware replacement produces a concrete need.

## Adapt later, after the core loop is trustworthy

These reference capabilities are useful but should not preempt D1–D6:

- multi-window workbench depth;
- SSH and remote-environment management;
- full browser automation inside Desktop;
- computer use across arbitrary native apps;
- scheduled autonomous local work;
- hardware/Bluetooth approval devices;
- ambient suggestions;
- complex app-generation/artifact studios; and
- a proprietary browser profile/runtime.

Their prerequisites are a stable event protocol, command registry, permission
model, sandbox, diagnostics, update ledger, and cross-device authority. Shipping
them first would multiply unsafe surface area around a still-scaffolded core.

## Reject explicitly

### 1. No ambient screen recording or inferred personal memory by default

The ChatGPT bundle's Chronicle/Skysight design is technically ambitious and a
serious trust warning. OpenAgents should not continuously record screens,
extract OCR, summarize activity, or promote “non-obvious context” into durable
profile memory as a default desktop feature.

If a future user explicitly asks for ambient memory, require:

- a separate install/enable action;
- visible recording state and instant pause;
- app/window allowlist rather than a best-effort denylist;
- short, user-selectable retention;
- local encryption and inspect/delete/export controls;
- no promotion into durable memory without review;
- taint/provenance on observed content;
- no use for advertising, scoring, or hidden routing; and
- a mechanical guarantee that disabled means no capture process.

### 2. No browser-runtime fork

Owl is rational for a company that owns a browser organization and wants deep
browser/computer-use integration. It is not rational for OpenAgents. A fork
would create a permanent Chromium security, packaging, compatibility, and
tooling obligation while stock Electron plus native services already supports
the required product.

### 3. No remote web deployment as privileged desktop application code

Remote content may render in a bounded browser/artifact surface. It must not
silently gain the preload capabilities of the installed app. Desktop privilege
changes require a signed app/component release and protocol review.

### 4. No opaque sidecar accumulation

Every long-lived process must have an owner, protocol, version, health model,
resource budget, shutdown rule, log-redaction policy, update source, and test.
If two sidecars can be one typed runtime service, keep one. If isolation is the
reason for a process, document the boundary and prove it.

### 5. No transcript-as-authority

Neither model prose nor a green UI row proves a command executed, a file
changed, a FleetRun exists, a payment settled, or an update installed. Product
state comes from typed runtime outcomes and Blueprint records and receipts.

### 6. No live-stream-as-authority or child-authority widening

A healthy SSE/WebSocket connection is not evidence that no event was missed.
Reconnect must repair from bounded projections and a durable log before the
live stream resumes. Likewise, a child agent's configured profile may narrow
the parent's delegation but may never widen it; effective child authority is
the intersection of parent grant, child policy, WorkContext, and containment.

## The OpenAgents differentiation

The three reference products leave a coherent opening:

- OpenAI has the most inspectable engine and the clearest rich-client protocol,
  sandbox, and explicit agent graph, but a closed desktop host and broad
  ambient-computer ambitions; the open runtime also carries substantial
  compatibility and feature-matrix debt.
- Anthropic has an open extension/SDK edge but a closed engine and a desktop
  whose normal renderer is partly controlled by a live web deployment.
- OpenCode opens the load-bearing host, engine, UI, protocol, and test seams,
  but renderer-held server credentials, in-process extensions, host execution,
  and an active legacy/next migration leave authority and complexity gaps.
- All three accumulate multiple state systems, runtimes, update planes, or
  compatibility boundaries that are hard for users to inspect as one system.

OpenAgents should differentiate on seven properties:

1. **Typed end to end:** Effect Schema from user intent through command,
   runtime event, Sync projection, and receipt.
2. **Open at the load-bearing seam:** open engine/protocol/catalog contracts,
   not merely an open wrapper around a proprietary executable.
3. **Explicit authority:** local, guest, cloud, spend, approval, and public
   projection boundaries remain visible and testable.
4. **Cross-device truth:** Desktop, mobile, and web converge on durable records
   instead of mirroring UI state.
5. **User-controlled memory and automation:** no ambient surveillance, hidden
   promotion, or implicit permission expansion.
6. **Explainable execution:** every run can show context provenance, admitted
   authority, effective containment, child topology, and delivery outcome as
   separate typed facts.
7. **Repairable local truth:** an append-only evidence log can rebuild an
   indexed state authority, and repair/deletion itself produces a receipt.

That is a stronger product thesis than “another desktop chat app” or “OpenCode
with Fleet buttons.” It is an inspectable operating surface for work performed
by multiple engines across multiple computers, with authority and evidence that
survive beyond the renderer process.

## Ordered implementation consequences

| Order | Decision | Owning program | Proof |
| ---: | --- | --- | --- |
| 1 | Freeze the tokenless renderer → host-owned Runtime Gateway and one Thread/Turn/Item conversation protocol | Desktop D0/D1 + Pylon | Boundary oracle proves no runtime credential or generic transport enters renderer; embedded/remote/interactive/headless/mobile fixtures hit the same request processor; exact-retry durable admission and conflicting-ID refusal pass |
| 2 | Bind Desktop and mobile to the existing R1 identity and R2 Khala Sync contracts | R1/R2 + Desktop/mobile adapters | Same server-derived owner/scope, independent revoke, SQLite restart, exact phases, no token in projections |
| 3 | Ship one real streamed Desktop conversation with immediate mobile continuation | Desktop D1 + mobile narrow Sync slice | Matching refs and explicit steer/queue; admitted/pending/promoted phases; current projection + durable log + volatile live stream converge after restart, gap, sleep, overflow, and lost ACK; one safe follow-up/interrupt |
| 4 | Extend the central command registry through host/runtime outcomes and generated clients | Desktop D2 + Effect Native | UI, keyboard, menu, mobile, SDK and test invoke the same command IDs/schema bundle and reconcile one durable outcome |
| 5 | Add bounded file/editor/Git/PTY foreign hosts while mobile remote-workroom work proceeds in parallel | Desktop D3 + mobile R6 + Effect Native | Useful coding loop on each form factor without renderer or phone local process/filesystem authority |
| 6 | Define authority manifests, compiled execution/egress profiles, recovery, and isolated extension compatibility | Desktop D3/D4 + Pylon + Cloud | Same task reports admitted authority separately from effective containment; cross-platform fail-closed profile, hermetic run, checkpoint/rewind, signed extension denial/update/rollback/run receipts pass |
| 7 | Compose the existing Fleet/approval/command-outcome authority into both clients | Desktop D5 + R3/R4 | One steer/approval converges to one effective durable outcome and receipt on both clients under faults |
| 8 | Package the compatible component set and dogfood | Desktop D6 + R7 | Fuses, signing, notarization, clean install, update, rollback, physical mobile, diagnostics, sustained handoff |
| 9 | Add computer use, scheduling, and advanced remote environments selectively | Post-R7 | Explicit grants, visible state, sandbox/authority receipts, failure recovery |

## Final recommendation

Do not chase any reference product's total feature surface. Adopt the common
architecture they validate, use OpenCode as the inspectable implementation
reference, then make its boundaries simpler and more legible:

- stock Electron;
- local Effect Native renderer;
- versioned open local/remote engine protocol with generated clients;
- durable command admission with explicit steer/queue delivery;
- current projection, durable replay log, and volatile event transports with
  explicit lifecycle and gap semantics;
- one conversation service and one indexed canonical event graph;
- WorkContext-scoped runtime services and generation-owned catalogs;
- generated stable/experimental clients around Thread/Turn/Item/Work Unit;
- host/guest execution split;
- separate authority manifests and containment receipts;
- named cross-platform permission profiles and managed egress audit;
- hermetic execution, checkpoints, rewind, and outcome-sensitive worktrees;
- staged inspect/commit/clear rewind with irreversible-effect disclosure;
- central typed command registry;
- MCPB-compatible signed catalog;
- one component/update compatibility ledger;
- cross-device Blueprint projections; and
- explicit, user-controlled permissions and memory.

Use OpenCode V1 as the inspectable desktop-host reference, OpenCode V2 as the
durable-admission/scoped-service reference, Codex as the inspectable
engine/protocol/sandbox reference, and Claude Code as the local
recovery/worktree reference. If OpenAgents delivers those foundations through
D6, computer use, agent computers, Fleet, and future AI-employee surfaces can
grow without turning the desktop app into an uninspectable collection of
privileged webviews and sidecars.

## Where we were then — frozen build status against this plan (2026-07-10, late)

This status ledger is a point-in-time snapshot, not current implementation or
issue authority. In particular, #8674/#8675 and later Desktop child-activity
work landed after it, while #8676 has advanced to deterministic implementation
and still awaits its real physical-device receipt. Use the Sol roadmap, live
issues, guarantees, code, and receipts for current state.

This section is the moving progress ledger against the nine ordered
consequences above. Rungs use the six-state vocabulary (code-landed /
fixture-proven / deployed / live-proven / owner-accepted / closed).

### Snapshot

| Step | Direction | Status | Evidence |
| ---: | --- | --- | --- |
| 1 | Tokenless renderer → host-owned Runtime Gateway + versioned protocol | **landed + gated** | D1-A/B closed the Desktop Runtime Gateway protocol/lifecycle and bound it to host-owned Khala Sync SQLite (#8655, #8656); hardened Electron boundary (contextIsolation/sandbox/deny-by-default, bridge-only preload) shipped with the greenfield app (#8574) |
| 2 | Bind Desktop + mobile to R1 identity / R2 Khala Sync contracts | **landed both clients** | R1/R2 identity+Sync contract published; Desktop D1-B..F (#8661–#8665) and mobile M1-A..D (#8657–#8660) shipped OS-encrypted/SecureStore native-session custody, validate-and-rotate-fail-closed, loopback+mobile OpenAuth PKCE sign-in/sign-out — server-derived scope, no token in projections |
| 3 | One streamed Desktop conversation + immediate mobile continuation | **partial** | Desktop shows real local Codex chats via a contract-gated thread projection (#8653, replacing the placeholder JSON); real chat components (transcript/composer, clear-on-submit fixed upstream at catalog v29). Cross-device continuation of a *live* streamed thread with matching refs/versions/cursor is the next honest proof, not yet demonstrated |
| 4 | Central command registry through host/runtime outcomes | **early** | Typed intents + a typed command surface exist per-app (Desktop session controls #8665); a single shared command-ID registry reconciling UI/keyboard/menu/mobile/test to one durable outcome is not yet unified |
| 5 | Bounded file/editor/Git/PTY foreign hosts + mobile remote workroom | **seam only** | The Electron foreign-host pattern is proven (media-video/host-driver lineage, effect-native #67/#70); no editor/PTY/Git host mounted yet — this is OpenCode-parity D3 work |
| 6 | Execution profiles, receipts, isolated extension compat | **substrate present** | Exact usage receipts + typed fail-closed provider execution landed in the Agent Computer lane (#8547: in-VM codex execution, broker-redeemed scratch home, Seatbelt-class isolation on the cloud side); the double-billing metering fix proves the receipt path is honest. Desktop-side execution profiles/extension signing not started |
| 7 | Compose Fleet/approval/command-outcome authority into both clients | **fleet substrate done, client compose pending** | FC-1/FC-2/FC-3 closed (durable FleetRun, mixed-harness Pylon supervisor, supervision surface); the desktop **Settings → Connect Codex account** flow proves host↔fleet-auth composition. One steer/approval converging to one durable outcome on BOTH clients under faults = the #8640 live burn, still owner+gate-blocked |
| 8 | Package compatible component set + dogfood | **runnable, not packaged** | Both greenfield apps run from source (`oa` desktop, Expo mobile TestFlight builds 103→116); Sparkle/fuses/notarization/clean-install/rollback checklist (adapted from the ChatGPT + OpenCode teardowns) not yet executed for OpenAgents Desktop; mobile ships local-first via TestFlight already |
| 9 | Computer use, scheduling, advanced remote envs | **deferred by design** | Not started; correctly post-R7 |

### What actually moved the needle since the teardowns

- **Identity + Sync is real on both clients** (steps 1–2): the largest single
  advance. The rev-24 reset made "one authenticated identity, Khala Sync as
  cross-device authority, device stores as caches" a P0, and ~12 issues
  (#8653–#8665) shipped it with fail-closed custody and PKCE on desktop and
  mobile — the foundation every later step assumes.
- **The renderer is tokenless and the boundary is hardened** (step 1): the
  differentiation thesis from the teardowns — "not an uninspectable pile of
  privileged webviews" — is enforced by an oracle, not a hope.
- **The receipt/execution honesty exists** (step 6 substrate): the Agent
  Computer lane proved in-VM provider execution with exact receipts and caught
  a real double-billing bug — the "receipts as trust material" adaptation is
  already load-bearing, not aspirational.
- **The chat surface stopped being a placeholder** (step 3): #8653 put real
  local Codex history behind a typed projection; real transcript/composer
  components landed (with the never-clearing-input bug fixed at the catalog
  root, catalog v29).

### The honest gaps (what the plan still needs)

1. **Live cross-device continuation** (step 3's real proof): a streamed thread
   started on desktop, continued on the phone with matching refs/versions and
   a safe interrupt through a network gap. The pieces exist; the demonstration
   does not.
2. **One unified command registry** (step 4): today each app has typed
   commands; the shared-ID reconciliation across form factors is unbuilt.
3. **The coding loop's foreign hosts** (step 5): editor/PTY/Git — the actual
   OpenCode-parity workbench, the largest remaining Desktop lane.
4. **The #8640 live burn** (step 7): the single event that converts "fleet
   substrate exists" into "an owner steers real parallel work from both
   clients." Blocked only on the Codex reconnect (now a desktop Settings
   click) and Sol's typecheck gate.
5. **Packaging + dogfood** (step 8): Sparkle-adapted update/rollback/notarize
   pipeline for Desktop.

### Reading

The teardowns argued: adopt the common architecture (stock Electron, local
Effect Native renderer, versioned host/engine protocol, tokenless renderer,
typed command registry, cross-device Blueprint projections, explicit
permissions), use OpenCode and Codex as inspectable host/engine references,
combine them with Claude Code's recovery ergonomics, and make the boundaries
simpler.

Two days later, steps 1, 2, and the substrate under 6 are **real and gated**;
step 3 is half-real; steps 4–5 and 8 are seams or early; step 7's engine is
built and waits on one live burn. The plan is not aspirational prose anymore —
it is a burndown, and the front of it is landing. The differentiators the
teardowns identified (open, typed, tokenless, receipted, one runtime) are the
parts already shipped, which is the correct order: build the trustworthy core
first, add surface after.

### Auth model clarification — how identity actually works today, and the local-first gap (2026-07-10)

**What the R1 lanes shipped (code-verified):** both clients authenticate
against **our own OpenAuth server, `auth.openagents.com`** — the same issuer
the `openagents.com` Worker already trusts (`OPENAUTH_ISSUER_URL`).

- Desktop (`apps/openagents-desktop/src/desktop-session-pkce.ts`): a temporary
  **literal-loopback listener on `127.0.0.1`** runs the public-client PKCE
  (S256) code flow against `auth.openagents.com/authorize` + `/token`, verifies
  the server owner, then stores the session in OS-encrypted custody. This is
  the exact pattern the ChatGPT/Codex teardown documented (loopback PKCE, no
  client secret) — we implemented the good version of it.
- Mobile (`apps/openagents-mobile/src/auth/native-session-pkce.ts`): the same
  OpenAuth PKCE against the same endpoints, with SecureStore custody and
  validate-and-rotate-fail-closed.

So identity is **first-party** (our auth server, not a third party), which is
correct — but as built it is **auth-required**: the current flows assume an
`auth.openagents.com` account before the app is fully useful.

**The owner's intended model (2026-07-10 direction), not yet reflected in the
build:** the apps should be usable with **local-only pairing and no auth beyond
the device itself** — pair a desktop and a local Codex/Pylon, run fleets, use
the workbench, entirely offline-of-account. An **OpenAgents auth account is an
opt-in upgrade**, not a gate — unlocking cross-device Khala Sync, Khala
network participation, hosted capacity, and the other account-scoped benefits.

**The gap and the fix (a design decision for the R-gate lanes):** R1 as shipped
conflates "has identity" with "has an `auth.openagents.com` account." The
target is a **two-tier identity model**:

1. **Local identity (default, no server auth):** a device-generated keypair /
   local account is authoritative for a purely-local pairing. Khala Sync runs
   in **local/device scope only** (the SQLite store already exists as the cache;
   here it is simply the authority for local-only data). Everything that does
   not cross devices or touch the network works with zero
   `auth.openagents.com` round-trip. This matches the "device stores are
   authoritative for local-only, caches for synced" spirit of the R2 contract.
2. **OpenAgents account (opt-in upgrade):** signing in with
   `auth.openagents.com` **links** the existing local identity to the
   server-scoped owner — promoting local-only projections to cross-device Khala
   Sync, enabling network/Khala participation and hosted capacity. The link is
   additive and reversible; sign-out returns to local-only, it does not wipe
   local work.

**Consequence for the ledger above:** step 2 ("bind both clients to R1/R2") is
**done for the account path, and needs the local-first tier added** so auth is
an upgrade, not an entry gate. This should be filed as an R1 amendment
(local-identity authority + account-link promotion) before the packaging /
dogfood step, since "open the app, pair locally, no login" is a core part of
the predictable-software thesis — the opposite of the ChatGPT app's
account-and-attestation-gated posture the teardown criticized.

## Episode 248–249 product calibration addendum — 2026-07-11

[Episode 248](../transcripts/248.md) and
[episode 249](../transcripts/249.md) turn the architecture above into two
user-visible release promises: recent work must be discoverable before detail
hydration can block first paint, and delegated work must remain a navigable
graph rather than collapse into spawn prose. Openness matters at this seam
because users must be able to inspect, extend, and mechanically verify the
interface they depend on.

The resulting product consequences are:

1. **Metadata-first startup is a contract.** Paint the shell and newest named
   top-level roots before selected transcript, checkpoint, or child detail.
   Loading, empty, missing, corrupt, and slow states are explicit; no blank
   startup or permanent loading copy is accepted.
2. **Recent-first is not an age ceiling.** Episode 248's rolling-24-hour v1
   entry point evolved into the loss-accounted v2 catalog: an initial bounded
   disclosure for fast paint, followed by explicit paging with no age
   ceiling. Children never pollute the root catalog, but they are never lost.
3. **Causal inline activity and complete topology are complementary.** A
   confirmed child-start edge appears once at the parent item with exact child
   identity, lifecycle, and one bounded redacted latest-activity preview. The
   complete roster stays available and each child opens an independent
   transcript. Preview prose and liveness never become completion authority.
4. **Fast supervision has one action path.** Click, tap, command palette,
   native menu, and conflict-safe hotkeys dispatch the same typed focus,
   inspect, steer, interrupt, and return intents. The “StarCraft” goal is
   persistent topology and low-latency direct control, not decorative density
   or a keyboard-only second authority.
5. **Historical and live graphs share semantics, not custody.** Imported Codex
   or Claude history remains owner-local and loss-accounted unless explicitly
   adopted. OpenAgents-owned live runs must project one canonical graph through
   Khala Sync. Desktop, mobile, and web preserve identity, topology, lifecycle,
   gaps, and navigation at surface-appropriate density; identical simultaneous
   layout is not required.
6. **Streaming must finish the interaction, not replace recovery.** The durable
   per-thread log is event authority and repairs the derived current projection
   before volatile live updates resume. Live child acceptance, lifecycle/latest activity, direct
   transcript access, replay deduplication, and explicit stale/gap state need a
   bounded post-#8676 acceptance leaf; historical #8674/#8675 do not prove it.
7. **The promise is checked twice.** Every human-visible milestone carries a
   versioned behavior contract and programmatic oracle plus the appropriate
   real Electron or physical-device journey. Public-safe QA receipts reuse the
   existing evidence machinery; this does not require reviving a second broad
   QA Swarm epic.

Portability adds one more consequence: a host move preserves canonical agent
refs, parent edges, independent transcript/activity cursors, and graph-wide
fencing. A source child may not remain able to accept work after target
activation, and target-native worker/thread IDs remain attachment-local
mappings rather than portable identity.

## Cursor addendum (2026-07-11)

The [Cursor product teardown](./2026-07-11-cursor-product-teardown.md)
revisits episode 197's study target after Cursor's 2.x–3.x agent-platform
pivot. Its evidence changes this document's decisions in the following ways.

1. **The thin-host/agent-first architecture bet is market-confirmed.**
   Cursor rebuilt its product around parallel agents isolated in worktrees
   and remote machines, demoting the VS Code editor to one reachable pane.
   This strengthens the executive decision (thin host, server-owned
   authority, no IDE-parity chase) and the CUT graph's worktree discipline.
   No scope change.
2. **Portable sessions gain an incumbent counterexample.** Cursor's CLI `&`
   cloud handoff and mobile Remote Control prove users want local-to-remote
   continuation and phone-driven supervision — but both are one-way gestures
   into one closed cloud, without portable identity, capability revocation,
   or receipts. This sharpens, and does not alter, the Revision 30/31
   portable-session and capability-broker packets: the differentiator is
   session movement as a receipted authority transfer, not a copy.
3. **Predictability failures are now documented at the market leader.**
   Cursor's Agents Window force-open regressions (setting ignored, projects
   forgotten, April–July 2026 forum record) are the live failure class that
   decision 16 exists to prevent. Strengthen the adaptation rule: every
   default-surface change lands with a restored-context behavior contract
   and oracle in the standing sweep — a bounded leaf under the D0/D6 work in
   #8574 rather than a new epic.
4. **Model provenance joins the no-substitution surface.** The Composer 2 /
   Kimi K2.5 disclosure incident shows users audit model identity
   forensically. Any future OpenAgents first-party or fine-tuned model must
   publish base, post-training, and serving path from day one; harness/model
   pinning UI (the #8712 pattern) should display provenance, not merely the
   pinned name. Bounded leaf under the D5/settings surfaces in #8574 when a
   first-party model exists; documentation posture is effective immediately.
5. **Usage truth must reach the pre-spend surface.** Both Cursor pricing
   crises (June 2025 credits transition; June 2026 pool split) were bills
   users could not predict from visible state. Extend capability-truthful
   affordances: before dispatch, the composer/fleet surface shows the
   account, lane, and budget that will be consumed; receipts reconcile after.
   A bounded leaf under the D5 fleet-cockpit work in #8574, using existing
   exact-or-`not_measured` accounting — no new metering system.
6. **Best-of-N is a fleet shape, not a UI garnish.** Cursor's `/best-of-n`
   validates one-task/many-workers comparison. OpenAgents already runs mixed
   Codex+Claude fleets (#8640) with per-child usage; typed comparison records
   over existing FleetRun/work-unit contracts are a natural post-parity leaf
   under the D5 cockpit — never a second orchestration path.
7. **Unattended authority stays deny-by-default.** Cursor enables computer
   use by default inside Automations. The episode-195 follow-up's Automations
   slice keeps the opposite posture: budgets, pause, Inbox visibility, and
   deny/ask tool authority. Reject the default-grant pattern explicitly.
8. **The open seam widened.** Cursor's pivot (closed fork, closed models,
   closed cloud, closed marketplace, now SpaceX-owned) leaves the open
   engine, typed public protocol, local execution, and economic
   participation unclaimed. The differentiation section of this document
   stands with more force, not less.

Per the standing rule, none of these items is authority here: each lives or
dies by its owning roadmap gate, issue, or contract when promoted.

---

## Cursor 3.11.13 bundle addendum (2026-07-11, evening)

The same-day local bundle survey of the installed Cursor 3.11.13 (see the
Cursor teardown's `[bundle]`/`[runtime]` section, landed `6ff4cf299a`) turns
several of the morning addendum's web-sourced items into artifact-backed
ones and adds four adaptation-relevant findings the web pass could not see.

1. **Containment ships as a binary; ours ships as profiles — keep the
   receipt half.** Cursor bundles `cursorsandbox`, a Rust deny-by-default
   Seatbelt wrapper for agent commands. `[bundle]` corroboration that the
   market has converged on OS-enforced containment below approval — but the
   binary is opaque: no per-run containment receipt, no profile identity in
   the product surface. The authority-manifest/execution-receipt split
   (Adapt-now 3) is the differentiating half; do not weaken it toward
   "sandbox present" boolean claims.
2. **The dual-workbench cost is measurable — the single-grammar bet gains
   evidence.** Cursor ships a 46 MB "Glass" agent workbench *beside* the
   41 MB classic VS Code workbench plus an 8 MB automations bundle and a
   separate React runtime. That is the compatibility-debt disease (§4.4 of
   the source teardowns) shipping at megabundle scale: two parallel UI
   universes over one product. Effect Native's one-typed-component-set bet
   (single catalog, thin renderers) is the structural refusal of exactly
   this outcome; conversion PRs that delete the replaced surface remain the
   enforcement mechanism.
3. **Model lineage is invisible at the artifact level — provenance surface
   confirmed as differentiation.** ~21,000 `composer` strings against five
   `kimi` strings in the shipped bundles: after a public non-disclosure
   incident, the client artifact still carries no lineage surface. The
   morning addendum's recommendation 3 (model provenance joins the
   no-substitution surface) is now backed by `[bundle]` evidence that the
   incumbent has not built it. The EP250 pinning UI (`model_substituted`
   typed failure, effective-model caption) is the seed; publishing
   base/post-training/serving-path provenance stays the bounded follow-on
   under the owning contracts.
4. **Update-chain posture is a live cautionary receipt for D6/CUT-26.**
   `[bundle]`: notarized under an individual's Developer ID rather than the
   company's, no App Sandbox, ATS disabled, and a plain-HTTP backup update
   URL. Adapt-now 7 (one signed component-compatibility ledger with
   rollback proof) and the CUT-26 hardened-distribution exit should cite
   this as the failure posture being refused; signing identity, TLS-only
   feeds, and fuse verification are release oracles, not checklists.
5. **Local retrieval index as a first-class agent asset (adapt-later,
   sharpened).** `crepectl` — a bundled Rust local code-index builder
   (gitoxide-based, disk-spilling) — shows the incumbent treating local
   retrieval as agent infrastructure, not an editor feature. Maps to the
   D3 workspace search/cache lane (CUT-17 #8697): when that work lands, an
   owned index service behind the grant-scoped workspace seam is the
   parity-plus-authority shape; no new commitment now.
6. **Dedicated transcript-search process (3.11) touches the Episode 248
   lineage.** `[runtime]`: a separate `conversation-search` process serving
   chat-history search. The predictable-history contract (#8674/#8675, no
   age ceiling, metadata-first) already owns discovery truth; full-text
   search over the loss-accounted catalog is a natural bounded leaf under
   D2 discovery when prioritized — indexes stay rebuildable caches, never
   authority (episode-195 follow-up's History/Discovery/Memory split).
7. **`~/.cursor` is now an agent-home, joining `~/.codex` and `~/.claude`.**
   Names-only survey shows agents/chats/projects/plugins/skills/hooks/mcp
   state trees. A third provider agent-home strengthens the loss-accounted
   provider-import direction (CUT-22 #8702 class): Cursor agent history is
   a future import adapter candidate under the same graph-adapter rules
   (explicit gaps, no `parentUuid`-style edge misuse, owner-local custody).

No morning-addendum recommendation is weakened; items 3, 4 above upgrade
two of them from `[public]`-argued to `[bundle]`-backed. Per the standing
rule, promotion into requirements happens only through the owning gates,
issues, and contracts.

## Executor addendum (2026-07-12)

The [Executor architecture teardown](./2026-07-12-executor-architecture-teardown.md)
adds a reference the original desktop synthesis did not have: a focused,
open-source integration substrate whose July Apps slice lets a typed authored
function compose authenticated tools in an isolate and then re-enter the
catalog as a durable tool.

This does not alter the original decision that OpenAgents owns its engine,
policy, containment, identity, and receipts. It sharpens what “signed, isolated
typed catalog” must mean and moves one narrow capability out of the broad
“app-generation studio” deferral.

### Decision

> Adapt Executor's authored-capability artifact, caller-bound account roles,
> staged compiler, default-deny function isolate, and nested canonical-dispatch
> pattern into OpenAgents. Consume Executor itself only as an optional MCP/HTTP
> capability provider. Do not embed it as Khala/Pylon's authority core.

The distinction matters because Executor and OpenAgents already overlap in
tool registry, MCP, account/credential references, policy, approval, Effect
runtime, storage, and host forms, and both are aimed at workflows, skills, and
remote execution. Embedding Executor would create two answers for catalog
projection, policy precedence, approval identity, cancellation, durability,
and receipts. Interoperability keeps the boundary explainable; wholesale
consumption does not.

### What the implemented Executor slice proves

At the pinned `v1.5.33` commit, Executor proves these claims in source and
tests:

1. **A durable authored tool is distinct from transient Code Mode.** Code Mode
   runs one generated program over a captured catalog. Apps adds source
   identity, bundling, deterministic schema collection, content-addressed
   publication, an active descriptor/tool-row projection, and tombstones.
   Generated code can become a reviewable reusable artifact rather than
   disappear with one model turn.
2. **Accounts can remain invocation parameters.** Source declares semantic
   roles such as `github`, `crm`, or `inboxes`; the caller supplies one or many
   concrete named connections. The handler receives callable proxies, never
   the selected addresses as ordinary business input and never raw secrets.
3. **Composition must return to the ordinary invocation processor.** Every
   nested proxy call reconstructs a connection-backed tool address and re-
   enters Executor policy, approval/elicitation, credential resolution, plugin
   dispatch, and result handling. Custom code is not a privileged adapter path.
4. **Local and hosted isolation can share an artifact contract.** Local/self-
   hosted Apps use a deny-network `workerd` subprocess with a random-token
   loopback bridge; cloud Apps use Cloudflare Dynamic Workers and tenant-
   separated isolate keys.
5. **Source publication is a lifecycle.** HTTPS Git and local-directory
   sources move through bounded discovery, bundle, collect, projection, no-op,
   failure, deletion, and tombstone behavior. The persisted source states are
   pending, published, up-to-date, and failed; a ref mismatch is reported as a
   failed publication diagnostic rather than a distinct lifecycle state.

Those are meaningful substrate results. They make the June characterization
of Executor as only an MCP catalog too narrow. The prior E2E audit remains
useful for its Target/capability/artifact test architecture, but it is not a
current whole-product description.

### What Executor does not prove yet

The Apps descriptor explicitly marks workflows, UI, and skills `not supported
yet`. The runtime `author_tool`/`create_workflow`/`skills.create` path, Git
promotion loop, reactive stores, rich-result handles, unified durable Run,
remote cores, typed meta-capabilities, and transitive `scope()` membrane are
vision, not this release.

Two source facts require particular caution:

- current v2 owner policy explicitly has no scope stack, while the vision
  describes strict `scope()` intersection; and
- current toolkits hold connection patterns and policy rules, while the vision
  describes future toolkits as pure curation separate from policy.

The implemented Apps bridge confines calls to declared integration roles and
caller-selected connections. It does not yet establish a captured author
authority ceiling that can never be widened at a later invocation. OpenAgents
must model and test that intersection before claiming object-capability non-
amplification.

Other boundaries stay open:

- authored integration clients return `Promise<unknown>`, not operation-typed
  results;
- source synchronization is operator-driven; repository webhooks, watchers,
  scheduled polling, and the envisioned deploy/pull promotion loop are not the
  implemented control plane;
- only direct `tools/<slug>.{ts,tsx,js,jsx}` files and root package/lock files are
  fetched, so ordinary multi-file source organization is not yet a full app
  model;
- publication source limits are good, but signature, SBOM, lockfile-integrity,
  publisher policy, dependency transparency, and reproducible-build receipts
  are absent;
- Git literal-host checks do not by themselves prove DNS-resolved private-
  address exclusion;
- tool rows and the active descriptor are not visibly switched in one storage
  transaction;
- org-wide source create/sync/delete has no Apps-local role check visible in
  the plugin handlers;
- `destructive` is stored but not projected as an independent core enforcement
  class, and author-declared `readOnly` affects the default approval hint; and
- Apps is not a stable public embedding package: the release allowlist omits
  it and its local/cloud runtime packages are private; Apps is wired into the
  local CLI/desktop, Cloud, and Docker host, but not the separate Cloudflare
  self-host plugin set at this snapshot.

These are reasons to strengthen the pattern inside OpenAgents, not reasons to
dismiss it.

### OpenAgents already owns the authority half

The relevant current OpenAgents code is materially beyond a blank slate:

- `@openagentsinc/khala-tools` already defines typed tool authority,
  availability, execution mode, input/output schema, permission mode,
  invocation, events, artifacts, public/private/redacted results, runtime
  services, dispatcher phases, turn call budgets, and bounded output.
- compiled Agent Definitions already carry allow/deny/ask toolsets, network
  and secret policy, triggers, lane, budgets, and escalation. Deny wins and
  unmatched authority fails closed.
- `@openagentsinc/mcp-contract` already defines authority classes, grants,
  scope refs, transport/config types, and high-risk classes, although its
  status still truthfully says phase-0 groundwork and no exposed runtime
  transport.
- Khala's concrete MCP client/server path already lists and invokes external
  tools through policy-filtered projections. This should converge with the
  shared MCP contract before Executor becomes another catalog source.
- Desktop's extension lifecycle already gives MCP servers, plugins, and skills
  one public-safe declare/validate/grant/revoke audit, while truthfully marking
  current provider disagreement. It is a projection, not yet an owning
  generation state machine.
- Runtime Gateway and mobile already project and decide typed provider
  questions, approvals, and plan reviews without putting runtime credentials
  in the renderer.
- local and managed sandbox provider interfaces, cloud workroom policy, exact
  usage, Blueprint/Fleet runs, Sync events, and receipt vocabulary already
  exist.

The missing slice is therefore specific: OpenAgents lacks Executor's
source → bundle → deterministic collect → staged generation → isolated
capability bridge → canonical catalog loop for user- or agent-authored tools.

### Adapt-now amendment

The original “Adapt now” catalog decision should be read with the following
amendment:

1. **Add `AuthoredCapabilityArtifact`.** One browser-safe Effect Schema should
   name immutable artifact/version, publisher/owner, source ref and hashes,
   license/signature/provenance, toolchain, input/output Schema identity,
   requested capability roles, effect/egress ceiling, compatible runtime,
   budgets, and prior/superseding generation.
2. **Add a Capability Broker.** At invocation, resolve declared roles into
   owner-, WorkContext-, run-, account-, scope-, and expiry-bound handles.
   Effective authority is the intersection of the parent grant, artifact
   declaration, selected connection, organization policy, and execution-
   profile ceiling. No term can widen inward.
3. **Add a brokered-function-isolate profile.** This sits between projection-
   only and workspace-bounded shell execution: no ambient filesystem, process,
   environment, network, or secret; only the canonical tool bridge, bounded
   dependencies/CPU/time/memory/nested calls/output/logs, and an effective
   containment receipt. `workerd` is a strong adapter candidate, not the
   product contract.
4. **Add a staged source compiler.** Fetch a pinned Git/local source, verify
   redirect and network policy, discover, bundle in an isolated worker,
   collect schemas twice, validate capability requests, content-address the
   descriptor, show authority/behavior diff, stage, approve if needed, then
   atomically activate. Update, revoke, tombstone, and rollback are tested
   lifecycle transitions.
5. **Project into the canonical Khala registry.** Authored tools reuse the
   current dispatcher, compiled Agent Definition policy, approvals, events,
   redaction, accounting, cancellation, and Run/Work Unit receipts. There is
   no Apps-only dispatcher or workflow store.
6. **Capture catalog generation at advertisement.** A model executes the tool
   generation and authority manifest it saw. Source sync cannot silently
   replace a described operation before call.
7. **Keep semantic routing.** Adapt exact namespace enumeration and lazy
   search/describe/execute. Do not copy Executor's default token/substring/
   field-weight ranking; user-facing tool selection remains the central typed
   semantic selector or embedding/structured-planner path required by the
   workspace invariant.
8. **Make nested effects visible.** Each inner operation is a child invocation
   with idempotency class, policy/approval, private durable events, accounting,
   and receipt lineage. Parent completion does not erase partial side effects.

This is a bounded extension/catalog lane under ordered consequence 6. It does
not supersede the current protocol, cross-device conversation, foreign-host,
Fleet, or packaging sequence.

### “Adapt later” refinement

The original deferral of complex app-generation/artifact studios remains
correct for:

- generative UI and durable dashboards;
- broad internal-app studios;
- dynamic integration plugins;
- reactive agent KV/SQLite/filesystem stores;
- remote-core federation; and
- durable workflows whose crash/retry/compensation semantics are not proven.

One narrow item moves earlier: **typed tool authoring and publication is a
catalog/runtime primitive, not a full app studio**. It can be built and proven
with one read-only composite and one approval-gated mutating composite without
waiting for UI generation or a general workflow engine.

OpenAgents' Agent Definition trigger, budget, escalation, scheduler, and
durable run contracts already exist. Authored tools should become capabilities
callable by that workflow authority. Executor's future workflow vision must
not create a second scheduler or run truth.

### Component-ledger amendment

The signed compatibility ledger in Adapt-now 7 should additionally bind:

- source repository and exact commit/tree digest;
- source and dependency lock digests;
- authored bundle and descriptor digests;
- compiler, bundler, runtime, and driver versions;
- Schema/catalog generation;
- required integration kinds and compatible adapter generations;
- requested versus effective authority and containment profile;
- publisher/license/signature/transparency evidence;
- activation, revocation, supersession, and rollback state; and
- last-known-good execution evidence.

Git is source and distribution, not trust. A reachable or updated repository
does not authorize organization-wide activation.

### Pylon, clients, and web amendment

Pylon should host the compiler/broker/isolate lifecycle behind the existing
runtime protocol, or route it to managed Cloud through the same contract. It
must not pass SDK clients or credentials into authored code. Every run captures
the exact artifact and leaf-tool generation and preserves interruption through
the isolate and every nested call.

Desktop and `openagents.com` may expose source connection, sync/build
diagnostics, requested roles, authority diff, staged generation, enable,
revoke, update, and rollback. The renderer receives serializable public-safe
state and typed commands, never a runtime handle, Git token, provider secret,
or arbitrary source filesystem capability. Mobile receives approval, status,
revocation, and receipt projections; it does not silently gain local install
authority.

Effect Native scope mapping is:

| Scope | Authored-capability responsibility |
| --- | --- |
| Process | source registry, compiler/runtime identity, semantic catalog index, component ledger |
| WorkContext | eligible integrations/accounts, organization policy, artifact visibility, containment choices |
| Conversation/run | captured artifact/catalog generations, selected bindings, budgets, parent receipt lineage |
| Request/command | decoding, idempotency, approval, nested-call accounting, transaction, events, receipt |
| Foreign host/view | compiler worker and authored-code isolate lifecycle, diagnostics, preview/editor UI |

### Direct-consumption boundary

Near-term direct consumption should be interoperability only, with the two
Executor surfaces kept distinct:

```text
HTTP leaf mode
  authenticated Executor HTTP catalog
    → OpenAgents-owned immutable catalog snapshot and policy
    → versioned Executor HTTP invocation
    → normalized result and foreign-provider evidence

MCP meta-tool mode
  OpenAgents WorkContext and policy
    → scoped Executor `execute` / `skills` / `resume` meta-tools
    → Executor-owned lazy leaf selection and execution
    → normalized result and foreign-provider evidence
```

Executor MCP does not enumerate the leaf catalog. The HTTP adapter may import
bounded leaf descriptors and capture them in an OpenAgents-owned catalog
generation; the MCP adapter advertises only the compact meta-tools and must not
invent a remote leaf generation. Both adapters pin endpoint/protocol version,
map their admitted surface into OpenAgents authority classes, preserve outer
policy and cancellation, bound/redact payloads, expose auth/stale/offline/
incompatible states, and label Executor credential/containment facts as
provider-asserted unless backed by verifiable receipts. Executor results never
directly authorize settlement, payout, deployment, or public claims.

This initially consumes tools that an Executor operator has already configured
and published. Apps source management is HTTP/console-driven at this snapshot;
the MCP `author_tool` path is vision. Direct interoperability is therefore not
yet an agent-facing OpenAgents authoring and publication route.

A narrow package spike may compare `@executor-js/codemode-core` or its QuickJS
adapter behind the same OpenAgents execution contract. It does not make
Executor's SDK, storage, plugin, or owner model authoritative. The Apps package
and its private runtime packages are not a supported core dependency at this
snapshot.

### Verification packet

Before promoting authored tools into an owning roadmap gate, require one
bounded packet:

1. a read-only composite spanning two fixture integrations and caller-selected
   accounts;
2. a mutating composite whose nested effect requires the existing durable
   Desktop/mobile approval path;
3. undeclared-role, wrong-integration, cross-owner, secret, network,
   filesystem, process, and authority-widening denials;
4. source-update-between-advertisement-and-call refusal;
5. runtime kill, cancellation, retry, and idempotency evidence with partial
   effects represented honestly;
6. activation, revoke, deleted-tool tombstone, update-conflict, and rollback;
7. local and managed isolate conformance with distinct effective containment
   receipts; and
8. the optional external Executor adapter driving the exact same outer Khala
   tool contract for comparison.

Per the standing rule, this addendum is still design evidence. Promotion into
implementation authority happens through the owning Effect Schema contracts,
invariant/model note, roadmap gate, issue, tests, and receipts.

## OpenChamber whole-product addendum (2026-07-12)

The commit-pinned
[OpenChamber v1.16.0 teardown](./2026-07-12-openchamber-product-teardown.md)
strengthens the product argument for a persistent coding workroom while
sharpening the durability boundary.

OpenChamber now demonstrates a coherent end-user system across web/PWA,
Electron, Capacitor mobile, and VS Code: dense project/worktree/session
navigation, a typed turn/tool/blocker timeline, adjacent Git/files/diff/PTY
review, branchable history, multiple runtime hosts, one-time pairing, outbound
E2EE relay, notifications, dictation/read-aloud, scheduled sessions, and
server-owned Session Goals. The correct OpenAgents adaptation is the whole
workroom and attention model, not isolated components or a larger settings
surface.

Two implementation lessons move into the adapt-now evidence set:

1. OpenChamber's measured streaming work validates touched-field reducer
   updates, store/program separation by change frequency, event-class-aware
   coalescing, explicit reconnect/backpressure/failure state, and a fault corpus
   for duplicate/full-part races, offline/hidden recovery, session switching,
   and pending blockers. OpenAgents should encode those cases through its
   schema-valid Effect reducers and Runtime Gateway/Khala Sync subscriptions,
   not copy Zustand.
2. Unattended work needs server-owned blocker reconciliation. OpenChamber's
   permission runtime persists policy, respects the nearest explicit ancestor,
   lets child deny override parent allow, fails closed on unknown lineage, and
   reconciles pending requests after startup/reconnect. OpenAgents should apply
   that ownership to its scoped `runtime_interaction` authority without copying
   blanket session-level auto-accept.

The goal and scheduling implementation also adds a negative requirement.
OpenChamber persists goal/task metadata but keeps goal quiet timers, in-flight
continuations, scheduler timers, queues, and running claims in process memory.
The goal loop performs no startup scan/backfill, so an already-idle active goal
can stall after restart; a crash between persisted accounting and asynchronous
prompt submission can leave a missing continuation. The scheduler skips
downtime occurrences and has no durable run lease. Therefore:

- metadata persistence must never be presented as execution recovery;
- exact interrupted-turn recovery (#8744) remains distinct from a next-turn
  objective loop;
- a future goal contract needs startup enumeration, durable continuation
  rows/outbox, idempotency, lease generation, reconciliation, and typed terminal
  outcomes;
- deterministic requirements and evidence refs outrank model audit prose; and
- scheduled autonomous work must use the existing durable run/work/attempt
  authority rather than creating a second process-local scheduler truth.

For the parallel audio program, harvest OpenChamber's sequence/ACK dictation,
contiguous acknowledgement, client retry retention, explicit finalization,
partial/final UI, and accept-partial/retry controls. Keep OpenAgents' existing
Google STT/TTS, persistent stream generation, consent/retention, Effect/Rust,
typed command, barge-in, and raw-media-exclusion decisions. OpenChamber Voice
Mode is dictation plus read-aloud, not proof of durable full-duplex voice.

The rejection list gains four concrete entries: no Capacitor/WebView product
architecture, no renderer-visible runtime credentials or unsandboxed generic
invoke bridge, no duplicated TypeScript/JavaScript security protocol, and no
model verdict as acceptance authority. As with every teardown, these are design
inputs; current schemas, invariants, roadmap gates, issues, tests, and receipts
remain authoritative.

## T3 Code addendum (2026-07-13)

The [T3 Code teardown](./2026-07-13-t3-code-teardown.md) adds something the
reference set previously lacked: a shipping, well-distributed product occupying
almost exactly the OpenAgents P0 supervision lane — a local Effect server
wrapping Codex, Claude Code, Cursor, Grok, and OpenCode in parallel worktrees,
projected to web, desktop, and mobile clients, with remote access and phone
notifications, and substantially built by the agents it hosts.

Its evidence changes this document's decisions in the following ways.

1. **The harness-supervision control plane is now a contested, fast market —
   and the authority half is still unclaimed.** T3 shipped worktree-parallel
   multi-harness supervision, diff review, one-click PR flows, and mobile
   Live Activities in roughly five months at ~9.6k stars. Cursor, OpenCode,
   and T3 now all ship the supervision surface; none of the three ships
   authority manifests, effective-containment receipts, delivery receipts,
   host-portable sessions, or economic participation. This strengthens the
   differentiation section and raises the urgency of Desktop D1/D5 and the
   #8640-class live burn: the supervision features are converging table
   stakes, so OpenAgents wins on the trust half or not at all. No scope
   change; sequencing pressure only.
2. **A third independent Effect 4 beta whole-app adoption.** OpenCode V2
   pins `4.0.0-beta.83`, Executor `beta.59`, T3 Code `beta.78` (patched, with
   tsgo typechecking and a custom oxlint plugin enforcing Effect hygiene);
   the original 2026-07-13 addendum observed OpenAgents at `beta.70`, while
   the 2026-07-14 Effect Native v39 vendor pins `beta.94`. The Effect Native
   bet gains market confirmation, and the framework-risk warning in the
   Effect audit gains
   force: four serious products on four different betas of a pre-1.0
   framework is a compatibility archipelago. Keep the upgrade regression
   gates; do not adopt T3's patch-the-framework habit without an upstream PR
   attached.
3. **The event-sourced core is now triply convergent.** T3 independently
   reinvented the OpenCode V2 shape — typed commands validated by invariants,
   a decider, durable events carrying `commandId`/`causationEventId`/
   `correlationId`, SQLite projections, and queue-backed reactors emitting
   typed completion receipts that tests await instead of polling. That last
   pattern (receipts as deterministic test signals at the runtime seam) is a
   direct ergonomic reference for OpenAgents oracles. T3 also shows the
   omissions that matter: no durable admission before scheduling, no
   steer/queue delivery semantics, no replay-to-live marker. Adapt-now A–C
   stand unchanged; T3 is corroborating evidence, not a new requirement.
4. **The environment/endpoint vocabulary sharpens the portable-sessions
   pathway.** T3's `ExecutionEnvironment` / `KnownEnvironment` /
   `AccessEndpoint` / `AdvertisedEndpoint` model — with access and launch as
   deliberately separate concerns, Tailscale as a pluggable endpoint
   provider, and desktop-managed SSH reduced to a launch-plus-forward helper
   — is cleaner language than "remote server" and matches the workspace's
   existing Tailnet posture. But T3 threads are environment-local by design
   ("a local clone and a remote clone are different projects"), so
   host-to-host session movement with preserved identity, authority, and
   receipts remains unclaimed by every audited product. The Rev 30/31
   portable-session packets stand, now with sharper naming to borrow.
5. **DPoP-scoped local-server access supersedes the shared-password
   critique.** The OpenCode teardowns flagged a shared Basic password as the
   local-server credential weakness; T3 demonstrates the fix in shipping
   code: per-client capability scopes (`orchestration:read`,
   `terminal:operate`, …), RFC 8693-shaped token exchange from a bootstrap
   token, pairing links, and DPoP proof-of-possession binding. Pylon/Runtime
   Gateway socket exposure and Khala Sync device grants should adopt this
   pattern rather than inventing a parallel one. Bounded leaf under the
   existing R-gate/device-grant contracts.
6. **The authority inversion is the posture to refuse, now with a named
   incumbent.** T3 guards *access to the environment* with serious
   cryptography while defaulting *execution* to `approvalPolicy: never` +
   `sandboxMode: danger-full-access`, shipping no containment of its own and
   delegating everything to the wrapped harnesses. This is the
   market-leading open competitor normalizing default-YOLO. Reject
   explicitly, alongside Cursor's computer-use-by-default: OpenAgents
   execution profiles stay deny-by-default, owner-local danger mode stays
   explicit and visually persistent, and effective containment stays a
   receipt, not an assumption.
7. **Mobile ambient supervision gains a second incumbent proof.** T3's iOS
   Live Activities (lock-screen agent status fed by an APNs relay) joins
   Cursor's Remote Control as evidence that phone-side ambient agent status
   and steering is a differentiating surface. The OpenAgents mobile lane
   should treat lock-screen/notification presence for running fleet work as
   a natural post-parity leaf over existing Khala Sync projections — typed
   status, never completion authority.
8. **The agent-operated factory is public now.** 1,929 commits in five
   months, 277 agent-prefixed, a checked-in `.plans/` corpus, vendored
   framework source that agents are instructed to read, vouch-gated
   community PRs, and 3-hourly nightlies: T3 is the most complete public
   example of the software-factory operating model OpenAgents runs
   internally. This validates the Khala fleet direction and adds a concrete
   reference for agent-repo ergonomics (vendored reference source, plan
   corpora, machine-checked task-completion gates).
9. **A thin relay is compatible with local-first — if it stays thin.** T3's
   hosted layer (Clerk identity, `cloudflared` tunnels, APNs push; no
   execution, no session custody; direct ws/wss and SSH remain account-free)
   is structurally similar to the two-tier identity model this document
   already records as the R1 amendment: local-first by default, account as
   an opt-in upgrade. OpenAgents' version additionally carries receipts and
   durable cross-device truth through Khala Sync, which T3 does not attempt.
10. **The frontend's strongest transferable artifact is its shared projection
    kernel.** Web/Electron and mobile share environment-scoped Effect queries,
    explicit command scheduling, cache-first shell/thread state, HTTP
    snapshots, WebSocket resume cursors and sequence deduplication, typed
    cached/synchronizing/live phases, and schema-versioned platform
    persistence. This is the right service boundary for Effect Native to
    adapt—strengthened by OpenAgents' replay-to-live marker,
    acknowledgement/worker-epoch rules, authority classes, and receipts. It is
    substantially more valuable than copying the visible chat shell.
11. **Renderer reuse stops below the UI layer, which sharpens the Effect Native
    requirement.** Electron hosts the exact React/Vite web renderer behind a
    hardened preload bridge; Expo mobile independently implements navigation,
    theme tokens, composer, Markdown, diff, terminal, controls, and adaptive
    layout. The latter proves that native specialization matters, but also
    leaves two design systems and two implementations of the most complex
    surfaces. OpenAgents should retain one typed component/token/intent
    contract with web, React Native, native, and canvas renderers—not adopt
    T3's independent Tailwind DOM and Uniwind/native trees. This does not ban
    React below that contract: the
    [React web renderer harmonization analysis](../effect-native/2026-07-14-react-web-renderer-harmonization-gap-analysis.md)
    recommends React as an internal DOM renderer, with Base UI and specialist
    libraries confined to reviewed lowerings/typed hosts rather than a second
    app grammar.
12. **Conversation rendering is infrastructure.** T3's virtualized and
    end-anchored Legend List feed, visible-content preservation, bounded
    Markdown highlight cache, sanitized rich text, worker-backed diff parsing,
    lazy secondary panels, and persistent Electron webview host are direct
    references for Desktop/mobile performance contracts. Copy those mechanics
    behind Effect Native foreign hosts, then add browser/device oracles for
    focus, keyboard, screen-reader terminal output, reduced motion, contrast,
    memory, and bundle budgets—the pinned source has material gaps in each of
    the first five.
13. **The state split is sound but needs a stricter ownership ledger.** T3
    mostly keeps server facts in Effect projections and drafts/layout in
    Zustand, yet command acknowledgement, optimistic UI, cache freshness, and
    cross-window persistence are partly hand-composed inside feature modules;
    `ChatView`, `Sidebar`, and the draft store have grown into multi-thousand-
    line integration boundaries. OpenAgents should declare owner, persistence,
    freshness, invalidation, acknowledgement, and disposal for every feature
    state; keep domain state out of renderer stores; and decompose through
    typed intents/services before adding a second renderer.

Per the standing rule, none of these items is authority here: each lives or
dies by its owning roadmap gate, issue, or contract when promoted.

## Crabbox addendum (2026-07-13)

The [Crabbox teardown](./2026-07-13-crabbox-teardown.md) adds the reference
set's first subject on the execution-infrastructure seam *under* the agent
products: a Go CLI plus optional Cloudflare Durable Object or Node/PostgreSQL
coordinator that leases runners across 77 provider adapters, syncs the
working-tree diff over a direct SSH data plane, runs commands, records durable
run evidence, and releases — with no agent engine, no UI ambition, and an
explicitly documented non-sandbox trust model. It overlaps OpenAgents' own
remote-execution surfaces (`crates/oa-codex-control`, the GCE capacity-lease
contracts, Pylon no-spend assignments) more directly than any prior subject.

Its evidence changes this document's decisions in the following ways.

1. **The lease/evidence seam is now a validated standalone market layer —
   and the settlement half is still unclaimed.** Crabbox shipped honest
   lease lifecycle semantics (`expiresAt = min(ttl, idle)`,
   reserved-versus-estimated cost, heartbeat-touch, fail-closed cleanup that
   never marks a machine gone while it may exist) and durable run evidence
   in ten weeks at ~1.1k stars. It stops exactly where the OpenAgents thesis
   begins: its signed run receipt prints `trust=self-signed`, carries no
   countersignature, containment class, or settlement fields. Strengthens
   the receipts differentiation line; sequencing pressure on the
   Cloud-crate receipt lanes (`resource_usage_receipt.v1` and the closeout
   contracts), no scope change.
2. **Evidence verbs answer the runbook's own recorded gaps.** Crabbox's
   early `run_` handle plus `attach`/`events`/`logs`/`results`/`history`
   over durable phase-tagged events is the exact ergonomics the Khala→Pylon
   delegation runbook lists as missing (silent `assignment run-no-spend`,
   no live progress, raw D1 queries as proof reads). Adopt as a typed
   assignment-evidence command family; owning lane: the Pylon/Khala
   delegation CLI and its closeout contracts.
3. **The credential-destination provenance lattice is a new idea worth
   importing.** Crabbox types the trust class of every configuration source
   (trusted file / repository / environment / flag) and refuses to let
   repo-sourced config select destinations for higher-trust credentials —
   and names that cross-trust routing an in-scope vulnerability class.
   OpenAgents types the *grant* (broker-only, per-turn, fail-closed) but not
   the *config-source trust class* that selects destinations. Composes with,
   never replaces, the broker-only invariant; owning surfaces: Pylon config
   loading, Desktop settings ingestion, `docs/cloud/INVARIANTS.md`.
4. **The coordinator-as-credential-holder is the custody posture to
   refuse, now with a well-built incumbent.** Crabbox's coordinator owns
   raw provider keys for five clouds in runtime secret env; its data plane
   bypasses the broker and runners stay credential-free, but control-plane
   compromise mints and destroys infrastructure. This is a second named
   incumbent (after T3's inverse authority posture) confirming the
   broker-redeemed-grant invariant as differentiation, not overhead. Reject
   explicitly; owning contract: the capability-broker invariants in
   `docs/cloud/INVARIANTS.md`.
5. **Ownership-proof lifecycle discipline gains market corroboration.**
   "Labels, names, and IDs alone are not ownership proof"; adoption never
   silently retargets a bound claim; sweeps touch only exact retained
   resources; cleanup fails closed on inventory failure; `cleanup` refuses
   to run beside a coordinator. This matches and extends the
   oa-codex-control "never leak a running instance" / idempotent-verified-
   release rules — promote the ownership-proof rule to an explicit tested
   invariant in the placement/cleanup contracts.
6. **Failure capsules are a cheap evidence artifact the fleet lacks.** A
   portable, replayable bundle capturing a failing run (`capsule
   from-actions` / `capsule replay`) makes failures reproducible objects
   instead of prose. Natural post-parity leaf on the assignment-evidence
   lane (item 2), receipted like any artifact.
7. **Release-gate separation gains a second precedent, including a blocked
   release.** Verification from protected-default code, credential-free
   candidate builds, pinned signer policy, byte-equal release notes,
   separately authorized publication — and a real `v0.37.0` tag
   publication-blocked over an ad-hoc re-signing trust defect. Strengthens
   the DMG-1 notarize/staple/fail-closed lane and the signed component
   ledger: a missing or broken trust chain blocks the release, full stop.
8. **Honest non-sandbox postures are becoming the market norm; containment
   truth stays unclaimed.** Crabbox states in `SECURITY.md` that isolation
   is a provider attribute and single-user trust is the boundary — more
   honest than T3's silent default-YOLO, but its evidence still never
   records which isolation class produced a result. Execution profiles,
   fail-closed negotiation, and effective-containment receipts remain the
   OpenAgents seam; no incumbent has claimed it.
9. **A credits-gateway competitor is forming on the compute side.**
   Crabbox's marketplace skeleton ([vision]: quote/status APIs shipped;
   ledger, capture, settlement explicitly not) is an OpenRouter-shaped
   gateway for sandbox capacity — adjacent to the OpenAgents compute-market
   thesis. Watch item for the market lanes; OpenAgents' version keeps
   adversarial tenancy, usage-truth pre-spend, and settlement receipts in
   the design from the start rather than growing them out of a
   trusted-team model.

Per the standing rule, none of these items is authority here: each lives or
dies by its owning roadmap gate, issue, or contract when promoted.

## Grok Build addendum (2026-07-15)

The [Grok Build teardown](./2026-07-15-grok-build-teardown.md) adds the
reference set's strongest open implementation of the terminal as a durable
agent application platform: a Rust Agent Client Protocol runtime and
process-shared leader under
full-screen and native-scrollback TUI modes, headless automation, dashboards,
persistent sessions, worktrees, tools, subagents, permissions, telemetry, and
runtime-aware updates. Its unusually deep emulator/PTY, signal, resize,
clipboard, leader-race, updater-race, scenario, fuzz, and performance structure
turns terminal behavior into product architecture rather than ANSI polish.

Its evidence changes this document's decisions in the following ways.

1. **The terminal is a foreign host with two legitimate presentation
   strategies.** Grok's full-screen renderer and native-scrollback renderer
   share semantic session state while making different ownership choices for
   history, live content, focus, selection, and host scrollback. OpenAgents
   should adopt one typed transcript/action/effect projection with multiple
   terminal lowerings, not a terminal-specific conversation engine. Terminal
   modes, resize, paste, clipboard, mouse, focus, image capability, crash
   cleanup, and finalized-block commit become explicit host contracts.
2. **A shared local leader is the right precedent for Pylon, with a stronger
   trust boundary.** Grok makes socket/lock discovery, backend identity,
   protocol and binary version, client capabilities, reconnect, stale-leader
   eviction, bounded drain, update relaunch, and session reload visible product
   state. OpenAgents should adapt that lifecycle and add peer identity or a
   protected per-generation client secret, owner-only endpoint creation,
   bounded mailboxes, overload responses, and receipts. Ambient filesystem
   permission around a local socket is not sufficient authority.
3. **Agent Client Protocol belongs at the compatibility edge, not at the
   center of the domain.** Grok proves the protocol can let TUI, headless,
   editor, and filesystem/terminal-owning clients share one runtime.
   OpenAgents should control Grok through an outbound client adapter backed by
   the same Runtime Gateway command processor, authority compiler, event log,
   and receipts as every first-party provider while keeping
   Thread/Turn/Item/Work Unit/Receipt canonical and generated. xAI or
   OpenAgents extension structs must not become a second state machine.
4. **Reconnect ordering is worth copying; durability names need tightening.**
   Grok's stable event IDs, cursor reconnect, full-replay fallback on any
   idempotency gap, and flush/gate/replay/delta-replay ordering are strong.
   Its JSONL readers also bound torn tails, quarantine or skip corrupt records,
   and repair partial presentation state. But ordinary flush acknowledgements
   are not consistently file-plus-directory `fsync`, write errors often warn
   and continue, and historical dual-writer corruption is acknowledged. Keep
   accepted, flushed, replay-ready, and power-loss-durable as separate typed
   outcomes; enforce one writer/lease per session.
5. **Retry ownership should be explicit by layer.** Grok gives transport retry
   to the sampler/request task and keeps compaction, auth refresh, conversation
   resubmission, and work disposition in the session actor, with cancellation
   and stream-drain barriers at the boundary. Adopt the taxonomy so two layers
   never retry the same side effect. Prefer bounded task actors on a shared
   runtime for the default OpenAgents scale curve; Grok's dedicated OS thread
   and 8 MiB stack per resident session is an evidence-backed isolation choice,
   not a free default.
6. **Queue/send-now is excellent delivery UX, not a substitute for durable
   admission.** Grok clearly distinguishes queue-after-current-turn from
   cancel-and-send-now and projects session-scoped queue events. OpenAgents
   should keep that interaction while retaining the stronger admit-first,
   client-chosen identity, exact-retry reconciliation, steer-at-safe-boundary,
   queue-until-yield, and worker-generation contract established by OpenCode
   V2 and the existing guarantees.
7. **The process-local dashboard validates the fast cockpit, not durable Fleet
   truth.** Roster, activity, reply, question, queue, interrupt, mode, usage,
   and transcript navigation show that dense multi-agent supervision belongs
   in the terminal too. The dashboard disappears with the pager process and a
   returned child summary proves neither delivery nor acceptance. OpenAgents
   should drive its low-latency roster from the persisted agent/work graph and
   keep review, commit, push, merge, acceptance, payout, and settlement as
   distinct outcomes.
8. **Conflict-aware rewind and typed worktrees are now a stronger combined
   reference.** Prompt-indexed snapshots, comparison with the last
   agent-produced state, external-modification categories, cross-compaction
   replay, Git/jj adapters, create/apply/remove/resume, copy modes, and pinned
   refs are all engine operations. Adapt them with stage/inspect/commit/clear,
   a redo baseline, ownership and generation checks, irreversible-effect
   disclosure, and a receipt for every partial restore. An isolation request
   must fail when worktree setup fails, never silently reuse the shared
   workspace.
9. **The typed tool runtime is a protocol asset; declared scope is not
   containment.** Grok separates typed arguments/results, progress and exactly
   one terminal outcome, wire/model/UI projections, concise discovery,
   registration generations, admission limits, cancellation races, and
   external-process lifecycle. OpenAgents should adopt those invariants while
   requiring authority scope at registration, failing closed on unknown or
   unresolved capability names, propagating exact deadlines, cancelling remote
   work on dropped consumers by default, showing exact MCP arguments, and
   binding catalog identity to execution and receipt lineage.
10. **The permission and sandbox gaps sharpen the authority compiler contract.**
    Deny-before-ask-before-allow, managed clamps, shell-segment parsing, folder
    trust, and OS policy compilation are useful. Prefix allow rules, non-
    canonical direct paths, heuristic unknown-tool classification, fail-open
    hooks, inherited subprocess environments, sandbox-off default, platform-
    unequal network isolation, and warn-and-run enforcement failure are the
    counterexamples. OpenAgents keeps canonical structured effects, exact
    approval arguments, parent-intersected authority, scrubbed subprocess
    environments, named fail-closed profiles, and effective-containment
    receipts. Advisory fail-open automation hooks and mandatory fail-closed
    policy hooks must be different types.
11. **Subagent provenance is good; non-amplification and admission remain the
    missing laws.** Grok persists independent child sessions, parent/prompt/
    session provenance, coarse capability modes, depth clamps, foreground and
    background cancellation semantics, and dashboard control. Children also
    inherit broad parent runtime resources, unresolved allowlists can preserve
    the full tool set, no explicit global child-admission bound was found, and
    worktree isolation can degrade. OpenAgents must compute child authority as
    an intersection, reject unresolved capabilities, bound concurrency, and
    record placement/containment rather than infer it from role labels.
12. **Exporter-side validation and coordinated relaunch are both worth
    importing behind stricter ledgers.** Grok's customer-OTEL double opt-in,
    content-free default, separate prompt/tool gates, closed typed vocabulary,
    and final exporter validation are the right observability firewall. Its
    versioned staging, atomic publication, concurrent-update convergence,
    bounded drain, leader relaunch, reconnect, rollback, and cleanup are the
    right runtime transaction. OpenAgents should add one comprehensible data-
    flow/retention matrix and require signed manifests, artifact digests,
    publisher identity, provenance, compatibility ranges, platform signatures,
    last-known-good state, and update/rollback receipts.
13. **A deep test architecture without a public gate is evidence and a
    warning.** The real-binary PTY harness, terminal emulator, declarative
    scenarios, real signals, clipboard, resize storms, leader clusters,
    updater races, fuzzing, and benchmark targets set a new verification bar.
    The public export omits CI/release workflows and checked-in platform
    performance baselines, while many real-host tests are ignored. OpenAgents
    should publish the exact platform matrix, baselines, commands, and retained
    receipts; source transparency does not substitute for release provenance.
14. **Mature terminal UX does not justify unchecked architectural scale.** The
    export is roughly 1.35 million Rust lines across 79 workspace members, with
    several massive modules and numerous unbounded channels. Copy the seams and
    invariants, not the size: every new scope needs an ownership law,
    dependency test, backpressure budget, observability boundary, and deletion
    gate.

Per the standing rule, none of these items is authority here: each lives or
dies by its owning roadmap gate, issue, or contract when promoted.

## Command Code addendum (2026-07-16)

The [Command Code teardown](./2026-07-16-command-code-teardown.md) adds the
reference set's first product whose differentiated system is a continuously
learned coding-preference plane. The public repositories do not contain its
engine source, so the audit used the official docs and changelog plus an
isolated install of the closed `command-code@0.51.0` npm package and its
source-mapped VS Code extension.

The evidence changes this document's decisions in the following ways.

1. **Learned preference is a separate product plane, not generic memory.**
   Command Code distinguishes explicit `AGENTS.md` instructions, inferred
   `taste.md` preferences, settings, and session history. OpenAgents should
   preserve that separation in its context envelope and product language.
2. **The restricted compiler boundary is worth adapting.** Ordinary coding
   tools cannot edit Taste directories. A separate learning agent writes only
   validated root/category `taste.md` files beneath a path-confined output
   tree. OpenAgents should use a named governed compiler rather than let the
   ordinary agent silently self-edit durable preferences.
3. **Portable text is useful but insufficient authority.** Human-readable,
   diffable Markdown packages are an excellent interchange and review format.
   Canonical state still needs stable ids, source observations, compiler/model
   version, applicability, confidence calibration, freshness, conflicts,
   owner disposition, activation, and outcome.
4. **Correction sources should be explicit jobs.** Git-history mining and
   Claude Code/Cursor/Codex session import are valuable onboarding paths.
   Each needs preview, bounded source/date/repository scope, custody policy,
   cancellation, deletion, and an import receipt. No ambient scan of provider
   histories is authorized.
5. **Preference cannot amplify authority.** A learned rule may rank or shape
   behavior only inside the current admitted tool, filesystem, execution,
   egress, account, spend, publication, and approval envelope. It cannot
   select a broader target, grant a tool, suppress an interaction, or convert
   agent completion into review, acceptance, release, payout, or settlement.
6. **“Local” must be decomposed.** Command Code stores Taste Markdown locally
   but the bundle also sends prompt batches and compiled correction context to
   its hosted generation endpoint. OpenAgents must disclose local artifact
   custody, local derivation, remote inference processing, cloud sync, and
   training use as separate facts.
7. **A learning feed is not governance.** Show evidence, scope,
   confidence/freshness, and approve/narrow/suspend/delete actions. Do not use
   celebratory live observations as a substitute for review or consent.
8. **History and preference discovery remain semantic-selection problems.**
   Applicability and retrieval must use a central typed semantic selector,
   embedding search, or structured planner. Do not add keyword rules for
   user-facing preference routing.
9. **The smaller host patterns corroborate existing decisions.** Read-only
   headless defaults, stable exit classes, per-session model restore,
   headless-history separation, fork provenance, three-mode rewind, bounded
   IDE IPC, progressive skill disclosure, and monitored command deltas are all
   worth retaining under OpenAgents' stronger runtime contracts.
10. **The security and durability gaps strengthen existing rejections.** The
    client rewrites complete session JSONL with regenerated record ids, has no
    established local OS containment, exposes a prompt bypass, collects an
    authenticated persistent machine fingerprint beyond its telemetry
    disclosure, and self-updates without a visible signed component ledger.
    Keep append-only admission/evidence, stable refs, explicit containment,
    privacy-purpose contracts, signed updates, rollback, and receipts.

The resulting proposed pathway is a **governed preference plane** after the
core Desktop and portable-session loop is trustworthy:

```text
admitted observation
  -> candidate preference + evidence
  -> owner review / bounded activation policy
  -> versioned preference generation
  -> exact turn/work-unit application with “why”
  -> outcome or correction
  -> reinforce / narrow / suspend / supersede / delete
```

This addendum does not promote the pathway into current implementation status
or sequencing. Any load-bearing requirement must move into the owning roadmap
gate, typed contract, privacy policy, issue, tests, and receipts before the
product can claim continuously learned preference.

## T3 Code Agent Client Protocol implementation addendum (2026-07-16)

The
[T3 Code Agent Client Protocol implementation teardown](./2026-07-16-t3-code-agent-client-protocol-implementation-teardown.md)
adds the reference set's strongest TypeScript/Effect implementation of the
protocol needed to control Grok through `grok agent stdio`, Cursor through
`agent acp`, and other compatible coding agents. It audits T3's generated
`effect-acp` package, custom bidirectional Effect RPC transport, client and
agent facades, structured errors, shared session runtime, Cursor/Grok launch
and extension layers, event projection, restore gate, hardening history, and
tests. It also verifies current Grok packages, current official schema and SDK
versions, registry scope, and OpenAgents' existing Grok fixture.

The evidence changes this document's decisions in the following ways.

1. **The required product is an Agent Client Protocol client.** OpenAgents
   hosts the Grok and Cursor ACP subprocesses and controls them through
   initialize, authentication, session, prompt, update, cancellation, and
   negotiated reverse methods. Exposing OpenAgents itself as an ACP agent is
   out of scope.
2. **Grok is using the standard protocol family.** Current Grok source pins
   Rust `agent-client-protocol` 0.10.4 with unstable features, resolves
   `agent-client-protocol-schema` 0.11.4, wraps it in `xai-acp-lib`, implements
   `acp::Agent`, and negotiates wire version 1. The published JavaScript hello
   path is not a separate xAI protocol.
3. **`schema-v1.19.0` should consume Grok's documented core path.** It also
   describes stable wire version 1 and contains `initialize`, `authenticate`,
   `session/new`, `session/prompt`, and `session/update`. Ship confidence still
   requires a pinned real-Grok compatibility fixture because xAI extensions
   and older unstable methods sit above that stable core.
4. **One schema does not erase peer profiles.** Wire compatibility comes from
   `protocolVersion`; optional behavior comes from capabilities; generated API
   compatibility comes from the schema or SDK artifact. The current registry
   spans Cursor, Codex, Claude, Gemini, Copilot, OpenCode, Qwen, Goose, and
   other agents, but every peer still needs launch/install/auth, capability,
   extension, and conformance metadata. Grok is source-compatible even though
   it is not currently in the curated registry.
5. **Copy T3's layering, not its pinned code.** T3 cleanly separates generated
   schema, bidirectional wire transport, typed facades, session lifecycle,
   canonical event projection, and provider quirks. Its pinned `v0.11.3`
   unstable schema predates `schema-v1.19.0`; OpenAgents starts from current
   stable wire version 1 and gates only the unstable families required by a
   named peer.
6. **Bidirectionality is the security boundary.** An agent can call the client
   for permissions, files, and terminals while a prompt is outstanding. Those
   calls enter Runtime Interactions and brokered workspace capabilities; they
   never become direct renderer or peer authority. Capabilities default false
   until the handler, authority compiler, cancellation, bounds, evidence, and
   tests all exist.
7. **OpenAgents' current Grok code is a fixture, not the foundation.** It proves
   initialize/auth/new/prompt and minimal update projection, but cannot answer
   reverse requests, kills the process to interrupt, loses most native update
   types, and advertises file/terminal capabilities it does not implement.
   Replace it behind the shared package and make those capabilities false
   before describing the path as general Agent Client Protocol support.
8. **Lifecycle and transport laws must be stronger than T3's private
   adapter.** Reuse idempotent startup, prompt serialization, session cancel,
   generation checks, update merging, drain barriers, and replay/live gating.
   Add bounded queues and pending maps, overload outcomes, request and session
   cancellation, deadlines, late-response policy, partial-write failure,
   scoped handlers, restart repair, and exact connection/session generations.
9. **Native fidelity and safe diagnostics are separate stores.** Retain the
   complete decoded native envelope in a bounded private evidence plane before
   portable projection, with explicit loss accounting. Prompts, file bodies,
   terminal output, headers, auth material, and error data never enter public
   receipts or ordinary diagnostics.
10. **The implementation order is Grok-and-Cursor-first and concrete.**
    Pin/generate current stable schemas and manifests; build the bounded
    bidirectional connection and conformance fixtures; replace the Grok fixture
    with the shared outbound client; implement separate Grok and Cursor peer
    profiles; prove auth, prompt/update, reverse requests, configuration,
    cancellation, resume, restart, and extensions against pinned binaries for
    each; then admit additional registry agents through separate peer profiles.

This addendum accepts Agent Client Protocol client support as an architecture
direction. Per the standing rule, the owning roadmap gate, typed contracts,
issues, tests, receipts, and public promise state still determine when Grok,
Cursor, or any additional peer is implemented or shipped. The
dependency-ordered work and independent provider release gates are tracked in
[#8887](https://github.com/OpenAgentsInc/openagents/issues/8887). The completed
[machine-readable release matrix](../../packages/agent-client-protocol-conformance/compatibility/release-matrix.json)
and [human proof ledger](../qa/2026-07-16-acp10-release-proof/README.md) now
derive the two provider claims independently. Both checked peers remain
experimental: real Darwin-arm64 prompt/restart evidence proves consumption,
while missing required live and cross-platform rows prevent general support
language.

## Factory Desktop and Droid CLI addendum (2026-07-16)

The
[Factory teardown](./2026-07-16-factory-desktop-cli-teardown.md)
adds the reference set's clearest closed commercial implementation of one
local coding-agent engine serving a terminal TUI, headless streaming JSON-RPC,
Desktop, SDKs, automations, integrations, Missions, and remote computers. The
application and CLI sources are not public, but the signed installed bundles
and commit-pinned TypeScript SDK expose the daemon lifecycle and protocol in
enough detail to change several decisions.

1. **One multi-client engine is now market-proven beyond the open reference
   set.** Factory Desktop embeds Droid and controls its authenticated daemon;
   `droid exec` exposes bidirectional JSON-RPC; the daemon multiplexes sessions
   for Desktop and integrations. OpenAgents should keep one Runtime Gateway
   command processor and durable Thread/Turn/Item/Work Unit/Interaction/
   Receipt graph for terminal, Desktop, mobile, automation, and SDK clients.
2. **The local-supervisor contract needs more than loopback and a token.**
   Factory has startup, port/Unix-IPC selection, authentication, liveness,
   reconnect, diagnostics, parent monitoring, and explicit remote access. Add
   client-scoped authority, protected runtime generations, bounded queues,
   overload outcomes, exact retry, durable admission, and shutdown/update
   receipts. Non-loopback SDK connections must require a protected transport;
   an arbitrary plaintext `ws://` carrying an API key is not acceptable.
3. **Desktop should stay a projection, but renderer origin is part of the
   authority boundary.** Factory's context-isolated preload is capability-
   shaped and the engine remains outside the renderer. The shipped app also
   lacks an explicit top-level navigation denial and IPC sender/origin checks
   across a broad bridge, while its CSP remains permissive. OpenAgents keeps
   the thin host and must gate navigation, sender identity, schemas, message
   size, cancellation, and generation at every privileged IPC entry.
4. **Terminal/headless/daemon are host modes, not separate products.** Exact
   session identity, resume/fork, permissions and user-question callbacks,
   structured output, streaming updates, tool discovery, and cancellation
   belong in one generated client contract. Factory's TUI and headless breadth
   strengthens the Grok and Codex conclusions without replacing OpenAgents'
   canonical domain model.
5. **Hierarchical non-weakening policy is worth adapting.** Org, project,
   folder, user, and machine-managed pre-login layers compose model, tool,
   autonomy, hook, MCP, telemetry, and sandbox policy. OpenAgents should retain
   explicit precedence, locked higher-level keys, unioned mandatory denies,
   and child authority as an intersection. An unsafe prompt bypass can never
   override hard policy.
6. **Permissions and containment remain separate products.** Factory's
   Seatbelt/bubblewrap sandbox and fail-closed whole-process mode are useful,
   but sandboxing is opt-in and the default per-command mode leaves the main
   authority-carrying runtime outside the OS boundary. OpenAgents keeps named,
   default fail-closed containment profiles and records requested versus
   effective enforcement.
7. **Worktrees, rewind, and Missions belong to the engine.** Factory exposes
   them across CLI and Desktop with worker/validator state and remote-computer
   placement. Adapt the UX and protocol shape while keeping admission,
   authority intersection, external-conflict detection, review, acceptance,
   commit, push, publication, and cleanup as distinct receipted outcomes.
8. **Executable catalogs need immutable provenance before startup.** Factory
   plugins can carry hooks, agents, skills, and MCP processes. The isolated
   failed-auth run cloned the default moving marketplace before authentication
   completed. OpenAgents should require content identity, publisher,
   provenance, declared authority, compatibility, review, explicit activation,
   and rollback; discovery must not silently mutate executable state.
9. **Data-flow language must describe separate planes.** Factory's official
   pages conflict on default cloud session sync, transcript/tool storage,
   direct provider traffic, Factory metric export, optional analytics, and
   airgap behavior. OpenAgents must separately disclose local custody, local
   execution, remote inference, cloud sync, telemetry destination, retention,
   deletion, region, and training use.
10. **Release identity includes entitlements and component compatibility.**
    Factory's notarized ASAR-integrity-bound Desktop is positive evidence. Its
    production CLIs still allow debugger attachment, DYLD environment access,
    unsigned executable memory, and disabled library validation; Desktop and
    standalone Droid also moved on independent versions during the audit.
    OpenAgents should gate a deny-by-default entitlement manifest and a signed
    host/engine/protocol compatibility ledger with staged activation,
    last-known-good state, and rollback receipts.

Factory therefore strengthens the existing direction rather than adding a new
control plane: one durable engine behind multiple typed clients, with stronger
origin checks, containment, extension provenance, data-flow truth, and release
proof than the audited product exposes. As elsewhere in this document, these
lessons become requirements only when promoted into their owning contracts,
roadmap gates, issues, tests, and receipts.

## Amp Code addendum (2026-07-16)

The [Amp teardown](./2026-07-16-amp-code-teardown.md) adds the clearest current
example of a coding thread becoming a distributed collaboration and execution
object. Amp's closed Bun CLI attaches to actor-backed server state, runs local
tools, can become a remotely addressable runner, and projects the same thread
into web/mobile remote control and diff review. Its model router, Oracle,
specialist agents, cross-thread reader, and plugin-defined modes/agents make
the thread—not one engine binary or provider—the stable product object.

1. **Cross-thread reading should preserve evidence, not only memory.** Amp's
   dedicated reader searches original history after compaction, considers
   later revisions and reverts, and treats tool calls as attempts rather than
   outcomes. OpenAgents should add a bounded history-reader role over exact
   accepted event refs with supersession, review, and acceptance state.
   Compaction remains an orientation artifact and can never replace evidence.
2. **Queue, steer, and interrupt belong in the canonical command algebra.**
   Amp exposes all three in the TUI and carries steer into JSONL automation.
   Runtime Gateway and Sync should bind each to a stable command id, target
   generation, durable admission, ordering, requested safe boundary, origin
   client, and terminal outcome rather than infer intent from arrival time.
3. **A terminal process can be both client and worker without becoming
   product authority.** Amp's CLI, stream JSON, SDK, IDE bridge, web/mobile
   controller, runner-only mode, and Orbs converge on one thread. OpenAgents
   should keep the same host convergence behind a public generated protocol,
   portable local event/receipt store, and Desktop/mobile workroom authority.
4. **Specialist diversity should be explicit and receipted.** Amp routes
   review, retrieval, thread reading, media, compaction, and the Oracle
   separately; the Oracle deliberately changes model family when the primary
   model would otherwise match. Adapt that diversity while recording exact
   provider, model, prompt/catalog generation, cost, retention class, and
   outcome. A second opinion is advice, not independent verification.
5. **Review fan-out is a useful product projection of an Assurance Manifest.**
   Amp runs each Markdown review check in a separate subagent with a selected
   tool set. OpenAgents should preserve the one-check/one-observation ergonomics
   while compiling exact source, environment, adapter, budget, catalog, and
   evidence bindings. Check output cannot self-admit or self-accept.
6. **Runner identity is not enough placement identity.** A named Amp runner
   accepts remotely created work in its starting directory. OpenAgents must
   additionally bind repository revision/dirty policy, owner and account,
   authority, containment, network, secrets, component generation, lease/
   fence, capacity, and receipt destination before remote admission.
7. **Plugin breadth validates one extension grammar but raises the isolation
   bar.** Amp plugins add commands, tools, synchronized UI, lifecycle
   continuation, modes, agents, model calls, and remote placement. OpenAgents
   should unify those extension classes only through immutable publisher/
   provenance identity, a declared capability manifest, scoped accounts,
   bounded resources, isolated execution, explicit activation, generation
   fencing, and rollback.
8. **Cloud thread visibility is a source-code security property.** Amp threads
   may include messages, full selected files, tool results, and attachments;
   unlisted links are internet-readable, workspace defaults can share threads,
   and administrators retain privileged visibility. OpenAgents must make data
   ownership, Sync, provider exposure, workspace/admin access, retention,
   deletion, and training state separately visible per thread.
9. **Default-open host tools remain unacceptable.** Amp candidly says tools do
   not ask by default and recommends policy plugins or external isolation for
   untrusted input. OpenAgents retains deny-first authorization and default
   fail-closed containment as separate observed artifacts; an AI/plugin policy
   judge cannot be the mandatory deny boundary.
10. **Release proof must be independent of the download origin.** Amp's direct
    installer fetches the executable and checksum from the same vendor origin,
    leaves its minisign path disabled, and installed macOS evidence has no
    working Developer ID/notarization signature. OpenAgents keeps its signed
    component manifest, platform trust, provenance/SBOM, compatibility,
    immutable candidate, staged activation, last-known-good rollback, and
    receipts ahead of updater convenience.

Amp therefore changes the history and remote-supervision design, not the
authority model. OpenAgents should make work history as useful and mobile as
Amp does while keeping accepted events local-portable, protocols public,
execution contained, extensions non-amplifying, data planes explicit, and
release identity independently verifiable. These conclusions remain analysis
until represented in the owning roadmap, schemas, tests, and receipts.

## Open Interpreter harness-emulation addendum (2026-07-18)

The
[Open Interpreter teardown](./2026-07-18-open-interpreter-teardown.md)
adds a runtime class the prior analyses did not name precisely. AI SDK v7,
Goose, T3 Code, and the Agent Client Protocol work all normalize **real agent
runtimes** behind an adapter. Open Interpreter keeps one Codex-derived runtime
and emulates foreign **model-facing harness policies** inside it. It changes
prompts, context, tools, provider wire requests, response grammar, title and
compaction calls, reminders, and result encoding while the native runtime
continues to own sessions, tools, approvals, sandboxing, persistence, ACP, and
app-server behavior.

This distinction changes the analysis in the following exact ways.

1. **“Harness” must split into two execution classes.** A
   `native_runtime_adapter` launches or attaches the real Codex, Claude Code,
   Grok, Cursor, OpenCode, Goose, or Open Interpreter runtime and preserves its
   native session semantics. An `in_process_emulation` is a content-addressed
   policy compiled inside an owned runtime. It may imitate a foreign dialect,
   but the foreign runtime did not execute. OpenAgents' existing
   `AgentRuntimeAdapterKind`, `AgentDefinitionHarnessKind`, and
   `AgentRuntimeLoopKind` provide the right starting separation; add the
   execution class rather than overloading another string.
2. **Outer runtime and inner policy are independently receipted.** An Open
   Interpreter peer can itself run `kimi-code` or `claude-code-bare`
   emulation. The run record therefore needs outer adapter/executable/protocol
   version and inner policy ID/version/digest, beside provider, model, wire,
   translator generations, known losses, auxiliary calls, and effective
   containment. “Kimi Code” alone is a false runtime identity.
3. **One manifest generates every policy projection.** Open Interpreter's
   core enum, TUI picker, app-server catalog, README, and guide already differ
   on membership and wire support; its app-server setter accepts arbitrary
   strings. OpenAgents should generate picker, API, route, docs, diagnostics,
   conformance, and receipts from one immutable manifest covering prompt/tool
   assets, compatibility, state, compaction, approvals, capabilities, losses,
   attribution, and evidence.
4. **Compatibility is admission, not fallback.** The audited resolver rejects
   unsupported Messages combinations but can silently route non-Claude named
   policies over Responses as native and ZCode/unknown policies over Chat as
   generic compatibility. OpenAgents must fail closed before thread creation
   unless the owner separately confirms a native fallback. Requested and
   effective policy are both returned; they can never diverge behind a green
   selected label.
5. **Recommendations cannot be string routers.** Open Interpreter recommends
   policies using substring checks over provider ID, name, model, and base URL,
   including a deliberate DeepSeek → Claude Code Bare choice. That is useful
   product evidence but violates the OpenAgents semantic-routing rule as an
   execution mechanism. Typed compatibility and evaluation records or the
   central semantic selector may rank policies; ordinary admission selects the
   effective one.
6. **Foreign tools remain below canonical authority.** Open Interpreter's
   best boundary is that Claude, Kimi, Qwen, DeepSeek, ZCode, OpenCode, and Pi
   tool aliases all terminate in one native Rust runtime. OpenAgents should
   decode each foreign schema into canonical typed intents, run the normal
   authority and containment compiler, and encode only the canonical outcome
   back. An emulation policy cannot widen filesystem, network, secret, child,
   spend, placement, publication, or acceptance authority.
7. **Policy changes are session transitions.** Open Interpreter's TUI starts a
   fresh chat after harness or provider/model/harness selection. Preserve that
   honesty. Setting a future default, starting a thread with a policy, and
   forking an existing thread under a different policy are distinct commands.
   In-place change is normally refused; a supported migration needs a safe
   boundary, new generation, context recompilation, lineage, and loss report.
8. **Conformance must split beside the taxonomy.** The current OpenAgents
   `harness-conformance` suite correctly proves real coding-worker runtimes
   through chat lifecycle, claim/closeout, readiness, metering, and typed
   failures. Emulations need a sibling enum-exhaustive suite for request/stream
   translators, canonical-tool authority equivalence, compatibility/no-
   fallback laws, compaction and translator state, behavioral evaluation,
   auxiliary usage, recovery losses, and requested/effective receipts. Do not
   make an emulation pose as another worker fixture.
9. **A pinned Open Interpreter peer is the first experiment.** Its ACP and
   Codex-compatible app-server/exec surfaces make it consumable without
   importing the fork. Add only an experimental exact-version peer profile,
   retain the private native event envelope, project portable events with loss
   accounting, and compare native OpenAgents/Codex, Open Interpreter native,
   and named Open Interpreter policies on exact admitted tuples. This measures
   value before OpenAgents accepts prompt/tool maintenance and attribution
   burden.
10. **Computer-use behavior and component admission separate.** Open
    Interpreter's QA skill has the right post-state verification doctrine, but
    downloads `agent-browser` from moving `latest` and pipes Cua installers
    from mutable `main`. Adapt snapshot/act/re-snapshot and consequential-
    action confirmation. Reject moving privileged installers; drivers belong
    in the signed component graph with screen/input authority and install/run
    receipts.
11. **Product-home isolation becomes a peer-profile requirement.** Open
    Interpreter intentionally ignores `CODEX_HOME` and uses
    `INTERPRETER_HOME`/`~/.openinterpreter`, preventing Codex credentials,
    config, and sessions from leaking across product identities. Any external
    runtime profile should declare and test its exact home, credential stores,
    migration behavior, and forbidden ambient homes.
12. **Do not fork or wholesale-copy branded prompts.** Open Interpreter's
    rapid Codex merge cadence and roughly 19.5k-line harness directory prove
    the maintenance cost. Consume it as a peer, learn the compiler seams, and
    implement an OpenAgents-owned minimal/model-family policy only after
    behavioral evidence and legal/attribution review. A branded foreign prompt
    is not a durable public protocol.

This addendum sharpens, rather than changes, the standing architecture. The
runtime remains outside the renderer; provider lanes remain generated and
admitted; foreign-native events remain private evidence beside portable
projections; authority and containment remain separate; and receipts remain
canonical. The new requirement is to make **runtime adaptation** and **harness
policy emulation** separately typed, visible, versioned, tested, and
receipted. As with every teardown, the owning roadmap gate, schemas, issue,
tests, and receipts determine whether the experimental peer or an owned policy
ever ships.
