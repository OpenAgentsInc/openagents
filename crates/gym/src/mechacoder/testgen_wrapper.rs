//! TestGen Wrapper - Wraps task instructions with TestGen protocol
//!
//! This module wraps task instructions with the TestGen protocol v2:
//! ANALYZE → EXPAND → REVIEW (loop) → IMPLEMENT → ITERATE
//!
//! Key improvements in v2:
//! - Deterministic test scaffold generation via Python script
//! - Fresh-context subagent review loops until thorough
//! - Lean protocol (~70 lines vs ~300 lines before)

/// TestGen instruction wrapper
pub struct TestGenWrapper;

impl TestGenWrapper {
    /// Wrap a task instruction with the TestGen protocol
    ///
    /// This prepends and appends TestGen workflow requirements to the original
    /// task instruction, ensuring Claude follows the ANALYZE → EXPAND → REVIEW
    /// → IMPLEMENT → ITERATE workflow before submitting a solution.
    pub fn wrap_instruction(original: &str) -> String {
        format!(
            r#"{preamble}

## YOUR TASK

{original}

## REQUIRED WORKFLOW: TestGen Protocol v2

Follow this EXACT workflow before submitting your solution:

### Step 1: ANALYZE (Output this section first)

Output a structured analysis in THIS EXACT FORMAT:

```markdown
### ENTITIES
- entity_name: description, format, validation_rules

### CONSTRAINTS
- constraint_id: description, applies_to: [entity1, entity2]

### MATRIX
| Constraint | Entity1 | Entity2 |
|------------|---------|---------|
| c1         | ✓       | ✓       |
```

### Step 2: EXPAND (Run deterministic script)

Run the scaffold generator:

```bash
python3 .claude/skills/testgen-protocol/expand_tests.py << 'EOF'
[paste your MATRIX table from Step 1]
EOF
```

Save output to `/app/testgen_tests.py`. Update `SOLUTION_PATH` for your task.

### Step 3: REVIEW with Fresh Subagent (Loop until approved)

Use the **Task tool** to spawn a fresh-context reviewer:

```
subagent_type: "general-purpose"
prompt: |
  You are a TEST COVERAGE REVIEWER with completely fresh context.

  TASK DESCRIPTION:
  [paste original task]

  ASSUMPTIONS:
  [paste ENTITIES/CONSTRAINTS/MATRIX]

  GENERATED TESTS:
  [paste test scaffold]

  Review for:
  1. Missing entity-constraint combinations
  2. Missing edge cases implied by the task
  3. Overly weak assertions

  Output ONLY this JSON:
  {{"thorough_enough": true/false, "gaps": [...], "suggestions": [...]}}
```

**If `thorough_enough=false`:** Address gaps, update tests, spawn fresh reviewer. Max 5 iterations.

### Step 4: IMPLEMENT

Fill in all `# TODO: Implement test logic` placeholders with actual assertions.

### Step 5: ITERATE

```
1. Write initial solution
2. Run: pytest /app/testgen_tests.py -v
3. If FAIL: fix solution (not tests!), go to 2
4. If PASS: done
```

## CRITICAL RULES

- **NEVER read /tests/*** - Those are for final verification only
- **NEVER read test_outputs.py** - That's benchmark test data (cheating!)
- **Derive tests from description ONLY**
- **Fix the solution, not the tests**

## START NOW

Begin with Step 1: ANALYZE. Output ENTITIES, CONSTRAINTS, and MATRIX before anything else.
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
const TESTGEN_PREAMBLE: &str = r#"# TestGen Protocol Active (v2)

This task requires Test-Driven Development with subagent review loops.

The workflow is: ANALYZE → EXPAND → REVIEW (loop) → IMPLEMENT → ITERATE

Key features of v2:
1. Deterministic test scaffold generation (Python script)
2. Fresh-context subagent reviews (Task tool)
3. Loop until reviewer says "thorough_enough"
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_instruction() {
        let original = "Write a program to validate input.";
        let wrapped = TestGenWrapper::wrap_instruction(original);

        assert!(wrapped.contains("TestGen Protocol Active"));
        assert!(wrapped.contains("Write a program to validate input."));
        assert!(wrapped.contains("ANALYZE"));
        assert!(wrapped.contains("EXPAND"));
        assert!(wrapped.contains("REVIEW with Fresh Subagent"));
        assert!(wrapped.contains("Task tool"));
        assert!(wrapped.contains("thorough_enough"));
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
