---
name: testgen-protocol
description: "Use when solving coding tasks. Generates comprehensive tests via deterministic expansion + subagent review loops."
---

# TestGen Protocol

**Workflow**: ANALYZE → EXPAND → REVIEW (loop) → IMPLEMENT → ITERATE

## Step 1: Analyze Task

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

## Step 2: Generate Test Scaffold

Run the deterministic expander (same input → same output):

```bash
python3 .claude/skills/testgen-protocol/expand_tests.py << 'EOF'
[paste your MATRIX table from Step 1]
EOF
```

Save output to `/app/testgen_tests.py`. Update `SOLUTION_PATH` for your task.

## Step 3: Review with Fresh Subagent (REQUIRED)

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
  1. Missing entity-constraint combinations (every ✓ in matrix needs a test)
  2. Missing edge cases implied by the task
  3. Overly weak assertions (tests that always pass)

  Output ONLY this JSON (no other text):
  {"thorough_enough": true, "gaps": [], "suggestions": []}
  or
  {"thorough_enough": false, "gaps": ["gap1", "gap2"], "suggestions": ["add X", "test Y"]}
```

**If `thorough_enough=false`:**
1. Address the identified gaps
2. Regenerate/update tests
3. Spawn another fresh reviewer
4. Loop until `thorough_enough=true` (max 5 iterations)

## Step 4: Implement Test Logic

Fill in all `# TODO: Implement test logic` placeholders with actual assertions.

## Step 5: Iterate Solution

```
1. Write initial solution
2. Run: pytest /app/testgen_tests.py -v
3. If FAIL: fix solution (not tests!), go to 2
4. If PASS: done
```

## Rules

- NEVER read `/tests/*` or `test_outputs.py` (benchmark files)
- Derive tests from task description ONLY
- Fix the solution, not the tests
