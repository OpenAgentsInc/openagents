# HillClimber & TestGen Comprehensive Test Plan

## Overview

Full test suite for HillClimber (MAP architecture) and TestGen systems - unit tests, integration tests, E2E tests, and Three Curves validation.

**Goal:** Ensure reliability of the entire flow from test generation through hill climbing optimization to TB2 benchmark validation.

## Architecture Reference

### HillClimber (MAP - Modular Agentic Planner)
```
Orchestrator
├── Task Decomposer → 4 subtasks (understand, write, test-iterate, validate)
├── Actor (FM) → Code generation via fm-bridge
├── Monitor → Action validation
├── Evaluator → Docker pytest verification
└── Parallel Sampler (TTC) → N=3 candidates at different temperatures
```

### TestGen
```
Generator
├── Iterative generation with reflection
├── 5 categories: AntiCheat, Existence, Correctness, Boundary, Integration
├── Analyzer → Quality metrics
├── Meta-Reasoner → Config evolution
└── Formatter → pytest output
```

---

## Part 1: Unit Tests

### 1.1 HillClimber Crate (`crates/hillclimber/src/`)

#### `orchestrator.rs` - Core MAP Loop
```rust
#[cfg(test)]
mod tests {
    // Trait implementations
    test_fm_client_trait_mock_implementation
    test_tool_executor_trait_mock_implementation
    test_emitter_trait_captures_events

    // Orchestration flow
    test_orchestrator_initializes_with_config
    test_orchestrator_runs_single_turn
    test_orchestrator_respects_max_turns
    test_orchestrator_stops_on_all_tests_pass
    test_orchestrator_handles_fm_error
    test_orchestrator_handles_tool_error
    test_orchestrator_emits_turn_start_event
    test_orchestrator_emits_turn_end_event
    test_orchestrator_tracks_best_score

    // State management
    test_turn_state_updates_correctly
    test_file_cache_persists_across_turns
    test_workspace_state_isolation
}
```

#### `runner.rs` - Entry Point & FMBridgeAdapter
```rust
test_runner_creates_orchestrator
test_runner_loads_config_from_store
test_runner_saves_results_to_store
test_fm_bridge_adapter_formats_request
test_fm_bridge_adapter_parses_response
test_fm_bridge_adapter_handles_timeout
test_fm_bridge_adapter_retries_on_error
```

#### `evaluator.rs` - Pytest Parsing & Docker Verification
```rust
// Pytest output parsing
test_parse_pytest_output_all_pass
test_parse_pytest_output_some_fail
test_parse_pytest_output_all_fail
test_parse_pytest_output_with_errors
test_parse_pytest_output_malformed
test_parse_pytest_output_empty
test_parse_pytest_captures_test_names
test_parse_pytest_captures_failure_reasons

// Docker verification
test_docker_container_starts
test_docker_container_timeout_handled
test_docker_mounts_workspace_correctly
test_docker_captures_stdout_stderr
test_docker_returns_exit_code
test_docker_cleanup_on_success
test_docker_cleanup_on_failure
```

#### `scoring.rs` - Score Calculation
```rust
test_score_all_pass_gives_max_bonus
test_score_no_pass_gives_zero_bonus
test_score_partial_pass_proportional
test_score_turn_efficiency_bonus
test_score_earlier_turn_better
test_score_formula_is_deterministic
test_score_overflow_protection
```

#### `store.rs` - SQLite Persistence
```rust
test_store_creates_tables_on_init
test_store_saves_config
test_store_loads_config
test_store_saves_run
test_store_loads_run
test_store_updates_best_config
test_store_queries_runs_by_task
test_store_handles_concurrent_access
test_store_migrations_are_idempotent
```

#### `decomposer.rs` - Task Decomposition
```rust
test_decompose_creates_4_subtasks
test_decompose_subtask_order_correct
test_decompose_includes_understand_phase
test_decompose_includes_write_phase
test_decompose_includes_test_iterate_phase
test_decompose_includes_validate_phase
test_decompose_task_description_preserved
test_decompose_no_hardcoded_task_ids  // CRITICAL: Anti-cheat
test_decompose_no_solution_hints      // CRITICAL: Anti-cheat
```

#### `prompt.rs` - FM Context Building
```rust
test_build_context_includes_task
test_build_context_includes_history
test_build_context_includes_test_results
test_build_context_respects_token_limit
test_build_context_truncates_old_history
test_parse_response_extracts_action
test_parse_response_extracts_reasoning
test_parse_response_handles_malformed
test_parse_response_handles_empty
```

#### `monitor.rs` - Action Validation
```rust
test_monitor_allows_workspace_file_write
test_monitor_blocks_outside_workspace
test_monitor_blocks_dangerous_commands
test_monitor_allows_safe_bash_commands
test_monitor_blocks_rm_rf_root
test_monitor_blocks_network_access
test_monitor_logs_blocked_actions
```

#### `sampler.rs` - Parallel Sampling (TTC)
```rust
test_sampler_generates_n_candidates
test_sampler_uses_different_temperatures
test_sampler_picks_best_by_score
test_sampler_handles_all_failures
test_sampler_handles_partial_failures
test_sampler_respects_timeout
test_sampler_runs_in_parallel
```

#### `testgen_writer.rs` - Pytest Generation
```rust
test_write_generates_valid_pytest
test_write_includes_all_categories
test_write_escapes_special_characters
test_write_formats_test_names
test_write_includes_docstrings
```

---

### 1.2 TestGen Crate (`crates/testgen/src/`)

#### `types.rs` - Core Types
```rust
test_test_category_all_variants
test_test_category_labels
test_test_category_descriptions
test_test_category_icons
test_generated_test_creation
test_testgen_config_defaults
test_testgen_run_creation
test_testgen_analysis_creation
```

#### `generator.rs` - Iterative Generation
```rust
// Generation flow
test_generator_creates_tests_from_description
test_generator_iterates_until_target
test_generator_respects_max_iterations
test_generator_reflects_on_gaps
test_generator_improves_comprehensiveness

// Category coverage
test_generator_creates_anti_cheat_tests
test_generator_creates_existence_tests
test_generator_creates_correctness_tests
test_generator_creates_boundary_tests
test_generator_creates_integration_tests

// Quality
test_generator_no_duplicate_tests
test_generator_tests_are_executable
test_generator_tests_have_assertions
```

#### `analyzer.rs` - Quality Metrics
```rust
test_analyzer_calculates_balance
test_analyzer_calculates_anti_cheat_coverage
test_analyzer_calculates_efficiency
test_analyzer_identifies_gaps
test_analyzer_suggests_improvements
test_analyzer_handles_empty_suite
```

#### `meta_reasoner.rs` - Config Evolution
```rust
test_meta_reasoner_evolves_config
test_meta_reasoner_applies_guardrails
test_meta_reasoner_prevents_regression
test_meta_reasoner_explores_new_configs
test_meta_reasoner_exploits_good_configs
test_meta_reasoner_respects_boundaries
```

#### `scoring.rs` - Test Suite Scoring
```rust
test_scoring_range_0_to_1000
test_scoring_comprehensiveness_weight
test_scoring_balance_weight
test_scoring_anti_cheat_weight
test_scoring_penalizes_gaps
test_scoring_rewards_coverage
```

#### `store.rs` - Persistence
```rust
test_store_creates_tables
test_store_saves_config
test_store_loads_config
test_store_saves_run_with_tests
test_store_loads_run_with_tests
test_store_queries_by_task
```

#### `formatter.rs` - Pytest Output
```rust
test_formatter_generates_pytest_file
test_formatter_groups_by_category
test_formatter_includes_imports
test_formatter_escapes_strings
test_formatter_valid_python_syntax
```

---

## Part 2: Integration Tests

### 2.1 HillClimber Integration (`crates/hillclimber/tests/`)

#### `integration_orchestrator.rs`
```rust
// Full orchestration with mock FM
test_orchestrator_full_run_mock_fm
test_orchestrator_recovers_from_errors
test_orchestrator_handles_flaky_tests
test_orchestrator_improves_over_turns

// Store integration
test_orchestrator_persists_progress
test_orchestrator_resumes_from_checkpoint
test_orchestrator_saves_best_solution

// Evaluator integration
test_orchestrator_evaluator_feedback_loop
test_orchestrator_uses_test_results_in_next_turn
```

#### `integration_sampler.rs`
```rust
// Parallel sampling with evaluation
test_sampler_evaluator_integration
test_sampler_picks_best_evaluated_candidate
test_sampler_handles_evaluation_failures
```

#### `integration_testgen.rs`
```rust
// TestGen → HillClimber flow
test_testgen_tests_used_by_hillclimber
test_testgen_writer_output_parseable
test_testgen_categories_all_evaluated
```

### 2.2 TestGen Integration (`crates/testgen/tests/`)

#### `integration_generation.rs`
```rust
// Full generation flow
test_generator_full_iteration_cycle
test_generator_analyzer_feedback_loop
test_generator_meta_reasoner_evolution

// Storage integration
test_generator_persists_tests
test_generator_loads_previous_tests
test_generator_compares_to_baseline
```

#### `integration_formatter.rs`
```rust
// Formatter → Evaluator
test_formatted_tests_are_executable
test_formatted_tests_pass_syntax_check
test_formatted_tests_run_in_docker
```

---

## Part 3: E2E Tests

### 3.1 Full Flow E2E (`crates/hillclimber/tests/e2e/`)

#### `e2e_complete_flow.rs`
```rust
#[test]
#[ignore] // Requires Docker + FM
fn test_e2e_complete_hillclimber_flow() {
    // 1. Load task from tasks crate
    // 2. Run TestGen to generate tests
    // 3. Run HillClimber with generated tests
    // 4. Verify solution passes TB2 evaluation
    // 5. Verify store contains run history
}

#[test]
#[ignore]
fn test_e2e_multi_turn_improvement() {
    // Verify scores improve over multiple turns
}

#[test]
#[ignore]
fn test_e2e_parallel_sampling_selects_best() {
    // Verify TTC sampling picks highest scorer
}
```

### 3.2 Three Curves Validation (`crates/hillclimber/tests/three_curves/`)

The Three Curves are our validation framework. Each curve must slope upward.

#### `curve_testgen_evolution.rs`
```rust
// Curve 1: TestGen Score vs Evolution Step
// Does meta-learning work?
#[test]
fn test_curve1_testgen_score_increases_with_evolution() {
    let scores: Vec<f64> = run_testgen_evolution_sequence();
    assert_monotonically_increasing(&scores, 0.8); // 80% tolerance
}
```

#### `curve_hillclimber_transfer.rs`
```rust
// Curve 2: HillClimber Pass Rate vs TestGen Config
// Does quality transfer?
#[test]
fn test_curve2_hillclimber_improves_with_better_testgen() {
    let pass_rates: Vec<f64> = vec![
        run_hillclimber_with_testgen_quality(Quality::Low),
        run_hillclimber_with_testgen_quality(Quality::Medium),
        run_hillclimber_with_testgen_quality(Quality::High),
    ];
    assert_monotonically_increasing(&pass_rates, 0.8);
}
```

#### `curve_tb2_correlation.rs`
```rust
// Curve 3: TB2 Performance vs Internal Metrics
// Is our proxy valid?
#[test]
fn test_curve3_internal_metrics_predict_tb2() {
    let runs = load_historical_runs();
    let correlation = pearson_correlation(
        &runs.iter().map(|r| r.internal_score).collect(),
        &runs.iter().map(|r| r.tb2_score).collect(),
    );
    assert!(correlation > 0.7, "Internal metrics should predict TB2");
}
```

---

## Part 4: Test Fixtures

### 4.1 HillClimber Test Crate (`crates/hillclimber_test/`)

Following the `hud_test` pattern:

```rust
// fixtures/mod.rs
pub struct OrchestratorFixture {
    orchestrator: Orchestrator<MockFM, MockExecutor, MockEmitter>,
    events: Arc<Mutex<Vec<HillClimberEvent>>>,
}

impl OrchestratorFixture {
    pub fn create() -> Self { ... }
    pub fn with_config(config: HillClimberConfig) -> Self { ... }
    pub fn with_mock_fm(responses: Vec<FMResponse>) -> Self { ... }
    pub fn run_turn(&mut self) -> TurnResult { ... }
    pub fn run_all_turns(&mut self) -> RunResult { ... }
    pub fn get_events(&self) -> Vec<HillClimberEvent> { ... }
}

// Fluent assertions
pub trait OrchestratorAssertExt {
    fn assert_that(&self) -> OrchestratorAssertions;
}

impl<'a> OrchestratorAssertions<'a> {
    pub fn completed_successfully(self) -> Self { ... }
    pub fn has_turn_count(self, expected: u32) -> Self { ... }
    pub fn has_score_above(self, threshold: u32) -> Self { ... }
    pub fn emitted_event(self, event_type: &str) -> Self { ... }
}
```

### 4.2 TestGen Test Crate (`crates/testgen_test/`)

```rust
// fixtures/mod.rs
pub struct GeneratorFixture {
    generator: Generator<MockFM>,
    store: TestGenStore,
}

impl GeneratorFixture {
    pub fn create() -> Self { ... }
    pub fn with_config(config: TestGenConfig) -> Self { ... }
    pub fn run_iteration(&mut self) -> IterationResult { ... }
    pub fn get_tests(&self) -> Vec<GeneratedTest> { ... }
}

// Fluent assertions
pub trait GeneratorAssertExt {
    fn assert_that(&self) -> GeneratorAssertions;
}

impl<'a> GeneratorAssertions<'a> {
    pub fn has_tests(self) -> Self { ... }
    pub fn has_test_count_at_least(self, min: usize) -> Self { ... }
    pub fn covers_category(self, category: TestCategory) -> Self { ... }
    pub fn has_comprehensiveness_at_least(self, min: f64) -> Self { ... }
}
```

### 4.3 Mock Infrastructure

```rust
// mocks/fm.rs
pub struct MockFM {
    responses: VecDeque<FMResponse>,
    calls: Arc<Mutex<Vec<FMRequest>>>,
}

impl FMClient for MockFM {
    async fn generate(&self, request: FMRequest) -> Result<FMResponse> { ... }
}

// mocks/executor.rs
pub struct MockExecutor {
    results: HashMap<String, ToolResult>,
}

impl ToolExecutor for MockExecutor {
    async fn execute(&self, action: Action) -> Result<ToolResult> { ... }
}

// mocks/docker.rs
pub struct MockDocker {
    pytest_output: String,
    exit_code: i32,
}

impl MockDocker {
    pub fn passing_all() -> Self { ... }
    pub fn failing_some(failures: &[&str]) -> Self { ... }
}
```

---

## Part 5: Anti-Cheat Verification Tests

**CRITICAL**: These tests ensure we're not gaming the benchmark.

```rust
// anti_cheat.rs
#[test]
fn test_no_hardcoded_task_ids() {
    let decomposer_src = include_str!("../src/decomposer.rs");
    assert!(!decomposer_src.contains("regex-log"));
    assert!(!decomposer_src.contains("fizzbuzz"));
    // No task ID references anywhere in decomposition logic
}

#[test]
fn test_no_solution_hints_in_prompts() {
    let prompt_src = include_str!("../src/prompt.rs");
    assert!(!prompt_src.contains("(?="));  // No regex lookahead hints
    assert!(!prompt_src.contains("\\b"));   // No word boundary hints
    // FM must discover techniques, not be given them
}

#[test]
fn test_testgen_uses_description_not_tb2() {
    let generator_src = include_str!("../src/generator.rs");
    assert!(!generator_src.contains("test_outputs.py"));
    assert!(!generator_src.contains("/app/"));
    // TestGen generates from description, not TB2 files
}

#[test]
fn test_decomposer_is_task_agnostic() {
    // Run decomposer on two different tasks
    let subtasks_a = decompose("Write a regex parser");
    let subtasks_b = decompose("Implement fizzbuzz");

    // Should produce same structure (4 subtasks)
    assert_eq!(subtasks_a.len(), subtasks_b.len());
    assert_eq!(subtasks_a[0].phase, subtasks_b[0].phase);
    // No task-specific logic
}
```

---

## Part 6: Implementation Order

### Phase 1: Foundation
1. Create `hillclimber_test` crate with mock infrastructure
2. Create `testgen_test` crate with fixtures
3. Add unit tests for `scoring.rs` (both crates)
4. Add unit tests for `store.rs` (both crates)

### Phase 2: Core Logic
5. Add unit tests for `evaluator.rs`
6. Add unit tests for `prompt.rs`
7. Add unit tests for `monitor.rs`
8. Add unit tests for `decomposer.rs`
9. Add unit tests for `generator.rs`
10. Add unit tests for `analyzer.rs`

### Phase 3: Orchestration
11. Add unit tests for `orchestrator.rs`
12. Add unit tests for `sampler.rs`
13. Add integration tests for orchestrator + evaluator
14. Add integration tests for generator + analyzer

### Phase 4: E2E & Validation
15. Add E2E test for complete flow
16. Add Three Curves validation tests
17. Add anti-cheat verification tests
18. Set up CI pipeline

---

## CI Integration

```bash
# Fast tests (no Docker)
cargo test --lib -p hillclimber
cargo test --lib -p testgen

# Integration tests
cargo test --test '*' -p hillclimber
cargo test --test '*' -p testgen

# E2E tests (requires Docker)
cargo test --test 'e2e_*' -p hillclimber -- --ignored

# Three Curves (requires historical data)
cargo test --test 'curve_*' -p hillclimber -- --ignored
```

---

## Critical Files to Create/Modify

**New crates:**
- `crates/hillclimber_test/` - Test fixtures and mocks
- `crates/testgen_test/` - Test fixtures and mocks

**New test files in hillclimber:**
- `crates/hillclimber/tests/integration_orchestrator.rs`
- `crates/hillclimber/tests/integration_sampler.rs`
- `crates/hillclimber/tests/integration_testgen.rs`
- `crates/hillclimber/tests/e2e/e2e_complete_flow.rs`
- `crates/hillclimber/tests/three_curves/curve_*.rs`
- `crates/hillclimber/tests/anti_cheat.rs`

**New test files in testgen:**
- `crates/testgen/tests/integration_generation.rs`
- `crates/testgen/tests/integration_formatter.rs`

---

## Success Criteria

- [ ] 100% of HillClimber modules have unit tests
- [ ] 100% of TestGen modules have unit tests
- [ ] Integration tests cover all cross-module flows
- [ ] E2E test passes with real Docker evaluation
- [ ] Three Curves tests implemented and passing
- [ ] Anti-cheat tests verify no hardcoded solutions
- [ ] CI runs all tests on every PR
- [ ] Test fixtures follow hud_test/gym_test pattern
