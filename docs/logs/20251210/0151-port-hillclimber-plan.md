# HillClimber: Effect → Rust Conversion Plan

## Overview

Convert `src/hillclimber/` (43 TypeScript/Effect files) to `crates/hillclimber/` following patterns from `crates/testgen`.

**User decisions:**
- MAP mode only (no legacy executor)
- Direct testgen crate dependency
- Local FM only (no OpenRouter)
- bollard crate for Docker verification

---

## Module Structure

```
crates/hillclimber/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Public exports
│   ├── types.rs            # Domain types (configs, runs, subtasks, actions, state)
│   ├── error.rs            # HillClimberError with thiserror
│   ├── store.rs            # SQLite persistence
│   ├── scoring.rs          # PASS_BONUS=1000, TURN_BASE=100 formula
│   ├── decomposer.rs       # Generic task decomposition (no hardcoding)
│   ├── monitor.rs          # Action validation before execution
│   ├── evaluator.rs        # Docker/local pytest + output parsing
│   ├── orchestrator.rs     # MAP main loop
│   ├── sampler.rs          # Parallel sampling (test-time compute)
│   ├── prompt.rs           # FM prompt building
│   └── bin/
│       └── hillclimber.rs  # CLI
```

---

## Implementation Phases

### Phase 1: Foundation
**Files:** `Cargo.toml`, `lib.rs`, `types.rs`, `error.rs`, `store.rs`

- Define all domain types (HillClimberConfig, HillClimberRun, BestConfig, Subtask, ExecutionState, FMAction, EvaluatorResult, etc.)
- Error handling with thiserror (`HillClimberError`)
- SQLite persistence following testgen patterns
- Config deduplication via SHA256 hash

**Dependencies:**
```toml
fm-bridge = { path = "../fm-bridge" }
testgen = { path = "../testgen" }
tokio = { version = "1", features = ["full"] }
rusqlite = { version = "0.31", features = ["bundled"] }
bollard = "0.16"
thiserror = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
clap = { version = "4", features = ["derive"] }
```

### Phase 2: Scoring & Decomposition
**Files:** `scoring.rs`, `decomposer.rs`

- `score_result(passed, turns) -> i32` with PASS_BONUS + turn efficiency
- `is_better_score(new, old) -> bool`
- Generic 4-subtask decomposition pattern:
  1. understand-requirements
  2. write-initial-solution
  3. test-and-iterate
  4. final-validation
- Extract output filenames from task description only (no hardcoding)

### Phase 3: Monitor & Evaluator
**Files:** `monitor.rs`, `evaluator.rs`

**Monitor rules:**
- Workspace bounds check
- Dangerous command detection
- Repetition detection
- Returns `MonitorDecision { allowed, reason, warning, suggestion }`

**Evaluator:**
- `DockerVerificationRunner` using bollard
- `LocalVerificationRunner` fallback (pytest subprocess)
- Pytest output parsing → `EvaluatorResult { passed, progress, tests_passing, tests_total, failures, suggestion }`

### Phase 4: MAP Orchestrator Core
**Files:** `prompt.rs`, `orchestrator.rs`

**Main loop:**
```
1. Call testgen to generate comprehensive tests
2. Decompose task → subtasks
3. Initialize ExecutionState
4. Loop (max_turns):
   a. Build FM context (task, subtask, feedback, file contents, hints)
   b. Get action from FM via fm-bridge
   c. Monitor validates action
   d. Execute action (write_file, read_file, run_command, verify_progress)
   e. Track modified files
   f. Evaluate if file modified
   g. Decide: continue | advance | complete | no_progress
5. Return MAPOrchestratorResult
```

**Critical:** Pass modified file contents between subtasks to prevent context loss.

**Emitter trait:**
```rust
pub trait HillClimberEmitter: Send + Sync {
    fn on_turn_start(&self, turn: u32, max_turns: u32, subtask_name: &str);
    fn on_verify_complete(&self, passing: u32, total: u32, progress: f64);
    fn on_heartbeat(&self, turn: u32, max_turns: u32, progress: f64, best: f64, elapsed_ms: u64);
    fn on_run_complete(&self, passed: bool, progress: f64);
    fn on_error(&self, error: &str);
}
```

### Phase 5: Parallel Sampling
**Files:** `sampler.rs`

- Generate N=3 candidates with temps [0.3, 0.5, 0.7]
- Create temp workspaces
- Verify all in parallel via Docker
- Select best by test progress
- Apply to main workspace
- Cleanup

### Phase 6: TestGen Integration

At orchestrator start:
```rust
let generator = TestGenerator::new(fm_client.clone());
let result = generator.generate_iteratively(&task.description, &task.id, &env, TestGenContext::Benchmark, &NoopEmitter).await?;
write_tests_to_workspace(&result.tests, workspace)?;
```

### Phase 7: CLI & Runner
**Files:** `bin/hillclimber.rs`

```rust
#[derive(Parser)]
struct Cli {
    #[arg(short, long, value_delimiter = ',')]
    tasks: Vec<String>,
    #[arg(short, long, default_value = "100")]
    max_runs: u32,
    #[arg(long, default_value = "5000")]
    sleep_ms: u64,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    show_stats: bool,
    #[arg(short, long)]
    verbose: bool,
}
```

Runner: round-robin tasks, load config, run MAP, save run, update best, sleep.

---

## Key Types Summary

| Type | Purpose |
|------|---------|
| `HillClimberConfig` | Config knobs: hint, use_skills, max_turns_override |
| `HillClimberRun` | Run record: passed, turns, score, duration |
| `BestConfig` | Best config per task with pass count |
| `Subtask` | Decomposed subtask with checkpoint |
| `ExecutionState` | Turn tracking, modified files, last evaluation |
| `FMAction` | Tool call from FM: tool_name, tool_args |
| `MonitorDecision` | allow/deny with reason/warning |
| `EvaluatorResult` | Test results with failures and suggestions |
| `MAPOrchestratorResult` | Final result with progress and subtask status |

---

## Critical Files to Reference

| Source (TypeScript) | Target (Rust) | Notes |
|---------------------|---------------|-------|
| `src/hillclimber/types.ts` | `types.rs` | Port all types |
| `src/hillclimber/store.ts` | `store.rs` | Follow testgen/store.rs pattern |
| `src/hillclimber/map-orchestrator.ts` | `orchestrator.rs` | Main loop + FM context |
| `src/hillclimber/monitor.ts` | `monitor.rs` | Action validation |
| `src/hillclimber/evaluator.ts` | `evaluator.rs` | Pytest parsing |
| `src/hillclimber/decomposer.ts` | `decomposer.rs` | Generic decomposition |
| `src/hillclimber/scoring.ts` | `scoring.rs` | Score formula |
| `crates/testgen/src/store.rs` | - | Pattern reference |
| `crates/testgen/src/generator.rs` | - | Async + emitter pattern |
| `crates/fm-bridge/src/client.rs` | - | FM integration |

---

## Estimated Scope

- ~10 new Rust files
- ~2500-3500 lines of Rust
- Phases 1-3: Foundation (~40%)
- Phase 4: Core orchestrator (~35%)
- Phases 5-7: Sampling, integration, CLI (~25%)
