# 1256 Blind Verification Implementation Log

## Session Overview

This session continued implementing the **HillClimber v3 Blind Verification Architecture** - a redesign to prove that architecture + local inference > model scale by ensuring the agent is truly blind to benchmark test cases.

## What Was Built

### 1. Test Generator (`src/hillclimber/test-generator.ts`)

A module that generates test cases from task descriptions WITHOUT seeing actual benchmark tests.

**Key interfaces:**
```typescript
export type TestCategory =
  | "existence"      // File/output created
  | "format"         // Structure valid
  | "happy_path"     // Basic correct behavior
  | "boundary"       // Min/max limits
  | "edge_case"      // Tricky scenarios
  | "invalid_input"  // Should fail/reject
  | "integration";   // System-level

export interface GeneratedTest {
  id: string;
  input: string;
  expectedOutput: string | null;
  reasoning: string;
  category: TestCategory;
  confidence: number;  // 0-1
}

export interface TestGenerationResult {
  tests: GeneratedTest[];
  requirements: string[];
  assumptions: string[];
  uncertainties: string[];
  model: string;
  durationMs: number;
}
```

**Key functions:**
- `generateTestsFromDescription(taskDescription, taskId, options)` - Main entry point
- `generateTestsWithClaude(...)` - Uses Anthropic API
- `generateTestsWithLocalFM(...)` - Uses Apple Foundation Models
- `summarizeCategories(tests)` - Debugging helper

### 2. Test Generator CLI (`src/hillclimber/test-gen-cli.ts`)

Standalone CLI to test the generator:

```bash
# Basic usage
bun run src/hillclimber/test-gen-cli.ts --task regex-log

# With local FM
bun run src/hillclimber/test-gen-cli.ts --task regex-log --model local

# Verbose mode
bun run src/hillclimber/test-gen-cli.ts --task regex-log --verbose
```

### 3. Test Generation Evaluator (`src/hillclimber/test-gen-evaluator.ts`)

Compares generated tests to actual TB2 tests to measure quality.

**Metrics calculated:**
- **Coverage**: % of actual test categories covered
- **Accuracy**: % of generated tests with correct expectations
- **Edge Case Detection**: % of hard cases anticipated
- **Category Balance**: Deviation from ideal distribution

**Key functions:**
- `extractActualTests(testFilePath, taskId)` - Parses TB2 test_outputs.py
- `compareTests(generated, actual)` - Computes quality score
- `evaluateTestGeneration(taskId, generated, tb2Path)` - Main API

### 4. Test Generation Benchmark CLI (`src/hillclimber/test-gen-benchmark.ts`)

Runs test generation across multiple tasks and evaluates quality:

```bash
# Single task with comparison
bun run src/hillclimber/test-gen-benchmark.ts --tasks regex-log --compare

# Random sample of tasks
bun run src/hillclimber/test-gen-benchmark.ts --sample 5 --model local

# All tasks (warning: slow with local FM)
bun run src/hillclimber/test-gen-benchmark.ts --all --json > results.json
```

## Findings and Learnings

### 1. Local FM Test Generation Quality Issues

When running test generation with local FM on `regex-log`:
- Generated 23 tests in ~93 seconds
- **All tests categorized as "happy_path"** - model isn't using the category taxonomy correctly
- Some expectations are incorrect (e.g., expecting null for dates in 2022/2024)
- Model tends to over-generate similar test cases

### 2. Test Extraction Parser Issues

The TB2 test file parser (`extractActualTests`) failed to parse `regex-log` tests:
```
Extraction errors: Could not extract test_lines or expected_dates from regex-log
```

**TODO**: The parser assumes a specific format (`test_lines = [...]`, `expected_dates = [...]`) that doesn't match the actual test file structure. Need to examine real TB2 test files and update parser.

### 3. Information Boundary Design

The user confirmed the absolute boundary rule:
> "No data should be leaking across the boundary from benchmark into runtime. None. Until at the very end when we run the final verification."

This means:
- Self-generated tests: FULL access to inputs, expected values, actual results
- Real benchmark tests: ONLY final pass/fail at the very end
- No progress hints, no category feedback, no partial scores

### 4. Test Category Distribution (Ideal)

From TB2 analysis, the ideal test distribution:
- existence: 5%
- format: 10%
- happy_path: 25%
- boundary: 20%
- edge_case: 25%
- invalid_input: 10%
- integration: 5%

### 5. Model Comparison Strategy

The plan is to try both Claude and local FM for test generation:
- Track which model produces better test coverage
- Compare overlap with real tests
- Measure quality metrics per model

## Files Created This Session

| File | Purpose |
|------|---------|
| `src/hillclimber/test-generator.ts` | Core test generation from description |
| `src/hillclimber/test-gen-cli.ts` | CLI for single-task testing |
| `src/hillclimber/test-gen-evaluator.ts` | Compare generated vs actual tests |
| `src/hillclimber/test-gen-benchmark.ts` | Multi-task benchmark runner |

## Pending Work

1. **Fix test extraction parser** - Update `extractActualTests` to handle actual TB2 test file format
2. **Add environment context** - Include Python codebase info, boundaries, determinism hints in prompts
3. **Create self-verifier** - Run agent's own tests with full feedback
4. **Create regex test runner** - Execute regex tests programmatically
5. **Create blind verifier** - Run real tests with pass/fail only
6. **Create blind orchestrator** - Wire everything together
7. **Run end-to-end test** - Full blind mode on regex-log

## Instructions for Future Agents

### Running Test Generation

```bash
# Test single task with local FM (no API key needed)
bun run src/hillclimber/test-gen-cli.ts --task regex-log --model local --verbose

# Test with Claude (requires ANTHROPIC_API_KEY)
bun run src/hillclimber/test-gen-cli.ts --task regex-log --model claude
```

### Running Benchmark

```bash
# Single task comparison
bun run src/hillclimber/test-gen-benchmark.ts --tasks regex-log --compare --model local

# Multiple tasks
bun run src/hillclimber/test-gen-benchmark.ts --tasks regex-log,path-tracing --model local

# Random sample
bun run src/hillclimber/test-gen-benchmark.ts --sample 5 --model local
```

### Key Design Principles

1. **Agent must be BLIND to real tests** - Never let test expected values leak into the runtime
2. **Self-tests are fully visible** - Agent can see and iterate on its own generated tests
3. **Final verification is pass/fail only** - No detailed feedback from real benchmark

### Improving Test Generation

To improve the quality of generated tests:

1. **Fix category classification** - The model is putting everything in "happy_path"
2. **Add environment context** - Tell the model about Python, test framework, boundaries
3. **Improve prompt** - Add more specific examples of each category
4. **Fix test extractor** - Parser needs to match actual TB2 test file format

### Related Documentation

- `/Users/christopherdavid/.claude/plans/proud-marinating-globe.md` - Full plan
- `docs/logs/20251208/1219-benchmark-gaming-analysis.md` - Analysis of benchmark gaming
- `docs/logs/20251208/1224-blind-verification-redesign.md` - Architecture redesign

---

*Log by Claude Opus 4.5, 2025-12-08*
