# TerminalBench Overnight Iteration System

## Overview

An overnight benchmark iteration system that runs TerminalBench repeatedly, collects performance metrics, and improves daily through learning. Supports both Claude Code and local Ollama models.

**Design Decisions:**
- **MVP first**: Sequential runner for Phase 1, parallelism in Phase 2
- **Any Ollama model via config**: No hardcoded defaults, user specifies model
- **Minimal learning for v1**: Store Episodes, basic prompt injection. Full Trainer/Archivist in later phases

**Aligns with existing specs:**
- `docs/subagents/gym-trainer.md` - Trainer & Gym concept (Phase 3+)
- `docs/subagents/archivist.md` - Memory Bank & reflection (Phase 3+)

---

## Phase 1: MVP (Sequential Runner)

Simple sequential overnight runner that can run tonight.

### Architecture

```
                    +---------------------+
                    | tbench-iterate CLI  |
                    +----------+----------+
                               |
                    +----------v----------+
                    |   Model Adapter     |
                    | (Claude Code/Ollama)|
                    +----------+----------+
                               |
               +---------------+---------------+
               |                               |
        +------v------+                 +------v------+
        | Iteration 1 |                 | Iteration N |
        | (sequential)|    ...          | (sequential)|
        +------+------+                 +------+------+
               |                               |
               +---------------+---------------+
                               |
                    +----------v----------+
                    |   Episode Store     |
                    | (.openagents/gym/)  |
                    +---------------------+
```

### New Files

| File | Purpose |
|------|---------|
| `src/cli/tbench-iterate.ts` | Main CLI for overnight iteration runs |
| `src/bench/model-adapter.ts` | Abstraction layer for Claude Code vs Ollama |
| `src/llm/ollama.ts` | Ollama HTTP client |
| `src/bench/episode-store.ts` | Store/query Episodes in `.openagents/gym/` |

### CLI Interface

```bash
# Basic overnight run with Claude Code
bun src/cli/tbench-iterate.ts \
  --suite ./tasks/tb-2.0.json \
  --output ./results/$(date +%Y%m%d) \
  --iterations 10

# With Ollama (any model)
bun src/cli/tbench-iterate.ts \
  --suite ./tasks/tb-2.0.json \
  --output ./results/$(date +%Y%m%d) \
  --model ollama:codellama:34b \
  --iterations 20

# With different Ollama endpoint
bun src/cli/tbench-iterate.ts \
  --suite ./tasks/tb-2.0.json \
  --model ollama:deepseek-coder:33b \
  --ollama-endpoint http://gpu-server:11434 \
  --iterations 20

# Mixed: Ollama for most, Claude for validation
bun src/cli/tbench-iterate.ts \
  --suite ./tasks/tb-2.0.json \
  --model ollama:qwen2.5-coder:32b \
  --claude-validation-rate 0.1 \
  --iterations 20

# Resume interrupted run
bun src/cli/tbench-iterate.ts --resume ./results/20251205/state.json
```

### Model Adapter (`src/bench/model-adapter.ts`)

```typescript
export interface ModelConfig {
  type: "claude-code" | "ollama";
  model?: string;              // For ollama: "codellama:34b", etc.
  endpoint?: string;           // For ollama: "http://localhost:11434"
}

export interface ModelRunner {
  runTask(task: TerminalBenchTask, workspace: string): Promise<TaskResult>;
}

export function createModelRunner(config: ModelConfig): ModelRunner;
```

### Ollama Client (`src/llm/ollama.ts`)

```typescript
export interface OllamaConfig {
  endpoint: string;  // Default: "http://localhost:11434"
  model: string;     // e.g., "codellama:34b"
}

export interface OllamaMessage {
  role: "system" | "user" | "assistant";
  content: string;
}

export async function ollamaChat(
  config: OllamaConfig,
  messages: OllamaMessage[],
  options?: { tools?: ToolDefinition[] }
): Promise<string>;
```

### Episode Store (`src/bench/episode-store.ts`)

Minimal Episode tracking for v1 (foundation for Archivist):

```typescript
export interface Episode {
  id: string;
  runId: string;
  iteration: number;
  model: string;
  startedAt: string;
  finishedAt: string;
  status: "success" | "partial" | "failed";
  summary: {
    total: number;
    passed: number;
    failed: number;
    passRate: number;
  };
  resultsPath: string;
}

export class EpisodeStore {
  constructor(gymDir: string);
  record(episode: Episode): Promise<void>;
  query(filter: { runId?: string; model?: string }): Promise<Episode[]>;
  getBaseline(): Promise<Episode | null>;
}
```

### Storage Layout

```
.openagents/
  gym/
    episodes.jsonl         # Episode records (minimal Archivist)

results/
  20251205/
    config.json            # Run configuration
    state.json             # For resume capability
    iterations/
      001/
        results.json       # TerminalBenchResults
        delta.json         # Comparison with previous
      002/
        ...
    summary.json           # Aggregate statistics
    report.md              # Human-readable report
```

### Iteration Loop

```typescript
async function runOvernightIteration(config: OvernightConfig) {
  const store = new EpisodeStore(path.join(config.projectRoot, ".openagents/gym"));
  const runner = createModelRunner(config.model);

  for (let i = 1; i <= config.iterations; i++) {
    console.log(`\n=== Iteration ${i}/${config.iterations} ===`);

    // Run benchmark suite (reuse tbench-local logic)
    const result = await runBenchmarkSuite(config.suite, runner);

    // Store results
    await writeIterationResults(config.output, i, result);

    // Record episode
    await store.record({
      id: `${config.runId}-${i}`,
      runId: config.runId,
      iteration: i,
      model: config.model.model || "claude-code",
      startedAt: result.startedAt,
      finishedAt: result.finishedAt,
      status: result.summary.passRate > 0.5 ? "success" : "failed",
      summary: result.summary,
      resultsPath: `${config.output}/iterations/${String(i).padStart(3, "0")}/results.json`,
    });

    // Compare with baseline
    if (i > 1) {
      const delta = compareWithPrevious(previousResult, result);
      await writeDelta(config.output, i, delta);
    }

    previousResult = result;
  }

  // Generate summary
  await writeSummary(config.output, episodes);
  await writeReport(config.output, episodes);
}
```

---

## Phase 2: Parallelism & Recovery

Add parallel execution using git worktrees.

### New Files

| File | Purpose |
|------|---------|
| `src/cli/tbench-overnight.ts` | Full parallel overnight runner |
| `src/bench/overnight-worker.ts` | Worker process for parallel runs |
| `src/bench/overnight-state.ts` | State persistence for crash recovery |
| `src/bench/overnight-recovery.ts` | Crash recovery logic |

### Key Features
- Git worktree isolation (reuse `src/agent/orchestrator/worktree.ts`)
- State persistence (reuse `src/agent/parallel/state.ts` patterns)
- Heartbeat system for detecting stuck workers
- Auto-scale workers based on available RAM

---

## Phase 3: Archivist Integration

Implement full Archivist per `docs/subagents/archivist.md`.

### New Files

| File | Purpose |
|------|---------|
| `src/archivist/schema.ts` | Episode, AgentMemory types |
| `src/archivist/service.ts` | ArchivistService implementation |
| `src/archivist/memory-service.ts` | MemoryService for lessons |
| `src/archivist/context.ts` | Build ReflectionContext from runs |

### Key Features
- Full AgentMemory with importance scoring
- Retrieval model (tag + scope + keyword search)
- Prompt injection of relevant lessons
- Reflection triggers after significant events

---

## Phase 4: Trainer/Gym Integration

Implement full Trainer per `docs/subagents/gym-trainer.md`.

### New Files

| File | Purpose |
|------|---------|
| `src/trainer/schema.ts` | AgentProfile, GymEpisode, TrainingPlan |
| `src/trainer/service.ts` | TrainerService implementation |
| `src/trainer/gym.ts` | GymEnvironment registry |
| `src/trainer/analyzer.ts` | Failure analysis and pattern detection |
| `src/trainer/evolution.ts` | Propose improved AgentProfiles |

### Key Features
- GymEnvironment abstraction (Terminal-Bench, MechaBench, etc.)
- AgentProfile variants for A/B testing
- TrainingPlan for structured evaluation
- Evolution strategies for prompt/config improvements

---

## Documentation Structure

Create `docs/tbench/` with:

| File | Content |
|------|---------|
| `README.md` | Overview, quick start, architecture |
| `overnight-runs.md` | How to run overnight benchmarks |
| `model-configuration.md` | Claude Code vs Ollama setup guide |
| `episode-store.md` | Episode schema and querying |
| `learning-roadmap.md` | Roadmap for Archivist/Trainer integration |

### `docs/tbench/README.md` outline

```markdown
# TerminalBench Overnight System

## Quick Start
bun src/cli/tbench-iterate.ts --suite ./tasks/tb-2.0.json --iterations 10

## Architecture
- Model Adapter: Unified interface for Claude Code and Ollama
- Episode Store: Track runs in .openagents/gym/episodes.jsonl
- Result Aggregation: Summary and comparison reports

## Models
- Claude Code: Full capabilities, premium
- Ollama: Local models, cost-effective

## Future: Learning Loop
- Phase 3: Archivist for memory and lessons
- Phase 4: Trainer for systematic improvement
```

---

## Tasks for `.openagents/tasks.jsonl`

### Phase 1 Tasks (MVP)

```
oa-tbench-01: Create src/llm/ollama.ts - Ollama HTTP client
oa-tbench-02: Create src/bench/model-adapter.ts - Model abstraction layer
oa-tbench-03: Create src/bench/episode-store.ts - Episode storage
oa-tbench-04: Create src/cli/tbench-iterate.ts - Main overnight CLI
oa-tbench-05: Create docs/tbench/README.md - Documentation
oa-tbench-06: Create docs/tbench/overnight-runs.md - Usage guide
oa-tbench-07: Create docs/tbench/model-configuration.md - Model setup
```

### Phase 2 Tasks (Parallelism)

```
oa-tbench-10: Create src/cli/tbench-overnight.ts - Parallel runner
oa-tbench-11: Create src/bench/overnight-worker.ts - Worker process
oa-tbench-12: Create src/bench/overnight-state.ts - State persistence
oa-tbench-13: Create src/bench/overnight-recovery.ts - Crash recovery
oa-tbench-14: Update docs/tbench/overnight-runs.md - Parallel docs
```

### Phase 3 Tasks (Archivist)

```
oa-tbench-20: Create src/archivist/schema.ts - Memory types
oa-tbench-21: Create src/archivist/service.ts - Archivist service
oa-tbench-22: Create src/archivist/memory-service.ts - Memory storage
oa-tbench-23: Wire Archivist into tbench-iterate end-of-run
oa-tbench-24: Add memory retrieval to model prompts
oa-tbench-25: Create docs/tbench/learning-loop.md - Learning docs
```

### Phase 4 Tasks (Trainer)

```
oa-tbench-30: Create src/trainer/schema.ts - Trainer types
oa-tbench-31: Create src/trainer/service.ts - Trainer service
oa-tbench-32: Create src/trainer/gym.ts - Gym environments
oa-tbench-33: Create src/trainer/analyzer.ts - Failure analysis
oa-tbench-34: Create src/trainer/evolution.ts - AgentProfile evolution
oa-tbench-35: Add trainer:run CLI command
```

---

## Critical Files to Read Before Implementation

1. `src/cli/tbench-local.ts` - Current TB runner to extract from
2. `src/agent/orchestrator/claude-code-subagent.ts` - Claude Code interface
3. `src/bench/terminal-bench.ts` - Task and result schemas
4. `src/bench/metrics.ts` - Metrics collection patterns
5. `src/agent/overnight-parallel.ts` - Parallel execution patterns (Phase 2)
6. `docs/subagents/archivist.md` - Full Archivist spec (Phase 3)
7. `docs/subagents/gym-trainer.md` - Full Trainer spec (Phase 4)

---

## Success Metrics

### Phase 1 (MVP)
- Can run 10-20 iterations overnight sequentially
- Supports both Claude Code and any Ollama model
- Stores Episodes for future learning

### Phase 2 (Parallelism)
- 100+ iterations per night with 4 workers
- Crash recovery without losing progress

### Phase 3+ (Learning)
- Pass rate improves week-over-week
- Lessons automatically injected into prompts
