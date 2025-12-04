# Durable Execution Lessons for MechaCoder

> **Summary**: This document analyzes durable execution patterns from systems like Temporal, Restate, and DBOS, and applies them to MechaCoder's autonomous agent architecture. Based on Jack Vanlightly's articles and our codebase analysis.

## Table of Contents

1. [Introduction](#1-introduction)
2. [Core Concepts](#2-core-concepts)
3. [MechaCoder Architecture Analysis](#3-mechacoder-architecture-analysis)
4. [Identified Gaps](#4-identified-gaps)
5. [Recommended Improvements](#5-recommended-improvements)
6. [Implementation Patterns](#6-implementation-patterns)
7. [Framework Comparison](#7-framework-comparison)
8. [Future Directions](#8-future-directions)

---

## 1. Introduction

### What is Durable Execution?

Durable execution is a programming model where long-running workflows can survive process crashes, restarts, and failures. The key insight is that by persisting execution state at key points, a workflow can resume from where it left off rather than starting over.

Systems implementing this pattern include:
- **Temporal**: Workflow-as-code with automatic replay
- **Restate**: Durable functions with virtual objects
- **DBOS**: Database-backed durable execution
- **Resonate**: Distributed async/await

### Why It Matters for MechaCoder

MechaCoder runs autonomously, often overnight without supervision. Unlike interactive coding assistants where a human can intervene on failure, MechaCoder must:
- Self-recover from crashes
- Avoid duplicate work on restart
- Maintain consistency across sessions
- Bridge context windows (Claude Code sessions)

The durable execution model provides proven patterns for these challenges.

### Source Material

This analysis draws from:
- Jack Vanlightly's "Demystifying Determinism in Durable Execution" (Nov 2025)
- Jack Vanlightly's "The Durable Function Tree, Part 1" (Dec 2025)
- Anthropic's "Effective Harnesses for Long-Running Agents"
- Analysis of MechaCoder codebase (`src/agent/`, `src/healer/`, `src/tasks/`)

---

## 2. Core Concepts

### 2.1 Determinism in Control Flow

**Key insight**: Determinism is only required for *control flow*, not for side effects.

In durable execution, recovery works by re-executing the function from the beginning, using memoized results for completed steps. If control flow depends on non-deterministic values, replays can take different paths than the original execution.

**Non-deterministic operations that affect control flow:**
- `Date.now()` / timestamps
- Random number generation
- Database queries (state may have changed)
- File system reads (files may have changed)
- External API calls

**Solution**: Treat non-deterministic operations as "durable steps" whose results are recorded and replayed.

```typescript
// BAD: Non-deterministic control flow
if (Date.now() > deadline) {
  return "timeout";
}

// GOOD: Durable step for time check
const now = yield* durableStep("check_time", () => Date.now());
if (now > deadline) {
  return "timeout";
}
```

### 2.2 Idempotency of Side Effects

Side effects (the actual work being done) should be **idempotent** - safe to execute multiple times with the same result.

If a step completes but the system crashes before persisting its completion, the step will be re-executed on recovery. The operation must tolerate this duplication.

**Idempotency strategies:**
1. **Natural idempotency**: `git push` to same branch/SHA is idempotent
2. **Idempotency keys**: Check if operation already done before executing
3. **Upsert patterns**: Update-or-insert instead of insert-only
4. **Conditional execution**: Only execute if precondition met

### 2.3 The Durable Function Tree

Durable functions can call other durable functions, forming a tree structure:

```
┌─────────────────────────────────────────┐
│         Root Workflow (Orchestrator)     │
└─────────────────────────────────────────┘
                    │
        ┌───────────┼───────────┐
        │           │           │
        ▼           ▼           ▼
   ┌─────────┐ ┌─────────┐ ┌─────────┐
   │ Subtask │ │ Subtask │ │ Subtask │
   │    1    │ │    2    │ │    3    │
   └─────────┘ └─────────┘ └─────────┘
        │
        ▼
   ┌─────────┐
   │ Healer  │
   └─────────┘
```

**Benefits of tree structure:**
- **Fault isolation**: Failures in subtrees don't cascade
- **Independent retry**: Can retry a subtask without re-running siblings
- **Parallel execution**: Siblings can run concurrently

### 2.4 Local vs Remote Context

Vanlightly distinguishes two types of operations:

| Type | Description | Can Suspend? | Examples |
|------|-------------|--------------|----------|
| **Local-context** | Runs in-process | No - function must stay active | DB queries, file I/O, bash commands |
| **Remote-context** | Runs independently | Yes - caller can suspend | Child workflows, external services, timers |

This distinction is crucial for understanding which operations can be "resumed" vs. which must run to completion.

### 2.5 Durable Promises

A durable promise is a write-once container whose value persists across failures:

```typescript
// Conceptual model
interface DurablePromise<T> {
  id: string;
  status: "pending" | "resolved" | "rejected";
  value?: T;
  error?: Error;
}
```

When a durable function awaits a promise:
1. If promise is resolved, return value immediately (replay)
2. If promise is pending, suspend and wait
3. If promise is rejected, propagate error

---

## 3. MechaCoder Architecture Analysis

### 3.1 Current Architecture

MechaCoder follows a two-layer architecture (similar to Temporal's workflow/activity model):

**Orchestrator (workflow layer)**
- Reads `project.json` and `tasks.jsonl`
- Selects and claims tasks
- Decomposes tasks into subtasks
- Coordinates verification
- Commits and updates state

**Subagents (activity layer)**
- Execute individual subtasks
- Run as Claude Code sessions
- Have limited scope and context

**Healer (recovery subagent)**
- Invoked on failure scenarios
- Executes recovery "spells"
- Linked to orchestrator via ATIF trajectories

### 3.2 What MechaCoder Does Well

**Function tree structure**: The orchestrator → subagent → healer hierarchy provides natural fault isolation. A failing subtask doesn't crash the orchestrator.

**Remote-context for subagents**: Claude Code sessions have `sessionId` for resumption. This is effectively a durable promise - the work continues independently and can be awaited later.

**Progress files for context bridging**: `progress.md` and subtask JSON files persist state across sessions, enabling context transfer between context windows.

**Worktree isolation for parallel execution**: The parallel runner uses git worktrees to isolate concurrent agents, preventing interference.

### 3.3 How MechaCoder Maps to Durable Execution

| Durable Execution Concept | MechaCoder Implementation |
|--------------------------|---------------------------|
| Workflow | Orchestrator (`orchestrator.ts`) |
| Activities | Subagent executions (Claude Code) |
| Durable promises | Claude Code session IDs |
| Step memoization | Subtask status in JSON files |
| Checkpoints | Implicit via progress.md |
| Compensation/rollback | Healer spells |
| Event log | ATIF trajectories |

---

## 4. Identified Gaps

### 4.1 Idempotency Issues

#### Commit/Task-Update Race Condition

**Location**: `src/agent/orchestrator/orchestrator.ts:800-834`

```typescript
// Phase 6: Commit
const sha = yield* createCommit(taskId, message);
// [CRASH WINDOW] - commit exists but task not updated
// Phase 8: Update Task
yield* updateTask({ id: taskId, status: "closed" });
```

**Risk**: If crash occurs between commit and task update, restart will find task still `in_progress` and may create duplicate commits.

#### Healer Spell Duplication

**Location**: `src/healer/spells/blocked.ts`, `src/healer/spells/progress.ts`

- `mark_task_blocked_with_followup`: May create duplicate follow-up tasks
- `update_progress_with_guidance`: May append duplicate sections to progress.md

#### Usage Record Duplication

**Location**: `src/usage/store.ts`

Usage records are appended without idempotency checks. Crash and restart could create duplicate entries.

### 4.2 Non-Deterministic Control Flow

Operations whose results affect control flow but aren't persisted:

| Operation | Non-Determinism Source | Impact |
|-----------|----------------------|--------|
| `pickNextTask()` | Task status can change | Different task selected on restart |
| `runInitScript()` | Git status, test results | Different init outcome |
| `decomposeTask()` | LLM output varies | Different subtasks generated |
| `buildHealerContext()` | Error output, git state | Different heuristics |

### 4.3 No Formal Checkpointing

While state IS persisted (progress.md, subtask JSON), there's no formal checkpoint mechanism:
- No atomic checkpoint writes
- No checkpoint validation on recovery
- No phase-specific resume logic

### 4.4 Missing Replay Semantics

The system can resume via Claude Code session, but the orchestrator itself doesn't support replay:
- Completed steps are re-executed (not replayed from memoized results)
- Non-deterministic operations return fresh values
- No replay mode detection

### 4.5 No Compensation/Rollback

When verification fails after code changes:
- The Healer attempts recovery
- But there's no automatic rollback of completed steps
- Partial work may be left in broken state

---

## 5. Recommended Improvements

### 5.1 Priority Matrix

| Priority | Issue | Impact | Effort |
|----------|-------|--------|--------|
| P0 | Commit/task-update race | Duplicate commits | Medium |
| P1 | Healer spell idempotency | Duplicate tasks/sections | Low |
| P1 | Orchestrator checkpoints | Crash recovery | Medium |
| P1 | Usage record idempotency | Inflated metrics | Low |
| P2 | Step memoization | Deterministic replay | High |
| P2 | ATIF for recovery | Single source of truth | Medium |
| P2 | Parallel runner coordination | Resume after crash | Medium |
| P3 | Saga pattern | Automatic rollback | High |

### 5.2 Quick Wins (P0-P1)

#### Two-Phase Commit for Tasks

```typescript
// Phase 1: Mark intent
yield* updateTask({ id: taskId, status: "commit_pending" });

// Phase 2: Execute and verify
const sha = yield* createCommit(message);
yield* updateTask({ id: taskId, status: "closed", commits: [sha] });
```

Recovery logic checks for `commit_pending` tasks and resolves them.

#### Idempotency Keys

```typescript
interface UsageRecord {
  // ... existing fields
  idempotencyKey: string;  // e.g., `${sessionId}:${subtaskId}:${stepIndex}`
}

// Before appending, check for existing key
const exists = records.some(r => r.idempotencyKey === newRecord.idempotencyKey);
if (exists) return { appended: false, reason: "duplicate" };
```

#### Healer Spell Guards

```typescript
// In mark_task_blocked_with_followup
const existingFollowup = yield* findTaskByParentAndScenario(taskId, scenario);
if (existingFollowup) {
  return { success: true, changesApplied: false, reason: "Follow-up already exists" };
}
```

### 5.3 Medium-Term (P2)

#### Step Result Memoization

```typescript
const durableStep = <A>(stepId: string, operation: () => Effect<A>) =>
  Effect.gen(function* () {
    const memoized = yield* getMemoizedResult(sessionId, stepId);
    if (Option.isSome(memoized)) {
      return memoized.value as A;
    }
    const result = yield* operation();
    yield* memoizeResult(sessionId, stepId, result);
    return result;
  });
```

#### ATIF-Based Recovery

Extend ATIF trajectories to support:
- Step status (pending, executing, completed, failed, replayed)
- Checkpoint markers
- Recovery info (recovered_from_session, recovered_at_step)

Write steps incrementally during execution, not just at the end.

### 5.4 Long-Term (P3)

#### Saga Pattern

Model the orchestrator as a saga with compensation actions:

```typescript
const orchestratorSaga: SagaStep[] = [
  { name: "select_task", action: pickTask, compensation: releaseTask },
  { name: "execute", action: runSubtasks, compensation: revertChanges },
  { name: "commit", action: createCommit, compensation: resetCommit },
  // ...
];
```

On failure, run compensations in reverse order.

---

## 6. Implementation Patterns

### 6.1 Atomic File Writes

Always use temp file + rename for atomic persistence:

```typescript
const atomicWrite = (path: string, content: string) =>
  Effect.gen(function* () {
    const tempPath = `${path}.tmp.${Date.now()}`;
    yield* fs.writeFileString(tempPath, content);
    yield* fs.rename(tempPath, path);
  });
```

### 6.2 Checkpoint Schema

```typescript
interface OrchestratorCheckpoint {
  version: 1;
  sessionId: string;
  timestamp: string;
  phase: OrchestratorPhase;
  taskId: string;
  completedSubtasks: string[];
  git: { branch: string; headCommit: string; isDirty: boolean };
  verification?: { passed: boolean; verifiedAt: string };
}
```

### 6.3 Recovery Flow

```typescript
const maybeRecover = Effect.gen(function* () {
  const checkpoint = yield* loadCheckpoint();
  if (Option.isNone(checkpoint)) return "fresh_start";

  // Validate checkpoint
  const age = Date.now() - new Date(checkpoint.value.timestamp).getTime();
  if (age > MAX_CHECKPOINT_AGE) {
    yield* clearCheckpoint();
    return "checkpoint_expired";
  }

  // Verify git state matches
  const currentHead = yield* getHeadCommit();
  if (currentHead !== checkpoint.value.git.headCommit) {
    return "git_state_mismatch";
  }

  return { type: "resume", checkpoint: checkpoint.value };
});
```

### 6.4 Idempotent Operations

```typescript
// Pattern: Check-then-act with idempotency key
const idempotentCreate = <T>(
  key: string,
  check: () => Effect<Option<T>>,
  create: () => Effect<T>
) => Effect.gen(function* () {
  const existing = yield* check();
  if (Option.isSome(existing)) {
    return { created: false, value: existing.value };
  }
  const value = yield* create();
  return { created: true, value };
});
```

---

## 7. Framework Comparison

How different durable execution frameworks handle the concepts relevant to MechaCoder:

### Temporal
- **Model**: Workflows + Activities (two layers)
- **Replay**: Full deterministic replay of workflow code
- **State**: Stored in Temporal server
- **Suspension**: Activities run separately, workflows await results

### Restate
- **Model**: Durable functions with virtual objects
- **Replay**: Partial replay from journal
- **State**: Embedded key-value store
- **Suspension**: Engine-managed based on I/O

### DBOS
- **Model**: Database-backed durable execution
- **Replay**: Transaction log replay
- **State**: PostgreSQL
- **Suspension**: Mostly local-context

### MechaCoder (Current)
- **Model**: Orchestrator + Subagents
- **Replay**: Not supported (fresh execution)
- **State**: Files (JSON, Markdown)
- **Suspension**: Via Claude Code session IDs

### MechaCoder (Target)
- **Model**: Orchestrator + Subagents + Saga
- **Replay**: Step memoization with ATIF trajectories
- **State**: Files with atomic writes and checkpoints
- **Suspension**: Claude Code + formal checkpoint resume

---

## 8. Future Directions

### 8.1 Effect-Based Durable Execution

Effect TypeScript has primitives that could support durable execution natively:

```typescript
// Conceptual: Effect saga/workflow support
const durableWorkflow = Effect.gen(function* () {
  yield* Effect.checkpoint("phase_1");
  const task = yield* durableStep("select", pickTask);
  yield* Effect.checkpoint("phase_2");
  // ...
});
```

### 8.2 Cloud Gateway Integration

The `.openagents/project.json` has a `cloud` section for future gateway integration:

```json
{
  "cloud": {
    "useGateway": false,
    "sendTelemetry": false,
    "relayUrl": null
  }
}
```

A cloud gateway could provide:
- Centralized checkpoint storage
- Multi-machine orchestration
- Telemetry and monitoring

### 8.3 Event Sourcing

ATIF trajectories are close to an event sourcing model. Full event sourcing would:
- Store all orchestrator events as append-only log
- Rebuild state by replaying events
- Enable time-travel debugging

### 8.4 Distributed Sagas

For truly distributed execution across machines:
- Saga coordinator service
- Distributed compensation
- Two-phase commit across agents

---

## Appendix A: Terminology

| Term | Definition |
|------|------------|
| **Durable execution** | Programming model where workflows survive crashes |
| **Checkpoint** | Saved state that enables recovery |
| **Memoization** | Storing step results for replay |
| **Replay** | Re-executing with memoized results |
| **Saga** | Pattern for distributed transactions with compensation |
| **Idempotency** | Operation safe to execute multiple times |
| **Local-context** | Operation that must run in-process |
| **Remote-context** | Operation that can run independently |

## Appendix B: Related Tasks

| Task ID | Title | Priority |
|---------|-------|----------|
| `oa-5b3d83` | Fix commit/task-update race condition | P0 |
| `oa-c3d1a1` | Make mark_task_blocked_with_followup idempotent | P1 |
| `oa-ebfdaa` | Make update_progress_with_guidance idempotent | P1 |
| `oa-254afa` | Add orchestrator phase checkpoints | P1 |
| `oa-59c504` | Add idempotency keys to usage records | P1 |
| `oa-23428c` | Add Healer invocation deduplication | P2 |
| `oa-5df6be` | Add step result memoization | P2 |
| `oa-a1d5b1` | Extend ATIF for recovery replay | P2 |
| `oa-013645` | Add parallel runner coordination | P2 |
| `oa-e0d9b0` | Model orchestrator as saga | P3 |
| `oa-850dd5` | Epic: Durable execution patterns | P1 |

## Appendix C: References

1. Vanlightly, J. "Demystifying Determinism in Durable Execution" (2025)
2. Vanlightly, J. "The Durable Function Tree, Part 1" (2025)
3. Anthropic. "Effective Harnesses for Long-Running Agents"
4. Temporal Documentation: https://docs.temporal.io
5. Restate Documentation: https://restate.dev
6. DBOS Documentation: https://docs.dbos.dev
