# TestGen v2: Deterministic Generation + Subagent Review Loops

## Key Insight

- **Python scripts** = deterministic tools (template expansion, JSON manipulation)
- **Subagent spawning** = via Claude's Task tool (instructed in SKILL.md)
- **Fresh context** = each Task spawn has no prior conversation history

## Architecture

```
.claude/skills/testgen-protocol/
├── SKILL.md                    # Orchestration: tells Claude what to do
├── parse_to_json.py            # Deterministic: structured extraction template
├── expand_tests.py             # Deterministic: JSON → test scaffold
└── schemas/
    └── assumptions.schema.json # JSON schema for validation
```

## How It Works

### SKILL.md Orchestrates Claude

The SKILL.md tells Claude:
1. Parse task description into structured JSON (run `parse_to_json.py`)
2. Expand JSON into test scaffold (run `expand_tests.py`)
3. **Use Task tool** to spawn fresh-context subagent for review
4. Loop until subagent says "thorough_enough"

### Python Scripts Are Deterministic

```python
# parse_to_json.py - Pure parsing, no LLM
# Takes structured markdown output from Claude
# Converts to validated JSON

# expand_tests.py - Pure template expansion
# Takes assumptions.json
# Outputs testgen_tests.py scaffold
```

### Subagent Review Via Task Tool

The SKILL.md instructs Claude to use the Task tool:

```markdown
## Step 3: Review with Fresh Subagent

Use the Task tool with subagent_type="general-purpose" to spawn a reviewer:

Prompt for the subagent:
"You are a test coverage reviewer with COMPLETELY FRESH CONTEXT.
Review these assumptions and tests. Output JSON:
{thorough_enough: bool, gaps: [], suggestions: []}"

If thorough_enough=false, incorporate suggestions and repeat.
Loop until thorough_enough=true (max 5 iterations).
```

## Implementation

### File 1: `SKILL.md` (Lean Orchestrator)

```markdown
---
name: testgen-protocol
description: "Use when solving coding tasks. Generates comprehensive tests via deterministic expansion + subagent review loops."
---

# TestGen Protocol

## Step 1: Analyze Task (Your Output)

Output a structured analysis in THIS EXACT FORMAT:

```
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

Run the deterministic expander:
```bash
python3 .claude/skills/testgen-protocol/expand_tests.py << 'EOF'
[paste your ENTITIES, CONSTRAINTS, MATRIX from Step 1]
EOF
```

This outputs a test scaffold to stdout. Save it to /app/testgen_tests.py

## Step 3: Review with Fresh Subagent (REQUIRED)

Use the Task tool to spawn a fresh-context reviewer:

```
subagent_type: "general-purpose"
prompt: |
  You are a TEST COVERAGE REVIEWER with fresh context.

  Task: [paste original task description]
  Assumptions: [paste ENTITIES/CONSTRAINTS/MATRIX]
  Tests: [paste generated test scaffold]

  Review for:
  1. Missing entity-constraint combinations
  2. Missing edge cases
  3. Overly weak assertions

  Output ONLY valid JSON:
  {"thorough_enough": true/false, "gaps": [...], "suggestions": [...]}
```

If thorough_enough=false:
- Address the gaps
- Regenerate tests
- Spawn another fresh reviewer
- Loop until thorough_enough=true (max 5 iterations)

## Step 4: Implement Test Logic

Fill in TODO placeholders with actual test implementations.

## Step 5: Iterate Solution

Run tests, fix solution, repeat until passing.
```

### File 2: `expand_tests.py` (Deterministic)

```python
#!/usr/bin/env python3
"""
Deterministic test scaffold generator.
Input: Structured markdown (ENTITIES, CONSTRAINTS, MATRIX)
Output: Python test file scaffold
"""
import sys
import re

def parse_matrix(text: str) -> list[dict]:
    """Parse markdown table into list of (entity, constraint) pairs."""
    tests = []
    # Find table rows, extract checkmarks
    lines = text.strip().split('\n')
    headers = []
    for line in lines:
        if '|' in line and 'Constraint' in line:
            # Header row - extract entity names
            parts = [p.strip() for p in line.split('|') if p.strip()]
            headers = parts[1:]  # Skip "Constraint" column
        elif '|' in line and '✓' in line:
            parts = [p.strip() for p in line.split('|') if p.strip()]
            constraint_id = parts[0]
            for i, cell in enumerate(parts[1:]):
                if '✓' in cell and i < len(headers):
                    tests.append({
                        'entity': headers[i].lower().replace(' ', '_'),
                        'constraint': constraint_id.lower().replace(' ', '_'),
                    })
    return tests

def generate_scaffold(tests: list[dict]) -> str:
    """Generate pytest scaffold from test list."""
    lines = [
        '"""TestGen-generated test scaffold."""',
        'import pytest',
        'import re',
        'import os',
        '',
        '# Solution file path',
        'SOLUTION_PATH = "/app/regex.txt"  # Update as needed',
        '',
        '@pytest.fixture',
        'def solution():',
        '    """Load the solution file."""',
        '    with open(SOLUTION_PATH) as f:',
        '        return f.read().strip()',
        '',
    ]

    for test in tests:
        func_name = f"test_{test['entity']}_{test['constraint']}"
        lines.extend([
            f'def {func_name}(solution):',
            f'    """Test {test["entity"]} for constraint: {test["constraint"]}"""',
            f'    # Entity: {test["entity"]}',
            f'    # Constraint: {test["constraint"]}',
            '    # TODO: Implement test logic',
            '    pass',
            '',
        ])

    return '\n'.join(lines)

if __name__ == "__main__":
    text = sys.stdin.read()
    tests = parse_matrix(text)
    if tests:
        print(generate_scaffold(tests))
    else:
        print("# ERROR: Could not parse matrix. Ensure format matches expected structure.", file=sys.stderr)
        sys.exit(1)
```

## Benefits

| Aspect | Before | After |
|--------|--------|-------|
| Test generation | LLM interprets each time | Deterministic from structure |
| Coverage review | Single pass | Multi-iteration with fresh context |
| Prompt length | ~300 lines | ~50 lines |
| Debugging | Opaque LLM decisions | Clear script + subagent boundaries |
| Consistency | Variable | Reproducible given same structure |

## Key Design Decisions

1. **No LLM calls from Python** - Scripts are pure functions
2. **Subagents via Task tool** - Claude spawns them, not scripts
3. **Fresh context** - Each reviewer sees only what's passed explicitly
4. **Loop until approved** - Not fixed iterations, but capped at 5
5. **Structured intermediate format** - ENTITIES/CONSTRAINTS/MATRIX in markdown

## Files to Create

| File | Lines | Purpose |
|------|-------|---------|
| `.claude/skills/testgen-protocol/SKILL.md` | ~50 | Orchestration |
| `.claude/skills/testgen-protocol/expand_tests.py` | ~60 | Deterministic expansion |

## Files to Modify

| File | Change |
|------|--------|
| `.claude/skills/testgen-protocol/SKILL.md` | Replace current 300-line version |

## Success Criteria

1. Test scaffold generation is deterministic (same input → same output)
2. Fresh subagent catches coverage gaps that main agent missed
3. Converges to "thorough_enough" within 5 iterations
4. SKILL.md under 60 lines


