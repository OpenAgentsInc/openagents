//! TestGen Wrapper - Wraps task instructions with TestGen protocol
//!
//! This module wraps task instructions with the TestGen protocol, requiring
//! Claude to DESCRIBE → MAP → WRITE TESTS → ITERATE before submitting solutions.
//!
//! The TestGen protocol ensures Claude:
//! 1. Understands requirements before implementing
//! 2. Maps all entities to all constraints (prevents coverage gaps)
//! 3. Generates comprehensive tests from the description alone
//! 4. Iterates until self-generated tests pass
//! 5. Never reads the official benchmark tests (anti-cheating)

/// TestGen instruction wrapper
pub struct TestGenWrapper;

impl TestGenWrapper {
    /// Wrap a task instruction with the TestGen protocol
    ///
    /// This prepends and appends TestGen workflow requirements to the original
    /// task instruction, ensuring Claude follows the DESCRIBE → MAP → TESTS → ITERATE
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
```

### Step 2: MAP ENTITIES & CONSTRAINTS (Required before writing tests)

Create a coverage matrix to ensure NO constraint is missed for ANY entity:

```markdown
### ENTITIES IDENTIFIED
List every distinct thing being validated/matched/processed:
1. [Entity 1] - e.g., "dates", "IPv4 addresses", "usernames"
2. [Entity 2]
...

### CONSTRAINTS IDENTIFIED
List every rule from the description (look for "must", "should", "ensure", "valid", "not"):
1. [Constraint 1] - e.g., "must be in format X", "not preceded by Y"
2. [Constraint 2]
...

### CONSTRAINT-ENTITY MATRIX
| Constraint | Entity 1 | Entity 2 | ... |
|------------|----------|----------|-----|
| Constraint 1 | ✓ or - | ✓ or - | ... |
| Constraint 2 | ✓ or - | ✓ or - | ... |

### REQUIRED TESTS (one per ✓ cell)
- test_entity1_constraint1
- test_entity2_constraint1 (if ✓)
- test_entity1_constraint2
...
```

**CRITICAL PATTERN**: When the description says "A and B must [constraint]", this means the constraint applies to BOTH A and B. You MUST write tests for BOTH.

Example: "ensure usernames and emails are properly validated"
- ✓ usernames → need test for username validation
- ✓ emails → need test for email validation
If you only test one entity, your solution may handle the other incorrectly!

### Step 3: WRITE TESTS (Create /app/testgen_tests.py)

Create pytest tests for EACH ✓ cell in your matrix:

```python
"""TestGen-generated tests - derived from task description ONLY."""
import pytest
import os

def test_solution_exists():
    """Required output file was created."""
    assert os.path.exists("/app/YOUR_OUTPUT_FILE")

# One test per ✓ cell in the CONSTRAINT-ENTITY MATRIX
def test_entity1_constraint1():
    """[What this tests - from matrix]"""
    pass

def test_entity2_constraint1():
    """[What this tests - from matrix]"""
    pass

# ... continue for ALL ✓ cells ...
```

### Step 4: ITERATE (Loop until tests pass)

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
- **Every ✓ in the matrix MUST have a test** - This prevents coverage gaps

## START NOW

Begin with Step 1: DESCRIBE. Then Step 2: MAP. Output both before writing any code.
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

The workflow is: DESCRIBE → MAP ENTITIES & CONSTRAINTS → WRITE TESTS → ITERATE → SOLUTION

CRITICAL: The MAP step prevents a common failure mode where constraints that
apply to MULTIPLE entities only get tested for ONE entity. Always build the
constraint-entity matrix before writing tests.
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
        assert!(wrapped.contains("MAP ENTITIES & CONSTRAINTS"));
        assert!(wrapped.contains("CONSTRAINT-ENTITY MATRIX"));
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
