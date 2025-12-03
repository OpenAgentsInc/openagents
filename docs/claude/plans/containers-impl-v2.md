# Parallel Agent Execution with Isolated Git Worktrees

> Plan for running N agents on N isolated git checkouts simultaneously

## Problem

MechaCoder currently runs single-agent per repo. We need:
- 10 agents working on 10 isolated copies of the same repo
- No git conflicts between agents
- Each agent has its own branch/working tree
- Efficient disk usage (not 10 full clones)

## Solution: Git Worktrees + Container Sandbox

**Git worktrees** enable multiple working directories from a single `.git`:
```
repo/
├── .git/                    # Shared (objects, refs, config)
├── [main working tree]      # Primary checkout
└── .worktrees/              # Agent worktrees
    ├── agent-001/           # Worktree 1 (branch: agent/oa-abc123)
    ├── agent-002/           # Worktree 2 (branch: agent/oa-def456)
    └── ...                  # Up to N worktrees
```

**Benefits:**
- 46% disk savings vs full clones (shared object database)
- Each worktree has isolated index/staging
- Git operations are atomic per worktree
- Clean branch per agent for easy merge/revert

## Architecture

```
src/agent/orchestrator/
├── worktree.ts          # NEW: Worktree management service
├── parallel-runner.ts   # NEW: Run N agents in parallel
├── agent-lock.ts        # MODIFY: Per-worktree locking
└── orchestrator.ts      # MODIFY: Support parallel mode

src/sandbox/
├── (existing files)     # Container backend (already implemented)
└── index.ts             # Add worktree+container integration
```

## New Files

### File 1: `src/agent/orchestrator/worktree.ts`

Worktree lifecycle management using Effect.

```typescript
import { Effect } from "effect";
import * as S from "effect/Schema";

// Schema
export const WorktreeConfigSchema = S.Struct({
  taskId: S.String,
  sessionId: S.String,
  baseBranch: S.optionalWith(S.String, { default: () => "main" }),
  timeoutMs: S.optionalWith(S.Number, { default: () => 30 * 60 * 1000 }), // 30min
});

export interface WorktreeInfo {
  taskId: string;
  path: string;           // .worktrees/{taskId}
  branch: string;         // agent/{taskId}
  pid: number;
  createdAt: string;
}

// Core functions
export const createWorktree: (config: WorktreeConfig) => Effect<WorktreeInfo, WorktreeError>;
export const removeWorktree: (taskId: string) => Effect<void, WorktreeError>;
export const listWorktrees: () => Effect<WorktreeInfo[]>;
export const pruneStaleWorktrees: (maxAgeMs: number) => Effect<number>; // returns count removed
```

**Implementation details:**
```bash
# Create worktree
git worktree add -b agent/{taskId} .worktrees/{taskId} {baseBranch}

# Remove worktree
git worktree remove --force .worktrees/{taskId}
git branch -D agent/{taskId}

# List worktrees
git worktree list --porcelain

# Prune orphaned entries
git worktree prune
```

### File 2: `src/agent/orchestrator/parallel-runner.ts`

Coordinate N parallel agents.

```typescript
export interface ParallelRunnerConfig {
  repoPath: string;
  maxAgents: number;           // e.g., 10
  tasks: Task[];               // Tasks to distribute
  containerImage?: string;     // Optional: run in containers
  onAgentEvent?: (agentId: string, event: AgentEvent) => void;
}

export interface AgentSlot {
  id: string;
  worktree: WorktreeInfo;
  task: Task;
  status: "running" | "completed" | "failed";
  result?: AgentResult;
}

export const runParallelAgents: (config: ParallelRunnerConfig) => Effect<AgentSlot[], ParallelRunnerError>;
```

**Flow:**
1. Create N worktrees (one per task, up to maxAgents)
2. For each worktree, spawn agent process
3. Monitor all agents, collect results
4. Cleanup worktrees on completion
5. Return aggregated results

### File 3: `src/agent/orchestrator/worktree.test.ts`

```typescript
describe("Worktree Management", () => {
  test("creates isolated worktree");
  test("multiple worktrees can exist simultaneously");
  test("worktree has independent index");
  test("cleanup removes worktree and branch");
  test("prune removes stale worktrees");
});
```

## Modifications to Existing Files

### agent-lock.ts Changes

Current: Single lock per repo prevents ALL parallel execution.
New: Per-worktree locks allow parallel agents.

```typescript
// New lock structure
export interface WorktreeLock {
  worktreeId: string;
  pid: number;
  sessionId: string;
  createdAt: string;
}

// Lock file location changes
// OLD: .openagents/agent.lock (single file)
// NEW: .openagents/locks/{worktreeId}.lock (one per worktree)

export const acquireWorktreeLock: (worktreeId: string, sessionId: string) => Effect<boolean>;
export const releaseWorktreeLock: (worktreeId: string) => Effect<void>;
export const listActiveLocks: () => Effect<WorktreeLock[]>;
```

### ProjectConfig Schema Update

Add to `src/tasks/schema.ts`:

```typescript
const ParallelExecutionConfig = S.Struct({
  enabled: S.optionalWith(S.Boolean, { default: () => false }),
  maxAgents: S.optionalWith(S.Number, { default: () => 4 }),
  worktreeTimeout: S.optionalWith(S.Number, { default: () => 30 * 60 * 1000 }),
  useContainers: S.optionalWith(S.Boolean, { default: () => false }),
});

// Add to ProjectConfig
parallelExecution: S.optionalWith(ParallelExecutionConfig, {
  default: () => S.decodeUnknownSync(ParallelExecutionConfig)({}),
}),
```

## Container + Worktree Integration

When both containers AND worktrees are enabled:

```typescript
const runAgentInIsolation = (task: Task, config: ParallelRunnerConfig) =>
  Effect.gen(function* () {
    // 1. Create worktree
    const worktree = yield* createWorktree({
      taskId: task.id,
      sessionId: config.sessionId,
      baseBranch: config.baseBranch,
    });

    // 2. Optionally wrap in container
    if (config.containerImage) {
      return yield* runInContainer(
        ["bun", "src/agent/orchestrator/subagent.ts"],
        {
          image: config.containerImage,
          workspaceDir: path.resolve(worktree.path),  // Mount worktree
          memoryLimit: "4G",
          cpuLimit: 2,
        }
      );
    }

    // 3. Or run directly in worktree
    return yield* runSubagentInWorktree(worktree, task);
  }).pipe(
    Effect.ensuring(cleanupWorktree(task.id))  // Always cleanup
  );
```

## CLI Interface

New CLI commands for managing parallel agents:

```bash
# Check worktree status
bun src/agent/orchestrator/cli.ts worktrees list

# Run N parallel agents
bun src/agent/orchestrator/cli.ts parallel --max-agents 10

# Cleanup stale worktrees
bun src/agent/orchestrator/cli.ts worktrees prune --max-age 3600000
```

## Implementation Order

1. **`src/agent/orchestrator/worktree.ts`** - Core worktree management
2. **`src/agent/orchestrator/worktree.test.ts`** - Tests
3. **Modify `agent-lock.ts`** - Per-worktree locking
4. **`src/agent/orchestrator/parallel-runner.ts`** - Parallel coordination
5. **Modify `src/tasks/schema.ts`** - ParallelExecutionConfig
6. **CLI updates** - Add worktree/parallel commands

## Files Summary

| File | Action | Purpose |
|------|--------|---------|
| `src/agent/orchestrator/worktree.ts` | CREATE | Worktree lifecycle |
| `src/agent/orchestrator/worktree.test.ts` | CREATE | Tests |
| `src/agent/orchestrator/parallel-runner.ts` | CREATE | N-agent coordination |
| `src/agent/orchestrator/agent-lock.ts` | MODIFY | Per-worktree locks |
| `src/tasks/schema.ts` | MODIFY | ParallelExecutionConfig |

## Example Configuration

`.openagents/project.json`:
```json
{
  "version": 1,
  "projectId": "openagents",
  "defaultBranch": "main",
  "testCommands": ["bun test"],
  "sandbox": {
    "enabled": true,
    "image": "oven/bun:latest"
  },
  "parallelExecution": {
    "enabled": true,
    "maxAgents": 10,
    "worktreeTimeout": 1800000,
    "useContainers": true
  }
}
```

## Merge Strategy (Auto-Select)

Based on agent count, automatically select the optimal merge strategy:

### Few Agents (≤4): Direct Commit to Main
```
Agent → worktree → commit to agent/{taskId} branch → fast-forward merge to main → push
```
- Fastest possible flow
- No PR overhead
- Lock main briefly during merge
- Retry on conflict (rebase and retry)

### Many Agents (5-50): Branch + Local Merge Queue
```
Agent → worktree → commit to agent/{taskId} branch → queue for merge
Merge Coordinator → processes queue sequentially → merges to main → pushes
```
- Serialized merges avoid conflicts
- Single push to remote (batched)
- Agents don't wait for merge

### Large Scale (50+): Branch + PR Flow
```
Agent → worktree → commit to agent/{taskId} branch → create PR
External process → review/merge PRs
```
- GitHub rate limit friendly (uses PR API sparingly)
- PRs can be batched
- Supports external review if needed

### Configuration
```typescript
export const MergeStrategySchema = S.Literal("auto", "direct", "queue", "pr");

// In ParallelExecutionConfig:
mergeStrategy: S.optionalWith(MergeStrategySchema, { default: () => "auto" as const }),
mergeThreshold: S.optionalWith(S.Number, { default: () => 4 }), // agents before switching to queue
prThreshold: S.optionalWith(S.Number, { default: () => 50 }),   // agents before switching to PR
```

### Implementation in `parallel-runner.ts`
```typescript
const selectMergeStrategy = (config: ParallelRunnerConfig): MergeStrategy => {
  if (config.mergeStrategy !== "auto") return config.mergeStrategy;

  const agentCount = config.tasks.length;
  if (agentCount <= config.mergeThreshold) return "direct";
  if (agentCount <= config.prThreshold) return "queue";
  return "pr";
};
```

## Key Considerations

1. **Disk space**: Each worktree duplicates working files (~50MB typical), but shares .git
2. **Git operations**: `git fetch`, `git gc` affect all worktrees (run from main tree)
3. **Branch conflicts**: Each agent gets unique branch `agent/{taskId}`
4. **Cleanup on crash**: Use `git worktree prune` on startup to clean orphans
5. **Container benefit**: Adds process/resource isolation on top of filesystem isolation
6. **Rate limits**: PR flow batches GitHub API calls; direct/queue flows minimize remote pushes
