# Claude TestGen Integration

**Date:** 2024-12-11 14:15
**Status:** Complete

## Goal

Equip Claude Code with TestGen methodology: Before writing solutions, Claude MUST:
1. **DESCRIBE** - Analyze task requirements into structured criteria
2. **WRITE TESTS** - Create pytest tests that would prove the description is satisfied
3. **ITERATE** - Write solution → run tests → fix → repeat until passing

If done correctly, generated tests should equal or exceed what the benchmark tests check - WITHOUT including any actual TB2 test data.

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

## Files Created

### 1. `.claude/skills/testgen-protocol/SKILL.md` (~250 lines)

Claude skill that teaches the TestGen methodology:

- **Phase 1: DESCRIBE** - Task analysis with acceptance criteria
- **Phase 2: WRITE TESTS** - Create pytest tests for each criterion
- **Phase 3: ITERATE** - TDD loop until tests pass
- **Anti-Cheating Rules** - Never read /tests/*, derive from description only

Key test categories:
| Category | Purpose | Example |
|----------|---------|---------|
| Existence | File/output created | `test_solution_file_exists` |
| Correctness | Meets each requirement | `test_criterion_N` |
| Boundary | Edge cases | `test_empty_input`, `test_max_size` |
| Anti-cheat | Not hardcoded | `test_solution_is_general` |

### 2. `crates/gym/src/mechacoder/testgen_wrapper.rs` (~100 lines)

Wraps task instructions with TestGen protocol:

```rust
pub struct TestGenWrapper;

impl TestGenWrapper {
    /// Wrap a task instruction with TestGen protocol
    pub fn wrap_instruction(original: &str) -> String {
        // Prepends DESCRIBE → TESTS → ITERATE requirements
    }

    /// Check if an instruction is already wrapped
    pub fn is_wrapped(instruction: &str) -> bool
}
```

### 3. `crates/gym/src/mechacoder/testgen_validator.rs` (~200 lines)

Validates Claude followed the TestGen protocol:

```rust
pub struct TestGenValidation {
    pub describe_found: bool,      // DESCRIBE section in output
    pub tests_created: bool,       // testgen_tests.py exists
    pub tests_passed: bool,        // pytest passed
    pub tests_passed_count: u32,
    pub tests_total_count: u32,
    pub iteration_count: u32,
    pub test_output: String,
}

pub struct TestGenValidator;

impl TestGenValidator {
    /// Check if testgen_tests.py exists
    pub fn tests_exist(workspace_dir: &Path) -> bool

    /// Parse Claude output for DESCRIBE section
    pub fn parse_describe_section(claude_output: &str) -> bool

    /// Run testgen tests in container
    pub async fn run_testgen_tests(...) -> Result<TestGenValidation>
}
```

## Files Modified

### `crates/gym/src/mechacoder/mod.rs`

Added module imports and TestGen validation before TB2 verification:

```rust
pub mod testgen_validator;
pub mod testgen_wrapper;

use self::testgen_validator::TestGenValidator;

// In run_docker_task():
// After Claude completes, before TB2 verification:
1. Check if testgen_tests.py exists
2. If exists, run pytest /app/testgen_tests.py
3. Log whether TestGen tests passed/failed
4. Proceed to TB2 verification
```

### `crates/gym/src/mechacoder/docker_runner.rs`

Integrated TestGen wrapper into Claude command:

```rust
use crate::mechacoder::testgen_wrapper::TestGenWrapper;

fn build_claude_command(&self, instruction: &str, max_turns: u32) -> Vec<String> {
    // Wrap instruction with TestGen protocol
    let wrapped_instruction = TestGenWrapper::wrap_instruction(instruction);
    // Build command with wrapped instruction + --dangerously-skip-permissions
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
│   └── claude-code.txt       # Full Claude output
└── verifier/
    ├── reward.txt            # TB2 final result
    └── ctrf.json             # TB2 test details

/tests/                       # TB2 official tests (read-only, Claude MUST NOT read)
├── test.sh
└── test_outputs.py
```

## Testing

All 7 unit tests pass:

```bash
cargo test -p gym mechacoder::testgen

test mechacoder::testgen_validator::tests::test_count_iterations ... ok
test mechacoder::testgen_validator::tests::test_protocol_followed ... ok
test mechacoder::testgen_validator::tests::test_parse_describe_section ... ok
test mechacoder::testgen_validator::tests::test_parse_pytest_output ... ok
test mechacoder::testgen_wrapper::tests::test_wrap_instruction ... ok
test mechacoder::testgen_validator::tests::test_validation_default ... ok
test mechacoder::testgen_wrapper::tests::test_is_wrapped ... ok

test result: ok. 7 passed; 0 failed
```

## How It Works

1. **User starts task** → MechaCoder loads task from TB2
2. **Instruction wrapped** → `TestGenWrapper::wrap_instruction()` adds TestGen protocol
3. **Claude runs in Docker** → Sees instruction with DESCRIBE → TESTS → ITERATE requirement
4. **Claude follows protocol**:
   - Outputs TASK ANALYSIS section
   - Creates `/app/testgen_tests.py`
   - Writes solution → runs pytest → iterates
5. **TestGen validation** → `TestGenValidator` checks tests exist and pass
6. **TB2 verification** → Official tests run only after TestGen validation
7. **Results logged** → Both TestGen and TB2 results shown in UI

## Success Criteria

1. Claude outputs DESCRIBE section with acceptance criteria
2. Claude creates `/app/testgen_tests.py` before solution
3. Claude iterates until testgen tests pass
4. TestGen tests correlate with TB2 test coverage (≥80% overlap)
5. No TB2 test data leakage in testgen tests

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

## Related Files

| File | Purpose |
|------|---------|
| `crates/gym/src/mechacoder/testgen_wrapper.rs` | Wraps instructions with protocol |
| `crates/gym/src/mechacoder/testgen_validator.rs` | Validates protocol followed |
| `crates/gym/src/mechacoder/docker_runner.rs` | Builds Claude command |
| `crates/gym/src/mechacoder/mod.rs` | Integration point |
| `.claude/skills/testgen-protocol/SKILL.md` | TestGen methodology |
| `docs/logs/20251208/1219-benchmark-gaming-analysis.md` | Anti-cheating philosophy |
| `docs/logs/20251209/1454-decomposer-cleanup-no-cheating.md` | Legitimate vs cheating |

## Next Steps

1. **End-to-end test** - Run full regex-log task with TestGen
2. **Measure correlation** - Compare TestGen tests vs TB2 tests
3. **Tune parameters** - Adjust iteration limits if needed
4. **Add metrics** - Track TestGen quality over multiple runs
