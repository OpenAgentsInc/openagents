# Claude Code Architecture Teardown — 2026-07-10

Read-only architecture audit of a historical Claude Code source snapshot,
commit `813c06acfa2d705076df6193b405c81eb11a18d1` (imported 2026-03-31),
used as a behavioral and structural reference for OpenAgents. This is a
separate analysis from the installed
[Claude desktop app teardown](./2026-07-10-claude-desktop-app-teardown.md):
the desktop document examines the Electron host, while this document examines
the coding-agent runtime that can operate in a terminal, through an SDK, or as
a remotely controlled local executor.

This is a clean-room product analysis. It records public product concepts,
observable contracts, and independently described architectural lessons. It
does not reproduce source code, internal source organization, or implementation
identifiers.

Every claim is tagged:

- **[source]** — directly supported by the commit-pinned source snapshot
- **[schema]** — directly supported by a typed settings, message, or wire schema
- **[inferred]** — a conclusion drawn from several visible mechanisms
- **[limitation]** — a boundary on what this snapshot can prove

The snapshot contains 1,904 tracked files and approximately 512,000 lines,
almost entirely TypeScript and TSX. It is nevertheless an incomplete source
extract: it has no package manifest, license, test suite, release tags, or
reproducible build instructions, its version is injected during compilation,
and several compile-time-gated modules are absent. The audit therefore
distinguishes architecture visible in source from behavior proven in a
shippable release.

## TL.DR

Claude Code is not fundamentally a terminal chat client. It is a local agent
engine with several front doors: an interactive React terminal application, a
headless command, a bidirectional streaming SDK protocol, IDE integrations,
and remote-control clients. Those surfaces converge on a model-streaming loop,
a typed tool runtime, a layered authority system, local append-only history,
background task managers, and an extensibility platform spanning MCP, skills,
agents, hooks, and plugins.

Its strongest architectural idea is the **local engine as the authority over
execution**. The terminal, IDE, SDK consumer, web client, and mobile client can
all drive a session without moving ordinary filesystem and shell authority
into the presentation layer. Its second strongest idea is that the SDK stream
is a real control protocol, not a log tail: it exposes initialization,
messages, partial deltas, tool progress, permission requests, tasks, hooks,
cost, usage, rewind, MCP control, model changes, and terminal results as typed
events and requests.

Its local-first operational design is unusually deep. Conversations are
append-only graphs rather than flat chat logs. Tool results may spill to disk.
file checkpoints enable rewind. Worktrees have explicit create, retain, and
cleanup behavior. Background agents and shell jobs have durable output. And
compaction attempts to preserve the context needed to continue work. These
choices make recovery and inspection possible even when the UI disappears.

The architecture also carries substantial complexity debt. Interactive mode
owns the query loop directly while headless/SDK mode has a separate
conversation-engine abstraction, creating parallel lifecycle paths that must
remain behaviorally aligned. The visible source contains 88 distinct
compile-time feature symbols, multiple partially overlapping memory systems,
several forms of task or agent execution, and compatibility transports for a
rapidly evolving remote-control plane. A few very large composition units and
module-global state further concentrate risk.

Security is layered and serious but easy to misunderstand. Workspace trust,
permission rules, permission hooks, shell analysis, tool safety checks, and OS
sandboxing are distinct mechanisms. The sandbox is opt-in in this snapshot,
and approval is not isolation. macOS credentials prefer Keychain, while the
visible non-macOS fallback is a mode-restricted plaintext file. Print mode can
skip the interactive trust dialog. These are important boundaries, not minor
implementation details.

For OpenAgents, the right adaptation is a single Effect-based engine protocol
shared by desktop, terminal, browser, and mobile. A canonical event graph and
task lifecycle. Explicit fail-closed execution profiles. Typed authority and
receipts. Durable local recovery. First-class worktrees. And open MCP/skill
compatibility. We should not copy the duplicated query owners, implicit JSONL
graph semantics, optional isolation, plaintext credential fallback, or build
matrix dominated by feature flags.

## 1. Snapshot identity and confidence

| Field | Value | Evidence |
| --- | --- | --- |
| Snapshot commit | `813c06acfa2d705076df6193b405c81eb11a18d1` | [source] |
| Commit/import date | 2026-03-31 | [source] |
| Tracked files | 1,904 | [source] |
| Approximate lines | 512,000 | [source] |
| Primary languages | TypeScript and TSX | [source] |
| Packaging target | Bun-compiled standalone CLI, with Node/npm installation compatibility | [source] |
| UI model | React terminal application on a custom Ink-style renderer | [source] |
| Embedded product version | Not available. Injected at build time | [limitation] |
| Reproducible build | Not possible from this extract alone | [limitation] |
| Test evidence | No test suite present in the snapshot | [limitation] |

The commit date is not a reliable Claude Code version date. The source contains
capabilities that also appear in later local Claude histories, while the
snapshot itself has only one import commit and no release lineage. It is best
understood as a historical source-shaped view of an evolving product, not as a
tagged upstream release. [limitation]

Several referenced implementations are absent behind build-time gates,
including portions of daemon execution, remote environments, jobs, SSH, and
workflow execution. The visible interfaces and call sites still reveal how
those capabilities join the larger system, but they do not prove complete
runtime behavior. [source] [limitation]

This teardown is consequently strongest on:

- the public CLI and SDK contracts.
- terminal composition and rendering.
- tool, permission, hook, and settings semantics.
- transcript, checkpoint, task, and worktree persistence.
- visible MCP, skill, plugin, IDE, and remote-control boundaries. And
- architectural coupling visible across those systems.

It is weaker on exact release packaging, disabled enterprise experiments,
absent gated modules, production service behavior, and any capability whose
implementation lives outside the extract.

## 2. Architecture at a glance

The runtime can be described as five planes:

```text
  Interactive TUI   Headless / SDK   IDE   Web / mobile remote control
          \              |            |               /
           +-------------+------------+--------------+
                                 |
                       Session / conversation plane
                prompt assembly, streaming, compaction,
                   model/provider and usage accounting
                                 |
                         Agent execution plane
               typed tools, permissions, hooks, tasks,
                  subagents, MCP, LSP, shell and Git
                                 |
                         Local authority plane
              workspace, credentials, settings, policy,
                transcripts, checkpoints and worktrees
                                 |
                      External provider / control plane
             model APIs, MCP servers, plugin sources,
                  update services and remote relays
```

[source] The key boundary is that ordinary execution remains local. Remote
clients can observe, prompt, and participate in approvals, but the local engine
owns the workspace, subprocesses, tools, credentials, and session persistence.
That is a much better trust topology than granting a web renderer ambient shell
authority.

| Concern | Owning layer | Important consequence |
| --- | --- | --- |
| Presentation | terminal renderer, SDK consumer, IDE, remote client | multiple clients can reuse one execution engine |
| Conversation | prompt/context assembly and streamed model loop | compaction and prompt-cache behavior become runtime concerns |
| Execution | typed tool dispatcher and task managers | concurrency, cancellation, progress, and output retention are centralized |
| Authority | permissions, hooks, trust, sandbox, managed policy | approval, policy, and isolation remain separate decisions |
| Persistence | append-only transcript, sidecars, task output, checkpoints | sessions can resume and rewind without the original UI |
| Extensions | MCP, skills, agents, plugins, LSP | third-party capability joins through several trust boundaries |
| Distribution | streaming protocol and remote bridge | the same session can be driven from terminal, SDK, web, or mobile |

The product is both a CLI application and a platform. Treating it only as a
chat interface misses most of its actual architecture.

## 3. Packaging, startup, and runtime shape

### 3.1 Bun compilation with aggressive dead-code elimination

[source] The primary distribution target is a single Bun-compiled executable.
The build injects version information and feature constants, then relies on
dead-code elimination to remove unavailable products and integration paths.
Compatibility remains for package-manager installations and Node-oriented
environments.

[source] The source references 88 distinct compile-time feature symbols. Some
gate product surfaces. Others select internal, enterprise, provider, remote,
browser, workflow, or experimental behavior. This provides distribution
flexibility but turns the build matrix into an architectural dimension. A code
path that type-checks in one build can be absent, structurally different, or
untested in another.

[inferred] This is an effective strategy for shipping one codebase into many
Anthropic environments, but a poor model for OpenAgents' core semantics. Our
engine protocol, authority rules, and task states should not change meaning by
build flavor. Provider and product adapters can be compiled conditionally at
the edge. The canonical state machine should remain invariant.

### 3.2 A deliberately fast command-line front door

[source] Startup is optimized around the observation that many invocations are
short-lived. Version output avoids importing the application. Heavy modules are
loaded only after command routing. Keychain and managed-device settings are
prefetched in parallel. Large subcommand families are skipped when the process
is plainly entering print mode. Background initialization is used for plugin
and policy work that need not block the first frame.

This is a coherent performance strategy:

1. decide the cheapest possible route.
2. return immediately for trivial commands.
3. begin high-latency secure-storage and policy reads early.
4. load the terminal or SDK surface only when selected. And
5. reconcile noncritical extension state after useful work can begin.

[inferred] OpenAgents should borrow the staged-startup discipline, especially
for desktop and terminal clients. It should not make correctness-critical
policy asynchronous unless the safe initial state is explicit.

### 3.3 Large composition roots

[source] The visible implementation has a broad CLI composition root and an
exceptionally large interactive application component. Both coordinate many
feature flags, contexts, modal surfaces, providers, tasks, hook events,
permissions, and remote states. Several services also depend on module-global
mutable state or caches.

[inferred] This makes cross-cutting behavior easy to add quickly but difficult
to reason about locally. The danger is not file size alone. It is that one
component becomes the implicit owner of lifecycle sequencing. OpenAgents should
keep composition roots declarative and move lifecycle ownership into typed
Effect services with testable scopes.

## 4. Product surfaces and operating modes

### 4.1 Interactive terminal application

[source] With no one-shot instruction, Claude Code opens a full-screen or
inline terminal application. It owns prompt editing, message rendering, model
and permission selection, tool progress, diffs, background tasks, plan mode,
cost and context displays, notifications, attachments, session navigation,
and multiple modal workflows.

This is the richest local surface, but it is not a separate engine. It consumes
the same tool, transcript, provider, settings, extension, and task systems used
elsewhere.

### 4.2 Print and headless execution

[schema] Print mode accepts plain text or streaming JSON input and produces
plain text, one final JSON result, or a stream of JSON messages. It supports:

- turn, token, and dollar budgets.
- model, effort, thinking, and fallback-model selection.
- allowed, denied, or explicitly selected tools.
- permission modes and permission-prompt integration.
- additional directories, settings sources, MCP configuration, agents, and
  plugins.
- continuation, resume, session fork, and resume from a specific message.
- partial assistant-message events.
- JSON Schema-constrained structured output.
- file checkpointing and rewind controls. And
- remote, worktree, browser, IDE, and environment-related modes when enabled.

[source] Print mode intentionally does not show the normal interactive
workspace-trust dialog. It emits a warning and expects callers to invoke it only
from trusted directories. That is practical for automation, but it makes the
launching process responsible for a security decision that interactive users
normally see.

### 4.3 Hermetic or “bare” operation

[source] A minimal mode deliberately suppresses ambient behavior: project
instruction discovery, hooks, language servers, plugin synchronization,
automatic memory, background prefetch, keychain lookup, and other convenience
state are skipped. Explicitly supplied tools, plugins, MCP servers, extra
directories, and credentials can still be used.

This is one of the best product concepts in the snapshot. It answers a hard
question: “What exactly influenced this run?” It is useful for CI, debugging,
formal verification, reproducibility, and security-sensitive automation.

[inferred] OpenAgents should provide an equivalent named execution profile. It
should produce a receipt listing every admitted source of authority and context,
not merely disable a collection of features.

### 4.4 Resume, fork, and rewind

[schema] Sessions can be continued, selected interactively, resumed by ID,
forked under a new ID, resumed at a selected message, or rewound with optional
file restoration. These are engine operations, not UI-only conveniences.

[source] A fork preserves the relevant conversation and prompt-cache prefix but
restamps the new session. A rewind chooses an earlier graph position. Append-only
records remain available even when no longer on the active branch. File rewind
is mediated by the checkpoint store rather than inferred from Git.

## 5. The terminal is a real application platform

### 5.1 React reconciler and layout engine

[source] The TUI is built with React on a custom Ink-derived renderer backed by
a terminal screen buffer and Yoga layout. It handles reconciliation, flexbox
layout, terminal measurement, cursor and focus, keyboard and mouse input,
alternate-screen behavior, bidirectional text, tmux passthrough, and incremental
screen updates.

This is materially different from printing Markdown between prompts. The
renderer must preserve terminal state while concurrent model deltas, tool
progress, permission prompts, background notifications, IDE events, and user
input all arrive.

### 5.2 Diffing and virtualization

[source] The renderer maintains a virtual screen, computes changes, and writes
only the necessary terminal updates. Long message histories use virtualization
and controlled measurement rather than rendering every line every time.

[inferred] The architectural lesson is broader than terminal UI: agent surfaces
must treat high-frequency progress as a projection of state, not as an endless
append to a component tree. OpenAgents Desktop should similarly project typed
events into bounded, virtualized views.

### 5.3 Presentation is still coupled to lifecycle

[source] Despite the capable renderer, interactive mode still directly owns
substantial parts of query submission, cancellation, compacting, session setup,
remote attachment, and result handling. Headless and SDK execution use a more
explicit persistent conversation abstraction.

[inferred] This is the clearest architectural fault line in the snapshot. Two
owners can drift on hook order, cancellation, context injection, cost handling,
permission behavior, or transcript writes. OpenAgents should require every
surface to call the same conversation service and consume the same event stream.

## 6. Conversation and model-streaming engine

### 6.1 The query loop

[source] A turn assembles system instructions, conversation messages, dynamic
context attachments, selected tools, model/provider options, and policy. It
streams model output, recognizes tool requests, executes them through the tool
runtime, appends normalized results, and continues until the model stops, a
budget is reached, the user interrupts, policy blocks progress, or execution
fails.

The loop also coordinates:

- retries and provider fallback.
- prompt caching and stable tool ordering.
- token, cost, and duration accounting.
- structured output validation and retry.
- compact boundaries and context recovery.
- tool-result size management.
- partial-message emission.
- task and hook notifications. And
- final stop-reason normalization.

[inferred] This is an agent runtime state machine even where the implementation
is expressed as generators, callbacks, and React state. Making that state
machine explicit would reduce a large class of recovery and cross-surface bugs.

### 6.2 Prompt-cache stability is an architectural constraint

[source] Tool definitions are sorted so a stable prefix survives dynamic MCP
changes. Forks can retain rendered system context. Large or replaced tool
results record the replacement decision so resume does not silently rebuild a
different prompt. Compaction restores selected context after the boundary.

[inferred] Prompt caching is not a provider optimization bolted onto the end.
It influences ordering, persistence, fork semantics, and context mutation.
OpenAgents should represent prompt-cache inputs as an explicit compiled
artifact with provenance rather than rely on incidental array order.

### 6.3 Budget and termination semantics

[schema] The runtime distinguishes successful completion from execution error,
maximum turns, maximum budget, and structured-output exhaustion. Final results
include wall duration, API duration, turn count, cost, aggregate usage,
per-model usage, stop reason, denied permissions, and optional structured
output.

[inferred] This is a good basis for receipts. OpenAgents should go further by
including effective authority, workspace identity, checkpoint/worktree outcome,
and durable task disposition in the same terminal record.

## 7. The SDK stream is a control protocol

### 7.1 Initialization contract

[schema] The first initialization message can describe:

- working directory and session identity.
- available tools and MCP servers.
- selected model and permission mode.
- slash commands.
- runtime version and output style.
- configured agents and skills.
- installed plugins. And
- fast-mode or other capability state.

This gives a client enough information to render a functional control surface
without importing the engine implementation.

### 7.2 Server-to-client event vocabulary

[schema] The stream includes more than user and assistant messages. It can
carry:

- partial model events and final result records.
- compact boundaries and session-state changes.
- retry, status, rate-limit, and authentication events.
- local-command and tool progress.
- hook start, progress, and response events.
- file-persistence notifications.
- task started, progress, and terminal notifications.
- tool-use summaries.
- MCP elicitation completion.
- prompt suggestions. And
- usage, cost, denial, and stop metadata.

[inferred] The breadth is an important competitive advantage. Clients do not
have to scrape terminal text to understand the engine.

### 7.3 Client-to-engine control vocabulary

[schema] Bidirectional control includes:

- interrupt and asynchronous cancellation.
- tool-permission decisions.
- permission-mode, model, and thinking changes.
- MCP status, message, set, reconnect, and toggle operations.
- current context-usage queries.
- file rewind.
- plugin reload.
- background-task stop.
- settings inspection and flag-setting controls.
- read-state seeding.
- hook callbacks. And
- MCP elicitation responses.

[source] The protocol preserves a legacy tool name for older SDK, transcript,
and hook consumers even though the current user-facing tool has another name.
This is a pragmatic compatibility bridge, but it demonstrates why wire names
must be versioned independently of UI labels.

### 7.4 Protocol assessment

The protocol gets four things right:

1. state changes are typed rather than inferred from prose.
2. control is bidirectional.
3. long-running tools and tasks expose progress and terminal outcomes. And
4. initialization advertises capabilities instead of assuming one client
   version.

It remains vulnerable to schema growth without a clearly visible external
version-negotiation story. [inferred] OpenAgents should define an Effect Schema
protocol with explicit version, capability negotiation, idempotency keys, event
sequence numbers, and replay boundaries from the first release.

## 8. Models, providers, and credentials

### 8.1 Provider topology

[source] The runtime can call Anthropic directly and can route through Amazon
Bedrock, Google Vertex AI, or Microsoft Foundry. It supports named aliases,
provider-specific model mapping, enterprise allowlists, fallback behavior,
prompt-caching variants, fast mode, and surfaced rate-limit state.

[inferred] Provider neutrality is implemented below the conversation surfaces,
which is the correct boundary. A terminal or SDK consumer should not need a
different lifecycle because the model is hosted through another provider.

### 8.2 Authentication sources

[source] Visible authentication paths include Anthropic account OAuth,
subscription-backed access, API keys, auth tokens, an external API-key helper,
and cloud-provider credentials. Managed desktop or remote contexts deliberately
avoid falling back to unrelated terminal-user API-key settings.

[source] OAuth refresh is serialized so simultaneous requests do not race to
replace credentials. Secure storage is prefetched during startup to reduce
latency. The API-key helper is guarded by workspace trust because it executes a
local command.

### 8.3 Secure-storage boundary

[source] On macOS, credentials prefer Keychain. The visible fallback writes a
mode-restricted local credentials file. The non-macOS path in this snapshot also
uses that plaintext file and contains an unimplemented secure-store direction.

File mode `0600` prevents casual cross-user reads, but it is not encryption at
rest. It remains exposed to compromise of the user account, malicious same-user
processes, backups, and accidental diagnostic collection.

[inferred] OpenAgents must use native secure storage on every supported desktop
platform or fail explicitly. A silent plaintext fallback is unacceptable for
provider keys, remote-control tokens, signing material, or wallet-adjacent
authority.

### 8.4 Authentication isolation

[source] The minimal execution mode avoids ambient keychain and settings lookup,
and managed contexts narrow which credential sources are admissible. These are
good examples of authentication being compiled from execution context rather
than globally available to every invocation.

OpenAgents should record the credential *class and provenance* in a run receipt
without recording the secret itself.

## 9. The typed tool runtime

### 9.1 Tool contract

[schema] A tool defines typed input and output, a model-facing description,
input validation, permission behavior, concurrency and destructiveness
characteristics, interruption semantics, execution, progress, UI rendering,
and conversion of its result back into model content.

[source] The built-in set covers agent delegation, task output, shell execution,
file search and reading, editing and writing, notebooks, web access, planning,
user questions, skills, persistent tasks, teams and messaging, worktrees, MCP
resources, language-server operations, and dynamic tool discovery. Some members
are build-gated or absent in the extract.

This contract is much stronger than a dictionary of callable functions. The
engine knows enough about a tool to schedule, authorize, interrupt, display,
persist, and summarize it.

### 9.2 Tool assembly and precedence

[source] Built-ins are filtered through policy before model exposure. MCP tools
are then merged and deduplicated, with built-ins retaining precedence on name
conflicts. Ordering is stabilized to preserve prompt-cache prefixes.

[inferred] This reduces accidental shadowing, but a globally shared flat name
space remains risky. OpenAgents should use canonical namespaced tool identity
while allowing clients to display friendly aliases.

### 9.3 Execution pipeline

[source] A tool request passes through a lifecycle equivalent to:

```text
schema validation
    -> deterministic safety analysis
    -> pre-execution hooks
    -> permission resolution
    -> cancellable execution and progress
    -> durable result handling
    -> success/failure hooks
    -> transcript and model continuation
```

Each stage can affect whether and how the next stage proceeds. Child abort
controllers let one request be cancelled without indiscriminately destroying
the parent session.

### 9.4 Concurrency and cancellation

[source] Consecutive tools declared safe for concurrent execution can run in
parallel, subject to a bounded concurrency limit. Mutating or otherwise unsafe
tools serialize. Certain shell failures can cancel sibling activity when
continuing would be misleading, while independent read failures need not stop
other reads.

[inferred] This is a practical scheduling policy, but “concurrency safe” should
be derived from declared effects and resource scopes, not only a Boolean tool
property. Effect services give OpenAgents a stronger way to express filesystem,
process, network, Git, and task mutations.

### 9.5 Large-result handling

[source] Large individual tool results are written to disk and replaced in the
prompt by a reference. Aggregate tool content within one model message also has
a budget. Replacement decisions are persisted so resume and prompt caching use
the same representation. Empty results are normalized to an explicit marker.

This is operationally sound: unbounded command output should not consume the
entire model context or UI memory. The weakness is that disk references and
replacement state become another persistence substrate a session must retain.

## 10. Authority: trust, permissions, hooks, and safety

Claude Code does not have one “permission system.” It has several layers with
different jobs:

| Layer | Question answered | Does not prove |
| --- | --- | --- |
| Workspace trust | May project-controlled configuration influence this run? | that every tool call is safe |
| Tool visibility | May the model even see this capability? | that a specific invocation is allowed |
| Deterministic safety analysis | Is this bounded request structurally safe? | that user or organization policy permits it |
| Permission rules | Is this tool/input allowed, denied, or askable? | that the OS will contain it |
| Permission hook | Does an external policy extension approve or reject it? | that core deny rules can be bypassed |
| Interactive approval | Does the operator approve this request now? | that the process is sandboxed |
| OS sandbox | What resources can the subprocess actually reach? | that the requested change is desirable |

Confusing these layers is the largest likely source of user misunderstanding.

### 10.1 Workspace trust

[source] Project-controlled settings, hooks, helpers, instructions, and related
ambient behavior are guarded by a workspace-trust decision in interactive use.
The aim is to prevent opening an arbitrary repository from immediately running
repository-supplied behavior.

[source] Print mode does not perform the same dialog, and the minimal mode skips
most ambient project influence. These exceptions need to be visible in any
automation receipt.

### 10.2 Permission modes and rules

[schema] User-facing modes include ordinary prompting, automatic edit
acceptance, plan-only operation, denial instead of prompting, and a bypass mode.
Internal paths also support automatic classification and permission bubbling.

[schema] Rules are sourced and typed, with allow, deny, and ask outcomes.
Denials are evaluated before allowances. A tool-wide deny can remove the tool
from the model-visible set. In noninteractive/background contexts, absence of a
positive decision can become denial rather than a blocked prompt.

[schema] Decision records preserve reasons such as matching rule, permission
mode, shell subcommand analysis, prompt-tool response, hook result, sandbox
state, classifier result, working-directory boundary, or fixed safety policy.
This is valuable audit data.

### 10.3 Hooks are an authority surface

[schema] The visible hook vocabulary includes 27 lifecycle events:

- before and after tool use, including failure.
- user-prompt submission, stop, and stop failure.
- session start and end.
- subagent start and stop.
- before and after compaction.
- permission request and denial.
- setup and notification.
- teammate idle, task creation, and task completion.
- MCP elicitation and its result.
- configuration change.
- worktree creation and removal.
- instruction loading.
- working-directory changes. And
- file changes.

[source] Hook handlers can use local commands, HTTP in supported contexts,
prompt/model evaluation, or agent execution depending on event and build.
They can add context, block progress, transform selected input or output,
provide permission decisions, and answer some MCP elicitations. Managed policy
can constrain what projects or agents register.

[inferred] Calling these “hooks” understates their authority. They are extension
middleware inside the agent lifecycle. OpenAgents should model internal policy
as typed Effect services and reserve external hooks for explicitly bounded,
untrusted integration. A shell hook should never become an invisible root of
authority.

### 10.4 Automatic classification

[source] Some builds can consult a classifier for permission decisions, while
fixed safety boundaries and deterministic parsing remain separate. The feature
is gated in this snapshot and cannot be treated as universally present.

[inferred] Semantic policy can help categorize intent, but it must never weaken
hard runtime boundaries. OpenAgents' typed authority compiler should produce a
bounded request first. Any model-assisted policy decision should be auditable,
revocable, and subordinate to deterministic invariants.

## 11. Sandboxing and shell safety

### 11.1 Sandbox architecture

[source] External commands can be wrapped by a dedicated sandbox runtime using
macOS Seatbelt or Linux/WSL2 isolation built from bubblewrap, proxying, and
system-call restrictions. WSL1 is not supported. The sandbox can constrain
filesystem reads and writes, network destinations, Unix sockets, and local
network binding.

[source] The execution environment normally admits the working directory and
temporary paths while denying sensitive configuration and extension surfaces.
It accounts for worktree/main-repository relationships and additional admitted
directories. It also cleans up certain dangerous bare Git metadata a command
could leave behind.

### 11.2 Optional in this snapshot

[source] Sandboxing is not globally on by default in the visible settings
schema. Configuration can require failure when unavailable. Otherwise explicit
sandbox requests may degrade with a warning. Sandboxed commands can be
auto-allowed, and an escape path for unsandboxed commands can remain enabled.

This makes the effective promise conditional:

- a permission approval may authorize an unsandboxed command.
- an “auto-allowed because sandboxed” command is only as safe as the effective
  sandbox configuration.
- a fallback warning is weaker than a fail-closed execution profile. And
- filesystem isolation does not imply network isolation unless both are set.

[inferred] OpenAgents should expose named profiles such as `observe`,
`workspace-write`, `networked-build`, and `full-host`, each compiling to an
explicit authority manifest. The UI should show the effective profile, not a
single ambiguous shield icon.

### 11.3 Shell parsing and command policy

[source] The runtime performs substantial command analysis: chains and
subcommands, directory changes, path traversal, process and environment access,
network utilities, sensitive configuration, executable resolution, and common
shell escape patterns. PowerShell has separate treatment rather than being
forced through POSIX assumptions.

[inferred] Parsing shell is defense in depth, not containment. Shell languages
are too expressive for a parser to prove safety in the general case. The best
architecture is typed tools for common operations, deterministic bounded
parsing for recognized shell fragments, and OS enforcement for everything the
process may actually access.

### 11.4 Bypass mode

[source] The explicit bypass mode is treated as dangerous and is framed for an
offline sandboxed environment. This is the right warning, but the runtime
cannot make an unsandboxed host safe merely by naming the mode.

OpenAgents should require a separately established containment receipt before a
“bypass prompts” mode can activate.

## 12. Context, instructions, memory, and compaction

### 12.1 Instruction hierarchy

[source] Instructions can come from managed policy, user-level configuration,
project files, project rule directories, and local project overrides. Discovery
walks directory hierarchies, supports included files, and can apply rules only
to matching paths. External includes require an approval boundary. Nested
instructions may load when work reaches the relevant directory.

[schema] Instruction-load events identify when new material joins the active
context. Skills can also activate conditionally based on file paths.

[inferred] This is powerful but easy to make invisible. Every injected
instruction should have stable identity, source, scope, content hash, and the
event that admitted it. OpenAgents should render this provenance as part of the
run, not bury it in a debug view.

### 12.2 Several memory systems

[source] The visible product includes project/user memory, automatic memory,
structured session memory, agent-specific memory at several scopes, relevance
selection, freshness behavior, and truncation. These coexist with ordinary
conversation history, project instructions, skills, tool results, and task
output.

[inferred] The product has evolved memory by adding specialized layers. That
can improve recall quickly, but it makes “why did the model know this?” harder
to answer. OpenAgents should use one typed memory envelope with source-specific
providers, not multiple uncoordinated injection mechanisms.

### 12.3 Dynamic context attachments

[source] A turn may receive file mentions, MCP resources, nested instructions,
changed-file summaries, team mailbox messages, task notifications, language
diagnostics, and other runtime attachments. Attachments can arrive between
ordinary user messages as the environment changes.

[inferred] These should be first-class events, never disguised as user-authored
text. Attribution is essential for security, debugging, and mobile sync.

### 12.4 Compaction stack

[source] Context reduction is not one algorithm. The snapshot contains manual
and automatic compaction, pre/post compact hooks, compact-boundary records,
session-memory compaction, smaller tool-result summarization, and gated
reactive/context-collapse strategies. After a major compact, the runtime can
reinject relevant tools, agents, MCP state, instructions, and continuation
context.

[source] Large tool results may be replaced before full conversation compacting
is necessary. Prompt-cache and resume stability constrain when replacements are
made and how they are recorded.

[inferred] OpenAgents should model compaction as a derivation with inputs,
algorithm version, output hash, retained authorities, and loss statement. A
summary should never silently become the only durable account of an approval,
file mutation, payment, or task outcome.

## 13. Persistence, recovery, and rewind

### 13.1 Append-only session graph

[source] Conversations are stored as project-scoped JSONL under `~/.claude`.
Message records carry stable IDs, parent IDs, session identity, working
directory, version, branch, and sidechain information. The active conversation
is reconstructed by selecting a live leaf and walking parents, not by reading
every record as a flat chronological chat.

This representation supports:

- ordinary continuation.
- branches created by retry, edit, or rewind.
- sidechain agent conversations.
- session forks.
- metadata events that do not behave like chat turns. And
- recovery without a database service.

### 13.2 Compatibility and dead branches

[source] Older progress records remain readable even though newer progress UI
events are not persisted into the message chain. Rewinds leave abandoned branch
records in place. When dead branches dominate a large file, a byte-oriented
prefilter can discard irrelevant regions before full parsing.

[source] Session files can become extremely large. The implementation includes
special behavior for files beyond normal in-memory rewrite thresholds and
comments acknowledging multi-gigabyte histories.

[inferred] JSONL is excellent for local inspectability and crash-tolerant
append. It is weaker as an implicit graph database with distributed sidecars,
branch garbage collection, and very large records. OpenAgents should keep an
exportable append log but maintain a canonical indexed event store with explicit
graph edges and retention.

### 13.3 Metadata and lazy materialization

[source] In addition to messages, storage can include summary, title, tag,
permission-mode, attribution, file-history, content-replacement, compact, and
context-collapse records. A session file may be created only after meaningful
user or assistant activity. Early metadata and hook output can buffer until
then.

[inferred] Lazy materialization avoids empty-session clutter but introduces a
window where state exists only in memory. A durable OpenAgents workflow should
materialize an execution receipt before the first external side effect.

### 13.4 File checkpoints

[source] The runtime can snapshot files before mutation, retain a bounded
in-memory index, and store content backups under a session-scoped file-history
area. Checkpoints retain permissions. Session forks can hard-link or copy the
relevant history. Rewind can restore files independently of Git history.

[source] Checkpointing is not uniformly enabled in every noninteractive path
unless explicitly configured.

This is a high-value feature: Git may be absent, dirty, ignored, or too coarse
for turn-level recovery. OpenAgents should adopt file checkpoints, but bind
every snapshot to the exact tool call and authority receipt that created it.

### 13.5 Retention and privacy

[source] Local retention is configurable, with a visible default measured in
weeks. A zero-day setting disables persistence and triggers deletion behavior.
Prompt-entry history is stored separately from session JSONL.

[inferred] Multiple local stores mean one “delete session” action must know
about transcripts, sidechains, task output, checkpoints, memory, prompt
history, debug logs, and remote identifiers. OpenAgents should define deletion
as a typed cross-store transaction with a receipt.

## 14. Extensions: MCP, skills, agents, plugins, and LSP

### 14.1 MCP as a native capability plane

[source] MCP servers can connect over local stdio, server-sent events, HTTP,
in-process SDK transport, or a hosted proxy. The runtime consumes MCP tools,
resources, prompts, notifications, and elicitation flows. It can reconnect,
toggle servers, surface status, and forward messages through the SDK protocol.

[source] Elicitation supports structured forms and URL-based continuation.
Tools join the same model-visible and permission machinery as built-ins after
policy filtering and conflict handling.

[inferred] OpenAgents should preserve MCP compatibility at the engine boundary,
while wrapping every MCP capability in the same typed authority and provenance
envelope as a native tool.

### 14.2 Skills and commands

[source] Skills are directory-based instruction packages with a `SKILL.md`
entry point. Legacy command Markdown remains supported. Discovery spans managed,
user, project, additional-directory, plugin, and MCP sources. Skill metadata can
select tools, model, context mode, hooks, path conditions, and whether execution
forks into an agent.

[source] Nested discovery is dynamic and respects ignored directories. Remote
skills face additional restrictions on inline local command execution.

[inferred] Skills are both instruction and authority bundles. OpenAgents should
separate their declarative knowledge from requested capabilities, sign the
package manifest, and show both provenance and admitted authority before use.

### 14.3 Custom agents

[schema] Agent definitions can select model, tools, permission behavior, skills,
MCP servers, hooks, memory scope, background execution, isolation, and whether
the context is fresh or forked. Built-in exploration and planning agents use the
same general shape.

This is a useful open customization model. The mistake would be treating agent
Markdown as the canonical runtime record. The durable instance needs a typed
identity, resolved configuration hash, parent edge, authority, and terminal
state.

### 14.4 Plugins and marketplaces

[source] Plugins can contribute commands, agents, skills, hooks, MCP servers,
language servers, output styles, and settings. Sources include Git repositories,
GitHub, npm packages, local directories, inline definitions, and managed
marketplaces. Installations use versioned caches and can reconcile or update in
the background.

[source] Policy can allow or block plugins and marketplaces. Visible defenses
include source verification for reserved official names, Unicode/homograph
checks, path-traversal validation, package integrity or Git commit identity,
and restrictions on dependencies that cross marketplace trust boundaries.

[limitation] An end-to-end executable-signature system for every plugin was not
observed in this extract. Package hashes and source provenance are valuable but
not equivalent to a publisher signature and verified capability manifest.

### 14.5 IDE and language-server integration

[source] IDE discovery and attachment use local coordination metadata and
workspace/process matching. Visible compatibility spans VS Code-family editors
and several JetBrains environments, including WSL path conversion. Integrations
can open diffs, focus files, report selections, and notify the engine about file
state.

[source] Language servers are managed as subprocesses over stdio. Diagnostics
can become context attachments, and plugins can contribute server definitions.

[inferred] IDE state is an advisory input, not the owner of the session. That
is the correct relationship for OpenAgents desktop/editor integrations.

## 15. Background work, subagents, teams, and workflows

The detailed historical reconstruction lives in
[Claude subagent histories](./2026-07-10-claude-subagents-rendering-analysis.md).
This section focuses on the architecture visible in the source snapshot.

### 15.1 Unified task supervision

[schema] A common task registry represents several kinds of work: local shell
jobs, local agents, remote agents, teammates, workflow-like execution, MCP
monitors, and memory-related background activity. Tasks have identifiers,
descriptions, status, progress, output location, start time, and terminal
notification behavior.

[schema] Common states include pending, running, completed, failed, and killed.
The SDK emits task-started, task-progress, and task-notification events.
Completed output can be retained for later retrieval or evicted under policy.

[source] Background shell jobs write durable output and include a watchdog for
commands that appear stalled on an interactive prompt. This prevents a hidden
background process from waiting forever for input nobody can provide.

### 15.2 Ordinary subagents

[source] A normal subagent runs another isolated query context inside the local
process. It can be foreground or background, has an independent cancellation
scope, obtains a stable task identity, and writes a separate transcript
sidechain. Context can begin fresh or fork selected parent state.

[source] Forked context preserves the parent's relevant prompt and tool state
but avoids unbounded recursive inheritance. A legacy tool name remains accepted
for older sessions and integrations while new sessions use the current agent
terminology.

[inferred] The execution model is stronger than the local rendering model. Full
child transcripts exist, but ordinary parent history often carries only a tool
result rather than an explicit typed child edge. OpenAgents should make
`spawned`, `started`, `waiting`, `completed`, `failed`, `cancelled`, and
`integrated` graph events canonical.

### 15.3 Teams and mailboxes

[source] Teams are a distinct coordination mode, not merely a synonym for
subagents. Teammates can run in-process or in separate terminal panes/processes.
Team configuration, shared task directories, ownership, and mailbox messages
coordinate work. Permission decisions can bubble to an owning process.

[inferred] Filesystem mailboxes are inspectable and resilient but introduce
another state store beside session JSONL and task output. OpenAgents should keep
the coordination semantics while placing them in one event authority.

### 15.4 Workflows and remote agents

[source] Visible interfaces reserve a more deterministic, resumable, and
cacheable workflow layer, although the implementation is incomplete in this
extract. Remote agents are represented separately from local subagents and use
sidecar identity for cloud-session resume and polling.

[limitation] The missing gated implementations prevent a full judgment of
workflow guarantees. The important visible lesson is that scripted workflow,
ordinary model delegation, team coordination, and remote cloud execution are
different execution classes and should not be collapsed into one vague
“agent” status.

## 16. Git and worktree lifecycle

### 16.1 First-class worktree operation

[source] Claude Code can start in a worktree or enter and exit one during a
session. Git-backed worktrees use a dedicated local area and temporary branch
identity derived from the current repository state. Terminal multiplexers can
open a worktree session in a new pane. Custom create/remove hooks allow another
version-control system to provide equivalent isolation.

[source] Changing working directory or worktree triggers context, configuration,
hook, and instruction reevaluation. This is important: workspace identity is a
security boundary, not merely a string shown in the prompt.

### 16.2 Outcome-sensitive cleanup

[source] Agent worktrees can be removed automatically when unchanged and
retained when they contain useful changes. Existing worktrees can be resumed.
Missing or invalid worktrees have recovery behavior rather than crashing the
whole session.

[source] Stale cleanup is conservative: it recognizes only expected ephemeral
patterns, requires age, refuses dirty state, and protects commits that have not
been pushed. Explicit removal similarly refuses to discard changes or commits
without a deliberate override and scopes itself to the current session's
worktree.

This is one of the most directly reusable designs in the teardown. Cleanup is a
state machine based on work outcome, not a blind `rm -rf` after process exit.

### 16.3 Integration is a separate state

[inferred] A completed agent task may leave a changed worktree, but that does
not mean its changes were reviewed, committed, merged, pushed, or accepted.
OpenAgents must represent those outcomes separately. “Agent completed” should
never be used as a proxy for “work landed.”

## 17. Remote control and distributed session operation

### 17.1 Local executor, remote controller

[source] A remote-control mode authenticates the local machine, advertises
environment capacity, receives work, and launches child CLI sessions through
the streaming protocol. Output and activity flow back to the service, while
tool-permission requests can be routed to the correct remote invocation.

[source] An existing interactive terminal session can also attach to a web or
mobile controller. Remote clients can send prompts, view events, and participate
in approvals. A viewer-only posture is also represented.

[inferred] The architectural win is that the remote surface does not become the
filesystem executor. The local host remains the authority boundary and can stop
work if the control channel disappears or policy changes.

### 17.2 Evolving transport compatibility

[source] The snapshot contains compatibility across older and newer transport
generations, using combinations of WebSocket, server-sent events, and
authenticated HTTP writes. The newer path tracks worker identity, epochs,
heartbeats, delivery acknowledgements, replay sequence, reconnect backoff,
sleep/wake behavior, and token refresh.

Those mechanisms reveal the real problem domain:

- commands can be delivered more than once.
- acknowledgements can be lost.
- a machine can sleep through a lease.
- an old process can reconnect after a replacement process starts.
- credentials can expire mid-session.
- progress events can arrive after a terminal state. And
- a permission response must reach the exact invocation that requested it.

[inferred] OpenAgents mobile sync needs this level of rigor from the start.
Every command should have idempotency identity. Every worker incarnation should
have an epoch. Every event should have sequence and causal session identity.
and terminal task state should be monotonic.

### 17.3 Other remote surfaces

[source] Visible boundaries also indicate direct-connect sessions, remote or
teleported sessions, environment runners, and SSH-related execution. Some
implementations are missing, so exact behavior cannot be certified.

[inferred] The common requirement is a session protocol independent of
transport. WebSocket, SSE, local stdio, and future Nostr transport should carry
the same canonical event and control types rather than define separate product
state machines.

## 18. Browser, computer use, and local integration

[source] Browser and computer-use capabilities are integrated as specialized
tool/MCP paths with their own hosts, screenshot and action rendering, and
native-browser coordination. Availability is build- and environment-dependent
in this snapshot.

[source] Deep links and native-host entry points are routed before the ordinary
interactive product. Deep-link launches visibly identify their provenance so a
user understands that another application initiated the action.

[inferred] Provenance banners are a good minimum, but high-authority actions
should require a signed, expiring launch envelope identifying origin, requested
capabilities, target workspace, and replay protection.

For OpenAgents, browser/computer use should run in an explicitly isolated
execution provider and emit typed observation/action receipts. A screenshot or
remote browser handle should not silently grant general host authority.

## 19. Settings and managed policy

### 19.1 Layered settings

[source] Configuration merges plugin defaults, user settings, shared project
settings, local project overrides, command-line settings, and managed policy.
Later ordinary sources override earlier ones. Arrays generally concatenate and
deduplicate during merge, while direct edits can replace arrays.

[source] Shared project configuration and local ignored configuration have
different collaboration semantics. Settings are schema-validated, parse errors
are preserved for display, and file watchers can apply changes without a full
restart.

[inferred] Merge provenance matters as much as the final value. OpenAgents
should retain a per-field explanation showing source, precedence, policy lock,
and normalization.

### 19.2 Managed policy precedence

[source] Managed policy itself has ordered sources, with remote policy and
operating-system management mechanisms taking precedence over managed files and
user-level policy. Policy can restrict settings sources, tools and permissions,
sandbox configuration, models, plugins, marketplaces, MCP, skills, agents,
hooks, and remote control.

[source] Remote managed settings can load asynchronously and apply while the
process is running. The visible fetch path prefers availability when remote
policy cannot be retrieved.

[inferred] Fail-open policy fetch may be acceptable only when a previously
verified local policy cache defines a safe floor. OpenAgents enterprise policy
should expose freshness, issuer, version, expiry, and the exact fallback used.

### 19.3 Configuration change as an event

[schema] Configuration changes can trigger hooks and SDK state events. This is
important because a long-lived session's authority can change after startup.

OpenAgents should recompile effective authority on every relevant configuration,
workspace, credential, or remote-policy transition and record the new policy
hash in the event log.

## 20. Installation and updates

### 20.1 Multiple distribution channels

[source] The updater recognizes native binary installs, npm/Bun-based installs,
Homebrew, winget, apk, and other package-manager ownership. It distinguishes
stable/latest channels and explicit versions, and the service can express
minimum or maximum compatible versions.

[source] Native self-update uses staging, interprocess locks, per-version
coordination, atomic replacement or symlink movement, cleanup of old versions,
and recovery behavior. Download integrity is checked through a published
SHA-256 manifest or package-manager integrity metadata.

### 20.2 Trust boundary

[limitation] A separate executable signature or transparency-log verification
was not observed in the extract. A checksum proves the bytes match the
manifest. It does not independently prove who authorized the manifest.

[inferred] OpenAgents should sign component/update manifests, bind them to
channel and platform, verify rollback constraints, and retain the verified
manifest in the upgrade receipt. Desktop host, engine, sandbox image, browser
bridge, and mobile protocol compatibility should be declared in one component
matrix.

### 20.3 Snapshot limitation

[limitation] Because the source version is injected and release history is
absent, this audit cannot map updater behavior to a precise public release or
verify which package paths shipped together.

## 21. Telemetry and privacy

### 21.1 Several telemetry audiences

[source] The runtime includes first-party product analytics, optional operational
telemetry, feature rollout, customer-configured OpenTelemetry export, and
performance tracing. Provider and environment settings can disable analytics.
an essential-only posture also suppresses nonessential network activity such as
some update and release-note checks.

[source] Customer OpenTelemetry export redacts prompt and tool content by
default unless explicit configuration admits it. Richer tracing modes can carry
more sensitive prompt/tool detail under additional gating. Internal telemetry
paths distinguish privileged fields and strip selected data before export.

### 21.2 The local data is still sensitive

[source] Local transcripts, task output, checkpoints, memories, debug data, and
prompt history can contain source code, file paths, commands, tool results,
user prompts, and model responses. Avoiding cloud telemetry does not make that
local corpus harmless.

[inferred] OpenAgents should classify every event field as public-syncable,
private-syncable, local-only, secret, or derived. Mobile sync must use a
public-safe projection by default rather than replicate raw local transcripts
and tool payloads wholesale.

### 21.3 Feature flags as a reliability dependency

[source] Feature rollout affects performance, provider, UI, memory, permission,
remote, and experimental behavior. Cached or unavailable flag state can
therefore influence security and availability, not just presentation.

[inferred] Core invariants cannot depend on a remotely mutable experiment.
OpenAgents may flag adapters and UX, but authority, event semantics, recovery,
and containment must have safe compiled defaults.

## 22. Performance engineering

Claude Code contains several complementary performance strategies:

| Strategy | Purpose | Tradeoff |
| --- | --- | --- |
| lazy entrypoint imports | fast trivial commands and print startup | more dynamic module boundaries |
| keychain/policy prefetch | hide unavoidable I/O latency | startup races require safe defaults |
| prompt-prefix stability | increase provider cache reuse | ordering and persistence become constrained |
| concurrent read-only tools | reduce agent wall time | resource effects must be classified correctly |
| bounded task output and disk spill | protect model context and UI memory | more lifecycle-managed local artifacts |
| virtualized terminal history | keep interactive rendering responsive | complex measurement and focus behavior |
| screen diffing | avoid repainting the terminal | renderer state becomes sophisticated |
| transcript byte prefilter | recover active branches from huge JSONL faster | storage format has exceeded simple-log scale |
| background extension reconciliation | improve time-to-first-prompt | eventual state must not weaken policy |

[inferred] The product is performance-aware at every layer. The cost is a large
number of caches, lazy states, and compatibility paths. OpenAgents should adopt
the measurable techniques while keeping one authoritative state representation.

## 23. Architectural assessment

### 23.1 What is exceptionally strong

1. **The CLI is an engine.** Terminal rendering is only one client of a much
   broader local runtime. [source]
2. **The SDK is semantic.** It exposes typed session, task, hook, tool,
   permission, usage, and control events instead of terminal scraping. [schema]
3. **Local state is recoverable.** Append-only history, sidechains, durable task
   output, checkpoints, and conservative worktree retention favor inspection
   over magical hidden state. [source]
4. **Execution is lifecycle-aware.** Tools declare validation, concurrency,
   permission, interruption, progress, result mapping, and UI behavior. [schema]
5. **Extensibility is broad.** MCP, skills, agents, plugins, hooks, IDEs, and
   language servers all join a common agent product. [source]
6. **Remote control preserves local authority.** Web and mobile can supervise a
   local executor rather than becoming the executor. [source]
7. **Git isolation is treated as product behavior.** Worktree creation,
   retention, resume, and cleanup reflect actual outcomes. [source]
8. **The runtime acknowledges operational failure.** Backoff, acknowledgements,
   epochs, watchdogs, cancellation, rollback, and stale-work cleanup are visible
   design concerns. [source]

### 23.2 Where the design is fragile

1. **Two query owners.** Interactive and headless/SDK paths do not yet share one
   fully encapsulated conversation owner. [source]
2. **Build combinatorics.** Eighty-eight compile-time feature symbols and absent
   gated modules make semantic equivalence difficult to establish. [source]
3. **Implicit graph storage.** JSONL plus parent pointers, sidechains, sidecars,
   content replacements, and branch filtering is locally inspectable but
   increasingly database-like. [source]
4. **Overlapping memory and compaction systems.** Several mechanisms can inject,
   summarize, replace, or preserve context. Provenance is hard to explain.
   [source] [inferred]
5. **Authority is cognitively dense.** Trust, rules, hooks, modes, classifiers,
   shell analysis, and sandboxing have different meanings that a user can
   conflate. [source] [inferred]
6. **Sandboxing is optional.** An approved command can still have broad host
   authority. [source]
7. **Non-macOS secure storage is incomplete in the visible snapshot.**
   Mode-restricted plaintext is not a complete credential strategy. [source]
8. **Extension and update trust stop short of a fully visible signature chain.**
   Integrity and provenance are present, but publisher authorization is not
   comprehensively demonstrated. [limitation]
9. **Remote compatibility is expensive.** Multiple transports and protocol
   generations increase the number of partial-failure states. [source]
10. **Large composition roots and global state.** Lifecycle changes can have
    broad hidden coupling. [source] [inferred]
11. **Local rendering underspecifies child topology.** Child histories exist,
    but ordinary delegation edges and integration outcomes are not uniformly
    first-class in the parent presentation. [source] [inferred]
12. **This snapshot has no tests.** Architecture can be inspected, but release
    confidence and behavior across the build matrix cannot be verified.
    [limitation]

### 23.3 Security posture summary

Claude Code has more security structure than most local coding agents, but its
strongest controls are conditional. The safe mental model is:

```text
policy decides whether a requested action is acceptable
sandboxing limits what the resulting process can actually reach
transcripts and receipts explain what was requested and what happened
```

None of those substitutes for the others.

### 23.4 Product-maturity summary

[inferred] The source reads like a fast-evolving platform that has repeatedly
absorbed new products: terminal agent, SDK, IDE, remote control, teams,
workflows, plugins, computer use, and enterprise policy. It has strong local
primitives, but the accreted integration surfaces now need consolidation around
one conversation state machine, one task graph, and one authority compiler.

## 24. What OpenAgents should adapt

### 24.1 Establish one engine protocol before adding more clients

Define one versioned Effect Schema protocol for desktop, terminal, browser,
mobile, and external SDK consumers. It should cover:

- capability initialization.
- conversation and partial model events.
- tool request, progress, result, and failure.
- permission request and decision.
- task and child-agent lifecycle.
- checkpoints, diffs, worktrees, and integration outcomes.
- context, memory, compact, and provenance events.
- remote connectivity and worker epoch.
- usage, budget, stop, and receipt data. And
- control requests with idempotency and cancellation.

Every surface should consume this protocol. No surface should own a separate
query loop.

### 24.2 Use one canonical event graph

Keep an append-only, human-exportable log, but do not make raw JSONL ordering
the database. Canonical events should have:

- event and aggregate IDs.
- parent/causal edges.
- monotonic sequence within an authority.
- actor, workspace, and device identity.
- schema version and content hash.
- public/private/local classification.
- idempotency key.
- authority-policy hash. And
- explicit terminal-state rules.

This graph should power desktop rendering, mobile sync, recovery, audit, and
issue/receipt projection.

### 24.3 Make task topology and integration explicit

Use distinct states for execution and delivery:

```text
created -> admitted -> running -> waiting -> completed / failed / cancelled
                                      |
                                      +-> changes_produced
                                             -> reviewed
                                             -> committed
                                             -> merged
                                             -> pushed
                                             -> accepted
```

A child result must carry its child session ID, parent edge, authority, output
artifacts, changed-workspace identity, and terminal reason. “Completed” must not
imply “landed.”

### 24.4 Compile authority into named execution profiles

Do not show one generic permission state. Compile settings, policy, user choice,
tool request, provider, sandbox, network, filesystem, credentials, and remote
origin into a typed authority manifest. Show the effective profile and block
execution when promised containment is unavailable.

Permission approval and isolation must remain separate, with separate receipts.

### 24.5 Add a hermetic mode early

Provide a reproducible mode that disables ambient instructions, memory, hooks,
plugins, background discovery, keychain fallback, and remote experiments unless
explicitly admitted. Emit a manifest of every included input. Use it for CI,
formal models, regression reproduction, and high-trust tasks.

### 24.6 Preserve local-first recovery

Adopt the strong Claude Code behaviors:

- append-before-side-effect event durability.
- disk-backed output for large tasks.
- turn/tool-level file checkpoints.
- session fork and rewind.
- conservative worktree retention.
- resumable background work. And
- inspectable export without a proprietary service.

Improve them with one indexed event authority, bounded retention, transactional
deletion, and typed cross-store receipts.

### 24.7 Design mobile sync as control-plane replication

Mobile should synchronize safe state and issue typed control commands. It
should not mirror a terminal or receive unrestricted local content by default.
Use worker epochs, sequence/ACK, replay windows, idempotency, expiring approvals,
and monotonic terminal states. The desktop/local engine remains execution
authority.

### 24.8 Adopt MCP and skill interoperability with stronger provenance

Support MCP transports and familiar skill packaging, but resolve everything to
canonical namespaced capabilities. Sign package manifests, display publisher
and source, compile requested permissions, and distinguish instructions from
executable authority.

### 24.9 Keep internal policy typed

Claude Code's broad hook vocabulary is useful. Shell and HTTP hooks are not the
right foundation for internal OpenAgents invariants. Implement first-party
policy, task coordination, receipt emission, and lifecycle reactions as typed
Effect services and events. Offer external hooks only at bounded extension
points with explicit inputs, outputs, timeouts, and authority.

### 24.10 Treat worktrees as durable product resources

Record worktree owner, base commit, branch, path, session, task, changes,
commits, push state, retention decision, and cleanup receipt. Cleanup should be
outcome-sensitive and fail closed around dirty or unpushed work.

### 24.11 Sign the component graph

Desktop host, engine, extension, sandbox image, updater, browser bridge, and
mobile protocol compatibility should be described by a signed manifest. Verify
publisher identity, integrity, version compatibility, channel, rollback floor,
and policy before activation.

### 24.12 Make context provenance visible

Represent managed instructions, project files, rules, memory, skill content,
MCP resources, diagnostics, task mail, and compacted summaries as distinct
typed attachments. The operator must be able to answer: “Why was this in the
model context?”

### 24.13 Use secure storage everywhere

Require platform secure storage or an explicitly configured encrypted secret
provider. Never silently fall back to plaintext for long-lived provider,
remote-control, signing, payment, or operator credentials.

### 24.14 Borrow performance techniques, not state duplication

Use lazy startup, parallel safe prefetch, prompt-prefix compilation,
virtualization, bounded concurrency, disk spill, and event projection. Do not
create multiple caches or client-specific lifecycle owners without one
authoritative state and invalidation model.

### 24.15 Keep invariants outside feature flags

Feature flags may select providers, experiments, or UI. They must not redefine
task terminality, permission meaning, event durability, signature verification,
or containment. Those contracts should be invariant across every OpenAgents
build.

## 25. Recommended implementation sequence for OpenAgents

1. **Protocol kernel:** define versioned engine events, controls, identities,
   task states, and receipts in Effect Schema.
2. **Single conversation service:** route desktop and terminal through one
   scoped runtime with deterministic cancellation and recovery.
3. **Canonical event store:** append, index, replay, fork, compact derivations,
   and local/public projections.
4. **Authority compiler:** merge policy, user approval, tool capabilities,
   sandbox, workspace, credentials, and remote provenance into a manifest.
5. **Execution providers:** local bounded tools, sandboxed shell, worktree
   provider, and explicit full-host escape profile.
6. **Task graph:** foreground/background tools, subagents, remote workers,
   output artifacts, and integration states.
7. **Desktop/mobile sync:** sequence, ACK, idempotency, worker epochs, safe
   projections, and expiring approval responses.
8. **Extension adapters:** MCP, skills, plugins, hooks, IDE/LSP, and browser
   capabilities through the same authority envelope.
9. **Recovery surfaces:** checkpoints, rewind, worktree retention, deletion,
   and export receipts.
10. **Signed distribution:** host/engine/sandbox/extension compatibility and
    verified update manifests.

This order gets the durable product architecture in place before accumulating
client-specific behavior. It also makes mobile sync an ordinary consumer of the
same event/control protocol rather than a separate synchronization subsystem.

## Final assessment

Claude Code's essential achievement is that it turns a local coding agent into
a reusable, remotely controllable execution engine without reducing the product
to a server daemon or a terminal transcript. Its typed stream, deep local
recovery, tool lifecycle, task supervision, worktree safety, and broad extension
model are all worth studying closely.

Its essential warning is that capability growth can outrun architectural
consolidation. Two conversation owners, many build-time variants, implicit graph
storage, overlapping memory systems, optional isolation, and several extension
trust mechanisms produce a runtime whose effective behavior is difficult to
explain in one sentence.

OpenAgents should adapt the engine-first topology and make it stricter: one
Effect runtime, one protocol, one event graph, one typed task model, one
authority compiler, explicit isolation, signed components, and projections for
each client. That provides the fastest path to a coherent desktop product and
mobile sync without inheriting the ambiguity of a rapidly accreted CLI.

## Source basis

- Historical Claude Code source snapshot at commit
  `813c06acfa2d705076df6193b405c81eb11a18d1` [source]
- [Claude desktop app teardown](./2026-07-10-claude-desktop-app-teardown.md),
  for the installed Electron host and its Claude Code sidecar relationship
- [Claude subagent histories](./2026-07-10-claude-subagents-rendering-analysis.md),
  for longitudinal evidence from local Claude histories and the older-source
  addendum
- [OpenAgents product adaptation analysis](./2026-07-10-openagents-product-adaptation-analysis.md),
  for cross-product synthesis

No claims in this document depend on credentials, private conversation content,
or live control of the Claude application. No external source code was copied.
