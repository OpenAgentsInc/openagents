# 1400 MechaCoder Sandbox vs Worktree Audit

## Executive Summary

MechaCoder's parallel execution uses **git worktrees** for isolation but the **container sandbox** implementation is disabled. These are complementary systems, not alternatives, but only worktrees are currently active.

## Architecture Overview

### Two Isolation Mechanisms Exist

| System | Purpose | Status |
|--------|---------|--------|
| **Git Worktrees** | File/branch isolation | **ACTIVE** |
| **Container Sandbox** | Execution isolation | **DISABLED** |

### Key Files

**Worktree System (ACTIVE):**
- `src/agent/orchestrator/worktree.ts` - Worktree management (create, remove, validate, repair)
- `src/agent/orchestrator/worktree-runner.ts` - Single-task worktree runner
- `src/agent/orchestrator/parallel-runner.ts` - Multi-agent parallel execution
- `src/agent/overnight-parallel.ts` - CLI entry point

**Sandbox System (DISABLED):**
- `src/sandbox/index.ts` - Container abstraction layer
- `src/sandbox/macos-container.ts` - Apple Container CLI backend
- `src/sandbox/detect.ts` - Backend auto-detection
- `src/agent/orchestrator/sandbox-runner.ts` - Orchestrator integration

## Current State Analysis

### 1. Project Configuration

`.openagents/project.json` shows sandbox is explicitly disabled:

```json
{
  "sandbox": {
    "enabled": false,
    "backend": "auto",
    "memoryLimit": "8G",
    "timeoutMs": 300000
  }
}
```

### 2. Parallel Execution Config

`src/tasks/schema.ts:177` - `useContainers` defaults to false:

```typescript
useContainers: S.optionalWith(S.Boolean, { default: () => false }),
```

All callers pass `useContainers: false`:
- `src/agent/do-one-task.ts:774`
- `src/agent/overnight.ts:502`

### 3. Sandbox-Runner Integration

The orchestrator (`src/agent/orchestrator/orchestrator.ts:278-293`) properly builds sandbox config, but it's gated:

```typescript
const sandboxRunnerConfig: SandboxRunnerConfig | undefined = config.sandbox
  ? { sandboxConfig: config.sandbox, cwd: config.cwd, ... }
  : undefined;
```

When `sandbox.enabled: false` in project.json, `config.sandbox` is undefined and no sandboxed verification runs.

### 4. Worktree Usage

The parallel runner creates worktrees but never sets `containerImage`:

```typescript
// parallel-runner.ts:65
containerImage?: string;  // Never set by any caller
```

## Why This Happened

1. **Design Intent Was Correct**: Sandbox was built for sandboxed test/typecheck execution, worktrees for git isolation. They're complementary.

2. **Incomplete Integration**: The `parallel-runner.ts:65` has `containerImage` param but no callers set it.

3. **Explicit Disable**: Project config has `sandbox.enabled: false`, so even the existing integration is bypassed.

4. **Schema Defaults**: `ParallelExecutionConfig.useContainers` defaults to `false` with no mechanism to enable it from CLI.

## Missing Connections

```
overnight-parallel.ts
       |
       v
   parallel-runner.ts
       |
       +---> worktree.ts [ACTIVE - creates .worktrees/oa-xxx/]
       |
       +---> containerImage [NEVER SET]
       |
       v
   orchestrator.ts
       |
       +---> sandbox-runner.ts [GATED by config.sandbox]
                  |
                  +---> src/sandbox/ [NEVER REACHED]
```

## Recommendations

### Option A: Enable Sandbox for Verification Only

The sandbox system is designed for verification (tests/typecheck), not full agent execution. To use it:

1. Set `sandbox.enabled: true` in `.openagents/project.json`
2. Worktrees still handle file isolation
3. Tests run in containers for additional safety

### Option B: Enable Container-in-Worktree Mode

For full containerized agents within worktrees:

1. Wire up `containerImage` parameter in `overnight-parallel.ts`
2. Pass it through `parallel-runner.ts` → `orchestrator.ts`
3. Have sandbox-runner wrap all subagent commands

### Option C: Keep Current Setup (Worktrees Only)

Worktrees provide sufficient isolation for most use cases:
- Each agent gets its own branch and working directory
- Changes are merged sequentially after completion
- No container overhead

## Current Worktree Behavior

When running `bun run mechacoder:parallel`:

1. Creates `.worktrees/oa-{taskId}/` directory per agent
2. Each worktree has its own `agent/{taskId}` branch
3. Runs `bun install` in each worktree
4. Orchestrator runs in worktree context
5. Changes merged to main after completion
6. Worktrees cleaned up

This provides **git-level isolation** but commands run on the **host** (not sandboxed).

## Files Read During Audit

- `src/agent/orchestrator/sandbox-runner.ts`
- `src/agent/orchestrator/worktree-runner.ts`
- `src/agent/orchestrator/worktree.ts`
- `src/agent/orchestrator/parallel-runner.ts`
- `src/agent/overnight-parallel.ts`
- `src/sandbox/index.ts`
- `src/tasks/schema.ts`
- `.openagents/project.json`

## Conclusion

The sandbox implementation is complete and tested but disabled via config. Worktrees are the active isolation mechanism. To enable container sandboxing, simply set `sandbox.enabled: true` in project config - the integration code exists but is bypassed.
