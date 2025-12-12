# HillClimber: TestGen Integration Plan

## Problem

HillClimber currently runs without generating tests first. The regex-log task fails because:
1. The FM agent has no feedback from comprehensive tests
2. It generates basic solutions that don't handle all requirements
3. Tests need to exist BEFORE the MAP loop starts

## Goal

Integrate testgen to generate comprehensive tests before the MAP loop executes, providing the FM with concrete test feedback to guide solution development.

---

## Current State

### What Works
- ✅ HillClimber orchestrator runs end-to-end
- ✅ FM generates basic solutions (e.g., `\b\d{4}-\d{2}-\d{2}\b` for regex-log)
- ✅ Pytest evaluation parses test output
- ✅ Prompt sanitization bypasses safety filters
- ✅ All infrastructure is functional

### What's Missing
- ❌ No test generation before MAP loop
- ❌ FM has no test feedback to guide improvements
- ❌ Solutions are incomplete (basic patterns, no edge cases)

---

## Implementation Plan

### Step 1: Add Pytest Formatter (NEW MODULE)
**File:** `crates/hillclimber/src/testgen_writer.rs`

Create a module to convert testgen's `GeneratedTest` objects to pytest format.

```rust
use testgen::types::{GeneratedTest, TestCategory};

/// Convert generated tests to pytest file format
pub fn format_as_pytest(tests: &[GeneratedTest], task_id: &str) -> String {
    let mut output = String::new();

    // Header
    output.push_str("# Generated tests for ");
    output.push_str(task_id);
    output.push_str("\n\nimport pytest\n\n");

    // Group by category
    for category in [AntiCheat, Existence, Correctness, Boundary, Integration] {
        let category_tests: Vec<_> = tests.iter()
            .filter(|t| t.category == category)
            .collect();

        if category_tests.is_empty() { continue; }

        output.push_str(&format!("\n# {} Tests\n", category_name));

        for test in category_tests {
            // Convert to pytest function
            output.push_str(&format!(
                "def test_{}():\n    \"\"\"{}\"\"\"\n",
                test.id.replace('-', '_'),
                test.reasoning
            ));

            // Add test body based on input/expected
            if let Some(expected) = &test.expected_output {
                output.push_str(&format!(
                    "    result = run_task(\"{}\")\n    assert result == \"{}\"\n\n",
                    test.input, expected
                ));
            } else {
                output.push_str(&format!(
                    "    run_task(\"{}\")\n    # Check manually\n\n",
                    test.input
                ));
            }
        }
    }

    output
}
```

**Critical:** This needs to be sophisticated enough to handle:
- Existence tests (file exists checks)
- Correctness tests (output matching)
- Boundary tests (edge case assertions)
- Anti-cheat tests (prohibited tool detection)

### Step 2: Add `generate_tests` Flag to Options
**File:** `crates/hillclimber/src/types.rs` (line ~345)

Add field to `MAPOrchestratorOptions`:
```rust
pub struct MAPOrchestratorOptions {
    pub workspace: PathBuf,
    pub timeout_secs: u64,
    pub max_turns: u32,
    pub task_description: String,
    pub verbose: bool,
    pub use_sampling: bool,
    pub generate_tests: bool,  // ← ADD THIS
}
```

**File:** `crates/hillclimber/src/runner.rs` (line ~108)

Update `RunOptions` default:
```rust
impl Default for RunOptions {
    fn default() -> Self {
        Self {
            // ... existing fields ...
            generate_tests: true,  // ALREADY EXISTS, just verify it's true
        }
    }
}
```

**File:** `crates/hillclimber/src/runner.rs` (line ~150)

Pass through to orchestrator options:
```rust
let orchestrator_options = MAPOrchestratorOptions {
    workspace: options.workspace.clone(),
    timeout_secs: options.timeout_secs,
    max_turns: options.max_turns,
    task_description: options.task.description.clone(),
    verbose: options.verbose,
    use_sampling: options.use_sampling,
    generate_tests: options.generate_tests,  // ← ADD THIS
};
```

### Step 3: Add TestGen Imports
**File:** `crates/hillclimber/src/orchestrator.rs` (at top)

Add imports for testgen:
```rust
use testgen::{TestGenerator, TestGenContext, EnvironmentInfo, NoopEmitter};
use fm_bridge::FMClient as FMBridgeClient;
```

**NOTE:** TestGen uses `fm_bridge::FMClient` directly - no adapter needed!

### Step 4: Inject TestGen Call in Orchestrator
**File:** `crates/hillclimber/src/orchestrator.rs` (after line 292)

Add test generation between file loading and MAP loop:

```rust
// Step 3: Load initial file contents
let mut file_contents: HashMap<String, String> = HashMap::new();
for path in &decomposition.files_to_read {
    if let Ok(result) = self.tool_executor.read_file(path).await {
        if result.success {
            file_contents.insert(path.clone(), result.output);
        }
    }
}

// Step 3.5: Generate comprehensive tests (NEW)
if self.options.generate_tests {
    if self.options.verbose {
        println!("Generating tests for task {}...", task.id);
    }

    // Create FM client for testgen (uses fm_bridge directly)
    let testgen_client = FMBridgeClient::new();
    let generator = TestGenerator::new(testgen_client);

    // Sanitize task description to avoid FM safety filter
    let sanitized_description = crate::prompt::sanitize_for_fm(&task.description);

    // Set up environment
    let environment = EnvironmentInfo::minimal();

    // Generate tests
    match generator.generate_iteratively(
        &sanitized_description,  // Use sanitized version
        &task.id,
        &environment,
        TestGenContext::Benchmark,  // All 5 categories
        &NoopEmitter,
    ).await {
        Ok(result) => {
            if self.options.verbose {
                println!("Generated {} tests", result.tests.len());
            }

            // Convert to pytest format
            let pytest_content = crate::testgen_writer::format_as_pytest(
                &result.tests,
                &task.id
            );

            // Write to workspace
            let test_file = "test_generated.py";
            if let Err(e) = self.tool_executor
                .write_file(test_file, &pytest_content)
                .await
            {
                if self.options.verbose {
                    println!("Warning: Failed to write tests: {}", e);
                }
                // Continue anyway - tests are enhancement, not requirement
            }
        }
        Err(e) => {
            if self.options.verbose {
                println!("Warning: Test generation failed: {}", e);
            }
            // Continue anyway
        }
    }
}

// Step 4: Main loop (EXISTING)
while state.total_turns < self.options.max_turns {
```

### Step 5: Update lib.rs Exports
**File:** `crates/hillclimber/src/lib.rs`

Add new module:
```rust
pub mod testgen_writer;  // ← ADD THIS

// ... existing modules ...
```

Re-export formatter:
```rust
pub use testgen_writer::format_as_pytest;  // ← ADD THIS
```

### Step 6: Handle TestGen Emitter (Optional Enhancement)
**File:** `crates/hillclimber/src/orchestrator.rs`

Instead of `NoopEmitter`, create a bridge:
```rust
struct TestGenEmitterBridge<'a> {
    hillclimber_emitter: &'a dyn HillClimberEmitter,
}

impl<'a> testgen::TestGenEmitter for TestGenEmitterBridge<'a> {
    fn on_progress(&self, phase: &str, category: Option<TestCategory>, round: u32, status: &str) {
        // Forward to hillclimber emitter if desired
        self.hillclimber_emitter.on_heartbeat(...);
    }
    // ... other methods
}
```

**NOTE:** This is optional - can start with NoopEmitter.

---

## Critical Implementation Details

### Pytest Format Requirements

The generated pytest file MUST:
1. **Be valid Python** - proper syntax, imports
2. **Match verification command** - if task uses `pytest -v`, file must be pytest-compatible
3. **Be executable** - tests must actually run
4. **Provide feedback** - assertion messages should guide FM

### Example Output

For regex-log task, the generated `test_generated.py` should produce:
```python
# Generated tests for regex-log

import pytest
import re
from pathlib import Path

def test_existence_regex_file():
    """Verify regex.txt exists"""
    assert Path("/app/regex.txt").exists()

def test_correctness_basic_date():
    """Match simple date with IP"""
    pattern = Path("/app/regex.txt").read_text().strip()
    text = "2025-01-09 User login from 192.168.0.1"
    matches = re.findall(pattern, text, re.MULTILINE)
    assert "2025-01-09" in matches

def test_correctness_last_date():
    """Match only last date on line with multiple dates"""
    pattern = Path("/app/regex.txt").read_text().strip()
    text = "192.168.1.100 accessed on 2023-12-31 and 2024-11-01"
    matches = re.findall(pattern, text, re.MULTILINE)
    assert matches == ["2024-11-01"]

# ... more tests
```

### Error Handling Strategy

1. **TestGen fails completely** → Warn and continue with empty tests
2. **TestGen returns empty** → Warn and continue
3. **File write fails** → Warn and continue
4. **Pytest format invalid** → Fail early with clear error

**Rationale:** Tests are an enhancement. If testgen fails, the MAP loop should still run (just without test feedback).

---

## Testing the Integration

### Manual Test
```bash
# Clean workspace
rm -rf /tmp/hillclimber-test/regex-log/test_generated.py

# Run with testgen enabled (default)
cargo run -p hillclimber -- \
  --tasks regex-log \
  --max-runs 1 \
  --max-turns 10 \
  --workspace /tmp/hillclimber-test/regex-log \
  -v

# Check generated tests
cat /tmp/hillclimber-test/regex-log/test_generated.py

# Verify tests run
cd /tmp/hillclimber-test/regex-log
pytest -v test_generated.py
```

### Success Criteria
1. ✅ `test_generated.py` file created
2. ✅ File contains valid pytest code
3. ✅ Tests execute without syntax errors
4. ✅ FM receives test feedback in subsequent turns
5. ✅ Solution improves (more tests pass)

---

## Files to Modify

| File | Lines | Change |
|------|-------|--------|
| `crates/hillclimber/src/lib.rs` | ~65 | Add `pub mod testgen_writer;` |
| `crates/hillclimber/src/testgen_writer.rs` | NEW | ~200 lines - pytest formatter |
| `crates/hillclimber/src/types.rs` | ~347 | Add `generate_tests: bool` field |
| `crates/hillclimber/src/runner.rs` | ~150 | Pass `generate_tests` to orchestrator |
| `crates/hillclimber/src/orchestrator.rs` | ~292 | Inject testgen call (20-40 lines) |

**Total:** ~250-300 new lines, 5 file modifications

---

## Risks & Mitigations

### Risk 1: TestGen Takes Too Long
**Impact:** Adds 30-120s to each run startup
**Mitigation:** Add timeout, make it skippable with `--no-generate-tests` flag

### Risk 2: FM Safety Filter Triggers in TestGen
**Impact:** Test generation fails completely
**Mitigation:** Sanitize task description before passing to testgen (reuse `sanitize_for_fm()`)

### Risk 3: Generated Tests Are Invalid Python
**Impact:** Pytest fails to run, no feedback
**Mitigation:** Validate pytest syntax before writing, fallback to empty file

### Risk 4: Context Window Exceeded in TestGen
**Impact:** Test generation fails
**Mitigation:** Use testgen's token limits, reduce `target_tests_per_category`

---

## Success Metrics

**Before testgen:**
- regex-log: 0/9 tests passing
- Score: 95 (FAIL)
- Turns: 5/5 used

**After testgen:**
- regex-log: Target 5+/9 tests passing
- Score: Target 1000+ (PASS)
- Tests provide concrete feedback to guide FM
