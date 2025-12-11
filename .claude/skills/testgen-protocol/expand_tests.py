#!/usr/bin/env python3
"""
Deterministic test scaffold generator.
Input: Structured markdown (ENTITIES, CONSTRAINTS, MATRIX)
Output: Python test file scaffold

This script is PURELY DETERMINISTIC - no LLM calls.
Same input always produces same output.
"""
import sys
import re


def parse_matrix(text: str) -> list[dict]:
    """Parse markdown table into list of (entity, constraint) pairs."""
    tests = []
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
        '# TODO: Update SOLUTION_PATH for your task',
        'SOLUTION_PATH = "/app/solution"',
        '',
        '@pytest.fixture',
        'def solution():',
        '    """Load the solution file."""',
        '    with open(SOLUTION_PATH) as f:',
        '        return f.read().strip()',
        '',
        '',
        'class TestSolutionExists:',
        '    """Verify solution file was created."""',
        '',
        '    def test_file_exists(self):',
        '        assert os.path.exists(SOLUTION_PATH), f"Solution not found at {SOLUTION_PATH}"',
        '',
        '    def test_file_not_empty(self):',
        '        with open(SOLUTION_PATH) as f:',
        '            content = f.read().strip()',
        '        assert len(content) > 0, "Solution file is empty"',
        '',
    ]

    # Group tests by entity
    by_entity = {}
    for test in tests:
        entity = test['entity']
        if entity not in by_entity:
            by_entity[entity] = []
        by_entity[entity].append(test)

    for entity, entity_tests in by_entity.items():
        class_name = f"Test{''.join(word.title() for word in entity.split('_'))}"
        lines.extend([
            '',
            f'class {class_name}:',
            f'    """Tests for entity: {entity}"""',
            '',
        ])

        for test in entity_tests:
            func_name = f"test_{test['constraint']}"
            lines.extend([
                f'    def {func_name}(self, solution):',
                f'        """Test constraint: {test["constraint"]}"""',
                '        # TODO: Implement test logic',
                '        pass',
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
        print("# Expected format:", file=sys.stderr)
        print("# | Constraint | Entity1 | Entity2 |", file=sys.stderr)
        print("# |------------|---------|---------|", file=sys.stderr)
        print("# | c1         | ✓       | -       |", file=sys.stderr)
        sys.exit(1)
