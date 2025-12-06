# MechaCoder Learning System - Comprehensive Test Plan

## Executive Summary

This plan covers thorough test coverage for the entire MechaCoder learning system:
- **8 new TRM+SOAR modules** (just implemented, no tests yet)
- **6 original modules** (schema tests exist, service tests missing)
- **Integration tests** for component interactions
- **End-to-end tests** for critical pipelines

**Current State**:
- TRM+SOAR implementation complete (3,802 lines across 8 files)
- Zero test coverage on new modules
- Original modules have schema tests but no service tests

**Goal**: Comprehensive test coverage using `bun:test` following existing codebase patterns.

---

## Testing Framework & Conventions

Based on codebase analysis:
- **Framework**: `bun:test` (native Bun testing)
- **Location**: `src/<module>/__tests__/*.test.ts`
- **Effect pattern**: `runWithBun` helper with `Effect.provide(BunContext.layer)`
- **Schema testing**: `S.decodeUnknownSync()` for validation
- **Error testing**: `.pipe(Effect.flip)` to convert errors for assertions

---

## Test Coverage Plan

### Phase 1: TRM Module Tests (Priority: Critical)

#### 1.1 `src/learning/__tests__/trm-state.test.ts`
**Target**: `src/learning/trm-state.ts` (435 lines)

| Test Suite | Tests |
|------------|-------|
| State Creation | Initial values, timestamps, default maxDepth |
| TaskContext | Valid/invalid inputs, optional fields |
| CandidateSolution | Attempt increment, validation tracking |
| ReasoningTrace | Hypothesis buffer (max 5), history buffer (max 10) |
| updateSolution | Immutability, attemptNumber auto-increment |
| addReasoningStep | History overflow, depth increment |
| markStuck | Error pattern dedup, stuckCount increment |
| detachState | Preserves knowledge, clears transients |
| Service Layer | Effect.sync wrapping, all methods |

**Edge Cases**:
- maxDepth = 0, 1, negative
- Empty inputs array
- Null/undefined optional fields
- Token overflow (large numbers)

#### 1.2 `src/learning/__tests__/trm-halt.test.ts`
**Target**: `src/learning/trm-halt.ts` (327 lines)

| Test Suite | Tests |
|------------|-------|
| checkMaxDepth | At boundary, below, above |
| checkTestsPassed | validated=true/false, missing validationResult |
| checkHighConfidence | At 0.95 threshold, minStepsBeforeHalt gate |
| checkAccuracyAchieved | At 1.0 threshold, minStepsBeforeHalt gate |
| checkStuck | stuckCount < 3, = 3, > 3 |
| shouldHalt | Priority order (tests > accuracy > confidence > depth > stuck) |
| detectProgress | All 5 progress types |
| Config | Default values, overrides, persistence |

**Edge Cases**:
- Floating point at exact thresholds (0.95, 1.0)
- Config override (maxDepthOverride)
- Multiple conditions true (verify priority)

#### 1.3 `src/learning/__tests__/trm-ema.test.ts`
**Target**: `src/learning/trm-ema.ts` (337 lines)

| Test Suite | Tests |
|------------|-------|
| EMA Formula | decay=0.999, 0.5, 0, 1 |
| Variance | Welford's algorithm, first sample |
| Confidence Interval | Bounds [0,1], sampleCount < 2 |
| isReliable | minSamples threshold (5) |
| TaskTypeStats | optimalDepth only on success |
| SkillEMAStats | Dual rates, jointConfidence average |
| Recent Values | Buffer overflow (max 10) |
| Service | Config persistence, all methods |

**Edge Cases**:
- Very small/large sample values
- Variance with identical samples
- Zero division guards

---

### Phase 2: SOAR Module Tests (Priority: Critical)

#### 2.1 `src/learning/__tests__/soar-hindsight.test.ts`
**Target**: `src/learning/soar-hindsight.ts` (372 lines)

| Test Suite | Tests |
|------------|-------|
| isSuitableForRelabeling | All 5 criteria, edge cases |
| generateSyntheticDescription | Truncation (100/50 chars), null handling |
| createSyntheticTask | ID generation, timestamp, confidence |
| relabelAttempt | Suitable → Some, unsuitable → None |
| relabelBatch | Grouping, sorting, capping (maxSyntheticPerTask=50) |
| Stats | Rates, averages, running totals |
| Service | Config updates, all methods |

**Edge Cases**:
- trainingAccuracy at 0.01, 0.99 boundaries
- Empty batch
- All attempts from same task (cap test)
- Null actualOutput

#### 2.2 `src/learning/__tests__/soar-validation.test.ts`
**Target**: `src/learning/soar-validation.ts` (466 lines)

| Test Suite | Tests |
|------------|-------|
| checkNonTrivialOutput | Length, all-same-char, null/empty |
| calculateSimilarity | Same=1, empty=0, character overlap |
| checkNonIdentity | At 0.95 threshold |
| checkCodeComplexity | Operator/keyword/call counting |
| checkNotLookupTable | Constant ratio at 0.8 |
| checkEntropy | Shannon entropy, disabled flag |
| validateSynthetic | All 5 checks, score calculation |
| validateBatch | Grouping, stats accumulation |
| Service | Config, stats, all methods |

**Edge Cases**:
- Empty code string
- Unicode characters in output
- Very long strings

#### 2.3 `src/learning/__tests__/soar-selection.test.ts`
**Target**: `src/learning/soar-selection.ts` (399 lines)

| Test Suite | Tests |
|------------|-------|
| calculateCodeSignature | All pattern types |
| calculateJaccardSimilarity | Empty sets, full overlap |
| calculateDiversityScore | Empty selection, single, multiple |
| selectTop | Greedy algorithm, < topK candidates |
| selectBottom | Diversity prioritization |
| selectGreedyDiverse | Combined top+bottom |
| groupByTask | Correct partitioning |
| selectWithTaskBalance | Per-task quotas |
| Service | Stats tracking, all methods |

**Edge Cases**:
- Fewer candidates than K
- All candidates same quality
- Empty candidates list

#### 2.4 `src/learning/__tests__/soar-voting.test.ts`
**Target**: `src/learning/soar-voting.ts` (368 lines)

| Test Suite | Tests |
|------------|-------|
| normalizeOutputKey | All types (null, string, array, object) |
| calculateVoteWeight | Base + skill weighting |
| groupVotes | Accumulation, averageAccuracy |
| breakTie | All 3 strategies (accuracy, count, random) |
| vote | Full pipeline, confidence calculation |
| ensembleVote | Convenience wrapper |
| minVotes | Invalid result when below threshold |
| Service | Stats, all methods |

**Edge Cases**:
- Object key ordering (sorted)
- Array order matters
- Zero total weight (division guard)
- Tie with random (non-deterministic - test separately)

#### 2.5 `src/learning/__tests__/soar-ttt.test.ts`
**Target**: `src/learning/soar-ttt.ts` (659 lines)

| Test Suite | Tests |
|------------|-------|
| createTTTState | Initial values |
| shouldContinueTTT | All 4 stop conditions |
| getStopReason | Priority order |
| processIteration | Best finding, averaging, improvement |
| outputsEqual | All recursive cases |
| updateSkillContext | Rates, boosting heuristic |
| createSessionResult | Timestamps, durations |
| Service | runTTT with mock callbacks, stats |

**Edge Cases**:
- Empty attempts list
- No improvement on iteration 0 (skip check)
- Immediate satisfaction (accuracy=1.0)
- Nested arrays/objects in outputsEqual

---

### Phase 3: Integration Tests (Priority: High)

#### 3.1 `src/learning/__tests__/trm-integration.test.ts`
**Tests**: TRM state → halt → EMA pipeline

```
State creation → Solution update → Halt check → EMA update
                                              ↓
                              Progress detection → Next iteration
```

| Test | Description |
|------|-------------|
| Supervision loop | State evolves correctly through iterations |
| Halt triggers | Each halt reason triggers at correct point |
| EMA stability | Success rate stabilizes with EMA |

#### 3.2 `src/learning/__tests__/soar-integration.test.ts`
**Tests**: SOAR hindsight → validation → selection → voting pipeline

```
AttemptRecord[] → Hindsight → SyntheticTaskSolution[]
                              ↓
                  Validation → Valid[]
                              ↓
                  Selection → Top25 + Bottom25
                              ↓
                    Voting → FinalPrediction
```

| Test | Description |
|------|-------------|
| Full pipeline | End-to-end data transformation |
| Quality filtering | Invalid synthetics filtered out |
| Selection balance | Top + bottom selected correctly |
| Voting consensus | Correct winner selected |

#### 3.3 `src/learning/__tests__/ttt-integration.test.ts`
**Tests**: Full TTT loop with mocked callbacks

| Test | Description |
|------|-------------|
| Single iteration | One attempt→validate→vote cycle |
| Multi-iteration | Improvement detection works |
| Stop conditions | All 4 stop reasons trigger correctly |
| Stats accumulation | Session stats calculated correctly |

---

### Phase 4: Original Module Service Tests (Priority: Medium)

These modules have schema tests but NO service tests:

#### 4.1 `src/skills/__tests__/service.test.ts`
| Test | Description |
|------|-------------|
| registerSkill | Creates skill, assigns ID |
| getSkill | Retrieves by ID |
| searchSkills | Semantic search works |
| updateSkill | Modifies existing |
| deleteSkill | Removes from store |

#### 4.2 `src/memory/__tests__/service.test.ts`
| Test | Description |
|------|-------------|
| addMemory | Creates memory, assigns ID |
| getRelevantMemories | Retrieval scoring works |
| recordTask | Episodic memory creation |
| recordKnowledge | Semantic memory creation |

#### 4.3 `src/reflexion/__tests__/service.test.ts`
| Test | Description |
|------|-------------|
| recordFailure | Creates failure context |
| reflect | Generates reflection |
| formatForPrompt | Correct prompt format |

#### 4.4 `src/archivist/__tests__/service.test.ts`
| Test | Description |
|------|-------------|
| recordTrajectory | Stores trajectory |
| extractPatterns | Pattern extraction |
| promotePatterns | Skill promotion |

#### 4.5 `src/trainer/__tests__/service.test.ts`
| Test | Description |
|------|-------------|
| runTask | Single task execution |
| runTasks | Batch execution |
| Gym integration | Isolated execution |

---

## Test File Structure

```
src/learning/__tests__/
├── trm-state.test.ts        (~150 lines)
├── trm-halt.test.ts         (~120 lines)
├── trm-ema.test.ts          (~140 lines)
├── soar-hindsight.test.ts   (~130 lines)
├── soar-validation.test.ts  (~180 lines)
├── soar-selection.test.ts   (~150 lines)
├── soar-voting.test.ts      (~140 lines)
├── soar-ttt.test.ts         (~200 lines)
├── trm-integration.test.ts  (~100 lines)
├── soar-integration.test.ts (~120 lines)
├── ttt-integration.test.ts  (~100 lines)
└── test-helpers.ts          (~80 lines)  # Shared mocks/factories
```

**Estimated total**: ~1,600 lines of tests

---

## Test Helpers (`test-helpers.ts`)

```typescript
// Factory functions for test data
export const createMockAttemptRecord = (overrides?: Partial<AttemptRecord>): AttemptRecord
export const createMockSyntheticTask = (overrides?: Partial<SyntheticTask>): SyntheticTask
export const createMockTRMState = (overrides?: Partial<TRMState>): TRMState
export const createMockVote = (overrides?: Partial<Vote>): Vote

// Effect test helper (from existing pattern)
export const runWithBun = <A, E>(
  program: Effect.Effect<A, E, FileSystem.FileSystem | Path.Path>
) => Effect.runPromise(program.pipe(Effect.provide(BunContext.layer)))

// Service layer test helper
export const runService = <A>(program: Effect.Effect<A, never, never>) =>
  Effect.runPromise(program)
```

---

## Execution Order

1. **Create test helpers** (`test-helpers.ts`)
2. **TRM unit tests** (state → halt → ema)
3. **SOAR unit tests** (hindsight → validation → selection → voting → ttt)
4. **Integration tests** (trm → soar → ttt)
5. **Original service tests** (skills → memory → reflexion → archivist → trainer)
6. **Run full suite**: `bun test src/learning/`

---

## Success Criteria

- [ ] All 8 TRM+SOAR modules have unit tests
- [ ] 100% of exported functions tested
- [ ] Edge cases covered (boundaries, empty inputs, null handling)
- [ ] Integration tests pass for all pipelines
- [ ] Original module service tests (skills, memory, reflexion, archivist, trainer)
- [ ] `bun test src/learning/` passes with 0 failures
- [ ] Coverage report shows >80% line coverage

---

## Scope Confirmation

**User selected**: Full system coverage

This includes:
1. **8 new TRM+SOAR unit tests** (~1,200 lines)
2. **3 integration tests** (~320 lines)
3. **5 original module service tests** (~500 lines)
4. **Test helpers** (~80 lines)

**Total estimated**: ~2,100 lines of tests

---

## Files to Create

| File | Lines | Priority |
|------|-------|----------|
| `src/learning/__tests__/test-helpers.ts` | ~80 | P0 |
| `src/learning/__tests__/trm-state.test.ts` | ~150 | P0 |
| `src/learning/__tests__/trm-halt.test.ts` | ~120 | P0 |
| `src/learning/__tests__/trm-ema.test.ts` | ~140 | P0 |
| `src/learning/__tests__/soar-hindsight.test.ts` | ~130 | P0 |
| `src/learning/__tests__/soar-validation.test.ts` | ~180 | P0 |
| `src/learning/__tests__/soar-selection.test.ts` | ~150 | P0 |
| `src/learning/__tests__/soar-voting.test.ts` | ~140 | P0 |
| `src/learning/__tests__/soar-ttt.test.ts` | ~200 | P0 |
| `src/learning/__tests__/trm-integration.test.ts` | ~100 | P1 |
| `src/learning/__tests__/soar-integration.test.ts` | ~120 | P1 |
| `src/learning/__tests__/ttt-integration.test.ts` | ~100 | P1 |
| `src/skills/__tests__/service.test.ts` | ~100 | P2 |
| `src/memory/__tests__/service.test.ts` | ~100 | P2 |
| `src/reflexion/__tests__/service.test.ts` | ~80 | P2 |
| `src/archivist/__tests__/service.test.ts` | ~120 | P2 |
| `src/trainer/__tests__/service.test.ts` | ~100 | P2 |
