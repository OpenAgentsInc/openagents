# What OpenAgents Should Adapt from the ChatGPT, Claude, and OpenCode Desktop Teardowns

Date: 2026-07-10

Status: product and architecture decision input

Evidence base:

- [ChatGPT / Codex desktop teardown](./2026-07-10-chatgpt-desktop-app-teardown.md)
- [Claude desktop teardown](./2026-07-10-claude-desktop-app-teardown.md)
- [OpenCode desktop source teardown](./2026-07-10-opencode-desktop-app-teardown.md)
- [Sol master roadmap](../sol/MASTER_ROADMAP.md), especially Desktop D0–D6
- [OpenAgents Desktop enforced guarantees](../../apps/openagents-desktop/GUARANTEES.md)

## Executive decision

OpenAgents should adapt the **shape** all three desktop products converge on,
while rejecting their least transparent trust choices. OpenCode matters
disproportionately because it exposes the implementation of that shape under an
MIT license rather than requiring inference from a signed bundle.

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
desktop client. The evidence does **not** justify an Owl-style runtime fork, a
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

## What the open source changes

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
- ordered event cursors and replay/resume;
- text, reasoning, tool, plan, todo, question, permission, approval, usage,
  error, interruption, and completion events;
- typed capability discovery rather than UI feature guessing;
- explicit cancellation, reconnect, and terminal states;
- connected, heartbeat, disposal, and stale-stream semantics;
- backpressure/coalescing rules and bounded replay cursors;
- redacted diagnostics and per-event provenance; and
- compatibility fixtures replayable without a live provider.

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

## Product-specific adaptations

### OpenAgents Desktop

Desktop should absorb the deepest lessons because it owns local capability.

Priority order aligned with D0–D6:

1. **D1:** real protocol-backed streamed sessions, eager subscribe, connected
   and heartbeat events, replay, interrupt, resume, reconnect, coalescing,
   permissions, approvals, and usage.
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
- receive input-needed, completion, failure, and budget notifications;
- make device/offline/capability state explicit;
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
- health, heartbeat, disposal, and stale-stream recovery;
- local/remote transport adapters behind stable runtime identities;
- plugin/MCP/skill composition; and
- redacted replayable event receipts.

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

## The OpenAgents differentiation

The three reference products leave a coherent opening:

- OpenAI has the more open engine but a closed desktop/plugin/runtime stack and
  extremely broad ambient-computer ambitions.
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

That is a stronger product thesis than “another desktop chat app” or “OpenCode
with Fleet buttons.” It is an inspectable operating surface for work performed
by multiple engines across multiple computers, with authority and evidence that
survive beyond the renderer process.

## Ordered implementation consequences

| Order | Decision | Owning program | Proof |
| ---: | --- | --- | --- |
| 1 | Freeze the tokenless renderer → host-owned Runtime Gateway and engine/event protocol | Desktop D0/D1 + Pylon | Boundary oracle proves no runtime credential or generic transport enters renderer; fixtures replay the versioned event algebra |
| 2 | Bind Desktop and mobile to the existing R1 identity and R2 Khala Sync contracts | R1/R2 + Desktop/mobile adapters | Same server-derived owner/scope, independent revoke, SQLite restart, exact phases, no token in projections |
| 3 | Ship one real streamed Desktop conversation with immediate mobile continuation | Desktop D1 + mobile narrow Sync slice | Matching thread/message refs, versions, phases, cursor and terminal outcome; one safe mobile follow-up/interrupt; restart/gap/lost-ACK proof |
| 4 | Extend the central command registry through host/runtime outcomes | Desktop D2 + Effect Native | UI, keyboard, menu, mobile and test invoke the same command IDs and reconcile one durable outcome |
| 5 | Add bounded file/editor/Git/PTY foreign hosts while mobile remote-workroom work proceeds in parallel | Desktop D3 + mobile R6 + Effect Native | Useful coding loop on each form factor without renderer or phone local process/filesystem authority |
| 6 | Define execution profiles, receipts, and isolated extension compatibility | Desktop D3/D4 + Pylon + Cloud | Same task reports grants/profile/evidence; signed extension denial/update/rollback/run receipts pass |
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
- query/command/event transports with explicit lifecycle semantics;
- host/guest execution split;
- central typed command registry;
- MCPB-compatible signed catalog;
- one component/update compatibility ledger;
- cross-device Source Authority projections; and
- explicit, user-controlled permissions and memory.

If OpenAgents delivers those foundations through D6, computer use, agent
computers, Fleet, and future AI-employee surfaces can grow without turning the
desktop app into an uninspectable collection of privileged webviews and
sidecars.
