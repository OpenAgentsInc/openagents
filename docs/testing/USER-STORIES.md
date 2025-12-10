# OpenAgents User Stories for Testing

> Comprehensive catalog of user stories across all system components.
> Priority: P0 (Critical), P1 (High), P2 (Medium), P3 (Low)

---

## Task Tracking Process

### How to Track Implementation

Each user story can be linked to one or more OpenAgents tasks. The mapping is tracked in the **Task Tracking Matrix** section at the end of this document.

**Workflow:**
1. **Creating tests**: When creating a task to implement tests for user stories, note which story IDs it covers in the task description
2. **Update matrix**: Add the mapping to the Task Tracking Matrix below
3. **Mark completion**: When tests pass and task is closed, update the Status column

**Status Legend:**
- ` ` (blank) - No task created yet
- `📋` - Task created, not started
- `🔄` - Task in progress
- `✅` - Tests implemented and passing
- `❌` - Tests failing / blocked

### Quick Reference: P0 Stories Needing Tasks

The following P0 stories still need tasks created:
- HUD-020..024 (Node display)
- HUD-030..034 (Real-time updates)
- HUD-050..051 (APM widget)
- TASK-001..003, TASK-010..012, TASK-020..022
- ORCH-001..002, ORCH-010..011, ORCH-020..022, ORCH-030..032, ORCH-040..041, ORCH-050, ORCH-070
- CLI-001..005
- LLM-010..012
- TOOL-001..002, TOOL-010..011, TOOL-020..021, TOOL-030..031
- CONF-001..002, CONF-010..011

---

## 1. Desktop Application (Electrobun HUD)

The HUD is the primary user interface - a real-time visualization of autonomous agent work.

### 1.1 Application Launch & Lifecycle

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| HUD-001 | P0 | As a user, I can launch the desktop app | App window opens without errors, shows loading state |
| HUD-002 | P0 | As a user, I see the flow diagram on startup | SVG canvas renders with grid background |
| HUD-003 | P1 | As a user, I see default/sample data when no agent is running | Placeholder nodes displayed gracefully |
| HUD-004 | P1 | As a user, I can close the app cleanly | Window closes, processes terminate, no orphans |
| HUD-005 | P2 | As a user, I can resize the window | Layout adapts, canvas remains usable |
| HUD-006 | P2 | As a user, the app remembers window position | Position persists across restarts |

### 1.2 Flow Visualization - Canvas Interactions

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| HUD-010 | P0 | As a user, I can pan the canvas by dragging | Click-drag moves viewport, cursor changes to grabbing |
| HUD-011 | P0 | As a user, I can zoom with scroll wheel | Wheel up/down scales view, zoom centers on cursor |
| HUD-012 | P0 | As a user, I can reset the view | Reset button returns to 100% zoom, centered position |
| HUD-013 | P1 | As a user, I see smooth inertial scrolling after pan | Velocity-based animation continues after release |
| HUD-014 | P1 | As a user, zoom has sensible limits | Clamps between 10% and 400% (or similar) |
| HUD-015 | P2 | As a user, I can double-click to zoom in | Double-click zooms to cursor position |
| HUD-016 | P2 | As a user, I can use keyboard shortcuts | Arrow keys pan, +/- zoom, 0 resets |
| HUD-017 | P3 | As a user, I can use trackpad gestures | Pinch-to-zoom, two-finger scroll |

### 1.3 Flow Visualization - Node Display

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| HUD-020 | P0 | As a user, I see the root agent node | "MechaCoder" node at top of hierarchy |
| HUD-021 | P0 | As a user, I see connected repository nodes | Repo nodes branch from agent, show repo name |
| HUD-022 | P0 | As a user, I see task nodes under repos | Task nodes show title, priority badge |
| HUD-023 | P0 | As a user, I see phase/subtask nodes | Subtasks branch from tasks during decomposition |
| HUD-024 | P0 | As a user, nodes have status colors | idle=gray, busy=amber, error=red, blocked=purple, completed=green |
| HUD-025 | P1 | As a user, I see node type themes | root=indigo, agent=amber, repo=blue, task=green, phase=gray |
| HUD-026 | P1 | As a user, connections between nodes are visible | Curved paths with glowing effect connect parent-child |
| HUD-027 | P2 | As a user, I can hover nodes for details | Tooltip shows full title, status, timestamps |
| HUD-028 | P2 | As a user, I can click a node to focus it | Zooms and centers on selected node |
| HUD-029 | P3 | As a user, I can collapse/expand node subtrees | Toggle visibility of children |

### 1.4 Real-Time Updates (WebSocket/HUD Protocol)

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| HUD-030 | P0 | As a user, I see live updates when agent starts | session_start triggers UI refresh |
| HUD-031 | P0 | As a user, I see task selection updates | task_selected message adds/highlights node |
| HUD-032 | P0 | As a user, I see subtask decomposition | task_decomposed creates child nodes |
| HUD-033 | P0 | As a user, I see subtask progress | subtask_start/complete changes node status |
| HUD-034 | P0 | As a user, I see verification status | verification_start/complete updates display |
| HUD-035 | P1 | As a user, I see commit/push events | commit_created/push_complete reflected in UI |
| HUD-036 | P1 | As a user, I see phase changes | phase_change message updates current state |
| HUD-037 | P1 | As a user, I see errors displayed | error message shows in UI with context |
| HUD-038 | P2 | As a user, I see streaming text output | text_output messages display in real-time |
| HUD-039 | P2 | As a user, I see tool calls/results | tool_call/tool_result logged for debugging |
| HUD-040 | P1 | As a user, updates work after WebSocket reconnect | Graceful reconnection, queued messages delivered |

### 1.5 APM Widget

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| HUD-050 | P0 | As a user, I see current session APM | Widget displays actions-per-minute value |
| HUD-051 | P0 | As a user, APM color reflects velocity | <5=gray, 5-15=blue, 15-30=green, >30=gold |
| HUD-052 | P1 | As a user, I see total actions count | Session action count displayed |
| HUD-053 | P1 | As a user, I see session duration | Minutes since session start |
| HUD-054 | P1 | As a user, I see efficiency ratio | MechaCoder vs Claude Code comparison |
| HUD-055 | P2 | As a user, I see historical APM (1h, 6h, 24h) | apm_snapshot message populates time windows |
| HUD-056 | P2 | As a user, I see tool usage breakdown | apm_tool_usage shows which tools used most |
| HUD-057 | P3 | As a user, APM updates smoothly | No flicker, animated transitions |

### 1.6 Error Handling & Resilience

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| HUD-060 | P0 | As a user, app doesn't crash on WebSocket disconnect | Shows last known state, indicates disconnected |
| HUD-061 | P0 | As a user, malformed messages don't crash app | Invalid JSON silently ignored, logged |
| HUD-062 | P1 | As a user, I see clear error indicators | Error state visible in nodes and status bar |
| HUD-063 | P1 | As a user, app recovers from multiple errors | Continues functioning after error sequence |
| HUD-064 | P2 | As a user, I can manually refresh the view | Refresh button or F5 reloads state |

---

## 2. Task System (.openagents/)

The task system is the backbone of autonomous work tracking.

### 2.1 Task Creation

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| TASK-001 | P0 | As an agent, I can create a new task | `tasks:create` produces valid task in tasks.jsonl |
| TASK-002 | P0 | As an agent, I can set task priority (0-4) | Priority persists and affects ordering |
| TASK-003 | P0 | As an agent, I can set task type | bug/feature/task/epic/chore supported |
| TASK-004 | P1 | As an agent, I can add labels to tasks | Labels array stored and searchable |
| TASK-005 | P1 | As an agent, I can set task description | Description stored, available in show |
| TASK-006 | P2 | As an agent, I can create tasks with dependencies | deps array with id and type |

### 2.2 Task Listing & Discovery

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| TASK-010 | P0 | As an agent, I can list all tasks | `tasks:list` returns all tasks as JSON |
| TASK-011 | P0 | As an agent, I can list ready tasks | `tasks:ready` shows only unblocked open tasks |
| TASK-012 | P0 | As an agent, I can claim next task | `tasks:next` atomically picks and marks in_progress |
| TASK-013 | P1 | As an agent, I can filter tasks by status | --status=open/in_progress/blocked/closed |
| TASK-014 | P1 | As an agent, I can search tasks by keyword | `tasks:search` matches title/description |
| TASK-015 | P2 | As an agent, I can filter by label | --labels=e2e,p0 filters to matching |
| TASK-016 | P2 | As an agent, I can filter by type | --type=bug shows only bugs |

### 2.3 Task State Management

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| TASK-020 | P0 | As an agent, I can update task status | open → in_progress → closed transitions |
| TASK-021 | P0 | As an agent, I can close a task with reason | `tasks:close` sets status, closeReason, closedAt |
| TASK-022 | P0 | As an agent, closed tasks record commit SHA | commits array populated on close |
| TASK-023 | P1 | As an agent, I can block a task | status=blocked with reason |
| TASK-024 | P1 | As an agent, blocking deps prevent ready | Task with open blocker not in ready list |
| TASK-025 | P2 | As an agent, I can reopen a closed task | status back to open, closedAt cleared |
| TASK-026 | P2 | As an agent, I can archive old tasks | `tasks:archive` moves to archive file |

### 2.4 Task Dependencies

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| TASK-030 | P1 | As an agent, I can add blocking dependencies | Task A blocks Task B |
| TASK-031 | P1 | As an agent, blocked tasks are excluded from ready | Dependency chain respected |
| TASK-032 | P2 | As an agent, I can add related dependencies | Non-blocking relationship tracked |
| TASK-033 | P2 | As an agent, I can add parent-child dependencies | Epic → subtask relationship |
| TASK-034 | P2 | As an agent, discovered-from tracks origins | New task linked to discovering task |

---

## 3. Agent Orchestrator (Golden Loop)

The orchestrator runs the autonomous task completion loop.

### 3.1 Task Selection & Decomposition

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| ORCH-001 | P0 | As MechaCoder, I pick highest priority ready task | P0 before P1, oldest first within priority |
| ORCH-002 | P0 | As MechaCoder, I decompose task into subtasks | task_decomposed event with subtask list |
| ORCH-003 | P1 | As MechaCoder, I skip tasks with failing deps | Blocked tasks never selected |
| ORCH-004 | P1 | As MechaCoder, I respect maxTasksPerRun | Stops after configured number of tasks |
| ORCH-005 | P2 | As MechaCoder, I respect maxRuntimeMinutes | Graceful stop at time limit |

### 3.2 Subagent Execution

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| ORCH-010 | P0 | As MechaCoder, I route to Claude Code subagent | Complex tasks go to Claude Code |
| ORCH-011 | P0 | As MechaCoder, I can use minimal subagent | Fallback for simple tasks or when CC unavailable |
| ORCH-012 | P1 | As MechaCoder, I track turns per subtask | maxTurnsPerSubtask enforced |
| ORCH-013 | P1 | As MechaCoder, I capture files modified | filesModified array in result |
| ORCH-014 | P0 | As MechaCoder, I prefer Claude Code for complex subtasks | When `claudeCode.enabled=true` and a subtask is labeled complex (epic, multi-file, or long description), orchestrator routes it to Claude Code instead of the minimal subagent |
| ORCH-015 | P0 | As MechaCoder, I fall back to minimal subagent on CC error | If Claude Code fails (timeout, rate limit, auth error) and `fallbackToMinimal=true`, orchestrator retries the subtask with the minimal subagent and logs the fallback |
| ORCH-016 | P1 | As MechaCoder, I resume Claude Code sessions across cycles | When a subtask spans multiple orchestrator cycles, the previously stored CC session ID is used so CC continues with full context |
| ORCH-017 | P1 | As a user, I can force CC-only or minimal-only mode | CLI flags (`--cc-only`, `--minimal-only`) override routing heuristics, and runs are logged with the chosen mode |

### 3.3 Verification

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| ORCH-020 | P0 | As MechaCoder, I run typecheck after changes | typecheckCommands executed |
| ORCH-021 | P0 | As MechaCoder, I run tests after changes | testCommands executed |
| ORCH-022 | P0 | As MechaCoder, I block commit on failures | No commit if typecheck or tests fail |
| ORCH-023 | P1 | As MechaCoder, I run e2e tests when configured | e2eCommands executed if non-empty |
| ORCH-024 | P1 | As MechaCoder, I capture test output | Output included in verification events |
| ORCH-025 | P2 | As MechaCoder, I retry on flaky tests | Configurable retry count |

### 3.4 Git Operations

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| ORCH-030 | P0 | As MechaCoder, I commit changes on success | git commit with task ID in message |
| ORCH-031 | P0 | As MechaCoder, I push when allowPush=true | git push to remote |
| ORCH-032 | P0 | As MechaCoder, I never force push to main | allowForcePush=false respected |
| ORCH-033 | P1 | As MechaCoder, I record commit SHA in task | commits array updated |
| ORCH-034 | P1 | As MechaCoder, I handle pre-commit hooks | Retry once if hook modifies files |
| ORCH-035 | P2 | As MechaCoder, I create branches when configured | Feature branch workflow supported |

### 3.5 Session Management

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| ORCH-040 | P0 | As MechaCoder, I emit session_start on begin | Session ID, timestamp in event |
| ORCH-041 | P0 | As MechaCoder, I emit session_complete on end | Success/failure, summary in event |
| ORCH-042 | P1 | As MechaCoder, I can resume interrupted sessions | Session state persisted and recoverable |
| ORCH-043 | P1 | As MechaCoder, I prevent concurrent runs | agent.lock file prevents overlap |
| ORCH-044 | P2 | As MechaCoder, I log full session to run-logs | JSONL log with all events |
| ORCH-045 | P1 | As MechaCoder, I write a full JSONL event stream | Each session creates `.openagents/run-logs/YYYYMMDD/run-<id>.jsonl` with all events (turn_start, llm_response, tool_call/result, retry_prompt, run_end) |
| ORCH-046 | P2 | As MechaCoder, I support session replay | A replay harness can reconstruct the agent's conversation and tool calls from the JSONL event stream without additional state |

### 3.6 Error Recovery

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| ORCH-050 | P0 | As MechaCoder, I emit error events | error message with phase and details |
| ORCH-051 | P1 | As MechaCoder, I track consecutive failures | After N failures, mark task blocked |
| ORCH-052 | P1 | As MechaCoder, I include retry context | Previous failure info in retry prompt |
| ORCH-053 | P2 | As MechaCoder, I gracefully handle stop signals | Clean shutdown on SIGTERM/SIGINT |

### 3.7 Safe Mode & Preflight

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| ORCH-070 | P0 | As MechaCoder, I run preflight checks before doing work | `.openagents/init.sh` is executed before task selection; failures abort the session with a clear error in logs and HUD |
| ORCH-071 | P1 | As a user, I can enable safe mode self-healing | With `--safe-mode`, when preflight fails due to typecheck errors, orchestrator attempts a repair subtask (e.g., "fix type errors") before giving up |
| ORCH-072 | P1 | As MechaCoder, I keep the workspace clean on preflight failure | After failed init, no partial commits are created and working tree changes are either reverted or clearly logged as needing manual cleanup |

### 3.8 Sandbox Execution

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| ORCH-080 | P1 | As MechaCoder, I run inside a sandbox when configured | For a project with `sandbox.enabled=true`, agent processes execute in the sandbox (container/Seatbelt) and cannot read/write outside the workspace |
| ORCH-081 | P1 | As MechaCoder, I fail clearly if sandbox startup fails | Missing/invalid container runtime produces a clear error and aborts the session without partially-modified tasks |
| ORCH-082 | P2 | As MechaCoder, I gracefully fall back when allowed | If `sandbox.backend="auto"` and preferred backend is unavailable, orchestrator either selects a fallback backend or runs unsandboxed according to config, and logs this decision |

---

## 4. CLI Commands

Command-line interfaces for external agent interaction.

### 4.1 Task CLI

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| CLI-001 | P0 | As a user, I can run `bun run tasks:list` | JSON output of all tasks |
| CLI-002 | P0 | As a user, I can run `bun run tasks:ready` | JSON output of ready tasks |
| CLI-003 | P0 | As a user, I can run `bun run tasks:next` | Claims and returns next task |
| CLI-004 | P0 | As a user, I can run `bun run tasks:create` | Creates task from flags |
| CLI-005 | P0 | As a user, I can run `bun run tasks:close` | Closes task with reason |
| CLI-006 | P1 | As a user, I can run `bun run tasks:search` | Searches tasks by keyword |
| CLI-007 | P2 | As a user, I can run `bun run tasks:archive` | Archives closed tasks |

### 4.2 MechaCoder CLI

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| CLI-010 | P0 | As a user, I can run `bun run mechacoder` | Executes one task via orchestrator (do-one-task); uses `--legacy` flag to hit the old direct agentLoop |
| CLI-011 | P0 | As a user, I can run `bun run mechacoder:overnight` | Runs orchestrator until maxTasks or maxRuntime; uses `--legacy` flag for direct agentLoop |
| CLI-012 | P1 | As a user, I can run `--cc-only` mode | Forces Claude Code subagent |
| CLI-013 | P1 | As a user, I can run `--minimal-only` mode | Uses minimal agent exclusively |
| CLI-014 | P2 | As a user, I can run `mechacoder:parallel` | Parallel task execution with worktrees |
| CLI-015 | P2 | As a user, I can run `mechacoder:parallel` with N agents | `bun run mechacoder:parallel --max-agents 4` runs up to 4 agents using worktrees, honoring ParallelExecutionConfig |

### 4.3 Session CLI

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| CLI-020 | P1 | As a user, I can run `session:list` | Lists recent sessions |
| CLI-021 | P1 | As a user, I can run `session:show` | Shows session details |
| CLI-022 | P2 | As a user, I can run `session:search` | Searches session content |
| CLI-023 | P2 | As a user, I can run `session:by-task` | Finds sessions for a task |
| CLI-024 | P1 | As a user, I can tail a running session's events | `tail -f .openagents/run-logs/YYYYMMDD/run-*.jsonl` shows streaming events (run_start, turn_start, tool_call, run_end) during an active run |
| CLI-025 | P2 | As a user, I can export a session to HTML | `bun run session:export-html <sessionId>` produces an HTML transcript from JSONL |
| CLI-026 | P2 | As a user, I can import a pi-mono JSONL session | `session:import-pi <path>` converts a pi-mono session file into `.openagents/sessions` format for replay |

### 4.4 Worktree CLI

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| CLI-030 | P1 | As a user, I can list active worktrees | `mechacoder:worktrees list` shows active worktrees with task IDs and branch names |
| CLI-031 | P1 | As a user, I can prune stale worktrees | `mechacoder:worktrees prune` removes stale/finished worktrees safely |
| CLI-032 | P2 | As a user, I can inspect a worktree | `mechacoder:worktrees show <id>` shows worktree details including task, branch, and status |

---

## 5. LLM Provider Integration

Provider abstraction for multiple AI backends.

### 5.1 Provider Selection

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| LLM-001 | P0 | As the system, I use Claude Code as the primary coding subagent | When `claudeCode.enabled=true`, Claude Code handles complex subtasks; OpenRouter/others serve as underlying chat providers where configured |
| LLM-002 | P1 | As the system, I can use Anthropic direct | ANTHROPIC_API_KEY enables direct calls |
| LLM-003 | P1 | As the system, I can use OpenAI | OPENAI_API_KEY enables OpenAI calls |
| LLM-004 | P2 | As the system, I can use Google Gemini | GOOGLE_API_KEY enables Gemini calls |
| LLM-005 | P2 | As the system, I can use custom base URLs | Provider overrides for Groq, Cerebras, xAI |

### 5.2 Chat & Tool Calling

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| LLM-010 | P0 | As the system, I can send chat messages | Request/response cycle works |
| LLM-011 | P0 | As the system, I can define tools | Tool schemas sent to provider |
| LLM-012 | P0 | As the system, I can receive tool calls | Tool call JSON parsed correctly |
| LLM-013 | P1 | As the system, I can stream responses | Chunked streaming supported |
| LLM-014 | P1 | As the system, I can stream partial tool args | Incremental tool argument parsing |
| LLM-015 | P2 | As the system, I handle thinking/reasoning | Extended thinking tokens processed |

### 5.3 Token & Cost Accounting

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| LLM-020 | P1 | As the system, I track input tokens | Usage metadata captured |
| LLM-021 | P1 | As the system, I track output tokens | Output token count recorded |
| LLM-022 | P2 | As the system, I track cache read/write | Cache hit tokens separated |
| LLM-023 | P2 | As the system, I calculate costs | Model-specific pricing applied |
| LLM-024 | P3 | As the system, I aggregate usage per session | Session-level token totals |

---

## 6. Core Tools

Built-in tools for file operations and system interaction.

### 6.1 File Reading

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| TOOL-001 | P0 | As an agent, I can read files | Read tool returns file contents |
| TOOL-002 | P0 | As an agent, I can read with offset/limit | Partial file reads work |
| TOOL-003 | P1 | As an agent, I see line numbers in output | cat -n style formatting |
| TOOL-004 | P2 | As an agent, I can read binary files | Images, PDFs handled gracefully |

### 6.2 File Writing

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| TOOL-010 | P0 | As an agent, I can write files | Write tool creates/overwrites files |
| TOOL-011 | P0 | As an agent, I can edit files | Edit tool does search/replace |
| TOOL-012 | P1 | As an agent, edits require unique match | Ambiguous edits rejected |
| TOOL-013 | P1 | As an agent, I can replace_all | Multiple occurrences replaced |

### 6.3 File Search

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| TOOL-020 | P0 | As an agent, I can grep for patterns | Grep tool finds matches |
| TOOL-021 | P0 | As an agent, I can find files by pattern | Find tool with glob support |
| TOOL-022 | P1 | As an agent, I can list directories | Ls tool shows directory contents |
| TOOL-023 | P2 | As an agent, grep supports regex | Full regex pattern matching |

### 6.4 Shell Execution

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| TOOL-030 | P0 | As an agent, I can run bash commands | Bash tool executes commands |
| TOOL-031 | P0 | As an agent, I see stdout/stderr | Output captured and returned |
| TOOL-032 | P1 | As an agent, I can set timeouts | Long commands can be killed |
| TOOL-033 | P2 | As an agent, I can run background tasks | Async execution supported |

---

## 7. Project Configuration

.openagents/project.json settings.

### 7.1 Basic Configuration

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| CONF-001 | P0 | As a user, I can set defaultBranch | Used for PR base branch |
| CONF-002 | P0 | As a user, I can set testCommands | Commands run in verification |
| CONF-003 | P1 | As a user, I can set typecheckCommands | Typecheck commands configured |
| CONF-004 | P1 | As a user, I can set e2eCommands | E2E test commands configured |
| CONF-005 | P2 | As a user, I can set defaultModel | LLM model selection |

### 7.2 Safety Configuration

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| CONF-010 | P0 | As a user, I can set allowPush | Controls git push behavior |
| CONF-011 | P0 | As a user, I can set allowForcePush | Prevents force push to main |
| CONF-012 | P1 | As a user, I can set maxTasksPerRun | Limits tasks per session |
| CONF-013 | P1 | As a user, I can set maxRuntimeMinutes | Time limit for sessions |

### 7.3 Claude Code Configuration

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| CONF-020 | P1 | As a user, I can enable/disable Claude Code | claudeCode.enabled flag |
| CONF-021 | P1 | As a user, I can set maxTurnsPerSubtask | Limits subagent iterations |
| CONF-022 | P2 | As a user, I can set permissionMode | Controls bypass behavior |
| CONF-023 | P2 | As a user, I can set fallbackToMinimal | Enables minimal fallback |
| CONF-024 | P1 | As a user, I can control CC routing heuristics | `claudeCode.preferForComplexTasks`, `complexityThreshold`, and label overrides (`forceMinimalLabels`, `forceClaudeCodeLabels`) change which subtasks are routed to CC vs minimal subagent |

### 7.4 Sandbox Configuration

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| CONF-030 | P1 | As a user, I can enable sandbox mode per project | `sandbox.enabled=true` in project.json causes MechaCoder to run tasks via the configured sandbox backend instead of directly on the host |
| CONF-031 | P1 | As a user, I can choose a sandbox backend | `sandbox.backend` accepts `"apple-container"`, `"docker"`, `"none"` (or `"auto"`), and orchestrator respects this choice |
| CONF-032 | P2 | As a user, I can set sandbox resource limits | `sandbox.memoryLimit` and `sandbox.cpuLimit` constrain container resources (verified via stress test) |
| CONF-033 | P2 | As a user, I can override sandbox at runtime | `--no-sandbox` CLI flag disables sandboxing even if enabled in project.json |

---

## 8. Parallel Execution & Worktrees

Multi-agent parallel task execution with git worktrees.

### 8.1 Parallel Orchestrator

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| PAR-001 | P1 | As MechaCoder, I can run multiple tasks in parallel | With `parallelExecution.enabled=true` and `maxAgents=2`, orchestrator starts two agents concurrently, each working on a different ready task |
| PAR-002 | P1 | As MechaCoder, each agent uses its own worktree | Each parallel agent runs in its own git worktree/branch (e.g., `.worktrees/oa-123` on `agent/oa-123`), and commits only affect that branch |
| PAR-003 | P1 | As a user, I can list and prune active worktrees | `mechacoder:worktrees list` shows active worktrees; `mechacoder:worktrees prune` removes stale/finished ones safely |
| PAR-004 | P2 | As MechaCoder, I respect configured merge strategy | `parallelExecution.mergeStrategy` (`direct` / `queue` / `pr` / `auto`) results in the expected main-branch update flow |
| PAR-005 | P2 | As a user, parallel mode doesn't corrupt main | After a parallel run, `git status` is clean on main, and all merged commits correspond to closed tasks |

### 8.2 Worktree Isolation

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| PAR-010 | P1 | As MechaCoder, worktrees are isolated | Changes in one worktree don't affect others or main until merged |
| PAR-011 | P1 | As MechaCoder, worktrees have full .openagents context | Each worktree has access to project.json and can update its task in tasks.jsonl |
| PAR-012 | P2 | As MechaCoder, I handle worktree conflicts | When multiple agents complete, merge conflicts are detected and reported |
| PAR-013 | P2 | As MechaCoder, I cleanup worktrees on completion | Finished worktrees are automatically removed after successful merge |

---

## 9. TerminalBench Command Center (TBCC)

The TBCC is a unified interface for managing and visualizing TerminalBench runs.

### 9.1 Dashboard

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| TBCC-001 | P0 | As a user, I can see the TBCC dashboard | Dashboard displays KPIs and recent runs |
| TBCC-002 | P1 | As a user, I can see key performance indicators | Pass rate, total runs, and average duration displayed |
| TBCC-003 | P1 | As a user, I can see a list of recent runs | Table shows run ID, status, task, and duration |
| TBCC-004 | P1 | As a user, I can quickly start a benchmark run | "Run Full Benchmark" button initiates a run |
| TBCC-005 | P2 | As a user, I can navigate to a run from the dashboard | Clicking a recent run switches to Run Browser and selects it |

### 9.2 Task Browser

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| TBCC-010 | P0 | As a user, I can browse available tasks | List of tasks from suite file displayed |
| TBCC-011 | P1 | As a user, I can filter tasks by difficulty | Filter buttons (Easy, Medium, Hard) update the list |
| TBCC-012 | P1 | As a user, I can search tasks by name | Search input filters the task list in real-time |
| TBCC-013 | P0 | As a user, I can view task details | Selecting a task shows description, timeout, and tags |
| TBCC-014 | P0 | As a user, I can run a specific task | "Run Task" button in details view initiates execution |

### 9.3 Run Browser

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| TBCC-020 | P0 | As a user, I can view local run history | List of local runs displayed with status indicators |
| TBCC-021 | P1 | As a user, I can view HuggingFace trajectories | Toggle/Tab to switch to HF dataset view |
| TBCC-022 | P0 | As a user, I can view run details | Selecting a run shows step-by-step execution details |
| TBCC-023 | P1 | As a user, I can see terminal output for a run | Terminal output tab/section in details view |
| TBCC-024 | P2 | As a user, I can filter runs by status | Filter by passed/failed/running |

### 9.4 Settings

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| TBCC-030 | P1 | As a user, I can configure execution settings | Max attempts, timeout, and recursion limit settings |
| TBCC-031 | P1 | As a user, I can configure logging settings | Toggle save trajectories, terminal output, traces |
| TBCC-032 | P0 | As a user, settings are persisted | Settings saved to local storage and restored on load |
| TBCC-033 | P2 | As a user, I can reset settings to default | "Reset Defaults" button restores original values |

---

## Testing Coverage Summary

### By Priority

| Priority | Count | Description |
|----------|-------|-------------|
| P0 | ~55 | Critical - Must work for basic functionality |
| P1 | ~60 | High - Important for production use |
| P2 | ~45 | Medium - Nice to have, improves UX |
| P3 | ~10 | Low - Polish, future enhancements |

### By Category

| Category | P0 | P1 | P2 | P3 | Total |
|----------|----|----|----|----|-------|
| HUD/Desktop | 15 | 15 | 10 | 3 | 43 |
| Task System | 8 | 8 | 10 | 0 | 26 |
| Orchestrator | 14 | 16 | 8 | 0 | 38 |
| CLI | 6 | 9 | 7 | 0 | 22 |
| LLM Providers | 3 | 5 | 4 | 1 | 13 |
| Core Tools | 8 | 4 | 4 | 0 | 16 |
| Configuration | 2 | 6 | 6 | 0 | 14 |
| Parallel/Worktrees | 0 | 6 | 4 | 0 | 10 |

### Current E2E Coverage

**Implemented (24 tests):**
- Visual/Layout: A1-A7 ✅
- Interactions: B1-B7 ✅
- Error Handling: D1-D6 ✅

**Pending:**
- Real-time Updates: C1-C10 (HUD-030 to HUD-040)
- Integration: E1-E5 (ORCH-001 to ORCH-044)
- Desktop Screenshots: (visual regression)
- Parallel Execution: (PAR-001 to PAR-013)
- Sandbox: (ORCH-080 to ORCH-082, CONF-030 to CONF-033)

---

## Next Steps

1. **Real-time Tests (P1)**: Implement C1-C10 covering HUD WebSocket messages
2. **Integration Tests (P1)**: Implement E1-E5 covering full Golden Loop
3. **CLI Tests (P1)**: Add integration tests for task CLI commands
4. **Orchestrator Tests (P1)**: Expand golden-loop-smoke.e2e.test.ts coverage
5. **Parallel Execution Tests (P1)**: Test worktree creation, isolation, and merging
6. **Sandbox Tests (P1)**: Test container/seatbelt execution and fallback behavior
7. **Safe Mode Tests (P1)**: Test preflight checks and self-healing behavior
8. **Provider Tests (P2)**: Add mock-based LLM provider tests
9. **Tool Tests (P0)**: Ensure all core tools have comprehensive coverage
10. **Session Replay Tests (P2)**: Test JSONL export/import and HTML generation

---

## 10. HillClimber / MAP Optimization

Overnight evolution and optimization for Terminal-Bench benchmarks.

### 10.1 Optimization Execution

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| HILL-001 | P1 | As a user, I can run overnight optimization | Start hillclimber, runs until stopped or complete |
| HILL-002 | P1 | As a user, I can see evolution progress | Live stats: generation, best score, time elapsed |
| HILL-003 | P1 | As a user, I can configure MAP parameters | Set population size, mutation rate, selection pressure |
| HILL-004 | P2 | As a user, I can pause/resume evolution | State saved, resumable from checkpoint |
| HILL-005 | P2 | As a user, I can view generation history | Historical scores, prompts, test results |
| HILL-006 | P2 | As a user, I can export best candidates | Save top N performers for deployment |

---

## 11. TestGen / Test Generation

Automated test generation and evolution system.

### 11.1 Test Generation

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| TGEN-001 | P1 | As a user, I can generate tests for a category | Specify TB category, generate tests |
| TGEN-002 | P1 | As a user, I can evolve test quality | Iteratively improve tests via scoring |
| TGEN-003 | P2 | As a user, I can validate generated tests | Run tests against known-good implementations |
| TGEN-004 | P2 | As a user, I can export test suites | Save to JSON format for Terminal-Bench |
| TGEN-005 | P3 | As a user, I can compare test coverage | Visualize coverage across categories |

---

## 12. ATIF / Agent Trajectories

Agent Trajectory Interchange Format for recording and analysis.

### 12.1 Trajectory Recording

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| ATIF-001 | P1 | As a user, I can record agent trajectories | Capture all actions, observations, thoughts |
| ATIF-002 | P1 | As a user, I can replay trajectories | Step through recorded session |
| ATIF-003 | P2 | As a user, I can export trajectories | Save as ATIF JSON, compatible with HuggingFace |
| ATIF-004 | P2 | As a user, I can import trajectories | Load external ATIF files for analysis |
| ATIF-005 | P2 | As a user, I can search trajectories | Query by action type, tool use, outcome |
| ATIF-006 | P3 | As a user, I can visualize trajectory graphs | Interactive timeline of agent actions |

---

## 13. Foundation Model Bridge

Apple on-device Foundation Model integration.

### 13.1 FM Inference

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| FM-001 | P0 | As a user, I can use Apple FM for inference | Local model responds to prompts |
| FM-002 | P1 | As a user, I can list available models | Query FM server for model capabilities |
| FM-003 | P1 | As a user, I can use guided generation | Constrained output with grammar/schema |
| FM-004 | P2 | As a user, I can track FM token usage | Count input/output tokens |
| FM-005 | P2 | As a user, I can configure FM parameters | Temperature, max tokens, stop sequences |
| FM-006 | P3 | As a user, I can use streaming responses | Real-time token generation |

---

## Task Tracking Matrix

This matrix maps user stories to OpenAgents tasks. Update this when creating or completing test tasks.

### HUD / Desktop Tests

| Story ID(s) | Task ID | Description | Status |
|-------------|---------|-------------|--------|
| HUD-001, HUD-002, HUD-010, HUD-012 | `oa-22017b` | E2E infra + smoke test | ✅ |
| HUD-010, HUD-011, HUD-012, HUD-014 | `oa-c21ca0` | Basic UI load + canvas | 📋 |
| HUD-060, HUD-061, HUD-062, HUD-063 | `oa-91f147` | Error handling + resilience | 📋 |
| HUD-020..024 | - | Node display tests | |
| HUD-030..034 | - | Real-time update tests | |
| HUD-050..051 | - | APM widget tests | |

Smoke coverage for HUD-001/002/010/012 lives in `e2e/tests/smoke/basic-smoke.spec.ts`.

### Task System Tests

| Story ID(s) | Task ID | Description | Status |
|-------------|---------|-------------|--------|
| TASK-001..022 | - | Task CLI tests | |

### Orchestrator Tests

| Story ID(s) | Task ID | Description | Status |
|-------------|---------|-------------|--------|
| ORCH-001..053 | - | Golden Loop tests | |
| ORCH-070..072 | - | Safe mode tests | |
| ORCH-080..082 | - | Sandbox tests | |

### CLI Tests

| Story ID(s) | Task ID | Description | Status |
|-------------|---------|-------------|--------|
| CLI-001..007 | - | Task CLI | |
| CLI-010..015 | - | MechaCoder CLI | |
| CLI-020..026 | - | Session CLI | |
| CLI-030..032 | - | Worktree CLI | |

### Core Infrastructure Tests

| Story ID(s) | Task ID | Description | Status |
|-------------|---------|-------------|--------|
| LLM-001..024 | - | LLM provider tests | |
| TOOL-001..033 | - | Core tool tests | |
| CONF-001..033 | - | Config validation tests | |
| PAR-001..013 | - | Parallel/worktree tests | |

### HillClimber / TestGen Tests

| Story ID(s) | Task ID | Description | Status |
|-------------|---------|-------------|--------|
| HILL-001..006 | - | MAP optimization tests | |
| TGEN-001..005 | - | Test generation tests | |

### ATIF / Trajectory Tests

| Story ID(s) | Task ID | Description | Status |
|-------------|---------|-------------|--------|
| ATIF-001..006 | - | Trajectory recording tests | |

### Foundation Model Tests

| Story ID(s) | Task ID | Description | Status |
|-------------|---------|-------------|--------|
| FM-001..006 | - | FM bridge tests | |

---

*Last updated: 2025-12-10*
