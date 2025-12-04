# Add Reflexion Pattern to Golden Loop v2 (Effect-First)

## Overview

Add the Reflexion pattern (NeurIPS 2023) to MechaCoder's Golden Loop v2. When a subtask fails, generate a verbal self-reflection analyzing what went wrong, store it, and inject it into the context for retry attempts.

**Evidence from Research:**
- +11% on HumanEval (91% vs 80%)
- Works through verbal self-reflection stored in episodic memory
- Best results with `LAST_ATTEMPT_AND_REFLEXION` strategy

**Current Gap:**
The orchestrator tracks `failureCount` and `lastFailureReason` but does NOT generate or use reflections. Retries just get the raw error - no analysis of what went wrong.

---

## Recommended Approach: Effect-First ReflectionService

Create a proper Effect service that:
1. Generates reflections via LLM after failures
2. Stores reflections in Archivist-compatible JSONL format
3. Retrieves and injects reflections into retry prompts
4. Uses Effect patterns (Context.Tag, Layer, Effect.gen)

### Why This Approach

| Alternative | Pros | Cons |
|-------------|------|------|
| **Minimal (inline)** | Fast to implement | Not Effect-first, no persistence, not testable |
| **Full Archivist** | Complete system | Too large scope, delays Reflexion benefits |
| **Effect Service** ✓ | Proper architecture, testable, Archivist-compatible | Slightly more code |

### Key Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| **Generation** | Claude Code (LLM) | High-quality reflections worth ~$0.01 per failure. Nuanced analysis beats templates. |
| **Storage** | `.openagents/memory/reflections.jsonl` | Persists across sessions, Archivist-compatible, enables later analysis. |
| **Default** | Enabled by default | All projects benefit automatically. Can disable via `reflexion.enabled: false` in project.json. |

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                     GOLDEN LOOP WITH REFLEXION                       │
│                                                                      │
│  Task → Decompose → Execute → Test                                   │
│                               │                                      │
│                               ├──[PASS]─→ Commit → Update → Log      │
│                               │                                      │
│                               └──[FAIL]─→ ┌─────────────────────┐   │
│                                           │ ReflectionService    │   │
│                                           │ .generate(failure)   │   │
│                                           └──────────┬──────────┘   │
│                                                      │              │
│                                                      ▼              │
│                                           ┌─────────────────────┐   │
│                                           │ Store in            │   │
│                                           │ .openagents/memory/ │   │
│                                           │ reflections.jsonl   │   │
│                                           └──────────┬──────────┘   │
│                                                      │              │
│                                                      ▼              │
│                                           ┌─────────────────────┐   │
│                                           │ Retry with          │   │
│                                           │ reflection context  │   │
│                                           └─────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
```

---

## File Structure

```
src/agent/orchestrator/reflection/
├── index.ts              # Public exports
├── schema.ts             # Reflection, FailureContext types (Effect/Schema)
├── errors.ts             # ReflectionError class
├── service.ts            # ReflectionService interface + Context.Tag
├── service-impl.ts       # makeReflectionService implementation
├── storage.ts            # JSONL storage (Archivist-compatible format)
├── generator.ts          # LLM-based reflection generation
├── prompt.ts             # Prompt templates
└── layer.ts              # ReflectionServiceLive + ReflectionServiceTest layers
```

---

## Type Definitions

### Reflection Schema

```typescript
// src/agent/orchestrator/reflection/schema.ts
import * as S from "effect/Schema";

export const ReflectionCategory = S.Literal(
  "root_cause",        // What caused the failure
  "misconception",     // What the agent misunderstood
  "environment",       // Environment/dependency issues
  "approach_error",    // Wrong approach taken
  "edge_case",         // Missed edge case
  "verification"       // Verification-specific insight
);

export const Reflection = S.Struct({
  id: S.String,
  sessionId: S.String,
  taskId: S.String,
  subtaskId: S.String,
  attemptNumber: S.Number,
  category: ReflectionCategory,
  analysis: S.String,           // What went wrong (1-2 sentences)
  suggestion: S.String,         // What to do differently (1-2 sentences)
  actionItems: S.Array(S.String),
  confidence: S.Number,         // 0-1
  createdAt: S.String,
});
export type Reflection = S.Schema.Type<typeof Reflection>;

export const FailureContext = S.Struct({
  id: S.String,
  sessionId: S.String,
  taskId: S.String,
  subtaskId: S.String,
  subtaskDescription: S.String,
  attemptNumber: S.Number,
  failureType: S.Literal("test_failure", "typecheck_failure", "runtime_error", "timeout"),
  errorOutput: S.String,
  filesModified: S.Array(S.String),
  previousReflections: S.Array(S.String),  // Avoid repetition
  createdAt: S.String,
});
export type FailureContext = S.Schema.Type<typeof FailureContext>;
```

### ReflectionService Interface

```typescript
// src/agent/orchestrator/reflection/service.ts
import { Context, Effect } from "effect";
import type { Reflection, FailureContext } from "./schema.js";
import type { ReflectionError } from "./errors.js";

export interface ReflectionService {
  generate(failure: FailureContext): Effect.Effect<Reflection, ReflectionError>;
  getRecent(subtaskId: string, limit?: number): Effect.Effect<Reflection[], ReflectionError>;
  save(reflection: Reflection): Effect.Effect<void, ReflectionError>;
  formatForPrompt(reflections: Reflection[]): Effect.Effect<string, ReflectionError>;
}

export class ReflectionServiceTag extends Context.Tag("ReflectionService")<
  ReflectionServiceTag,
  ReflectionService
>() {}
```

---

## Integration Points

### 1. Orchestrator Failure Handler (orchestrator.ts ~line 662)

```typescript
// After subtask failure is detected, before Healer runs
if (config.reflectionService && subtask.failureCount && subtask.failureCount > 0) {
  const failureContext = buildFailureContext(subtask, result, state, progress);

  const reflection = yield* config.reflectionService.generate(failureContext).pipe(
    Effect.catchAll((e) => {
      console.log("[Reflexion] Failed to generate reflection:", e.message);
      return Effect.succeed(null);
    })
  );

  if (reflection) {
    yield* config.reflectionService.save(reflection).pipe(
      Effect.catchAll(() => Effect.void)
    );
  }
}
```

### 2. Subagent Prompt Builder (claude-code-subagent.ts ~line 84)

```typescript
// In defaultBuildPrompt, after failure context section
if (subtask.failureCount && subtask.failureCount > 0) {
  // Existing failure context...

  // NEW: Get and inject reflections
  const reflections = await reflectionService?.getRecent(subtask.id, 3);
  if (reflections && reflections.length > 0) {
    const reflectionContext = await reflectionService?.formatForPrompt(reflections);
    prompt += reflectionContext;
  }
}
```

### 3. OrchestratorConfig Extension (types.ts)

```typescript
export interface OrchestratorConfig {
  // ... existing fields ...

  /** Optional ReflectionService for Reflexion pattern */
  reflectionService?: ReflectionService;

  /** Reflexion configuration */
  reflexionConfig?: {
    enabled: boolean;
    maxReflectionsPerRetry: number;  // Default: 3
    useLLM: boolean;                  // Use LLM vs heuristic
  };
}
```

---

## Reflection Generation

### Prompt Template

```typescript
const REFLECTION_PROMPT = `You are analyzing a failed coding task attempt.

## Failed Subtask
{subtaskDescription}

## Attempt Number
{attemptNumber}

## Error Output
\`\`\`
{errorOutput}
\`\`\`

## Previous Reflections (avoid repeating)
{previousReflections}

Generate a concise reflection:

1. **What went wrong**: Root cause in 1-2 sentences
2. **What to do differently**: Specific alternative approach

Respond in JSON:
{
  "category": "root_cause" | "misconception" | "approach_error" | "edge_case",
  "analysis": "...",
  "suggestion": "...",
  "actionItems": ["...", "..."],
  "confidence": 0.0-1.0
}`;
```

### Prompt Injection Format

```typescript
const formatForPrompt = (reflections: Reflection[]): string => {
  if (reflections.length === 0) return "";

  return `
## Reflections from Previous Attempts

Learn from these insights before proceeding:

${reflections.map(r => `
### Attempt ${r.attemptNumber}
- **What went wrong**: ${r.analysis}
- **What to do differently**: ${r.suggestion}
- **Action items**: ${r.actionItems.map(a => `\n  - ${a}`).join("")}
`).join("\n")}

You MUST address these issues. Do NOT repeat the same mistakes.
`;
};
```

---

## Storage (Archivist-Compatible)

### File Location
```
.openagents/memory/reflections.jsonl
```

### Format
```jsonl
{"id":"ref-abc123","sessionId":"sess-456","taskId":"oa-xyz","subtaskId":"sub-1","attemptNumber":1,"category":"root_cause","analysis":"Tests failed because fixture data was stale","suggestion":"Regenerate fixtures before running tests","actionItems":["Check fixture dates","Run fixture generator"],"confidence":0.8,"createdAt":"2025-12-04T10:30:00Z"}
```

This format is compatible with the Archivist's `AgentMemory` schema, enabling future migration.

---

## Layer Composition

```typescript
// src/agent/orchestrator/reflection/layer.ts
export const ReflectionServiceLive = (config: ReflexionConfig, openagentsDir: string) =>
  Layer.effect(
    ReflectionServiceTag,
    makeReflectionService(config, openagentsDir)
  );

export const ReflectionServiceTest = Layer.succeed(
  ReflectionServiceTag,
  {
    generate: () => Effect.succeed(mockReflection),
    getRecent: () => Effect.succeed([]),
    save: () => Effect.void,
    formatForPrompt: () => Effect.succeed(""),
  }
);
```

### Usage in Entry Points

```typescript
// In do-one-task.ts or overnight.ts
const reflectionLayer = projectConfig.reflexion?.enabled
  ? ReflectionServiceLive(projectConfig.reflexion, openagentsDir)
  : ReflectionServiceTest;

const program = runOrchestrator(config).pipe(
  Effect.provide(reflectionLayer),
  Effect.provide(BunContext.layer)
);
```

---

## Critical Files to Modify

| File | Changes |
|------|---------|
| `src/agent/orchestrator/types.ts` | Add `reflectionService` and `reflexionConfig` to OrchestratorConfig |
| `src/agent/orchestrator/orchestrator.ts` | Call reflection generation after failure (~line 662) |
| `src/agent/orchestrator/claude-code-subagent.ts` | Inject reflections into retry prompt (~line 84) |
| `src/tasks/schema.ts` | Add `ReflexionConfig` to ProjectConfig |
| `docs/mechacoder/GOLDEN-LOOP-v2.md` | Document Reflexion in Section 4.1 |

## Files to Create

| File | Purpose |
|------|---------|
| `src/agent/orchestrator/reflection/index.ts` | Public exports |
| `src/agent/orchestrator/reflection/schema.ts` | Effect/Schema types |
| `src/agent/orchestrator/reflection/errors.ts` | ReflectionError class |
| `src/agent/orchestrator/reflection/service.ts` | Interface + Context.Tag |
| `src/agent/orchestrator/reflection/service-impl.ts` | Implementation |
| `src/agent/orchestrator/reflection/storage.ts` | JSONL storage |
| `src/agent/orchestrator/reflection/generator.ts` | LLM generation |
| `src/agent/orchestrator/reflection/prompt.ts` | Prompt templates |
| `src/agent/orchestrator/reflection/layer.ts` | Live + Test layers |

---

## Testing Strategy

```typescript
// src/agent/orchestrator/reflection/__tests__/service.test.ts
describe("ReflectionService", () => {
  test("generates reflection from failure context", async () => {
    // Mock LLM response, verify parsing
  });

  test("stores and retrieves reflections", async () => {
    // Test JSONL storage round-trip
  });

  test("formats reflections for prompt injection", () => {
    // Verify markdown output format
  });

  test("limits to max reflections per retry", () => {
    // Test FIFO with limit=3
  });
});
```

---

## Success Criteria

1. **Reflection generated** after each subtask failure
2. **Reflection stored** in `.openagents/memory/reflections.jsonl`
3. **Reflection injected** into retry prompt with formatted context
4. **Max 3 reflections** per retry (paper shows diminishing returns)
5. **Graceful degradation** if reflection generation fails
6. **Tests pass** for the reflection module
7. **Documentation updated** in GOLDEN-LOOP-v2.md
