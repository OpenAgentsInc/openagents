# Claude Subagents: History Format, Implicit Topology, and Orchestration Evolution

- Date: 2026-07-10
- Subject: how Claude Code creates, persists, links, controls, and renders
  subagents; how that model changed across recent Claude Code versions; and
  what OpenAgents should preserve or improve
- Compared with:
  [`Codex subagent rendering analysis`](./2026-07-10-codex-subagents-rendering-analysis.md)
  and the
  [`Claude Desktop teardown`](./2026-07-10-claude-desktop-app-teardown.md)
- Status: point-in-time read-only analysis, not a protocol guarantee

## Method and privacy boundary

This analysis used three local evidence sources:

1. a structural scan of the current machine's `~/.claude` history tree;
2. names-only/static-string inspection of the installed Claude Code 2.1.206
   executable; and
3. read-only `claude --help` / `claude agents --help` output.

Evidence tags:

- **`[history]`** — derived from JSON field names, record types, timestamps,
  versions, identifiers, and link equality in the local history corpus;
- **`[binary]`** — present in the installed Claude Code executable's static
  strings or schemas;
- **`[cli]`** — exposed by current command help;
- **`[inferred]`** — reasoned from those observations rather than directly
  asserted by the closed runtime; and
- **`[comparison]`** — contrast with the separately sourced Codex analysis.

The scan deliberately did **not** copy or publish prompts, assistant text,
thinking, command lines, tool arguments/results, repository names, project
paths, credentials, account identifiers, or generated code. Aggregation used
record shapes and equality tests only. The resulting document contains no raw
conversation payload.

The inspected `~/.claude` directory was 2.7 GB. It contained 2,289 JSONL files:
2,288 under `projects/` plus the top-level prompt-history file. The project
corpus contained:

| File class | Files | Parsed records |
| --- | ---: | ---: |
| Parent session histories | 369 | 240,999 |
| Ordinary subagent histories | 1,870 | 386,769 |
| Workflow-agent histories | 47 | 3,630 |
| Workflow journals | 2 | 94 |
| **Total** | **2,288** | **631,492** |

All 631,492 project records decoded as JSON. The timestamp range was
2026-06-09 through 2026-07-11 UTC, spanning Claude Code versions 2.1.170 through
2.1.206. This is a dense one-month behavioral sample, not a complete history of
Claude Code or evidence that every feature was generally available.

---

## 1. Executive summary

- **A Claude subagent is a sidechain transcript inside a parent session
  namespace.** Ordinary children persist at the structural path
  `<project>/<sessionId>/subagents/agent-<agentId>.jsonl`. Every one of the
  1,870 ordinary files carried exactly one stable `agentId`, exactly one
  `sessionId` matching its containing directory, and `isSidechain: true`; the
  filename's id matched the record id in every file. `[history]`

- **The child transcript is rich and independent.** Ordinary child histories
  contained 243,712 assistant records, 139,294 user records, 41,450 thinking
  blocks, 65,455 assistant text blocks, 136,824 tool-use blocks, and 136,821
  tool-result blocks. Claude does not merely retain a final child summary; it
  retains nearly the whole agent loop as its own JSONL chain. `[history]`

- **Parent/child topology is implicit, not stored in `parentUuid`.** The first
  record of every ordinary and workflow child had `parentUuid: null`.
  Subsequent `parentUuid` values overwhelmingly pointed to an earlier record in
  the same child file. The agent edge instead comes from the invoking
  `Agent` tool call and its parent's structured `toolUseResult.agentId`.
  `[history]`

- **The implicit graph is reconstructable, but not lossless.** Structured
  `Agent` results linked 1,819 of 1,870 ordinary child files (97.27%). Starting
  at parent sessions, 1,808 children (96.68%) were reachable. The recovered
  trees contained 1,457 depth-one children, 317 depth-two children, 32
  depth-three children, and 2 depth-four children, with no cycle or
  multi-parent edge. Fifty-one ordinary child files lacked a structured edge
  in the retained parent/ancestor result and must be represented as topology
  gaps rather than silently hidden. `[history]`

- **Claude's orchestration evolved in layers without replacing the sidechain
  record.** The corpus begins with synchronous `Agent` results carrying final
  content, usage, duration, and tool statistics. Version 2.1.173 adds explicit
  background launch and output-file state. By 2.1.197, omitting
  `run_in_background` consistently launches asynchronously in this corpus;
  current 2.1.206 instructions state that background is the default and
  `run_in_background: false` requests synchronous completion. Versions 2.1.186
  and later add mailbox/task coordination, while 2.1.195 introduces scripted
  multi-agent Workflows with their own journal. `[history + binary]`

- **The normal parent view is a lossy projection over richer disk history.** A
  foreground child returns one result to the parent. A background child returns
  an id/output-file descriptor, later injecting task-notification/usage markup
  into the parent. The complete child thinking, tools, and intermediate text
  remain in the sidechain file rather than appearing inline in the parent.
  Current binary strings explicitly filter `isSidechain=true` histories from
  the normal `/resume` picker. `[history + binary]`

- **Claude and Codex solve topology differently.** Codex records child threads
  and lifecycle in an explicit graph store plus collaboration events. Claude's
  inspected history uses per-agent JSONL sidechains, structured `Agent`
  results, task records, notifications, and workflow journals. Claude's format
  is locally inspectable and content-rich, but a durable GUI tree requires a
  reconstruction adapter and explicit gap accounting. `[comparison]`

---

## 2. The persistent history model

### 2.1 Parent sessions and child sidechains

The ordinary layout is:

```text
~/.claude/projects/<encoded-project>/
├── <sessionId>.jsonl
└── <sessionId>/
    └── subagents/
        ├── agent-<agentId>.jsonl
        ├── agent-<agentId>.jsonl
        └── ...
```

The parent and every ordinary child share the same `sessionId`. The child's
`agentId` is its local identity and its records set `isSidechain: true`; parent
conversation records set `isSidechain: false`. Child files do not get a second
session id. They are parallel histories under the same session namespace.
`[history]`

This has an important semantic consequence: **session identity does not imply
conversation identity**. A client that groups only by `sessionId` will merge
the parent and all children. A client that reads only `<sessionId>.jsonl` will
drop every child. Correct reconstruction needs the pair `(sessionId, agentId)`,
with a distinguished root for the parent.

### 2.2 Each JSONL is its own causal chain

Message-like records carry `uuid` and `parentUuid`. Across ordinary child
histories:

- 1,879 records had a null parent (the 1,870 file roots plus a small number of
  explicit local resets);
- 384,881 non-null parents matched an earlier UUID in the same child file; and
- only 9 were missing or cross-file from the bounded scan.

Every child's first record had `parentUuid: null`. Therefore `parentUuid` is
the causal/message chain **inside that transcript**, not the agent-parent edge.
The same pattern holds for workflow children. `[history]`

Fields such as `sourceToolAssistantUUID`, `sourceToolUseID`, `promptId`, and
top-level `toolUseResult` help connect a tool result to the tool invocation and
turn that produced it. In this corpus, `promptId` was not an agent-topology key.
The stable cross-file join was `toolUseResult.agentId` to the child filename/id.

### 2.3 Child records preserve the actual loop

The 386,769 ordinary child records break down as:

| Record/content class | Count |
| --- | ---: |
| Assistant records | 243,712 |
| User records | 139,294 |
| Attachment records | 3,763 |
| Assistant tool-use blocks | 136,824 |
| User tool-result blocks | 136,821 |
| Assistant text blocks | 65,455 |
| Assistant thinking blocks | 41,450 |

Tool use and tool result are almost one-for-one because each subagent history
stores the same agent loop shape as a parent: assistant tool request, user-role
tool result, next assistant continuation. The format also carries model
response ids, request ids, usage, errors, attachments, attribution, version,
working directory, and Git branch where applicable. `[history]`

This is valuable historical evidence and highly sensitive private data. A
renderer should consume a typed, redacted projection. It should never upload or
expose raw child JSONL merely because the file is local.

### 2.4 The top-level prompt history is not the transcript archive

`~/.claude/history.jsonl` is separate from the 2,288 project JSONLs. It is the
interactive prompt-history surface, not the authoritative parent/child event
tree. Reconstructing subagents from it would lose tool linkage, child identity,
thinking, status, and results. `[history]`

---

## 3. How an ordinary subagent is created

### 3.1 Creation is a normal `Agent` tool call

The parent assistant emits a `tool_use` block named `Agent`. Observed input
shapes combine:

- `description`;
- `prompt`;
- `subagent_type`;
- optional `run_in_background`;
- optional `isolation`;
- optional `model`; and
- optional `effort`.

Current binary instructions add that agent definitions can supply model,
reasoning effort, and tools; a per-call model can override the definition.
Current CLI also accepts `--agent` for the current session and `--agents <json>`
for custom definitions. `[history + binary + cli]`

The runtime creates a new `agentId`, starts the sidechain JSONL, and places the
delegated prompt in the first user record. That first record is a new local
root (`parentUuid: null`), not a continuation of the parent's message UUID
chain. The child subsequently runs a normal assistant/thinking/tool loop.
`[history]`

### 3.2 Context inheritance is exceptional, not ambient

Current binary instructions distinguish fresh agents from a `fork` subtype:

- ordinary fresh agents begin without the parent's conversation context and
  require a self-contained prompt;
- a fork may inherit context;
- a fork cannot recursively fork again; and
- the runtime enforces a subagent nesting cap.

The history scan proves nesting and the input fields, but not the contents of
the prompts or whether a particular child used the fork subtype. The binary's
policy distinction is nevertheless important: a sibling child should not be
assumed to know the parent conversation merely because both share a session id.
`[binary]`

### 3.3 Foreground completion shape

The original/foreground result is stored on the parent tool-result record as a
structured `toolUseResult` with fields such as:

```text
agentId
agentType
status = completed
content
resolvedModel
usage
totalTokens
totalDurationMs
totalToolUseCount
toolStats
```

Worktree-isolated foreground results may additionally include a worktree path
and branch. This document records only the field names; their values are
private. `[history]`

The result is a **summary/return value**, not the child transcript. The child
JSONL remains the only observed persistent source for its intermediate
thinking, text, tool requests, and tool results.

### 3.4 Background launch shape

The asynchronous result is smaller and lifecycle-oriented:

```text
agentId
status = async_launched
isAsync = true
description
resolvedModel
outputFile
canReadOutputFile
```

The output file is an incremental progress/result channel managed by Claude
Code. Current binary symbols name initialization, append, flush, delta read,
size, symlink repointing, eviction, and cleanup operations for task output.
`TaskOutput` and `Monitor` expose bounded ways to observe it. `[history +
binary]`

The parent later receives injected user-role strings containing task
notification and usage markup. The structural scan found 1,511 task-
notification records and 960 usage-marked records in parent sessions. Child
histories also received notifications when nested children completed.
`[history]`

### 3.5 Explicit isolation

Observed `Agent` calls can request isolation. Current binary help defines two
important modes:

- `worktree` creates a temporary Git worktree and can return its path/branch;
- `remote` launches in a remote cloud environment and is always background.

The corpus included nine completed foreground results with worktree metadata.
No raw path or branch value was retained in this analysis. `[history + binary]`

Isolation is orthogonal to topology. A child remains a sidechain with an
`agentId` whether it runs in the current workspace, a worktree, or a remote
environment.

---

## 4. Lifecycle and control are distributed across several records

Claude's inspected history does not expose one Codex-like canonical
`AgentStatus` enum on every child. Lifecycle is assembled from several planes.

### 4.1 `Agent` result status

The corpus observed two normal result states:

- `completed` for synchronous final results; and
- `async_launched` for background/output-file results.

Current binary strings additionally include remote launch and unexpected-result
handling. Error/denial cases can produce string or absent structured results,
which accounts for some topology gaps. `[history + binary]`

### 4.2 Task/output control

The surrounding tool vocabulary includes:

- `TaskStop` — stop a background task by id;
- `TaskOutput` — retrieve/wait on a task output;
- `Monitor` — stream or inspect background command/task state;
- `SendMessage` — deliver mailbox messages to named/id-addressed agents;
- `TaskCreate`, `TaskUpdate`, `TaskList`, and `TaskGet` — maintain a separate
  dependency/status ledger.

The local `~/.claude/tasks` store contained 58 JSON task records, all with the
shape `id`, `subject`, `description`, `status`, `blockedBy`, and `blocks`; 54
were completed, 3 in progress, and 1 pending at inspection time. Task records
are coordination state, not substitutes for the child transcript or the
parent/child edge. `[history]`

### 4.3 Completion notification

A background child does not need the parent to poll continuously. Current
instructions state that completion re-invokes/notifies the parent. The parent
history records that return as a notification/usage message. The complete
child result remains addressable by the agent/task id and output path, while
the parent decides how to summarize it for the user. `[history + binary]`

### 4.4 Session shutdown and interruption

The corpus contains stop-hook summaries, explicit task-stop calls, API error
records, interrupted-message fields, and one `agents_killed` system record.
Current binary strings also carry depth-cap, permission-denied, unavailable
agent type, missing MCP requirement, background-denied, and recursive-fork
diagnostics. `[history + binary]`

This is a richer failure model than a simple completed/running badge, but it is
not normalized in the retained JSONL into one child lifecycle record. A client
must reduce these inputs carefully and disclose unknown state.

---

## 5. Reconstructing the agent tree

### 5.1 The edge equation

For an ordinary child, the strongest observed edge is:

```text
invoking transcript
  assistant.tool_use(name = Agent, id = toolUseId)
  -> user.tool_result(tool_use_id = toolUseId)
  -> top-level toolUseResult.agentId
  -> subagents/agent-<agentId>.jsonl
```

If the invoking transcript is the parent session, the child is depth one. If
the invoking transcript is itself `agent-<parentAgentId>.jsonl`, the child is
nested beneath that agent. `sourceToolAssistantUUID` ties a tool-result record
back to the assistant record inside the invoking transcript; it does not replace
the `agentId` edge. `[history]`

### 5.2 Recovered topology

The scan recovered these root-reachable depths:

```text
root session
├── depth 1: 1,457 agents
│   └── depth 2: 317 agents
│       └── depth 3: 32 agents
│           └── depth 4: 2 agents
```

There were no cycles and no child with more than one recovered parent. Child
histories contained 407 `Agent` tool calls, proving that nested delegation is a
real behavior rather than a theoretical schema capability. `[history]`

Current binary strings enforce a numeric depth cap and special restrictions:
forked workers cannot recursively fork, and in-process teammates cannot launch
background children. The exact configured limit is not asserted here; the
observed corpus maximum was four. `[binary + history]`

### 5.3 Completeness accounting

Of 1,870 ordinary child files:

- 1,819 had a recovered structured incoming edge (97.27%);
- 1,808 were reachable from a retained root-session edge (96.68%); and
- 51 had no retained structured incoming edge.

The missing group spans several versions. Common corresponding parent result
shapes were a plain string or lacked top-level `toolUseResult`, consistent with
legacy/error/denial records that retained a child file without a normal
structured result. Eleven children lived in components whose ancestor edge was
missing, explaining the difference between edge coverage and root reachability.
`[history + inferred]`

A truthful renderer therefore needs this equation:

```text
child files = linked tree nodes + explicit orphan/gap nodes
```

It must not claim a complete hierarchy merely because most ids joined.

### 5.4 No observed canonical graph store

The inspected filesystem had no separate Claude equivalent of Codex's
`agent-graph-store`. The tree was reconstructed from JSONL tool edges, task
state, and workflow journals. Because Claude Code is closed, this is an
observation about the local persistence surface, not proof that the runtime has
no in-memory or private graph representation. `[history + inferred]`

---

## 6. Four orchestration layers grew over the same sidechain primitive

### 6.1 Layer one — ordinary `Agent` delegation

The earliest inspected versions already had:

- typed `Agent` creation;
- custom agent type selection;
- independent child JSONL;
- nested child creation;
- tool/usage statistics on synchronous completion; and
- task stop/monitor primitives.

This is classic parent-to-child delegation. Each child returns one result to
the invoker.

### 6.2 Layer two — background agents plus mailbox/task coordination

Version 2.1.173 introduces explicit background launch and
`pendingBackgroundAgentCount`. Version 2.1.186 introduces `SendMessage` in the
observed histories. `TaskCreate`/`TaskUpdate` existed earlier and later acquire
more dependency/metadata fields. Together they support a team-like pattern:

```text
root or child
  -> launch agent(s)
  -> record shared tasks/dependencies
  -> send mailbox messages
  -> receive notifications
  -> stop/read output as needed
```

The corpus contained 160 root `SendMessage` calls and 20 child `SendMessage`
calls. Current binary instructions refer to active teammates by name, while an
agent id can address/resume completed background work. However, this corpus did
not contain observed `Agent` inputs with `name` or `mode`, so the document does
not infer a stable persisted team schema from those strings alone. `[history +
binary]`

### 6.3 Layer three — scripted Workflows

The filesystem contains a second child layout:

```text
<sessionId>/subagents/workflows/wf_<runId>/
├── journal.jsonl
├── agent-<agentId>.jsonl
└── ...
```

Two observed Workflow runs produced 47 child histories:

- a Claude Code 2.1.195 script-based run with 24 agents; and
- a Claude Code 2.1.198 named/argument-based run with 23 agents.

Each journal contained one `started` and one `result` entry per agent. Workflow
children retained the same `isSidechain: true`, stable `agentId`, assistant/
user/tool loop as ordinary agents, but their topology is journal/run-key based
rather than an ordinary `Agent` result edge. Workflow children used a required
`StructuredOutput` tool to return schema-shaped results. `[history]`

Current binary strings describe Workflows as deterministic session-persisted
scripts. They can launch many `agent()` calls, cache completed unchanged calls,
resume a run, enforce call/budget caps, and require reapproval when script
content changes. The feature is plan/setting gated. `[binary]`

This is a material evolution: Claude moved from model-chosen delegation to a
replayable programmable orchestration layer without changing the underlying
child transcript format.

### 6.4 Layer four — top-level background sessions

Current Claude Code 2.1.206 exposes `claude --background` and `claude agents`.
The latter manages independent background sessions and can emit a JSON list for
scripting. These are top-level Claude sessions, not necessarily
`isSidechain:true` children of the current conversation. The local agent view
was empty during inspection, so this analysis records the CLI surface but makes
no persistence claim for it. `[cli]`

---

## 7. Change timeline from the retained histories

### 2.1.170 — synchronous sidechains and task control

First observed 2026-06-09/10:

- ordinary `Agent(description, prompt, subagent_type)`;
- child `agentId`/`isSidechain` JSONLs;
- final `completed` result with usage, duration, tool count, and tool stats;
- nested Agent calls;
- `TaskCreate`, `TaskUpdate`, `TaskStop`, and `Monitor`;
- `stop_hook_summary` and turn-duration system records; and
- `attributionAgent` on child records.

### 2.1.173 — explicit asynchronous agents

First observed 2026-06-11:

- `run_in_background` input;
- `async_launched` result with output-file/readability fields;
- background completion notification; and
- `pendingBackgroundAgentCount` in selected records.

### 2.1.177 — isolation and model choice

First observed 2026-06-14:

- `isolation` input;
- per-call `model` override; and
- worktree path/branch on completed isolated results.

### 2.1.181 — effort choice

First observed 2026-06-19:

- per-call `effort` input.

### 2.1.186 — mailbox/team behavior

First observed 2026-06-23/24:

- `SendMessage` between root/agents; and
- an `agents_killed` system record.

### 2.1.195 — Workflows and the async-default transition

First observed 2026-06-27:

- `Workflow` with a persisted script;
- workflow run/task/transcript metadata;
- per-workflow agent journals;
- required `StructuredOutput` results; and
- `pendingWorkflowCount`.

Version 2.1.195 contains both synchronous and asynchronous results when
`run_in_background` is omitted. From 2.1.196 onward in this corpus, every
normally structured omitted-field result was asynchronous. Explicit `false`
calls in July returned synchronously. Current 2.1.206 instructions confirm
background as the default. `[history + binary]`

### 2.1.198–2.1.201 — named workflows and stronger control records

- 2.1.198: named Workflow plus args and additional structured result shapes;
- 2.1.199: `TaskGet`;
- 2.1.200: `TaskOutput`;
- 2.1.201: task metadata and explicit tool-denial metadata.

### 2.1.206 — independent background-agent CLI

Current help exposes top-level background start and the `claude agents`
manager, plus custom agent definitions, explicit agent choice, effort, model,
permissions, plugins, MCP, and allowed directories for dispatched sessions.
`[cli]`

---

## 8. What Claude Code renders versus what it retains

### 8.1 Parent transcript

The parent transcript retains the `Agent` tool invocation and one of:

- a complete foreground result summary; or
- a background launch descriptor followed by task-notification/usage messages.

That is enough to tell the parent model that work exists and later finished.
It is not the child's complete transcript.

### 8.2 Child transcript

The child JSONL retains thinking, text, tool calls, results, errors, usage, and
its own nested launches. Those items do not automatically become ordinary rows
in the parent scrollback. The model/user sees a returned summary or must inspect
the sidechain/output separately. `[history]`

### 8.3 Historical discoverability gap

Current binary strings explicitly describe `isSidechain=true` histories as
filtered from `/resume`. The standard conversation picker therefore protects
users from thousands of internal children, but it also removes the obvious UI
path to completed child history. `[binary]`

The result resembles the Codex TUI gap for a different reason:

- Codex has an explicit graph and rich shared protocol, then flattens it in the
  terminal.
- Claude persists rich child files but leaves the graph implicit and normally
  shows only parent tool/notification projections.

In both systems, **the raw evidence is richer than the linear parent view**.

### 8.4 Workflow rendering pressure

A Workflow can create dozens of children. Rendering all 47 child transcripts
inline would overwhelm a parent conversation; rendering only one workflow
summary hides individual errors and evidence. The correct desktop shape is a
hierarchy:

```text
session
├── ordinary agents
│   └── nested agents
└── workflow run
    ├── workflow journal item
    └── workflow agents
```

The parent timeline should show bounded lifecycle/result cards. Selecting any
node should open its full authorized transcript and item details. Orphan files,
missing results, and unsupported versions must appear as counted gaps.

---

## 9. Claude versus Codex

| Concern | Codex | Claude Code history |
| --- | --- | --- |
| Child identity | Separate thread id plus path/nickname/role | Same session id plus stable `agentId`, `isSidechain:true` |
| Parent edge | Persisted spawn source and graph store | Reconstructed from invoking `Agent` result's `agentId` |
| Child causal chain | Thread event/item history | Per-child `uuid`/`parentUuid` JSONL chain |
| Lifecycle | Explicit status enum and collaboration events | Result status + task/output/notification/error records |
| Nested graph | Explicit graph traversal | Implicit; reconstructed to depth four here |
| UI projection | Shared reduced `ThreadItem` protocol | Parent tool result/notifications plus separate child file |
| History picker | Can switch active agent in TUI | Sidechains filtered from normal `/resume` picker |
| Batch orchestration | Multi-agent collaboration tools | Scripted Workflow run + journal + structured child outputs |
| Completeness risk | UI truncation despite explicit graph | Missing/unstructured edge despite rich child transcript |

Codex's graph is the stronger topology contract. Claude's sidechain JSONL is a
strong, simple audit artifact and its Workflow journal is a useful programmable
orchestration pattern. OpenAgents should combine those strengths rather than
copy either persistence format literally.

---

## 10. Implications for OpenAgents Desktop/mobile

### 10.1 Provider import needs a graph adapter, not a message parser

A Claude history adapter should:

1. discover parent sessions and sidechain/workflow child files;
2. key nodes by provider/session/agent identity;
3. reduce every supported child line into one typed item, explicit redaction,
   or explicit gap;
4. reconstruct ordinary edges through `Agent` tool/result identity;
5. reconstruct workflow edges through run/journal identity;
6. preserve per-file causal order without inventing a total order across
   concurrent children; and
7. report orphan, corrupt, missing-parent, unsupported-version, and unloaded
   counts.

### 10.2 Do not use `parentUuid` as the agent-parent edge

That field names the local message chain. Treating it as cross-agent topology
would make every child appear rootless or attach it to the wrong item. The
provider adapter needs distinct concepts:

- `parentAgentRef`;
- `sourceToolUseRef` / `delegationRef`;
- `parentItemRef`; and
- child-local `parentItemRef` for its own message chain.

### 10.3 Normalize lifecycle without erasing uncertainty

OpenAgents can map Claude observations into provider-neutral states such as
pending, running, blocked, completed, failed, interrupted, stopped, and
unknown. But `async_launched` is an accepted launch, not running proof or
completion. A notification is not verification. A missing structured result
is a gap, not failure or success.

### 10.4 Separate execution evidence from display summaries

Claude's foreground `content` and background notification are useful summaries.
The child tool history, usage, worktree result, verification, and artifacts are
the evidence. OpenAgents should attach summaries to exact child/run refs and
never promote prose into outcome authority.

### 10.5 Preserve explicit context provenance

Claude's fresh-agent/fork distinction validates the Episode 195 follow-up:
context inheritance must be explicit. A provider-neutral delegation record
should say which parent message/file/artifact refs were selected, whether a
provider fork inherited context, and which grants/tools/model/effort/isolation
were effective.

### 10.6 Render one tree on both clients at different density

Desktop can retain the planned three-pane view: conversations, selected
parent/child transcript, and Agents/Item inspector. Mobile should use a drawer
or drill-down tree with the same nodes, statuses, gaps, and receipts. Neither
client should reduce children to “currently running” only; completed and
orphaned children remain discoverable.

### 10.7 Keep raw provider history private

Claude sidechains can contain thinking, private prompts, full tool arguments,
command output, paths, repository content, attachments, and provider metadata.
They stay owner-local by default. Khala Sync receives only explicitly allowed
typed projections and canonical OpenAgents conversation/run state, never a raw
`~/.claude` mirror.

### 10.8 Adopt Workflow ideas only through existing authority

The useful Workflow ideas are deterministic scripts, bounded agent calls,
structured results, resumability, content pinning, and a per-agent journal.
OpenAgents should express those through its existing typed action, FleetRun,
claim, work-unit, attempt, policy, budget, and receipt contracts. It should not
create a provider-specific Workflow run universe.

---

## 11. Recommended acceptance corpus

The Claude adapter should be tested against a public-safe synthetic corpus
covering:

1. a synchronous depth-one child with thinking, text, tools, usage, and final
   result;
2. a background child with launch descriptor, notification, output, and final
   state;
3. simultaneous siblings whose events overlap in time;
4. a child spawning a child and a depth-three/depth-four tree;
5. explicit worktree isolation metadata;
6. task stop/interruption and API error;
7. mailbox/task-ledger coordination;
8. one Workflow with journaled structured children;
9. a missing/unstructured `toolUseResult` with an orphan child file;
10. an unsupported version and corrupt/truncated line;
11. large histories with paging and no fixed child/message cap; and
12. privacy tripwires for prompts, thinking, command output, credentials,
    absolute paths, and repository contents.

The completeness equation should match the Codex importer:

```text
authorized source items = rendered items + explicit redactions + explicit gaps
discovered child files = linked nodes + explicit orphan/gap nodes
```

No source line or child file disappears silently.

---

## 12. Key observed fields and surfaces

| Concern | Observed history/binary surface |
| --- | --- |
| Parent history | `projects/<project>/<sessionId>.jsonl` |
| Ordinary child | `<sessionId>/subagents/agent-<agentId>.jsonl` |
| Workflow child | `<sessionId>/subagents/workflows/wf_<runId>/agent-<agentId>.jsonl` |
| Workflow lifecycle | `journal.jsonl` with `started` / `result` keyed by `agentId` |
| Child identity | `sessionId`, `agentId`, `isSidechain`, `entrypoint`, `version` |
| Local item chain | `uuid`, `parentUuid`, `sourceToolAssistantUUID`, `sourceToolUseID` |
| Spawn input | `Agent`: description, prompt, type, background, isolation, model, effort |
| Foreground result | completed content + model/usage/duration/tool stats |
| Background result | async id + output file/readability + resolved model/status |
| Coordination | task CRUD/list/get, mailbox send, stop/output/monitor |
| Current top-level agent manager | `claude --background`, `claude agents` |
| Normal history filtering | binary explicitly filters `isSidechain=true` from `/resume` |

## 13. Addendum: historical implementation mechanics

After the history-first analysis above was complete, I inspected the historical
`cc` reference snapshot at commit
`813c06acfa2d705076df6193b405c81eb11a18d1` (import commit dated 2026-03-31).
This addendum translates the implementation into behavioral findings; it does
not reproduce source, private identifiers, or internal file paths.

### 13.1 Provenance caveat

The snapshot cannot be assigned a trustworthy upstream Claude Code version.
Its build version is injected outside the checked-in source, and the import
commit date is not a product-version boundary. It also contains mechanisms
that resemble the later behaviors observed in the June/July histories,
including background agent output, forked context, workflows, worktrees, and a
top-level agent manager. It may therefore be a mixed or later source snapshot
under older repository metadata.

The retained `~/.claude` histories remain the authority for the chronology and
counts in sections 1-8. The source snapshot is useful for explaining mechanics
behind those records, not for moving a feature earlier in the timeline.

### 13.2 Ordinary dispatch is in-process state isolation

The ordinary `Agent` path runs another query loop inside the existing process.
The tool-facing prompt calls these agents subprocesses, but the implementation
constructs an isolated child context rather than starting one OS process per
ordinary child. That distinction matters for monitoring and recovery: a
sidechain file represents a logical agent, not proof of a separate process.

At dispatch, the runtime creates a stable child agent identity, records the
immediate invoking request when available, labels the invocation as spawn or
resume, and starts a fresh query-tracking chain with incremented depth. It then
clones or replaces mutable state deliberately:

- file-read/cache state is copied rather than shared blindly;
- denial counters and nested tool-decision state are child-local;
- parent UI mutation callbacks are absent from ordinary background children;
- general application mutation is disabled, while task registration and kill
  still have an explicit channel to the root task store; and
- foreground execution can share selected UI/state and cancellation behavior,
  while background execution receives an independent cancellation controller.

This is a stronger model than “call the model again.” Dispatch is a scoped
runtime allocation with identity, cancellation ownership, state boundaries,
and selected root capabilities.

### 13.3 `Task` became `Agent` through a compatibility layer

The snapshot makes the naming transition explicit. `Agent` is the current tool
name, while `Task` remains a legacy alias for permission rules, hooks, resumed
sessions, and SDK compatibility. One compatibility surface deliberately still
emits `Task` to older SDK consumers even though the internal dispatch path is
`Agent`.

That explains why importers must normalize tool aliases without treating them
as different orchestration primitives. It also reinforces a broader rule:
wire-name migrations need an explicit compatibility/version layer. A raw
`Task` record can be an agent dispatch; a task-ledger record can be something
else entirely.

### 13.4 Context and capabilities are compiled per child

A normal fresh subagent does not inherit the parent's whole message history.
It starts from the delegated prompt plus selected policy-shaped context. Some
read-only built-ins intentionally omit expensive or stale parent context. Tool
availability is assembled again from the chosen agent definition, active MCP
tools, allow/deny rules, required-server readiness, and the effective
permission mode. Required MCP dependencies are checked for usable tools, not
merely configured server names.

Interactive permission handling is also mode-dependent. Background children
normally cannot suspend themselves on an invisible prompt. The fork path can
instead use a permission-bubbling mode that surfaces the decision to the
parent. Start hooks may add context; agent-scoped stop hooks are translated to
the subagent stop lifecycle; and managed trust policy can prevent untrusted
agent definitions from registering hooks.

The gated fork path is materially different from fresh delegation. It clones
the parent conversation and system prompt, preserves the exact tool array and
model/thinking configuration for prompt-cache compatibility, and runs
asynchronously. It inserts placeholder results for sibling tool calls when
necessary so the inherited assistant message remains API-valid, blocks a fork
from recursively forking itself, and adds path-remapping guidance when the fork
uses an isolated worktree.

For OpenAgents, the persisted delegation record should therefore capture both
requested and effective context/tool/policy state. Agent type alone is not a
capability grant.

### 13.5 Persistence is agent-local and resume is reconstruction

Before the child loop starts, the runtime best-effort writes the initial child
messages and metadata such as agent type, description, and worktree. That write
does not block execution. Later messages append to the sidechain with the
child's own `parentUuid` chain.

Resume locates the leaf for the requested `agentId`, rebuilds only that local
chain, restores agent metadata and prompt-cache replacement state, appends the
new prompt, and reuses the original worktree when it still exists. A missing
worktree falls back to the parent's current directory. Older records without
agent-type metadata fall back to a general-purpose definition, and a resumed
agent does not repeat the original agent-type denial gate.

The latter is an important policy question for OpenAgents: resuming an accepted
attempt may preserve its original grant, or current policy may require
reauthorization. Whichever rule is chosen must be versioned and visible. It
must not be an accidental consequence of transcript loading.

### 13.6 Background completion is a staged protocol

An asynchronous launch immediately registers a task and a readable output
surface. In this snapshot, that output surface is backed by the agent's JSONL
transcript rather than a second authoritative result stream. The live loop
updates retained progress and transcript data. On success it marks the task
completed before optional handoff classification and worktree inspection, then
enqueues the enriched final notification. Abort and error take distinct killed
or failed paths and preserve partial/error output where possible.

This explains several otherwise ambiguous history shapes:

```text
accepted launch
  -> running/progress evidence
  -> terminal task state
  -> optional post-processing
  -> addressed completion notification
  -> parent-visible summary/receipt
```

Those are not one atomic event. `async_launched` is not running proof, a
terminal task flag can precede the notification, and a notification is not
verification. OpenAgents should keep acceptance, execution, terminal outcome,
notification delivery, artifact/writeback inspection, and verified receipt as
separate typed facts.

### 13.7 Addressed queues prevent parent/child cross-talk

The shared in-process command queue is filtered by recipient. The root session
drains user prompts and root notifications. A child drains only task
notifications addressed to its own agent identity and does not consume the
general user-prompt stream. Named-agent messaging resolves a name to a stable
identity before delivery.

The runtime also knows more topology than the persisted history exposes. It can
associate a child with its immediate invoking agent/session and emits the
invoking request edge sparsely at spawn or resume. That corroborates the
history finding: runtime provenance exists, but the durable local record still
does not provide one canonical graph table. Import must continue to reconcile
sidechains, tool results, task state, and notifications.

### 13.8 Subagents, teammates, workflows, and sessions are different classes

The implementation separates at least three execution/lifecycle classes:

1. an ordinary in-process delegated subagent;
2. a named teammate, which may run in-process or in a separate terminal/process
   and owns team/task/mailbox state; and
3. an independent foreground/background session managed outside the ordinary
   parent tool call.

In-process teammates are further restricted from launching background
subagents, although they can run synchronous children. Workflow execution uses
the core child runner but adds its own deterministic script and journal
boundary. These distinctions support the layered model in section 6 and warn
against presenting every Claude child-like entity as the same kind of node.

### 13.9 Worktree retention is outcome-sensitive

Worktree isolation is selected explicitly or by agent definition, and the
agent identity participates in worktree ownership. At completion, an unchanged
agent worktree can be removed automatically; a changed worktree is retained
and returned for inspection/writeback. Removed worktree metadata is cleared so
resume does not target a stale path. Existing resumed worktrees have their
liveness refreshed, while missing ones fall back safely as described above.

OpenAgents should model isolation allocation, retained changes, writeback,
cleanup, and resume location separately. “Agent completed” is not equivalent
to “changes integrated” or “workspace can be deleted.”

### 13.10 What this changes in the audit

The source review does not change the history counts, edge-gap rate, or
timeline. It strengthens five design conclusions:

- normalize `Task`/`Agent` aliases before graph reconstruction;
- treat dispatch as capability/context compilation, not simple inheritance;
- persist cancellation and workspace ownership as first-class lifecycle data;
- separate task terminal state from notification and verified receipt; and
- keep subagents, teammates, workflows, and independent sessions distinct even
  when they reuse a query runner or render similarly.

## Final assessment

Claude Code has not used one fixed “subagent feature.” Across the inspected
month it evolved from synchronous tool-return delegation into a background
task system, then mailbox/task coordination, then deterministic scripted
Workflows, and now an independent background-agent manager. The persistent
core stayed remarkably stable: one full JSONL sidechain per agent, sharing the
parent session id and distinguished by `agentId`.

That stability makes Claude history import feasible. The weakness is that the
topology and lifecycle are spread across tool results, notifications, task
records, child files, and workflow journals. A desktop UI can make Claude's
subagents far more legible than the linear CLI view, but only if the importer
reconstructs the tree honestly and preserves the roughly 3% edge-gap class.

For OpenAgents, the lesson is precise: persist a canonical typed agent graph
and outcomes for our own runs; use provider adapters to recover Claude's rich
sidechains; render complete children on Desktop and mobile; and treat raw local
history, summaries, and notifications as evidence inputs rather than authority.
