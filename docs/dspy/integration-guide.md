# DSPy Integration Guide

How to add DSPy signatures to new components in the OpenAgents codebase.

## Overview

This guide walks through the process of:
1. Identifying decision points that benefit from DSPy
2. Creating signatures with the `#[Signature]` macro
3. Wiring signatures to the self-improvement loop
4. Adding training data collection
5. Integrating with the auto-optimizer

## When to Use DSPy

Use DSPy signatures when you have:

| Pattern | Example | Why DSPy Helps |
|---------|---------|----------------|
| **Classification** | Is this task simple or complex? | Learn subtle distinctions from examples |
| **Routing** | Which tool should handle this? | Discover non-obvious routing rules |
| **Extraction** | Parse status from unstructured text | Handle edge cases automatically |
| **Generation** | Create a plan for this task | Optimize prompts for your domain |
| **Validation** | Is this path safe? | Learn patterns from feedback |

**Don't use DSPy for:**
- Pure computation (math, string manipulation)
- Deterministic lookups (database queries, config reads)
- Simple if-else with known conditions

## Step 1: Identify the Decision Point

Look for code that:
- Uses hardcoded prompts
- Has keyword-based heuristics
- Parses unstructured LLM output
- Makes routing decisions based on content

**Example: Before DSPy**
```rust
fn is_complex_task(description: &str, file_count: u32) -> bool {
    // Hardcoded keyword heuristics
    let complex_keywords = ["refactor", "architecture", "migration"];
    let has_keywords = complex_keywords.iter().any(|k| description.contains(k));
    has_keywords || file_count > 10 || description.len() > 500
}
```

## Step 2: Create the Signature

Add `dsrs` to your crate's dependencies:
```toml
# Cargo.toml
[dependencies]
dsrs = { path = "../dsrs" }
```

Create a signature:
```rust
use dsrs::Signature;

#[Signature]
struct TaskComplexityClassifier {
    /// Classify the complexity of a coding task.
    /// Consider: number of files affected, scope of changes, architectural impact.
    /// Output confidence as a probability (0.0 to 1.0).

    #[input] pub task_description: String,
    #[input] pub file_count: u32,
    #[input] pub codebase_context: String,

    #[output] pub complexity: String,     // Simple/Moderate/Complex/VeryComplex
    #[output] pub reasoning: String,
    #[output] pub confidence: f32,
}
```

### Signature Design Rules

1. **Docstring = System Prompt**: The struct docstring becomes the LLM instruction
2. **Field Names Matter**: `task_description` is clearer than `desc`
3. **Always Include Confidence**: Enables fallback to legacy when unsure
4. **Use String for JSON**: Complex outputs should be JSON strings, validated at runtime

## Step 3: Create a Pipeline

Wrap the signature in a pipeline struct:
```rust
use dsrs::{Predict, Example, example};

pub struct TaskComplexityPipeline {
    predictor: Predict<TaskComplexityClassifier>,
}

impl TaskComplexityPipeline {
    pub fn new() -> Self {
        Self {
            predictor: Predict::new(),
        }
    }

    pub async fn classify(
        &self,
        task_description: &str,
        file_count: u32,
        codebase_context: &str,
    ) -> Result<(String, f32), anyhow::Error> {
        let inputs = example! {
            task_description: task_description.to_string(),
            file_count: file_count,
            codebase_context: codebase_context.to_string(),
        };

        let prediction = self.predictor.forward(inputs).await?;

        let complexity = prediction.get::<String>("complexity")?;
        let confidence = prediction.get::<f32>("confidence")?;

        Ok((complexity, confidence))
    }
}
```

## Step 4: Add Fallback Logic

Always provide a fallback when confidence is low:
```rust
const CONFIDENCE_THRESHOLD: f32 = 0.7;

pub async fn is_complex_task(
    pipeline: &TaskComplexityPipeline,
    description: &str,
    file_count: u32,
    context: &str,
) -> bool {
    // Try DSPy first
    match pipeline.classify(description, file_count, context).await {
        Ok((complexity, confidence)) if confidence >= CONFIDENCE_THRESHOLD => {
            // Use DSPy result
            matches!(complexity.as_str(), "Complex" | "VeryComplex")
        }
        _ => {
            // Fallback to legacy heuristics
            legacy_is_complex_task(description, file_count)
        }
    }
}

fn legacy_is_complex_task(description: &str, file_count: u32) -> bool {
    let complex_keywords = ["refactor", "architecture", "migration"];
    let has_keywords = complex_keywords.iter().any(|k| description.contains(k));
    has_keywords || file_count > 10 || description.len() > 500
}
```

## Step 5: Add Training Data Collection

Create a training example struct:
```rust
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityTrainingExample {
    pub timestamp: DateTime<Utc>,
    pub task_description: String,
    pub file_count: u32,
    pub codebase_context: String,
    pub predicted_complexity: String,
    pub confidence: f32,
    pub actual_outcome: Option<bool>,  // Was the prediction correct?
}
```

Collect training data after successful executions:
```rust
use crate::dspy::TrainingCollector;

pub async fn execute_with_training(
    pipeline: &TaskComplexityPipeline,
    collector: &mut TrainingCollector,
    task: &Task,
) -> Result<TaskResult, Error> {
    // Get prediction
    let (complexity, confidence) = pipeline.classify(
        &task.description,
        task.file_count,
        &task.context,
    ).await?;

    // Execute task...
    let result = execute_task(task).await?;

    // Collect training data if high confidence
    if confidence >= CONFIDENCE_THRESHOLD {
        let example = ComplexityTrainingExample {
            timestamp: Utc::now(),
            task_description: task.description.clone(),
            file_count: task.file_count,
            codebase_context: task.context.clone(),
            predicted_complexity: complexity,
            confidence,
            actual_outcome: Some(result.success),
        };
        collector.add_complexity_example(example)?;
    }

    Ok(result)
}
```

## Step 6: Wire to Self-Improvement Loop

Integrate with the performance tracker:
```rust
use crate::dspy::{PerformanceTracker, OutcomeFeedback};

pub async fn complete_session(
    session_id: &str,
    outcome: SessionOutcome,
    tracker: &mut PerformanceTracker,
) -> Result<(), Error> {
    // Record outcome
    let was_correct = matches!(outcome, SessionOutcome::Success);
    tracker.record_outcome("task_complexity", was_correct);

    // Label training examples
    let feedback = OutcomeFeedback::new();
    feedback.label_session_decisions(session_id, outcome).await?;

    // Save metrics
    tracker.save()?;

    Ok(())
}
```

## Step 7: Configure Auto-Optimization

Add your signature to the auto-optimizer:
```rust
use crate::dspy::{AutoOptimizer, AutoOptimizerConfig};

pub fn configure_auto_optimizer() -> AutoOptimizerConfig {
    AutoOptimizerConfig {
        enabled: true,
        min_examples: 20,
        accuracy_threshold: 0.7,
        optimization_interval_hours: 24,
        signatures_to_optimize: vec![
            "task_complexity".to_string(),
            "delegation".to_string(),
            "rlm_trigger".to_string(),
        ],
    }
}
```

## Complete Example: Adding DSPy to a New Crate

### File Structure
```
crates/my-crate/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── dspy/
    │   ├── mod.rs
    │   ├── signatures.rs
    │   ├── pipelines.rs
    │   └── training.rs
    └── ...
```

### Cargo.toml
```toml
[dependencies]
dsrs = { path = "../dsrs" }
serde = { version = "1", features = ["derive"] }
chrono = { version = "0.4", features = ["serde"] }
anyhow = "1"
```

### src/dspy/mod.rs
```rust
mod signatures;
mod pipelines;
mod training;

pub use signatures::*;
pub use pipelines::*;
pub use training::*;
```

### src/dspy/signatures.rs
```rust
use dsrs::Signature;

#[Signature]
pub struct MyDecisionSignature {
    /// Make a decision about X.
    /// Consider factors A, B, and C.

    #[input] pub input_a: String,
    #[input] pub input_b: u32,
    #[output] pub decision: String,
    #[output] pub reasoning: String,
    #[output] pub confidence: f32,
}
```

### src/dspy/pipelines.rs
```rust
use dsrs::{Predict, Example, example};
use super::MyDecisionSignature;

pub struct MyDecisionPipeline {
    predictor: Predict<MyDecisionSignature>,
}

impl MyDecisionPipeline {
    pub fn new() -> Self {
        Self {
            predictor: Predict::new(),
        }
    }

    pub async fn decide(
        &self,
        input_a: &str,
        input_b: u32,
    ) -> Result<(String, f32), anyhow::Error> {
        let inputs = example! {
            input_a: input_a.to_string(),
            input_b: input_b,
        };

        let prediction = self.predictor.forward(inputs).await?;

        let decision = prediction.get::<String>("decision")?;
        let confidence = prediction.get::<f32>("confidence")?;

        Ok((decision, confidence))
    }
}
```

### src/dspy/training.rs
```rust
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyDecisionTrainingExample {
    pub timestamp: DateTime<Utc>,
    pub input_a: String,
    pub input_b: u32,
    pub decision: String,
    pub confidence: f32,
    pub was_correct: Option<bool>,
}
```

## Metrics and Monitoring

### Performance Tracking
```rust
// View signature accuracy
let tracker = PerformanceTracker::open()?;
let summary = tracker.summary();
println!("Task Complexity Accuracy: {:.1}%", summary.complexity_accuracy * 100.0);
```

### CLI Commands
```bash
# View DSPy performance
autopilot dspy performance

# View failed sessions for analysis
autopilot dspy sessions --failed

# Manually trigger optimization
autopilot dspy optimize --signature task_complexity
```

## Checklist

- [ ] Created signature with `#[Signature]` macro
- [ ] Docstring clearly describes the decision
- [ ] Field names are descriptive
- [ ] Confidence field included
- [ ] Pipeline wraps signature with `Predict`
- [ ] Fallback logic for low confidence
- [ ] Training data collection implemented
- [ ] Connected to performance tracker
- [ ] Added to auto-optimizer config
- [ ] Signature documented in [signatures-catalog.md](./signatures-catalog.md)

## Related Documentation

- [README.md](./README.md) — DSPy strategy overview
- [signatures-catalog.md](./signatures-catalog.md) — Complete signature inventory
- [rust.md](./rust.md) — dsrs usage guide
- [crates/dsrs/docs/](../../crates/dsrs/docs/) — dsrs implementation docs
