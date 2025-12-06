# Phase 3: Bridge FM Learning Into Real Coding & Archivist

> **Status**: Ready for implementation
> **Prerequisites**: Phase 2 complete (commit 863fce8d7)
> **Goal**: Use learned skills from TB in real MechaCoder/FM coding runs, and start capturing long-term lessons via Archivist.

---

## Executive Summary

Phase 2 established FM + learning on Terminal-Bench. Phase 3 bridges that learning into **real coding tasks**:

1. **Task 3A**: Inject skills/memory into FM coding prompts (the infrastructure exists but isn't wired)
2. **Task 3B**: Hook Archivist into TB runs to create long-term lessons
3. **Task 3C**: Make FM coding agent consume Archivist lessons
4. **Task 3D**: Run a small FM training campaign to validate the loop

---

## Current State Analysis

### What's Implemented

| Component | Status | Location | Notes |
|-----------|--------|----------|-------|
| **SkillService** | Ready | `src/skills/service.ts` | `selectSkills()`, `formatForPrompt()` fully working |
| **MemoryService** | Ready | `src/memory/service.ts` | `getRelevantMemories()`, `formatForPrompt()` fully working |
| **FM Model Adapter** | Ready | `src/bench/model-adapter.ts` | Has skill/memory injection for TB |
| **Archivist Store** | Ready | `src/archivist/store.ts` | Trajectory JSONL persistence |
| **Pattern Extractor** | Ready | `src/archivist/extractor.ts` | FM-based pattern extraction |
| **ATIF System** | Ready | `src/atif/*.ts` | Full trajectory capture + streaming |

### Critical Gaps

| Gap | Impact | Fix |
|-----|--------|-----|
| **FM coding subagent has no skill/memory injection** | Skills learned from TB aren't used in coding | Task 3A |
| **FMSettings flags are defined but never checked** | `useSkills`, `useMemory`, `useReflection` are ignored in subagent-router | Task 3A |
| **Archivist not hooked into TB end-of-run** | No long-term lessons created from TB runs | Task 3B |
| **No lesson retrieval for prompts** | Archivist lessons not injected into FM coding | Task 3C |

---

## Task 3A: Inject Skills/Memory into FM Coding Subagent

**Goal**: Make skills and memories learned from TB runs automatically available to FM coding tasks in MechaCoder.

### Read First

- `src/agent/orchestrator/subagent-router.ts` (lines 92-328, especially 114-135)
- `src/skills/service.ts` and `src/skills/retrieval.ts`
- `src/memory/service.ts` and `src/memory/retrieval.ts`
- `src/bench/model-adapter.ts` (see how TB does skill injection around line 400-450)

### Current FM Prompt (lines 114-127 of subagent-router.ts)

```typescript
const systemPrompt = `You are an expert coding assistant. Complete the subtask below.

Tools available:
- read_file(path): Read a file
- write_file(path, content): Write a file
- edit_file(path, old_text, new_text): Edit a file
- run_command(command): Run a shell command

To use a tool, output:
<tool_call>{"name": "tool_name", "arguments": {"arg1": "value1"}}</tool_call>

When you have completed the subtask, output: SUBTASK_COMPLETE`;
```

This is **extremely minimal** - no context, no skills, no memories.

### Implementation

#### 1. Check FMSettings flags and inject context

In `subagent-router.ts`, modify `runFMSubagent()` to:

```typescript
// At function start, check FMSettings
const settings = options.fm ?? {};
const useSkills = settings.useSkills !== false; // default true
const useMemory = settings.useMemory ?? false;
const maxSkills = settings.maxSkills ?? 5;
const maxMemories = settings.maxMemories ?? 3;
const minSimilarity = settings.minSimilarity ?? 0.3;

// Build enhanced system prompt
let enhancedSystemPrompt = systemPrompt;

// Inject skills if enabled
if (useSkills) {
  const skillLayer = makeSkillServiceLive(settings.projectRoot ?? process.cwd());
  const skillContext = await Effect.runPromise(
    Effect.gen(function* () {
      const service = yield* SkillService;
      return yield* service.formatForPrompt(subtask.description, {
        topK: maxSkills,
        minSimilarity,
      });
    }).pipe(
      Effect.provide(skillLayer),
      Effect.catchAll(() => Effect.succeed(""))
    )
  );
  if (skillContext) {
    enhancedSystemPrompt += `\n\n${skillContext}`;
  }
}

// Inject memories if enabled
if (useMemory) {
  const memoryLayer = makeMemoryServiceLive(settings.projectRoot ?? process.cwd());
  const memoryContext = await Effect.runPromise(
    Effect.gen(function* () {
      const service = yield* MemoryService;
      return yield* service.formatForPrompt(subtask.description, {
        limit: maxMemories,
        minRelevance: minSimilarity,
      });
    }).pipe(
      Effect.provide(memoryLayer),
      Effect.catchAll(() => Effect.succeed(""))
    )
  );
  if (memoryContext) {
    enhancedSystemPrompt += `\n\n${memoryContext}`;
  }
}
```

#### 2. Track skill usage after completion

After FM successfully completes a subtask, record skill usage:

```typescript
// After SUBTASK_COMPLETE detected
if (useSkills && skillsUsedInPrompt.length > 0) {
  const skillLayer = makeSkillServiceLive(settings.projectRoot ?? process.cwd());
  await Effect.runPromise(
    Effect.gen(function* () {
      const service = yield* SkillService;
      for (const skillId of skillsUsedInPrompt) {
        yield* service.recordUsage(skillId, /* success */ true);
      }
    }).pipe(
      Effect.provide(skillLayer),
      Effect.catchAll(() => Effect.void)
    )
  );
}
```

#### 3. Add imports

```typescript
import { SkillService, makeSkillServiceLive } from "../skills/index.js";
import { MemoryService, makeMemoryServiceLive } from "../memory/index.js";
```

### Acceptance Criteria

- [ ] Running MechaCoder with FM shows skills in console output (debug logging)
- [ ] Skills from `.openagents/skills/library.jsonl` appear in FM prompts
- [ ] Memories from `.openagents/memories.jsonl` appear when `useMemory: true`
- [ ] `FMSettings.useSkills = false` disables skill injection
- [ ] Skill usage recorded after successful subtask completion

---

## Task 3B: Hook Archivist into TB End-of-Run

**Goal**: After each TB iteration, extract patterns and create long-term lessons in the Archivist store.

### Read First

- `src/archivist/service.ts` (especially `runArchive()`, `promotePatterns()`)
- `src/archivist/extractor.ts` (pattern extraction from trajectories)
- `src/cli/tbench-iterate.ts` (where to hook in - around line 974-1020)
- `src/training/episode-learner.ts` (existing learning hook)

### Current State

The `--learn` flag in `tbench-iterate.ts` already:
- Calls `createEpisodeLearner({ projectRoot })`
- Processes episode with `learner.processEpisode(episode)`
- Registers extracted skills with `SkillService`

But Archivist trajectory analysis is **not connected**.

### Implementation

#### 1. Create Archivist TB hook

Create `src/archivist/tbench.ts`:

```typescript
import { Effect } from "effect";
import { ArchivistService, makeArchivistServiceLive } from "./service.js";
import type { Episode } from "../bench/episode-store.js";
import type { TerminalBenchResults } from "../bench/terminal-bench.js";
import type { LearningResult } from "../training/episode-learner.js";

export interface TBenchArchivistInput {
  episode: Episode;
  iterResults: TerminalBenchResults;
  learningResult?: LearningResult | null;
  modelName: string;
  suiteName: string;
  atifDir?: string;
}

export interface TBenchArchivistResult {
  lessonsCreated: number;
  patternsExtracted: number;
  trajectoriesProcessed: number;
}

export const archiveFromTBenchIteration = (
  input: TBenchArchivistInput,
  projectRoot = process.cwd()
): Effect.Effect<TBenchArchivistResult, never, never> =>
  Effect.gen(function* () {
    // Load ATIF trajectories from the iteration output dir
    // Extract patterns from successful/failed task trajectories
    // Create lessons summarizing cross-task patterns
    // Return stats
  }).pipe(
    Effect.provide(makeArchivistServiceLive(projectRoot)),
    Effect.catchAll(() => Effect.succeed({
      lessonsCreated: 0,
      patternsExtracted: 0,
      trajectoriesProcessed: 0,
    }))
  );
```

#### 2. Hook into tbench-iterate.ts

After the `--learn` block (around line 1020), add:

```typescript
// Archivist: Extract patterns and create lessons
if (args.learn) {
  console.log(`  [Archivist] Processing iteration for pattern extraction...`);
  try {
    const archivistResult = await Effect.runPromise(
      archiveFromTBenchIteration({
        episode,
        iterResults,
        learningResult,
        modelName,
        suiteName: suite.name,
        atifDir: join(iterOutputDir, "atif"),
      })
    );

    if (archivistResult.patternsExtracted > 0) {
      console.log(`    [Archivist] Extracted ${archivistResult.patternsExtracted} patterns`);
    }
    if (archivistResult.lessonsCreated > 0) {
      console.log(`    [Archivist] Created ${archivistResult.lessonsCreated} lessons`);
    }
  } catch (e) {
    console.log(`    [Archivist] Error: ${e instanceof Error ? e.message : String(e)}`);
  }
}
```

#### 3. Define ArchivistLesson schema

If not already in `src/archivist/schema.ts`, add:

```typescript
export interface ArchivistLesson {
  id: string;
  source: "terminal-bench" | "mechacoder" | "manual";
  taskId?: string;
  suite?: string;
  model: string;
  summary: string;
  failurePatterns?: string[];
  successPatterns?: string[];
  skillsMentioned?: string[];
  confidence: number;
  createdAt: string;
}
```

### Acceptance Criteria

- [ ] Running `tbench-iterate --learn` shows Archivist processing messages
- [ ] `.openagents/archivist/lessons.jsonl` contains lessons after TB run
- [ ] Lessons include `source: "terminal-bench"`, model, suite name
- [ ] Patterns from successful/failed tasks are captured

---

## Task 3C: Make FM Coding Agent Consume Archivist Lessons

**Goal**: Let FM coding prompts include relevant lessons from Archivist, not just skills.

### Read First

- `src/archivist/store.ts` (add lesson retrieval methods)
- `src/agent/orchestrator/subagent-router.ts` (where to inject)
- `src/memory/retrieval.ts` (pattern for semantic retrieval)

### Implementation

#### 1. Add lesson query helper to Archivist

In `src/archivist/store.ts` or a new `src/archivist/retrieval.ts`:

```typescript
export interface LessonQuery {
  source?: "terminal-bench" | "mechacoder";
  model?: string;
  taskLabels?: string[];
  errorPatterns?: string[];
  limit?: number;
}

export const getRelevantLessons = (
  query: LessonQuery
): Effect.Effect<ArchivistLesson[], ArchivistStoreError> =>
  Effect.gen(function* () {
    const store = yield* ArchivistStore;
    const lessons = yield* store.listLessons();

    // Filter by source/model if specified
    let filtered = lessons;
    if (query.source) {
      filtered = filtered.filter(l => l.source === query.source);
    }
    if (query.model) {
      filtered = filtered.filter(l => l.model === query.model);
    }

    // Simple keyword matching on error patterns
    if (query.errorPatterns?.length) {
      filtered = filtered.filter(l =>
        l.failurePatterns?.some(fp =>
          query.errorPatterns!.some(ep => fp.includes(ep))
        )
      );
    }

    // Sort by recency and confidence
    filtered.sort((a, b) => {
      const confDiff = b.confidence - a.confidence;
      if (Math.abs(confDiff) > 0.1) return confDiff;
      return new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime();
    });

    return filtered.slice(0, query.limit ?? 3);
  });

export const formatLessonsForPrompt = (lessons: ArchivistLesson[]): string => {
  if (lessons.length === 0) return "";

  const formatted = lessons.map(l =>
    `- ${l.summary}${l.skillsMentioned?.length ? ` (Skills: ${l.skillsMentioned.join(", ")})` : ""}`
  ).join("\n");

  return `## Lessons from Similar Work\n\n${formatted}`;
};
```

#### 2. Inject lessons into FM coding prompts

In `subagent-router.ts`, after skill/memory injection:

```typescript
// Inject Archivist lessons if available
if (useSkills) { // Lessons are enabled with skills
  const archivistLayer = makeArchivistServiceLive(settings.projectRoot ?? process.cwd());
  const lessonsContext = await Effect.runPromise(
    Effect.gen(function* () {
      const lessons = yield* getRelevantLessons({
        source: "terminal-bench",
        model: "fm",
        limit: 3,
      });
      return formatLessonsForPrompt(lessons);
    }).pipe(
      Effect.provide(archivistLayer),
      Effect.catchAll(() => Effect.succeed(""))
    )
  );
  if (lessonsContext) {
    enhancedSystemPrompt += `\n\n${lessonsContext}`;
  }
}
```

### Acceptance Criteria

- [ ] FM coding prompts show "Lessons from Similar Work" section when lessons exist
- [ ] Lessons are filtered by model ("fm") and source ("terminal-bench")
- [ ] Most confident/recent lessons appear first
- [ ] Empty lessons section doesn't clutter prompt

---

## Task 3D: Run FM Training Campaign

**Goal**: Validate the complete loop by running a series of TB iterations and observing learning improvements.

### Prerequisites

- Tasks 3A, 3B, 3C implemented
- FM bridge running (`swift/foundation-bridge/run.sh`)
- FM mini-suite tasks defined

### Campaign Steps

#### 1. Configure FM defaults

Set in `.openagents/project.json`:

```json
{
  "tbench": {
    "defaultModel": "fm",
    "defaultSuite": "tasks/fm-mini-suite.json",
    "defaultLearning": {
      "skills": true,
      "memory": true,
      "reflexion": true,
      "learn": true
    }
  }
}
```

#### 2. Run baseline (no learning)

```bash
# 5 iterations without learning features
bun src/cli/tbench-iterate.ts \
  --suite tasks/fm-mini-suite.json \
  --model fm \
  --no-skills \
  --iterations 5 \
  --output ./results/fm-baseline
```

Record: pass rate, avg turns, avg duration

#### 3. Run with full learning

```bash
# 10 iterations with full learning stack
bun src/cli/tbench-iterate.ts \
  --suite tasks/fm-mini-suite.json \
  --model fm \
  --skills --memory --reflect --learn \
  --iterations 10 \
  --output ./results/fm-learning
```

#### 4. Inspect learning artifacts

```bash
# Check skills learned
cat .openagents/skills/library.jsonl | jq -s 'length'

# Check memories created
cat .openagents/memories.jsonl | jq -s 'length'

# Check Archivist lessons
cat .openagents/archivist/lessons.jsonl | jq -s 'length'

# Compare pass rates
cat ./results/fm-baseline/summary.md
cat ./results/fm-learning/summary.md
```

#### 5. Run MechaCoder coding task

```bash
# Small contained task to see if skills transfer
bun run mechacoder --cc-only=false
```

Watch for:
- Skills injected into FM prompt
- Lessons appearing in prompts
- Whether FM performs better on similar tasks

### Success Metrics

| Metric | Baseline | Learning | Target |
|--------|----------|----------|--------|
| Pass rate | ~70% | ? | >80% |
| Skills in library | 0 | ? | >5 |
| Lessons created | 0 | ? | >3 |
| Skill usage in coding | 0 | ? | >0 |

---

## File Changes Summary

| File | Task | Changes |
|------|------|---------|
| `src/agent/orchestrator/subagent-router.ts` | 3A, 3C | Inject skills/memory/lessons into FM prompts |
| `src/archivist/tbench.ts` | 3B | New: TB-specific Archivist hook |
| `src/archivist/retrieval.ts` | 3C | New: Lesson query and formatting |
| `src/archivist/schema.ts` | 3B | Add ArchivistLesson type if missing |
| `src/cli/tbench-iterate.ts` | 3B | Hook Archivist after learning |
| `.openagents/project.json` | 3D | FM defaults configuration |

---

## Dependencies

```
Task 3A (skill/memory injection)
    ↓
Task 3B (Archivist TB hook)
    ↓
Task 3C (lesson retrieval)
    ↓
Task 3D (validation campaign)
```

Tasks 3A and 3B can be done in parallel. 3C requires 3B (needs lessons to exist). 3D requires all others.

---

## Next Phase Preview (Phase 4)

After Phase 3 validates the learning loop:

- **TRM/SOAR integration**: Advanced meta-learning on top of skill/memory/Archivist
- **Gym loops**: Automated training over larger TB suites
- **Cross-project learning**: Share lessons between different codebases
- **FM model fine-tuning**: Use ATIF trajectories for SFT/RL

---

## Quick Start

```bash
# 1. Start FM bridge
cd swift/foundation-bridge && ./run.sh

# 2. Verify FM health
curl http://localhost:11435/health

# 3. Run with learning
bun src/cli/tbench-iterate.ts \
  --suite tasks/fm-mini-suite.json \
  --model fm \
  --skills --memory --reflect --learn \
  --iterations 5

# 4. Check artifacts
ls -la .openagents/skills/
ls -la .openagents/memories.jsonl
ls -la .openagents/archivist/
```
