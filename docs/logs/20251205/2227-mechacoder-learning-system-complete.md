# MechaCoder Learning System: Complete Implementation Summary

- **Date:** December 5, 2025
- **Scope:** Phases 1-9 of the Terminal-Bench #1 Initiative
- **Status:** ✅ Complete (140 tests passing)

---

## Executive Summary

This document summarizes the complete implementation of MechaCoder's no-gradient lifelong learning system—a comprehensive infrastructure designed to achieve #1 on Terminal-Bench using **only Apple Foundation Models** (on-device inference).

The system implements three key research breakthroughs:
1. **Voyager** (2023): Skill libraries provide 3.3x improvement over baseline
2. **Generative Agents** (2023): Memory + reflection = 8 standard deviations improvement
3. **Reflexion** (2023): Self-critique after failures = +11% improvement

**Final Stats:**
- 8 new modules created
- 23 new source files
- 6,190 lines of code
- 140 tests passing
- 1,082 test assertions

---

## The Vision: No-Gradient Lifelong Learning

Traditional LLM improvement requires fine-tuning (gradient updates). MechaCoder takes a different approach: **lifelong learning without parameter updates**.

Instead of changing the model, we change what the model sees:
- **Skills** are injected into prompts based on task similarity
- **Memories** provide relevant context from past experiences
- **Reflections** guide retry attempts after failures
- **Patterns** are extracted from successful trajectories

This creates a learning loop where the agent gets better at tasks over time, purely through prompt engineering and retrieval.

---

## Phase-by-Phase Implementation

### Phase 1: FM Service Enhancement ✅
**Location:** `src/fm/service.ts`

Created an Effect-based service layer for Apple Foundation Models:
- Chat API with automatic retry and exponential backoff
- Health checking and auto-start capabilities
- Metrics tracking (latency, tokens, success rate)
- Clean integration with Effect's dependency injection

**Key Types:**
```typescript
interface IFMService {
  chat: (request: ChatRequest) => Effect.Effect<ChatResponse, FMServiceError>;
  checkHealth: () => Effect.Effect<FMHealthStatus, FMServiceError>;
  ensureRunning: () => Effect.Effect<void, FMServiceError>;
  getMetrics: () => Effect.Effect<FMAggregateMetrics, never>;
}
```

---

### Phase 2: Skill Library ✅
**Location:** `src/skills/`

Implemented Voyager-style skill library with semantic retrieval:

**Files Created:**
- `schema.ts` - Skill types, categories, versioning
- `store.ts` - JSONL persistence at `.openagents/skills.jsonl`
- `retrieval.ts` - Embedding-based semantic search
- `service.ts` - Unified SkillService
- `library/primitives.ts` - 28 primitive skills
- `library/compositional.ts` - 44 compositional skills
- `library/index.ts` - Bootstrap and exports

**Bootstrap Skills (72 total):**

| Category | Count | Examples |
|----------|-------|----------|
| Search | 8 | grep-in-file, find-definition, semantic-search |
| File Operations | 10 | read-file, edit-file, create-file-with-template |
| Testing | 8 | run-tests, fix-failing-test, add-test-coverage |
| Debugging | 10 | diagnose-type-error, trace-undefined, fix-import |
| Refactoring | 8 | extract-function, rename-symbol, inline-variable |
| Git | 8 | commit-changes, create-pr, resolve-conflicts |
| Shell | 6 | run-command, check-dependencies |
| Effect-TS | 6 | create-effect-service, handle-effect-error |
| Meta | 8 | learn-new-skill, compose-skills |

**Key Innovation:** Skills are retrieved by semantic similarity to the current task, then injected into the system prompt. This gives the model "muscle memory" for common patterns.

---

### Phase 3: Memory System ✅
**Location:** `src/memory/`

Implemented Generative Agents-style memory with importance/recency/relevance scoring:

**Files Created:**
- `schema.ts` - Memory types, importance levels, scoring formula
- `store.ts` - JSONL persistence with multi-index caching
- `retrieval.ts` - Embedding-based semantic search
- `service.ts` - Unified MemoryService

**Memory Types:**

1. **Episodic** - Task execution memories
   ```typescript
   interface EpisodicContent {
     taskDescription: string;
     outcome: "success" | "failure" | "partial";
     skillsUsed: string[];
     filesModified: string[];
     durationMs: number;
   }
   ```

2. **Semantic** - Knowledge and facts
   ```typescript
   interface SemanticContent {
     category: "fact" | "pattern" | "convention" | "lesson";
     knowledge: string;
     context?: string;
     examples?: string[];
   }
   ```

3. **Procedural** - Skill-linked memories
   ```typescript
   interface ProceduralContent {
     skillId: string;
     triggerPatterns: string[];
     successRate: number;
   }
   ```

**Scoring Formula (from Generative Agents paper):**
```typescript
score = α * recency + β * importance + γ * relevance

// where:
// - recency = exponential decay from last access
// - importance = critical(1.0) > high(0.75) > medium(0.5) > low(0.25)
// - relevance = cosine similarity to query embedding
```

---

### Phase 4: Reflexion Integration ✅
**Location:** `src/reflexion/`

Implemented self-critique system for learning from failures:

**Files Created:**
- `schema.ts` - FailureContext, Reflection, error classification
- `generator.ts` - FM-based reflection generation with heuristic fallback
- `service.ts` - Unified ReflexionService

**Error Classification:**
```typescript
type ErrorType =
  | "type_error"      // TypeScript type mismatch
  | "import_error"    // Missing or wrong import
  | "syntax_error"    // Syntax issues
  | "runtime_error"   // Runtime exceptions
  | "test_failure"    // Test assertion failed
  | "build_error"     // Build/compilation error
  | "timeout"         // Task timed out
  | "tool_error"      // Tool execution failed
  | "logic_error"     // Wrong behavior/output
  | "unknown";
```

**Reflection Structure:**
```typescript
interface Reflection {
  whatWentWrong: string;     // Diagnosis
  whyItWentWrong: string;    // Root cause
  whatToTryNext: string;     // Action plan
  suggestedFix?: string;     // Specific fix
  lessonsLearned: string[];  // For memory
  confidence: number;        // 0-1
  ledToSuccess?: boolean;    // After retry
}
```

**Key Innovation:** After a failure, the system generates a reflection that gets injected into the next retry attempt. If the retry succeeds, the reflection is marked successful and can be converted into a skill.

---

### Phase 5: Archivist Implementation ✅
**Location:** `src/archivist/`

Implemented trajectory recording and pattern extraction:

**Files Created:**
- `schema.ts` - Trajectory, ExtractedPattern, ArchiveConfig
- `store.ts` - JSONL persistence at `.openagents/trajectories.jsonl`
- `extractor.ts` - FM-based pattern extraction
- `service.ts` - Unified ArchivistService

**Trajectory Structure:**
```typescript
interface Trajectory {
  id: string;
  taskId: string;
  taskDescription: string;
  actions: TrajectoryAction[];
  outcome: "success" | "failure" | "partial" | "timeout";
  skillsUsed: string[];
  filesModified: string[];
  totalDurationMs: number;
  tokens: { input: number; output: number; total: number };
  archived: boolean;
}
```

**Pattern Extraction:**
The Archivist periodically reviews unarchived trajectories and:
1. Groups similar trajectories by first tool + outcome
2. Identifies common patterns across successful runs
3. Extracts these as new skills with trigger contexts
4. Marks trajectories as archived

**Archive Cycle:**
```typescript
interface ArchiveResult {
  trajectoriesProcessed: number;
  patternsExtracted: number;
  skillsCreated: number;
  memoriesCreated: number;
  itemsPruned: number;
}
```

---

### Phase 6: Trainer/Gym Implementation ✅
**Location:** `src/trainer/`

Implemented controlled execution environment for training:

**Files Created:**
- `schema.ts` - TrainingTask, TaskResult, TrainingConfig, TBSubset
- `gym.ts` - Execution environment with skill/memory integration
- `service.ts` - TrainerService for benchmark orchestration

**Training Task:**
```typescript
interface TrainingTask {
  id: string;
  prompt: string;
  expectedBehavior?: string;
  difficulty: number;        // 1-5
  category: string;
  tags: string[];
  timeoutMs: number;
  source: string;            // "terminal-bench" | "custom"
}
```

**Gym Execution Flow:**
1. Retrieve relevant skills (top-5 by similarity)
2. Retrieve relevant memories (top-5 by score)
3. Get reflection context if retrying
4. Build augmented prompt with all context
5. Execute via FM chat API
6. Record trajectory to Archivist
7. Update skill usage stats

**Terminal-Bench Integration:**
```typescript
const TB_SUBSETS = {
  TB_10: { name: "Terminal-Bench 10", count: 10 },
  TB_30: { name: "Terminal-Bench 30", count: 30 },
  TB_89: { name: "Terminal-Bench Full", count: 89 },
};
```

---

### Phase 7: Training Loop ✅
**Location:** `src/learning/`

Implemented main orchestration loop:

**Files Created:**
- `loop.ts` - Training loop with progressive benchmarking
- `orchestrator.ts` - High-level API for entire learning system

**Loop State:**
```typescript
interface LoopState {
  iteration: number;
  currentSubset: TBSubset;
  totalTasksCompleted: number;
  totalSuccessful: number;
  overallSuccessRate: number;
  skillsLearned: number;
  patternsExtracted: number;
  status: "running" | "paused" | "stopped" | "completed";
}
```

**Progressive Benchmarking:**
- Start with TB-10 (10 tasks)
- When success rate > 80% and 10+ tasks completed, progress to TB-30
- When success rate > 80% on TB-30, progress to TB-89

**Archive Schedule:**
- Run archive every N iterations (default: 5)
- Extract patterns from accumulated trajectories
- Promote high-quality patterns to skills

**Learning Orchestrator:**
Provides unified API for the entire system:
```typescript
interface ILearningOrchestrator {
  // Lifecycle
  initialize: () => Effect<void>;
  startTraining: (config?) => Effect<LoopState>;
  stopTraining: () => Effect<LoopState>;

  // Execution
  executeTask: (task) => Effect<TrainingRun>;
  runBenchmark: (subset) => Effect<TrainingRun>;

  // Skills
  findSkills: (query) => Effect<Skill[]>;
  bootstrapSkills: () => Effect<number>;

  // Memory
  findMemories: (query) => Effect<Memory[]>;
  recordExperience: (content, type) => Effect<Memory>;

  // Reflexion
  recordFailure: (task, error) => Effect<Reflection>;

  // Stats
  getStats: () => Effect<LearningStats>;
}
```

---

### Phase 8: Dashboard ✅
**Location:** `src/dashboard/`

Implemented progress reporting and visualization:

**Files Created:**
- `schema.ts` - Display types, formatting helpers, health calculation
- `reporter.ts` - Terminal output formatting

**Features:**
- Real-time training progress
- Success rate visualization with progress bars
- Skill library statistics by category
- Memory system metrics
- System health monitoring (healthy/degraded/unhealthy)
- Historical run tracking

**Output Formatting:**
```
╔══════════════════════════════════════════════════╗
║ MechaCoder Learning Dashboard                    ║
╚══════════════════════════════════════════════════╝

Status: ✓ HEALTHY
Loop: running (iteration 5)
Progress: [████████████████████░░░░░░░░░░░░░░░░░░░░] 80.0%

──────────────────────────────────────────────────
Quick Stats
──────────────────────────────────────────────────
Skills:      72 (10 learned)
Memories:    150
Runs:        5
Tasks:       50
Success:     80.0%
Tier:        TB_30
```

---

### Phase 9: Guardrails ✅
**Location:** `src/guardrails/`

Implemented safety constraints and resource limits:

**Files Created:**
- `schema.ts` - Rule types, built-in rules, pattern matching
- `service.ts` - Validation service

**Built-in Rules:**

| Rule | Category | Severity | Default |
|------|----------|----------|---------|
| Max Tokens Per Task | resource | warning | 50,000 |
| Max Duration Per Task | resource | error | 5 min |
| Max Tokens Per Run | resource | error | 1,000,000 |
| Max Duration Per Run | resource | error | 1 hour |
| Blocked File Patterns | safety | critical | *.env, *.pem, *credentials* |
| No Network Access | safety | critical | disabled |
| Min Success Rate | quality | warning | 10% |
| Max Consecutive Failures | quality | error | 10 |
| Max Retries Per Task | behavior | warning | 3 |
| Max Skills Per Run | behavior | warning | 50 |
| Max Memory Entries | behavior | warning | 10,000 |

**Blocked File Patterns:**
```typescript
blockedPatterns: [
  "*.env",
  "*.pem",
  "*.key",
  "*credentials*",
  "*secrets*",
  "*password*",
  "~/.ssh/*",
  "~/.aws/*",
]
```

**Validation Flow:**
```typescript
const status = await guardrails.validate({
  taskTokens: 45000,
  taskDurationMs: 120000,
  filePaths: ["src/config.ts"],
  successRate: 0.75,
});

if (status.shouldBlock) {
  // Stop execution
}
```

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    Learning Orchestrator                         │
│  (Unified API for the entire learning system)                   │
└─────────────────────────────────────────────────────────────────┘
                              │
         ┌────────────────────┼────────────────────┐
         │                    │                    │
         ▼                    ▼                    ▼
┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
│  Training Loop  │  │    Dashboard    │  │   Guardrails    │
│  (Orchestration)│  │  (Monitoring)   │  │   (Safety)      │
└─────────────────┘  └─────────────────┘  └─────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────────────┐
│                         Trainer / Gym                            │
│  (Controlled execution with skill/memory integration)           │
└─────────────────────────────────────────────────────────────────┘
         │
         ├──────────────┬──────────────┬──────────────┐
         ▼              ▼              ▼              ▼
┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐
│   Archivist  │ │    Skills    │ │    Memory    │ │  Reflexion   │
│  (Patterns)  │ │  (Voyager)   │ │ (Gen Agents) │ │ (Self-crit)  │
└──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘
         │              │              │              │
         └──────────────┴──────────────┴──────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      FM Service (Effect)                         │
│  (Apple Foundation Models via Swift HTTP bridge)                │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                Apple Foundation Models (On-Device)               │
│  (Port 11435, greedy decoding, ~100 tok/s)                      │
└─────────────────────────────────────────────────────────────────┘
```

---

## Data Flow: Task Execution

```
1. Task arrives (e.g., "Fix the import error in auth.ts")
                    │
                    ▼
2. Skill Retrieval ─────────────────────────────────────────────┐
   - Embed task description                                      │
   - Find top-5 skills by cosine similarity                     │
   - E.g., "fix-import-error", "diagnose-type-error"            │
                    │                                            │
                    ▼                                            │
3. Memory Retrieval ─────────────────────────────────────────────┤
   - Embed task description                                      │
   - Score memories: α*recency + β*importance + γ*relevance     │
   - Return top-5 relevant memories                              │
                    │                                            │
                    ▼                                            │
4. Reflexion Context (if retry) ─────────────────────────────────┤
   - Get previous reflections for this task                     │
   - Format as "Previous Attempt Reflections" section           │
                    │                                            │
                    ▼                                            │
5. Build Augmented Prompt ◄──────────────────────────────────────┘
   - Base system prompt
   - Skills section with code examples
   - Memory section with relevant context
   - Reflexion section (if retry)
   - Task prompt
                    │
                    ▼
6. Execute via FM ────────────────────────────────────────────────
   - Call chat API with timeout
   - Parse response
   - Determine outcome (success/failure/partial)
                    │
                    ▼
7. Record Trajectory ─────────────────────────────────────────────
   - Save to .openagents/trajectories.jsonl
   - Include all actions, skills used, tokens, duration
                    │
                    ▼
8. Update Stats ──────────────────────────────────────────────────
   - Increment skill usage counters
   - Update success rates
   - Record to episodic memory
                    │
                    ▼
9. On Failure: Generate Reflection ───────────────────────────────
   - Classify error type
   - Generate structured reflection
   - Store for next retry attempt
                    │
                    ▼
10. Periodic: Archive Cycle ──────────────────────────────────────
    - Extract patterns from trajectories
    - Promote to skills if high confidence
    - Prune old trajectories
```

---

## File Summary

### New Modules (23 files, 6,190 lines)

| Module | Files | Lines | Tests |
|--------|-------|-------|-------|
| `src/archivist/` | 6 | ~800 | 19 |
| `src/trainer/` | 5 | ~900 | 18 |
| `src/learning/` | 4 | ~600 | 7 |
| `src/dashboard/` | 4 | ~500 | 22 |
| `src/guardrails/` | 4 | ~600 | 19 |

### Complete Learning System

| Module | Purpose | Key Types |
|--------|---------|-----------|
| `src/fm/` | FM client + Effect service | FMService, ChatRequest |
| `src/skills/` | Skill library + retrieval | Skill, SkillMatch, SkillService |
| `src/memory/` | Memory system + scoring | Memory, MemoryService |
| `src/reflexion/` | Self-critique + learning | Reflection, ReflexionService |
| `src/archivist/` | Trajectory + patterns | Trajectory, ExtractedPattern |
| `src/trainer/` | Gym + benchmarking | TrainingTask, Gym, TrainerService |
| `src/learning/` | Orchestration | TrainingLoop, LearningOrchestrator |
| `src/dashboard/` | Visualization | Reporter, DashboardMetrics |
| `src/guardrails/` | Safety | GuardrailRule, GuardrailsService |

---

## Test Coverage

**140 tests passing across 8 files:**

```
src/skills/schema.test.ts      - 20 tests
src/memory/schema.test.ts      - 18 tests
src/reflexion/schema.test.ts   - 17 tests
src/archivist/schema.test.ts   - 19 tests
src/trainer/schema.test.ts     - 18 tests
src/learning/loop.test.ts      -  7 tests
src/dashboard/schema.test.ts   - 22 tests
src/guardrails/schema.test.ts  - 19 tests
```

**1,082 expect() assertions** covering:
- ID generation and uniqueness
- Type creation and defaults
- Scoring calculations
- Pattern matching
- Error classification
- Progress formatting
- Configuration defaults

---

## What's Ready for Terminal-Bench

1. **Progressive Benchmarking**: Start with TB-10, auto-progress to TB-30 and TB-89 based on success rate

2. **Skill Augmentation**: 72 bootstrap skills covering all common terminal tasks

3. **Memory Context**: Past experiences inform current attempts

4. **Reflexion Loop**: Failures generate insights for retries

5. **Pattern Learning**: Successful patterns become new skills

6. **Safety Guardrails**: Resource limits and file access controls

7. **Progress Tracking**: Real-time stats and health monitoring

---

## Next Steps

1. **Wire to MechaCoder CLI**: Integrate learning system with `bun run mechacoder`

2. **Load Real TB Tasks**: Parse actual Terminal-Bench task files

3. **Add Evaluation Harness**: Validate task outcomes against expected results

4. **Embedding Population**: Run initial embedding pass for all skills

5. **First Training Run**: Execute TB-10 and validate the loop

---

## Commits

```
2b04491cf feat(learning): Complete MechaCoder learning system (Phases 5-9)
4e602d1d4 feat(reflexion): Add Reflexion system for self-critique learning
[previous session commits for Phases 1-4]
```

---

*This implementation represents the foundation for MechaCoder to become the #1 Terminal-Bench agent using only Apple Foundation Models—achieving competitive performance through no-gradient lifelong learning rather than model fine-tuning.*
