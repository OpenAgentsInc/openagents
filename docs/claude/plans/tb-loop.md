# FM Terminal-Bench Loop Implementation Plan

**Goal**: Enable MechaCoder to hit #1 on Terminal-Bench using only Apple's on-device Foundation Model, with learning capabilities that improve over time.

---

## Current State

| Component | Status | Location |
|-----------|--------|----------|
| FM Service | ✅ Complete | `src/fm/service.ts` - retry, metrics, health |
| Swift Bridge | ✅ Built | `swift/foundation-bridge/` - needs macOS 26 validation |
| Skills System | ✅ Integrated (FM only) | `src/skills/` + `src/bench/model-adapter.ts:591-622` |
| Memory System | ✅ Built, ❌ Not integrated | `src/memory/service.ts` |
| Reflexion System | ✅ Built, ❌ Not integrated | `src/reflexion/service.ts` |
| Episode Learner | ✅ Built, ❌ Not wired to TB | `src/training/episode-learner.ts` |
| TB CLI Flags | ❌ Missing | No --skills, --memory, --reflect |
| FM in Subagent Router | ❌ Missing | Router only has Claude Code → OpenRouter |

---

## Implementation Phases

### Phase 1: Foundation (P0 - Today)

#### 1.1 Validate Swift Bridge on macOS 26
**Task**: `oa-79dcc1`

```bash
cd swift/foundation-bridge
./build.sh
./run.sh  # Keep running in one terminal

# In another terminal:
curl http://localhost:8788/health
curl http://localhost:8788/v1/models
bun test src/fm/*.test.ts
```

**Done when**: Bridge builds, health endpoint responds, FM tests pass.

---

### Phase 2: Learning Flags + Memory/Reflexion Integration (P1)

#### 2.1 Add CLI Learning Flags to tbench-iterate
**Task**: `oa-5df233`
**File**: `src/cli/tbench-iterate.ts`

Add options:
```typescript
"skills": { type: "boolean" },      // Enable skill injection (default: true for FM)
"memory": { type: "boolean" },      // Enable memory injection (default: false)
"reflect": { type: "boolean" },     // Enable reflexion on failures (default: false)
"max-retries": { type: "string" },  // Max reflection-based retries (default: 2)
```

#### 2.2 Add Memory Injection to FM Runner
**File**: `src/bench/model-adapter.ts`

1. Add `useMemory?: boolean` and `maxMemories?: number` to `FMModelConfig`
2. Create `getRelevantMemories()` helper (copy pattern from `getRelevantSkills` at lines 591-622)
3. Inject formatted memories into `buildFMSystemPrompt()`

#### 2.3 Add Reflexion Integration to FM Runner
**File**: `src/bench/model-adapter.ts`

1. Add `useReflection?: boolean` and `maxReflectionRetries?: number` to `FMModelConfig`
2. In `runTask()`, after failure:
   - Call `ReflexionService.recordFailure()` with task context
   - Call `ReflexionService.reflect()` for FM-generated deep reflection (smarter learning)
   - Inject reflection into retry prompt via `buildRetryPrompt()`
   - Retry up to `maxReflectionRetries` times

**Note**: Using full FM-generated reflection (`reflect()`) instead of heuristic-based (`quickReflect()`) for better learning quality.

#### 2.4 Wire Flags Through CLI → Model Adapter
**File**: `src/cli/tbench-iterate.ts`

Pass learning flags when creating model runner config.

---

### Phase 3: FM in Subagent Router (P1)

#### 3.1 Add FM to Routing Chain
**Task**: `oa-0edc48`
**File**: `src/agent/orchestrator/subagent-router.ts`

**Routing order**:
```
shouldUseClaudeCode() → YES → Claude Code
  ↓ NO
shouldUseFM() → YES (macOS + bridge healthy) → FM with learning
  ↓ NO
OpenRouter (Grok) fallback
```

**Implementation**:
1. Add `detectFMAvailability()` - check macOS + bridge health
2. Add `runFMSubagent()` - wrapper using FM model adapter pattern
3. Update `runBestAvailableSubagent()` routing logic

---

### Phase 4: Post-Run Learning (P1)

#### 4.1 Wire Episode Learner to TB Pipeline
**Files**: `src/cli/tbench-iterate.ts`, `src/training/episode-learner.ts`

After each iteration completes:
1. Call `EpisodeLearner.processEpisode()` with run results
2. Register extracted skills via `SkillService.registerSkill()`
3. Record episodic memories via `MemoryService.recordTask()`
4. Generate reflections for failures via `ReflexionService.recordFailure()`

---

### Phase 5: Documentation (P1)

#### 5.1 Update TB Docs for FM
**Task**: `oa-be762a`
**File**: `docs/terminal-bench.md` or `docs/mechacoder/`

Add "Using Apple Foundation Models" section with:
- Prerequisites (macOS 26, Apple Intelligence, bridge built)
- Example commands
- Learning flags documentation

---

## Critical Files to Modify

| File | Changes |
|------|---------|
| `src/bench/model-adapter.ts` | Add memory + reflexion injection to FM runner |
| `src/cli/tbench-iterate.ts` | Add --skills, --memory, --reflect flags |
| `src/agent/orchestrator/subagent-router.ts` | Add FM routing option |
| `docs/terminal-bench.md` | FM usage documentation |

## Critical Files to Read Before Implementation

| File | Reason |
|------|--------|
| `src/bench/model-adapter.ts:580-927` | Existing FM runner + skill injection pattern |
| `src/memory/service.ts:275-283` | `formatForPrompt()` method to use |
| `src/reflexion/service.ts:264-305` | `quickReflect()` and `buildRetryPrompt()` methods |
| `src/skills/service.ts:294-305` | Layer composition pattern |

---

## Canonical Commands After Implementation

```bash
# Basic FM run (skills enabled by default)
bun run tbench-iterate --model fm --max-tasks 10

# FM with full learning stack
bun run tbench-iterate \
  --model fm \
  --skills \
  --memory \
  --reflect \
  --max-tasks 10

# Overnight learning sweep
bun run tbench-iterate \
  --model fm \
  --skills --memory --reflect \
  --iterations 10 \
  --learn-post-run
```

---

## Task Dependencies

```
oa-79dcc1 (Swift bridge validation)
    |
    v
oa-5df233 (CLI learning flags) ←→ Memory + Reflexion in model-adapter
    |
    v
oa-0edc48 (FM in subagent router)
    |
    v
Post-run episode learning integration
    |
    v
oa-be762a (Documentation)
```

---

## Future Work (P2+)

These are NOT blockers for the basic FM TB loop:

- **LearningContext Service**: Unified abstraction combining Skills + Memory + Reflexion
- **Archivist**: Turn trajectories into higher-level lessons (oa-7d04ae → oa-280919)
- **Trainer/Gym**: Structured training episodes (oa-e2c048 → oa-a1aeb6)
- **TRM/SOAR**: Advanced self-improvement patterns
