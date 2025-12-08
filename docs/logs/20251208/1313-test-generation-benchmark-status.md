# 1313 Test Generation Benchmark Status

## Overview

We've built a miniature benchmark system for iterating on test generation quality. The goal: generate test suites from task descriptions that match or exceed the quality of actual TB2 tests, **without ever seeing those tests**.

## Components Built

### 1. Test Generator (`src/hillclimber/test-generator.ts`)

Generates test cases from task descriptions using LLMs.

**Features:**
- Supports Claude (Anthropic API) and local FM (Apple Foundation Models)
- **Guided generation** for local FM - guarantees valid JSON structure
- Categorizes tests: existence, format, happy_path, boundary, edge_case, invalid_input, integration
- Outputs requirements, assumptions, uncertainties alongside tests

**Usage:**
```bash
bun run src/hillclimber/test-gen-cli.ts --task regex-log --model local
```

### 2. Test Evaluator (`src/hillclimber/test-gen-evaluator.ts`)

Compares generated tests to actual TB2 tests to measure quality.

**Metrics:**
- **Coverage**: % of actual test categories covered
- **Accuracy**: % of generated tests with correct expectations
- **Edge Case Detection**: % of hard cases anticipated
- **Category Balance**: Deviation from ideal distribution
- **Match Types**: exact, partial, category_only, no_match

### 3. Benchmark CLI (`src/hillclimber/test-gen-benchmark.ts`)

Runs test generation across multiple tasks and evaluates quality.

**Usage:**
```bash
# Single task
bun run src/hillclimber/test-gen-benchmark.ts --tasks regex-log --compare --model local

# Multiple tasks
bun run src/hillclimber/test-gen-benchmark.ts --sample 5 --model local

# All tasks (slow)
bun run src/hillclimber/test-gen-benchmark.ts --all --json > results.json
```

### 4. Guided Generation (NEW)

Major improvement: Apple's `@Generable` macros constrain model output.

**Swift types in `GuidedTypes.swift`:**
```swift
@Generable(description: "A test case")
struct GeneratedTest: Codable {
    var id: String
    var input: String
    var expectedOutput: String?
    var reasoning: String

    @Guide(description: "Category", .anyOf([...]))
    var category: String

    @Guide(description: "Confidence", .range(0.0...1.0))
    var confidence: Float
}
```

## Performance Results

### Before Guided Generation (unguided)

```
Task: regex-log
Duration: ~93 seconds
Tests: 23
Categories: happy_path:23 (ALL SAME - broken!)
JSON validity: ~90% (sometimes malformed)
```

### After Guided Generation

```
Task: regex-log
Duration: ~18-22 seconds (4x FASTER)
Tests: 9-10
Categories: existence:1, happy_path:2, boundary:2, edge_case:2, invalid_input:3
JSON validity: 100% (GUARANTEED)
```

| Metric | Unguided | Guided | Improvement |
|--------|----------|--------|-------------|
| Duration | 93s | 18-22s | **4x faster** |
| Category accuracy | 0% | 100% | Fixed taxonomy |
| JSON validity | ~90% | 100% | No retries |
| Confidence bounds | Sometimes >1 | Always 0-1 | Enforced range |

## Current Benchmark Results

Latest run on `regex-log`:

```
=== Generation Complete ===
Model: local-fm-guided
Duration: 18671ms
Tests generated: 9
Categories: happy_path:2, boundary:2, edge_case:2, invalid_input:3

--- Requirements Identified ---
  - Regex must correctly match dates in YYYY-MM-DD format in lines with IPv4 addresses.
  - Valid dates must be matched, ignoring any preceding or following alphanumeric characters.
  - Regex must handle multiple dates per line by matching only the last date.

--- Generated Tests ---
[happy_path] happy_path_1: "192.168.1.1 - 2023-02-29: Log entry 1" → ['2023-02-29']
[happy_path] happy_path_2: "192.168.1.2 - 2023-03-01, 2023-03-02: Log entry 2" → ['2023-03-02']
[boundary] boundary_min: "192.168.1.3 - 2021-01-01: Log entry 3" → ['2021-01-01']
[boundary] boundary_max: "192.168.1.4 - 2030-12-31: Log entry 4" → ['2030-12-31']
[edge_case] edge_case_single: Tests single date without alphanumeric neighbors
[edge_case] edge_case_multiple: Tests multiple dates, ensures last is matched
[invalid_input] invalid_format_1: Alphanumeric before date → null
[invalid_input] invalid_format_2: Alphanumeric after date → null
[invalid_input] invalid_input_empty: Empty line → null
```

## Known Issues

### 1. Test Extractor Parser Not Working

The evaluator's `extractActualTests()` function failed to parse `regex-log` tests:
```
Extraction errors: Could not extract test_lines or expected_dates from regex-log
```

**Root cause**: Parser assumes a specific format that doesn't match actual TB2 test file structure.

**TODO**: Examine actual TB2 test files at `/Users/christopherdavid/code/terminal-bench-2/{task}/tests/test_outputs.py` and update parser.

### 2. Test Count Below Minimum

Requested 15-30 tests, generated only 9-10. The model is being conservative. May need to:
- Adjust prompt to be more explicit about minimum count
- Add examples of diverse test cases
- Increase temperature slightly

### 3. Some Test Expectations May Be Wrong

Example: `2023-02-29` is invalid (2023 is not a leap year). The model generated this as a valid test case. Need better date validation awareness in the prompt.

## Files Created

| File | Purpose |
|------|---------|
| `src/hillclimber/test-generator.ts` | Core test generation |
| `src/hillclimber/test-gen-cli.ts` | Single-task CLI |
| `src/hillclimber/test-gen-evaluator.ts` | Quality scoring |
| `src/hillclimber/test-gen-benchmark.ts` | Multi-task benchmark |
| `swift/.../GuidedTypes.swift` | Generable schema types |
| `docs/foundation-models/guided-generation.md` | Documentation |

## Next Steps

1. **Fix test extractor** - Parse actual TB2 test files correctly
2. **Run full benchmark** - Evaluate all 89 TB2 tasks
3. **Iterate on prompt** - Improve category coverage and test quality
4. **Add environment context** - Include Python/boundaries/determinism hints
5. **Build self-verifier** - Run generated tests against implementations
6. **Build blind verifier** - Final pass/fail only verification

## Architecture Reminder

```
┌─────────────────────────────────────────────────────────────────┐
│                    Information Boundary                          │
│                                                                  │
│  INSIDE (agent can see):          OUTSIDE (agent CANNOT see):   │
│  - Task description               - Actual TB2 test cases       │
│  - Self-generated tests           - Expected values from TB2    │
│  - Self-test results              - Specific failure details    │
│                                                                  │
│  Until final verification: ONLY pass/fail                       │
└─────────────────────────────────────────────────────────────────┘
```

The benchmark helps us iterate on test generation quality so that self-generated tests cover similar ground to real tests - proving the agent truly understands requirements rather than gaming known answers.

---

*Status log by Claude Opus 4.5, 2025-12-08 13:13 CT*
