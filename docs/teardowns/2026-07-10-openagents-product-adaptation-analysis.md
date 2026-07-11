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
cross-device continuity, and Pylon/Source Authority for execution and receipts.
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
  bound to account, repository, placement, containment, and Source Authority.
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
- **managed cloud** — Source Authority, spend, topology, and receipt gates.

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

The assistant may propose an action, but policy and Source Authority decide
whether it executes. This is the bridge between Effect Native intents and the
actual operating system/runtime.

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
provider state into application code.

The in-process Effect Native/runtime composition should substitute an owned
transport into the same generated client and request processor used over the
network. It must not become a privileged direct-service API that bypasses
middleware, WorkContext resolution, policy, events, or receipts.

OpenCode's `Platform` adapter is the useful comparison point, while its
Ghostty terminal shows the desired lifecycle shape: create/connect/replay,
resize/input, persist bounded presentation state, disconnect, and dispose. The
Effect Native version should make that lifecycle an owned Effect resource and
keep the runtime token/transport outside ordinary renderer state.

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
state comes from typed runtime outcomes and Source Authority receipts.

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

OpenAgents should differentiate on five properties:

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
- cross-device Source Authority projections; and
- explicit, user-controlled permissions and memory.

Use OpenCode V1 as the inspectable desktop-host reference, OpenCode V2 as the
durable-admission/scoped-service reference, Codex as the inspectable
engine/protocol/sandbox reference, and Claude Code as the local
recovery/worktree reference. If OpenAgents delivers those foundations through
D6, computer use, agent computers, Fleet, and future AI-employee surfaces can
grow without turning the desktop app into an uninspectable collection of
privileged webviews and sidecars.

## Where we are now — build status against this plan (2026-07-10, late)

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
typed command registry, cross-device Source Authority, explicit permissions),
use OpenCode and Codex as inspectable host/engine references, combine them with
Claude Code's recovery ergonomics, and make the boundaries simpler.

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
   local account is the Source Authority for a purely-local pairing. Khala Sync
   runs in **local/device scope only** (the SQLite store already exists as the
   cache; here it is simply the authority for local-only data). Everything that
   does not cross devices or touch the network works with zero
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
(local-identity Source Authority + account-link promotion) before the packaging
/ dogfood step, since "open the app, pair locally, no login" is a core part of
the predictable-software thesis — the opposite of the ChatGPT app's
account-and-attestation-gated posture the teardown criticized.
