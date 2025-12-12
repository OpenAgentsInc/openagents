# Claude TestGen Integration Plan

## Goal

Equip Claude Code with TestGen methodology: Before writing solutions, Claude MUST:
1. **DESCRIBE** - Analyze task requirements into structured criteria
2. **WRITE TESTS** - Create pytest tests that would prove the description is satisfied
3. **ITERATE** - Write solution → run tests → fix → repeat until passing

Tests run in-container via pytest. If done correctly, generated tests should equal or exceed TB2 benchmark tests WITHOUT including any actual TB2 test data.

## The Philosophy (Anti-Cheating)

```
LEGITIMATE                                                      CHEATING
    |                                                               |
    |   Domain     Process    Test         Expected    Hardcoded   |
    |   Knowledge  Knowledge  Feedback     Output      Solutions   |
    |   (regex     (TDD       (pass/fail)  Leakage     (if/else    |
    |   syntax)    approach)               ("got X,    task==X)    |
    |                                       expected               |
    +---------------------------------------------------------------+
```

**Key Principle**: Claude discovers solutions through iteration against SELF-GENERATED tests derived from the task description. Never from TB2 test files.

## Architecture

```
Task Instruction
       |
       v
┌─────────────────────────────────────────────┐
│        TESTGEN WORKFLOW (in Docker)         │
│                                             │
│  1. DESCRIBE ──────────────────────────┐    │
│     Parse requirements into criteria   │    │
│                                        │    │
│  2. WRITE TESTS ◄──────────────────────┤    │
│     Generate pytest from description   │    │
│     Save to /app/testgen_tests.py      │    │
│                                        │    │
│  3. ITERATE ◄──────────────────────────┤    │
│     Write solution → pytest → fix      │    │
│     Loop until tests pass              │    │
│                                        │    │
└─────────────────────────────────────────────┘
       |
       v (only after internal tests pass)
TB2 Verification (tests/test.sh)
       |
       v
Pass/Fail + reward.txt
```

## Implementation Approach

### Option: Instruction-Embedded TestGen Protocol

The simplest approach that leverages Claude's existing capabilities:

1. **Wrap task instruction** with TestGen protocol
2. Claude follows protocol autonomously
3. Verify protocol was followed before TB2 submission

This avoids complex tooling while ensuring the workflow is followed.

## Files to Create

### 1. `.claude/skills/testgen-protocol/SKILL.md`

Claude skill that teaches the TestGen methodology:

```yaml
---
name: testgen-protocol
description: "Use this skill when solving any coding task to ensure systematic test-driven development"
---

# TestGen Protocol

When solving ANY coding task, you MUST follow this exact workflow:

## Phase 1: DESCRIBE (Required)

Before writing ANY code, analyze the task and output a structured description:

```
### TASK ANALYSIS
**Goal**: [One sentence summary]
**Output**: [What file/format to produce]
**Constraints**: [List all constraints from the description]

### ACCEPTANCE CRITERIA
1. [Criterion 1 - must be testable]
2. [Criterion 2 - must be testable]
...
```

## Phase 2: WRITE TESTS (Required)

Create `/app/testgen_tests.py` with pytest tests for EACH acceptance criterion:

```python
import pytest

# Test for criterion 1
def test_criterion_1():
    """[Description of what this tests]"""
    # Test implementation
    assert ...

# Test for criterion 2
def test_criterion_2():
    """[Description of what this tests]"""
    # Test implementation
    assert ...
```

**Rules**:
- Tests must be derived ONLY from the task description
- Include edge cases you can infer from the description
- Include boundary conditions
- NEVER copy tests from /tests/ directory (that's cheating)

## Phase 3: ITERATE (Required)

1. Write initial solution
2. Run: `pytest /app/testgen_tests.py -v`
3. If tests fail: analyze failures, fix solution, goto step 2
4. If tests pass: solution is ready

## CRITICAL: Anti-Cheating Rules

- NEVER read /tests/test_outputs.py or any TB2 test files
- NEVER hardcode solutions based on task ID
- Tests must be DERIVABLE from task description alone
- If you find yourself "knowing" a test case, ask: "Could I derive this from the description?"
```

### 2. `crates/gym/src/mechacoder/testgen_wrapper.rs` (~150 lines)

Wraps task instruction with TestGen protocol requirement:

```rust
pub struct TestGenWrapper;

impl TestGenWrapper {
    /// Wrap a task instruction with TestGen protocol
    pub fn wrap_instruction(original: &str) -> String {
        format!(r#"
{TESTGEN_PREAMBLE}

## YOUR TASK

{original}

## REQUIRED WORKFLOW

You MUST follow the TestGen Protocol skill before submitting your solution:

1. **DESCRIBE**: Output your task analysis with acceptance criteria
2. **WRITE TESTS**: Create /app/testgen_tests.py with tests for each criterion
3. **ITERATE**: Write solution → run pytest → fix → repeat

Only proceed to final solution after ALL your generated tests pass.

DO NOT read or reference /tests/* - those are for final verification only.
"#, TESTGEN_PREAMBLE = TESTGEN_PREAMBLE, original = original)
    }
}

const TESTGEN_PREAMBLE: &str = r#"
# TestGen Protocol Active

This task requires Test-Driven Development. You must generate your own tests
from the task description before writing the solution.

This ensures you UNDERSTAND the requirements before implementing.
"#;
```

### 3. `crates/gym/src/mechacoder/testgen_validator.rs` (~100 lines)

Validates Claude followed the TestGen protocol:

```rust
pub struct TestGenValidator;

pub struct ValidationResult {
    pub describe_found: bool,
    pub tests_created: bool,
    pub tests_passed: bool,
    pub iteration_count: u32,
}

impl TestGenValidator {
    /// Check if /app/testgen_tests.py exists
    pub async fn check_tests_exist(workspace: &Path) -> bool {
        workspace.join("testgen_tests.py").exists()
    }

    /// Run testgen tests and return pass/fail
    pub async fn run_testgen_tests(container_id: &str) -> Result<(bool, String)> {
        // docker exec pytest /app/testgen_tests.py -v
    }

    /// Parse Claude output for DESCRIBE section
    pub fn parse_describe_section(output: &str) -> Option<TaskAnalysis> {
        // Look for "### TASK ANALYSIS" section
    }
}
```

## Files to Modify

### `crates/gym/src/mechacoder/docker_runner.rs`

Add TestGen wrapper to instruction:

```rust
// In build_claude_command():
fn build_claude_command(&self, config: &DockerRunConfig) -> Vec<String> {
    // Wrap instruction with TestGen protocol
    let wrapped_instruction = TestGenWrapper::wrap_instruction(&config.task.instruction);

    // Build command with wrapped instruction
    ...
}
```

### `crates/gym/src/mechacoder/mod.rs`

Add TestGen validation before TB2 verification:

```rust
// In run_docker_task():
async fn run_docker_task(...) {
    // ... Claude runs with TestGen protocol ...

    // Validate TestGen was followed
    let validation = TestGenValidator::validate(&workspace_dir).await;
    if !validation.tests_created {
        // Log warning: Claude skipped test generation
    }

    // Run testgen tests first
    let (testgen_passed, testgen_output) = TestGenValidator::run_testgen_tests(...).await?;

    // Only run TB2 verification if testgen passed
    if testgen_passed {
        // Run TB2 verification
    }
}
```

## Container Directory Structure

```
/app/
├── testgen_tests.py      # Claude-generated tests (TestGen)
├── regex.txt             # Claude's solution (or whatever output)
└── ...

/logs/
├── agent/
│   ├── testgen_analysis.md   # Claude's DESCRIBE output
│   └── claude-code.txt       # Full Claude output
└── verifier/
    ├── reward.txt            # TB2 final result
    └── ctrf.json             # TB2 test details

/tests/                       # TB2 official tests (read-only, Claude MUST NOT read)
├── test.sh
└── test_outputs.py
```

## Validation: Three Curves for Claude TestGen

### Curve 1: TestGen Quality vs Task Complexity
- Do Claude-generated tests cover the actual requirements?
- Measured by: TestGen tests passed → TB2 tests passed correlation

### Curve 2: Iteration Count vs Success Rate
- Does more iteration lead to better solutions?
- Track: iterations until testgen pass vs TB2 pass rate

### Curve 3: TestGen Coverage vs TB2 Coverage
- Do Claude's tests predict TB2 test coverage?
- Measured by: Jaccard similarity of test cases

## Implementation Order

1. **Create TestGen skill** - `.claude/skills/testgen-protocol/SKILL.md`
2. **Create wrapper module** - `testgen_wrapper.rs`
3. **Create validator module** - `testgen_validator.rs`
4. **Integrate into docker_runner** - Wrap instructions
5. **Integrate into mod.rs** - Validate before TB2 verification
6. **Test end-to-end** - Run on regex-log task

## Success Criteria

1. Claude outputs DESCRIBE section with acceptance criteria
2. Claude creates `/app/testgen_tests.py` before solution
3. Claude iterates until testgen tests pass
4. TestGen tests correlate with TB2 test coverage (≥80% overlap)
5. No TB2 test data leakage in testgen tests

## Critical Files Reference

| File | Purpose |
|------|---------|
| `crates/gym/src/mechacoder/docker_runner.rs` | Wrap instruction with TestGen |
| `crates/gym/src/mechacoder/mod.rs` | Add TestGen validation step |
| `.claude/skills/testgen-protocol/SKILL.md` | TestGen methodology for Claude |
| `docs/logs/20251208/1219-benchmark-gaming-analysis.md` | Anti-cheating philosophy |
| `docs/logs/20251209/1454-decomposer-cleanup-no-cheating.md` | What's legitimate vs cheating |


