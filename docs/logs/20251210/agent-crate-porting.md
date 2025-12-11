# Agent Crate Porting Progress

**Date:** 2025-12-10
**Status:** In Progress (Phase 3)

## Overview

Porting the TypeScript `src/agent/orchestrator/` module to Rust `crates/agent/`.

## Modules Ported (25 total)

| Rust Module | TypeScript Source | Tests | Description |
|-------------|-------------------|-------|-------------|
| `agent_lock.rs` | `agent-lock.ts` | 9 | Agent locking to prevent concurrent runs |
| `agent_loop.rs` | N/A (new) | 6 | Agent loop execution types |
| `checkpoint.rs` | `checkpoint.ts` | 6 | Orchestrator checkpoint/crash recovery |
| `claude_code_detector.rs` | `claude-code-detector.ts` | 5 | Detect Claude Code CLI availability |
| `claude_code_mcp.rs` | `claude-code-mcp.ts` | 8 | MCP tools for Claude Code integration |
| `decompose.rs` | `decompose.ts` | 2 | Task decomposition heuristics |
| `error.rs` | N/A | 0 | AgentError types |
| `git.rs` | `git-helpers.ts` | 2 | Git operations (merge, commits, etc.) |
| `golden_loop_fixture.rs` | `golden-loop-fixture.ts` | 4 | Test fixture creation |
| `init_script.rs` | `init-script.ts` | 7 | Run `.openagents/init.sh` |
| `install_deps.rs` | `install-deps.ts` | 6 | Dependency installation helper |
| `orchestrator.rs` | `orchestrator.ts` | 3 | Main orchestrator state machine |
| `progress.rs` | `progress.ts` | 6 | Progress file markdown format |
| `recovery.rs` | `recovery.ts` | 4 | Two-phase commit crash recovery |
| `session.rs` | N/A | 3 | Session management, JSONL logging |
| `step_results.rs` | `step-results.ts` | 6 | Step memoization for replay |
| `subagent.rs` | `subagent.ts` | 3 | Subagent completion detection |
| `tool_log_buffer.rs` | `tool-log-buffer.ts` | 7 | Tool output chunk accumulation |
| `types.rs` | `types.ts` | 5 | Core types (Task, Subtask, Events) |
| `verification.rs` | `verification-*.ts` | 8 | Verification pipeline (typecheck, tests) |
| `worktree.rs` | `worktree.ts` | 6 | Git worktree management |
| `worktree_guards.rs` | `worktree-guards.ts` | 6 | File operation boundary enforcement |
| `worktree_runner.rs` | `worktree-runner.ts` | 4 | Worktree + orchestrator integration |

**Total Tests:** 128 passing

## Remaining TypeScript Files

| File | Complexity | Notes |
|------|------------|-------|
| `claude-code-subagent.ts` | High | Full Claude Code SDK integration |
| `parallel-runner.ts` | High | Parallel orchestrator with worktrees |
| `sandbox-runner.ts` | Medium | Docker sandbox execution |
| `subagent-router.ts` | Medium | Route between FM/Claude Code |
| `cli.ts` | Low | CLI entry point |

## Dependencies

The agent crate depends on:
- `llm` - LLM provider traits
- `tools` - Tool definitions
- `atif` - Transcript types

## Blocking Issues

TypeScript files cannot be deleted until all callers are migrated:
- `src/healer/` imports types
- `src/bench/` imports orchestrator
- `src/agent/overnight.ts` imports orchestrator
- `src/atif/` imports types

## Next Steps

1. Port `subagent-router.ts` - routing logic between FM and Claude Code
2. Port `sandbox-runner.ts` - sandbox execution
3. Port `parallel-runner.ts` - parallel orchestration
4. Port `claude-code-subagent.ts` - full Claude Code integration

## Architecture Notes

The Rust port follows the same architecture as TypeScript:
- **Orchestrator**: Task selection, decomposition, verification
- **Subagent**: Minimal coding agent for one subtask
- **Worktree**: Isolation for parallel execution
- **Recovery**: Two-phase commit crash recovery

Key differences from TypeScript:
- No Effect library - using Result/Option
- Sync APIs where possible (git operations)
- Traits for LLM providers instead of Effect services
