# Plan: Autopilot UI ↔ CLI Bidirectional Connection + DSPy Verification

## Overview

Two main goals:
1. **Bidirectional UI Connection** - Start autopilot runs from desktop app + improve event streaming
2. **DSPy Integration** - Convert the verification phase to use DSPy signatures

---

## Part 1: Bidirectional UI Connection

### Current State
- `autopilot-shell` uses `AutopilotRuntime` which wraps `StartupState`
- "Full Auto" mode ticks the runtime and displays snapshots
- UI can view sessions but cannot START new runs
- No way to input a prompt or select an issue from the UI

### Goal
- User can type a prompt or select an issue in the UI to start a new autopilot run
- Real-time streaming continues to work
- Session state flows bidirectionally

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    AUTOPILOT-SHELL (UI)                      │
│  ┌─────────────┐   ┌─────────────┐   ┌─────────────────────┐│
│  │PromptInput  │   │ IssueList   │   │   ThreadView        ││
│  │ (new)       │   │ (new)       │   │   (existing)        ││
│  └──────┬──────┘   └──────┬──────┘   └─────────────────────┘│
│         │                 │                    ▲             │
│         └────────┬────────┘                    │             │
│                  ▼                             │             │
│         ┌───────────────────┐                  │             │
│         │  AutopilotRuntime │──────────────────┘             │
│         │  (enhanced)       │  RuntimeSnapshot               │
│         └─────────┬─────────┘                                │
└───────────────────┼──────────────────────────────────────────┘
                    │
                    ▼ start_run(prompt) / start_issue(number)
         ┌─────────────────────┐
         │  autopilot-service  │
         │  (new methods)      │
         └─────────────────────┘
```

### Implementation Steps

#### 1.1 Add Prompt Input to UI
**File:** `crates/autopilot-shell/src/components/prompt_input.rs` (NEW)

```rust
pub struct PromptInput {
    text: String,
    focused: bool,
    placeholder: &'static str,
}

impl PromptInput {
    pub fn new() -> Self { ... }
    pub fn value(&self) -> &str { &self.text }
    pub fn clear(&mut self) { self.text.clear(); }
}

impl Component for PromptInput {
    fn paint(&self, ctx: &mut PaintCtx, node: &LayoutNode) { ... }
    fn event(&mut self, event: &Event) -> bool { ... }
}
```

#### 1.2 Add Issue Selector to Sessions Panel
**File:** `crates/autopilot-shell/src/panels/sessions.rs` (MODIFY)

Add:
- Issue list from `.openagents/issues.json`
- Click to select issue → trigger run
- Show issue status (open/claimed/blocked)

#### 1.3 Extend AutopilotRuntime with start methods
**File:** `crates/autopilot-service/src/runtime.rs` (MODIFY)

```rust
impl AutopilotRuntime {
    /// Start a new run with a prompt
    pub fn start_run(&mut self, prompt: String, model: ClaudeModel) -> Result<()> {
        self.state = StartupState::new_with_prompt(prompt, model);
        Ok(())
    }

    /// Start a run for a specific issue
    pub fn start_issue(&mut self, issue_number: u32, model: ClaudeModel) -> Result<()> {
        let issue = load_issue(issue_number)?;
        let prompt = issue.to_prompt();
        self.start_run(prompt, model)
    }

    /// Check if currently running
    pub fn is_running(&self) -> bool {
        !matches!(self.state.phase, StartupPhase::Idle | StartupPhase::Complete)
    }
}
```

#### 1.4 Add StartupState::new_with_prompt
**File:** `crates/autopilot/src/startup.rs` (MODIFY)

```rust
impl StartupState {
    /// Create a new state with a specific prompt
    pub fn new_with_prompt(prompt: String, model: ClaudeModel) -> Self {
        Self {
            phase: StartupPhase::CheckingOpenCode,
            user_prompt: Some(prompt),
            model,
            session_id: generate_session_id(),
            ..Default::default()
        }
    }
}
```

#### 1.5 Wire up UI events
**File:** `crates/autopilot-shell/src/shell.rs` (MODIFY)

Add keyboard shortcut (⌘Enter) and button to submit prompt:
```rust
fn handle_submit(&mut self) {
    if let Some(prompt) = self.prompt_input.take_value() {
        let model = self.sessions_panel.selected_model();
        if let Err(e) = self.runtime.start_run(prompt, model) {
            self.show_error(e);
        }
    }
}
```

#### 1.6 Add idle state handling
**File:** `crates/autopilot/src/startup.rs` (MODIFY)

Add `StartupPhase::Idle` variant for when no run is active:
```rust
pub enum StartupPhase {
    Idle,  // NEW - waiting for user input
    CheckingOpenCode,
    // ... rest unchanged
}
```

---

## Part 2: DSPy Verification Integration

### Why Verification Phase?

The verification phase is ideal for DSPy because:
1. **Well-defined I/O** - TerminationChecklist has 10 typed fields
2. **Iterative** - Runs up to 6 times, perfect for optimization
3. **Evaluation-friendly** - Pass/fail metrics work with DSPy's Evaluator
4. **Chain-of-thought** - Each check benefits from reasoning
5. **Optimizable** - COPRO/MIPROv2 can improve prompts over time

### Current Verification Flow

```
VerificationRunner
  ├── cargo build → build_clean
  ├── cargo clippy → clippy_clean
  ├── cargo test → tests_passing
  ├── cargo llvm-cov → coverage_adequate
  ├── grep "todo!|unimplemented!" → no_stubs
  ├── check .openagents/TODO.md → todos_complete
  └── ... (10 checks total)
```

### New DSPy Verification Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                   DspyVerificationRunner                     │
│  ┌─────────────────────────────────────────────────────────┐│
│  │ Module: VerificationPipeline                            ││
│  │   #[parameter] build_checker: Predict<BuildCheck>       ││
│  │   #[parameter] test_checker: Predict<TestCheck>         ││
│  │   #[parameter] coverage_checker: Predict<CoverageCheck> ││
│  │   #[parameter] stub_checker: Predict<StubCheck>         ││
│  │   #[parameter] todos_checker: Predict<TodosCheck>       ││
│  │   #[parameter] completeness_checker: Predict<Complete>  ││
│  └─────────────────────────────────────────────────────────┘│
│                              │                               │
│                              ▼                               │
│  ┌─────────────────────────────────────────────────────────┐│
│  │ Evaluator: ChecklistEvaluator                           ││
│  │   metric() → 0.0-1.0 based on pass/fail counts          ││
│  └─────────────────────────────────────────────────────────┘│
│                              │                               │
│  ┌─────────────────────────────────────────────────────────┐│
│  │ Optimizer: COPRO (lightweight, fast iteration)          ││
│  │   Trains on historical verification runs                ││
│  └─────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
```

### Implementation Steps

#### 2.1 Add DSRs dependency
**File:** `crates/autopilot/Cargo.toml` (MODIFY)

```toml
[features]
dspy = ["dspy-rs"]

[dependencies]
dspy-rs = { path = "../../DSRs/crates/dspy-rs", optional = true }
```

#### 2.2 Create verification signatures
**File:** `crates/autopilot/src/verification/signatures.rs` (NEW)

```rust
use dspy_rs::*;

/// Check if build output indicates success
#[Signature(cot, desc = "Analyze build output to determine if compilation succeeded")]
pub struct BuildCheck {
    #[input(desc = "Raw output from cargo build")]
    pub build_output: String,
    #[input(desc = "List of expected artifacts")]
    pub expected_artifacts: String,
    #[output(desc = "PASS if build succeeded with no errors, FAIL otherwise")]
    pub verdict: String,
    #[output(desc = "Brief explanation of the verdict")]
    pub reasoning: String,
}

/// Check if tests pass
#[Signature(cot, desc = "Analyze test output to determine if all tests pass")]
pub struct TestCheck {
    #[input(desc = "Raw output from cargo test")]
    pub test_output: String,
    #[output(desc = "PASS if all tests pass, FAIL if any fail")]
    pub verdict: String,
    #[output(desc = "Count of passed/failed/ignored tests")]
    pub summary: String,
    #[output(desc = "List of failed test names if any")]
    pub failed_tests: String,
}

/// Check code coverage
#[Signature(cot, desc = "Analyze coverage report against threshold")]
pub struct CoverageCheck {
    #[input(desc = "Coverage percentage as decimal (e.g., 0.85)")]
    pub coverage: f32,
    #[input(desc = "Required threshold (e.g., 0.90)")]
    pub threshold: f32,
    #[input(desc = "Uncovered lines report")]
    pub uncovered_lines: String,
    #[output(desc = "PASS if coverage >= threshold, FAIL otherwise")]
    pub verdict: String,
    #[output(desc = "Specific recommendations to improve coverage")]
    pub recommendations: String,
}

/// Check for stubs and unimplemented code
#[Signature(cot, desc = "Scan codebase for incomplete implementations")]
pub struct StubCheck {
    #[input(desc = "Files containing todo!, unimplemented!, or stub patterns")]
    pub stub_locations: String,
    #[output(desc = "PASS if no stubs found, FAIL otherwise")]
    pub verdict: String,
    #[output(desc = "List of files and line numbers with stubs")]
    pub stub_list: String,
}

/// Check if TODOs are complete
#[Signature(cot, desc = "Verify all TODO items have been addressed")]
pub struct TodosCheck {
    #[input(desc = "Contents of .openagents/TODO.md")]
    pub todo_contents: String,
    #[input(desc = "Recent git diff showing changes")]
    pub git_diff: String,
    #[output(desc = "PASS if all TODOs addressed, FAIL otherwise")]
    pub verdict: String,
    #[output(desc = "List of incomplete TODO items")]
    pub incomplete_items: String,
}

/// Overall completeness check
#[Signature(cot, desc = "Synthesize all checks into final verdict")]
pub struct CompletenessCheck {
    #[input(desc = "Results from all individual checks")]
    pub check_results: String,
    #[input(desc = "Original task requirements")]
    pub requirements: String,
    #[output(desc = "PASS if task is complete, FAIL otherwise")]
    pub verdict: String,
    #[output(desc = "Overall summary of completion status")]
    pub summary: String,
    #[output(desc = "Specific items to fix if FAIL")]
    pub fix_items: String,
}
```

#### 2.3 Create verification module
**File:** `crates/autopilot/src/verification/dspy_runner.rs` (NEW)

```rust
use dspy_rs::*;
use super::signatures::*;
use super::TerminationChecklist;

#[derive(Builder, Optimizable)]
pub struct VerificationPipeline {
    #[parameter]
    build_checker: Predict,
    #[parameter]
    test_checker: Predict,
    #[parameter]
    coverage_checker: Predict,
    #[parameter]
    stub_checker: Predict,
    #[parameter]
    todos_checker: Predict,
    #[parameter]
    completeness_checker: Predict,
}

impl VerificationPipeline {
    pub fn new() -> Self {
        Self {
            build_checker: Predict::new(BuildCheck::new()),
            test_checker: Predict::new(TestCheck::new()),
            coverage_checker: Predict::new(CoverageCheck::new()),
            stub_checker: Predict::new(StubCheck::new()),
            todos_checker: Predict::new(TodosCheck::new()),
            completeness_checker: Predict::new(CompletenessCheck::new()),
        }
    }
}

impl Module for VerificationPipeline {
    async fn forward(&self, inputs: Example) -> Result<Prediction> {
        // Run individual checks
        let build_result = self.build_checker.forward(example! {
            "build_output": "input" => inputs.get("build_output", None),
            "expected_artifacts": "input" => inputs.get("expected_artifacts", None),
        }).await?;

        let test_result = self.test_checker.forward(example! {
            "test_output": "input" => inputs.get("test_output", None),
        }).await?;

        // ... run other checks ...

        // Synthesize results
        let all_results = format!(
            "Build: {}\nTests: {}\nCoverage: {}\nStubs: {}\nTodos: {}",
            build_result.get("verdict", None),
            test_result.get("verdict", None),
            // ... other results
        );

        let final_result = self.completeness_checker.forward(example! {
            "check_results": "input" => all_results,
            "requirements": "input" => inputs.get("requirements", None),
        }).await?;

        Ok(final_result)
    }
}

impl Evaluator for VerificationPipeline {
    async fn metric(&self, _example: &Example, prediction: &Prediction) -> f32 {
        // Score based on verdict
        match prediction.get("verdict", None).as_str() {
            "PASS" => 1.0,
            "PARTIAL" => 0.5,
            _ => 0.0,
        }
    }
}

/// Convert DSPy pipeline result to TerminationChecklist
pub fn to_checklist(prediction: &Prediction) -> TerminationChecklist {
    // Parse verdicts from prediction and create checklist
    TerminationChecklist {
        build_clean: parse_check_result(prediction, "build"),
        clippy_clean: parse_check_result(prediction, "clippy"),
        tests_passing: parse_check_result(prediction, "tests"),
        coverage_adequate: parse_check_result(prediction, "coverage"),
        no_stubs: parse_check_result(prediction, "stubs"),
        todos_complete: parse_check_result(prediction, "todos"),
        // ... rest of fields
    }
}
```

#### 2.4 Integrate into StartupState
**File:** `crates/autopilot/src/startup.rs` (MODIFY)

```rust
#[cfg(feature = "dspy")]
use crate::verification::dspy_runner::{VerificationPipeline, to_checklist};

impl StartupState {
    async fn run_verification(&mut self) -> TerminationChecklist {
        #[cfg(feature = "dspy")]
        {
            // Use DSPy pipeline
            let pipeline = VerificationPipeline::new();
            let inputs = self.gather_verification_inputs().await;
            let prediction = pipeline.forward(inputs).await.unwrap();
            to_checklist(&prediction)
        }

        #[cfg(not(feature = "dspy"))]
        {
            // Fallback to existing verification
            self.verification_runner.as_mut().unwrap().run().await
        }
    }
}
```

#### 2.5 Add optimization support
**File:** `crates/autopilot/src/verification/optimizer.rs` (NEW)

```rust
use dspy_rs::*;
use super::dspy_runner::VerificationPipeline;

/// Load historical verification runs for training
pub fn load_training_examples(sessions_dir: &Path) -> Vec<Example> {
    // Load from ~/.openagents/sessions/*/checkpoint.json
    // Extract verification inputs and outcomes
    vec![]
}

/// Optimize the verification pipeline using COPRO
pub async fn optimize_verification(
    pipeline: &mut VerificationPipeline,
    sessions_dir: &Path,
) -> Result<()> {
    let examples = load_training_examples(sessions_dir);
    if examples.len() < 10 {
        // Not enough data to optimize
        return Ok(());
    }

    let optimizer = COPRO::builder()
        .breadth(5)
        .depth(2)
        .build();

    optimizer.compile(pipeline, examples).await?;
    Ok(())
}
```

---

## Part 3: File Changes Summary

### New Files
| File | Purpose |
|------|---------|
| `crates/autopilot-shell/src/components/prompt_input.rs` | Text input for prompts |
| `crates/autopilot/src/verification/signatures.rs` | DSPy signatures for checks |
| `crates/autopilot/src/verification/dspy_runner.rs` | DSPy verification pipeline |
| `crates/autopilot/src/verification/optimizer.rs` | COPRO optimization |
| `crates/autopilot/src/verification/mod.rs` | Module exports |

### Modified Files
| File | Change |
|------|--------|
| `crates/autopilot-shell/src/shell.rs` | Add prompt submission, idle state |
| `crates/autopilot-shell/src/panels/sessions.rs` | Add issue list, model selector |
| `crates/autopilot-shell/src/components/mod.rs` | Export PromptInput |
| `crates/autopilot-service/src/runtime.rs` | Add start_run, start_issue methods |
| `crates/autopilot/src/startup.rs` | Add Idle phase, new_with_prompt, DSPy integration |
| `crates/autopilot/Cargo.toml` | Add dspy-rs dependency |

---

## Part 4: Implementation Order

### Phase A: UI Bidirectional Connection
1. Add `StartupPhase::Idle` and `new_with_prompt()` to startup.rs
2. Add `start_run()` and `start_issue()` to runtime.rs
3. Create `PromptInput` component
4. Add issue list to sessions panel
5. Wire up ⌘Enter shortcut in shell.rs
6. Test: start run from UI, verify streaming works

### Phase B: DSPy Verification
1. Add dspy-rs dependency with feature flag
2. Create verification signatures
3. Implement VerificationPipeline module
4. Integrate into StartupState behind feature flag
5. Add COPRO optimization (optional, can be deferred)
6. Test: run with `--features dspy`, verify checklist populated

---

## Success Criteria

### Part 1: Bidirectional UI
- [ ] User can type prompt in UI and press ⌘Enter to start run
- [ ] User can click issue in sidebar to start run for that issue
- [ ] Model selector (Sonnet/Opus/Haiku) is wired up
- [ ] Streaming events appear in ThreadView in real-time
- [ ] Session persists and can be resumed

### Part 2: DSPy Verification
- [ ] `cargo build -p autopilot --features dspy` compiles
- [ ] Verification phase uses DSPy signatures
- [ ] Chain-of-thought reasoning visible in logs
- [ ] TerminationChecklist correctly populated from DSPy output
- [ ] Fallback to existing verification when feature disabled
