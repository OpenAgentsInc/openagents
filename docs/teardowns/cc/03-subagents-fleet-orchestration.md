# Teardown 03 — Subagents & Fleet Orchestration

**Claude Code** (`~/work/projects/repos/cc`) vs **OpenAgents Coder** (`packages/openagents-cli/src`)

## Component / Subsystem Breakdown

| Concern | Claude Code | OpenAgents Coder |
|---|---|---|
| Task state machine | `Task.ts` (125 ln) + `tasks/*` (~10.8k ln, 7 types) | `coder-tasks.ts` (374 ln, 1 type) |
| Subagent engine | `tools/AgentTool/runAgent.ts` (973 ln, in-process) | `coder-delegate.ts` (1,994 ln, external harnesses) |
| Spawn plumbing | `tools/shared/spawnMultiAgent.ts` (1,093 ln) | `DelegateFleet.submit()` |
| Execution backends | `utils/swarm/backends/` (tmux, iTerm2, in-process) | `DelegateHarness` impls (opencode, claude, codex, Devin/ACP) |
| Inter-agent comms | `utils/teammateMailbox.ts` (1,183 ln) + `SendMessageTool` (997 ln) | none |
| Shared blackboard | `TaskCreate/Get/List/Update/Stop/Output` tools | none |
| Orchestrator persona | `coordinator/coordinatorMode.ts` (369 ln) | none |
| Fleet rendering | React components + `pillLabel.ts` | `coder-fleet.ts` (272 ln, pure text) + `coder-ui.ts` sidebar |

## Claude Code Implementation Details

**The dual registries.** CC separates two things both called "tasks." The *runtime registry* (`AppState.tasks`) holds live executions: `TaskStateBase` (id, type, status, startTime, `outputFile`, `outputOffset`, `notified`) extended by seven discriminated unions — `local_shell`, `local_agent`, `remote_agent`, `in_process_teammate`, `local_workflow`, `monitor_mcp`, `dream`. IDs are prefixed per type (`a`/`b`/`r`/`t`/`w`/`m`/`d`) plus 8 chars of `crypto.randomBytes` base36 (36⁸ ≈ 2.8T, chosen to resist symlink brute-force on shared output dirs). Each type implements `Task { name, type, kill(taskId, setAppState) }`; `tasks.ts` dispatches only `kill` polymorphically. The *coordination blackboard* (`utils/tasks.ts`) is a persisted task list with `pending/in_progress/completed` statuses and owners, exposed to the model via TaskCreate/TaskUpdate so multiple agents can self-assign work.

**The subagent engine.** `runAgent()` is an async generator that recursively re-enters the full query loop *in the parent process*, parameterized by an `AgentDefinition` (from `loadAgentsDir.ts`, 755 ln: zod-validated `.claude/agents/*.md` frontmatter — tools allowlist, model, permission modes, hooks, effort levels, memory scope). Variants: `forkSubagent.ts` clones parent context into the child; `resumeAgent.ts` resumes a prior agent transcript including content-replacement state; `worktreePath` gives git-isolation per child. Built-ins (`builtInAgents.ts`) compose with user agents: explore, plan, general-purpose, verification (gated), plus coordinator-mode worker agents loaded lazily to break import cycles.

**Cache economics as architecture.** `shouldInjectAgentListInMessages()` moves the dynamic agent list out of the tool description into an `agent_listing_delta` attachment because the mutable description was **~10.2% of fleet cache_creation tokens** — any plugin load or permission change busting the tool-schema cache. This measurement drove a protocol change, not a tweak.

**Swarm mode.** `spawnMultiAgent.ts` resolves teammate model (`inherit` alias → leader model), picks a backend from the cached registry (`tmux` | `iterm2` | `in-process`), creates a colored pane, seeds a file mailbox. `inProcessRunner.ts` (1,552 ln) runs teammates in-process with isolated context and `onIdleCallbacks` so the leader waits without polling. Mailboxes carry typed envelopes: idle notifications, permission requests/responses, sandbox permissions, plan approvals. `SendMessageTool` extends delivery across sessions via UDS sockets and a bridge. `coordinatorMode.ts` swaps the whole persona: a coordinator system prompt, an `ASYNC_AGENT_ALLOWED_TOOLS` minus internal-tools worker allowlist, and a permission-free scratchpad directory for cross-worker knowledge. Completion flows back as `<task-notification>` XML injected into the leader's message queue — background agents return a task ID immediately and interrupt the leader later.

## OpenAgents Coder Implementation State

**One task shape, five statuses.** `CoderTaskRegistry` holds `CoderTask` records: `pending/running/completed/failed/stopped` (`stopped` deliberately ≠ `failed`), insertion-ordered, with `CoderTaskProgress` using exactly CC's token scheme (latest cumulative input replaces; output sums) and `MAX_RECENT_ACTIVITIES = 5` — direct lineage. Mutators no-op on unknown IDs; register-before-start makes launch failures visible; `prune()` drops terminal-read tasks after `STOPPED_DISPLAY_MS`; unread badges apply only to background children.

**Fleet as harness multiplexer.** `DelegateHarness` normalizes four external CLIs (opencode, claude, codex, Devin-over-ACP) into five event kinds (`session/tool/tokens/text/error`) via pure line parsers. `DelegateFleet` enforces `maxConcurrent` with an 8× queued bound, refuses with result-codes rather than throws (`fleet_full`, `empty_prompt`), appends the raw event stream to `<tmp>/openagents-coder-delegations/<id>.jsonl` whether watched or not, retries provider failures with backoff and session-resume, and honors abort → `registry.stopAll()`. Lanes are harness+model pairs (`CHILD_LANES`); the delegate tool sends one prompt to ≤32 children with `identify()` prepending "You are child N of M," reports aggregated outcomes, and blocks until all finish. `coder-child-gateway.ts` mints model-pinned `ChildGrant`s served on a loopback proxy (flatten-to-text turns, one grant-refresh retry); `computer-agents.ts` adds an Effect-layered ACP process service with byte-capped, scrubbed output. Rendering is pure text (`coder-fleet.ts`) consumed identically by TUI, `--plain`, and headless, with a focus-navigable sidebar and a live child-transcript screen in `coder-ui.ts`. Test coverage is genuine: fake harnesses drive scheduling/cancellation without a model.

## Detailed Gap Analysis

1. **No true background delegation.** OA's `delegate` tool awaits every child before returning; CC returns a task ID and injects `<task-notification>` later. OA's `unread` badge informs the *human* only — the model can never react to a finished child mid-turn. This is the largest behavioral gap.
2. **No in-process subagent.** CC re-enters its own loop with scoped tools/context at zero process cost; OA can only spawn foreign CLIs. Its own model cannot recurse, so no explore/plan-style cheap helpers exist.
3. **No user-defined agents.** Nothing answers `.claude/agents/*.md`; `AgentCatalogEntry` covers ACP inventory, not declarative presets (model/lane/tools/timeouts).
4. **No inter-agent communication.** No mailboxes, no SendMessage, no teams, no permission-over-mailbox, no idle callbacks. A child gets one prompt and returns one string; follow-ups require new children.
5. **No shared blackboard.** No persistent owner-bearing task list for cross-child coordination.
6. **No per-child isolation.** CC's `isolation: "worktree"` has no analog; OA children share `cwd` (docs say "in this repository").
7. **No coordinator persona**, scratchpad gating, or worker tool allowlists (though `coder-memory.ts` inherit/harvest already mirrors the scratchpad spirit).

Deliberate, defensible divergences: one registry instead of two (simpler, honest); pure-text rendering instead of per-task React trees; refusal-as-result instead of exceptions; transcripts-always-written. These are worth keeping.

## Actionable Porting Recommendations

1. **Make delegation interruptible** (highest value): when `count × estimated cost` warrants, return task IDs immediately, keep children running in `DelegateFleet`, and push a completion notice into `coder-thread.ts` at the next turn boundary — CC's `<task-notification>` pattern mapped onto the existing `unread`/registry machinery.
2. **Add `.openagents/agents/*.md`**: frontmatter → `{lane, cwd, timeoutMs, toolFamily allowlist}` folded into `DelegationRequest`; inject the catalog as an attachment-style delta, copying CC's measured cache-bust rationale verbatim.
3. **Add an in-process child runner**: wrap the existing `coder-thread` loop as an async-generator harness implementing `DelegateHarness`, constrained by `coder-tool-budget.ts` — registers in the same registry, renders in the same fleet, no process spawn.
4. **Worktree flag**: `git worktree add` in `execute()`, report branch in the fleet row (CC's `WORKTREE_BRANCH_TAG`).
5. **Child mailboxes**: per-child append-only JSONL plus harness stdin where available; start with `pendingUserMessages` drained on turn boundaries, then permission round-trips.
6. **Coordinator profile**: a system-prompt variant enumerating lanes and child capabilities over `coder-memory` scratchpads — a cheap, high-leverage port.
