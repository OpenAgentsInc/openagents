# 1830 Reflexion Pattern Implementation Log

**Task ID:** oa-7803b9
**Feature:** Add Reflexion pattern to Golden Loop v2
**Approach:** Effect-first architecture

---

## Overview

Implemented the Reflexion pattern (from NeurIPS 2023 paper "Reflexion: Language Agents with Verbal Reinforcement Learning") for the MechaCoder orchestrator. This pattern provides verbal self-reflection on failures, storing reflections in episodic memory and injecting them into retry prompts to improve agent learning.

**Key insight from paper:** +11% on HumanEval benchmark by having agents reflect on their mistakes.

---

## Files Created

### Reflection Module (`src/agent/orchestrator/reflection/`)

| File | Purpose |
|------|---------|
| `schema.ts` | Effect/Schema types: `Reflection`, `FailureContext`, `ReflexionConfig`, `ReflectionCategory`, `FailureType` |
| `errors.ts` | `ReflectionError` class with typed reasons: `generation_failed`, `parse_error`, `storage_error`, `not_found`, `timeout` |
| `service.ts` | `IReflectionService` interface + `ReflectionService` Context.Tag for Effect DI |
| `prompt.ts` | LLM prompt templates: `REFLECTION_GENERATION_PROMPT`, `buildGenerationPrompt()`, `formatReflectionsForPrompt()`, `parseReflectionResponse()` |
| `storage.ts` | JSONL persistence: `makeFileStorage()` for `.openagents/memory/reflections.jsonl`, `makeMemoryStorage()` for tests |
| `generator.ts` | LLM generation via Claude Code + heuristic fallback: `generateReflection()`, `generateHeuristicReflection()` |
| `service-impl.ts` | Full service implementation: `makeReflectionService()` factory |
| `layer.ts` | Effect Layers: `ReflectionServiceLive`, `ReflectionServiceTest`, `ReflectionServiceTestWithStorage` |
| `index.ts` | Clean public exports for all types, services, and utilities |

---

## Files Modified

### Core Integration

| File | Changes |
|------|---------|
| `src/agent/orchestrator/types.ts` | Added `reflectionService` and `reflexionConfig` to `OrchestratorConfig` interface. Added imports for `ReflectionType`, `FailureContextType`, `ReflexionConfigType`. |
| `src/agent/orchestrator/orchestrator.ts` | 1) Added `FailureContextType` import. 2) After subtask failure (line ~666), generates reflection via `config.reflectionService.generate()` and saves it. 3) Before subtask execution, fetches recent reflections for retries and passes them to subagent. |
| `src/agent/orchestrator/subagent-router.ts` | Added `reflections?: string` to `RunBestAvailableSubagentOptions`. Passes reflections through to Claude Code runner. |
| `src/agent/orchestrator/claude-code-subagent.ts` | 1) Added `reflections?: string` to `ClaudeCodeSubagentOptions`. 2) Modified `defaultBuildPrompt()` to accept and inject reflections under "Learning from Previous Failures" section. |

### Schema & Configuration

| File | Changes |
|------|---------|
| `src/tasks/schema.ts` | Added `ReflexionConfig` schema with fields: `enabled`, `maxReflectionsPerRetry`, `generationTimeoutMs`, `retentionDays`. Added to `ProjectConfig` struct. |
| `src/tasks/index.ts` | Exported `ReflexionConfig` and `type ReflexionConfigT` |
| `src/agent/do-one-task.ts` | Added default `reflexion` config to hardcoded `ProjectConfig` |
| `src/agent/overnight.ts` | Added default `reflexion` config to hardcoded `ProjectConfig` |

### Documentation

| File | Changes |
|------|---------|
| `AGENTS.md` | Added "Never Use Inline Imports" rule under Lessons Learned / Effect TypeScript Patterns. Documents that `import("./path").Type` patterns are forbidden and should be refactored. |

---

## Architecture

### Data Flow

```
Subtask Fails
    │
    ▼
orchestrator.ts: Generate FailureContext
    │
    ▼
reflectionService.generate(failureContext)
    │
    ├─► generator.ts: Try Claude Code (maxTurns=1, mode="plan")
    │       │
    │       ▼
    │   parseReflectionResponse() → Reflection object
    │
    └─► (on failure) generateHeuristicReflection() → Fallback Reflection
    │
    ▼
reflectionService.save(reflection)
    │
    ▼
storage.ts: Append to .openagents/memory/reflections.jsonl
```

### Retry Flow

```
Subtask Retry (failureCount > 0)
    │
    ▼
orchestrator.ts: Check if reflexion enabled
    │
    ▼
reflectionService.getRecent(subtaskId, limit=3)
    │
    ▼
reflectionService.formatForPrompt(reflections)
    │
    ▼
subagentRunner({ ...options, reflections: formattedText })
    │
    ▼
claude-code-subagent.ts: defaultBuildPrompt() injects reflections
    │
    ▼
Claude Code prompt includes:
  "## Learning from Previous Failures
   The following reflections were generated..."
```

### Effect Service Pattern

```typescript
// Interface (service contract)
export interface IReflectionService {
  generate(failure: FailureContext): Effect.Effect<Reflection, ReflectionError>;
  getRecent(subtaskId: string, limit?: number): Effect.Effect<Reflection[], ReflectionError>;
  save(reflection: Reflection): Effect.Effect<void, ReflectionError>;
  formatForPrompt(reflections: Reflection[]): Effect.Effect<string, ReflectionError>;
  prune(maxAgeMs: number): Effect.Effect<number, ReflectionError>;
}

// Context Tag (for DI)
export class ReflectionService extends Context.Tag("ReflectionService")<
  ReflectionService,
  IReflectionService
>() {}

// Layer (provides implementation)
export const ReflectionServiceLive = (options: ReflectionServiceOptions) =>
  Layer.succeed(ReflectionService, makeReflectionService(options));
```

---

## Configuration

### ReflexionConfig Schema

```typescript
export const ReflexionConfig = S.Struct({
  enabled: S.optionalWith(S.Boolean, { default: () => true }),
  maxReflectionsPerRetry: S.optionalWith(S.Number, { default: () => 3 }),
  generationTimeoutMs: S.optionalWith(S.Number, { default: () => 30000 }),
  retentionDays: S.optionalWith(S.Number, { default: () => 30 }),
});
```

### Example project.json

```json
{
  "reflexion": {
    "enabled": true,
    "maxReflectionsPerRetry": 3,
    "generationTimeoutMs": 30000,
    "retentionDays": 30
  }
}
```

---

## Reflection Schema

```typescript
export const Reflection = S.Struct({
  id: S.String,
  sessionId: S.String,
  taskId: S.String,
  subtaskId: S.String,
  attemptNumber: S.Number,
  category: ReflectionCategory,  // "root_cause" | "approach_flaw" | "missing_context" | "tool_misuse" | "test_gap" | "verification"
  analysis: S.String,
  suggestion: S.String,
  actionItems: S.Array(S.String),
  confidence: S.Number,  // 0.0 - 1.0
  createdAt: S.String,
});
```

---

## Storage Format

Reflections stored in `.openagents/memory/reflections.jsonl`:

```jsonl
{"id":"ref-abc123","sessionId":"sess-xyz","taskId":"oa-task1","subtaskId":"sub-1","attemptNumber":1,"category":"root_cause","analysis":"The error occurred because...","suggestion":"Instead of X, try Y...","actionItems":["Check input validation","Add error handling"],"confidence":0.85,"createdAt":"2024-12-04T18:30:00.000Z"}
```

---

## Tests

All related tests pass:

```bash
$ bun test ./src/agent/orchestrator/subagent-router.test.ts
# 17 pass, 0 fail

$ bun test ./src/agent/orchestrator/claude-code-subagent.test.ts
# 21 pass, 0 fail

$ bun test ./src/tasks/schema.test.ts
# 26 pass, 0 fail
```

Typecheck passes for all reflection module files. Pre-existing errors in `src/bun/` and `src/mainview/` are unrelated.

---

## Key Design Decisions

1. **LLM-first generation with heuristic fallback**: Uses Claude Code in `plan` mode for reflection generation, falls back to pattern-matched heuristics if LLM fails.

2. **JSONL storage**: Simple append-only format for reflections, easy to debug and process.

3. **Effect-first architecture**: Full Effect/Schema types, Context.Tag for DI, Layers for test/production switching.

4. **Enabled by default**: Reflexion is on by default (`enabled: true`) since it's low overhead and improves retry success.

5. **Max 3 reflections per retry**: Limits context window usage while providing recent failure insights.

6. **Integration after Healer**: Reflections are generated after subtask failure but before Healer runs, providing complementary self-improvement.

---

## Future Improvements

- [ ] Add unit tests for reflection module
- [ ] Add pruning job for old reflections
- [ ] Track reflection effectiveness metrics
- [ ] Consider cross-task reflection sharing for similar failures
- [ ] Add reflection categories to improve analysis targeting

---

## References

- [Reflexion Paper (NeurIPS 2023)](https://arxiv.org/abs/2303.11366)
- `docs/research/no-gradient-lifelong-learning.md` - Research analysis
- `docs/research/reflexion-summary.md` - Pattern summary
