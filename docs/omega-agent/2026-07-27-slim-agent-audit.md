# Omega Agent slim-agent audit

- Status: audit for the owner direction of 2026-07-27
- Owner: OpenAgents
- Date: 2026-07-27
- Companion: [slim-agent specification](./2026-07-27-slim-agent-spec.md)
- Omega source pin: `OpenAgentsInc/omega` `beb0e870b2` (49 commits after
  `v0.2.0-rc19`, 2026-07-26)
- Upstream fork base: `zed-industries/zed` `640286ae3e` (2026-07-24)
- ProductSpec of record: `specs/omega/omega-agent.product-spec.md` at
  `spec_revision: 1`

This audit records the current state of the Omega agent stack, the Pi
principles the owner wants, and the exact distance between them.
The companion specification records the proposed target shape.
This audit changes no code and admits no packet.

## 1. The owner direction

The owner direction of 2026-07-27 is:

1. Evolve Omega Agent into a simple first-party agent.
2. Keep the agent slim, on the principles of the Pi coding agent.
3. Give the agent five tools: `read`, `write`, `edit`, `bash`, and
   `delegate`.
4. Make `delegate` a first-class tool that hands work to the other
   harnesses Omega already runs, for example Claude Code and Codex.
5. Design the agent so it cannot repeat the work-destruction incident in
   [the oopsiewoopsies record](../oopsiewoopsies/2026-07-27-git-checkout-destroyed-uncommitted-work-twice.md).

The direction changes the center of gravity of the `OMEGA-AGENT` program.
ProductSpec revision 1 admits Omega Agent as a router that owns no
execution.
The slim agent is an executor with its own tool set.
Section 7 records that tension.
The companion specification proposes the reconciliation.

## 2. What "Zed Agent" is

Omega is a fork of Zed.
The first-party agent Omega inherited is the upstream "Zed Agent".
We build the slim Omega Agent as our own version of that agent, so its
definition matters.

### 2.1 The runtime, not a model

Zed Agent is not a model.
It is an in-process orchestration layer above a selected `LanguageModel`.
`NativeAgentServer::connect` creates a GPUI `NativeAgent` entity and returns
a `NativeAgentConnection`.
No child process starts and no ACP transport opens for the native path.
The connection implements the same `AgentConnection` trait that external ACP
agents implement, so one thread UI serves both.
Evidence: `crates/agent/src/native_agent_server.rs`,
`crates/agent/src/agent.rs`, and
[the Zed teardown](../teardowns/2026-07-18-zed-teardown.md) section 14.

### 2.2 The turn loop

`Thread::send` appends one user message and starts a running turn.
The loop compacts context when a threshold requires it, builds a request
from history plus function schemas, streams the completion, races model
events against tool futures and cancellation, appends tool results, and
calls the model again until the turn stops.
The loop reads the model, profile, and enabled tools again between tool
rounds, so mid-turn changes apply to the next request.
Unknown tools and invalid JSON become structured tool errors, not implicit
success.
Evidence: `crates/agent/src/thread.rs`.

### 2.3 The tool contract

A native tool implements the `AgentTool` trait at
`crates/agent/src/thread.rs:5325`:

- A typed `Input` that derives a JSON schema.
- A typed `Output` that converts into tool-result content.
- A constant `NAME`, a `kind()`, and an `initial_title`.
- A `run` method that returns `Result<Output, Output>`, so an error is
  structured, model-legible data with the same shape as success.
- The tool description comes from the doc comment on the `Input` struct,
  so tool prompt engineering lives in Rust doc comments.
- A tool can opt into streamed input. Only `edit_file` and `write_file`
  do, which is what makes live-diff editing possible.
- A `replay` method re-emits UI events when a persisted thread reopens.

The fork adds one method under `OMEGA-DELTA-0111`: a tool states whether its
result is already bounded.
The default answer is no, so a tool that does not answer gets bounded.

### 2.4 The registered tool inventory

`crates/agent/src/tools.rs` in the fork registers 25 tools.
Upstream registers 23 of them.
The fork adds `read_subagent_transcript` and `read_tool_result_artifact`.

| Group | Tools |
| --- | --- |
| Read | `read_file`, `list_directory`, `diagnostics`, `read_subagent_transcript`, `read_tool_result_artifact` |
| Search | `grep`, `find_path`, `find_references`, `go_to_definition`, `get_code_actions` |
| Edit | `edit_file`, `write_file`, `create_directory` |
| Move and delete | `delete_path`, `move_path`, `copy_path` |
| Execute | `terminal` (with a sandboxed twin) |
| Fetch | `fetch`, `search_web` |
| Refactor | `rename_symbol`, `apply_code_action` |
| Orchestration | `spawn_agent`, `create_thread`, `list_agents_and_models`, `skill` |

Three gates shape the enabled set below the registered set:

1. The active profile allowlist in `assets/settings/default.json`.
2. The tool-permissions setup test in `crates/settings_ui`.
3. Feature flags in `tools.rs`.

The enabled set is therefore already a policy surface.
A slim tool set does not need tool deletion.
It needs a slim profile and a slim prompt.

### 2.5 The layers around the loop

The native loop already carries:

- Profiles that gate what the model sees
  (`crates/agent_settings/src/agent_profile.rs`).
- Permission rules that gate approval, with a command-chain parser for
  terminal input that fails closed on unsafe substitution or chaining
  (`crates/agent/src/tool_permissions.rs`).
- Project context and instruction files (`AGENTS.md`, `.rules`,
  `CLAUDE.md`) with prompt-cache-stable refresh.
- Skills from `.agents/skills` at three precedence levels, plus a dynamic
  `skill` tool.
- Local thread persistence, checkpoints, the action log, and diff review.
- Subagents through `spawn_agent`, with bounded depth and cancellation
  propagation.
  Only the subagent's final message returns to the parent.
- Sibling threads through the flag-gated `create_thread` tool.
  The UI-owned sibling host can create a fresh git worktree per thread and
  can target an external agent, so this path already dispatches work to
  Claude Code from the native loop.

These layers are assets.
The slim agent should keep the loop, the persistence, the action log, and
the permission parser, and shrink what the model sees.

### 2.6 The work-safety machinery already present

The upstream runtime already carries four mechanisms that bear on the
work-destruction requirement in section 6:

1. **Git checkpoints per user message.** On send, the thread snapshots the
   working tree, untracked files included, and attaches the checkpoint to
   the user message.
   A restore button appears only when the agent changed something.
   Restore cancels the running turn, rewinds the thread, and restores the
   snapshot.
   Evidence: `crates/acp_thread/src/acp_thread.rs`,
   `crates/project/src/git_store.rs`.
2. **Read-time staleness detection.** The action log records the file
   modification time at every agent read.
   Before an edit applies, the edit session compares the current
   modification time against that record and reports staleness when the
   file changed on disk after the read.
   Evidence: `crates/action_log/src/action_log.rs`,
   `crates/agent/src/tools/edit_session.rs`.
3. **Dirty-buffer prompts.** When the agent edits a buffer with unsaved
   user changes, the tool prompts: save the user's edits first, or
   discard.
   When the agent overwrites such a buffer, the prompt offers cancel, and
   cancel preserves the user's unsaved work.
   Evidence: `crates/agent/src/tools/tool_permissions.rs`.
4. **A prompt-level law.** The upstream system prompt states: "Keep user
   work safe. Do not overwrite, remove, or revert changes you did not
   make."

These mechanisms protect against the agent's *editor-path* mutations.
None of them protects against a *shell-path* restore: `git checkout --
<path>` through the terminal tool bypasses all four.
That is exactly the hole the oopsiewoopsies incident fell through, and
section 6 turns it into a requirement.

## 3. What the fork already landed

The fork is 257 commits ahead of upstream at the pin.
The `OMEGA-AGENT` program is further along in code than the 2026-07-25
planning corpus records.
The delta register holds 74 numbered deltas with mechanical checks in
`crates/omega_deltas`.

### 3.1 The router is live

- `OMEGA-DELTA-0024`: Omega Agent is the first-party agent identity.
  `OMEGA_AGENT_ID` is defined at `crates/agent/src/agent.rs:2567`.
- `OMEGA-DELTA-0029` and `OMEGA-DELTA-0035`: `OmegaAgentConnection` in
  `crates/agent_ui/src/omega_router.rs` implements every `AgentConnection`
  method by delegation, routes deterministically, fails closed, and writes
  a clock-free route journal.
  A mechanical check fails if any method body stops delegating.
- `crates/omega_front_door` owns the typed decision layer: the closed
  three-variant `ExecutorClass` enum, `RouteDecision`, `ExecutorPin`, and
  `ExecutorDisclosure`.
- `OMEGA-DELTA-0021` and `OMEGA-DELTA-0101`: every thread and every
  subagent disclose their executor as a typed record, not a sentence.

### 3.2 Delegation machinery is live

The raw material for the `delegate` tool already exists:

| Capability | Where | Delta |
| --- | --- | --- |
| Spawn a subagent on a named executor | `crates/agent/src/tools/spawn_agent_tool.rs` plus `subagent_executor.rs` | `OMEGA-DELTA-0061` |
| Never substitute an executor | `resolve_subagent_executor`: a request for `codex-acp` runs Codex or fails | `OMEGA-DELTA-0061` |
| Spawn an external ACP subagent and find it in the panel | `crates/agent/src/external_subagent_sessions.rs` | `OMEGA-DELTA-0112` |
| Read a spawned subagent transcript, parent-only | `read_subagent_transcript_tool.rs` | `OMEGA-DELTA-0060` |
| Attach the installed coding agent (Codex, Claude) | `crates/agent_ui/src/omega_agent_attach.rs`, `crates/omega_agent_detect` | `OMEGA-DELTA-0095` |
| External ACP client transport | `crates/agent_servers` (`codex-acp`, `claude-acp` configured out of the box) | `OMEGA-DELTA-0027` |
| Engine lanes with run authority and receipts | `crates/omega_effectd`, `crates/full_auto_ui` | `OMEGA-DELTA-0020` |
| Exo as a pinned lane with an authority gate | `crates/omega_exo_lane`, `omega_exo_log`, `omega_exo_connection.rs` | `OMEGA-DELTA-0042` |
| Wrapped harness provenance pins | `crates/omega_harness` | `OMEGA-DELTA-0025` |

The subagent depth cap is `MAX_SUBAGENT_DEPTH = 1`.

### 3.3 Result bounding is live

- `OMEGA-DELTA-0103`, `OMEGA-DELTA-0111`, `OMEGA-DELTA-0121`: every native
  tool result becomes a versioned artifact with a `tool:<tool_call_id>`
  address.
  The event carries a bounded preview that names what it withheld.
  Every printed address is one the model can spend.
- `read_tool_result_artifact` makes the addresses readable.
- Artifact addresses survive a thread reopen.

This estate matters for the slim design.
Pi bounds nothing and trusts the terminal.
Omega already has a stronger answer, and the slim agent should keep it.

### 3.4 The monorepo delegation estate

Outside the fork, the monorepo owns:

- The `khala-fleet` skill at `.agents/skills/khala-fleet/SKILL.md`, a
  launcher over the Khala -> Pylon -> Codex delegation runbook in the root
  `AGENTS.md`.
- The typed delegate program `khala.fleet.delegate` in
  `packages/khala-tools/src/fleet-delegate-program.ts`, with worker kinds
  `auto`, `claude`, `codex`, `grok` and workflow classes
  `claude_agent_task` and `codex_agent_task`.
- The fleet intents package `packages/khala-fleet-intents` with typed
  steer, approval, worker-selection, and run-control intents.
- A TLA+ supervisor spec at `specs/khala-fleet-delegate/`.

There is no skill named `delegate`.
The `khala-fleet` skill is prose guidance for agents that operate the
fleet.
The owner direction promotes delegation from a skill an agent may read to a
tool the first-party agent always holds.
The Khala seams stay reserved: the roadmap keeps `OMEGA-AGENT-K1` through
`K3` behind a separate owner admission, so the first `delegate` tool
targets local executors and engine lanes only.

## 4. The Pi principles

Sources: [the Pi teardown](../teardowns/2026-07-21-pi-agent-teardown.md) and
the public essay by a Pi maintainer (lucumr.pocoo.org, 2026-01-31).

1. **Four tools.** Pi ships `read`, `write`, `edit`, `bash` as the default
   active set.
   Read-only `grep`, `find`, and `ls` exist but are not the identity.
   The essay is direct: "it has the shortest system prompt of any agent
   that I'm aware of and it only has four tools."
2. **The shell is the capability surface.** Search, listing, browsing, and
   most integrations run as commands through `bash`, not as
   context-loaded tools.
   "LLMs are really good at writing and running code, so embrace this."
3. **A short system prompt.** The prompt states the contract and stops.
   Skills and project files add capability on demand.
4. **Self-extension over MCP.** Pi rejects MCP in core because MCP tools
   load into context at session start and break cache reuse.
   The agent extends itself with skills and small programs instead.
5. **Session records that survive.** Sessions persist as JSONL trees with
   forks, clones, and compaction summaries as typed entries.
6. **Steer and follow-up as first-class verbs.** Mid-turn user injection
   has typed queue modes.
7. **No permission system.** Pi runs with full user authority and tells
   the user to containerize externally.

Principles 1 through 4 are the ones the owner direction adopts.
Principles 5 and 6 are already satisfied differently in Omega: threads
persist locally, and `OMEGA-DELTA-0032` gives send-during-turn one declared
answer per executor with durable queue admission.
Principle 7 is the one Omega must not adopt blindly.
Omega already chose allow-by-default (`OMEGA-DELTA-0002`), and the standing
law is: confirm on irreversible data loss, never on capability.
The slim agent keeps that law and section 6 makes it mechanical.

## 5. The gap, measured

| Pi property | Omega native loop today | Gap |
| --- | --- | --- |
| 4 tools in the default set | 25 registered tools, profile-gated | Large. The model-visible set must shrink to 5. |
| Shortest known system prompt | Upstream Zed template at `crates/agent/src/templates/system_prompt.hbs`, sized for 25 tools and editor workflows | Large. The prompt must be rewritten for the slim set and measured. |
| Shell as the capability surface | `terminal` exists with a fail-closed command-chain parser, plus dedicated `grep`, `find_path`, `list_directory`, LSP tools | The dedicated tools move behind `bash` or into an opt-in profile. |
| Delegation as a capability | `spawn_agent` exists with a named-executor law, but it is one of 25 tools and its name says nothing about harnesses | Rename and promote. `delegate` becomes one of five, with executor classes in its contract. |
| No MCP in core | `context_server` (MCP client) exists and feeds the same tool map | Default off in the slim profile. Keep the crate. |
| Skills on demand | `.agents/skills` discovery plus a dynamic `skill` tool | Keep discovery. Fold invocation into `read` plus the prompt catalog, or keep `skill` as prompt-level machinery. The specification decides. |
| Bounded results | Stronger than Pi: artifacts plus previews | Keep. Fold artifact reading into `read`. |
| Work-loss safety | Checkpoints and diff review exist. `bash` can still destroy uncommitted work with one git command. | New. Section 6. |

## 6. The work-destruction audit

[The oopsiewoopsies record](../oopsiewoopsies/2026-07-27-git-checkout-destroyed-uncommitted-work-twice.md)
documents an agent session in this program that destroyed its own
uncommitted fix twice in one hour with `git checkout -- <file>`, then
walked past three independent warning signals.
The owner asked that the new agent prevent this class.

The record's own conclusions, restated as tool requirements:

1. **The dangerous commands are file-scoped restores, not edits.**
   `git checkout -- <path>`, `git restore`, `git stash`, `git reset
   --hard`, and `git clean -f` operate on files, not on the caller's last
   edit, and they do not know which changes were the caller's.
   Requirement: the `bash` tool must treat these commands as
   irreversible-data-loss commands when the working tree holds uncommitted
   changes, and the standing confirm-on-data-loss law then applies to
   them.
2. **Undo must be a snapshot the harness took.**
   A restore that copies back a snapshot cannot destroy work it did not
   copy.
   Requirement: the agent checkpoints the touched files before a
   mutating tool call, and offers restore from its own checkpoint, never
   from the index.
3. **A verification step that can no-op silently verifies nothing.**
   Requirement: guard decisions and checkpoint writes are typed results
   in the transcript, not silent successes.
4. **Green signals need a falsifier.**
   Requirement: every new guard lands as a numbered delta with a
   mechanical check, and the check is watched failing before it is
   trusted, per the fork's delta discipline.

The fork already holds the two building blocks: the terminal
command-chain parser that fails closed on unparseable input, and hardcoded
denial for selected catastrophic commands.
The gap is one policy layer: a dirty-tree guard for file-scoped git
restore commands, plus checkpoint-before-mutation as a default.

## 7. The tension with ProductSpec revision 1

Revision 1 admits Omega Agent as a router.
The admitted hypothesis is: "Omega Agent implements the existing
`AgentConnection` trait and owns routing, disclosure, and receipts. It
owns no execution and no durable run state."
`OMEGA-AGENT-AC-01` states that Omega Agent does not name the native loop.

The owner direction makes the first-party agent an executor with five
tools.
Under revision 1 vocabulary, the direction reshapes the **native
executor** and its relationship to delegation:

1. The native loop stays one executor, and it becomes slim: five tools, a
   short prompt, and a work-loss guard.
2. Delegation moves from a router-only concern to an in-turn capability of
   the slim executor: the `delegate` tool dispatches to the same executor
   classes the router names.
3. The router seam does not disappear.
   Per-thread pinning, disclosure, and fail-closed routing stay where they
   landed (`OMEGA-DELTA-0029`, `0033`, `0035`, `0055`).

Two readings are possible, and the specification puts the choice to the
owner:

- **Compose.** Keep the router at the thread seam and give the slim native
  executor the `delegate` tool.
  Routing answers "who owns this thread".
  Delegation answers "who runs this subtask".
  This reading needs a ProductSpec revision that renames the native-loop
  identity question, because the slim loop becomes the default face of
  Omega Agent in practice.
- **Collapse.** Make the slim executor the only front door and retire
  per-thread routing in favor of delegation from inside the turn.
  This reading deletes landed, delta-checked machinery and widens the
  spec change.

Either reading requires a `spec_revision` bump.
Revision 1 admits no executor-shape change, and the repository law is
that a spec is never edited to match implementation without a revision.

## 8. What the audit does not decide

1. It does not choose compose versus collapse. The owner decides on the
   specification.
2. It does not allocate delta numbers.
3. It does not change the executor-class enum, which is closed at three by
   `OMEGA-AGENT-AC-04`.
4. It does not open the Khala packets.
5. It does not answer the Omega Nostr identity signing question, which
   stays owner-reserved from the shape record.
