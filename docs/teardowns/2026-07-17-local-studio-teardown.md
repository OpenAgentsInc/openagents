# Local Studio Teardown — 2026-07-17

Read-only architecture and product audit of the public
`sybil-solutions/local-studio` source tree at an exact commit. The audit did
not install runtimes, download model weights, launch an inference backend, open
the desktop application, connect a Google account or MCP server, read local
user state, or exercise a remote controller.

## TL.DR

Local Studio is the strongest open reference in this teardown set for the seam
between a **self-hosted model control plane** and a **local coding-agent
workbench**. One product discovers and installs vLLM, SGLang, llama.cpp, and MLX
runtimes. Launches and evicts model servers. Reports GPU, process, download,
log, and usage state. Exposes an OpenAI-compatible inference proxy. And feeds
those models into a Pi-based coding agent with files, Git, terminals, browser
control, skills, plugins, MCP connectors, Google Workspace observation, speech,
and a macOS Electron shell. [source]

The architecture has three local server processes rather than one:

```text
Electron main
  -> loopback Next.js standalone server
       -> loopback Pi agent-runtime sidecar
       -> local or remote Bun/Hono controller
            -> vLLM / SGLang / llama.cpp / MLX process
```

That split is useful. The controller is a real hardware-and-model authority,
not a chat backend. The agent sidecar is a separately packaged runtime rather
than model execution hidden in Next route handlers. The renderer receives a
narrow preload bridge while files, sessions, connectors, browser automation,
OAuth, model processes, and persistence stay in server or main-process code.
The controller also contains unusually concrete operational mechanics: typed
recipe families, runtime-target discovery, cancellable install jobs, launch
failure budgets, process-tree cleanup, GPU leases shared with speech, health
probing, SSE, bounded request bodies, log redaction, SQLite stores, and explicit
remote-controller configuration. [source]

Its most important OpenAgents lesson is therefore not “add local models.” It is
to model local inference as a first-class placement with observable hardware,
runtime, model, process, compatibility, and usage facts. A local model is not
just another provider string. It has installation, resource ownership, startup,
health, eviction, and recovery lifecycles.

The current source snapshot also contains material counterexamples:

- controller authentication correctly becomes mandatory on non-loopback binds,
  but may be explicitly disabled, while the privileged Next frontend is open by
  default even in production unless an operator opts into a separate token.
- the Electron-owned Next server and agent sidecar bind loopback without a
  per-generation capability, so another process under the same host authority
  can call shell, filesystem, agent-turn, browser, connector, and session
  surfaces if it discovers their ports.
- the Pi agent executes with host-user authority. Permissions and selected
  extensions are not demonstrated OS workload containment.
- a recipe may default `trust_remote_code` to true, plugins may resolve stdio
  executables, skills are discovered across several other agents' local stores,
  and selected extension paths are loaded into the trusted Pi runtime.
- the in-memory agent event log is capped at 2,000 entries and resets when a
  runtime fingerprint changes, while durable Pi JSONL, Local Studio metadata,
  current runtime state, and UI projections have separate identities and no
  published admission/replay/repair law.
- release signing exists for macOS, but the app disables the macOS App Sandbox
  and library validation, and update metadata is not tied to a public signed
  component-compatibility ledger. And
- the audited commit deleted 132 tracked test/support/fixture files totaling
  22,669 removed lines, removed all `test` scripts, and reduced CI to structural,
  type, lint, cleanup, and build checks, while `AGENTS.md` and the README still
  require nonexistent test commands. [history] [source]

The central OpenAgents decision is: **adapt Local Studio's typed local-inference
control plane, backend abstraction, hardware/resource observability, and
controller/workbench separation. Reject ambient loopback authority, host-user
agent execution as a safety boundary, default remote-code trust, fragmented
runtime truth, unsigned component drift, and a release gate with no executable
behavioral tests.**

## 1. Snapshot, provenance, and limitations

### 1.1 Exact source identity

| Artifact          | Identity                                                                                    | What it establishes                                                |
| ----------------- | ------------------------------------------------------------------------------------------- | ------------------------------------------------------------------ |
| Public repository | `https://github.com/sybil-solutions/local-studio`                                           | Open Local Studio source and history                               |
| Audited commit    | `4a16109fae58b12b03d172a3463cc62cf244a758` on `main`, committed `2026-07-17T13:19:17-04:00` | Exact source snapshot used here                                    |
| Commit subject    | `fix: stabilize follow-up chat submission`                                                  | The latest change and the test-removal boundary discussed below    |
| Product version   | root, controller, frontend, and agent-runtime packages at `2.0.0`                           | Current declared product generation                                |
| License           | Apache License 2.0, copyright 2025 `0xSero`                                                 | Source reuse boundary                                              |
| Source scale      | 786 tracked files. About 96,132 tracked TypeScript/TSX lines                                | Approximate implementation scale, excluding generated dependencies |
| Primary runtimes  | Bun controller. Node/Next frontend. Node-packaged Pi sidecar. Electron desktop              | Actual process/runtime split                                       |
| UI stack          | Next.js 16.2.7, React 19.2.1, Electron 43.1.1                                               | Current client foundation                                          |
| Effect line       | `effect@4.0.0-beta.90` in controller-facing and frontend runtime packages                   | Typed runtime dependency, still a beta line                        |
| Agent engine      | `@earendil-works/pi-ai` and `@earendil-works/pi-coding-agent` `0.80.8`                      | Underlying coding-agent implementation                             |

The repository is a normal public source reference under
`projects/repos/local-studio/`. No private branch, credential, user session,
model, or runtime artifact was inspected. [source]

### 1.2 Evidence labels

- **`[source]`** — observed in the exact commit's tracked source or manifest.
- **`[history]`** — established by the public Git history at or before the exact
  commit.
- **`[schema]`** — encoded in a typed contract or Effect Schema.
- **`[test]`** — encoded in a tracked executable test at the audited commit.
- **`[inferred]`** — reasoned from several source observations rather than
  directly asserted by one artifact. And
- **`[limitation]`** — a boundary on what the source-only audit can prove.

There are intentionally no `[runtime]` observations in this document.

### 1.3 What this audit cannot establish

Source establishes intended client and controller behavior, local validation,
packaging, and current gates. It does not establish that every supported GPU,
driver, Python environment, model, Docker image, browser, OAuth flow, remote
host, installer, updater, or signed artifact works in production. It also does
not prove that a model backend obeys its advertised OpenAI-compatible contract,
that an MCP server's annotations are truthful, or that a successful macOS build
is notarized. [limitation]

The absence of current test files is a source fact. It does not prove that the
project has never run private, local, generated, or external tests. It does mean
the audited public revision cannot reproduce the prior tracked behavioral
suite, and its current package/CI contracts do not invoke such a suite.
[source] [limitation]

## 2. Whole-product architecture

### 2.1 Four authorities, three local server processes

Local Studio's product name hides four distinct authorities:

| Plane             | Owner                                       | Responsibilities                                                                                                                                 |
| ----------------- | ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| Desktop host      | Electron main and preload                   | Window lifecycle, embedded server supervision, secure OAuth storage, projects, PTYs, quick panel, controller deployment, updates                 |
| Product/workbench | Next standalone server and React renderer   | Status, Configure, Workbench, Usage, Settings, Logs, API mediation, UI projections                                                               |
| Coding agent      | standalone `services/agent-runtime` sidecar | Pi sessions, turn/steer/follow-up, event replay, browser host, skills, plugins, connectors, OAuth adapters                                       |
| Model control     | Bun/Hono controller                         | recipes, downloads, runtime install/upgrade, model lifecycle, GPU/system state, OpenAI-compatible proxy, speech, usage, durable controller state |

The desktop process starts the agent sidecar and the Next server on stable
loopback ports. The Next server proxies agent routes to the sidecar and
controller routes to a configured controller. The controller may be local or
remote and launches an inference server as a further process or Docker-backed
runtime. [source]

This is a more useful decomposition than putting GPU orchestration, agent
execution, and Electron host capabilities in one process. It also creates a
component-identity problem: Electron, Next, Pi, controller, model backend,
Python environment, model weights, plugins, and connectors can move
independently. Local Studio has version and health facts for several pieces but
does not publish one compatibility ledger binding the exact set. [inferred]

### 2.2 Product surface

Version 2.0 consolidates the visible application into:

- `/` for controller and hardware status.
- `/agent` for the Workbench.
- `/configure` for machines, models, integrations, and serving.
- `/usage`, `/settings`, and `/logs`. And
- compatibility redirects from older recipe, discovery, integration, and
  server routes.

The Workbench is not a thin chat page. It contains sessions, a pane grid,
filesystem tree and editor, Git diff, persistent terminals, plan and canvas
panels, a computer/browser panel, attachments, queue state, project/session
navigation, model selection, reasoning display, and a global quick panel.
[source]

## 3. Controller: local inference as an operating system

### 3.1 Typed backend families

The controller has explicit engine specifications for four backend families:

| Backend   | Primary artifact                     | Typical platform       |
| --------- | ------------------------------------ | ---------------------- |
| vLLM      | Python environment or Docker runtime | NVIDIA/CUDA Linux      |
| SGLang    | Python environment                   | NVIDIA/CUDA Linux      |
| llama.cpp | `llama-server` plus GGUF model       | CPU/GPU cross-platform |
| MLX       | `mlx_lm.server` Python environment   | Apple Silicon          |

Each family owns arguments, health paths, model-path extraction, runtime
targets, and recipe behavior rather than flowing through one untyped shell
template. Shared contracts define engine arguments, capabilities, recipes,
rigs, system facts, usage, speech, and controller events. [schema]

The useful design lesson is that provider compatibility and runtime
compatibility are different. Two servers may expose `/v1/chat/completions` but
require different installation, hardware, launch, model-loading, health,
tokenization, and shutdown laws.

### 3.2 Runtime discovery and installation

Runtime targets may be configured, discovered from Python or binaries, found
through Docker, or managed in Local Studio's data directory. Managed venvs and
llama.cpp installs use explicit install locks and jobs, expose progress and
cancellation, and retain runtime information for the UI. CUDA and ROCm probes,
Python-package checks, and backend-specific target factories turn setup into an
observable lifecycle rather than a README-only prerequisite. [source]

That shape is directly relevant to OpenAgents placement. A local inference
placement should report at least:

```text
host -> accelerator -> driver/runtime -> backend -> model artifact
     -> launch generation -> endpoint -> health -> capacity -> usage
```

No single provider label can honestly replace that chain.

### 3.3 Launch, eviction, and resource ownership

`EngineCoordinator` serializes lifecycle switches, aborts superseded launches,
publishes progress, applies a launch-failure budget, waits for backend-specific
health, kills failed starts, and starts a liveness monitor. The process manager
detects inference processes, collects child processes, handles Docker container
cleanup, captures bounded failure output, and confirms that orphaned vLLM
workers are gone before declaring stop. [source]

GPU leases distinguish at least LLM and speech ownership. Recipe GPU selectors
resolve to UUIDs. Unresolved selectors fail. An implicit all-GPU launch can fail
when isolation cannot be verified. And a lease is released only after process
shutdown is confirmed. This is one of Local Studio's strongest seams because it
turns VRAM conflict from a UI warning into a controller decision. [source]

The lease is still process-local controller state rather than a distributed
capacity reservation. It does not establish cross-host fencing, restart-safe
lease recovery, multi-controller exclusion, or a receipted scheduler.
[limitation]

### 3.4 Controller API and storage

The controller exposes model lifecycle, recipes, downloads, runtime targets,
runtime jobs, system compatibility, GPUs, logs, SSE events, usage, rigs,
providers, model discovery, storage operations, speech, audio, and
OpenAI-compatible model/chat/tokenization endpoints. An OpenAPI document and
Swagger UI are generated from controller code. [source]

SQLite stores controller requests, inference requests, rigs, settings, and
usage-oriented facts. Downloads and recipes also have explicit stores. This is
an operational database, but it is not a canonical Thread/Turn/Item or workroom
event log for the coding agent. The Pi session store remains separate.

## 4. Desktop and frontend boundary

### 4.1 Electron is a host, not the model engine

The main window uses context isolation, disables Node integration, enables the
renderer sandbox and web security, disables insecure mixed content and drag
navigation, and loads a locally served origin. Window creation is denied.
ordinary HTTP(S) links open externally. Top-level navigation is locked to the
app origin, and microphone permission is restricted to the main frame, main
web contents, exact app origin, and audio-only requests. [source]

The preload exposes named operations for runtime metadata, external links,
updates, projects, preferences, PTYs, the quick panel, and controller deploy.
It does not expose `require`, a generic IPC sender, or a raw process primitive.
This is a sound baseline. [source]

The bridge is still highly privileged. It can add arbitrary project
directories, create PTYs, send terminal input, choose working directories, and
deploy a controller over SSH. Main-process handlers validate basic types, but
the inspected handlers do not bind every IPC call to the exact sender origin,
window identity, one-shot authority, payload digest, or app-server generation.
Renderer integrity therefore remains load-bearing. [source] [inferred]

### 4.2 Embedded server supervision

Electron packages a Next standalone server and a bundled agent-runtime module.
It persists a stable frontend port so browser storage survives restarts,
supervises the child, polls `/api/desktop-health`, backs off repeated restarts,
and kills stale or current children during replacement and shutdown. The agent
sidecar's exit also stops the dependent frontend. [source]

This is much better than assuming a child remains alive because the window is
open. The lifecycle is still PID/port/health based. It has no per-generation
secret in the shown server protocol, so a healthy process already listening on
the preferred agent port and returning the expected public health payload may
be adopted without a cryptographic identity check. [source]

### 4.3 Packaging and updates

Electron Builder packages arm64 macOS DMG/ZIP targets, Windows x64 NSIS, and a
Linux AppImage. The macOS build uses hardened runtime and a named Developer ID,
with an optional notarization command. The app is not App Sandbox-contained.
JIT and network client access are enabled, unsigned executable memory and DYLD
environment variables are disabled, and library validation is disabled.
[source]

The packaged resources include the Next server, agent sidecar, native PTY,
Pi extensions, MCP helpers, skills, plugins, and controller installer. That
bundle is already a component set even though the release surface presents it
as one app. A public signed ledger should bind all of those components plus the
compatible controller and runtime schema families before self-update.
[inferred]

## 5. Coding-agent runtime

### 5.1 Pi as an embedded engine

The sidecar uses `@earendil-works/pi-coding-agent` to create one runtime per
Local Studio session identity. It resolves a model from controller/provider
catalogs, creates Pi services and a `SessionManager`, resumes a matching Pi
session file, loads selected first-party extensions and skills, and subscribes
to typed Pi events. Ordinary user/drop-in Pi extensions are disabled in the
service constructor, while Local Studio explicitly supplies its own extension
paths. [source]

Turn input distinguishes `prompt`, `steer`, and `follow_up`. A busy prompt
defaults to steering. Explicit control requests reject when the target is no
longer active. This is a meaningful semantic improvement over treating every
message arriving during a stream as the same operation. [schema]

The response still exposes only accepted, queued, or rejected outcomes around
process-local state. It has no durable command id, idempotency key, admission
sequence, target generation, delivery acknowledgement, or terminal outcome
reference. A caller disconnect between acceptance and durable Pi persistence
therefore has no published exactly-once reconciliation law. [source]

### 5.2 Persistence and replay

Pi owns its JSONL session files. Local Studio separately stores session
metadata, archive state, project mapping, preferences, comments, plans, drafts,
and view state. Runtime events receive a monotonically increasing in-process
sequence and are retained in a 2,000-entry array. Status and SSE callers can ask
for events after a cursor. Replay clamps stale cursors and coalesces the UI.
[source]

This is good client resilience within one process generation. It is not durable
event admission:

- the event sequence starts at zero when the runtime fingerprint changes.
- the in-memory log drops older events after 2,000 entries.
- active state is process-local.
- Pi JSONL identity and Local Studio runtime identity are joined heuristically.
- several current projections live in separate JSON documents. And
- no schema version, synchronization marker, repair transaction, or loss
  receipt binds all of those facts.

OpenAgents should treat Local Studio as evidence for a useful adapter, not as a
replacement for its durable Thread/Turn/Item and Runtime Gateway contracts.

### 5.3 Workbench tools

The product exposes two tool planes:

1. Pi's coding-agent tools and Local Studio-supplied Pi extensions. And
2. Next/desktop workbench routes for files, Git, terminals, browser, projects,
   comments, plans, canvas, skills, plugins, and connectors.

Filesystem operations reject system roots and lexical/symlink escapes, hide
common generated/secret-adjacent directories from listing, cap file reads at
5 MB, reject binary-looking text, and write only existing files. Terminal
commands require a non-system absolute working directory, cap output at 2 MB,
and time out after 60 seconds. These are useful application bounds. [source]

They are not containment. A workspace may be nearly any non-system directory,
the terminal executes a shell command with host-user authority, PTYs are native
host processes, and Pi tool behavior depends on its own engine and extensions.
Path checks reduce accidental scope. They do not isolate a malicious command or
model-generated process.

### 5.4 Browser, skills, plugins, and connectors

The browser host launches Chromium and drives page targets over CDP. It bounds
text/HTML snapshots, message sizes, screencast activity, request time, and
snapshot count. The visible Computer panel and model tools share the hosted
page rather than controlling an Electron webview directly. [source]

Skills are discovered from Local Studio plus Claude, Pi, Codex, Factory,
OpenCode, and installed Codex application plugin roots. Full instructions are
read only for a selected skill and capped at 6,000 characters. This is useful
progressive disclosure, but source-name discovery is not provenance,
compatibility, review, or authority. [source]

Plugin discovery understands Codex plugin manifests. Bundled plugins can be
marked trusted. Other plugins may contribute skills, apps, and MCP servers.
Plugin server paths are constrained to the bundle when relative, connectors
start disabled, and tool allowlists can narrow enabled connectors. Google
Workspace bindings additionally require exact managed shapes and read-only MCP
annotations before use. These are thoughtful controls. [source]

Two important boundaries remain:

- an absolute stdio command in a plugin manifest is accepted as an executable
  target. And
- connector and plugin processes run with host-user authority and an inherited
  environment plus configured secrets.

Annotations, disabled-by-default state, and path containment do not replace
process isolation, immutable artifact identity, publisher trust, resource
bounds, egress policy, or effect receipts.

## 6. Security and data-flow assessment

### 6.1 Controller access is stronger than frontend access

The controller binds `127.0.0.1` by default. A non-loopback bind without an API
key fails startup unless `LOCAL_STUDIO_ALLOW_UNAUTHENTICATED=true` explicitly
opts out. When a key exists, every controller route except `/health` requires a
constant-time bearer or `X-API-Key` match. CORS is allowlisted, mutating and read
requests have separate bounded rate-limit stores, and long-lived monitoring
paths are intentionally exempt. [source]

Calling the middleware `createMutatingAuthMiddleware` is stale naming: it
actually authenticates reads and mutations. The implementation is stronger
than the name.

The explicit unauthenticated-LAN escape hatch remains a real danger mode. The
controller can install runtimes, launch arbitrary backends, delete or move
models, mutate providers, and proxy inference. “Trusted LAN” is not a durable
principal, and bearer possession is not operation-scoped authority.

### 6.2 Frontend access is opt-in, not safe by default

The frontend access module's prose first calls the default “safe-by-default,”
then its operative comment and code say gating is opt-in. In production, the
frontend remains open when `LOCAL_STUDIO_FRONTEND_TOKEN` is unset. Any nonempty
`LOCAL_STUDIO_DATA_DIR`—the desktop configuration—forces an unconditional allow
even if a token exists. [source]

The token middleware and route-level guards are useful when enabled, but the
default web deployment posture is materially weaker than the controller's
non-loopback fail-closed rule. Because the frontend exposes shell, filesystem,
Git, connectors, OAuth initiation, agent turns, browser control, and settings,
this is a host-code-execution boundary, not an ordinary dashboard login.

### 6.3 Ambient loopback is the desktop capability

The Electron server and agent sidecar both bind loopback. No generated
generation secret, peer credential, Unix-socket permission, DPoP proof, or
request-scoped capability is present in their shown HTTP contracts. The Next
server forwards caller headers to the agent sidecar, but the sidecar itself has
no authentication middleware. [source]

Loopback blocks remote network reach by default. It does not distinguish the
Local Studio renderer from another local process. Port discovery is easy for a
same-user process, and the stable frontend port is intentionally persisted.
OpenAgents should not adopt loopback location as local authority.

### 6.4 Model and extension code trust

Custom recipe launch commands are default-off behind an environment flag, a
good control. Conversely, the example configuration states that omitted
`trust_remote_code` defaults to true so models with custom modeling code work
out of the box. A model artifact can therefore become executable Python code
unless the operator hardens the default or recipe. [source]

The product also imports code through:

- Python packages and backend upgrades.
- Docker images.
- llama.cpp binaries.
- Pi extensions loaded through `jiti`.
- plugin MCP stdio commands.
- skills and prompt templates affecting model behavior. And
- Chromium/browser content plus connector responses.

These inputs need one supply-chain and capability law. Local custody does not
make downloaded model code, packages, extensions, or plugins trustworthy.

### 6.5 Data custody

Local Studio keeps model weights, controller SQLite, recipes, runtime venvs,
logs, usage, Pi sessions, metadata, connectors, projects, settings, and OAuth
material in several local roots. Connector configuration files are atomically
written and chmodded `0600`. Desktop OAuth values are encrypted through
Electron `safeStorage` before a `0600` JSON vault write. [source]

Remote data flow is feature-dependent:

- a controller may run on another host.
- model requests may reach a configured remote OpenAI-compatible provider.
- Hugging Face supplies model metadata and downloads.
- Google OAuth and Workspace adapters reach Google endpoints.
- MCP HTTP connectors reach configured servers.
- Sitegeist can use a relay token.
- update and release flows reach GitHub. And
- browser automation reaches arbitrary user/model-selected sites.

“Local-first” is directionally fair for default custody and execution, but it
does not summarize these independent flows. The UI should disclose exact
placement, content class, credential, retention authority, and egress per
operation.

## 7. Reliability, verification, and release posture

### 7.1 Strong implementation mechanics

The source contains many worthwhile reliability patterns:

- atomic temp-write/rename for JSON settings and metadata.
- serialized per-file updates.
- bounded HTTP request bodies, logs, browser snapshots, command output, and
  in-memory event history.
- explicit cancellation and cleanup for downloads, installs, launches,
  processes, OAuth callbacks, MCP calls, CDP messages, and child servers.
- launch failure budgets and health-based readiness.
- GPU lease conflict and release-after-stop behavior.
- session adoption and stale-navigation guards.
- child-process supervision and restart backoff. And
- log redaction plus public-safe error responses.

These mechanics show serious operational intent even though they lack current
executable regression coverage. [source]

### 7.2 Test corpus deletion at the audited boundary

The parent commit `c42511e9` added three recorded Playwright E2E files and still
contained a broad controller/frontend suite. The audited commit
`4a16109f` then removed:

- 132 tracked test, fixture, support, and replay files.
- 22,669 lines net from those files and related test configuration.
- controller unit and integration suites for process, runtime, recipe, proxy,
  speech, storage, GPU lease, redaction, streaming, and model contracts.
- frontend regression suites for sessions, replay, auth, filesystem bounds,
  plugins, OAuth, navigation, terminals, settings, usage, and browser tools.
- the three new Playwright E2E flows. And
- every package `test` and `test:integration` script. [history]

The commit subject, `fix: stabilize follow-up chat submission`, does not disclose
that verification reset. Current CI runs structural checks, controller
typecheck/lint/cleanup, and the frontend quality gate. The quality gate runs
lint, typecheck, dependency/dead-code/duplication/cycle checks, and a production
build, but no behavioral test. [history] [source]

Meanwhile:

- root `AGENTS.md` still requires frontend tests and integration tests.
- the README still tells contributors to run `npm run test:integration`.
- the pull-request template still suggests tests. And
- `playwright` remains a frontend dependency and config file without tracked
  E2E specifications.

This is not documentation polish. It removes the only executable evidence for
many of the most consequential boundaries described above. The exact test
deletion should be treated as a release blocker until it is intentionally
explained or reversed. [inferred]

### 7.3 Release chain

GitHub Actions runs static CI, security scanning, CodeQL, dependency review,
semantic release, labels, and Pages. Release waits for the CI workflow's
successful `main` push, then tags and publishes GitHub release notes. Desktop
artifacts are built separately on a Developer-ID-equipped Mac and uploaded to
the release. [source]

The pipeline has useful commit-to-release ordering, but at this snapshot the
successful CI prerequisite proves no runtime behavior. The public source also
does not establish a publisher-signed application manifest binding hashes,
SBOM/provenance, updater metadata, controller compatibility, agent sidecar,
extensions, model-runtime families, activation, last-known-good state, and
rollback receipt. [limitation]

## 8. Comparison with the reference set

| Concern               | Local Studio                                                         | Closest or stronger reference                                 | OpenAgents consequence                                                 |
| --------------------- | -------------------------------------------------------------------- | ------------------------------------------------------------- | ---------------------------------------------------------------------- |
| Local model lifecycle | First-class vLLM/SGLang/llama.cpp/MLX install, launch, health, evict | None in the teardown set at this breadth                      | Adapt the typed runtime/placement lifecycle                            |
| Hardware truth        | NVIDIA/AMD/Intel/ROCm/CUDA probes, GPU leases, metrics               | T3 environment capabilities. Crabbox provider facts           | Make accelerator/capacity/lease facts explicit and receipted           |
| Model endpoint        | OpenAI-compatible proxy over local or configured providers           | OpenCode provider layer                                       | Keep compatibility above exact backend/model identity                  |
| Desktop shape         | Electron -> Next server -> Pi sidecar                                | OpenCode Electron server. Factory daemon                      | Prefer one generated engine contract and authenticated generation      |
| Agent runtime         | Embedded Pi SDK with prompt/steer/follow-up                          | Amp steer/queue. OpenCode V2 admission                        | Add durable command identity, admission, replay, and terminal outcomes |
| Sessions              | Pi JSONL plus Local Studio metadata and volatile event log           | Codex SQLite+JSONL. Claude append histories                   | Preserve adapter evidence but project into one canonical typed log     |
| Workbench             | Files, Git, terminals, browser, plans, canvas, projects              | Factory/OpenChamber/T3                                        | Keep progressive depth without renderer authority                      |
| Extensions            | Skills, Codex plugins, MCP, Google adapters, Pi extensions           | Executor/OpenCode/Factory                                     | Require immutable capability generations and isolation                 |
| Host access           | Opt-in frontend token. Ambient loopback desktop/sidecar              | Codex scoped app-server. T3 DPoP environment access           | Reject port possession as authority                                    |
| Workload isolation    | Not established for ordinary Pi/shell/plugin execution               | Codex Seatbelt/Landlock. Claude Cowork guest                  | Treat permission, policy, and containment separately                   |
| Release proof         | Static CI, GitHub release, optional notarized Mac build              | OpenCode target matrix. OpenAgents signed ledger requirements | Restore behavior tests and bind every component to signed promotion    |
| Test posture          | No tracked executable tests at audited commit                        | Grok PTY/race suite. Prior Local Studio parent                | Do not admit the current gate as behavioral verification               |

## 9. What OpenAgents should adapt

### 9.1 Make local inference a typed placement

Represent a local model execution with exact, independently inspectable facts:

```text
placement + host generation + accelerator inventory + lease
+ backend/runtime artifact + model artifact + trust mode
+ launch command digest + endpoint + health + capacity
+ context/features + observed usage + stop/cleanup receipt
```

The friendly model selector may remain simple. Evidence and supervision must
retain the complete identity.

### 9.2 Separate model control from agent control

Local Studio correctly distinguishes controller concerns from Pi session
concerns. OpenAgents should keep its Runtime Gateway/Thread authority separate
from a local-inference supervisor. A model server becoming healthy does not
admit an agent turn. An agent turn completing does not prove model-process
cleanup or release capacity.

### 9.3 Adopt backend specifications, not backend conditionals

Each inference family should provide a typed capability implementation for:

- discovery and compatibility.
- install/upgrade identity.
- model artifact compatibility.
- launch arguments and environment.
- health/readiness.
- usage/metrics.
- graceful and forced stop. And
- recovery/cleanup.

Local Studio's engine-spec pattern is the right starting point. OpenAgents
should add immutable artifact identity, Effect resource scope, receipts, and
remote lease/fence semantics.

### 9.4 Make accelerator ownership visible

Adapt GPU leases and speech/LLM conflict handling. Strengthen them into
restart-safe, generation-bound reservations with observed process attachment,
capacity accounting, and release only after exact workload termination. A
lease record alone is not proof that VRAM is free.

### 9.5 Preserve the sidecar/workbench split

Files, Git, terminals, browser control, connectors, and sessions should remain
server/host-owned while Desktop and mobile render typed projections. Replace
ambient ports with authenticated generation-scoped capabilities and one public
generated protocol across embedded, remote, CLI, SDK, and test transports.

### 9.6 Treat local-model UX as an honest operational surface

Local Studio's Status/Configure/Usage consolidation is a strong product model.
Users need one answer to:

- what hardware is available.
- what is installed.
- what model is loaded.
- what owns each accelerator.
- whether the endpoint is healthy.
- what a turn will cost in time, memory, energy, and remote data flow. And
- how to stop or repair it.

Those facts should deepen the workroom without turning the workroom into a GPU
admin console by default.

### 9.7 Reuse bounded operational mechanics

Carry forward Local Studio's atomic JSON writes, body/output caps, install
locks, cancellable jobs, process-tree cleanup, launch failure budgets, health
gates, redaction, and child supervision. Re-express load-bearing paths as
Effect resources with deterministic tests and typed failure outcomes rather
than scattered best-effort cleanup.

## 10. What OpenAgents should reject

1. **No ambient loopback authority.** Port knowledge or a public health payload
   cannot authorize agent turns, shell, files, browser, connectors, or OAuth.
2. **No opt-in protection for a privileged web server.** Any non-loopback
   frontend or controller fails closed without an enrolled principal and
   scoped capability.
3. **No host-user execution presented as containment.** Pi, shell, PTY, plugin,
   MCP, and model code need explicit effective isolation facts.
4. **No default `trust_remote_code=true`.** Executable model artifacts require
   explicit immutable identity, review/policy, isolation, and disclosure.
5. **No provider string as model identity.** Preserve backend, runtime, model
   artifact, quantization, context/features, host, and generation.
6. **No volatile event cursor as durable replay.** Keep stable command/event
   identities, admission, promotion, replay, projection repair, and loss state.
7. **No cross-product skill discovery as trust.** Discovery source, publisher,
   bytes, review, compatibility, capabilities, activation, and generation are
   separate facts.
8. **No plugin stdio process without isolation.** Relative-path containment and
   a tool allowlist do not constrain host filesystem, egress, secrets, or child
   processes.
9. **No bearer-or-LAN binary policy.** Device/client identity and operation
   capability should be narrow, revocable, expiring, and receipt-bound.
10. **No static-only release gate for a privileged agent.** Typecheck, lint,
    build, CodeQL, and secret scan cannot replace behavior, fault, packaging,
    and real-runtime proofs.
11. **No misleading verification reset.** Large test removal requires an
    explicit rationale, replacement evidence, documentation reconciliation,
    and release disposition.
12. **No one-app version hiding component drift.** Bind Desktop, renderer,
    sidecar, controller, backend, extension catalog, and schema compatibility
    in a signed ledger.

## 11. Recommended OpenAgents sequence

1. Define a provider-neutral `LocalInferencePlacement` schema covering host,
   accelerator, runtime, model artifact, trust, endpoint, health, capacity,
   usage, and lifecycle generations.
2. Implement one backend adapter first—likely llama.cpp or MLX for the narrowest
   owner-local proof—behind the same placement lease used by other runtimes.
3. Add accelerator inventory and generation-bound resource leases with
   restart/reconciliation tests before multi-backend scheduling.
4. Bind local model readiness to the existing Runtime Gateway without making
   the inference controller a second Thread/Turn authority.
5. Project exact backend/model/placement/data-flow facts into Desktop and
   mobile model selection and usage receipts.
6. Add vLLM/SGLang only after install, remote-code, container, driver, health,
   cancellation, orphan cleanup, and rollback profiles are explicit.
7. Keep extensions and model code in named fail-closed containment profiles.
   never inherit host authority from “local.”
8. Require restored behavioral, fault, packaging, and real-backend evidence
   before any public local-inference promise.

These are research and architecture inputs. They do not supersede the Sol
roadmap, ProductSpec, AssuranceSpec, current issue/claim state, or accepted
OpenAgents invariants.

## Final assessment

Local Studio makes the self-hosted model operational. It treats model serving
as more than an API base URL: runtimes must be found or installed, hardware
must be understood and leased, models must be downloaded and matched to a
backend, processes must become healthy, usage must be observed, and failed or
superseded workloads must be cleaned up. Combining that controller with a real
coding workbench makes the repository an unusually relevant OpenAgents
reference.

The same combination raises the trust bar. An app that can install Python,
load remote model code, launch containers and processes, edit files, run a
shell, drive Chromium, decrypt OAuth material, and call plugins cannot use
loopback, same-user access, annotations, or UI approvals as its primary safety
law. Its event and component identities also need to survive restarts and
updates, not only healthy-process polling.

Most urgently, the audited revision removed the public executable evidence for
the very boundaries that make the product consequential. The source remains
valuable architecture evidence. The current green CI is not behavioral proof.

OpenAgents should adapt the controller grammar and strengthen the laws: local
inference as a typed receipted placement, explicit accelerator ownership,
generated authenticated protocols, durable admission and replay, isolated
model/extension execution, exact component identity, and release gates that
exercise the real system.

## Primary source map

### Commit-pinned source

- `projects/repos/local-studio` at
  `4a16109fae58b12b03d172a3463cc62cf244a758`
- <https://github.com/sybil-solutions/local-studio/tree/4a16109fae58b12b03d172a3463cc62cf244a758>

### Architecture and manifests

- `README.md`
- `controller/README.md`
- `frontend/README.md`
- `package.json`
- `controller/package.json`
- `frontend/package.json`
- `services/agent-runtime/package.json`
- `controller/contracts/`
- `shared/agent/`

### Controller

- `controller/src/http/app.ts`
- `controller/src/http/security-middleware.ts`
- `controller/src/config/env.ts`
- `controller/src/modules/engines/`
- `controller/src/modules/system/gpu-leases.ts`
- `controller/src/modules/proxy/`
- `controller/src/stores/`

### Desktop, workbench, and agent runtime

- `frontend/desktop/logic/app-server.ts`
- `frontend/desktop/logic/agent-runtime-server.ts`
- `frontend/desktop/logic/security.ts`
- `frontend/desktop/logic/window-manager.ts`
- `frontend/desktop/preload.ts`
- `frontend/desktop/electron-builder.yml`
- `frontend/src/lib/auth/access.ts`
- `frontend/src/proxy.ts`
- `services/agent-runtime/src/server.ts`
- `services/agent-runtime/src/pi-runtime.ts`
- `services/agent-runtime/src/plugin-runtime.ts`
- `services/agent-runtime/src/skill-discovery.ts`

### Verification and history

- `.github/workflows/ci.yml`
- `.github/workflows/release.yml`
- `AGENTS.md`
- parent test snapshot `c42511e9`
- audited deletion boundary `4a16109f`
