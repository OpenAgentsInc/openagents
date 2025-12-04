# ATIF Integration Plan

Full integration of ATIF (Agent Trajectory Interchange Format) v1.4 into openagents for capturing all agent and subagent trajectories.

## Summary

- **Scope**: Full Effect-native implementation with validation, services, and HUD integration
- **Storage**: `.openagents/trajectories/YYYYMMDD/<session-id>.atif.json`
- **Coverage**: All agents (orchestrator, Claude Code subagents, minimal subagents)
- **Linking**: Parent/child trajectories linked via `subagent_trajectory_ref`

---

## Module Structure

```
src/atif/
├── schema.ts              # ATIF v1.4 schema definitions (Effect/Schema)
├── validation.ts          # Trajectory validation service
├── collector.ts           # TrajectoryCollector service for capturing interactions
├── service.ts             # TrajectoryService for storage/retrieval
├── adapter.ts             # Event-to-ATIF converters
├── hud-integration.ts     # HUD event emitters for ATIF
├── index.ts               # Public exports
└── __tests__/
    ├── schema.test.ts
    ├── validation.test.ts
    ├── collector.test.ts
    ├── service.test.ts
    └── adapter.test.ts
```

---

## Implementation Tasks

### Phase 1: Core Schema (P1)

**Task 1.1: ATIF Schema Definitions**
- Create `src/atif/schema.ts` with Effect/Schema definitions
- Types: `Trajectory`, `Step`, `Agent`, `ToolCall`, `Observation`, `ObservationResult`, `SubagentTrajectoryRef`, `Metrics`, `FinalMetrics`
- Follow patterns from `src/sessions/schema.ts`
- Export type helpers: `decodeTrajectory`, `isAgentStep`, `hasToolCalls`

**Task 1.2: ATIF Validation Service**
- Create `src/atif/validation.ts`
- Implement 5 validation rules:
  1. step_id sequential starting from 1
  2. timestamps in ISO 8601
  3. source restricted to: user, agent, system
  4. tool call references in observations must match existing tool_call_ids
  5. Agent-only fields (model_name, reasoning_content) only on agent steps
- Return `Effect.Effect<Trajectory, TrajectoryValidationError>`

**Task 1.3: Schema Tests**
- Create `src/atif/__tests__/schema.test.ts`
- Test decode/encode for all types
- Test rejection of invalid structures
- Test type guards

**Task 1.4: Validation Tests**
- Create `src/atif/__tests__/validation.test.ts`
- Test all 5 rules with positive and negative cases

### Phase 2: Collector Service (P1)

**Task 2.1: TrajectoryCollector Service**
- Create `src/atif/collector.ts`
- Use `Context.Tag` pattern from `src/llm/provider.ts`
- Methods:
  - `startTrajectory(sessionId, agent, parentSessionId?)`
  - `recordUserStep(message)`
  - `recordAgentStep(message, modelName?, reasoningContent?, toolCalls?, metrics?)`
  - `recordSystemStep(message)`
  - `recordObservation(results[])`
  - `registerSubagent(sessionId, trajectoryPath?)`
  - `finishTrajectory() -> Trajectory`
- Use `Ref` for state management
- Auto-sequence step_ids
- Track tool_call_ids for observation linking
- Accumulate metrics for final_metrics

**Task 2.2: Collector Tests**
- Create `src/atif/__tests__/collector.test.ts`
- Test step sequencing, observation linking, metrics accumulation
- Test subagent registration

### Phase 3: Storage Service (P1)

**Task 3.1: TrajectoryService**
- Create `src/atif/service.ts`
- Storage path: `.openagents/trajectories/YYYYMMDD/<session-id>.atif.json`
- Methods:
  - `saveTrajectory(trajectory) -> path`
  - `loadTrajectory(sessionId) -> Trajectory`
  - `listTrajectories() -> sessionIds[]`
  - `findChildTrajectories(parentSessionId) -> Trajectory[]`
  - `getTrajectoryTree(sessionId) -> Trajectory[]` (parent + all descendants)
- Validate before save
- Create date directories as needed

**Task 3.2: Service Tests**
- Create `src/atif/__tests__/service.test.ts`
- Test save/load, listing, tree traversal
- Use temp directories for isolation

### Phase 4: Event Adapters (P1)

**Task 4.1: Event-to-ATIF Adapters**
- Create `src/atif/adapter.ts`
- Convert existing events to ATIF:
  - `LoopEvent` (from `src/agent/loop.ts`) -> Steps
  - `OrchestratorEvent` (from `src/agent/orchestrator/types.ts`) -> Steps
  - `SessionEntry` (from `src/sessions/schema.ts`) -> Steps
  - `SubagentResult` -> Observation with subagent_trajectory_ref
- Agent factories:
  - `createMechaCoderAgent(model, version)`
  - `createClaudeCodeAgent(model, version)`
  - `createMinimalSubagent(model, version)`

**Task 4.2: Adapter Tests**
- Create `src/atif/__tests__/adapter.test.ts`
- Test conversion accuracy for all event types

### Phase 5: Integration (P1)

**Task 5.1: Agent Loop Integration**
- Modify `src/agent/loop.ts`
- Wire TrajectoryCollector into agent loop
- Capture: user message, agent responses, tool calls, tool results
- Finalize and save trajectory on loop completion

**Task 5.2: Orchestrator Integration**
- Modify `src/agent/orchestrator/orchestrator.ts`
- Create orchestrator trajectory at session start
- Record phase transitions as system steps
- Link subagent trajectories via registerSubagent

**Task 5.3: Claude Code Subagent Integration**
- Modify `src/agent/orchestrator/claude-code-subagent.ts`
- Create subagent trajectory for each invocation
- Pass parent session ID for linking
- Save trajectory on completion
- Return trajectory path for parent observation

**Task 5.4: Minimal Subagent Integration**
- Modify `src/agent/orchestrator/subagent.ts`
- Same pattern as Claude Code integration

### Phase 6: HUD Integration (P2)

**Task 6.1: ATIF HUD Messages**
- Modify `src/hud/protocol.ts` to add ATIF message types:
  - `atif_trajectory_start`
  - `atif_step_recorded`
  - `atif_subagent_spawned`
  - `atif_trajectory_complete`

**Task 6.2: ATIF HUD Emitter**
- Create `src/atif/hud-integration.ts`
- Emit ATIF events to HUD during collection
- Follow pattern from `src/hud/emit.ts`

### Phase 7: ProjectConfig Extension (P2)

**Task 7.1: Trajectory Settings**
- Modify `src/tasks/project.ts` (ProjectConfig schema)
- Add optional `trajectoryConfig`:
  - `enabled: boolean` (default true)
  - `retentionDays: number` (default 30)
  - `maxSizeGB: number` (default 5)

---

## Files to Create

| File | Purpose | Lines (est) |
|------|---------|-------------|
| `src/atif/schema.ts` | ATIF v1.4 schema definitions | ~150 |
| `src/atif/validation.ts` | Trajectory validation | ~100 |
| `src/atif/collector.ts` | Event collection service | ~200 |
| `src/atif/service.ts` | Storage/retrieval service | ~150 |
| `src/atif/adapter.ts` | Event converters | ~200 |
| `src/atif/hud-integration.ts` | HUD event emission | ~80 |
| `src/atif/index.ts` | Public exports | ~20 |
| `src/atif/__tests__/*.test.ts` | Tests (5 files) | ~400 |

## Files to Modify

| File | Changes |
|------|---------|
| `src/agent/loop.ts` | Wire TrajectoryCollector |
| `src/agent/orchestrator/orchestrator.ts` | Create/save orchestrator trajectory |
| `src/agent/orchestrator/claude-code-subagent.ts` | Create/save subagent trajectories |
| `src/agent/orchestrator/subagent.ts` | Create/save minimal subagent trajectories |
| `src/hud/protocol.ts` | Add ATIF message types |
| `src/tasks/project.ts` | Add trajectoryConfig to ProjectConfig |

---

## Critical Files to Read Before Implementation

1. `src/sessions/schema.ts` - Effect/Schema patterns
2. `src/sessions/service.ts` - JSONL service pattern with Context.Tag
3. `src/hud/protocol.ts` - HUD message type definitions
4. `src/agent/orchestrator/types.ts` - OrchestratorEvent types to adapt
5. `src/agent/loop.ts` - Agent loop integration point
6. `src/agent/apm.ts` - Collector/accumulation pattern

---

## Storage Format

```
.openagents/
└── trajectories/
    ├── 20251203/
    │   ├── session-20251203-103000-abc123.atif.json
    │   └── session-20251203-103005-def456.atif.json  # subagent
    └── 20251204/
        └── ...
```

Each `.atif.json` file contains a complete ATIF v1.4 trajectory with parent/child linking via `subagent_trajectory_ref`.

---

## Task Dependencies

```
Phase 1 (Schema + Validation)
    └── Phase 2 (Collector)
        └── Phase 3 (Storage)
            └── Phase 4 (Adapters)
                └── Phase 5 (Integration)
                    └── Phase 6 (HUD) [can run parallel]
                    └── Phase 7 (Config) [can run parallel]
```

---

## Tasks to Create in .openagents/tasks.jsonl

After plan approval, create these tasks:

1. **oa-atif-schema** (P1, feature): Implement ATIF v1.4 schema with Effect/Schema
2. **oa-atif-validation** (P1, feature): Implement trajectory validation service
3. **oa-atif-collector** (P1, feature): Implement TrajectoryCollector service
4. **oa-atif-service** (P1, feature): Implement TrajectoryService for storage
5. **oa-atif-adapters** (P1, feature): Implement event-to-ATIF adapters
6. **oa-atif-loop-integration** (P1, feature): Integrate ATIF into agent loop
7. **oa-atif-orchestrator-integration** (P1, feature): Integrate ATIF into orchestrator
8. **oa-atif-subagent-integration** (P1, feature): Capture Claude Code and minimal subagent trajectories
9. **oa-atif-hud** (P2, feature): Add ATIF events to HUD protocol
10. **oa-atif-config** (P2, feature): Add trajectory settings to ProjectConfig
