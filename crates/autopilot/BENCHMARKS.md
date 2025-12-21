# Autopilot Benchmark Suite

Standard benchmark tasks for measuring autopilot performance across versions.

## Goal

Provide reproducible benchmark tasks that:
1. Have known, deterministic solutions
2. Cover common development workflows
3. Can be run automatically on each code change
4. Detect performance regressions

## Benchmark Categories

### 1. File Operations
Tasks focused on reading, editing, and writing files.

#### B-001: Simple File Edit
- **Task**: "Change the version number in version.txt from 1.0.0 to 1.0.1"
- **Setup**: Create `version.txt` with "version = 1.0.0"
- **Expected**: File edited with exact string replacement
- **Metrics**: Time, tool calls, token usage

#### B-002: Multi-File Search and Edit
- **Task**: "Replace all occurrences of 'OLD_API' with 'NEW_API' across all .rs files"
- **Setup**: 10 Rust files with 5 occurrences each
- **Expected**: All 50 occurrences replaced
- **Metrics**: Time, tool calls, parallelization rate

#### B-003: Complex Refactoring
- **Task**: "Rename struct User to Account and update all references"
- **Setup**: 5 files with User struct and 20 references
- **Expected**: All references updated, code compiles
- **Metrics**: Time, tool calls, compilation success

### 2. Git Operations
Tasks involving version control workflows.

#### B-004: Simple Commit
- **Task**: "Commit the changes with message 'Update version'"
- **Setup**: 1 modified file in git repo
- **Expected**: 1 commit created with exact message
- **Metrics**: Time, git command count

#### B-005: Branch and PR Workflow
- **Task**: "Create branch 'feature-x', commit changes, push, create PR"
- **Setup**: Modified file in main branch
- **Expected**: Branch created, pushed, PR opened
- **Metrics**: Time, git operations, success rate

### 3. Testing
Tasks involving test execution and fixing.

#### B-006: Run Passing Tests
- **Task**: "Run the test suite and verify all tests pass"
- **Setup**: Crate with 10 passing tests
- **Expected**: Tests run, status reported
- **Metrics**: Time, correct status detection

#### B-007: Fix Failing Test
- **Task**: "Fix the failing test in math.rs"
- **Setup**: 1 test with known failure (off-by-one)
- **Expected**: Test fixed and passing
- **Metrics**: Time, tool calls, iterations needed

### 4. Code Generation
Tasks creating new code from scratch.

#### B-008: Implement Simple Function
- **Task**: "Write a function that checks if a number is prime"
- **Setup**: Empty file with TODO comment
- **Expected**: Working implementation with tests
- **Metrics**: Time, correctness, test coverage

#### B-009: Add CRUD Endpoint
- **Task**: "Add a REST endpoint for creating users"
- **Setup**: Existing Actix app structure
- **Expected**: Route, handler, tests added
- **Metrics**: Time, completeness, correctness

### 5. Debugging
Tasks requiring analysis and problem-solving.

#### B-010: Fix Compilation Error
- **Task**: "Fix the compilation error in auth.rs"
- **Setup**: Missing trait import (known error)
- **Expected**: Code compiles successfully
- **Metrics**: Time, attempts, correct fix

#### B-011: Debug Logic Error
- **Task**: "Fix the bug causing incorrect sorting"
- **Setup**: Sort function with edge case bug
- **Expected**: Bug identified and fixed
- **Metrics**: Time, diagnosis steps, fix quality

### 6. Documentation
Tasks involving documentation creation.

#### B-012: Add Function Documentation
- **Task**: "Add rustdoc comments to all public functions in lib.rs"
- **Setup**: 5 public functions without docs
- **Expected**: All functions documented
- **Metrics**: Time, completeness, quality

#### B-013: Generate README
- **Task**: "Create README.md for this crate"
- **Setup**: Cargo.toml with metadata
- **Expected**: README with usage, examples
- **Metrics**: Time, completeness

### 7. Issue Workflow
Tasks involving the autopilot issue system.

#### B-014: Create and Complete Issue
- **Task**: "Create issue to add error handling, then implement it"
- **Setup**: Clean issue database
- **Expected**: Issue created, claimed, completed
- **Metrics**: Time, workflow correctness

#### B-015: Multi-Issue Session
- **Task**: "Complete 3 sequential issues"
- **Setup**: 3 ready issues (simple tasks)
- **Expected**: All 3 completed in order
- **Metrics**: Time, efficiency, context retention

### 8. Error Handling
Tasks testing error recovery.

#### B-016: Retry on Tool Failure
- **Task**: "Edit config.toml even if first attempt fails"
- **Setup**: File temporarily locked (simulate error)
- **Expected**: Retry and succeed
- **Metrics**: Retry count, recovery time

#### B-017: Handle Missing File
- **Task**: "Create utils.rs if it doesn't exist, then add function"
- **Setup**: No utils.rs file
- **Expected**: File created with function
- **Metrics**: Time, error detection

### 9. Optimization
Tasks measuring efficiency.

#### B-018: Parallel Tool Execution
- **Task**: "Read 5 independent files and summarize each"
- **Setup**: 5 unrelated files
- **Expected**: All reads in parallel
- **Metrics**: Parallelization rate, time

#### B-019: Cache Utilization
- **Task**: "Repeat previous file edit task"
- **Setup**: Same task as B-001
- **Expected**: High cache hit rate
- **Metrics**: Cache percentage, token savings

### 10. Integration
Full workflow tasks.

#### B-020: Feature Implementation
- **Task**: "Add authentication middleware with tests and docs"
- **Setup**: Existing web app structure
- **Expected**: Middleware, tests, docs, integrated
- **Metrics**: Time, completeness, quality

## Benchmark Runner

Implementation: `crates/autopilot/src/benchmark/mod.rs`

### How It Works

The benchmark runner now executes **real autopilot agents** (not placeholder metrics). Here's how:

1. **Agent Execution**: Each benchmark uses the Claude Agent SDK to spawn an actual agent
2. **Trajectory Collection**: A `TrajectoryCollector` captures all tool calls, thinking, and responses
3. **Metrics Extraction**: Real metrics are extracted from the trajectory:
   - Token counts from Claude API responses
   - Tool call counts from trajectory steps
   - Tool errors from failed operations
   - Duration from wall-clock measurement
   - Cost calculated from token usage

This means benchmarks consume API tokens and take real time to execute.

### Running Benchmarks

```bash
# Run all benchmarks
cargo autopilot benchmark

# Run specific category
cargo autopilot benchmark --category file-ops

# Run single benchmark
cargo autopilot benchmark B-001

# Compare against baseline
cargo autopilot benchmark --baseline v0.1.0

# Save baseline
cargo autopilot benchmark --save-baseline v0.1.0
```

**Note**: Benchmarks consume Claude API tokens. A simple benchmark (B-001) typically uses:
- ~500-2000 input tokens
- ~200-800 output tokens
- ~$0.01-0.05 per run
- 10-60 seconds execution time

### Integration Tests

The benchmark system includes comprehensive integration tests in `tests/benchmark_execution.rs`:

```bash
# Run fast tests (database, schema validation)
cargo test --package autopilot --test benchmark_execution

# Run expensive tests (actual agent execution)
cargo test --package autopilot --test benchmark_execution --ignored

# Run specific expensive test
cargo test --package autopilot --test benchmark_execution test_simple_file_edit_benchmark_execution --ignored
```

**Test Categories**:
- **Fast tests** (default): Database creation, schema validation, result persistence
- **Expensive tests** (`#[ignore]`): Full agent execution with real API calls

The `#[ignore]` attribute prevents expensive tests from running in CI by default, but they can be run manually for verification.

## Metrics Collected

For each benchmark run (extracted from trajectory data):

1. **Performance**
   - Total time (wall clock measurement)
   - Token usage (input/output/cached from API responses)
   - Cost (calculated: $3/MTok input + $15/MTok output for Sonnet)

2. **Behavior**
   - Tool calls (counted from `StepType::ToolCall` in trajectory)
   - Tool errors (counted from `StepType::ToolResult { success: false }`)
   - Actual tools used (extracted from trajectory steps)

3. **Outcome**
   - Success (validation function passes)
   - Correctness (solution matches expected output)
   - Completeness (all requirements met)

4. **Efficiency**
   - Time per task
   - Tokens per task
   - Tool error rate (tool_errors / tool_calls)

### Performance Characteristics by Benchmark Type

| Benchmark Type | Typical Tokens | Typical Duration | Typical Cost | Tool Calls |
|----------------|----------------|------------------|--------------|------------|
| Simple Edit (B-001) | 500-2000 | 10-60s | $0.01-0.05 | 2-5 |
| Multi-File (B-002) | 2000-5000 | 30-120s | $0.05-0.15 | 10-30 |
| Refactor (B-003) | 3000-8000 | 60-180s | $0.10-0.30 | 15-40 |
| Git Operations | 1000-3000 | 20-90s | $0.03-0.10 | 5-15 |
| Testing | 2000-6000 | 40-150s | $0.06-0.20 | 10-25 |

**Note**: These are estimates based on Sonnet 4.5 usage. Actual values depend on:
- Workspace complexity
- Git repository state
- Cached token availability
- Agent decision-making path

## Storage

Benchmark results stored in SQLite database: `autopilot-benchmarks.db`

```sql
CREATE TABLE benchmark_runs (
    id INTEGER PRIMARY KEY,
    benchmark_id TEXT NOT NULL,
    version TEXT NOT NULL,
    timestamp DATETIME NOT NULL,
    success BOOLEAN NOT NULL,
    duration_ms INTEGER NOT NULL,
    tokens_in INTEGER NOT NULL,
    tokens_out INTEGER NOT NULL,
    tokens_cached INTEGER NOT NULL,
    cost_usd REAL NOT NULL,
    tool_calls INTEGER NOT NULL,
    tool_errors INTEGER NOT NULL
);

CREATE TABLE benchmark_details (
    run_id INTEGER REFERENCES benchmark_runs(id),
    metric_name TEXT NOT NULL,
    metric_value REAL NOT NULL,
    PRIMARY KEY (run_id, metric_name)
);
```

## Regression Detection

After each benchmark run:

1. Compare against baseline for this benchmark
2. Flag if performance degrades by >10%
3. Flag if success rate drops below 100%
4. Flag if correctness issues detected

Report regressions as issues for investigation.

## Baseline Management

Baselines stored per semantic version:

- `v0.1.0` - Initial baseline
- `v0.2.0` - After major refactor
- `main` - Rolling baseline (updated weekly)

```bash
# Save current run as baseline
cargo autopilot benchmark --save-baseline v0.1.0

# Compare against specific baseline
cargo autopilot benchmark --baseline v0.1.0

# List all baselines
cargo autopilot benchmark --list-baselines
```

## CI Integration

Run benchmarks on:

1. Every PR (compare against main)
2. Every merge to main (update rolling baseline)
3. Every release (save versioned baseline)

Gate merges if:
- Any benchmark fails
- Performance degrades >20%
- New regressions introduced

## Future Enhancements

1. **Adversarial Benchmarks**: Tasks designed to trigger common failure modes
2. **Scalability Benchmarks**: Large codebases, many files
3. **Real-World Benchmarks**: Actual historical issues as benchmarks
4. **Model Comparison**: Run same benchmarks on different models
5. **Interactive Benchmarks**: Tasks requiring user input simulation

## Implementation Phases

### Phase 1: Foundation (This Issue)
- [ ] Create BENCHMARKS.md (this file)
- [ ] Define first 5 benchmarks (B-001 to B-005)
- [ ] Document metrics and storage schema
- [ ] Plan benchmark runner architecture

### Phase 2: Runner Implementation
- [ ] Create `crates/autopilot/src/benchmark.rs`
- [ ] Implement benchmark setup/teardown
- [ ] Implement metrics collection
- [ ] Add benchmark database

### Phase 3: Benchmark Tasks
- [ ] Implement B-001 through B-005
- [ ] Validate against manual runs
- [ ] Tune expected results

### Phase 4: Analysis & Reporting
- [ ] Implement baseline comparison
- [ ] Generate regression reports
- [ ] CLI output formatting

### Phase 5: CI Integration
- [ ] Add GitHub Actions workflow
- [ ] Automated regression detection
- [ ] Baseline management automation

## Related

- `docs/autopilot/IMPROVEMENT-DIMENSIONS.md` - Metrics framework
- `crates/autopilot/src/metrics.rs` - Metrics collection
- `crates/autopilot/src/analyze.rs` - Analysis pipeline
- Directive d-004 Phase 7 - Benchmark Suite
