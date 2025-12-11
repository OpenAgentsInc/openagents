---
name: testgen-protocol
description: "Use when solving coding tasks to ensure systematic test-driven development. Generates tests from task description before implementing solutions."
---

# TestGen Protocol

When solving ANY coding task, you MUST follow this exact workflow. This ensures you UNDERSTAND the requirements before implementing, and your solution will be verified against tests YOU generate.

## Phase 1: DESCRIBE (Required)

Before writing ANY code, analyze the task and output a structured description:

```markdown
### TASK ANALYSIS

**Goal**: [One sentence summary of what needs to be accomplished]

**Output**: [What file/format to produce, e.g., "/app/regex.txt containing a regex pattern"]

**Constraints**:
- [Constraint 1 from the description]
- [Constraint 2 from the description]
- [Continue listing ALL constraints]

### ACCEPTANCE CRITERIA

1. [Criterion 1 - must be testable and specific]
2. [Criterion 2 - must be testable and specific]
3. [Continue for ALL requirements]

### EDGE CASES TO CONSIDER

- [Edge case 1 that can be inferred from the description]
- [Edge case 2]
- [Continue listing edge cases]
```

## Phase 2: WRITE TESTS (Required)

Create `/app/testgen_tests.py` with pytest tests for EACH acceptance criterion:

```python
"""
TestGen-generated tests for task validation.
These tests are derived from the task description ONLY.
"""
import pytest
import re
import os

# ============================================================
# EXISTENCE TESTS - Does the solution exist?
# ============================================================

def test_solution_file_exists():
    """Verify the required output file was created."""
    assert os.path.exists("/app/OUTPUT_FILE"), "Solution file not created"

def test_solution_not_empty():
    """Verify the solution file is not empty."""
    with open("/app/OUTPUT_FILE") as f:
        content = f.read().strip()
    assert len(content) > 0, "Solution file is empty"

# ============================================================
# CORRECTNESS TESTS - Does it meet the requirements?
# ============================================================

def test_criterion_1():
    """[Describe what criterion 1 tests - from ACCEPTANCE CRITERIA]"""
    # Load solution
    with open("/app/OUTPUT_FILE") as f:
        solution = f.read().strip()

    # Test against criterion
    # ... implementation based on what the criterion requires ...
    assert condition, "Failed criterion 1: explanation"

def test_criterion_2():
    """[Describe what criterion 2 tests]"""
    # ... similar structure ...
    pass

# ============================================================
# BOUNDARY TESTS - Edge cases from the description
# ============================================================

def test_edge_case_1():
    """[Describe the edge case being tested]"""
    # ... test implementation ...
    pass

# ============================================================
# ANTI-CHEAT TESTS - Ensure solution is not hardcoded
# ============================================================

def test_solution_is_general():
    """Verify solution works for variations, not just known cases."""
    # Generate similar but different test cases
    # Solution should handle variations correctly
    pass
```

### Test Generation Rules

1. **DERIVE FROM DESCRIPTION ONLY** - Every test must trace back to the task description
2. **NO TB2 TEST READING** - NEVER read /tests/test_outputs.py or any benchmark test files
3. **INCLUDE EDGE CASES** - Think about what edge cases the description implies
4. **BE COMPREHENSIVE** - Cover all acceptance criteria
5. **BE SPECIFIC** - Each test should fail for a clear reason

### Categories to Cover

| Category | Purpose | Example |
|----------|---------|---------|
| Existence | File/output created | `test_solution_file_exists` |
| Correctness | Meets each requirement | `test_criterion_N` |
| Boundary | Edge cases | `test_empty_input`, `test_max_size` |
| Anti-cheat | Not hardcoded | `test_solution_is_general` |

## Phase 3: ITERATE (Required)

Follow this loop until ALL your tests pass:

```
1. Write initial solution based on your understanding
2. Run: pytest /app/testgen_tests.py -v
3. If tests FAIL:
   a. Read the failure message
   b. Understand what went wrong
   c. Fix the solution (not the tests!)
   d. Go back to step 2
4. If tests PASS:
   a. Solution is ready for verification
   b. Do NOT modify the solution further
```

### Iteration Best Practices

- **Fix the solution, not the tests** - If tests fail, the solution is wrong
- **One fix at a time** - Don't try to fix multiple failures at once
- **Understand before fixing** - Read the error message carefully
- **Track iterations** - Note which iteration you're on

## CRITICAL: Anti-Cheating Rules

### NEVER DO THESE:

1. **NEVER read `/tests/*`** - Those are for final verification only
2. **NEVER read `test_outputs.py`** - That's the benchmark's test data
3. **NEVER hardcode based on task ID** - No `if task == "regex-log"`
4. **NEVER copy known solutions** - Discover through iteration

### ALWAYS DO THESE:

1. **Derive tests from description** - Ask: "Could I write this test knowing ONLY the description?"
2. **Test your understanding** - Tests reveal if you understood the requirements
3. **Iterate honestly** - If tests fail, the solution needs work

### The Philosophy

The purpose of TestGen is to ensure you UNDERSTAND the requirements before implementing. If your tests are comprehensive and derived only from the description, they should cover what the official benchmark tests check.

```
Good TestGen = Understanding Requirements = Passing Benchmark
```

If you find yourself wanting to "peek" at the official tests, that means your understanding is incomplete. Go back to Phase 1 and re-read the description.

## Example Workflow

For a task "Write a function that reverses a string and saves to /app/output.txt":

### Phase 1: DESCRIBE
```markdown
### TASK ANALYSIS
**Goal**: Create a string reversal function
**Output**: /app/output.txt with reversed string
**Constraints**: Must reverse the input string

### ACCEPTANCE CRITERIA
1. Output file must exist at /app/output.txt
2. Content must be the reverse of input
3. Empty string should produce empty output

### EDGE CASES
- Empty string
- Single character
- Unicode characters
- Very long strings
```

### Phase 2: WRITE TESTS
```python
def test_output_exists():
    assert os.path.exists("/app/output.txt")

def test_reverses_simple_string():
    # Assuming input was "hello"
    with open("/app/output.txt") as f:
        assert f.read().strip() == "olleh"

def test_empty_string():
    # If input is "", output should be ""
    pass

def test_unicode():
    # Unicode should be handled correctly
    pass
```

### Phase 3: ITERATE
```
Iteration 1: Write solution -> Run tests -> 2/4 pass
Iteration 2: Fix unicode handling -> Run tests -> 3/4 pass
Iteration 3: Fix empty string case -> Run tests -> 4/4 pass
DONE
```
