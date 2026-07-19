# Goose Teardown — 2026-07-17

Read-only architecture and product audit of the public `aaif-goose/goose`
source tree at an exact, freshly fast-forwarded commit. The audit did not build
Goose, install an extension, launch the CLI or desktop app, connect a provider,
read local Goose state, or execute an agent turn.

## TL.DR

Goose is a mature local agent platform whose defining architectural move is
**one Rust agent engine presented through several hosts**: a native CLI, an
Electron desktop app, ACP server transports, a terminal ACP client, SDKs, and
scheduled or delegated executions. The same engine owns the provider loop,
MCP extension lifecycle, permissions, hooks, recipes, subagents, context
compaction, SQLite session history, and telemetry. ACP also runs in the other
direction: Claude Code, Codex, Amp, Pi, and other ACP agents can become Goose
providers while Goose passes its MCP extensions through to them. [source]

```text
CLI / Electron / text TUI / editor / SDK
                    |
             ACP or in-process API
                    |
              Rust Goose engine
       +------------+-------------+
       |            |             |
   providers    MCP tools     durable state
  native/ACP    built-in +    SQLite sessions,
 local/remote   external      JSON schedules
       |            |
       +------ agent loop -----+
              permissions,
          hooks, compaction,
        recipes and subagents
```

The strongest reusable seams are:

- a single engine contract across multiple clients rather than a separate
  desktop agent implementation.
- ACP as both a host protocol and provider adapter, with MCP passed through as
  the shared capability language.
- explicit conversation audiences that survive ACP projection, persistence,
  export, search, orchestration, and compaction.
- durable searchable SQLite sessions with usage, lineage, archive, schedule,
  recipe, provider, model, and mode metadata.
- recipes, hooks, schedules, and subagents built into the engine instead of
  simulated entirely in prompts.
- declarative provider definitions beside compiled implementations.
- a loopback desktop sidecar protected by a generated secret and an Electron
  renderer with context isolation, Node integration disabled, and web security
  enabled. And
- a serious cross-platform release system for CLI, desktop, SDK, containers,
  package managers, signing, and updates. [source]

The source also exposes boundaries OpenAgents should not inherit:

- Goose documents **Completely Autonomous** as the default mode. Its built-in
  developer tools normally execute with the host user's authority. Container
  execution is explicit, not a universal safety boundary. [source]
- approvals, model-based read-only classification, adversary scanning, prompt
  injection detection, and command classification improve decisions but are
  not OS enforcement. Smart Approval had to be fixed at the audited tip so a
  classification is scoped to the exact request rather than reused too
  broadly. [history] [source]
- scheduled recipes recently gained bounded, regular-file validation and
  private copies, but coordination remains JSON plus in-process Tokio jobs
  rather than a restart-safe lease, fence, and admission ledger. [source]
- sessions are durable, but execution is not modeled as a receipted portable
  run with independently verifiable admission, authority, placement, side
  effects, and outcome. [inferred]

The central OpenAgents decision is: **adapt Goose's one-engine/many-client
shape, bidirectional ACP/MCP bridge, audience-safe projections, durable
session/search model, provider catalog, and workflow primitives. Reject
autonomous host execution as the default, model judgment as policy or
containment, timer-plus-JSON scheduling as recovery, and session persistence
as a substitute for portable receipted work.**

## 1. Snapshot, provenance, and limitations

### 1.1 Exact source identity

Before inspection, the local reference clone was clean and on `main`. It was
fast-forwarded from `e359b35ae` to the then-current `origin/main` tip below.

| Artifact          | Identity                                                                                      | What it establishes                              |
| ----------------- | --------------------------------------------------------------------------------------------- | ------------------------------------------------ |
| Public repository | `https://github.com/aaif-goose/goose`                                                         | Public source and history                        |
| Audited commit    | `f43870951f6b887c49dd196165f495d9dfcb5713` on `main`                                          | Exact snapshot used here                         |
| Commit time       | `2026-07-17T20:49:32Z`                                                                        | Freshness of the audited tip                     |
| Commit subject    | `fix(config): require absolute goose path roots (#10454)`                                     | Latest path-authority hardening                  |
| Product version   | `1.43.0`                                                                                      | Cargo workspace and desktop generation           |
| License           | Apache License 2.0                                                                            | Source reuse boundary                            |
| Source scale      | 2,409 tracked files. About 215,071 Rust and 104,079 TypeScript/TSX lines                      | Approximate implementation scale                 |
| Workspace         | 12 Rust crates, Electron desktop, text TUI, TypeScript and Python SDKs, docs, evals, services | Repository topology                              |
| Recent history    | 45 commits since `2026-07-15T00:00:00Z`                                                       | Material activity in the requested recent window |

The fast-forward included July 16–17 fixes for request-scoped Smart Approval,
bounded scheduled-recipe validation, protected OAuth token caches,
audience-safe ACP projections, secure large-response spill files, absolute
Goose path roots, local-inference teardown stability, and a consolidated
release workflow. [history]

### 1.2 Evidence labels

- **`[source]`** — tracked source, docs, manifests, or workflows at the commit.
- **`[schema]`** — a typed Rust, ACP, MCP, SDK, recipe, or storage contract.
- **`[test]`** — a tracked executable test or CI check.
- **`[history]`** — Git history at or before the audited commit.
- **`[inferred]`** — reasoned from several observations. And
- **`[limitation]`** — a source-only audit boundary.

There are intentionally no `[runtime]` observations in this document.

### 1.3 Limits

Source cannot prove provider behavior, model quality, extension safety,
production signing, updater success, platform compatibility, telemetry
delivery, package provenance, or crash recovery. The tests described below
were inspected but not run in the read-only upstream clone. [limitation]

## 2. One engine, several projections

The `goose` crate is the kernel. `goose-cli` links it directly. Electron spawns
an authenticated `goose serve` backend and speaks ACP over loopback. Editors
can start `goose acp` over stdio. The text UI is another ACP client. SDK crates
expose operations without making a UI authoritative. [source]

This is better than sharing only provider adapters while each client
reimplements sessions, permissions, tools, and streaming. A mode switch,
audience rule, extension behavior, or session migration belongs to the engine
and projects consistently. ACP is still emerging and some host-specific code
remains, but rich clients consume one agent rather than becoming competing
agents. [source] [inferred]

The agent loop crosses more state than `provider -> shell`:

1. a host submits a message and session configuration.
2. the engine projects visible history and tools to a provider.
3. typed reply parts stream back.
4. calls cross inspectors, hooks, confirmation, execution, response-size, and
   extension boundaries.
5. recoverable failures become tool results.
6. eligible history may be summarized or compacted. And
7. messages, usage, metadata, and state are persisted. [source]

| Crate                           | Primary role                                                                                  |
| ------------------------------- | --------------------------------------------------------------------------------------------- |
| `goose`                         | Agent engine, ACP, extensions, permissions, sessions, recipes, schedules, security, providers |
| `goose-cli`                     | CLI, configuration, ACP/serve entrypoints, import/export                                      |
| `goose-provider-types`          | Canonical messages, models, usage, errors, permissions, provider traits                       |
| `goose-providers`               | Native and declarative provider clients                                                       |
| `goose-local-inference`         | Local model runtime support                                                                   |
| `goose-mcp`                     | Built-in MCP servers and artifact tooling                                                     |
| `goose-sdk` / `goose-sdk-types` | SDK and custom request/notification contracts                                                 |
| `goose-acp-macros`              | ACP method/schema generation                                                                  |
| `goose-download-manager`        | Managed downloads                                                                             |

## 3. ACP in both directions, MCP underneath

### 3.1 Goose as an ACP agent

`goose acp` exposes Goose over stdio JSON-RPC. The server supports concurrent
isolated sessions, model and mode switching, client file and terminal
operations, Goose extensions, and client-supplied MCP servers. ACP sessions
also enter Goose history. [source]

`goose serve` exposes draft HTTP/WebSocket transports. It requires
`GOOSE_SERVER__SECRET_KEY` unless explicitly started with
`--dangerously-unauthenticated`. CORS and WebSocket Origin defaults are
loopback-oriented. Electron generates a secret, launches on `127.0.0.1`,
probes authenticated ACP, retains a backend lease per window, and redacts the
token in diagnostics. [source] [test]

OpenAgents should adapt the generated authenticated client contract but bind
it to engine generation, client identity, scope, and placement. A bearer
secret proves possession, not which renderer intent or owner action authorized
a side effect. [inferred]

### 3.2 ACP agents as providers

Goose can treat ACP agents as providers. Adapters cover Amp, Claude, Codex,
Copilot, Cursor, Gemini, Pi, and related systems. Goose maps its modes into
provider-native permission or sandbox settings and passes Goose extensions
through as MCP servers. [source]

```text
Goose host and sessions
       -> ACP provider adapter
            -> external coding agent
                 -> Goose MCP extensions
```

The docs acknowledge session identity and resume/fork limitations. OpenAgents
should retain a lossless provider-native plane plus a loss-accounted portable
projection rather than pretending different session and execution models are
identical. [source] [inferred]

### 3.3 MCP extensions and apps

Goose supports built-in, child-process, streamable HTTP, and client-supplied
MCP servers. OAuth, elicitation, sampling, MCP Apps, validation, and
container-routed stdio extensions are present. Built-ins include shell, files,
documents, PDFs, spreadsheets, memory, planning, and other capabilities.
[source]

That creates ecosystem breadth but also a supply chain. Instructions, schemas,
annotations, executable resolution, OAuth tokens, endpoints, and returned UI
resources need admission, identity, policy, pinning, and revocation. Successful
MCP handshake or tool metadata is not sufficient trust. [inferred]

## 4. Conversation truth and persistence

### 4.1 Audience-safe transforms

The July 17 ACP patch propagated audience visibility through provider
messages, ACP conversion, reply parts, compaction, chat recall, orchestration,
search, CLI output, and export. Tests cover preservation and rejection of
hidden nested content. [history] [test]

This is the right invariant: projection is not serialization. Content hidden
from an agent, user, provider, or exporter must remain hidden after compaction,
search, replay, protocol conversion, and summarization. OpenAgents should adapt
this and receipt every loss, redaction, and derived summary. [inferred]

### 4.2 SQLite sessions

`SessionManager` uses SQLite in WAL mode with pooled access, schema versioning,
transactional migrations, indexes, and legacy import. Metadata includes:

- user and generated names.
- user, subagent, schedule, hidden, terminal, and gateway types.
- working directory, extension data, recipe and supplied values.
- provider, model, and Goose mode.
- usage totals and accumulated cost.
- archive, project, parent-session, and schedule identifiers. And
- message visibility and tool metadata. [schema] [source]

It supports create, update, list, cursor paging, filtering, archive, delete,
copy, truncate, import/export, naming, usage recording, and history search. It
is a credible local durable thread store, not a renderer cache. [source]

It is not a portable execution authority. A session row does not prove exact
admission, capability set, policy, placement, workload identity, side-effect
digest, external receipt, or recovery decision. OpenAgents should borrow the
ergonomics without weakening Thread/Turn/Item and receipt authority. [inferred]

### 4.3 Compaction

Goose can compact near context limits and summarize older tool-call/result
pairs while protecting recent calls. Originals retain visibility metadata
while a derived summary becomes provider-visible context. Provider-managed
context can opt out. Tests exercise audience projection during compaction.
[source] [test]

Derived summaries still need lineage: source messages, model, prompt, version,
visibility projection, and failure mode. OpenAgents should keep that lineage
queryable rather than let summary text silently become truth.

## 5. Permissions and security

### 5.1 Four modes, risky default

Goose exposes Autonomous, Manual Approval, Smart Approval, and Chat Only, plus
per-tool Always Allow, Ask Before, and Never Allow. Client-native permission
requests can route through Goose. [source]

The critical choice is that Autonomous is documented as the default. The
developer extension can read, write, edit, and run shell commands under the
Goose process's host identity. Docker modes and provider sandboxes exist, but
the engine does not universally enforce containment around every tool.
[source]

OpenAgents should reject this default. Approval is owner interaction. Policy
is authorization. A sandbox is enforcement. A container or VM is workload
containment. Egress constrains disclosure. Receipts establish what happened.
None substitutes for another.

### 5.2 Smart Approval is classification

Smart Approval combines explicit rules, tool annotations, cached
classification, and an LLM read-only judge. The July 17 fix scoped judgment to
the matching request instead of broadly authorizing later calls with the same
tool. [source] [history]

The patch demonstrates that tool name is too coarse an authority key.
Arguments, resolved paths, working directory, environment, destination,
session, caller, and generation can change the effect. Semantic assistance may
explain a decision. Deterministic typed policy must authorize the exact
resolved invocation. [inferred]

### 5.3 Defense in depth

Goose contains adversary and egress inspectors, prompt-injection detection,
command-pattern scanning, overlapping-window command classification, extension
malware checks, private secret/OAuth files, private response spill files,
absolute configurable roots, bounded regular-file scheduled recipes, and
Electron protocol restrictions. [source]

| Recent change                     | Architectural lesson                                      |
| --------------------------------- | --------------------------------------------------------- |
| Request-scoped Smart Approval     | Authority binds the exact invocation                      |
| Private OAuth caches              | Secure creation and migration both matter                 |
| Private response spill files      | Overflow remains in the disclosure boundary               |
| Absolute Goose roots              | Roots resolve before authorization                        |
| Bounded regular scheduled recipes | Background inputs need type, size, and private-copy rules |
| Audience-safe ACP projection      | Visibility survives every transform                       |
| Trimmed macOS entitlements        | Release privilege should shrink continuously              |

These are worthwhile layers, not a proof that a hostile tool cannot escape
host authority. [inferred]

### 5.4 Electron

Main windows disable Node integration, enable context isolation and web
security, use a preload, constrain navigation and protocols, and authenticate
the backend. MCP Apps render in sandboxed iframes with a host bridge and CSP.
[source]

This is a sound renderer baseline. Main still exposes privileged IPC, starts a
shell-capable backend, and handles deep links and external URLs. Every IPC path
needs caller, origin, payload, and authority validation. Renderer hardening
does not contain the agent backend. [inferred]

## 6. Recipes, hooks, subagents, and schedules

Recipes are YAML manifests with instructions or prompts, parameters,
extensions, settings, subrecipes, and scheduling support. They can be stored,
templated, deep-linked, validated, and launched from CLI or desktop. [schema]

This is more inspectable than burying repeatable work in chat. OpenAgents
should compile recipes into admitted typed work packets with exact inputs,
capabilities, policy, placement, and acceptance. YAML is source material, not
execution authority.

Hooks receive structured JSON for session, prompt, tool, shell, file, and stop
events. `PreToolUse` and `Stop` can block. Others are observational. Inputs and
working directory are available where applicable. [source]

The deny contract is useful, but arbitrary hook processes are another policy
plane. OpenAgents should admit and version hooks, define precedence and
timeouts, receipt decisions, and keep mandatory policy below optional
automation.

Subagent handlers and the Orchestrator can create sessions, list and inspect
them, summarize histories, and track parent-session relationships. [source]
OpenAgents should retain its fuller canonical graph: exact causal parent event,
mailbox, states, authority inheritance, and independent transcript.

The scheduler uses `tokio-cron-scheduler`, persists `schedule.json`, copies
recipes into private storage, launches ordinary agent sessions, and records
schedule IDs. It can list, pause, resume, update, remove, kill, and inspect
recent schedule sessions. [source]

The current tip limits recipe size, requires regular files, preserves a trusted
source directory for relative references, and writes mode `0600` copies on
Unix. Tests cover bounds, permissions, cleanup, missing recipes, pause,
removal, and execution. [test]

The gap is coordination truth. In-process cron plus JSON does not supply
durable admission, lease epochs, fencing, restart reconciliation, or receipts.
OpenAgents should borrow the UX and recipe linkage, not the recovery model.

## 7. Providers and local inference

Goose separates canonical provider types from implementations. Complex
providers use compiled Rust for authentication, streaming, tools, retries, and
model discovery. A declarative OpenAI-compatible layer contains 37 checked-in
JSON provider definitions, including the July 17 Sakana/Fugu addition.
[source]

That is the right split: ordinary compatible endpoints should not require an
engine fork, while distinct auth, event, tool, context, or model laws remain
typed code. OpenAgents should also require exact model identity, pricing,
capabilities, disclosure, and pre/post-spend truth.

The registry spans direct APIs, OAuth/subscription flows, compatible services,
local servers, cloud platforms, and ACP agents. Provider choice therefore
changes credential authority, transcript disclosure, tool ownership, session
semantics, sandboxing, accounting, and identity. Those differences belong in
the receipt, not behind one provider string. [source] [inferred]

Local inference is feature-gated Rust using Candle and optional CUDA, Vulkan,
and MLX, alongside local server and transcription paths. It is not a full
hardware control plane like Local Studio. OpenAgents should compose the two
concerns: provider compatibility is separate from accelerator, artifact,
process, capacity, health, and cleanup truth.

## 8. Testing and release

The snapshot has 454 Rust and 589 TypeScript/TSX files. A bounded inventory
found 255 Rust files containing tests and 68 TypeScript test/spec files. Tests
cover agent turns, ACP, permissions, compaction, session migration/search,
scheduled-recipe security, Electron backend authentication, URL normalization,
and UI state. [test]

CI checks Rust formatting, build/tests, TLS backends, Windows, minimum Rust
version, linting, generated ACP schemas/clients, and Electron lint/tests.
Separate workflows cover recipe security, docs, SDK wheels, containers,
releases, and platform bundles. [source]

The release system builds CLI and desktop artifacts across macOS, Windows, and
Linux, with Apple signing, update resources, package-manager publication, NPM
and Python SDKs, Docker multi-arch attestation, stable/canary flows, and smoke
steps. The July 17 consolidation centralized duplicated platform pipelines.
[source] [history]

OpenAgents should adapt the matrix beneath its stricter signed manifest,
component compatibility, immutable candidate, retained rollback, and receipt
invariants.

## 9. OpenAgents disposition

### Adapt

1. One durable engine projected into CLI, desktop, mobile, SDK, editor, and
   automation clients.
2. ACP for foreign agent hosts/providers and MCP for capability exchange.
3. Audience-preserving persistence, search, export, compaction, orchestration,
   and protocol adapters.
4. Cursor-paged searchable sessions with archive, lineage, usage, migrations,
   and import/export.
5. Declarative compatible providers beside typed protocol-distinct providers.
6. Inspectable recipes, hooks, schedules, and child work.
7. Generated authenticated local sidecars outside the renderer.
8. Cross-platform release staging and workflow consolidation.

### Adapt with stronger boundaries

1. Compile permission against the resolved invocation, caller, session,
   policy, placement, and generation.
2. Preserve a foreign ACP native plane plus loss-accounted portable projection.
3. Admit MCP servers, apps, recipes, hooks, and provider definitions as pinned
   supply-chain artifacts.
4. Promote schedules into durable admission, leases, fencing, recovery, and
   receipts.
5. Promote parent links into a complete canonical agent graph.
6. Make summaries derived artifacts with source and visibility lineage.
7. Separate provider compatibility from local hardware/process truth.

### Reject

1. Autonomous host-user execution as the default.
2. Model classification, prompt scanning, or annotations as authorization or
   containment.
3. Bearer-authenticated loopback as proof of owner intent.
4. Durable chat as proof of recoverable or portable execution.
5. In-process cron as exactly-once background work.
6. MCP compatibility as extension trust.
7. Provider normalization that hides identity, authority, disclosure, sandbox,
   and accounting differences.

## 10. Recommended sequence

1. Add Goose to Fast Follow with lessons for the one-engine fabric,
   bidirectional ACP/MCP, audience-safe projection, and workflow artifacts.
2. Add audience-preservation tests across Thread/Turn/Item, Sync, search,
   export, compaction, mobile, and foreign-provider adapters.
3. Compare Goose ACP mappings with Runtime Gateway and enumerate every loss,
   identity split, and permission translation.
4. Use recipes/hooks/schedules as UX input for admitted work packets and
   durable schedule leases, not as a direct runtime port.
5. Incorporate the provider catalog and local inference surface into current
   placement/model truth while retaining separate accelerator authority.
6. Add adversarial cases for repeated same-tool requests, hidden audiences,
   spill files, configurable roots, background recipes, token theft, and ACP
   permission downgrade.

## 11. Final assessment

Goose is one of the best public references for an interoperable local agent
engine because it refuses to make one UI the architecture. CLI, Electron,
editors, a TUI, SDKs, schedulers, and foreign ACP agents meet at a shared Rust
core. MCP supplies a broad tool plane. SQLite supplies credible local history.
workflows are tangible. And releases treat the product as real cross-platform
software.

Its weaknesses follow from the same local-first pragmatism. Broad host tools,
Autonomous as default, model-assisted permissions, optional containment, and
in-process scheduling can feel coherent on one machine while leaving
authorization, isolation, recovery, and portable evidence underspecified.

OpenAgents should use Goose to sharpen its client fabric and interoperability
layer beneath a stricter thesis: one durable engine, many projections, exact
audience and provider boundaries, admitted capabilities, contained placements,
restart-safe coordination, and receipts that survive movement across hosts.

## Primary source map

- `README.md`, `Cargo.toml`, `AGENTS.md`
- `crates/goose/src/agents/`
- `crates/goose/src/acp/`
- `crates/goose/src/context_mgmt/mod.rs`
- `crates/goose/src/permission/`, `crates/goose/src/security/`
- `crates/goose/src/session/session_manager.rs`
- `crates/goose/src/scheduler.rs`, `crates/goose/src/recipe/`
- `crates/goose/src/hooks/mod.rs`, `crates/goose/src/providers/`
- `crates/goose-providers/`, `crates/goose-local-inference/`
- `crates/goose-mcp/`, `crates/goose-cli/`
- `ui/desktop/src/main.ts`, `ui/desktop/src/preload.ts`
- `ui/desktop/src/gooseServe.ts`, `ui/desktop/src/acp/`
- `ui/text/`, `ui/sdk/`
- `documentation/docs/goose-architecture/`
- `documentation/docs/guides/acp-clients.md`
- `documentation/docs/guides/acp-providers.md`
- `documentation/docs/guides/managing-tools/`
- `documentation/docs/guides/security/`
- `documentation/docs/guides/context-engineering/`
- `documentation/docs/guides/recipes/`
- `.github/workflows/`
- Git history from `e359b35ae` through `f43870951`
