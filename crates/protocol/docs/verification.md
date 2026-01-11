# Verification Modes

This document explains the verification system in the protocol crate, including objective vs subjective modes, redundancy, and adjudication strategies.

## Overview

Not all job results can be verified the same way:

| Job Type | Output | Verification |
|----------|--------|--------------|
| Run tests | Pass/Fail | **Objective** - deterministic |
| Analyze code | Summary | **Subjective** - requires judgment |
| Build project | Exit code | **Objective** - deterministic |
| Rerank results | Ranking | **Subjective** - requires judgment |

The protocol handles this with verification modes and adjudication strategies.

## Verification Modes

### Objective

Results can be verified deterministically. Given the same input, all correct providers should produce the same output.

**Characteristics:**
- Deterministic output (exit codes, hashes, pass/fail)
- Single provider sufficient (redundancy = 1)
- No adjudication needed
- Reproducible across runs

**Examples:**
- Test execution (pass/fail)
- Build status (success/failure)
- Linting results (errors/warnings)
- Hash computation

```rust
use protocol::verification::Verification;

let verification = Verification::objective();
assert_eq!(verification.redundancy, 1);
```

### Subjective

Results require judgment to verify. Different providers may produce different valid outputs.

**Characteristics:**
- Non-deterministic output (summaries, analysis, rankings)
- Multiple providers recommended (redundancy >= 2)
- Adjudication needed to select/combine results
- Quality varies between providers

**Examples:**
- Code summaries
- Bug hypothesis generation
- Document ranking
- Code review comments

```rust
let verification = Verification::subjective_with_judge(2);
assert_eq!(verification.redundancy, 2);
```

## Redundancy

Redundancy specifies how many providers should execute the same job.

| Redundancy | Use Case |
|------------|----------|
| 1 | Objective jobs, trusted providers |
| 2 | Subjective jobs, quality assurance |
| 3+ | High-stakes decisions, consensus |

```rust
// Single provider for objective work
let v = Verification::objective(); // redundancy = 1

// Two providers for subjective work
let v = Verification::subjective_with_judge(2); // redundancy = 2

// Three providers for critical decisions
let v = Verification::subjective_with_majority(3); // redundancy = 3
```

## Adjudication Strategies

When multiple providers return results, adjudication determines the final output.

### None

Use the first valid result. Appropriate for objective jobs or when redundancy = 1.

```rust
use protocol::verification::AdjudicationStrategy;

let v = Verification {
    mode: VerificationMode::Objective,
    redundancy: 1,
    adjudication: AdjudicationStrategy::None,
    judge_model: None,
};
```

### MajorityVote

Select the result that most providers agree on. Best for categorical outputs.

**How it works:**
1. Collect all provider results
2. Group by similarity/equality
3. Select the group with most members
4. Return representative result

**Best for:**
- Classification tasks
- Yes/no decisions
- Ranking positions
- Discrete choices

```rust
let v = Verification::subjective_with_majority(3);
// With 3 providers, majority = 2 agreeing
```

### JudgeModel

Use a separate model to evaluate and select the best result. Best for complex outputs where quality is subjective.

**How it works:**
1. Collect all provider results
2. Send results to judge model with evaluation criteria
3. Judge selects best result (or synthesizes from multiple)
4. Return judge's selection

**Best for:**
- Code summaries (quality varies)
- Analysis reports (depth varies)
- Hypothesis generation (creativity varies)

```rust
let v = Verification::subjective_with_judge(2)
    .with_judge_model("codex-3-opus");
```

### Merge

Combine results by aggregating. Best for cumulative outputs.

**How it works:**
1. Collect all provider results
2. Merge/aggregate results (union, intersection, etc.)
3. Deduplicate if necessary
4. Return combined result

**Best for:**
- Symbol extraction (union of symbols)
- Bug finding (union of bugs)
- Candidate lists (combined candidates)

```rust
let v = Verification::subjective_with_merge(2);
```

## Default Verification by Job Type

| Job Type | Mode | Redundancy | Adjudication |
|----------|------|------------|--------------|
| `oa.code_chunk_analysis.v1` | Subjective | 2 | JudgeModel |
| `oa.retrieval_rerank.v1` | Subjective | 2 | MajorityVote |
| `oa.sandbox_run.v1` | Objective | 1 | None |

## API Reference

### Creating Verification Configs

```rust
use protocol::verification::{Verification, VerificationMode, AdjudicationStrategy};

// Objective (default)
let v = Verification::objective();

// Subjective with majority vote
let v = Verification::subjective_with_majority(2);

// Subjective with judge model
let v = Verification::subjective_with_judge(2);

// Subjective with merge
let v = Verification::subjective_with_merge(2);

// With specific judge model
let v = Verification::subjective_with_judge(2)
    .with_judge_model("gpt-4");

// Custom configuration
let v = Verification {
    mode: VerificationMode::Subjective,
    redundancy: 3,
    adjudication: AdjudicationStrategy::MajorityVote,
    judge_model: None,
};
```

### Checking Verification Settings

```rust
use protocol::jobs::JobRequest;

let request = ChunkAnalysisRequest::default();
let verification = request.verification();

match verification.mode {
    VerificationMode::Objective => {
        // Single provider, deterministic
    }
    VerificationMode::Subjective => {
        // Multiple providers, needs adjudication
        println!("Redundancy: {}", verification.redundancy);
        println!("Adjudication: {:?}", verification.adjudication);
    }
}
```

## Serialization

Verification configs serialize with snake_case:

```json
{
  "mode": "subjective",
  "redundancy": 2,
  "adjudication": "judge_model",
  "judge_model": "codex-3-opus"
}
```

## Implementation Guide

### For Job Submitters

1. Use default verification for standard use cases
2. Increase redundancy for critical decisions
3. Consider cost vs quality tradeoffs

```rust
// Standard quality
let request = ChunkAnalysisRequest {
    verification: Verification::subjective_with_judge(2),
    ..Default::default()
};

// Higher quality, higher cost
let request = ChunkAnalysisRequest {
    verification: Verification::subjective_with_judge(3)
        .with_judge_model("codex-3-opus"),
    ..Default::default()
};

// Lower cost, acceptable for non-critical
let request = ChunkAnalysisRequest {
    verification: Verification {
        mode: VerificationMode::Subjective,
        redundancy: 1,
        adjudication: AdjudicationStrategy::None,
        judge_model: None,
    },
    ..Default::default()
};
```

### For Orchestrators

1. Route jobs to `redundancy` providers
2. Collect all responses
3. Apply adjudication strategy
4. Return final result with combined provenance

```rust
async fn execute_with_verification(
    request: &ChunkAnalysisRequest,
    providers: &[Provider],
) -> Result<ChunkAnalysisResponse> {
    let verification = request.verification();

    // Select providers based on redundancy
    let selected = select_providers(providers, verification.redundancy as usize);

    // Execute on all selected providers
    let results: Vec<ChunkAnalysisResponse> =
        futures::future::join_all(
            selected.iter().map(|p| p.execute(request))
        ).await
        .into_iter()
        .filter_map(|r| r.ok())
        .collect();

    // Adjudicate
    match verification.adjudication {
        AdjudicationStrategy::None => results.into_iter().next().unwrap(),
        AdjudicationStrategy::MajorityVote => majority_vote(results),
        AdjudicationStrategy::JudgeModel => {
            let judge = verification.judge_model.as_deref().unwrap_or("default");
            judge_select(results, judge).await
        }
        AdjudicationStrategy::Merge => merge_results(results),
    }
}
```

## Best Practices

1. **Use objective for deterministic tasks**: Tests, builds, linting
2. **Use subjective with judge for quality-sensitive tasks**: Summaries, analysis
3. **Use majority vote for categorical decisions**: Rankings, classifications
4. **Increase redundancy for critical decisions**: Security analysis, bug finding
5. **Consider cost**: More redundancy = higher cost but better quality
