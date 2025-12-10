# Port TestGen to Rust

Port the TestGen flow from TypeScript (`src/hillclimber/testgen-*.ts`) to a new `crates/testgen/` Rust crate with full LLM integration via `fm-bridge` and SQLite persistence.

## Crate Structure

```
crates/testgen/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Module exports
│   ├── types.rs            # TestGenConfig, TestGenRun, GeneratedTest, etc.
│   ├── error.rs            # TestGenError using thiserror
│   ├── analyzer.rs         # Analysis functions (category balance, anti-cheat, etc.)
│   ├── scoring.rs          # 0-1000 scoring system
│   ├── meta_reasoner.rs    # Config evolution with guardrails
│   ├── generator.rs        # Iterative test generation (5 categories)
│   ├── store.rs            # SQLite persistence (rusqlite)
│   ├── environment.rs      # EnvironmentInfo types
│   └── bin/
│       └── testgen.rs      # CLI tool
```

## Implementation Steps

### Step 1: Create crate scaffold
- [ ] Create `crates/testgen/Cargo.toml` with dependencies
- [ ] Add `"crates/testgen"` to workspace `Cargo.toml` members
- [ ] Create `src/lib.rs` with module declarations

### Step 2: Core types (`types.rs`)
- [ ] `TestCategory` enum (AntiCheat, Existence, Correctness, Boundary, Integration)
- [ ] `ModelType` enum (Local, Claude)
- [ ] `TestGenContext` enum (Benchmark, Commander, MechaCoder, Custom)
- [ ] `GeneratedTest` struct (id, input, expected_output, reasoning, category, confidence)
- [ ] `TestGenConfig` struct (all config knobs)
- [ ] `TestGenConfigInput` struct (optional fields for creation)
- [ ] `TestGenRun` struct (run record with metrics)
- [ ] `TestGenAnalysis` struct (category_distribution, balance, coverage, etc.)
- [ ] `TestGenConfigChange` struct (change proposal)
- [ ] `TestGenStats` struct (aggregates)

### Step 3: Error types (`error.rs`)
- [ ] `TestGenError` enum with variants: Database, Serialization, FmBridge, ConfigNotFound, GuardrailViolation, ParseError, etc.
- [ ] `Result<T>` type alias

### Step 4: Analysis functions (`analyzer.rs`)
- [ ] `analyze_category_distribution(tests) -> HashMap<Category, u32>`
- [ ] `calculate_category_balance(distribution) -> f64` (variance-based 0-1 score)
- [ ] `analyze_anti_cheat_coverage(tests, env, desc) -> f64`
- [ ] `analyze_parameter_discovery(tests, env) -> f64`
- [ ] `analyze_reflection_effectiveness(reflections, tests, rounds) -> f64`
- [ ] `analyze_token_efficiency(tokens, comprehensiveness) -> f64`
- [ ] `analyze_testgen_run(trajectory) -> TestGenAnalysis`

### Step 5: Scoring (`scoring.rs`)
- [ ] `score_testgen_run(analysis, comprehensiveness) -> i32`
  - Formula: `comp*40 + balance*200 + antiCheat*200 + efficiency*200`
- [ ] `compute_overall_score(analysis, comprehensiveness) -> TestGenAnalysis`

### Step 6: Meta-reasoner (`meta_reasoner.rs`)
- [ ] `Guardrails` struct (max_temperature_delta=0.1, max_tests_delta=1, etc.)
- [ ] `validate_config_change(current, change, guardrails) -> Result<()>`
- [ ] `apply_config_change(current, change) -> TestGenConfigInput`
- [ ] `increment_version(version) -> String`
- [ ] `propose_config_change(client, config, runs, analysis, task_type) -> Result<ConfigChange>` (uses fm-bridge)
- [ ] `build_meta_prompt()` and `parse_config_change()` helpers

### Step 7: SQLite store (`store.rs`)
- [ ] `TestGenStore` struct wrapping `rusqlite::Connection`
- [ ] `open(path)` and `open_in_memory()` constructors
- [ ] `ensure_schema()` - create tables from migration SQL
- [ ] Config ops: `save_config`, `get_config_by_id`, `get_config_by_hash`, `get_current_config`, `set_current_config`
- [ ] Run ops: `save_run`, `get_run_by_id`, `get_recent_runs`, `get_run_history`
- [ ] Best config ops: `get_best_config`, `update_best_config`
- [ ] Stats: `get_stats() -> TestGenStats`
- [ ] Helper: `hash_config(input) -> String` (SHA256)

### Step 8: Environment types (`environment.rs`)
- [ ] `EnvironmentInfo` struct (platform, tools, files, resources)
- [ ] `PlatformInfo` struct (type, container detection)
- [ ] `ToolsInfo` struct (available, prohibited)
- [ ] `FilesInfo` struct (listing, task_files with previews)

### Step 9: Generator (`generator.rs`)
- [ ] `IterationConfig` struct (defaults: minTests=2, maxTests=5, maxRounds=3, etc.)
- [ ] `GeneratorState` struct (tests per category, rounds, tokens)
- [ ] `TestGenerator` struct wrapping `FMClient`
- [ ] `get_categories_for_context(context) -> Vec<Category>`
- [ ] `build_category_prompt(task, category, existing, round) -> String`
- [ ] `parse_tests_response(content, category) -> Vec<GeneratedTest>`
- [ ] `generate_for_category(task, category, existing, round) -> Result<(tests, tokens)>`
- [ ] `reflect_on_category(category, tests) -> Result<(reflection, tokens)>`
- [ ] `assess_comprehensiveness(state, task, env) -> Result<(score, gaps, recommendations)>`
- [ ] `generate_iteratively(task, env, emitter) -> Result<TestResult>` - main entry point

### Step 10: CLI (`bin/testgen.rs`)
- [ ] Basic CLI with clap: `testgen generate --task <id>`, `testgen stats`, `testgen config show`

### Step 11: Integration & testing
- [ ] Unit tests for analyzer, scoring, meta_reasoner
- [ ] Integration tests for store (in-memory SQLite)
- [ ] End-to-end test with mock FM responses

## Dependencies (Cargo.toml)

```toml
[dependencies]
fm-bridge = { path = "../fm-bridge" }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
anyhow = "1"
rusqlite = { version = "0.31", features = ["bundled", "serde_json"] }
sha2 = "0.10"
hex = "0.4"
clap = { version = "4", features = ["derive"] }
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4"] }
tracing = "0.1"
regex = "1"
```

## Source Files to Port

| TypeScript Source | Rust Target |
|-------------------|-------------|
| `src/hillclimber/testgen-types.ts` | `types.rs` |
| `src/hillclimber/testgen-analyzer.ts` | `analyzer.rs` |
| `src/hillclimber/testgen-scoring.ts` | `scoring.rs` |
| `src/hillclimber/testgen-meta-reasoner.ts` | `meta_reasoner.rs` |
| `src/hillclimber/testgen-store.ts` | `store.rs` |
| `src/hillclimber/test-generator-iterative.ts` | `generator.rs` |
| `src/hillclimber/environment-info.ts` | `environment.rs` |

## SQLite Schema

Copy from `.openagents/migrations/005_testgen_evolution.sql`:
- `testgen_configs` - configuration parameters
- `testgen_runs` - run records with metrics
- `testgen_best_configs` - best config per task type
- `testgen_evolution` - config change history

## Key Algorithms

### Category Balance (0-1)
```rust
let ideal = total / num_categories;
let variance = counts.iter().map(|c| (c - ideal).powi(2)).sum() / num_categories;
let max_variance = ideal.powi(2) * (num_categories - 1);
balance = 1.0 - (variance / max_variance)
```

### Scoring (0-1000)
```rust
score = comprehensiveness * 40  // 0-400
      + category_balance * 200  // 0-200
      + anti_cheat_coverage * 200  // 0-200
      + token_efficiency * 200  // 0-200
```

### Guardrails
- Temperature: ±0.1 max
- Tests per category: ±1 max
- Rounds per category: ±1 max
- Weights: ±0.1 max
- Min tests per category: 2 (hard minimum)

## FM-Bridge Integration

Use existing `FMClient` from `crates/fm-bridge`:
```rust
let client = FMClient::new();
let response = client.complete(&prompt, Some(CompletionOptions {
    temperature: Some(0.3),
    max_tokens: Some(2048),
    ..Default::default()
})).await?;
```
