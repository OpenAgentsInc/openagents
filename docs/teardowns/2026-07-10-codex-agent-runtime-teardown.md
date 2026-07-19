# Codex CLI and Agent Runtime Architecture Teardown — 2026-07-10

Read-only architecture audit of the open-source OpenAI Codex repository at
commit `08ba14b03d0b3ce3cfdf8c88c0469b9b1924953d` (committed 2026-07-11
UTC, still 2026-07-10 in the workspace timezone). The purpose is product and
architecture research for OpenAgents, following the structure and depth of the
[Claude Code architecture teardown](./2026-07-10-claude-code-teardown.md).

This is separate from the
[ChatGPT desktop app teardown](./2026-07-10-chatgpt-desktop-app-teardown.md).
That document examines the installed macOS host and its bundled Codex
relationship. This document examines the open Rust agent engine, terminal UI,
app-server protocol, SDKs, persistence, sandboxing, extensions, subagents,
remote control, and release system that clients build upon.

Every claim is tagged:

- **[source]** — directly observed in the commit-pinned repository
- **[schema]** — directly encoded in a typed protocol, config, or generated
  schema
- **[test]** — explicitly exercised by repository tests or snapshots
- **[public]** — corroborated by current official OpenAI documentation
- **[inferred]** — a conclusion drawn from several visible mechanisms
- **[limitation]** — a boundary on what this audit can establish

The repository checkout contained one unrelated untracked local artifact named
`codex`. It was not read, modified, or used as evidence.

## TL.DR

Codex is an open, local-first Rust agent runtime whose most important
architectural boundary is `codex app-server`. The app server exposes a typed,
bidirectional Thread → Turn → Item protocol over JSON-RPC-like messages and can
run over stdio, an experimental WebSocket listener, a private Unix control
socket, or an in-process typed client. The terminal UI now uses that same app
server rather than owning a separate conversation engine. This is the clearest
source-backed validation yet of OpenAgents' planned one-engine/many-clients
architecture.

The repository is a complete production monorepo rather than a source-shaped
extract: 5,386 tracked files, approximately 1.13 million lines of Rust, 125
Cargo workspace members, TypeScript and Python SDKs, generated TypeScript and
JSON schemas, Cargo and Bazel builds, cross-platform release automation, 917
test-bearing files, and 587 UI snapshots. Its Apache-2.0 license makes the
load-bearing seams inspectable and reusable as design reference.

Codex's app-server V2 protocol is unusually strong. A client initializes with
capabilities, starts/resumes/forks a thread, starts or steers a turn, receives
typed item lifecycle and delta notifications, resolves approvals and MCP
elicitations, manages settings/auth/models/plugins/skills/apps, and can inspect
or mutate thread state without parsing terminal output. The protocol generates
version-matched TypeScript and JSON Schema artifacts, gates experimental
members explicitly, and has public integration tests.

Persistence is hybrid by design. Append-only JSONL rollout files remain the
durable, inspectable conversation record under `~/.codex/sessions`, while a
migrated SQLite state database indexes thread metadata, goals, memories,
dynamic tools, remote-control enrollments, agent jobs, and parent/child spawn
edges. A storage-neutral thread store and a dedicated agent-graph interface
separate consumers from the local representation. This directly improves on
the implicit sidechain topology seen in Claude Code.

Security is a major subsystem, not a prompt convention. Typed permission
profiles describe filesystem and network access. Managed requirements constrain
what users can select. Approval policy is distinct from sandbox enforcement.
execpolicy evaluates token-prefix rules with explicit allow/prompt/forbidden
outcomes. MacOS uses Seatbelt, Linux defaults to bubblewrap plus namespaces and
seccomp, and Windows has restricted-token/elevated sandbox paths. A managed
network proxy adds domain, method, local-network, Unix-socket, and optional TLS
inspection policy with structured audit events.

Remote control is also a real distributed protocol. A Unix app-server daemon
owns lifecycle and update behavior. ChatGPT-authenticated enrollment creates a
remote environment. Short-lived pairing grants controller access. WebSocket
envelopes carry client, stream, and sequence identity. Acknowledgements and
cursors support replay. Large JSON-RPC messages are bounded and segmented. And
controller grants can be listed and revoked. This is a closer architectural
match for Khala Sync than pixel streaming.

The strongest Codex lessons for OpenAgents are:

1. make the same typed engine protocol serve the local TUI, desktop, SDK, and
   remote clients.
2. pair append-only human-readable history with an indexed canonical state and
   explicit agent graph.
3. compile named permission profiles into OS-enforced filesystem/network
   policy and keep approval separate.
4. generate clients and compatibility fixtures from the protocol source.
5. use bounded queues, overload errors, replay identity, and monotonic states.
6. make every context fragment typed, attributed, and capped.
7. treat remote-control daemon lifecycle and updates as explicit product state.
   and
8. test user-visible terminal projections with snapshots and engine behavior
   through the public protocol.

The main warnings are scale and migration complexity. The workspace has 125
crates, a 92-entry feature registry including removed compatibility flags,
parallel legacy and next-generation permission/config concepts, V1 and V2 app
server surfaces, JSONL plus SQLite plus auxiliary stores, and a very large TUI
surface. The source itself warns that `codex-core` is bloated and that several
central TUI modules should not grow further. OpenAgents should adapt the
boundaries, not the accumulated compatibility burden.

## 1. Snapshot identity and confidence

| Field | Value | Evidence |
| --- | --- | --- |
| Repository | OpenAI Codex | [source] |
| Snapshot commit | `08ba14b03d0b3ce3cfdf8c88c0469b9b1924953d` | [source] |
| Snapshot time | 2026-07-11 01:42 UTC | [source] |
| License | Apache-2.0 | [source] `LICENSE`, workspace metadata |
| Tracked files | 5,386 | [source] |
| Rust files | 2,506 | [source] |
| Approximate Rust lines | 1,126,396 | [source] |
| Cargo workspace members | 125 | [source] |
| Test-bearing files | 917 | [source] |
| UI snapshots | 587 | [test] |
| State SQL migrations | 45 across active/auxiliary stores | [source] |
| Generated app-server TypeScript schema files | 601 | [schema] |
| Primary runtime | Rust 2024 edition, Tokio async runtime | [source] |
| Primary local UI | Ratatui/Crossterm terminal application | [source] |
| Public SDKs | TypeScript and Python | [source] [public] |
| Build systems | Cargo/Just and Bazel | [source] |

Unlike the historical Claude Code snapshot, this tree contains a license,
package manifests, build instructions, release automation, schema generation,
integration tests, cross-platform sandbox code, and active version history.
Architectural confidence is therefore materially higher. [source]

The audit still has limits:

- it did not execute the full million-line test suite.
- hosted ChatGPT/Codex backend behavior is visible only through client
  contracts and mocks.
- experimental and under-development features may not ship to every account or
  binary.
- public docs can lag or lead the exact commit. And
- private deployment, policy, model, and entitlement configuration is outside
  this repository. [limitation]

Current official documentation was consulted only to corroborate public
product surfaces. The commit-pinned source remains the primary architecture
evidence.

## 2. Architecture at a glance

```text
  Codex TUI       codex exec       Python/TS SDK       IDE/Desktop/Mobile
      |                |                 |                      |
      |      JSONL wrapper or typed app-server client           |
      +----------------+-----------------+----------------------+
                               |
                         Codex app-server
              in-process | stdio | Unix socket | WebSocket
                               |
                 Thread -> Turn -> Item protocol
            settings, auth, models, tools, approvals,
              plugins, skills, MCP, goals, subagents
                               |
                         codex-core runtime
          model streaming, context, tools, compaction,
             policy, hooks, orchestration, telemetry
                               |
       +-----------------------+------------------------+
       |                       |                        |
  Local authority        Execution boundary       Durable state
  config/secrets/Git     Seatbelt/bwrap/Windows   JSONL rollouts
  MCP/plugins/skills     PTY/network proxy        SQLite indexes
                                                  graph/jobs/memory
                               |
                Model APIs and remote control relay
```

[source] The most important property is that the TUI is now a client of the
app-server contract. The `tui_app_server` feature remains only as a removed
compatibility flag because app-server use is unconditional. The TUI can embed
the app server in-process or connect to a remote instance through the same
typed facade.

| Concern | Primary owner | Architectural effect |
| --- | --- | --- |
| Conversation execution | `codex-core` behind app-server | clients do not own model/tool loops |
| Client protocol | app-server V2 Thread/Turn/Item API | terminal, IDE, desktop, SDK, and remote can converge |
| Local presentation | Ratatui TUI | UI is a projection with its own bounded state machines |
| Noninteractive automation | `codex exec` and SDK adapters | JSONL events and structured final output |
| Persistence | rollout JSONL + SQLite state + thread store | inspectability plus query/index performance |
| Agent topology | dedicated graph store + thread metadata | parent/child lifecycle is explicit |
| Authority | config requirements, profiles, approvals, policy | organization, user, and runtime constraints compose |
| Containment | OS sandbox + network proxy | permissions are enforced below model/tool prose |
| Extensions | MCP, apps, skills, plugins, hooks, code mode | capabilities have typed/provenanced integration points |
| Distribution | native packages, npm, installers, daemon updater | runtime is independently usable and remotely maintainable |

## 3. Repository and runtime shape

### 3.1 A large Rust workspace with intentionally narrow crates

[source] The `codex-rs` workspace has 125 members spanning protocol, app
server, transport, CLI, TUI, core, tools, rollout storage, SQLite state,
sandboxing, network proxy, auth, model providers, MCP, plugins, skills, hooks,
multi-agent graphs, remote control, code mode, cloud tasks, SDK support, and
utilities.

The repository contract explicitly warns contributors not to keep adding new
concepts to `codex-core`. It prefers new focused crates, private modules, narrow
public APIs, and modules below roughly 500 lines where practical. Several
central TUI files are named as high-risk growth points. [source]

[inferred] That guidance is evidence of a real pressure: a successful agent
core attracts every cross-cutting capability. Crate count alone is not a
problem, but the boundary map must remain comprehensible. OpenAgents should
keep fewer Effect packages with clearer dependency direction and use service
interfaces before package proliferation.

### 3.2 Cargo and Bazel together

[source] Local development uses Cargo, Just, rustfmt, Clippy, nextest, and
snapshot tooling. Bazel supplies hermetic and cross-platform build/test paths,
including release targets, Windows/Wine support, pinned V8 dependencies, and
lock drift checks. Dependency changes require both Cargo and Bazel lock
updates.

[test] The repository contract requires targeted crate tests for changes,
integration tests for agent-logic changes, app-server tests against the public
JSON-RPC surface, and TUI snapshots for user-visible changes.

[inferred] Dual build systems add maintenance cost but provide release
reproducibility and platform coverage a fast-moving agent runtime needs.
OpenAgents should not copy Bazel by default. It should copy the principle that
release artifacts and source-local tests exercise the same boundaries.

### 3.3 Native binary plus package wrappers

[source] The core product ships as native Rust binaries. The npm package is a
thin launcher that selects platform-specific packages. Standalone install
scripts, GitHub release archives, Homebrew, npm, and a DotSlash artifact cover
different distribution workflows. Python runtime packaging and SDK packages
have separate release lanes.

[inferred] The native engine is the stable product artifact. JavaScript and
Python packages adapt it rather than reimplement it. This is exactly the right
shape for an engine intended to serve several UI stacks.

## 4. Product surfaces and entry modes

### 4.1 Interactive TUI

[source] `codex` launches a full terminal application supporting streamed
conversation, reasoning and tool projections, approvals, plans, review,
multi-agent navigation, goals, model/effort/personality selection, MCP and
plugin surfaces, session resume/fork/archive/delete, background terminals,
images, notifications, and diagnostics.

### 4.2 Noninteractive execution

[source] `codex exec` runs a new or resumed thread, emits human output or typed
JSONL, supports structured output through JSON Schema, accepts images, and can
write the final model message to a selected file. It has explicit ephemeral
execution, strict config validation, user-config suppression, rule suppression,
Git repository validation, review mode, and the dangerous combined bypass
flag.

[schema] The typed exec event projection includes thread lifecycle, turn
lifecycle, command execution, patch changes, MCP tools, collaboration tools,
agent status, usage, errors, and final output.

### 4.3 App server

[source] `codex app-server` is the rich-client engine API. It supports stdio,
experimental WebSocket, private Unix-socket control, in-process embedding,
schema generation, a test client, a proxy for the control socket, daemon
lifecycle, and remote control.

### 4.4 SDKs

[source] The TypeScript SDK wraps the CLI, exchanges JSONL over stdin/stdout,
and exposes thread/turn objects, buffered or streaming execution, structured
output, images, resume, working-directory control, and explicit environment
construction.

[source] The beta Python SDK is app-server-oriented and exposes sync/async
thread lifecycle, streaming, approvals, auth, goals, settings, and generated V2
types. Its test suite validates public signatures and real app-server behavior.

[inferred] Two SDK integration generations are visible: exec-JSONL wrapping is
simple and robust for automation, while the app-server protocol is the richer
long-lived application boundary. OpenAgents should publish one protocol-native
SDK and offer one-shot helpers on top, not maintain semantically separate SDKs.

### 4.5 Codex as an MCP server

[source] `codex mcp-server` lets another agent or host consume Codex over MCP.
It translates MCP tool calls into Codex runs and carries execution/patch
approval elicitations through the MCP relationship.

### 4.6 Cloud, review, apply, sandbox, and diagnostics

[source] Additional commands browse/execute cloud tasks, apply cloud-generated
diffs locally, review uncommitted/base/commit changes, evaluate execpolicy,
run arbitrary commands inside the platform sandbox, manage MCP/plugins, and
produce a redacted diagnostic report covering installation, configuration,
auth, network, Git, terminal, sandbox, app-server, updates, and thread state.

[public] Current official command documentation identifies TUI, exec, app
server, MCP server, remote control, sandbox, review, plugin, cloud, doctor,
archive/delete/fork/resume, and update as distinct surfaces.

## 5. The TUI as an app-server client

### 5.1 The migration is complete at the engine seam

[source] The TUI owns an app-server session facade that keeps typed requests
and responses out of general UI components. It can create an in-process client
or target a remote app server. The same facade starts, resumes, forks, reads,
lists, archives, deletes, compacts, steers, interrupts, and configures threads.

[source] A removed compatibility flag states that the TUI now always uses the
app-server implementation. This is a crucial contrast with the audited Claude
Code snapshot, where interactive and SDK execution still had separate query
owners.

[inferred] Codex has paid the migration cost OpenAgents should avoid creating:
the rich local UI no longer needs a privileged private path into the engine.
OpenAgents Desktop and terminal should begin on the common protocol rather than
converge later.

### 5.2 In-process does not bypass the contract

[source] The embedded app-server path uses typed request/notification enums and
bounded channels instead of serializing every message, but it follows the same
message processor and traces requests as an `in-process` transport. The remote
path serializes JSON-RPC over its transport.

This provides performance without forking semantics. A local TUI call and a
remote desktop call reach the same request processor and state machine.

### 5.3 UI still has substantial state

[source] The TUI remains a large application. Ratatui and Crossterm handle
terminal drawing and input. App-level modules coordinate redraw, paste bursts,
focus, scrollback/reflow, composer state, approvals, background terminals,
agent navigation, goals, notifications, settings, and remote events.

[test] User-visible behavior is captured in hundreds of `insta` snapshots,
including platform-specific variants, approval prompts, resumed histories,
multi-agent progress, hooks, goals, MCP failures, and background execution.

[inferred] A unified engine protocol does not eliminate presentation state. It
does make presentation replaceable and testable against fixtures.

### 5.4 Compatibility residue

[source] The TUI still imports legacy core configuration through an app-server
client compatibility layer and contains fallbacks for older remote app servers
that do not support newer settings methods. The source labels several paths as
legacy or compatibility projections.

[inferred] Protocol migration succeeds only when deletion gates are explicit.
OpenAgents should version its first public protocol conservatively and attach a
removal milestone to every compatibility projection.

## 6. Core conversation engine

### 6.1 Submission and event model

[schema] The internal protocol uses typed operations submitted to a thread and
typed events emitted back. Operations cover user turns, interruption,
approvals, settings, dynamic tool results, MCP elicitation, history, compaction,
review, realtime input, and collaboration controls. Events cover configuration,
turn and item lifecycle, deltas, commands, patches, tools, usage, rates,
warnings/errors, hooks, agents, compaction, and realtime state.

[source] A thread manager owns loaded conversations. Each conversation has
scoped runtime resources, a submission channel, an event stream, persistence,
configuration, model/provider state, tools, MCP clients, and cancellation.

### 6.2 Thread → Turn → Item is the public semantic model

[schema] App-server formalizes:

- **Thread:** the durable conversation and runtime settings.
- **Turn:** one user-driven execution interval with explicit status. And
- **Item:** a typed unit within the turn.

Item types include user and agent messages, hook prompts, reasoning, plans,
command execution, file changes, dynamic tools, MCP calls, web search, image
operations, collaboration calls, subagent activity, review-mode boundaries,
and context compaction. [schema]

[inferred] This is much stronger than exposing provider-native response items
directly. It gives clients a product-level vocabulary while preserving raw
response events as an explicitly experimental seam.

### 6.3 Steering and interruption

[schema] A client can add input to an already-running ordinary turn through
`turn/steer`. Review and compaction turns are explicitly non-steerable.
`turn/interrupt` identifies both thread and turn, and the terminal state becomes
`interrupted`.

[test] App-server and TUI tests cover stale events, retries, turn identity,
interruption, settings changes, and replay behavior.

### 6.4 Review and goal execution are distinct turn classes

[source] Review has its own targets and delivery behavior. Persistent goals
have a separate extension, accounting, continuation prompts, steering, status,
budget, and API surface. This prevents “keep going” automation from being
implemented only as repeated user messages.

[inferred] Goals are an important precedent for OpenAgents standing workflows:
continuation, budgets, pause/block/complete states, and user steering belong in
the runtime rather than an external polling script.

## 7. App-server protocol

### 7.1 Transport and handshake

[source] App-server uses JSON-RPC 2.0 semantics while omitting the usual
`jsonrpc: "2.0"` field. Supported transports are:

- newline-delimited JSON over stdio.
- experimental WebSocket, one message per text frame.
- WebSocket framing over a private Unix socket. And
- typed in-process channels.

[source] Each connection must initialize once with client metadata and
capabilities, then send an initialized notification. Calls before initialization
and repeated initialization are rejected. Clients may opt out of exact
notification methods and opt into experimental APIs and extended MCP forms.

### 7.2 Bounded queues and overload

[source] Transport ingress, request processing, and outbound writes use bounded
queues. Saturated ingress returns a specific retryable server-overloaded error.
clients are told to use exponential backoff with jitter.

[inferred] Backpressure is part of the protocol contract, not merely a Tokio
implementation detail. OpenAgents' Runtime Gateway should likewise define
queue bounds, shedding priority, retry classification, and replay behavior.

### 7.3 V2 discipline

[source] Repository rules require active API development in V2. Methods use
resource/action naming, typed `Params`/`Response`/`Notification` payloads,
camelCase wire fields, string IDs at the boundary, cursor pagination for new
list methods, explicit tagged unions, and nullable optional fields with
consistent TypeScript generation.

[schema] Experimental methods and fields are annotated and omitted unless the
client opts in. Schema fixtures are generated from Rust to TypeScript and JSON
Schema, including an experimental bundle.

[test] Protocol tests verify serialization, generated fixture drift,
experimental filtering, request/response shapes, and app-server behavior.

### 7.4 Thread lifecycle API

[schema] The surface includes:

- start, resume, fork, read, list, loaded-list, archive, unarchive, delete, and
  unsubscribe.
- paginated turns and items.
- names, metadata, goals, memory eligibility, and settings.
- compact, rollback compatibility, shell commands, and background terminals.
- start, steer, and interrupt turn.
- review, auth, models, skills, plugins, apps, MCP, config, filesystem, and
  remote-control resources. And
- streamed thread, turn, item, hook, usage, settings, goal, and status
  notifications.

[source] Threads may be ephemeral. Forking can stop at a completed turn and
records source identity. Forking an active thread avoids inheriting an
unmarked partial suffix by writing an interruption boundary.

### 7.5 Subscription lifecycle

[source] Starting, resuming, or forking auto-subscribes the connection to that
thread. Unsubscribing does not immediately destroy active state. A loaded thread
with no subscribers and no activity is eventually unloaded and emits a closed
notification.

[inferred] Subscription, loaded residency, durable existence, and active turn
are separate states. OpenAgents should preserve that distinction in Khala Sync.

### 7.6 One dangerous exception is explicit

[source] The user-initiated bang-shell app-server method runs unsandboxed with
full access rather than inheriting the thread sandbox. The documentation says
so directly.

[inferred] Explicit documentation is better than hidden escalation, but a
general client protocol should not let a renderer accidentally invoke such a
method. OpenAgents should put host-full-access actions in a separately granted
capability namespace.

## 8. SDK architecture

### 8.1 TypeScript: process wrapper

[source] The TypeScript SDK launches the Codex CLI and exchanges structured
JSONL. It is convenient for Node workflows and Electron hosts because callers
can fully replace the child environment and keep engine authority out of the
JavaScript process.

[test] Tests cover execution, streaming, abort, environment setup, and response
proxy behavior.

### 8.2 Python: protocol client

[source] The Python SDK manages an app-server lifecycle and exposes generated
V2 models plus higher-level sync and async clients. It supports authentication,
threads, turns, streaming, approvals, settings, goals, and artifact workflows.

[test] Tests validate public API signatures, contract generation, inputs,
streaming, approvals, turn controls, login, lifecycle, and real app-server
integration.

### 8.3 OpenAgents consequence

[inferred] The app-server-native model is the durable one. OpenAgents should
generate TypeScript, Swift/Kotlin, and Rust clients from one Effect Schema
contract, then provide `exec`-style convenience wrappers as applications of
that client.

## 9. Models, providers, and authentication

### 9.1 Responses API as the common wire

[source] The current model-provider abstraction centers on the Responses API.
The old chat wire is rejected with migration guidance. First-party ChatGPT and
API access, custom OpenAI-compatible providers, Azure-compatible endpoints,
Amazon Bedrock, Ollama, and LM Studio are represented behind provider
interfaces.

[source] Provider objects own runtime base URL, catalog, authentication, and
request behavior. Model selection, reasoning effort, service tier, prompt
caching, streaming transports, rate limits, and fallback behavior are resolved
below the UI.

### 9.2 Authentication modes

[source] Login supports ChatGPT browser OAuth, device authorization, API keys,
and access-token input. Managed configuration can force a login method and pin
a ChatGPT workspace. Remote control requires ChatGPT authentication and rejects
API-key-only auth.

[source] OAuth callback logging deliberately redacts query parameters,
fragments, embedded credentials, tokens, codes, and sensitive request/response
fields. Tests assert that synthetic secrets never enter logs.

### 9.3 Credential storage

[source] Configurable storage supports file, keyring, or automatic selection.
The newer secret backend stores an encryption passphrase in the platform
keyring and writes age-encrypted auth material under `~/.codex/secrets`, then
removes fallback `auth.json`. Compatibility paths still support direct keyring
entries and file auth.

[test] Storage tests cover preference, migration, fallback, deletion across all
backends, keyring errors, and removal of obsolete plaintext files.

[inferred] This is materially stronger than Claude Code's visible non-macOS
plaintext-only path, but compatibility fallback remains a policy decision.
OpenAgents should require encrypted storage for long-lived authority and report
the effective backend.

### 9.4 Account and provider boundaries

[source] App-server exposes account state, login/logout flows, rate limits, and
model catalogs without handing raw credentials to the client. The TUI bootstrap
reads account and model state through app-server.

[inferred] This is the correct tokenless-renderer pattern for OpenAgents:
clients receive capability and account projections, while the host runtime owns
credential material and refresh.

## 10. Typed tool runtime

### 10.1 Registry, router, handler, runtime

[source] Tools are assembled from typed specs and executors into a registry.
The router converts provider response items into internal calls, resolves
namespaces, checks handler presence and parallel capability, then delegates to
runtime-specific implementations.

[schema] Tool specs include function, freeform, web-search, search/deferred,
and namespace shapes. Tool names carry optional namespaces rather than relying
only on one flat string.

### 10.2 Built-in capabilities

[source] Visible built-ins include:

- shell and a unified PTY-backed exec/write pair.
- apply-patch with a dedicated parser.
- plan and user-input requests.
- permission requests.
- image viewing and generation extensions.
- web search.
- MCP calls and MCP resource access.
- dynamic client-provided tools.
- tool search and deferred loading.
- current time, context remaining, sleep, and environment wait.
- plugins and installation requests.
- multi-agent, agent jobs, and goals. And
- experimental JavaScript code mode.

### 10.3 Lifecycle and projection

[source] Tool execution emits normalized start, delta/progress, success, and
failure events. Shell and patch executions become typed Turn Items for clients.
Approvals are resolved before the privileged runtime action. Cancellation
tokens propagate into nested operations.

[test] Tool router and handler tests cover schemas, parallel declarations,
namespaced MCP behavior, patch parsing, shell execution, permission requests,
dynamic result cancellation, and event projection.

### 10.4 Parallel execution

[source] The router asks each handler whether it supports parallel calls.
parallel scheduling is explicit. Namespaced tools do not accidentally inherit
parallel behavior from a same-named local tool. MCP parallel capability comes
from server metadata.

[inferred] OpenAgents should express resource effects more richly than a
Boolean parallel flag, but Codex demonstrates the minimum safe requirement:
parallelism is handler-declared and namespace-sensitive.

### 10.5 Tool search and context pressure

[source] Large tool catalogs can be deferred and searched rather than all
injected into the prompt. Apps and MCP tools carry provenance and selection
metadata. Tool-search behavior has evolved enough that old feature flags remain
as removed compatibility no-ops.

[inferred] Semantic tool retrieval is part of context management. OpenAgents
should use its central typed semantic selector and preserve the exact selected
tool-set hash in the run receipt.

## 11. Permissions and approvals

### 11.1 Named permission profiles

[schema] Codex supports built-in read-only, workspace, and full-access profiles
plus named custom profiles. Profiles describe filesystem access, nested
read-only/denied carveouts, special workspace roots, network behavior, and
inheritance. Active profile identity and provenance can be projected through
app-server.

[test] Configuration tests cover profile inheritance, cycles, unknown parents,
reserved names, workspace-root resolution, nested policies, managed allowlists,
and legacy translation.

### 11.2 Approval policy is separate

[schema] Approval policy controls when a request pauses for review. It includes
interactive policies, never-prompt automation, and granular prompt categories.
An automatic guardian/reviewer can evaluate eligible prompts without changing
the sandbox itself.

[source] Permission profiles and approval policy are resolved together for a
turn but remain distinct types. A dangerous bypass flag explicitly disables
both approval and sandboxing.

### 11.3 Managed requirements

[source] `requirements.toml`, MDM, and cloud-managed requirements can constrain
allowed approval policies, sandbox modes, web-search modes, permission
profiles, models, hooks, remote control, computer use, features, and network
policy. Requirements are non-bypassable constraints. Managed defaults remain
user-changeable startup values. [public]

[inferred] This is a strong model for OpenAgents organizations: preference and
requirement must be separate schemas with different precedence and UI.

### 11.4 Execpolicy

[source] The execpolicy engine parses Starlark-like prefix rules with
allow/prompt/forbidden decisions, optional justifications, executable-path
metadata, and embedded positive/negative examples validated at load. The
strictest matching rule wins.

[test] Parser and evaluation tests cover token matching, executable resolution,
policy merging, examples, and decision output.

[inferred] Policy-as-code with self-tests is a high-value idea. OpenAgents can
adapt it into Effect Schema policy records and model-check bounded authority
transitions rather than adopt Starlark itself.

### 11.5 Hooks

[schema] Codex implements Claude-style lifecycle hooks for pre/post tool use,
permission request, compaction, session start, subagent start/stop, user prompt,
and stop events. Hook schemas are generated. Command handlers are the current
executed type. Parsed prompt/agent handlers are not all active. Managed policy
can allow only managed hooks.

[source] Hook trust, source, scope, execution mode, run status, output entries,
and summaries are explicit protocol concepts. The CLI exposes a deliberate
hook-trust bypass rather than silently executing untrusted project hooks.

[inferred] Codex improves on “just run a shell callback” by surfacing trust and
results, but internal OpenAgents invariants should still live in typed services,
not external hooks.

## 12. Sandboxing and network enforcement

### 12.1 macOS Seatbelt

[source] macOS command construction emits Seatbelt policy from the effective
permission profile. It handles read/write roots, protected subpaths, network
policy, proxy endpoints, Unix sockets, and managed-network constraints.

[test] Tests assert fail-closed DNS/network behavior, writable carveouts,
worktree `.git` indirection, proxy ports, and policy rendering.

### 12.2 Linux bubblewrap

[source] Linux now defaults to bubblewrap. It uses a read-only root, binds
writable roots, reapplies `.git`/`.codex` and other protected paths as read-only,
orders overlapping policy by path specificity, masks denied glob matches,
rejects unsafe symlink/missing protected paths, unshares user and PID
namespaces, optionally unshares network, applies `no_new_privs`, and adds
seccomp restrictions.

[source] System bubblewrap is preferred when safe and sufficiently capable. A
bundled helper is the fallback. WSL2 follows the Linux path, while WSL1 is
rejected for sandboxed commands. Legacy Landlock remains an explicit fallback
only for policies it can represent without losing semantics.

### 12.3 Windows sandbox

[source] Windows has restricted-token and elevated sandbox implementations,
filesystem policy translation, proxy handling, setup/runner flows, and explicit
refusal when a policy cannot be represented without weakening it.

[inferred] Refusing to run unsandboxed when a nested policy cannot be expressed
is exactly the fail-closed behavior OpenAgents needs.

### 12.4 Managed network proxy

[source] The embedded proxy supports HTTP and SOCKS5, allow/deny domain policy,
local/private-address protection, limited read-only HTTP methods, optional TLS
interception for method policy and hooks, Unix-socket policy on macOS, upstream
proxy chaining, and loopback-only listener defaults.

[source] Explicit deny always wins. With no allow entries, access is blocked.
Limited mode blocks encrypted tunnels it cannot inspect. MITM private key
material remains in memory while public CA files are exposed to sandboxed
clients.

[schema] Structured audit events record scope, decision, source, reason,
protocol, endpoint, method, and whether a decider overrode an allowlist miss,
while intentionally omitting full URL/query data.

### 12.5 Approval is not containment

[source] Codex makes this distinction structurally: execpolicy/approval decides
whether a request proceeds. The permission profile and OS backend decide what
the process can reach. App-server exposes both approval and effective sandbox
settings.

[inferred] OpenAgents should copy this separation and add one signed authority
manifest plus one effective-containment receipt for every high-authority run.

## 13. Context, instructions, memory, and compaction

### 13.1 Context fragments are typed and bounded

[source] The repository contract states strong model-context invariants:

- build history incrementally rather than rewrite it.
- avoid changes that destroy prompt-cache prefixes.
- hard-cap every injected item.
- reject individual items beyond a large fixed token ceiling. And
- represent injected fragments as typed context structures.

[source] Context fragment types cover user/project instructions, permission
state, workspace/environment state, apps, plugins, skills, agents, model
switches, time, token/rollout budgets, hook context, subagent notifications,
network rules, and interruption state.

[inferred] This is one of Codex's best internal contracts. OpenAgents should
make the same rule enforceable through Effect Schema constructors and property
tests.

### 13.2 AGENTS.md hierarchy

[source] Codex discovers durable repository guidance through `AGENTS.md`, with
closer files taking precedence for their subtree. An agent-guidance manager
tracks relevant files and projects them into model context. [public]

[test] Core and exec integration tests cover guidance discovery and context
rendering.

### 13.3 World-state projection

[source] The context layer increasingly renders mutable environment state—such
as filesystem/network permissions, active agents, apps, plugins, and
instructions—through a typed world-state abstraction rather than repeatedly
injecting ad hoc prose.

[inferred] This aligns closely with OpenAgents' public/private projection
model. The difference is that OpenAgents should persist the provenance and
version of every projection used for a turn.

### 13.4 Compaction

[source] Local and remote compaction paths preserve a compact boundary and
reconstruct the model-visible context without rewriting the durable rollout.
Manual compaction is a distinct turn operation. Hooks can run before and after.
clients receive a compaction item/notification.

[test] Core tests exercise compaction, resume, and history behavior.

### 13.5 Memory pipeline

[source] Memory is feature-gated and implemented as extraction and
consolidation jobs with leases, ownership tokens, watermarks, bounded
concurrency, retry budgets, backoff, cooldown, and SQLite state. Per-thread
memory eligibility is persisted and reset is an explicit API operation.

[inferred] This is much more operationally rigorous than a hidden Markdown
append. The remaining product requirement is explainability: every memory
injection should link to source events and a user-visible retention decision.

## 14. Persistence and recovery

### 14.1 JSONL rollout as durable event record

[source] Threads persist append-only rollout JSONL under date-partitioned
`~/.codex/sessions` paths. Records include session metadata, response items,
turn context, events, compaction, world state, and inter-agent communication.
Archive moves rollouts into a separate collection. Delete removes them.

[source] Ephemeral threads intentionally have no rollout path. Fork copies
history into a new thread and records the source. Resume rehydrates the model
history and settings from the rollout.

### 14.2 SQLite as index and operational state

[source] The SQLite state database has evolved through dozens of migrations.
It indexes thread metadata, preview, source, model/effort, dynamic tools,
recency, history mode, agent nickname/path, spawn edges, remote-control
enrollment, external migrations, jobs, and other operational state.

[source] State recovery can scan rollouts and repair missing or stale index
rows. Thread listing can use state-only fast paths or filesystem scans that
backfill the database.

[test] Migration and recovery tests cover newer migration versions, repair,
stale hits, archive/list behavior, and cross-version compatibility.

### 14.3 Storage-neutral thread interface

[source] A thread-store abstraction exposes listing, reading, pagination,
archive, delete, and relation filters without binding app-server consumers to
JSONL/SQLite internals. Local storage is one implementation.

[inferred] This is a better migration path than letting every client parse
rollouts. OpenAgents should similarly keep its export log separate from the
queryable authority interface.

### 14.4 Thread history modes

[source] Legacy and paginated history modes coexist. The newer mode persists
typed item completion appropriate for paginated app-server reads. Metrics
measure pre/post-filter item and byte counts and validate turn boundaries.

[inferred] This is evidence of storage evolution under a live protocol. The
compatibility cost is real. OpenAgents should choose pagination and item IDs at
the first durable release.

### 14.5 No file checkpoint system equivalent to Claude's was found

[limitation] Codex persists file-change items and Git context, and its patch
tool can report edits, but this audit did not find a general per-turn file
checkpoint/rewind store comparable to Claude Code's file-history snapshots.
Rollback applies to conversation history, not a guaranteed restoration of all
workspace files.

[inferred] OpenAgents should combine Codex's indexed event graph with Claude's
explicit file checkpoints.

## 15. Extensions and interoperability

### 15.1 MCP client and server

[source] Codex can consume stdio and Streamable HTTP MCP servers, perform OAuth
discovery/login, aggregate tools/resources/templates, reconnect failed
servers, cache selected tool catalogs, route elicitations, normalize tool
names, and preserve plugin provenance. It can also expose Codex itself as an
MCP server.

[schema] MCP status, auth, tools, resources, prompts, and elicitations join the
app-server protocol. Approval policy applies to MCP tool calls and elicitations.

### 15.2 Skills

[source] Skills are filesystem packages discovered from multiple roots. The
runtime tracks skill metadata, interface, dependencies, tool/MCP requirements,
product restrictions, implicit-invocation policy, enabled state, owning
filesystem/environment, and exact path identity.

[test] Selection tests cover exact mentions, ambiguous names, structured
mentions, disabled paths, connector conflicts, deduplication, and environment
ownership.

### 15.3 Plugins and marketplaces

[source] A plugin manifest can bind skills, MCP servers, apps, hooks, and other
resources. Plugin IDs include marketplace identity. Marketplaces can be
installed from Git or local sources, upgraded, removed, and governed by
configuration/requirements. Path resolution prevents manifest resources from
escaping the plugin root.

[source] The CLI and TUI can browse/install plugins. Current official docs
describe plugins as bundles of skills, apps/MCP, browser capabilities, hooks,
and scheduled templates. [public]

[limitation] The audit found integrity, source, marketplace, and path controls,
but did not establish a universal publisher-signature/transparency chain for
every plugin artifact.

### 15.4 Apps/connectors

[source] Hosted apps/connectors are represented as MCP-backed tools with
per-app and per-tool enablement, destructive/open-world hints, approval modes,
auth elicitations, provenance, and tool-search metadata.

[inferred] OpenAgents can preserve connector interoperability while keeping the
authority envelope local and public-safe.

### 15.5 Code mode

[source] Experimental code mode exposes a small `exec`/`wait` interface to the
model. Model-generated JavaScript runs in V8 and can orchestrate nested typed
tools, parallel promises, output, images, and delayed work. A separate stdio
host process has version/capability negotiation and bounded frames. A missing
host currently falls back to in-process V8.

[test] Tests cover cancellation, pending tool calls, delayed work, host
handshake, frame bounds, panics, session limits, and process reuse.

[inferred] Code mode can reduce model round trips and express dataflow, but the
fallback is a boundary warning: if process isolation is part of the promise,
missing isolation must fail closed. OpenAgents should initially keep orchestration
in typed Effect workflows, not execute arbitrary model-generated JavaScript in
the authority process.

## 16. Multi-agent architecture

### 16.1 Agent threads are ordinary engine threads with explicit lineage

[source] A subagent gets a distinct thread ID, identity/path, model/effort,
role, cancellation scope, status, and parent relationship. Agent execution
reuses the same core thread machinery rather than introducing an unrelated
conversation implementation.

[schema] Turn items distinguish collaboration tool calls and subagent activity.
Events cover spawn, interaction, wait, close, resume, and activity lifecycle.

### 16.2 Two collaboration tool generations

[source] The repository contains legacy collaboration tools and a V2 set with
task-path-oriented operations such as spawn, follow-up, message, interrupt,
list, and wait. Compatibility and staged rollout remain visible.

[inferred] The V2 direction is product-significant: canonical task paths and
addressable agents are easier to render and synchronize than opaque tool-call
results.

### 16.3 Persisted graph store

[source] A dedicated storage-neutral graph interface persists one directional
parent edge per child, tracks open/closed lifecycle, lists direct children, and
walks descendants in stable breadth-first order. SQLite stores the edges and
indexes parent/status.

[test] Graph tests cover idempotent upsert, status transition, stable ordering,
direct/descendant traversal, and filtering.

This directly fixes the weakness identified in Claude Code histories: child
topology is no longer recoverable only by scanning tool results and sidecars.

### 16.4 Agent jobs

[source] CSV-backed agent jobs persist a job plus row-level items, assignment
thread, attempt count, result, errors, timestamps, and optional maximum runtime.
Tools spawn workers and report results into this durable job state.

[test] Integration and handler tests cover job creation, fanout, reporting,
timeouts, and schemas.

[inferred] This is a practical bridge from conversational delegation to batch
work. OpenAgents should adapt the typed job/item ledger but avoid CSV as the
canonical internal format.

### 16.5 Goals and automatic continuation

[source] A single persisted goal can be active for a materialized thread. Goal
state includes budget/accounting and stopped outcomes. Automatic continuation
uses dedicated prompts and can be steered, paused, blocked, completed, or
budget-limited.

[inferred] This is closer to a durable workflow than a chat loop. OpenAgents
should integrate equivalent goal state with its issue/task/receipt graph rather
than maintain it only inside an agent transcript.

### 16.6 Rendering is richer than Claude's ordinary parent row

[source] The TUI tracks agent metadata, navigation, activity feeds, child
history, streamed agent lists, wait states, active-agent labels, and explicit
spawn completion with model/effort. App-server can list direct children or all
descendants and reports `parentThreadId`.

[test] Snapshot tests encode many multi-agent render states.

[inferred] Codex has moved child topology into product state, though execution
completion still does not prove Git integration or delivery.

## 17. Git, patches, and workspace lifecycle

### 17.1 Git context is metadata, not the thread database

[source] Threads record Git SHA, branch, and origin metadata. Resume pickers
can search/display branch and path. Review targets include uncommitted changes,
base branch, commit, and custom instructions.

### 17.2 Apply-patch is a first-class tool

[source] Patch application has a dedicated grammar, parser, verifier, runtime,
approval conversion, streamed input progress, file-change items, and tests.
The engine does not rely on shelling out to an opaque patch command for every
edit.

[inferred] Typed file changes are essential for review, receipts, and mobile
projection.

### 17.3 Worktrees are not a first-class lifecycle resource

[limitation] The sandbox understands Git worktree `.git` indirection and the
product can operate in any worktree, but this audit did not find Claude-style
worktree create/resume/retain/remove commands or a durable worktree ownership
state machine in Codex itself.

[inferred] OpenAgents should retain the worktree lifecycle recommendation from
Claude Code: creation, base, owner, dirty state, commits, push/merge state,
retention, and cleanup need explicit records beyond generic shell access.

### 17.4 Cloud changes remain a separate delivery step

[source] Cloud tasks run against a selected Git ref and `codex apply` brings a
cloud-generated diff into the local tree. This keeps hosted execution and local
application distinct.

[inferred] OpenAgents should use an even more explicit delivery state machine:
changes produced, reviewed, committed, pushed, PR opened, merged, and accepted
are different facts.

## 18. Remote control and daemon lifecycle

### 18.1 App-server daemon

[source] The experimental Unix-only daemon manages a standalone-installed app
server through pidfiles, a private Unix socket, settings, a lifecycle lock, and
graceful-then-forced termination. Start is idempotent. Mutations serialize per
`CODEX_HOME`. Version probes verify readiness.

[source] Bootstrap can install a detached updater loop. It runs the standalone
installer on an hourly cadence, restarts app-server on a changed binary, then
replaces its own process image. It is not reboot-persistent.

[inferred] This is an honest lifecycle contract with known limitations. Khala
Pylon should likewise expose installation identity, binary version, process
generation, socket, readiness, updater state, and last transition rather than
hide them behind “online.”

### 18.2 Enrollment and pairing

[source] Remote control requires ChatGPT auth. Enrollment registers machine
name, OS, architecture, app-server version, and installation ID, then receives
server/environment identity and an expiring token. Pairing produces short-lived
artifacts. Controller grants can be listed and revoked.

[schema] App-server exposes enable, disable, status, pairing start/status,
client list/revoke, and status-change notification. Managed requirements can
force-disable remote control.

### 18.3 Reliable stream envelope

[schema] Remote-control envelopes carry:

- controller client ID.
- per-connection stream ID.
- monotonically addressed sequence ID.
- cursor.
- JSON-RPC message or bounded chunk.
- acknowledgement. And
- ping/pong and close state.

[source] Acknowledgements are cumulative per stream. Chunk acknowledgements can
retain only unacknowledged segments across reconnect. Stream changes reset
partial assembly.

### 18.4 Bounded segmentation

[source] Large messages are segmented with target/max frame sizes, total
reassembly caps, segment-count caps, bounded concurrent assemblies, strict
ordering, duplicate suppression, metadata consistency, and least-recently-used
assembly eviction. Invalid or oversized data is dropped rather than allocated
without bound.

[test] Pairing, client tracking, segmentation, replay, reconnect, auth refresh,
and server API paths have dedicated tests.

### 18.5 OpenAgents consequence

[inferred] This protocol supplies concrete patterns for Khala Sync:

- separate device/controller identity from stream incarnation.
- sequence and acknowledge every authority-bearing message.
- resume from a durable cursor.
- cap frames, total messages, and partial assemblies.
- make pairing short-lived and grants revocable. And
- keep the local app-server/Pylon as execution authority.

## 19. Configuration and enterprise policy

### 19.1 Layer stack

[source] Codex loads user config, trusted project config, command-line
overrides, profiles, managed config, cloud-delivered fragments, MDM, and
requirements through an explicit layer stack. Project config cannot override
machine-owned provider, authentication, telemetry, or host metadata fields.
[public]

[source] Config has a generated JSON Schema. Strict mode rejects unknown fields.
normal mode can preserve compatibility. Edits use structured TOML mutation to
avoid destroying unrelated user formatting and sections.

### 19.2 Requirements versus defaults

[source] Requirements constrain permissible values. Managed config supplies
defaults. The precedence and source are carried in constrained values rather
than flattened into untraceable configuration.

[inferred] OpenAgents should expose a per-field effective-value explanation:
selected value, source, constraint, allowed alternatives, and reload status.

### 19.3 Project trust

[source] Project-scoped config loads only for trusted projects. App-server can
mark a project trusted when a client explicitly starts a writable thread in
that directory. Hook trust and project trust are related but distinct.

[inferred] A writable thread request should not silently broaden all future
project authority. OpenAgents should keep trust grants scoped, expiring, and
visible.

### 19.4 Feature registry

[source] The central feature registry contains 92 entries across stable,
experimental, under-development, deprecated, and removed stages. Removed flags
continue to parse old configs. Tests ensure default-enabled features are stable
or removed and dependency relationships remain consistent.

[inferred] This is more disciplined than scattered Boolean gates, but 92 flags
still represent substantial semantic surface. OpenAgents should copy the
maturity metadata and compatibility parsing, not the count.

## 20. Installation, updates, and supply chain

### 20.1 Release artifacts

[source] Releases produce native archives for multiple macOS, Linux, and
Windows targets. Npm wrappers and platform packages. Shell and PowerShell
installers. Python packages. A DotSlash launcher. And auxiliary binaries such
as sandbox, code-mode, response-proxy, and shell helpers.

[source] Build scripts verify expected layout and required components before
packaging.

### 20.2 Standalone installer

[source] Installers resolve a release, obtain GitHub asset metadata, verify
SHA-256 digests, stage under versioned release directories, atomically retarget
the current link/junction, preserve rollback-capable prior versions, serialize
through an install lock, detect conflicting package-manager installs, and run a
post-install version check.

### 20.3 Code signing

[source] Release automation includes macOS signing through Azure Key Vault
PKCS#11 tooling, pinned hashes for signing dependencies, certificate checks,
and Windows release lanes. GitHub Actions orchestrate the upstream project's
CI and release process.

[limitation] This audit did not reconstruct every release attestation or verify
published artifacts independently. Checksums retrieved from release metadata
still depend on the authority and integrity of that release channel.

### 20.4 Component compatibility

[source] App-server schema is generated per binary version. Daemon outputs both
CLI and running app-server versions. Remote clients can detect unsupported
methods. Code-mode host negotiates protocol versions/capabilities. Installers
bundle known component layouts.

[inferred] Codex has compatibility mechanisms, but not one obvious signed
component ledger spanning every helper, SDK, plugin, and protocol. OpenAgents
should preserve its planned component manifest.

## 21. Telemetry, diagnostics, and privacy

### 21.1 Structured telemetry

[source] The runtime supports tracing, metrics, OpenTelemetry export, analytics,
performance spans, rollout-size metrics, network policy audit, tool dispatch
traces, and client identity on app-server requests. OTel export is off by
default in current public guidance. Prompts are redacted by default when
enabled. [public]

### 21.2 Bounded local diagnostics

[source] TUI diagnostics use bounded local stores by default. A plaintext TUI
log requires explicit `log_dir` configuration. Login has a focused durable log
with secret-redaction tests. `codex doctor` produces human or redacted JSON
diagnostics without dumping credentials.

### 21.3 Compliance identity

[source] App-server initialization requires clients to identify themselves.
the client name can feed enterprise compliance logging. Unknown third-party
clients may need registration for supported enterprise use.

[inferred] Client provenance is a real policy input. OpenAgents should sign
first-party client identities and distinguish them from arbitrary protocol
consumers without making the open protocol unusable.

### 21.4 Local corpus sensitivity

[source] Rollouts, prompt history, memory, logs, plugin state, agent-job CSV,
and SQLite metadata can contain private prompts, code, paths, command output,
and operational relationships.

[inferred] Open source does not reduce local data sensitivity. Khala Sync must
project public-safe typed state rather than replicate entire rollouts by
default.

## 22. Performance and reliability engineering

| Technique | Evidence | Architectural purpose |
| --- | --- | --- |
| in-process typed app server | [source] | preserve one protocol without JSON serialization overhead locally |
| bounded protocol queues | [source] | reject overload instead of unbounded memory growth |
| notification opt-out | [schema] | let clients suppress high-volume deltas they do not render |
| prompt-context invariants | [source] | preserve cache prefixes and cap injected content |
| deferred tool search | [source] | avoid placing large catalogs in every prompt |
| parallel handler declaration | [source] | reduce turn latency without racing arbitrary tools |
| unified PTY execution | [source] | maintain long-lived commands and bounded polling |
| SQLite thread index | [source] | avoid scanning all JSONL for common queries |
| repairable rollout/index split | [test] | retain recovery when the index is stale or missing |
| paginated turns/items | [schema] | bound client load for long histories |
| bounded remote segmentation | [source] | carry large events without unbounded frames/reassembly |
| lazy code-mode host | [source] | avoid V8 process cost until used |
| memory leases/backoff | [source] | coordinate background extraction without duplicate storms |
| snapshot-tested TUI | [test] | catch rendering regressions across dense states |

[inferred] Codex is designed for long-lived sessions and many clients, not only
one-shot CLI prompts. Reliability concerns are visible in public types rather
than hidden solely in operations code.

## 23. Test and verification architecture

### 23.1 Protocol-first integration tests

[test] Agent behavior changes are expected to use mocked Responses API streams
and drive the public core/app-server APIs. Helpers capture outbound requests so
tests assert exact structured input and tool outputs rather than inspect private
implementation state.

### 23.2 Schema drift tests

[test] Generated TypeScript and JSON schema fixtures are checked against Rust
types. Experimental and stable bundles are separate. V2 naming and optionality
rules are enforced by repository convention and generation tests.

### 23.3 UI snapshots

[test] Ratatui rendering is snapshot-tested for messages, diffs, approvals,
agents, goals, status lines, errors, hooks, MCP, and platform variants. Changes
must review and accept the rendered output artifact.

### 23.4 Cross-platform sandbox tests

[test] Sandboxing has policy-generation tests, integration helpers, Windows
and Wine lanes, bubblewrap compatibility paths, and network-proxy tests. Some
tests self-skip when the enclosing environment cannot nest the required
sandbox.

### 23.5 Formal boundary opportunity

[inferred] The typed Thread/Turn/Item state, permission profiles, agent graph,
goal states, and remote sequence/ACK rules are good candidates for bounded
formal models. The repository contains extensive tests but this audit did not
find a single formal state-machine model covering those cross-component
invariants.

OpenAgents should use its invariant discipline to model task terminality,
approval/containment composition, replay, and worktree delivery alongside
regression tests.

## 24. Architectural assessment

### 24.1 What Codex gets exceptionally right

1. **One engine seam for rich clients.** TUI, embedded, remote, IDE, desktop,
   and SDK paths converge on app-server. [source]
2. **A semantic protocol.** Thread, Turn, and Item express product state, not
   terminal text or raw provider events. [schema]
3. **Generated contracts.** Rust types produce TypeScript and JSON schemas tied
   to the running version. [schema] [test]
4. **Local inspectability plus indexed state.** JSONL remains recoverable while
   SQLite supplies queries, migrations, and operational state. [source]
5. **Explicit agent topology.** Parent/child edges and lifecycle live in a
   dedicated graph store. [source] [test]
6. **Real containment.** Platform sandboxes and network policy compile typed
   profiles into OS enforcement. [source] [test]
7. **Approval remains distinct from sandbox.** Policy and containment can be
   explained separately. [schema]
8. **Remote control has distributed-systems semantics.** Client/stream/sequence,
   ACK, cursor, reconnect, chunk bounds, pairing, and revocation are explicit.
   [source] [test]
9. **Context injection has invariants.** Typed fragments and hard bounds are a
   repository-level review requirement. [source]
10. **Release and test systems are part of the product.** Schemas, snapshots,
    cross-platform packages, signing, and installer verification are visible.
    [source] [test]

### 24.2 Where Codex is carrying debt or risk

1. **Scale is high.** 125 crates and more than a million Rust lines demand
   strong boundary ownership. [source]
2. **Core remains bloated.** The repository explicitly warns contributors to
   resist adding to it. [source]
3. **TUI composition is still large.** Several central modules are called out
   as too large/high-touch despite the app-server migration. [source]
4. **Compatibility layers are numerous.** V1/V2 protocol, legacy/new
   permissions, history modes, removed flags, old/new collaboration tools, and
   client fallbacks all remain. [source]
5. **Feature surface is broad.** Ninety-two registered entries include many
   deprecated/removed aliases that must continue parsing. [source]
6. **Persistence is hybrid and distributed.** Rollout JSONL, SQLite, prompt
   history, memory files, secrets, plugin caches, daemon state, jobs, and logs
   require coordinated deletion and migration. [source]
7. **No general file rewind store was found.** Conversation rollback cannot be
   treated as workspace restoration. [limitation]
8. **No first-class worktree lifecycle was found.** Git isolation/integration
   can still be driven through shell rather than typed resource state.
   [limitation]
9. **Hermetic automation is partial.** Exec can be ephemeral and ignore user
   config/rules, but there is no single obvious mode that proves every ambient
   instruction, hook, skill, plugin, credential, and network source was
   excluded. [source] [inferred]
10. **Code-mode process fallback weakens a potential isolation promise.** A
    missing sidecar can fall back to in-process V8. [source]
11. **Remote WebSocket app-server remains experimental.** Rich remote clients
    must tolerate capability and version differences. [source]
12. **Full-host shell is reachable through explicit paths.** Bang-shell and the
    combined bypass mode require careful client capability gating. [source]
13. **Plugin publisher signatures were not comprehensively established.**
    Provenance and path controls are not the entire supply-chain story.
    [limitation]

### 24.3 Comparison with Claude Code

| Dimension | Codex | Claude Code snapshot | OpenAgents conclusion |
| --- | --- | --- | --- |
| Rich-client engine seam | TUI now uses app-server in-process/remote | interactive and SDK query ownership still split | one conversation owner from day one |
| Protocol model | Thread/Turn/Item JSON-RPC with generated schemas | bidirectional stream events/control schemas | preserve semantic typed protocol and generation |
| Persistence | JSONL rollout + SQLite index/state | append-only parent-linked JSONL + sidecars | append log plus canonical indexed graph |
| Agent topology | persisted graph store and parentThreadId API | separate sidechains, ordinary topology partly implicit | explicit spawn/lifecycle edges |
| File recovery | no general checkpoint store found | file-history checkpoints and rewind | add Claude-style checkpoints to Codex-style graph |
| Worktrees | sandbox-aware, not first-class lifecycle | explicit create/retain/remove lifecycle | make worktrees durable resources |
| Sandboxing | default platform-specific OS enforcement and profiles | capable but opt-in in audited snapshot | fail-closed named execution profiles |
| Network | managed proxy with audit and method/domain policy | sandbox network domains/sockets | typed egress policy and receipts |
| Remote control | app-server relay with stream/seq/ACK/chunks/pairing | mature compatibility transports and approvals | one transport-neutral event/control envelope |
| Extensions | MCP, skills, plugins, apps, hooks, code mode | MCP, skills, agents, plugins, hooks, IDE/LSP | open compatibility with signatures and authority |
| Hermetic mode | partial exec flags | explicit bare/minimal mode | ship a complete admitted-input manifest |
| Test evidence | extensive integration/schema/snapshot suite | absent from source extract | protocol fixtures and public-boundary tests required |

## 25. What OpenAgents should adapt

### 25.1 Make the Runtime Gateway the only conversation owner

Codex demonstrates the end state: an in-process client is a transport
optimization, not a private engine API. OpenAgents Desktop, terminal, mobile,
web, and external SDK should all submit the same Effect Schema commands and
consume the same events.

The engine must own thread, turn, item, tool, task, approval, compaction,
checkpoint, and terminal state. Renderers own only projections and local input
state.

### 25.2 Adopt Thread → Turn → Item, then extend it with Work Unit and Receipt

Codex's semantic hierarchy is the right base. OpenAgents should add:

- **Work Unit:** task/subagent/job identity with causal parent and delivery
  lifecycle.
- **Authority Manifest:** compiled policy, approvals, capabilities, credentials
  classes, and workspace identity.
- **Execution Receipt:** effective sandbox, mounts, egress, process/provider,
  usage, and terminal reason. And
- **Delivery Receipt:** changed artifacts, checkpoint/worktree, commit, push,
  PR, merge, acceptance, and public-safe evidence refs.

### 25.3 Pair an append log with an indexed authority

Keep a human-exportable append-only event log like Codex rollouts, but make the
queryable store canonical for topology, current state, constraints, and
projection. Recovery should rebuild indexes from the log and produce a repair
receipt.

Do not let clients parse raw log files to reconstruct product truth.

### 25.4 Persist the agent graph explicitly

Adopt Codex's storage-neutral parent/child interface and extend edge state
beyond open/closed:

```text
requested -> admitted -> starting -> running -> waiting
          -> completed | failed | cancelled | lost
          -> changes_produced -> reviewed -> integrated -> accepted
```

List direct children and descendants deterministically. Synchronize graph
events to mobile as public-safe projections.

### 25.5 Generate every client from one schema

Generate TypeScript, Rust, Swift/Kotlin, and JSON Schema from the Effect Schema
protocol. Split stable and experimental capability bundles. Version the
protocol independently of UI marketing versions and retain fixtures for every
supported compatibility window.

### 25.6 Compile named permission profiles into enforcement

Use named read-only, workspace, networked-build, isolated-guest, and full-host
profiles. Profiles should compile to:

- filesystem read/write/deny maps with protected metadata carveouts.
- network domain/method/socket policy.
- process and executable policy.
- secret/provider capability refs.
- browser/computer-use scope. And
- an OS-specific enforcement plan.

If the OS cannot represent the requested policy, fail closed.

### 25.7 Keep approval, policy, and containment separate

Borrow Codex's structural separation and make it legible in UI:

- organization requirements constrain choices.
- user/session policy determines ask/allow/deny.
- a reviewer decides eligible prompts.
- the authority manifest records admission. And
- the sandbox receipt records what was actually enforced.

### 25.8 Build a managed egress service

Codex's network proxy is a strong reference for domain deny precedence,
local-network protection, method-limited modes, proxy-only sandboxing, Unix
socket restrictions, and redacted decision audit. OpenAgents should implement
the equivalent as an Effect service and isolate TLS-interception capability
behind explicit enterprise policy.

### 25.9 Combine Codex state with Claude recovery

The ideal OpenAgents local runtime combines:

- Codex's semantic protocol, SQLite index, graph store, bounded queues, and
  remote envelope.
- Claude Code's per-tool file checkpoints, explicit rewind, and outcome-aware
  worktree cleanup. And
- OpenAgents' typed Blueprint governance, public-safe projections, and formal
  invariants.

### 25.10 Make remote control transport-neutral and replay-safe

Use controller ID, device/executor ID, stream epoch, sequence, ACK, replay
cursor, idempotency key, expiry, and bounded segmentation. A local Pylon or
desktop engine remains execution authority. Khala Sync carries state/control.
it never turns the phone into a holder of raw local credentials or paths.

### 25.11 Treat daemon lifecycle as product state

Expose install/version/process/socket/readiness/updater identity and lifecycle
transitions. Serialize mutations. Make start idempotent, stop bounded, update
atomic, rollback explicit, and reboot persistence intentional.

### 25.12 Enforce context-fragment budgets mechanically

Every context source must implement a typed fragment interface with source,
scope, content hash, token/byte cap, public/private class, and rendering
version. Prompt assembly should reject unbounded fragments before model
submission.

### 25.13 Keep a truly hermetic mode

Codex provides useful partial controls. Claude Code provides the clearer
product concept. OpenAgents should offer a single profile that excludes all
ambient config, instructions, memory, hooks, plugins, skills, MCP, keychain,
network, and experiments unless explicitly admitted, and emits the manifest.

### 25.14 Test through the public protocol

Follow Codex's testing discipline:

- agent logic through streamed provider mocks and public engine commands.
- app-server/Runtime Gateway through generated client fixtures.
- every schema change through drift generation.
- every user-visible state through snapshots.
- every sandbox profile through OS-policy tests. And
- every replay/agent/goal transition through model-based invariant tests.

### 25.15 Keep worktrees and delivery first-class

Codex's Git metadata and patch protocol are useful, but OpenAgents should not
leave isolation and integration to generic shell commands. Worktrees need typed
create/resume/retain/remove operations and delivery must distinguish produced,
committed, pushed, reviewed, merged, and accepted.

### 25.16 Do not copy the compatibility burden

Avoid parallel V1/V2 semantics, client-private query paths, ambiguous legacy
permission shorthands, multiple multi-agent APIs, removed flags that live
forever, and overlapping stores without a deletion transaction. Every
temporary compatibility layer should have an owner, telemetry, expiry gate,
and removal issue.

## 26. Recommended OpenAgents implementation sequence

1. **Protocol kernel:** Effect Schema Thread/Turn/Item/WorkUnit/Receipt plus
   stable/experimental capability negotiation.
2. **Single engine host:** in-process and socket transports call the same
   Runtime Gateway request processor.
3. **Event authority:** append log, indexed SQLite state, deterministic replay,
   graph edges, repair, archive, delete, and public-safe projection.
4. **Generated clients:** TypeScript and mobile bindings plus compatibility
   fixtures and overload/retry semantics.
5. **Authority compiler:** requirements, defaults, approval, named profiles,
   filesystem, network, secrets, and provider identity.
6. **Execution enforcement:** macOS/Linux/Windows providers plus managed egress
   and fail-closed capability negotiation.
7. **Recovery:** file checkpoints, fork/rewind, durable output, worktrees, and
   delivery receipts.
8. **Agent graph and goals:** explicit lineage, jobs, standing continuation,
   budgets, escalation, and terminal/integration states.
9. **Khala remote control:** pairing, revocation, stream epoch, sequence/ACK,
   cursor replay, bounded segmentation, and device-safe projection.
10. **Extensions:** MCP, skills, plugins, apps, and hooks through signed
    provenance and the same authority envelope.
11. **Distribution:** signed component graph, daemon lifecycle, atomic update,
    rollback, and clean-install receipts.
12. **Formal and fault proof:** model task/replay/authority invariants and burn
    them under disconnect, duplicate delivery, stale worker, sandbox failure,
    and partial integration.

## Final assessment

Codex is the strongest direct architectural reference in this teardown set for
OpenAgents' engine layer. OpenCode shows a server-first desktop topology.
Claude Code shows deep local recovery and worktree/task ergonomics. Codex shows
how an open Rust agent engine can unify its own TUI and external rich clients
behind a typed protocol while preserving real local sandboxing and durable
state.

The pivotal finding is not that Codex has many tools. It is that the terminal
client no longer needs a different conversation owner. An in-process app
server and a remote app server implement the same product contract. That is
the architecture OpenAgents should lock before widening Desktop or mobile.

Codex also demonstrates the value of pairing append-only local evidence with a
queryable index and explicit agent topology. OpenAgents should adopt that
combination, then add the pieces Codex does not make first-class: file
checkpoint/rewind, worktree ownership and cleanup, delivery/acceptance state,
one complete hermetic mode, signed extension/component provenance, and typed
public/private Sync projections.

The warning is accretion. A million-line, 125-crate runtime with 92 feature
entries, legacy protocol surfaces, and several state generations is costly to
change even when its seams are good. OpenAgents should copy the semantic
contracts and verification habits while keeping its Effect service graph
smaller, its invariants independent of flags, and every compatibility bridge
temporary.

## Source basis

- [OpenAI Codex repository at the audited commit](https://github.com/openai/codex/tree/08ba14b03d0b3ce3cfdf8c88c0469b9b1924953d)
  [source]
- [Codex app-server README at the audited commit](https://github.com/openai/codex/blob/08ba14b03d0b3ce3cfdf8c88c0469b9b1924953d/codex-rs/app-server/README.md)
  [source] [schema]
- [Current official Codex documentation](https://developers.openai.com/codex)
  [public]
- [Current official Codex command documentation](https://learn.chatgpt.com/docs/developer-commands)
  [public]
- [Current official Codex configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference)
  [public]
- [Claude Code architecture teardown](./2026-07-10-claude-code-teardown.md)
- [ChatGPT desktop app teardown](./2026-07-10-chatgpt-desktop-app-teardown.md)
- [OpenCode desktop app teardown](./2026-07-10-opencode-desktop-app-teardown.md)

No credentials, private conversation content, local Codex histories, or live
application state were read for this audit. The source repository was treated
as a read-only external reference.
