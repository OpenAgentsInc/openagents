# Flawed Context Invariants

1. **Unambiguous Identity & Generational Handles (`flawed/ids.coax`, `#authority`).**
   - `FlawedSessionId`, `FlawedAgentId`, and `FlawedToolUseId` are distinct nominal structs validated at runtime parsing boundaries (`FlawedSessionId`: 36-byte canonical lowercase UUID; `FlawedAgentId`: `/^a(?:.+-)?[0-9a-f]{16}$/`; `FlawedToolUseId`: `[A-Za-z0-9_-]{1,64}`).
   - Process-local live agent/task slots use generational `Handle(T)` (`coax/lib/handle.coax`) so stale references after removal/reuse are rejected.
   - Composite skill keys (`${agentId ?? ''}:${skillName}`) keep main-thread and subagent invoked skills disjoint (`coax mutate flawed/ids.coax --root-only`: 0 surviving mutants).
2. **Three-Lifetime State Discipline (`flawed/state.coax`).**
   - `FlawedProcessEnv` (`original_cwd`, `project_root`, `session_id`) is immutable after startup; switching worktrees mutates `FlawedSessionState.worktree_cwd` only.
   - All `FlawedSessionState` mutators enforce exact no-op bailouts (`Object.is` semantics): identical writes never increment `gen`, never flip `dirty_mask`, and never emit events.
   - `FlawedTurnState` subagent forks isolate `agent_id`, reset `step_count`/`aborted`, and cap `max_steps <= parent.max_steps`.
3. **Bounded File Awareness & Partial-View Guard (`flawed/file_cache.coax`).**
   - Every key is normalized (`\` → `/`, `//` collapsed, `.`/`..` resolved, root escape refused).
   - Dual LRU caps enforce both max entry count and max byte slab with overlap-safe slab compaction; empty files carry logical weight `1`; oversized files degrade to metadata-only `is_partial_view = true`.
   - `flawed_fc_check_edit_safe` refuses unread files, partial views (`CLAUDE.md` / sliced reads), and stale disk state (`dev`, `ino`, `mtime_ns`, or SHA-256 mismatch).
   - `FlawedMemoryPathSet` tracks loaded memory paths independently so LRU eviction never triggers duplicate re-injection.
4. **Fail-Closed Concurrency-Barrier Scheduler (`flawed/scheduler.coax`, `#authority`).**
   - Work slots default to `concurrency_safe = false` (serial barrier).
   - At most `max_concurrent` concurrency-safe slots run in parallel; serial slots wait for `running == 0`, execute exclusively (`serial_active = true`), and block subsequent slots behind them until settled (`coax mutate flawed/scheduler.coax --root-only`: 0 surviving mutants).
5. **Fail-Closed Tool Contract & Prompt-Cache-Stable Pool (`flawed/tool.coax`, `#authority`).**
   - `flawed_tool_defaults` enforces fail-closed defaults (`concurrency_safe = false`, `read_only = false`, `interrupt_behavior = BLOCK`); mutating invocations can never be classified concurrency-safe.
   - `flawed_is_unc_path` detects `\\...` and `//...` so `validateInput` never triggers outbound SMB/NTLM probes prior to permission checks.
   - `flawed_assemble_tool_pool` applies `simple_mode`, `repl_mode`, and blanket deny rules (exact names + strict `mcp__<server>__` boundaries), keeps built-ins in a sorted contiguous prefix `[0..<builtin_count]` ahead of sorted MCP tools for global prompt-cache stability, and blocks MCP name-shadowing of built-ins (`coax mutate flawed/tool.coax --root-only`: 0 surviving mutants).
6. **GlobTool Contract & Dual Projection (`flawed/glob_tool.coax`).**
   - Separates LLM `tool_result` projection (`"No files found"` + truncation hints) from transcript search text (`extractSearchText`, exact visible paths with zero chrome) and relativizes paths under `cwd` to save tokens.
