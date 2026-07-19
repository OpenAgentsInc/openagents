# OpenCode Desktop Source Teardown — 2026-07-10

Read-only source inspection of the open-source OpenCode repository at
`projects/repos/opencode`, pinned to upstream `origin/dev` commit
[`9976269ab1accfc9f9dc98a4a688c516934de422`](https://github.com/anomalyco/opencode/tree/9976269ab1accfc9f9dc98a4a688c516934de422).
Purpose: perform the same desktop architecture analysis as the
[ChatGPT](./2026-07-10-chatgpt-desktop-app-teardown.md) and
[Claude](./2026-07-10-claude-desktop-app-teardown.md) teardowns, with source
rather than compiled-bundle evidence, and identify consequences for
OpenAgents Desktop.

Every claim is tagged:

- **[source]** — observed directly in the pinned source tree
- **[test]** — encoded in a repository test, benchmark, or CI verification
- **[public]** — stated in public project documentation or release metadata

No source was modified in the reference repository. The analysis did not run
the application or inspect user data, credentials, or local OpenCode state.
Source proves intended implementation at this commit. It does not prove that
every path is enabled in every released binary or deployment channel.

## TL.DR

OpenCode Desktop is now a **stock Electron 42.3.3** application with a local,
bundled SolidJS workbench and an embedded OpenCode server running in an
Electron utility process. The main process chooses a loopback port, generates a
random password on each launch, starts the server with Basic authentication,
and gives the renderer the URL and credentials through a narrow preload. The
renderer then talks to the same HTTP, SSE, and WebSocket protocol used by web,
CLI, SDK, and remote-server clients.

That topology is the clearest open implementation of the architecture inferred
from the two closed products:

```text
Electron main
  ├─ sandboxed Solid renderer
  │    └─ generated HTTP SDK + SSE + ticketed PTY WebSocket
  ├─ typed preload / IPC for desktop-only capabilities
  └─ Electron utility-process sidecar
       └─ OpenCode server + SQLite + agents + tools + MCP + plugins + PTYs
```

The renderer is not a privileged IDE process. Files, Git, terminals, sessions,
providers, permissions, questions, worktrees, and tool execution live behind
the server contract. Electron IPC is reserved for desktop-specific behavior:
windows, dialogs, picked-file grants, storage, links, updates, deep links,
notifications, native menus, logging, and Windows/WSL setup.

The strongest design choices are the shared local/remote server abstraction,
the generated typed SDK, the eager SSE subscription with heartbeats and
reconnect/coalescing logic, workspace-scoped state, a central renderer command
catalog, ticketed PTY WebSockets, and an unusually serious test/performance
surface. The most important cautions are a very broad monorepo and migration
surface, coexistence of legacy and next-generation protocols/layouts, an
extension model that loads code into the trusted server process, broad macOS
code-signing entitlements, and local credentials that necessarily enter the
renderer so it can call the loopback API.

OpenCode most strongly validates OpenAgents Desktop D1–D4. It also changes the
cross-teardown conclusion: **Electron is only the host. The durable product
architecture is a local-first, typed server protocol shared by every client.**

## 1. Identification and source scope

| Field | Value | Evidence |
| --- | --- | --- |
| Repository | `anomalyco/opencode` | [source] root `package.json` |
| Commit | `9976269ab1accfc9f9dc98a4a688c516934de422` | [source] fetched `origin/dev` |
| Commit time | `2026-07-10T14:13:46-04:00` | [source] Git metadata |
| License | MIT | [source] `LICENSE`, package manifests |
| Desktop package | `@opencode-ai/desktop` `1.17.18` | [source] `packages/desktop/package.json` |
| Desktop runtime | Electron `42.3.3` | [source] desktop package manifest |
| Renderer | SolidJS `1.9.10`, Vite `7.1.4`, shared `@opencode-ai/app` | [source] root/app manifests |
| Server runtime | TypeScript, Effect `4.0.0-beta.83`, Node/Bun-compatible build | [source] manifests and imports |
| Desktop build | electron-vite `5`, electron-builder `26.15.2` | [source] desktop manifest |
| Platforms | macOS x64/arm64, Windows x64/arm64, Linux x64/arm64 | [source/test] builder config and release matrix |
| Release formats | DMG/ZIP, NSIS, AppImage/DEB/RPM | [source] builder config |

The pinned snapshot is ahead of the source version used by the earlier Sol
parity audit. In particular, the current desktop package is Electron, not the
older Tauri host. Any OpenCode architectural claim should therefore include a
commit or release version. The project is changing quickly.

## 2. Repository anatomy: one product, several reusable contracts

The desktop app is not a standalone repository. It is a thin package inside a
Bun/Turborepo monorepo whose relevant layers are [source]:

| Package/path | Responsibility |
| --- | --- |
| `packages/desktop` | Electron main, preload, renderer entrypoint, native bridge, updater, packaging |
| `packages/app` | Shared Solid product UI used by desktop and web |
| `packages/opencode` | Agent runtime, current/legacy HTTP server composition, CLI, providers, tools, MCP, plugins |
| `packages/core` | Effect services, SQLite/database, events, PTY, sessions, projects, filesystem, control-plane primitives |
| `packages/schema` | Shared Effect Schema event and domain contracts |
| `packages/protocol` | Next-generation Effect HTTP API definitions |
| `packages/server` | Next-generation typed handlers and middleware |
| `packages/client` | Generated Promise and Effect clients for the new protocol |
| `packages/sdk/js` | Generated public SDK used by the current shared app and CLI |
| `packages/session-ui`, `packages/ui` | Workbench and design-system primitives |
| `packages/plugin` | Public plugin types and hook/tool contracts |

The separation is meaningful. Desktop-specific code can be replaced without
moving sessions, tools, or persistence into another product. The shared app can
connect to the built-in sidecar, WSL, an arbitrary HTTP server, or a future SSH
proxy while using the same concepts.

It is also a migration in flight. The snapshot contains legacy routes and
events, `/api/*` next-generation routes, `EventV2Bridge`, old and new layout
paths, compatibility projections, two generated-client families, and explicit
legacy state migration tests [source/test]. That investment preserves shipped
behavior, but it demonstrates how expensive an unfrozen protocol becomes.

## 3. Electron host: orchestration rather than product logic

The Electron main process owns lifecycle and native integration [source]:

- single-instance locking and `opencode:` deep links.
- window restoration, geometry, focus, titlebar, zoom, theme background, and
  multi-window identity.
- desktop menus and command forwarding.
- system certificate and proxy-environment adoption.
- local-server and WSL sidecar lifecycle.
- native pickers, notifications, clipboard image reads, external links/apps,
  storage, logs, crash reporting, and update installation.
- renderer failure/unresponsive recovery. And
- clean shutdown/relaunch of sidecars before process exit.

It does **not** implement conversation state, provider calls, agent tools,
filesystem traversal, Git review, or PTY semantics. Those remain server-owned.
This is the correct direction for an agentic Electron application: keep the
main process capable but small in domain terms.

### Window and origin model

Each window is created with [source]:

```ts
webPreferences: {
  preload: ...,
  contextIsolation: true,
  nodeIntegration: false,
  sandbox: true,
}
```

Packaged renderer assets are served from a private privileged
`oc://renderer/` scheme rather than `file://`. The protocol handler rejects any
other host and resolves paths beneath the renderer root with traversal checks.
Development uses the electron-vite dev URL.

Permission handlers allow only sanitized clipboard writes and notifications,
only from the trusted renderer origin, and only for the expected
`webContents`. Renderer load failure, process death, preload failure, and
unresponsive states are logged and surfaced through recovery dialogs that can
export logs, relaunch, quit, or keep waiting [source].

This is materially better than treating a remote web origin as the app. The
renderer and preload ship together, and web deployments cannot silently gain
desktop privileges.

## 4. Preload and IPC: desktop capabilities are explicit, but validation is uneven

The preload exposes one `window.api` object with `contextBridge`. The renderer
never receives raw `ipcRenderer` [source]. Its surface includes:

- initialization and sidecar shutdown.
- updater state/check/install.
- WSL discovery, installation, server start, and terminal launch.
- default-server selection.
- onboarding and display settings.
- namespaced persistent storage.
- directory/file/save dialogs.
- token-bound reads of explicitly picked attachment files.
- external link/path opening and app resolution.
- clipboard images and notifications.
- window identity/focus/show/relaunch/zoom/titlebar.
- native-menu commands, deep links, log export, and fatal-error reporting.

The picked-file implementation is a good capability pattern: selection returns
a temporary token plus metadata. Reads require the same renderer identity,
token, and exact authorized path. A byte budget is enforced. And the renderer
releases the grant after constructing browser `File` objects [source/test].

The IPC layer is not uniformly schema-decoded. Many handlers rely on TypeScript
types and direct values from the renderer, and several operations such as
`shell.openExternal`, `shell.openPath`, store names/keys, app names, markdown,
and URLs do not perform a shared sender-origin and Effect Schema validation in
the handler itself [source]. The sandboxed local renderer is the assumed trust
principal. OpenAgents should keep the explicit surface while making runtime
validation and sender checks mechanical for every handler.

## 5. Embedded engine: a server in an Electron utility process

The desktop build compiles `packages/opencode` for Node and aliases it into the
Electron main bundle as `virtual:opencode-server`. At runtime [source]:

1. Main selects a free `127.0.0.1` port.
2. Main generates a random UUID password.
3. Main forks `sidecar.js` with `utilityProcess.fork`, inheriting the desktop
   shell environment, system certificates, and proxy settings.
4. Main sends `{ hostname, port, password, userDataPath }` over the utility
   process message port.
5. Sidecar imports the bundled server and listens with username `opencode`,
   the random password, and CORS limited to `oc://renderer`.
6. Main resolves initialization to the renderer as soon as the server reports
   ready, then independently waits for `/global/health`.
7. Stop sends an orderly message, waits up to six seconds, then kills the
   utility process if required.

The server is therefore isolated from the Electron main process without
requiring a separately downloaded CLI binary. It has a named service, explicit
start/ready/error/stop messages, a stall timeout, health probing, exit logging,
and deterministic reaping.

This is simpler than Claude's downloaded CLI compatibility plane and more
conventional than ChatGPT's Owl/Chromium runtime integration. It also means
that an engine update is normally an app update: the server build and renderer
SDK can ship as one compatible set.

## 6. Client/server protocol: HTTP commands, SSE facts, WebSocket terminals

OpenCode uses three transports for three different jobs [source]:

| Transport | Role | Important properties |
| --- | --- | --- |
| HTTP request/response | queries and commands | generated SDK, typed routes, directory/workspace routing, Basic auth |
| Server-Sent Events | global and instance event streams | eager listener registration, connected/heartbeat/disposed events, reconnect |
| WebSocket | interactive PTY data | one-time connect ticket, origin check, replay cursor, ordered queue, close semantics |

The current `/api/*` protocol is declared with Effect HTTP API and Effect
Schema. It covers health, location, agents, sessions, messages, models,
providers, integrations, credentials, permissions, questions, filesystem,
commands, skills, references, events, and PTYs. Generated Promise and Effect
clients live in `packages/client`. The shipped app currently consumes the
generated v2 JavaScript SDK [source].

### SSE lifecycle

The instance event handler registers its queue listener before constructing the
response body so events cannot fall into a subscribe/start race. It emits an
initial `server.connected`, ten-second `server.heartbeat` records, scoped
domain events, and a terminal `server.instance.disposed`. The shared app [source]:

- maintains one server-scoped stream.
- aborts and reconnects after failures.
- detects stale visible streams via heartbeat age.
- restarts after browser back/forward-cache restoration.
- yields periodically so a flood does not monopolize the UI thread.
- coalesces adjacent LSP and message-part updates. And
- batches delivery into Solid stores.

This is the operational detail behind “streaming UI.” A protocol definition
alone is insufficient. Recovery, ordering, backpressure, and disposal are part
of the product contract.

### PTY security and continuity

The terminal first creates or restores a PTY over HTTP, requests a short-lived
connect ticket using a special header and valid origin, then opens a WebSocket
with the ticket and replay cursor [source]. The server consumes the ticket for
the exact PTY/directory/workspace scope, replays buffered output, sends cursor
metadata, serializes live output and close frames through one queue, and tracks
sockets for forced shutdown.

The UI uses `ghostty-web`, persists terminal metadata and bounded buffer/scroll
state, restores sessions, supports multiple sortable terminals, and caps a
workspace at 20 terminal sessions [source]. This is a mature foreign-host
pattern, not a textarea attached directly to `node-pty`.

## 7. Shared renderer: a local, server-agnostic Solid workbench

`packages/desktop/src/renderer/index.tsx` adapts native desktop capabilities
into the shared app's `Platform` interface and mounts `AppInterface` from
`@opencode-ai/app` [source]. The same product UI can therefore run with a web
platform adapter or the Electron adapter.

The renderer starts with four async prerequisites: local sidecar credentials,
saved default server, window count, and locale. It constructs the built-in
server as a typed `sidecar` connection, adds ready WSL connections, then mounts
the app behind server-, platform-, settings-, language-, query-, permission-,
file-, model-, layout-, notification-, prompt-, and tabs-scoped providers.

Server identity is first-class. Connection types include [source]:

- built-in sidecar.
- WSL sidecar.
- regular HTTP server. And
- SSH-backed HTTP proxy.

Projects, recent projects, last active project, tabs, permissions, terminals,
and caches are keyed by server/workspace/session scope. This avoids a common
local-first error: treating “localhost” or a filesystem path as globally unique
when the client can connect to several runtimes.

### Workbench anatomy

The product surface includes [source]:

- project/workspace sidebar and directory picker.
- persistent, draggable session/draft/terminal tabs and closed-tab history.
- streamed conversation timeline with virtualized rendering.
- prompt history, files, image attachments, context items, slash commands,
  models, agents, variants, and usage/context breakdown.
- permission, question, follow-up, todo, revert, retry, fork, archive, compact,
  interrupt, and share flows.
- lazy file tree, search, file tabs, selected-line context, and cache eviction.
- Git/snapshot review with split/unified diffs and line comments.
- integrated Ghostty terminal with replay and restoration.
- provider/model management, custom providers, MCP selection, server settings,
  keybindings, themes, and release notes. And
- native menus, deep links, notifications, multi-window restore, WSL, and
  automatic updates.

It is a coding workbench organized around sessions, not a code editor with a
chat panel bolted on. Conversation remains the central timeline. Files, review,
and terminal are contextual side surfaces.

## 8. Command system: one catalog across palette, keys, slash, and menus

The shared app has a central `CommandProvider`. A command declares a stable ID,
title, description, category, keybinding, optional slash alias, visibility,
availability predicate, and selection handler [source]. Registrations are
scoped and deduplicated by key. The catalog drives:

- keyboard dispatch.
- command palette.
- slash suggestions.
- settings/keybinding presentation.
- native Electron menu forwarding. And
- contextual session/file/terminal commands.

This validates the Sol requirement that material UI actions need stable command
IDs. OpenCode's command definitions remain renderer callbacks, however. They do
not yet constitute an end-to-end authority contract with typed input/output,
policy metadata, idempotency, and durable outcomes. OpenAgents should use the
same ergonomic catalog while extending the command identity through Effect
Native, Pylon, Blueprint, Khala Sync, and receipts.

## 9. Runtime: Effect services, SQLite facts, and scoped instances

The server is actively moving onto Effect 4 services [source]. Its package
contract requires:

- `Effect.gen` composition and named `Effect.fn` traces.
- shared memoized runtimes rather than ad hoc service construction.
- `InstanceState` backed by a scoped cache for per-directory/project state.
- finalizers/acquire-release for subprocesses and subscriptions.
- scoped fibers for background consumers. And
- Effect platform services for files, child processes, HTTP, path, config,
  clocks, and callbacks.

The local database is SQLite through Drizzle/Effect. Startup enables WAL,
foreign keys, a five-second busy timeout, a 64 MB cache, and migrations
[source]. Tables cover projects/directories, sessions, messages, parts, todos,
inputs, context epochs, workspaces, event sequences, and durable events. Session
records carry model, agent, permission, token, cost, summary, revert, parent,
workspace, share, and archive state.

Per-project services are not global singletons. Opening a directory resolves a
project identity, provisions scoped services, and cleans them up on disposal.
The event system records aggregate sequence numbers and owners, and the code
contains replay/sync bridges for the transition from legacy bus events to
durable v2 events [source].

This is unusually close to OpenAgents' intended Effect/typed-state direction.
The lesson is not to copy the implementation. It is that local desktop state
deserves the same transactional, scoped, observable rigor as cloud state.

## 10. Agent, tool, provider, MCP, skill, and plugin planes

The embedded runtime is provider-agnostic. Its manifest includes Anthropic,
OpenAI, Google/Vertex, Bedrock, Azure, Cerebras, Cohere, Groq, Mistral,
Perplexity, Together, xAI, OpenRouter, GitHub Copilot, GitLab, Poe, Cloudflare,
DigitalOcean, Snowflake, and compatible-provider paths [source]. Providers and
models are resolved in the server. The renderer sees catalogs and connection
flows rather than provider SDKs.

Built-in agents include build, plan, general-purpose subagent, exploration,
compaction, title, and summary roles. Permission defaults ask for external
directories and `.env` reads, deny or constrain sensitive mode transitions,
and merge built-in, agent, and user rules [source]. The UI can answer a request
once, always, or reject, with bounded auto-accept state and duplicate-response
suppression.

The tool registry composes built-ins, config-defined tools, plugin tools, MCP
tools, and experimental tools. Built-ins cover shell, read, glob, grep, edit,
write, task/subagent, web fetch/search, todo, skill, patch, question, LSP, and
plan transitions [source]. Tool execution is traced with session/message/call
identities and truncates oversized output into referenced files.

MCP supports stdio, Streamable HTTP, legacy SSE, OAuth, roots, resources, tool
changes, browser-open events, timeouts, and scoped process cleanup [source].
Skills are discovered from local configuration and remote indexes. Versioned
remote skill updates download into staging, require `SKILL.md`, atomically
promote, and roll back on failure.

Plugins are more powerful and riskier. Built-ins and external packages execute
inside the trusted server process and can register hooks, tools, provider auth,
shell environment, and workspace adapters [source]. External dependencies may
be installed dynamically. The source has compatibility and deterministic-order
care, but no process sandbox or signed marketplace boundary is evident at this
layer. OpenAgents should ingest this ecosystem only through a declared,
isolated, provenance-bearing capability model.

## 11. Files, Git, worktrees, and review are server capabilities

The renderer's file tree and editor never read the filesystem through Electron
IPC. They call directory-scoped API routes for list, read, search, status, and
watch-driven invalidation [source]. File content is cached with explicit byte
accounting and LRU eviction. Selected lines can become prompt context or review
comments without leaking a general filesystem handle into the UI.

Git runs behind an Effect service with bounded commands and output limits.
Project identity is derived from VCS state. Status, refs, branches, merge bases,
show, diff, and snapshots feed review surfaces. Worktrees receive generated
names and `opencode/<name>` branches, live under an application-owned data
root, can run project start commands, and have typed failure modes [source].

This division is directly applicable to OpenAgents Desktop D3: make files,
review, Git, and PTY coherent runtime services with typed events. Do not give
the renderer Node access merely because the product resembles an IDE.

## 12. Security posture

### Strong choices

- sandboxed renderer, context isolation, Node integration off [source].
- local packaged renderer origin with host/path checks [source].
- deny-by-default Electron permissions except two explicit capabilities
  [source].
- random per-launch server password, loopback binding, and origin-limited CORS
  [source].
- no password in command-line arguments. Main-to-sidecar message handoff and
  environment setup happen inside the process tree [source].
- generated SDK sends Basic auth. Health checks authenticate [source].
- one-time, scope-bound, origin-checked PTY tickets [source].
- tokenized attachment reads tied to sender and byte budget [source/test].
- subprocess, socket, stream, and instance cleanup through owned finalizers
  [source]. And
- signed/notarized macOS packaging and Windows signature verification in CI
  [source/test].

### Residual risks and gaps

- The sidecar password is delivered to and retained by the renderer because
  the renderer makes HTTP requests. Renderer compromise therefore becomes
  local server authority for that launch [source].
- IPC runtime validation and sender/origin checks are not uniformly centralized
  or schema-driven [source].
- No explicit Content Security Policy, top-level navigation denial, or
  `setWindowOpenHandler` denial was found in the desktop renderer/window source
  at this commit [source]. The private local origin reduces exposure but does
  not replace those controls.
- External plugins execute in the trusted server process and can expand tool
  and environment behavior [source].
- The macOS entitlement file enables JIT, unsigned executable memory, disabled
  executable-page protection, dyld environment variables, disabled library
  validation, and audio input. No macOS App Sandbox entitlement is present
  [source]. Some are likely needed by Electron/native modules, but the set is a
  broad trust footprint.
- No explicit Electron fuse configuration was found in the pinned desktop
  builder config [source]. Packaged defaults were not inspected, so fuse state
  is unknown rather than presumed weak.
- The window session injects permissive `Access-Control-Allow-*` headers. The
  server still enforces auth/CORS, but this widens renderer network mechanics
  and deserves a documented threat model [source].
- A configured remote HTTP server can be plain `http://`. Transport security
  and trust are left to connection configuration [source].
- Permission “always” and auto-accept are powerful persisted affordances. The
  code scopes and deduplicates them, but UX clarity remains part of safety
  [source].

The key distinction is between **renderer hardening** and **execution
isolation**. OpenCode does the first well. Agent shell/tools still execute on
the host under application permissions unless another environment or policy
provides isolation. OpenAgents must not mistake a sandboxed web renderer for a
sandboxed agent workload.

## 13. Packaging, updates, and release engineering

electron-vite builds main, preload, and renderer bundles. The desktop prebuild
also compiles the OpenCode server for Node and copies required WASM assets.
electron-builder then packages output and native modules [source].

The release matrix builds six OS/architecture targets [source/test]:

- macOS x64 and arm64 with imported code-signing certificates, hardened
  runtime, notarization, DMG, ZIP, and `.app.tar.gz` artifacts.
- Windows x64 and arm64 with Azure Trusted Signing verification and NSIS. And
- Linux x64 and arm64 with AppImage, DEB, and RPM.

Dev, beta, and production use distinct app IDs, names, icons, and release
channels. The `opencode:` protocol is registered on all channels. Production
and beta update metadata is published through GitHub releases [source].

The updater is a small explicit state machine: disabled/idle, checking,
up-to-date, downloading, ready, installing, and error. It does not auto-install
on quit. It persists a downloaded version, checks at startup and every ten
minutes, stops sidecars before installation, and restores `ready` on an install
failure [source/test]. Downgrades are allowed, which aids rollback but makes
release trust and metadata integrity more important.

The source does not show a separately signed component ledger for server,
plugins, skills, native modules, schemas, and renderer compatibility. The app
normally ships its server and UI together, but plugins and remote skills remain
independent moving inputs.

## 14. Verification and performance discipline

The repository has a large verification surface [test]:

- 12 desktop package tests.
- 165 shared-app unit tests plus browser, Playwright, stability, and benchmark
  suites.
- 245 runtime tests.
- 141 core tests.
- protocol/schema/client contract and import-boundary tests.
- generated API exercise modes for coverage, authentication, and Effect
  behavior. And
- release CI that builds every desktop platform/architecture and verifies
  Windows signatures.

The app's performance suite measures timeline stability, visual stability,
session-tab switching, rendering, tracing, and data loss. The package contract
explicitly requires production benchmark baselines before session/timeline
changes [source/test]. The renderer contains event coalescing, virtualized
timelines, scoped caches, ref-counted directory SDK contexts, file-content byte
budgets, and tab/session state eviction [source].

This is a substantive product lesson: long-running agent transcripts are a
performance domain of their own. D1 is incomplete without load, reconnect,
scroll, tab-switch, and memory-retention oracles.

## 15. Open/closed split

OpenCode's load-bearing desktop stack is MIT-licensed source [source]:

- Electron host, preload, renderer adapter, updater, and packaging.
- shared Solid application and UI packages.
- server, protocol, schemas, generated clients, persistence, agents, tools,
  MCP, skills, plugins, Git, PTY, and worktree services. And
- tests, benchmarks, migrations, and release workflows.

External dependencies and services remain outside that license boundary:
model-provider APIs, OAuth/provider accounts, hosted OpenCode services, npm
plugin packages, remote MCP servers, remote skill indexes, Sentry, GitHub
release infrastructure, and operating-system signing/notarization services.

This is a more meaningful open split than an open wrapper around a proprietary
engine. A third party can inspect, fork, build, and replace the host, engine,
protocol, client, and UI. That makes OpenCode the most useful implementation
reference of the three products—and the most important reminder to open the
seams that actually carry authority.

## 16. What OpenAgents should adapt

### Adapt directly

1. **Server-first desktop topology.** Keep Electron as host and run the real
   Pylon/OpenAgents runtime outside the renderer behind one typed protocol.
2. **One connection abstraction for local and remote.** Desktop, web, mobile,
   WSL/SSH, and Fleet projections should use stable server/runtime identities,
   not special-case “local” throughout the UI.
3. **Generated schema-owned clients.** Generate Promise/Effect clients and
   fixtures from the same protocol definitions used by handlers.
4. **Transport specialization.** Use request/response for commands, resumable
   event streams for facts, and a bounded interactive transport for PTY-like
   streams.
5. **Eager event subscription, heartbeat, reconnect, coalescing, and disposal.**
   These are D1 requirements, not post-launch polish.
6. **Scoped local state.** Key projects, sessions, permissions, terminals,
   tabs, caches, and events by runtime/workspace/session identity.
7. **Server-owned workbench capabilities.** Files, Git, review, PTY, tools,
   providers, MCP, and agent execution stay behind typed services.
8. **Central command catalog.** Palette, keys, menus, slash commands, mobile,
   and model-proposed actions share stable IDs, then OpenAgents carries those
   IDs through policy and receipts.
9. **Capability-shaped file grants.** Preserve temporary, sender-bound,
   budgeted attachment access rather than exposing path-generic read IPC.
10. **Performance as contract.** Add transcript scale, stream churn,
    reconnect, tab-switch, file-cache, and renderer memory gates to D1–D3.

### Adapt with stronger boundaries

- Keep local server credentials and general HTTP authority out of the renderer
  where possible. Prefer a typed host transport or capability-token broker that
  does not turn one renderer compromise into full runtime authority.
- Decode every IPC request and response with Effect Schema and validate sender,
  frame, origin, path, URL, and capability mechanically.
- Run third-party plugins, MCP servers, skills with executable content, and
  generated code in declared isolation profiles with signatures/provenance,
  resource budgets, egress rules, and receipts.
- Preserve the local renderer and shared-package model, but use Effect Native
  intents and foreign hosts rather than leaking library or Electron instances.
- Keep durable events and projections from the start. Avoid maintaining two
  protocol/event/layout generations longer than an explicit migration window.
- Verify Electron fuses, entitlements, native modules, signing, update,
  rollback, and component compatibility as release oracles.

### Do not copy

- Do not make the renderer a general authenticated localhost client.
- Do not treat host tool permissions as execution isolation.
- Do not dynamically load untrusted extension code into the primary runtime.
- Do not let persisted “always allow” state become invisible ambient authority.
- Do not carry legacy and next-generation contracts indefinitely.
- Do not equate a large feature inventory with a coherent authority model.

## 17. Final assessment

OpenCode Desktop is the closest public implementation reference for the current
OpenAgents Desktop roadmap. Its most important achievement is not a particular
sidebar, terminal library, or provider list. It establishes a clean product
sentence:

> A desktop coding agent is a local/remote runtime protocol with a durable
> workbench client. Electron supplies native hosting, not application authority.

ChatGPT shows the ceiling of a deeply integrated proprietary host. Claude shows
the value of stock Electron plus explicit engine and VM planes. OpenCode shows
how much of the same shape can be built openly with one local renderer, one
embedded server, generated contracts, and reusable clients.

OpenAgents should take that topology, strengthen the renderer-to-runtime
capability boundary, make execution profiles and receipts first-class, and add
Khala Sync/Fleet/source-authority semantics that OpenCode does not attempt. If
that is done, parity work becomes implementation of a coherent protocol—not a
sequence of UI imitations.

## Primary source map

All links are pinned to the analyzed commit:

- [desktop package and dependencies](https://github.com/anomalyco/opencode/blob/9976269ab1accfc9f9dc98a4a688c516934de422/packages/desktop/package.json)
- [Electron main process](https://github.com/anomalyco/opencode/blob/9976269ab1accfc9f9dc98a4a688c516934de422/packages/desktop/src/main/index.ts)
- [window hardening and private renderer protocol](https://github.com/anomalyco/opencode/blob/9976269ab1accfc9f9dc98a4a688c516934de422/packages/desktop/src/main/windows.ts)
- [preload API](https://github.com/anomalyco/opencode/blob/9976269ab1accfc9f9dc98a4a688c516934de422/packages/desktop/src/preload/index.ts)
- [desktop IPC handlers](https://github.com/anomalyco/opencode/blob/9976269ab1accfc9f9dc98a4a688c516934de422/packages/desktop/src/main/ipc.ts)
- [sidecar supervisor](https://github.com/anomalyco/opencode/blob/9976269ab1accfc9f9dc98a4a688c516934de422/packages/desktop/src/main/server.ts)
- [sidecar entrypoint](https://github.com/anomalyco/opencode/blob/9976269ab1accfc9f9dc98a4a688c516934de422/packages/desktop/src/main/sidecar.ts)
- [desktop renderer adapter](https://github.com/anomalyco/opencode/blob/9976269ab1accfc9f9dc98a4a688c516934de422/packages/desktop/src/renderer/index.tsx)
- [shared app](https://github.com/anomalyco/opencode/tree/9976269ab1accfc9f9dc98a4a688c516934de422/packages/app)
- [server composition](https://github.com/anomalyco/opencode/blob/9976269ab1accfc9f9dc98a4a688c516934de422/packages/opencode/src/server/routes/instance/httpapi/server.ts)
- [next protocol](https://github.com/anomalyco/opencode/tree/9976269ab1accfc9f9dc98a4a688c516934de422/packages/protocol)
- [schemas and events](https://github.com/anomalyco/opencode/tree/9976269ab1accfc9f9dc98a4a688c516934de422/packages/schema)
- [generated clients](https://github.com/anomalyco/opencode/tree/9976269ab1accfc9f9dc98a4a688c516934de422/packages/client)
- [Electron builder configuration](https://github.com/anomalyco/opencode/blob/9976269ab1accfc9f9dc98a4a688c516934de422/packages/desktop/electron-builder.config.ts)
- [release workflow](https://github.com/anomalyco/opencode/blob/9976269ab1accfc9f9dc98a4a688c516934de422/.github/workflows/publish.yml)
- [MIT license](https://github.com/anomalyco/opencode/blob/9976269ab1accfc9f9dc98a4a688c516934de422/LICENSE)
