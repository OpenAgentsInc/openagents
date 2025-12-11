# Agent Crate Porting Progress

**Date:** 2025-12-10
**Status:** Complete (Phase 3 - Core Orchestrator Module)

## Overview

Ported the TypeScript `src/agent/orchestrator/` module to Rust `crates/agent/`.

The core orchestrator module is **complete**. CLI entry points (`overnight.ts`, `do-one-task.ts`, `overnight-parallel.ts`) are wrappers around these modules and are lower priority per the plan.

## Modules Ported (29 total)

| Rust Module | TypeScript Source | Tests | Description |
|-------------|-------------------|-------|-------------|
| `agent_lock.rs` | `agent-lock.ts` | 9 | Agent locking to prevent concurrent runs |
| `agent_loop.rs` | N/A (new) | 6 | Agent loop execution types |
| `checkpoint.rs` | `checkpoint.ts` | 6 | Orchestrator checkpoint/crash recovery |
| `claude_code_detector.rs` | `claude-code-detector.ts` | 5 | Detect Claude Code CLI availability |
| `claude_code_mcp.rs` | `claude-code-mcp.ts` | 8 | MCP tools for Claude Code integration |
| `claude_code_subagent.rs` | `claude-code-subagent.ts` | 14 | Claude Code SDK integration (stub) |
| `decompose.rs` | `decompose.ts` | 2 | Task decomposition heuristics |
| `error.rs` | N/A | 0 | AgentError types |
| `git.rs` | `git-helpers.ts` | 2 | Git operations (merge, commits, etc.) |
| `golden_loop_fixture.rs` | `golden-loop-fixture.ts` | 4 | Test fixture creation |
| `init_script.rs` | `init-script.ts` | 7 | Run `.openagents/init.sh` |
| `install_deps.rs` | `install-deps.ts` | 6 | Dependency installation helper |
| `orchestrator.rs` | `orchestrator.ts` | 3 | Main orchestrator state machine |
| `parallel_runner.rs` | `parallel-runner.ts` | 13 | Parallel agent orchestration with worktrees |
| `progress.rs` | `progress.ts` | 6 | Progress file markdown format |
| `recovery.rs` | `recovery.ts` | 4 | Two-phase commit crash recovery |
| `sandbox_runner.rs` | `sandbox-runner.ts` | 12 | Docker/podman sandbox execution |
| `session.rs` | N/A | 3 | Session management, JSONL logging |
| `step_results.rs` | `step-results.ts` | 6 | Step memoization for replay |
| `subagent.rs` | `subagent.ts` | 3 | Subagent completion detection |
| `subagent_router.rs` | `subagent-router.ts` | 14 | Route between Claude Code, FM, Minimal |
| `tool_log_buffer.rs` | `tool-log-buffer.ts` | 7 | Tool output chunk accumulation |
| `types.rs` | `types.ts` | 5 | Core types (Task, Subtask, Events) |
| `verification.rs` | `verification-*.ts` | 8 | Verification pipeline (typecheck, tests) |
| `worktree.rs` | `worktree.ts` | 6 | Git worktree management |
| `worktree_guards.rs` | `worktree-guards.ts` | 6 | File operation boundary enforcement |
| `worktree_runner.rs` | `worktree-runner.ts` | 4 | Worktree + orchestrator integration |

**Total Tests:** 180 passing

## Remaining TypeScript Files (Out of Scope)

| File | Type | Notes |
|------|------|-------|
| `overnight.ts` | CLI wrapper | Uses ported modules (orchestrator, parallel_runner) |
| `overnight-parallel.ts` | CLI wrapper | Uses ported modules (parallel_runner) |
| `do-one-task.ts` | CLI wrapper | Uses ported modules (orchestrator) |
| `loop.ts` | Business logic | Already implemented in `agent_loop.rs` |
| `prompts.ts` | Constants | Git conventions, system prompts (minimal) |

These files are either CLI entry points (lower priority) or already have Rust equivalents.

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

## Architecture Notes

The Rust port follows the same architecture as TypeScript:
- **Orchestrator**: Task selection, decomposition, verification
- **Subagent**: Minimal coding agent for one subtask
- **SubagentRouter**: Routes to Claude Code, FM, or Minimal based on task
- **ClaudeCodeSubagent**: Claude Code Agent SDK integration (stub - needs FFI)
- **Worktree**: Isolation for parallel execution
- **ParallelRunner**: N agents on N isolated worktrees
- **Sandbox**: Container-based command execution
- **Recovery**: Two-phase commit crash recovery

Key differences from TypeScript:
- No Effect library - using Result/Option
- Sync APIs where possible (git operations)
- Traits for LLM providers instead of Effect services
- Claude Code SDK integration is a stub (needs FFI or native SDK)

## Commits Today

1. `Add agent crate: port orchestrator module to Rust (24 modules, 128 tests)`
2. `Add subagent_router.rs: route between Claude Code, FM, and minimal`
3. `Add sandbox_runner.rs: container-based command execution`
4. `Add parallel_runner.rs: N agents on N isolated worktrees`
5. `Add claude_code_subagent.rs: Claude Code SDK integration types`
