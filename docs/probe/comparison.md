# Probe and Omega Agent: Architectural Comparison and Integration Roadmap

Date: 2026-07-30

## Executive summary

Probe and Omega Agent are adjacent coding-agent efforts, but they are not
currently two implementations of the same product. Omega Agent is a mature,
in-process, GPUI-bound interactive agent engine. It owns the editor-facing
conversation loop, project-aware tools, permission and terminal policy,
durable thread history, ACP bridging, and native/external subagent execution.

Probe is a Bun/Effect runtime being rebuilt as the first-party OpenAgents
coding runtime. Its current concrete slice concentrates on assignment,
provider, backend, evidence, and policy contracts; it also has a lightweight
CLI coding loop for Gemini. Its intended destination is a portable,
assignment-driven runtime that can run locally or remotely and report redacted
events and artifacts back to OpenAgents infrastructure.

The premise that the current Probe is a semantic/structural code index is not
supported by its current source. Its `search_code` implementation invokes
`rg` for each call. It has no persisted repository index, embeddings, AST
database, symbol graph, or incremental indexer. The Blueprint
`code_search` definition is an allowed-tool contract, not an implementation of
semantic retrieval. The best integration is therefore not to embed current
Probe as a magical index, but to establish an analysis-only Probe executor
with a compact, structured repository-map protocol, then add indexed search
behind that protocol when measurements justify it.

## Scope and evidence

This review uses the current source trees:

- Probe: `/Users/christopherdavid/work/probe`, especially `README.md`,
  `packages/runtime/src/cli.ts`, `llm/`, `blueprint/`, `workspace.ts`, and
  `contracts/assignment.ts`.
- Omega: `/Users/christopherdavid/work/omega/crates/agent`, especially its
  README, `src/agent.rs`, `src/thread.rs`, and `src/tools/`, plus
  `docs/src/ai/omega-agent.md`.

Probe's README labels the project as a reset and identifies old implementations
as source material rather than a compatibility contract. Historical commits
show earlier daemon/session and code-search experiments, but this report does
not mistake those for the current runtime.

## Probe today

### Intent and execution model

Probe's declared product surface is one first-party OpenAgents coding runtime.
An assignment identifies the goal, runner session, repository reference,
optional backend/provider/grant, sandbox data, callback, and Blueprint scope.
The direction is to hide whether inference is local, swarm, API-key, hosted,
or future OpenAgents routing. A run is expected to operate in a bounded
sandbox, emit redacted events/artifacts, and separate execution completion
from external product acceptance.

The implemented runtime is TypeScript on Bun with Effect 4. The CLI currently
offers account/linking commands, Gemini completion/chat, and Apple Foundation
Models readiness/smoke/tool-stream demonstrations. The generic LLM tool
runtime dispatches typed JSON-schema tools and emits tool-result/error events.
It is a small provider/backend loop, not an editor host or a general daemon
with a durable local session store.

### Current code-understanding and tool surface

The Gemini chat path supplies seven concrete local tools:

| Tool | Current behavior |
| --- | --- |
| `read_file`, `list_files` | Bounded reads/listings inside a workspace-root policy. |
| `search_code` | Starts `rg --line-number` for every request; returns a bounded list of matching lines. |
| `write_file`, `edit_file`, `apply_patch` | Workspace file mutation with BOM/line-ending/stale-content handling in the richer mutation path. |
| `current_time` | Returns the local timestamp. |

The CLI has no real shell/terminal execution tool, LSP definition/reference
tools, or subagent delegation. Its default permission handler currently
allows operations; its own comment says interactive approval still needs a
side channel. The Apple FM path deliberately projects a narrower Blueprint
tool menu: `read_file`, `code_search`, `record_evidence`, and
`propose_action_submission`. That menu is policy metadata and tool schema
selection, not the complete local Gemini menu.

`search_code` is useful fast lexical retrieval: it scopes paths through
`resolveWorkspacePath`, invokes ripgrep, and caps output (default 80 lines).
It does not retain results or build a repository model. Thus it cannot yet
answer symbol/reference/impact questions more accurately than an LLM using
repeated textual searches, and every search pays process startup and full scan
cost subject to ripgrep's normal file filtering.

### State, trust, and persistence

Probe's strongest implemented state is run-facing and portable: typed
assignment/Blueprint/provider schemas; runner identity; scoped auth
materialization and scrubbing; backend health/receipts; token telemetry; and
benchmark/evidence closeout artifacts. The intended architecture calls for a
durable session/event log, replay, and closeout, but no SQLite-backed session
history or local code index is present in the current package.

The security posture is deliberately ref-oriented. Blueprint lookup narrows
the allowed tool menu, unsupported scopes are omitted, denied scopes are
recorded, and direct external effects require a proposal/acceptance boundary.
This is a good basis for remote dispatch, but it is not yet Omega's
interactive, command-level approval system.

## Omega Agent today

Omega Agent is Omega's native in-process GPUI library, not a standalone
service. `NativeAgent` coordinates ACP-facing sessions and project context;
`Thread` owns the conversation and tool/model loop; `ThreadStore` and
`ThreadsDatabase` persist data in SQLite. The UI remains outside the engine,
but binds it to the Agent Panel, Threads Sidebar, editor/project services,
terminal, review, and ACP events.

On each turn, `Thread::run_turn_internal` compacts when necessary, assembles a
request from project context/templates/history/tools, streams model output,
executes tool calls, records results, and continues until completion,
cancellation, steering, or failure. Context includes relevant project/global
instruction files, skills, mentions, workspace context, selected model/profile,
and MCP tools. Tool output is bounded; full output can be retained and read
later through versioned tool-result artifacts.

Omega's built-in surface is broad: read/write/edit and path operations;
regex/search and file discovery; terminal execution; web/fetch; diagnostics;
go-to-definition, references, rename, and code actions; skills; MCP tools;
thread resume/fork; and subagent/transcript tools. Search and navigation use
Omega's loaded `Project` and language services, so they can respect project
roots, exclusions, private-file configuration, and editor semantic state.

Its state is durable and user-facing. The append-only `ThreadEventLog` records
message and prompt-cache history with branch/restore support. SQLite snapshots
and event rows persist threads, tool results, settings, summaries, and
subagent context. This makes recovery, replay, history selection, and UI
continuity first-class concerns rather than add-ons.

Omega also has materially stronger interaction safety: profile/provider and
workspace gating determine visibility; permission precedence yields allow,
confirm, or deny; terminal chains are parsed rather than trusted as strings;
destructive Git receives special protection; and supported terminal sandboxing
limits filesystem/network authority. MCP tools are incorporated into request
construction under the same host control.

## Direct comparison

| Dimension | Probe | Omega Agent |
| --- | --- | --- |
| Primary purpose | Portable first-party OpenAgents runtime for bounded, policy-routed coding assignments. | Full interactive in-editor coding agent for Omega users. |
| Host architecture | Bun/Effect CLI/backend runtime; designed for local/remote sandbox hosts and OpenAgents receipts. | Rust GPUI library embedded in Omega; ACP and UI bindings are native concerns. |
| Main loop | Provider/backend completion and tool dispatch; current concrete chat is lightweight. | Durable streaming turn loop with compaction, steering, retry, tool rounds, and UI events. |
| Retrieval | Current lexical `rg` search only; no index or semantic graph. | Project search plus language-service navigation, diagnostics, and code actions. |
| Tools | Small local CLI set plus Blueprint-selected backend menu; no real terminal or delegation currently. | Broad filesystem, terminal, editor-semantic, MCP, artifact, and delegation tools. |
| Persistence | Contracts, receipts, telemetry, and closeout artifacts; durable sessions are a stated future direction. | SQLite snapshots plus append-only event log, thread branches, forks, and replay. |
| Policy | Assignment/Blueprint capability narrowing and redacted evidence; local approval is incomplete. | Per-tool/command permissions, sandbox grants, project trust, profile/provider filters. |
| Deployment | Intended to run in local or remote approved sandboxes and report to OpenAgents. | Runs inside Omega and uses its project/editor/terminal host services. |

### Latency and cost implications

Probe's current lexical path can be very low latency for one narrow search and
requires no warm-up, index disk space, or embedding/model cost. It becomes
less efficient for exploratory work because each query launches ripgrep,
returns line-oriented text, and makes the model drive repeated search/read
round trips. Remote provider latency, auth materialization, backend health,
and event/receipt handling dominate its end-to-end costs in its intended
deployment model.

Omega has a higher resident footprint: editor/project state, language services,
SQLite state, UI synchronization, and broader policy machinery. In return, it
avoids reconstructing context on every action, can reuse prompt-cache layouts,
and gives the agent richer semantic queries and a durable transcript. Its main
variable cost remains language-model tokens and tool rounds; it controls this
with compaction, tool-result truncation/artifacts, and cache-aware request
assembly.

An index should not be introduced solely because it sounds more semantic.
Measure it against a baseline of `rg` + read + Omega LSP. It earns its cost
only if it reduces time-to-first-relevant-evidence and model/tool rounds on
large, cold, or cross-language repositories without creating stale or
unexplainable answers.

## Roadmap: Probe as an Omega specialized analysis subagent

### Product boundary

Make the integration an *analysis worker*, not a second unrestricted coding
agent. Its initial allowed outcomes should be repository maps, symbol traces,
candidate files, dependency/impact summaries, and evidence references. It
must not edit files, execute arbitrary shell commands, independently delegate,
or change Omega's approval decision. The parent Omega thread remains the sole
interactive actor and decides whether to read, edit, test, or delegate further.

This boundary uses Probe's policy-selected tool-menu and evidence strengths,
while avoiding a confusing duplicate of Omega's mature editor/terminal loop.

### Transport and lifecycle

Omega's `delegate` (`spawn_agent`) facility chooses either the inherited native
loop or a named external ACP executor and rejects silent substitution. It does
not currently expose an arbitrary in-process service call as a delegate.
Therefore the clean integration path is:

1. Build a Probe analysis executable that speaks ACP, registers as an external
   executor (for example `probe-analysis-acp`), and reports a coherent executor
   disclosure. Do not overload the generic `probe` CLI chat protocol.
2. Let Omega's existing external-subagent creation/resume/transcript flow own
   lifecycle, cancellation, UI events, attribution, and parent/child linkage.
   The parent calls `delegate` with the installed executor name and receives a
   normal subagent result/transcript.
3. Pass a bounded assignment through ACP metadata or a versioned request
   envelope: workspace/worktree identity, repository revision, task,
   allowlisted paths, evidence/output budget, deadline, and analysis mode.
   No bearer credentials or broad sandbox grant belongs in it.
4. Return a versioned structured result plus concise Markdown: file and symbol
   identifiers, source ranges, relationship edges, confidence, commands or
   queries used, repository revision, index revision/freshness (when indexed),
   omissions, and errors. Store bulky maps as an artifact and give the parent a
   stable reference.

ACP is preferable to a new bespoke direct delegate interface because Omega
already models external ACP agents, sessions, lifecycle events, transcripts,
and executor disclosure. A later native integration can share the same
envelope/result schema, but should not precede a working external boundary.

### Phased capability plan

**Phase 0 — baseline and contract.** Define `RepositoryAnalysisRequest` and
`RepositoryAnalysisResult`; implement four read-only operations using current
Probe primitives: `repo_map`, `text_search`, `trace_symbol`, and
`impact_summary`. Use ripgrep plus bounded reads, return exact evidence, and
benchmark against Omega `grep`, definition, and references. Reject paths
outside the assigned worktree and cap bytes, files, tool calls, and wall time.

**Phase 1 — structural adapter.** Add language adapters that emit a normalized
symbol graph from parsers/LSP, with source ranges and an explicit `partial`
status for unsupported languages. Consume Omega's project/LSP facts where the
ACP boundary can safely carry them; otherwise rebuild only enough read-only
state in Probe. Do not claim semantic completeness from text search.

**Phase 2 — incremental repository index.** Add a content-addressed index
keyed by repository identity, commit/worktree generation, file digest, parser
version, and configuration. Update changed files incrementally; invalidate on
worktree/config/language changes; namespace it by workspace; never reuse it
across revisions without checking digests. Search ranking must return evidence
and index freshness so the parent can detect stale results.

**Phase 3 — cost-aware routing.** In Omega, expose Probe Analysis as a named
delegate only for broad mapping, cross-cutting traces, or repositories whose
measured index hit makes it cheaper than normal tools. Keep small direct
lookups in Omega. Capture elapsed time, files scanned, cache/index hit rate,
result precision judged by follow-up opens, and model/tool-round reduction.

**Phase 4 — trusted remote operation.** Only after the read-only version is
reliable, use Probe's assignment, runner identity, redacted receipt, and
scoped-auth mechanisms for remote execution. Preserve Omega's local permission
and review authority; remote completion is evidence, not acceptance or a
writeback authorization.

### Acceptance criteria

- A delegated Probe session is explicitly identified as Probe Analysis and can
  be cancelled, resumed, and read through Omega's ordinary subagent UI.
- Every answer cites exact paths/ranges and the repository plus index revision.
- Analysis cannot mutate the worktree or open a terminal with broader authority.
- Stale/missing/partial index state is visible rather than silently hidden.
- On representative large-repository investigations, the route demonstrates a
  measurable reduction in time or model/tool rounds without reduced evidence
  accuracy versus Omega's native search/navigation baseline.

## Recommendation

Use Omega Agent as the primary interactive coding surface. Evolve Probe into a
portable, bounded execution and analysis runtime whose initial Omega role is a
read-only ACP specialist. Preserve its assignment/evidence/remote-host design,
but do not duplicate Omega's editor, terminal, persistence, or permission
systems. Introduce structural and indexed retrieval only behind an observable,
revision-safe analysis contract and only after the lexical and LSP baselines
show a clear gap.
