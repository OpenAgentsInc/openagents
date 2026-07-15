# Grok Build Architecture Teardown — 2026-07-15

Read-only architecture audit of the public xai-org/grok-build repository at
commit
[c1b5909ec707c069f1d21a93917af044e71da0d7](https://github.com/xai-org/grok-build/tree/c1b5909ec707c069f1d21a93917af044e71da0d7),
committed 2026-07-15. The purpose is product and architecture research for
OpenAgents, following the evidence convention used by the
[Claude Code](./2026-07-10-claude-code-teardown.md),
[Codex](./2026-07-10-codex-agent-runtime-teardown.md), and
[OpenCode V2](./2026-07-10-opencode-v2-architecture-teardown.md) teardowns.

This document examines the open Rust CLI, TUI, ACP agent runtime, shared leader,
session persistence, tools, subagents, worktrees, permissions, sandbox,
extensions, telemetry, updater, and verification system. It does not inspect a
signed installed binary, private SpaceXAI services, credentials, or user data.

Evidence labels:

- **[source]** — observed in the pinned source tree
- **[schema]** — encoded in a typed wire, configuration, persistence, or event
  contract
- **[test]** — encoded in a source test, scenario, benchmark, or fixture
- **[public]** — stated in the repository README or bundled user guide
- **[inferred]** — concluded from several observations
- **[limitation]** — a boundary on what this public snapshot proves

The repository is a one-commit export periodically synchronized from an
internal monorepo. It includes the full first-party source closure needed to
build Grok Build, but no public CI configuration, release workflow, historical
development sequence, or checked-in PTY performance baselines. External
contributions are explicitly not accepted. Those boundaries matter when
distinguishing source-visible design from release-proven behavior.

## TL;DR

Grok Build is a terminal-first Rust coding-agent platform, not a chat loop
wrapped in ANSI output. One binary supports a full-screen TUI, an experimental
native-scrollback renderer, headless JSON/NDJSON automation, Agent Client
Protocol clients, dashboards, persistent sessions, background work, subagents,
worktrees, MCP, skills, plugins, hooks, memory, and custom models.

Its most important engine boundary is a shared local **leader** process. Clients
connect over framed local IPC and exchange ACP messages with the same agent
runtime. The leader has explicit lock/socket discovery, per-client identity and
capabilities, protocol and binary versions, bounded registration, reconnect,
stale-version eviction, control requests, graceful update relaunch, and session
reload. This is a strong precedent for OpenAgents Pylon and Runtime Gateway:
one managed local engine can serve terminal, headless, editor, and future rich
clients without granting each surface its own conversation loop.

Persistence is deliberately local and inspectable. Each session owns an
authoritative ACP update log, model-facing chat history, summary metadata,
task plan, file rewind points, usage signals, feedback, compaction
checkpoints, and child metadata. A separate SQLite FTS5 index accelerates
search. Resume, fork, rewind, compaction, worktree creation, and remote-session
lookup are engine operations rather than TUI inventions. Unlike Codex's
append-log-plus-index design, Grok can replace chat history during compaction or
rewind; the user-visible update log remains the restore authority.

The terminal product is unusually mature. A thin event loop feeds explicit
state/action/effect machinery; rendering is decomposed into scrollback blocks,
selection, layout, prompt, modal, dashboard, and terminal-host behavior. The
native-scrollback mode prints finalized blocks once and keeps only live content
in a pinned region. Crash recovery restores terminal modes. Diagnostics turn
tmux, Wayland, clipboard, focus, color, image, and keyboard quirks into
actionable product state. Emulator-backed PTY tests, declarative scenarios,
real signals, resize storms, leader clusters, clipboard integration, fuzzing,
and performance workloads treat the terminal as a serious foreign host.

Authority is more mixed. Permission rules, approval modes, hooks, folder trust,
managed requirements, and OS sandboxing are separate mechanisms, which is
correct. Grok also documents their gaps with unusual candor:

- sandboxing is off by default;
- built-in sandbox application can warn and continue without enforcement;
- macOS child-network restriction is a no-op;
- in-process web/model/MCP networking is outside child-network restriction;
- a Bash allow prefix can approve a dangerous chained suffix;
- direct read/edit path checks do not canonicalize symlinks; and
- hooks fail open.

The source audit adds less visible gaps: local leader authorization appears to
depend on ambient socket-directory permissions rather than peer credentials or
a per-client secret; several hot actor and IPC paths use unbounded channels;
unknown or misclassified hub tools can miss the intended approval gate; and
subagents inherit broad parent runtime resources without one proven authority-
intersection law. These are architecture risks, not demonstrated exploits.

Custom deny-bearing sandbox profiles improve the posture: malformed or
unrepresentable policy refuses startup, Linux uses bubblewrap bind-over for
read denial, macOS uses Seatbelt, denied paths resist rename bypass, and a
session cannot silently resume under a different profile. But this is not a
uniform fail-closed default, and it does not yet match OpenAgents' intended
authority-manifest/effective-containment-receipt split.

Grok Build's update mechanics are operationally strong—versioned artifacts,
atomic publication, concurrent-update convergence, stale cleanup, rollback
paths, smoke tests, and leader relaunch—but the public installer/updater path
does not visibly verify a detached signature or published artifact checksum.
The public snapshot also omits its release pipeline, reproducible performance
baselines, and SBOM/provenance evidence. Source transparency is valuable, but
it is not release transparency.

For OpenAgents, the highest-value adaptations are:

1. ACP-compatible multi-client runtime semantics behind one managed local
   supervisor;
2. durable session identity plus explicit client/leader generations and
   reconnect;
3. native-scrollback and full-screen terminal hosts over one typed projection;
4. layered terminal verification that includes emulator, real host, race, and
   performance gates;
5. typed, default-off observability with an exporter-side fail-closed schema;
6. durable file rewind and worktree lifecycle as engine state; and
7. atomic component update mechanics strengthened with signatures, provenance,
   protocol compatibility, rollback receipts, and fail-closed containment.

OpenAgents should not copy Grok's permissive default posture, prefix-only
authorization, fail-open policy hooks, process-wide sandbox degradation,
ambient local-IPC trust, unbounded hot-path queues, plain-file long-lived
credential assumption, opaque public release gate, or million-line/79-crate
scale.

## 1. Snapshot identity and confidence

| Field | Value | Evidence |
| --- | --- | --- |
| Repository | xai-org/grok-build | [source] |
| Snapshot commit | c1b5909ec707c069f1d21a93917af044e71da0d7 | [source] |
| Commit time | 2026-07-15 21:11:03 +01:00 | [source] |
| Public history | One export commit | [source] [limitation] |
| License | Apache-2.0 first-party code; retained upstream licenses for ports and vendored source | [source] |
| Tracked files | 2,715 | [source] |
| Rust files | 2,219 | [source] |
| Tracked Rust lines | Approximately 1,348,496, including vendored Rust | [source] |
| Cargo workspace members | 79 | [source] |
| Rust test attributes | Approximately 25,318 across 1,456 Rust files | [test] |
| Ignored test attributes | 402 | [test] |
| Primary runtime | Rust 2024 edition, Tokio | [source] |
| Public agent protocol | Agent Client Protocol 0.10.4 with unstable extensions | [schema] |
| Primary UI | Ratatui/Crossterm terminal application | [source] |
| Public build version | Crate manifests name 0.1.220-alpha.4; shared version crate defaults to 0.2.0-dev unless injected | [source] [limitation] |
| Source-build hosts | macOS and Linux supported; Windows best-effort and not tested from this tree | [public] |

The checkout was clean and exactly matched origin/main before and after the
audit. No build, runtime, credentials, user sessions, or installed application
state were inspected.

Confidence is high for source architecture, typed contracts, bundled user
documentation, and encoded tests. Confidence is lower for:

- which ignored suites and platforms gate official releases;
- how private Grok, registry, sync, telemetry, and Computer Hub services behave;
- whether every public-build feature is enabled in released binaries;
- artifact signing, publication authorization, and release attestations;
- production retention and enterprise policy configuration; and
- performance claims whose platform baselines are absent from the export.

## 2. Architecture at a glance

~~~text
  Full-screen TUI     Native scrollback     Headless JSON     ACP editor/client
          \                  |                   |                   /
           +-----------------+-------------------+------------------+
                                      |
                           local client / leader seam
                  framed IPC, registration, versions, capabilities,
                    control, reconnect, update relaunch, session load
                                      |
                               ACP agent runtime
                     prompt queue, model loop, tools, approvals,
                  hooks, MCP, memory, compaction, subagents, goals
                                      |
              +-----------------------+------------------------+
              |                       |                        |
       local workspace          durable session state       extension planes
       files/Git/PTTY/LSP       ACP updates + chat JSONL     MCP/plugins/skills
       rewind/worktrees         summaries/checkpoints        Computer Hub/tools
              |                       |                        |
              +-----------------------+------------------------+
                                      |
                       model, web, auth, sync, telemetry APIs
~~~

[source] The public product has more than one protocol seam:

- ACP is the client-facing session and agent protocol.
- The local leader wraps ACP messages in length-prefixed IPC frames and adds
  registration, capability, lifecycle, and control metadata.
- The Computer Hub tool protocol is a separate versioned WebSocket/JSON-RPC
  contract for harnesses, tool servers, session bindings, catalog search,
  calls, progress, notifications, hooks, and server lifecycle.
- Workspace client/server types provide another bounded host-local/remote
  execution seam.

[inferred] This is a capable decomposition, but the number of protocols makes
identity and authority reconciliation load-bearing. A session ID, ACP client,
leader client, tool-hub connection, tool-server generation, workspace, model
request, and subagent must not become independent answers to “who may do what
for this run?”

## 3. Repository and package shape

### 3.1 A large, generated Rust workspace

[source] The root Cargo manifest is generated and explicitly read-only. Its 79
members divide the system into:

- pager, renderer, minimal renderer, PTY harness, prompt, and terminal support;
- shell, session, agent, lifecycle, chat-state, sampling, and compaction;
- tools, tool protocol/runtime/types, Computer Hub, MCP, LSP, and workspace;
- config, auth, secrets, paths, environment, models, hooks, plugins, and skills;
- memory, telemetry, tracing, update, crash handling, system power, and voice;
- Git status, fast worktrees, file events, code graph, hunk tracking, and
  SQLite journal selection; and
- four vendored graph/Mermaid crates.

[source] The repository explains that this is a periodically synchronized
closure from a larger internal monorepo. CONTRIBUTING.md rejects unsolicited
patches. There is no public development history beyond the export commit.

[inferred] The Apache-2.0 source makes load-bearing implementation inspectable,
but “open source” here means transparency and local build rights, not an open
governance or upstream contribution path.

### 3.2 Source reuse is declared

[source] Grok Build ports tool implementations from OpenAI Codex and OpenCode,
with prominent Apache/MIT notices and change attribution. Release builds may
embed ripgrep and optionally ugrep/bfs; the repository includes a large
third-party notice corpus and crate-local notices.

[inferred] Reusing mature open tool implementations is sensible. The stronger
lesson is that license/provenance belongs beside the code and release artifact,
not only in a marketing attribution page.

### 3.3 Complexity remains concentrated

[source] Despite crate boundaries, several composition modules are enormous:
settings, agent configuration, main application view, dashboard state, and
textarea implementation each approach or exceed ten thousand lines.

[inferred] Crate count alone does not guarantee comprehensible ownership.
OpenAgents should keep its Effect package graph smaller and treat
multi-thousand-line composition units as architecture-test targets, not only
refactoring preferences.

## 4. Product surfaces

### 4.1 Interactive TUI

[public] The default product is a full-screen terminal workbench with streamed
model output, thinking blocks, tool calls, inline diffs, task state, file
mentions, permissions, themes, images, Mermaid, voice dictation, hyperlinks,
selection, Vim navigation, modals, dashboards, and session navigation.

[source] The CLI also exposes auth, inspect, MCP, plugin, memory, model,
session, setup, update, completion, worktree, dashboard, trace/export, wrapper
PTY, and agent subcommands.

### 4.2 Native-scrollback mode

[source] The minimal renderer is not a cosmetic theme. Finalized conversation
blocks are committed once into native terminal history; only the active turn
and prompt remain in a pinned live region. The implementation handles
synchronized terminal updates, resize races, viewport remeasurement, and
multi-block commits explicitly.

[inferred] This is a valuable alternative to owning an infinite virtual
scrollback. OpenAgents terminal support should define one semantic transcript
projection and permit both a full-screen renderer and a native-history host.

### 4.3 Headless automation

[public] Headless mode supports plain output, one JSON result, or streaming
NDJSON; prompts from arguments, stdin, files, or JSON; structured output
schemas; exact resume/continue behavior; tool allow/deny filters; model and
permission selection; and session identity in output.

[public] Machine output stays on stdout while logs and update notices use
stderr. This is a small but essential automation contract.

### 4.4 ACP integration

[public] Agent stdio and server modes expose session/new, session/load, prompts,
tool progress, permissions, terminal/file operations, and xAI extensions to
editors or other clients through ACP.

[source] The shared xai-acp-lib wraps line-buffered and channel-based message
transport behind typed agent/client gateways.

### 4.5 Dashboard and current boundary

[public] The dashboard provides a process-local roster, state grouping,
pinning, reordering, search, latest-activity preview, direct reply, queued input
to busy agents, mode changes, permission/question response, stop, and detail
navigation.

[limitation] The bundled guide explicitly says the current dashboard lists only
agents owned by that pager process. A persistent supervisor that continues
agents after pager exit is a later phase, despite the existence of a shared
leader.

[inferred] Grok validates the interaction design for fast terminal supervision,
but not yet a complete cross-process, cross-device Fleet authority.

## 5. Terminal application architecture

### 5.1 Thin event loop, explicit effects

[source] The pager event loop is a Tokio select loop over terminal input,
runtime messages, timers, notifications, and task results. AppView owns UI
state and rendering. Dispatch produces Effect values; the effect executor
returns TaskResult values through the same state transition path.

[inferred] This resembles the OpenAgents Effect Native intent/service goal even
though it is implemented in Rust. The transferable pattern is a pure-ish state
transition surface with foreign I/O returned as typed results, not the choice
of UI framework.

### 5.2 Presentation is decomposed

[source] Scrollback is divided into blocks, entries, state, layout, sticky
headers, selection, rendering wrappers, and live updates. App state makes
authentication, folder trust, active view, voice, reconnect, destructive
confirmation, modal, and dashboard lifecycles explicit.

[source] Terminal diagnostics detect or explain tmux/Byobu/WezTerm, Wayland,
clipboard, focus tracking, color, keyboard, and image behavior. The crash
handler restores synchronized update, cursor, mouse, paste, focus, Kitty
keyboard, and alternate-screen modes.

[inferred] A terminal agent that fails to restore host state is not merely
ugly—it damages the user's shell. Terminal cleanup and diagnostics belong in
release guarantees.

### 5.3 Keyboard and pointer share actions

[public] Settings, dashboards, prompts, permission choices, selection, and
modals expose both keyboard and mouse behavior. Vim mode changes navigation,
not authority.

[inferred] This corroborates OpenAgents' “one action path” rule: keyboard,
pointer, native menu, command palette, and remote client should invoke one
typed command rather than parallel callbacks.

## 6. ACP and the shared leader

### 6.1 A managed local engine process

[source] The leader uses a private local socket plus sibling lock. Different
backend WebSocket URLs derive different socket suffixes, allowing environment
isolation. Connect-or-spawn is serialized by the lock; clients verify listener
readiness, register, and can adopt an existing compatible process.

[schema] Each leader client has:

- monotonically generated local client identity;
- client mode (stdio or headless);
- protocol version and binary version;
- yolo/automatic mode;
- default model;
- code-navigation capability;
- client-owned terminal capability;
- client-owned filesystem read/write capability; and
- extension/control feature discovery.

[source] IPC frames are length-prefixed JSON with a 64 MiB maximum. The
protocol distinguishes connection close, malformed JSON, oversize messages,
ACP payloads, and leader control messages.

### 6.2 Version and lifecycle are product state

[source/test] The leader exposes PID, socket, lock, backend suffix, protocol
version, binary version, profiling state, and capabilities. New clients can
evict a strictly older leader, while equal/older clients do not create an
eviction war. Tests cover stale locks, hung registration, bounded reconnect,
concurrent replacement, and version comparison.

[source] Update relaunch stops admitting new turns, waits a bounded grace
period, flushes session state, exits with a typed update reason, and lets
clients reconnect to the new binary and reload sessions.

[inferred] This is the strongest new Grok-specific lesson for OpenAgents:
Runtime Gateway/Pylon lifecycle should be a typed, queryable, versioned record,
and the update transaction should coordinate the daemon and its clients rather
than replace a binary behind a live process.

### 6.3 Embedded/local does not eliminate protocol concerns

[source] The TUI, headless entry points, and agent clients can all share the
leader, but the public tree still contains multiple ACP adapters and xAI
extension methods.

[limitation] The audit did not find one generated multi-language schema bundle
equivalent to Codex app-server's generated TypeScript and JSON Schema output.
ACP supplies a public base, while xAI extensions remain Rust-source-defined.

[inferred] OpenAgents should support ACP interoperability as an adapter while
keeping its richer Thread/Turn/Item/Work Unit/Receipt contract canonical and
generated for every first-party client.

## 7. Session actor and prompt delivery

### 7.1 One actor owns session activity

[source] The session actor coordinates initialization, prompt queue, running
task, model switching, MCP initialization/restart, file events, chat-state
events, replay buffering, compaction, memory timers, hooks, cancellation,
completion, notification drain, and terminal idle.

[source] It uses a message-passing chat-state actor and explicit persistence
messages rather than letting UI components mutate the conversation directly.

[source] Each resident session runs on a dedicated OS thread with an 8 MiB
stack, a current-thread Tokio runtime, and a `LocalSet`. That keeps the
non-`Send` session actor inside one ownership boundary, while tool calls can
execute concurrently and reassemble results in stable order.

[inferred] The ownership model is clear; the resource curve is not free.
OpenAgents should copy one-owner typed actor boundaries and explicit retry
ownership, but prefer bounded task actors on shared runtimes unless profiling
justifies a thread and reserved stack per session.

### 7.2 Follow-ups are queued or interrupting

[public/source] A prompt sent to a busy agent normally queues and drains after
the current turn. A send-now path can cancel current work before admitting the
new input. Dashboard replies reuse the same queue behavior.

[schema] Queue events carry session identity and per-item metadata; broadcasts
without session identity fail to parse.

[limitation] The source has prompt IDs, persistence acknowledgements, replay
buffers, and queue state, but the audit did not establish an OpenCode-V2-style
transaction in which client-chosen input identity is durably admitted before
every advisory wake and exact retry/conflicting reuse is reconciled uniformly
across all clients.

[inferred] OpenAgents should preserve Grok's user-facing queue/send-now clarity,
but keep its stronger durable admission plus explicit steer-at-safe-boundary
and queue-until-yield contract.

### 7.3 Current execution remains process-owned

[source] Running task and queue ownership are held by the live session actor.
Session logs and checkpoints enable reload, but no claim of hard-crash
exactly-once tool/provider execution is made.

[inferred] Durable session metadata is not a durable execution lease.
OpenAgents should keep active worker generation, durable admission, attempt,
tool side effects, and restart disposition separate.

## 8. Persistence, search, and recovery

### 8.1 Per-session storage

[public/schema] A session directory contains:

~~~text
summary.json
updates.jsonl
chat_history.jsonl
plan.json
rewind_points.jsonl
signals.json
feedback.jsonl
compaction_checkpoints/
subagents/
~~~

The directory is grouped under an encoded working-directory identity. Long
encoded names fall back to slug/hash plus a retained original path.

[public] updates.jsonl is the authoritative restore log for conversation/tool
presentation. chat_history.jsonl records model-facing items. summary.json
indexes title, times, model, message counts, parent, and agent definition.

### 8.2 JSONL plus SQLite

[source] Session content is locally inspectable JSON/JSONL. Search uses a local
SQLite FTS5 index over titles and prompts and can merge remote results.
Filesystem-aware journal selection chooses WAL on local disks and rollback
journal on network filesystems where shared-memory WAL is unsafe.

[inferred] This is a thoughtful local-first split: append-friendly truth,
rebuildable search acceleration, and filesystem-aware database behavior.

### 8.3 Not everything is append-only

[source] The persistence actor can append chat items, replace full chat
history, and flush. Compaction and rewind can rewrite model-facing history.
Conversation reset and replay paths reconstruct state from updates and
checkpoints.

[inferred] “JSONL” must not be treated as synonymous with immutable evidence.
OpenAgents should retain an append-only audit/event authority even when a
derived active-history file is compacted or rewritten.

### 8.4 Recovery-tolerant is not fully durable

[source] Readers contain torn-tail and corrupt-line recovery: a partial final
record is bounded, malformed update/chat lines can be skipped, the first
corrupt chat source is preserved, and replay repairs partial tool cards and
orphaned background state. Leader reconnect also flushes, replays from disk,
then delta-replays bytes appended after the starting offset; a missing cursor
or event ID forces full replay instead of unsafe incremental application.

[source/limitation] Ordinary JSONL writes flush userspace buffers but do not
consistently `fsync` the file and containing directory. The persistence
`FlushAndAck` path acknowledges `flush_pending`, not a durable barrier. Several
write errors warn and continue, and comments identify historical dual-writer
chat corruption during reconnect. This supports a claim of corruption-
tolerant parsing and careful replay ordering, not transactional or power-loss-
durable persistence.

[inferred] OpenAgents should name `flush`, `replay_ready`, and
`durable_barrier` separately; keep one writer/lease per session; fsync required
checkpoint files and directories; and make terminal persistence failure typed
state rather than a warning.

### 8.5 Remote restore is incomplete in this export

[source/limitation] The worktree restore path can resolve a local or remote
session and download memory, but the public build's session-state archive
restore path returns “unavailable in this build” and refuses to pretend the
conversation was recovered.

[inferred] This is good failure honesty. It also means the snapshot does not
prove host-portable session continuity.

## 9. Rewind, checkpoints, and worktrees

### 9.1 File rewind is engine state

[public/source] Rewind points capture file snapshots per user prompt.
Rewind can target conversation, files, or both. Before mutation, the engine
builds a preview, compares current content with the last agent-produced
snapshot, and reports clean files versus externally modified, created, or
deleted conflicts.

[source] Forced commit restores or deletes files and truncates/replays
conversation state, including cross-compaction handling.

[limitation] Individual file restore failures are logged and processing can
continue. The operation is not one atomic filesystem transaction, and the
bundled guide warns reverted changes are lost unless separately preserved in
Git.

[inferred] OpenAgents should adapt conflict preview and prompt-indexed
snapshots, then add stage/inspect/commit/clear, a redo baseline, worktree
generation checks, irreversible-effect disclosure, and a receipt that records
partial restore.

### 9.2 Worktrees are first-class operations

[source/public] Grok supports create, list, remove, apply, resume-in-worktree,
Git and jj detection, linked/standalone/Git modes, copy-on-write acceleration,
clean or dirty copy modes, pinned refs, and cleanup after failed setup.

[source] Resume can locate sessions across worktrees belonging to one
repository. Checkout of a persisted commit stashes dirty state first and
surfaces the stash outcome.

[inferred] This is materially stronger than treating worktrees as shell
commands. OpenAgents should keep worktree ownership and delivery states in the
canonical Work Unit/Receipt graph.

## 10. Tools and Computer Hub

### 10.1 Typed tool implementations

[source] Built-in capabilities include file read/edit/search/list, terminal,
web search/fetch, TODOs, skills, memory, LSP, subagents, task output, images,
PDF/PPTX reading, and deferred tool discovery. Different implementation
families include Grok-native, concise, hashline, Codex-derived, and
OpenCode-derived adapters.

[source] Default model-visible tool output is bounded to 40 KB; shell output is
bounded to 20,000 characters. MCP output has its own configurable cap.

[schema/source] The common runtime gives tools schema-bearing typed arguments,
typed outputs, stable IDs, capability metadata, context-aware descriptions,
dynamic listing, and a streaming contract of zero or more progress frames
followed by exactly one terminal frame. Dispatch rejects a stream that ends
without that terminal outcome. Wire JSON, model-facing content blocks, and
chat-completion projections remain distinct.

[inferred] Output budgeting at the tool boundary is correct. OpenAgents should
retain exact full-output artifacts and show when the model received a bounded
projection. It should also preserve Grok's terminal-stream invariant and
separate wire, model, UI, and durable-evidence representations.

### 10.2 Computer Hub is a separate tool plane

[schema] The xAI tool protocol has a semantic version, authenticated handshake,
hub-derived user identity, connection/server IDs, capability discovery, session
open/close/bind/attach, tool list/search/call/cancel/progress, notifications,
hooks, server status/eviction, and telemetry donation methods.

[source] The SDK adds connection pooling, reconnect, harness, and tool-server
runtime. Its call path uses session, connection, and global admission limits
with one shared deadline; installs waiters before sending to avoid an early-
response race; bounds cancel-before-register tombstones; and removes waiter
state when streams are dropped. MCP-discovered tools can bridge into the native
hub registry.

[limitation] Progress buffering can drop intermediate frames for a slow
consumer. Cancel-on-drop is opt-in, remote calls currently omit a transmitted
deadline, and local registrations silently shadow same-ID remote tools.

[inferred] The protocol is a serious capability plane, not a collection of
callbacks. OpenAgents should compare it with the authored-capability broker
design, but keep caller-bound authority, artifact generation, nested receipt
lineage, and settlement outside any foreign provider's assertions.

### 10.3 Declared capability is not proven containment

[schema] Tool capability metadata includes concurrency, cancellation,
read-only status, behavior version, frame limits, timeouts, hooks, and an
optional read/write scope. The contract says absent scope is treated as read
and mutating tools should declare write so the hub can route them to a leader.

[limitation] The audited client/core crates declare and serialize that scope,
but do not enforce leader-only routing; that consumer appears to live in the
unavailable hub service. The snapshot therefore proves a metadata contract,
not containment. Missing or misclassified scope defaulting to read is unsafe
for new extensions.

[inferred] OpenAgents should require a validated authority class at tool
registration, reject unknown scope, bind it to catalog generation, and still
intersect every call with caller, work, organization, and containment policy.

### 10.4 Catalog identity must be tied to execution

[schema] Tool server lifecycle and serve snapshots make mutable catalogs
explicit, while method enumeration and capability discovery handle version
skew.

[limitation] The audit did not prove that every model-advertised tool
definition is content-addressed and immutably captured through the later call
across ACP, MCP, plugins, and Computer Hub.

[inferred] OpenAgents should keep its captured catalog-generation requirement.

## 11. Permissions, approvals, hooks, and trust

### 11.1 Several layers answer different questions

| Layer | Purpose | Does not prove |
| --- | --- | --- |
| Folder/project trust | Whether project-controlled behavior may load | A requested action is safe |
| Tool visibility | Which capabilities the model sees | A concrete invocation is approved |
| Permission rules | Allow, ask, or deny tool/input patterns | The OS contains execution |
| Interactive approval/mode | Operator decision for a call/session | Filesystem/network enforcement |
| Hooks | External lifecycle policy/integration | Reliable fail-closed enforcement |
| Sandbox | Kernel-enforced filesystem/process limits | The requested side effect is desirable |

[public/source] Rules merge from CLI, native global/project config, managed
config/requirements, Claude-compatible settings, and remembered local grants.
Severity is deny before ask before allow regardless of source order.

### 11.2 Rule semantics contain documented holes

[public] Bash allow rules match the whole command string. A prefix such as
“git ” can approve a chain whose later segment is destructive. Deny and ask
rules receive more segment analysis than allow rules. Prefixes also have no
word-boundary guarantee unless written carefully.

[public] Direct read/edit path rules match the supplied string without
canonicalization; shell-level denied-path checks resolve symlinks, but direct
tools do not.

[public] Project permission allow rules can apply without a separate trust
prompt. The guide tells users to review unfamiliar repository configuration.

[source/limitation] Hub permission classification also relies partly on tool-
name heuristics. Unknown names can return no access classification, generic
dynamic input falls back toward read, and hub MCP approval payloads omit the
exact arguments even though the local access type retains them. An unresolved
subagent tool allowlist can preserve the full Grok tool set instead of failing
closed.

[inferred] These are convenience authorization rules, not a complete authority
compiler. OpenAgents must parse structured effects, canonicalize paths, bind
workspace identity, reject unknown authority classes, show exact approval
arguments, and rely on OS enforcement for the actual reach.

### 11.3 Hooks fail open

[public/source] Hooks can run on session, prompt, tool, permission, subagent,
compaction, and other lifecycle events. They may block or add context, but
script failure, timeout, or absence proceeds as if allowed and is surfaced in
the UI.

[inferred] Fail-open hooks are appropriate for advisory automation, not policy.
OpenAgents first-party invariants should remain typed Effect services;
enforcement hooks must have a separately declared fail-closed class.

### 11.4 Always-approve is explicit but broad

[public] Users can enable always-approve from a flag, slash command, or key.
Managed requirements can lock it off. The runtime reads back the actual
clamped state before emitting mode-change events.

[test] The state-reporting test prevents a rejected enable request from being
announced as active.

[inferred] Correct projection of effective state is valuable. The safer
OpenAgents product remains explicit, persistent danger-mode labeling plus a
separate containment receipt.

## 12. OS sandbox

### 12.1 Whole-process, irreversible profiles

[public/source] Sandbox profiles apply to the entire Grok process at startup,
so in-process file tools and child commands inherit filesystem restrictions.
The profile is fixed for the life of a session; resume refuses a different
profile rather than silently widening or narrowing it.

Built-ins include off, workspace, devbox, read-only, and strict. Custom profiles
can extend a built-in and add read-only, read-write, deny, and network policy.

### 12.2 Platform enforcement differs

[public/source] macOS uses Seatbelt. Linux uses Landlock and, for denied-path
read protection, bubblewrap bind-over. Linux child-process networking can be
blocked with seccomp. macOS child-network restriction is a no-op.

[public] In-process model, web, and MCP HTTP operations are not blocked by the
child-process network filter.

[source] MCP and command hooks inherit the host process environment unless
their own configuration overrides individual keys. Worktree-isolated
subagents can also fall back to the shared workspace when worktree setup or
rehydration fails.

[public] Linux glob denial expands files present at startup; matching files
created later are not covered. macOS translates globs into runtime Seatbelt
regex and covers later-created matches.

### 12.3 Fail-closed behavior is conditional

[public] Sandbox is off by default. If a built-in profile cannot be applied,
Grok can warn and continue without enforcement. An explicitly requested custom
profile with malformed or unrepresentable deny policy refuses startup.

[inferred] Grok proves useful cross-platform policy compilation, denied-path
rename resistance, and session-profile pinning. It does not prove a uniform
fail-closed profile. OpenAgents should never label a run “sandboxed” when the
effective enforcement silently degraded, and an explicit isolation request
must fail rather than fall back to a shared workspace.

## 13. Subagents, tasks, and topology

### 13.1 Independent child sessions

[public/source] A subagent has its own session, context window, model/tool
resolution, optional persona, capability mode, background/foreground state,
and optional worktree isolation. Built-in general, explore, and plan roles can
be shadowed by user/project definitions.

[public] Capability modes distinguish read-only, read-write, execute, and all.
Personas declare instructions plus optional input/output file contracts, model,
reasoning effort, and default isolation.

### 13.2 Continuation and result delivery

[public] resume_from continues a completed child's transcript and state while
re-rendering its system prompt and tools from the current definition.
Background work returns an ID and can be queried later. The parent receives a
summary on completion.

[source] Child metadata is persisted under the parent session while child
sessions remain normal session directories. The runtime has explicit
coordinator, activity, roster, cancellation, usage-fold, and completion paths.

[source] Depth is capped at one and child builders remove recursive task tools
at that boundary. Foreground cancellation is scoped to the parent prompt;
background work intentionally survives it. Children retain parent/prompt/
session provenance, but share the parent's permission handle, terminal
backend, environment, memory/config snapshots, scheduler, and MCP pool unless
worktree placement changes the working directory.

[limitation] The product dashboard remains process-local, and a returned child
summary does not establish review, commit, push, merge, acceptance, payout, or
settlement.

[inferred] OpenAgents should preserve independent child transcripts and
low-latency dashboard control, while keeping the persisted graph and delivery
receipt authoritative.

### 13.3 Child authority needs an intersection law

[public/schema] Capability modes narrow coarse tool classes, and worktree
isolation can change placement. Persona/role/spawn-time precedence resolves
model and isolation.

[limitation] The audit did not establish a single invariant that effective
child authority is always the intersection of parent delegation, child
definition, WorkContext, organization policy, and containment.

[limitation] No explicit concurrent-subagent admission bound was found, and
the coordinator transport is unbounded. Optional worktree isolation can fail
back to the shared workspace.

[inferred] OpenAgents must retain that non-amplification rule.

## 14. MCP, plugins, skills, and memory

### 14.1 MCP

[public/source] MCP supports local and HTTP transports, OAuth, managed
configuration, resource/tool discovery, restart, liveness monitoring, catalog
caching, status projection, and per-server tool policy. The dedicated crate
quarantines an otherwise incompatible reqwest generation.

[source/test] The session dispatcher distinguishes transport closure,
handshake/list failures, restart backoff, exhausted retries, and HTTP client
reset. Status changes are projected to clients.

[inferred] Dependency quarantine and explicit liveness are strong operational
patterns. Extension health should be typed product state rather than a generic
tool failure.

### 14.2 Plugins and skills

[public/source] Skills are instruction/tool packages discovered from user,
project, bundled, plugin, and compatible sources. Plugins can contribute
agents, skills, hooks, MCP, and other resources, with local/Git/marketplace
discovery and trust records.

[limitation] The audit did not establish universal publisher signatures,
reproducible build attestations, per-generation process isolation, or
capability manifests for every plugin.

[inferred] OpenAgents should keep format interoperability while adding signed
artifact identity, scoped generation ownership, isolated execution, authority
diff, update/rollback, and receipts.

### 14.3 Memory and compaction

[source/public] Grok has cross-session memory, session-end summaries, idle
flush, recovery search, “dream” processing, compact checkpoints, automatic and
manual compaction, image-budget compaction, and model-facing history rewrite.

[source] The session loop has multiple memory/compaction timers and recovery
paths. Subagents skip some parent-session memory behavior.

[inferred] The breadth improves continuation but increases explainability
cost. OpenAgents should use one typed context/memory envelope with provenance,
retention, derivation version, and explicit loss accounting.

## 15. Models and authentication

### 15.1 Model/provider flexibility

[public/source] Grok Build supports first-party Grok models plus custom
OpenAI-compatible endpoints, Ollama, per-model API keys/environment keys,
context limits, reasoning effort, and model switching.

[source] Sampling is separated into an actor-based HTTP/streaming/retry crate
rather than embedded directly in the TUI.

### 15.2 Authentication flows

[public/source] Authentication includes SpaceXAI browser OAuth, device code,
enterprise OIDC with loopback PKCE, API keys, and an external auth command
whose stdout is the token and stderr is user-visible status. Refresh is
serialized and reacts to expiry or 401/403.

[public] Long-lived credentials are stored in GROK_HOME/auth.json. This audit
did not find a documented platform-keyring or encrypted-at-rest default for
that file.

[inferred] OpenAgents should keep browser/device/OIDC interoperability but
retain OS-encrypted custody or an explicitly configured encrypted provider.

### 15.3 Documentation drift is visible

[public/limitation] The bundled authentication guide says an active session
token precedes XAI_API_KEY, while the shell README says the API key takes
precedence. Manifest versions also mix 0.1.220-alpha.4 and a shared
0.2.0-dev default/injected version.

[inferred] Generated reference docs and one effective account/provider
projection are safer than duplicating precedence prose across guides.

## 16. Telemetry, diagnostics, and privacy

### 16.1 Product telemetry has explicit modes

[source] Product telemetry resolves among disabled, metadata-only session
metrics, and full analytics using requirements, environment, local config,
remote settings, and a disabled fallback.

[public] Users can disable product telemetry. ZDR and coding-data-retention
settings also govern trace/research collection and remote behavior.

### 16.2 Customer OpenTelemetry is structurally separate

[public/source] External OTEL is off by default and requires both a master
switch and an exporter. It does not reuse SpaceXAI credentials. Prompt text and
tool detail require separate gates; Bash command text is never exported in the
documented v1 schema.

[schema] Metrics and events use closed attribute/event vocabularies with
schema version, session/turn/prompt identity, usage, tool decisions, model,
MCP, compaction, subagent, and auth fields.

[source/test] Strings are capped and scrubbed at emission. An exporter-side
validator drops unknown keys, closed-gate content, secret-shaped values,
unscrubbed paths, free-text bodies, and unsupported value types. Tests use
canaries for prompts, tool arguments, paths, MCP names, metric keys, and
closed gates.

[inferred] The exporter chokepoint is an excellent defense-in-depth pattern for
OpenAgents public/private/local projections.

### 16.3 Controls are still hard to explain together

[public] Product telemetry, trace upload, ZDR, coding retention, and
customer-owned OTEL are independent controls with different directions and
fallbacks.

[inferred] OpenAgents should present one data-flow matrix: source, destination,
content class, authority, retention, encryption, opt-in, and deletion behavior.

## 17. Updates and supply chain

### 17.1 Strong local update transaction

[source/test] The updater uses versioned binaries, unique temporary siblings,
executable-before-publish ordering, atomic rename/symlink or Windows swap,
rollback-aware paths, stale cleanup, concurrency convergence, smoke tests, and
leader relaunch. Tests cover network failure, corrupt downloads, chunk
assembly, cancellation, same/different-version races, stale symlinks, and
installer variants.

[inferred] Update is treated as a distributed lifecycle across file, leader,
and clients rather than “download then overwrite.”

### 17.2 Public provenance is incomplete

[source/limitation] The public install scripts and updater do not visibly
verify a detached artifact signature or published digest before execution.
The repository has no public CI/release workflow, SBOM, Sigstore/cosign, SLSA
provenance, or release-attestation bundle.

[source] bin/protoc does use Dotslash with platform hashes, Cargo.lock pins
registry checksums, and the Git dependency is commit-pinned. This proves the
team understands content-addressed dependencies; it does not close the final
binary distribution chain.

[source/limitation] Managed configuration includes an Ed25519,
identity/expiry-bound signed-policy verifier and tests, but the embedded public
key list is empty in this snapshot, so the public build does not activate that
verification path.

[inferred] OpenAgents should adapt the atomic mechanics only behind its signed
component compatibility ledger and notarization/rollback receipts.

## 18. Test, performance, and fault architecture

### 18.1 Layered verification

[test] The tree contains approximately 25,318 Rust test attributes. Most unit
tests assert typed state and buffers directly. Eight checked-in Insta snapshot
files focus on edit/diff rendering rather than snapshotting the entire product.

[test] The PTY harness launches the real binary behind Alacritty terminal
emulation and a mock inference server. It supports raw bytes, screenshots,
signals, leader clusters, host clipboard behavior, scroll matrices, scripted
YAML scenarios, and benchmarks.

[test] Forty-five declarative terminal scenarios cover user journeys. The
primary pager PTY tree alone has 161 Rust modules. A Markdown fuzz package
retains targets and seeds.

### 18.2 Performance is treated as a contract

[test] Workloads cover idle cost, large code blocks, mixed interaction, resize
storms, scrolling, streaming, and paste latency. The benchmark README
describes a platform-specific gate at more than 15 percent p99 frame-time
regression.

[limitation] The baseline directory contains only its README in this public
snapshot. Linux/macOS JSON baselines are absent, and most real-binary PTY tests
are ignored by default. No public CI configuration shows which suites gate a
release.

[inferred] OpenAgents should copy the layered test architecture and check in
the actual baselines, gate commands, platform matrix, and receipts.

## 19. Architectural assessment

### 19.1 What Grok Build gets exceptionally right

1. **The terminal is an application platform.** Rendering, input, scrollback,
   host quirks, crash cleanup, and performance are first-class systems.
2. **The local engine has a managed lifecycle.** Leader discovery, version,
   client capability, reconnect, replacement, and update are explicit.
3. **ACP is a real interoperability seam.** Editors and automation can drive
   semantic session state instead of scraping terminal output.
4. **Local recovery is deep.** Sessions, checkpoints, rewind, compaction,
   worktrees, tasks, and search survive beyond one UI frame.
5. **The dashboard optimizes supervision.** Roster, activity, reply, queue,
   interrupt, mode, and questions are close to the StarCraft-like terminal
   control surface OpenAgents wants.
6. **Tools and extensions have protocol shape.** MCP and Computer Hub include
   lifecycle, capability, catalog, progress, and reconnect concepts.
7. **Observability has a fail-closed export boundary.** Typed keys, content
   gates, scrubbing, caps, and final validation compose.
8. **Testing attacks races and hosts.** Emulator-backed PTY tests and update
   concurrency go beyond happy-path unit tests.
9. **The docs disclose security limits.** Users are told where prefix rules,
   symlinks, hooks, and platform sandboxing fall short.
10. **License provenance is unusually explicit.** Ported and bundled source is
    named with retained notices.

### 19.2 Where the design is fragile or incomplete

1. **Sandboxing is off by default.**
2. **Built-in sandbox failure can degrade to unsandboxed execution.**
3. **Network enforcement is platform- and process-class-dependent.**
4. **Permission allow matching has a documented chained-command hole.**
5. **Direct file rules do not canonicalize symlinks.**
6. **Hooks fail open and therefore cannot be enforcement authority.**
7. **Credentials are documented as a local JSON file, not encrypted custody.**
8. **Dashboard supervision is still pager-process-local.**
9. **Durable admission/exact retry is not as explicit as OpenCode V2.**
10. **Remote conversation-state restoration is unavailable in the public
    build path inspected.**
11. **The public extension signature/isolation story is incomplete.**
12. **Binary update signature/provenance is not visible.**
13. **Release CI and performance baselines are absent from the export.**
14. **Documentation precedence/version drift exists.**
15. **Local leader authorization relies on ambient filesystem boundaries in
    the audited code.**
16. **Several session, persistence, and IPC paths are unbounded.**
17. **Persistence acknowledgements are not consistently durable barriers.**
18. **Tool-scope enforcement is not present in the audited hub client/core.**
19. **The source closure is huge and several modules remain massive.**

## 20. Comparison with the other terminal-agent references

| Dimension | Grok Build | Codex | Claude Code snapshot | OpenAgents conclusion |
| --- | --- | --- | --- | --- |
| Rich-client seam | ACP plus shared leader and xAI extensions | Generated app-server used by TUI and rich clients | Bidirectional SDK stream but interactive owner still partly separate | One canonical generated protocol; ACP as adapter |
| Managed runtime | Socket/lock leader with client caps, reconnect, update relaunch | App-server daemon and remote-control relay | Local engine plus evolving remote transports | Typed Pylon lifecycle and process generation |
| Session state | Per-session ACP/chat JSONL, metadata, checkpoints, FTS5 | Rollout JSONL plus SQLite index/graph | Parent-linked JSONL plus sidecars | Append evidence plus indexed authority |
| File recovery | Prompt-indexed snapshots and conflict-preview rewind | No general file rewind found | File-history checkpoint rewind | Keep Grok/Claude recovery with receipts |
| Worktrees | First-class create/apply/remove/resume; Git/jj modes | Not first-class lifecycle in audited snapshot | Outcome-aware lifecycle | Typed worktree ownership and delivery |
| Subagents | Independent sessions, capability modes, process-local dashboard | Persisted graph and rich protocol/TUI | Sidechain histories, topology partly implicit | Persistent graph plus fast roster/control |
| Permission policy | Deny/ask/allow, compatibility, explicit known holes | Profiles, execpolicy, managed requirements | Layered trust/rules/hooks/classifier | Structured effects, canonical paths, deny first |
| Containment | Whole-process Landlock/bwrap/Seatbelt; off by default and conditional | Cross-platform named profiles, proxy, fail-closed translation | Capable but opt-in in snapshot | Uniform fail-closed profiles and receipts |
| Extensions | MCP, skills, plugins, hooks, Computer Hub | MCP, skills, plugins, apps, hooks, code mode | MCP, skills, plugins, hooks, agents | Signed isolated generations |
| Terminal proof | Deep emulator/PTY/race/perf system | Extensive Ratatui snapshots and integration tests | Capable custom React terminal renderer; no tests in extract | Combine semantic snapshots with real PTY faults |
| Updates | Atomic versioned swap plus leader relaunch; signature not visible | Signed release lanes and atomic standalone install | Multi-channel updater; signature not established in extract | Signed component ledger plus coordinated relaunch |

## 21. What OpenAgents should adapt

### 21.1 Add ACP as an interoperability adapter

Expose a bounded ACP adapter over the canonical Runtime Gateway so editors and
third-party clients can create/load sessions, prompt, observe tools, answer
permissions, and provide terminal/filesystem capabilities. ACP must enter the
same command processor, authority compiler, events, and receipts as Desktop,
mobile, terminal, and SDK.

Do not make ACP extension structs a second domain model. Generate first-party
clients from the richer OpenAgents protocol and map ACP at the edge.

### 21.2 Make Pylon lifecycle as explicit as the Grok leader

Persist and expose:

- runtime/process generation;
- binary and protocol versions;
- endpoint/socket and backend identity;
- authenticated client identity and capability set;
- readiness and active sessions;
- update/relaunch state;
- reconnect/eviction reason;
- last transition; and
- bounded shutdown/flush outcome.

Start must be idempotent. Replacement must avoid PID/socket reuse. A newer
client may request a compatible relaunch; it may not silently kill active work
or attach to an unverified old process.

Authenticate local clients with peer identity or a protected per-generation
secret, create the endpoint directory with explicit owner-only permissions,
bound every hot queue, and expose overload/backpressure as protocol state.

### 21.3 Copy the replay and retry ownership, not the durability ambiguity

Use stable event IDs and reconnect cursors; fall back to full replay whenever
incremental idempotency cannot be proven. During reload, gate live delivery,
flush pending writes, replay the stable prefix, then delta-replay the appended
tail. Assign transport retry to the provider/sampler layer and compaction,
auth refresh, and work disposition to the session layer so two layers never
retry the same effect independently.

Define separate acknowledged contracts for accepted, flushed, replay-ready,
and power-loss-durable state. Use one session writer, bounded mailboxes, and
file-plus-directory fsync for the checkpoints whose receipts claim durability.

### 21.4 Build a real terminal renderer family

Use one typed transcript/command projection with:

- full-screen workbench renderer;
- native-scrollback renderer;
- bounded headless JSON/NDJSON output; and
- ACP/editor projection.

Define atomic frame, resize, focus, paste, selection, clipboard, image, crash
restore, native scrollback commit, accessibility, and terminal-capability
invariants.

### 21.5 Make terminal proof a release gate

Combine:

- pure reducer/buffer tests;
- scenario fixtures;
- emulator-backed PTY tests against a real binary;
- real OS signals and terminal-mode cleanup;
- leader/client reconnect and version skew;
- clipboard, mouse, paste, resize, and scroll matrices;
- fuzzing; and
- checked-in p50/p95/p99 performance baselines per supported platform.

The public gate should name exactly which ignored/real-host suites ran and
retain results as receipts.

### 21.6 Adopt conflict-aware rewind

Before restoring files, compare current content with the last engine-produced
state and surface external modifications. Then strengthen Grok's shape with
stage/inspect/commit/clear, partial-failure accounting, irreversible-effect
lists, worktree generation, redo baseline, and a checkpoint receipt.

### 21.7 Treat worktrees as engine resources

Support typed create, copy mode, base/ref, apply, retain, resume, remove,
cleanup, and Git/jj adapter behavior. Bind each worktree to Work Unit, owner,
authority, source state, dirty state, commits, delivery, and cleanup receipt.

### 21.8 Adopt the exporter-side telemetry firewall

Every telemetry/public-sync sink should accept only a closed typed vocabulary,
apply content gates, scrub and cap values, then validate again immediately
before export. Unknown or unsafe fields fail closed.

Render one user-facing matrix for product analytics, diagnostics, private raw
evidence, customer OTEL, public receipts, retention, ZDR, and deletion.

### 21.9 Coordinate updates with the runtime

Use versioned staging, atomic publication, concurrent-update convergence,
stale cleanup, bounded graceful drain, process relaunch, client reconnect, and
session reload. Add the pieces absent from the public Grok path:

- signed manifest and publisher identity;
- artifact digest and transparency/provenance;
- protocol/client compatibility range;
- rollback floor and last-known-good set;
- notarization/platform signature; and
- install/update/rollback receipts.

### 21.10 Preserve security-limit candor

Document exact platform gaps and degrade behavior in the product, protocol,
and receipt. A macOS no-op network restriction, Linux launch-time-only glob,
missing bubblewrap, or unsupported policy must never render as one generic
green shield.

## 22. What OpenAgents should reject

1. No sandbox-off default for autonomous coding.
2. No silent warn-and-run when promised containment cannot be established.
3. No prefix-only shell allow as a security boundary.
4. No uncanonicalized path policy for direct filesystem tools.
5. No fail-open enforcement hooks.
6. No child profile that can widen parent authority.
7. No plain local JSON fallback for long-lived high-authority credentials.
8. No in-process plugin authority without signature, capability, isolation,
   generation ownership, and receipts.
9. No unsigned updater or curl-pipe-shell trust chain as the only public
   installation proof.
10. No public “performance-gated” claim without the baselines and run receipt.
11. No process-local roster presented as durable Fleet truth.
12. No model-facing rewritten history as the only audit record.
13. No documentation copies that disagree on auth precedence or runtime
    version.
14. No second protocol/state machine for ACP, terminal, or Computer Hub.
15. No missing/unknown tool scope that defaults to read or bypasses approval.
16. No opt-in cancel-on-drop or calls without propagated deadlines.
17. No unbounded session, persistence, IPC, or subagent admission queues.
18. No ambient local-socket trust without owner-only creation and client
    authentication.
19. No claimed durable barrier that only flushes userspace buffers.
20. No inherited host environment for MCP, hooks, or plugins without an
    explicit allowlist and secret broker.
21. No million-line expansion without explicit scope law, dependency tests,
    and deletion gates.

## 23. Recommended OpenAgents sequence

1. **Canonical runtime protocol:** keep Thread/Turn/Item/Work Unit/Receipt and
   add an ACP adapter with fixture parity.
2. **Managed Pylon lifecycle:** process generation, identity, capabilities,
   version negotiation, socket discovery, reconnect, drain, and relaunch.
3. **Durable event authority:** exact admission, projection, replay marker,
   indexed search, repair, archive/delete, and public-safe Sync.
4. **Terminal hosts:** full-screen and native-scrollback renderers over the
   same typed state and command registry.
5. **Terminal verification:** reducer, scenario, PTY, real-host, race, fuzz,
   and performance receipts.
6. **Recovery/worktrees:** prompt checkpoints, conflict preview, staged rewind,
   ownership, delivery, apply/retain/remove, and cleanup receipts.
7. **Authority compiler:** structured shell effects, canonical paths, parent
   intersection, named execution profiles, and fail-closed platform plans.
8. **Extension/tool generations:** MCP, skills, plugins, authored tools, and
   foreign Computer Hub adapters under one captured signed catalog.
9. **Observability matrix:** typed exporter firewall plus explicit retention,
   private/public, customer, and ZDR projections.
10. **Signed update graph:** atomic runtime-aware relaunch behind manifest,
    signature, provenance, compatibility, rollback, and notarization proof.

## Final assessment

Grok Build is now one of the strongest open references for the **terminal
product** layer of an agent system. Codex remains the stronger generated
rich-client protocol and cross-platform containment reference; OpenCode V2
remains the clearest durable-admission/scoped-service reference; Claude Code
remains the clearest historical recovery/worktree ergonomics reference. Grok
adds the missing terminal-host discipline: a shared leader under ACP clients,
two serious rendering modes, a fast multi-agent dashboard, deep local session
resources, conflict-aware rewind, worktree breadth, explicit terminal
diagnostics, runtime-aware updates, and an unusually aggressive PTY/race test
matrix.

Its caution is that product maturity and safe authority do not automatically
move together. The same repository that carefully versions its leader,
refuses malformed custom deny profiles, tests updater races, and validates OTEL
again at export still defaults sandboxing off, permits built-in enforcement
degradation, documents shell/path policy bypasses, runs hooks fail-open, stores
auth in a local JSON file, and exposes no public binary-signature gate.

OpenAgents should adapt Grok Build's terminal and supervisor mechanics while
keeping the stricter thesis already established by the other teardowns:

- durable admission before execution;
- one generated protocol and request processor;
- complete agent graph and independent transcripts;
- parent-intersected authority;
- named fail-closed containment;
- authority and execution receipts;
- signed extension/component generations;
- public-safe cross-device projections; and
- delivery, acceptance, and settlement distinct from agent completion.

That combination would give OpenAgents a terminal surface as capable and
resilient as Grok's without inheriting its ambiguous safety defaults or opaque
release boundary.

## Source basis

- [Grok Build repository at the audited commit](https://github.com/xai-org/grok-build/tree/c1b5909ec707c069f1d21a93917af044e71da0d7)
  [source]
- [Repository README](https://github.com/xai-org/grok-build/blob/c1b5909ec707c069f1d21a93917af044e71da0d7/README.md)
  [public]
- [Shell and runtime README](https://github.com/xai-org/grok-build/blob/c1b5909ec707c069f1d21a93917af044e71da0d7/crates/codegen/xai-grok-shell/README.md)
  [public]
- [Bundled user guide](https://github.com/xai-org/grok-build/tree/c1b5909ec707c069f1d21a93917af044e71da0d7/crates/codegen/xai-grok-pager/docs/user-guide)
  [public]
- [Leader implementation](https://github.com/xai-org/grok-build/tree/c1b5909ec707c069f1d21a93917af044e71da0d7/crates/codegen/xai-grok-shell/src/leader)
  [source] [test]
- [Session runtime and persistence](https://github.com/xai-org/grok-build/tree/c1b5909ec707c069f1d21a93917af044e71da0d7/crates/codegen/xai-grok-shell/src/session)
  [source] [schema] [test]
- [Tool protocol](https://github.com/xai-org/grok-build/tree/c1b5909ec707c069f1d21a93917af044e71da0d7/crates/common/xai-tool-protocol)
  [schema] [test]
- [Sandbox implementation](https://github.com/xai-org/grok-build/tree/c1b5909ec707c069f1d21a93917af044e71da0d7/crates/codegen/xai-grok-sandbox)
  [source] [test]
- [Pager and PTY harness](https://github.com/xai-org/grok-build/tree/c1b5909ec707c069f1d21a93917af044e71da0d7/crates/codegen/xai-grok-pager)
  [source] [test]
- [Updater implementation](https://github.com/xai-org/grok-build/tree/c1b5909ec707c069f1d21a93917af044e71da0d7/crates/codegen/xai-grok-update)
  [source] [test]
- [Telemetry implementation](https://github.com/xai-org/grok-build/tree/c1b5909ec707c069f1d21a93917af044e71da0d7/crates/codegen/xai-grok-telemetry)
  [source] [schema] [test]

No credentials, private conversation content, installed application state, or
private SpaceXAI service behavior was read or used as evidence.
