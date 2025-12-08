# 1318 Test Generation Gap Analysis: rstan-to-pystan

## Task Overview

**Task**: `rstan-to-pystan` - Convert an R/RStan script to Python/PyStan for Bayesian posterior sampling.

**Actual TB2 Tests**: 6 test functions
**Our Generated Tests**: 7 test cases

## Side-by-Side Comparison

| Actual TB2 Tests | Our Generated Tests |
|------------------|---------------------|
| `test_r_rstan_not_installed` (anti-cheat) | - (MISSED) |
| `test_output_files_exist` (existence) | `happy_path_1`, `happy_path_2` (vague overlap) |
| `test_alpha_estimation_accuracy` [1.08-1.1] | `boundary_test_alpha` (min value, no range) |
| `test_sigma_estimation_accuracy` [0.133-0.136] | `boundary_test_sigma` (max value, no range) |
| `test_rho_estimation_accuracy` | - (MISSED) |
| `test_beta_estimation_accuracy` | - (MISSED) |
| - | `edge_case_empty_input` (FALSE POSITIVE) |
| - | `invalid_format_header` (FALSE POSITIVE) |
| - | `invalid_format_data` (FALSE POSITIVE) |

## Analysis

### What We Got Right

1. **Parameter awareness**: We identified `alpha` and `sigma` as important parameters
2. **Output file verification**: We understood files need to be created
3. **Test count**: Similar (7 vs 6)
4. **Category thinking**: We used boundary/edge cases appropriately

### Critical Gaps

#### 1. **Anti-Cheat Test (BIGGEST MISS)**

The actual suite includes `test_r_rstan_not_installed` - verifying that R/RStan is **NOT** installed. This is brilliant because:

- A lazy solution might just keep R and call the original script
- The task requires *converting* to Python, not wrapping R
- This test catches gaming behavior

**Insight**: We need **adversarial reasoning** in our prompt - "What would a lazy/cheating implementation do?"

#### 2. **Missing Parameters (rho, beta)**

Actual tests verify 4 parameters: `alpha`, `sigma`, `rho`, `beta`
We only identified: `alpha`, `sigma`

**Root cause**: We can't see the R script content. The description mentions "posterior sampling" but doesn't enumerate all parameters.

**Insight**: For tasks with external files (R scripts, data files), the description alone isn't enough. We need to either:
- Parse the referenced files for structure hints
- Generate more exploratory tests
- Ask the model to infer likely parameters from domain knowledge

#### 3. **Precision Ranges**

Actual tests have SPECIFIC bounds:
- `alpha`: [1.08, 1.1]
- `sigma`: [0.133, 0.136]

Our tests have GENERIC bounds:
- "Alpha parameter at minimum valid value"
- "Sigma parameter at maximum valid value"

**Insight**: Without domain knowledge or example runs, we can't know the expected numeric ranges. Our tests would fail even if the implementation is correct.

#### 4. **False Positive Tests**

We generated tests that don't exist in the actual suite:
- `edge_case_empty_input` - Empty inputs
- `invalid_format_header` - Malformed R script
- `invalid_format_data` - Out-of-range values

These aren't necessarily wrong - they could be valid tests. But they represent **wasted effort** if the actual benchmark doesn't test for these.

**Insight**: Our model is being conservative and generating "safe" tests. The actual TB2 suite is more focused on correctness verification than robustness testing.

## Scoring

| Metric | Score | Notes |
|--------|-------|-------|
| **Parameter Coverage** | 50% | Got alpha, sigma. Missed rho, beta |
| **Test Type Coverage** | 60% | Got existence, boundary. Missed anti-cheat |
| **Precision Match** | 0% | No specific numeric ranges |
| **False Positives** | 3 tests | Wasted on non-existent checks |
| **Overall Alignment** | ~40% | Partial understanding, major gaps |

## Evolution Recommendations

### Immediate Improvements

1. **Add anti-cheat reasoning to prompt**:
   ```
   Think adversarially: what would a lazy or cheating implementation do?
   What tests would catch someone gaming the benchmark instead of solving it?
   ```

2. **Domain-specific parameter extraction**:
   ```
   For statistical/ML tasks, identify ALL model parameters that should be verified.
   For this Bayesian model, what parameters would posterior sampling produce?
   ```

3. **Reduce generic edge cases**:
   ```
   Focus on correctness tests first. Only add robustness tests (empty input,
   malformed data) if the description specifically mentions error handling.
   ```

### Longer-term Improvements

1. **File content hinting**: If task references external files, try to infer structure:
   - R script → likely parameter names
   - CSV files → column structure
   - Stan models → variable names

2. **Domain knowledge injection**: Add task-type-specific prompts:
   - Statistical tasks → parameter estimation accuracy
   - Conversion tasks → anti-cheat (original tool not used)
   - File format tasks → magic bytes, structure validation

3. **Confidence calibration**: Lower confidence on tests where we're guessing:
   - `boundary_test_alpha` → confidence: 0.3 (don't know actual range)
   - `test_r_not_installed` → would have high confidence IF we thought of it

## Key Learnings

1. **Anti-cheat tests are CRITICAL** for conversion tasks. The actual benchmark is smarter than just "does it produce output" - it verifies the task was done the right way.

2. **Specific precision bounds require domain knowledge or reference runs**. We can't guess that alpha should be [1.08, 1.1].

3. **Our prompt is too generic**. We need task-type-specific reasoning:
   - Conversion tasks → verify original tool NOT used
   - Precision tasks → identify ALL parameters
   - Format tasks → verify exact structure

4. **Quality over quantity**. 7 tests with 40% alignment is worse than 4 tests with 80% alignment.

## Next Steps

1. Run comparison on more tasks to see if these patterns hold
2. Update test generator prompt with anti-cheat reasoning
3. Add task-type detection (conversion, precision, format, etc.)
4. Consider multi-pass generation: first pass for structure, second for precision

---

*Analysis by Claude Opus 4.5, 2025-12-08 13:18 CT*
