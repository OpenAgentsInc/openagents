//! TestGen Wrapper - Wraps task instructions with TestGen protocol
//!
//! This module wraps task instructions with the TestGen protocol, requiring
//! Claude to DESCRIBE → WRITE TESTS → ITERATE before submitting solutions.
//!
//! The TestGen protocol ensures Claude:
//! 1. Understands requirements before implementing
//! 2. Generates comprehensive tests from the description alone
//! 3. Iterates until self-generated tests pass
//! 4. Never reads the official benchmark tests (anti-cheating)

/// TestGen instruction wrapper
pub struct TestGenWrapper;

impl TestGenWrapper {
    /// Wrap a task instruction with the TestGen protocol
    ///
    /// This prepends and appends TestGen workflow requirements to the original
    /// task instruction, ensuring Claude follows the DESCRIBE → TESTS → ITERATE
    /// workflow before submitting a solution.
    pub fn wrap_instruction(original: &str) -> String {
        format!(
            r#"{preamble}

## YOUR TASK

{original}

## REQUIRED WORKFLOW: TestGen Protocol

You MUST follow this EXACT workflow before submitting your solution:

### Step 1: DESCRIBE (Output this section first)

Analyze the task and output a structured description:

```markdown
### TASK ANALYSIS
**Goal**: [One sentence summary]
**Output**: [What file/format to produce]
**Constraints**: [List ALL constraints from the description]

### ACCEPTANCE CRITERIA
1. [Testable criterion 1]
2. [Testable criterion 2]
...

### EDGE CASES TO CONSIDER
- [Edge case 1 inferred from description]
- [Edge case 2]
...
```

### Step 2: WRITE TESTS (Create /app/testgen_tests.py)

Create pytest tests for EACH acceptance criterion:

```python
"""TestGen-generated tests - derived from task description ONLY."""
import pytest
import os

def test_solution_exists():
    """Required output file was created."""
    assert os.path.exists("/app/YOUR_OUTPUT_FILE")

def test_criterion_1():
    """[Describe what this tests]"""
    # Test implementation
    pass

# ... more tests for each criterion and edge case ...
```

### Step 3: ITERATE (Loop until tests pass)

```
1. Write initial solution
2. Run: pytest /app/testgen_tests.py -v
3. If FAIL: fix solution, go to step 2
4. If PASS: you're done
```

## CRITICAL RULES

- **NEVER read /tests/*** - Those are for final verification only
- **NEVER read test_outputs.py** - That's benchmark test data (cheating!)
- **Derive tests from description ONLY** - Ask: "Could I write this test knowing ONLY the description?"
- **Fix the solution, not the tests** - If tests fail, the solution is wrong

## START NOW

Begin with Step 1: DESCRIBE. Output your task analysis before writing any code.
"#,
            preamble = TESTGEN_PREAMBLE,
            original = original
        )
    }

    /// Check if an instruction is already wrapped with TestGen protocol
    pub fn is_wrapped(instruction: &str) -> bool {
        instruction.contains("TestGen Protocol Active")
            || instruction.contains("REQUIRED WORKFLOW: TestGen Protocol")
    }
}

/// Preamble added to the beginning of every TestGen-wrapped instruction
const TESTGEN_PREAMBLE: &str = r#"# TestGen Protocol Active

This task requires Test-Driven Development. You MUST generate your own tests
from the task description before writing the solution.

This ensures you UNDERSTAND the requirements before implementing. If your
self-generated tests are comprehensive, they will cover what the benchmark
tests check - WITHOUT you needing to see the benchmark tests.

The workflow is: DESCRIBE → WRITE TESTS → ITERATE → SOLUTION
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_instruction() {
        let original = "Write a regex to match dates.";
        let wrapped = TestGenWrapper::wrap_instruction(original);

        assert!(wrapped.contains("TestGen Protocol Active"));
        assert!(wrapped.contains("Write a regex to match dates."));
        assert!(wrapped.contains("DESCRIBE"));
        assert!(wrapped.contains("WRITE TESTS"));
        assert!(wrapped.contains("ITERATE"));
        assert!(wrapped.contains("NEVER read /tests/*"));
    }

    #[test]
    fn test_is_wrapped() {
        let unwrapped = "Just a regular instruction.";
        let wrapped = TestGenWrapper::wrap_instruction(unwrapped);

        assert!(!TestGenWrapper::is_wrapped(unwrapped));
        assert!(TestGenWrapper::is_wrapped(&wrapped));
    }
}
