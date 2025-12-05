# Plan: Streaming ATIF Trajectory Persistence with Real-Time HUD Updates

## Overview

Integrate ATIF (Agent Trajectory Interchange Format) v1.4 with streaming JSONL persistence for complete agent trajectory capture. All agent execution (orchestrator + subagents) will save trajectories incrementally to `.openagents/trajectories/` with real-time HUD updates to the desktop UI.

## Current State (Broken)

**ATIF v1.4 Implementation:** ✅ Complete (61 tests passing)
- Schema, validation, collector, service all ready
- Integration helpers exist (`createOrchestratorATIF`, `createSubagentATIF`)
- **BUT: Never wired into agent execution**

**Result:**
- Agents run successfully but trajectories are NEVER captured
- `.openagents/trajectories/` stays empty
- No observability into what agents did
- No recovery from crashes
- No learning data for SFT

## Target Architecture

```
Agent Execution
    ↓
StandaloneTrajectoryCollector (in-memory)
    ↓ (after each step)
StreamingWriter → JSONL + Index files
    ↓
.openagents/trajectories/YYYYMMDD/
    ├── session-ID.atif.jsonl  (append-only steps)
    └── session-ID.index.json  (metadata + checkpoint)
    ↓ (simultaneously)
HUD Events → Desktop UI (real-time updates)
```

## Key Design Decisions

1. **JSONL format** - One step per line, append-only for crash safety
2. **Immediate writes** - Flush after every step (~5ms overhead, zero data loss)
3. **Index file** - Atomic metadata updates for checkpointing
4. **ATIF only** - Skip SDK session persistence (can add later)
5. **Complete integration** - Wire at all agent levels (orchestrator, Claude Code, minimal)

## Implementation Tasks

### Phase 1: Streaming JSONL Writer (Core Infrastructure)

**Task 1.1: Create StreamingWriter Service**
- File: `src/atif/streaming-writer.ts` (NEW, ~350 LOC)
- Purpose: Append-only JSONL writer with atomic index updates
- Key methods:
  - `writeStep(step)` - Append step to .jsonl, update index
  - `updateMetadata(final_metrics, status)` - Atomic .index.json update via .tmp rename
  - `close()` - Finalize with final_metrics
- Features:
  - Atomic writes (write to .tmp, rename on success)
  - Auto-create date directories
  - Crash-safe (incomplete lines skipped on recovery)
- Test coverage: Unit tests for append, index update, crash scenarios

**Task 1.2: Create Recovery Loader**
- File: `src/atif/recovery.ts` (NEW, ~250 LOC)
- Purpose: Load partial trajectories from JSONL
- Key methods:
  - `loadFromJSONL(path)` - Parse JSONL into Trajectory
  - `detectIncomplete(index)` - Check if status="in_progress"
  - `getRecoveryPlan(sessionId)` - Suggest resume vs restart
- Features:
  - Skip incomplete final line
  - Validate steps during load
  - Reconstruct final_metrics from step metrics
  - Support both .jsonl (new) and .json (legacy) formats

**Task 1.3: Update Collector for Streaming**
- File: `src/atif/collector.ts` (MODIFY, +30 LOC)
- Changes:
  - Add optional `streamingWriter?: StreamingWriter` to constructor
  - Call `streamingWriter.writeStep()` after each `recordXStep()`
  - Auto-flush on `finishTrajectory()`
- Backward compatible: Works with or without streaming

### Phase 2: Integration at All Agent Levels

**Task 2.1: Wire Orchestrator**
- File: `src/agent/do-one-task.ts` (MODIFY, lines 846-900, +40 LOC)
- Location: After `createHudCallbacks()` (line 848)
- Changes:
  ```typescript
  // NEW: Create ATIF streaming context
  const atif = createOrchestratorATIF({
    cwd: config.workDir,
    modelName: projectConfig.orchestrator?.model ?? "grok-4.1-fast",
    streamingWriter: new StreamingWriter({
      baseDir: join(config.workDir, ".openagents/trajectories"),
      sessionId,
    }),
  });

  // NEW: Wrap emit to capture ATIF
  const emit = wrapEventHandler(hudEmit, (event) => {
    atif.handleEvent(event);
    emitATIFHudEvent(event, atif.sessionId);  // Real-time UI
  });

  // After orchestrator completes:
  const trajectoryPath = await atif.finalize();
  console.log(`Trajectory saved: ${trajectoryPath}`);
  ```

**Task 2.2: Wire Claude Code Subagent**
- File: `src/agent/orchestrator/claude-code-subagent.ts` (MODIFY, +50 LOC)
- Location: In `runClaudeCodeSubagent()` function (lines 280-650)
- Changes:
  - Create `createSubagentATIF()` at function start
  - Capture prompt as user step
  - Capture each message as agent step (with tool calls)
  - Record tool results as observations
  - Call `finalize()` after query completes
  - Store trajectory path in `SubagentResult`

**Task 2.3: Wire Minimal Subagent** (if used)
- File: `src/agent/loop.ts` or wrapper
- Similar pattern to Claude Code integration

### Phase 3: HUD Real-Time Streaming

**Task 3.1: Add ATIF HUD Message Types**
- File: `src/hud/protocol.ts` (MODIFY, +50 LOC)
- Add to `HudMessage` union:
  ```typescript
  interface ATIFStepWrittenMessage {
    type: "atif_step_written";
    sessionId: string;
    stepId: number;
    source: "user" | "agent" | "system";
    hasToolCalls: boolean;
    metrics?: { promptTokens, completionTokens, costUsd };
  }

  interface ATIFCheckpointMessage {
    type: "atif_checkpoint";
    sessionId: string;
    lastStepId: number;
    totalSteps: number;
    totalCost: number;
  }

  interface ATIFTrajectoryCompleteMessage {
    type: "atif_trajectory_complete";
    sessionId: string;
    trajectoryPath: string;
    totalSteps: number;
    finalMetrics: FinalMetrics;
  }
  ```

**Task 3.2: Emit HUD Events During Collection**
- File: `src/atif/hud-streaming.ts` (NEW, ~100 LOC)
- Purpose: Emit HUD messages as trajectory is built
- Integration: Called from `StreamingWriter.writeStep()`
- Event flow: Step written → HUD event → Desktop UI update

## Critical Files

### Files to Create
- `src/atif/streaming-writer.ts` (~350 LOC) - JSONL writer + index manager
- `src/atif/recovery.ts` (~250 LOC) - Load partial trajectories
- `src/atif/hud-streaming.ts` (~100 LOC) - HUD event emitter

### Files to Modify
- `src/atif/collector.ts` (+30 LOC) - Add streaming support
- `src/atif/integration.ts` (+50 LOC) - Enhance contexts with streaming
- `src/agent/do-one-task.ts` (+40 LOC) - Wire orchestrator ATIF
- `src/agent/orchestrator/claude-code-subagent.ts` (+50 LOC) - Wire subagent ATIF
- `src/hud/protocol.ts` (+50 LOC) - Add ATIF HUD message types

### Files to Read (Reference Only)
- `src/atif/schema.ts` - ATIF types
- `src/atif/service.ts` - Existing persistence service
- `src/agent/orchestrator/types.ts` - OrchestratorEvent types

## File Format Details

### JSONL Format (.atif.jsonl)
```jsonl
{"__header__":true,"schema_version":"ATIF-v1.4","session_id":"session-2025-12-05...","agent":{...},"created_at":"2025-12-05T..."}
{"step_id":1,"timestamp":"2025-12-05T...","source":"system","message":"Session started"}
{"step_id":2,"timestamp":"2025-12-05T...","source":"user","message":"Wire e2eCommands..."}
{"step_id":3,"timestamp":"2025-12-05T...","source":"agent","message":"I'll explore...","tool_calls":[...]}
{"step_id":4,"timestamp":"2025-12-05T...","source":"system","observation":{"results":[...]}}
...
```

### Index Format (.index.json)
```json
{
  "session_id": "session-2025-12-05T...",
  "agent": {"name": "MechaCoder", "version": "2.0"},
  "checkpoint": {
    "step_id": 47,
    "timestamp": "2025-12-05T...",
    "completed_step_count": 47
  },
  "status": "in_progress",
  "final_metrics": null
}
```

## Data Flow Example

**Orchestrator Run:**
```
1. do-one-task.ts starts
2. createOrchestratorATIF() initializes StreamingWriter
3. Task selected → recordSystemStep() → writeStep() → append to .jsonl + update .index.json → emit HUD event
4. Task decomposed → recordAgentStep() → writeStep() → ...
5. Subtask start → recordSystemStep() → writeStep() → ...
6. [Claude Code subagent spawns]
   6a. createSubagentATIF() with parentSessionId
   6b. Each agent message → recordAgentStep() → writeStep()
   6c. finalize() → saves child trajectory
7. Subtask complete → recordObservation(with subagent ref) → writeStep()
8. Verification → recordObservation() → writeStep()
9. finalize() → update index with status="complete", final_metrics
```

**Crash Recovery (if agent dies at step 47):**
```
1. Next run detects .index.json with status="in_progress"
2. loadFromJSONL() reads steps 1-47
3. Offer user: "Resume from step 48" or "Restart"
4. If resume: replay steps into collector, continue from 48
```

## HUD Real-Time Updates

**UI will show:**
- "Step 3 saved: Agent exploring codebase (2 tool calls)"
- "Checkpoint at step 10: 1,250 tokens, $0.03 spent"
- "Subagent linked: session-...def456 (typecheck subtask)"
- "Trajectory complete: 47 steps, $0.28 total, saved to .openagents/trajectories/20251205/session-...abc123.atif.jsonl"

## Performance Impact

- **Per-step overhead:** ~5-10ms (fsync + index update)
- **Agent step interval:** ~10-30 seconds (LLM thinking time)
- **Overhead ratio:** <0.1% (negligible)
- **Benefit:** Zero data loss, full crash recovery

## Testing Strategy

1. **Unit tests:** StreamingWriter append, index update, atomicity
2. **Integration tests:** Full orchestrator run → verify .jsonl + .index.json created
3. **Crash tests:** Kill agent mid-run → verify partial trajectory recoverable
4. **HUD tests:** Verify events emitted in real-time
5. **Performance tests:** Measure fsync overhead (should be <10ms)

## Implementation Roadmap

### Phase 1: Core Streaming Infrastructure (Priority 1)
1. Create `StreamingWriter` service
2. Create `loadFromJSONL()` recovery loader
3. Update `StandaloneTrajectoryCollector` with streaming support
4. Tests: Unit + crash recovery

**Estimate:** 2-3 hours | **Deliverable:** Streaming writes working

### Phase 2: Orchestrator Integration (Priority 1)
1. Wire `createOrchestratorATIF()` into `do-one-task.ts`
2. Wrap emit callback with ATIF handler
3. Call finalize() on completion
4. Tests: Integration test verifying .jsonl created

**Estimate:** 1 hour | **Deliverable:** Orchestrator trajectories saving

### Phase 3: Subagent Integration (Priority 1)
1. Wire `createSubagentATIF()` into Claude Code subagent
2. Capture messages → steps
3. Link to parent via subagent_trajectory_ref
4. Tests: Verify parent/child linking

**Estimate:** 1-2 hours | **Deliverable:** Full trajectory tree captured

### Phase 4: HUD Real-Time Streaming (Priority 2)
1. Add ATIF message types to `src/hud/protocol.ts`
2. Create `hud-streaming.ts` emitter
3. Wire into `StreamingWriter.writeStep()`
4. Update desktop UI to display ATIF events

**Estimate:** 1 hour | **Deliverable:** Real-time UI updates

**Total Effort:** 5-7 hours

## Success Criteria

After implementation, verify:
- ✅ `.openagents/trajectories/YYYYMMDD/` contains .jsonl + .index.json files
- ✅ Each agent run creates a new trajectory
- ✅ Parent/child trajectories are linked
- ✅ HUD shows real-time "Step N saved" messages
- ✅ Kill agent mid-run → partial trajectory recoverable
- ✅ Validation passes for all saved trajectories
- ✅ All tests passing (existing 61 + new ~50 = 111 total)

## Future Enhancements (Out of Scope)

- SDK session persistence to `~/.claude/session-env/` (Phase 5)
- Batched writes optimization (if >10ms overhead measured)
- Trajectory visualization UI (separate task)
- SFT dataset export from trajectories (separate task)
