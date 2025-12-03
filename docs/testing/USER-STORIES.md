# OpenAgents User Stories for Testing

> Comprehensive catalog of user stories across all system components.
> Priority: P0 (Critical), P1 (High), P2 (Medium), P3 (Low)

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
| ORCH-014 | P2 | As MechaCoder, I fallback on subagent failure | fallbackToMinimal config respected |

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

### 3.6 Error Recovery

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| ORCH-050 | P0 | As MechaCoder, I emit error events | error message with phase and details |
| ORCH-051 | P1 | As MechaCoder, I track consecutive failures | After N failures, mark task blocked |
| ORCH-052 | P1 | As MechaCoder, I include retry context | Previous failure info in retry prompt |
| ORCH-053 | P2 | As MechaCoder, I gracefully handle stop signals | Clean shutdown on SIGTERM/SIGINT |

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
| CLI-010 | P0 | As a user, I can run `bun run mechacoder` | Executes one task (do-one-task) |
| CLI-011 | P0 | As a user, I can run `bun run mechacoder:overnight` | Runs until maxTasks or maxRuntime |
| CLI-012 | P1 | As a user, I can run `--cc-only` mode | Forces Claude Code subagent |
| CLI-013 | P1 | As a user, I can run `--legacy` mode | Uses legacy minimal agent |
| CLI-014 | P2 | As a user, I can run `mechacoder:parallel` | Parallel task execution |

### 4.3 Session CLI

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| CLI-020 | P1 | As a user, I can run `session:list` | Lists recent sessions |
| CLI-021 | P1 | As a user, I can run `session:show` | Shows session details |
| CLI-022 | P2 | As a user, I can run `session:search` | Searches session content |
| CLI-023 | P2 | As a user, I can run `session:by-task` | Finds sessions for a task |

---

## 5. LLM Provider Integration

Provider abstraction for multiple AI backends.

### 5.1 Provider Selection

| ID | Priority | User Story | Acceptance Criteria |
|----|----------|------------|---------------------|
| LLM-001 | P0 | As the system, I use OpenRouter by default | Default provider works out of box |
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

---

## Testing Coverage Summary

### By Priority

| Priority | Count | Description |
|----------|-------|-------------|
| P0 | ~50 | Critical - Must work for basic functionality |
| P1 | ~45 | High - Important for production use |
| P2 | ~35 | Medium - Nice to have, improves UX |
| P3 | ~10 | Low - Polish, future enhancements |

### By Category

| Category | P0 | P1 | P2 | P3 | Total |
|----------|----|----|----|----|-------|
| HUD/Desktop | 15 | 15 | 10 | 3 | 43 |
| Task System | 8 | 8 | 10 | 0 | 26 |
| Orchestrator | 12 | 12 | 6 | 0 | 30 |
| CLI | 6 | 5 | 3 | 0 | 14 |
| LLM Providers | 3 | 5 | 4 | 1 | 13 |
| Core Tools | 8 | 4 | 4 | 0 | 16 |
| Configuration | 2 | 4 | 4 | 0 | 10 |

### Current E2E Coverage

**Implemented (24 tests):**
- Visual/Layout: A1-A7 ✅
- Interactions: B1-B7 ✅
- Error Handling: D1-D6 ✅

**Pending:**
- Real-time Updates: C1-C10 (HUD-030 to HUD-040)
- Integration: E1-E5 (ORCH-001 to ORCH-044)
- Desktop Screenshots: (visual regression)

---

## Next Steps

1. **Real-time Tests (P1)**: Implement C1-C10 covering HUD WebSocket messages
2. **Integration Tests (P1)**: Implement E1-E5 covering full Golden Loop
3. **CLI Tests (P1)**: Add integration tests for task CLI commands
4. **Orchestrator Tests (P1)**: Expand golden-loop-smoke.e2e.test.ts coverage
5. **Provider Tests (P2)**: Add mock-based LLM provider tests
6. **Tool Tests (P0)**: Ensure all core tools have comprehensive coverage
