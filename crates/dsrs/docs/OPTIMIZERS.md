# Optimizers

Learning without training models.

## Overview

An **Optimizer** improves your DSPy program automatically by:

- Trying different prompt strategies
- Adjusting intermediate reasoning
- Selecting better decompositions

This is **ML-style optimization**, but applied to *program structure*, not weights. No fine-tuning required.

> **Mental model:** "The system learns how to ask better questions."

---

## Optimizer Trait

```rust
// File: crates/dsrs/src/optimizer/mod.rs

#[allow(async_fn_in_trait)]
pub trait Optimizer {
    /// Optimize the module using training data.
    async fn compile<M>(
        &self,
        module: &mut M,
        trainset: Vec<Example>,
    ) -> Result<()>
    where
        M: Module + Optimizable + Evaluator;
}
```

**Requirements for Module:**
- `Module` - Can execute forward pass
- `Optimizable` - Exposes parameters for optimization
- `Evaluator` - Provides metric for scoring

---

## MIPROv2

**Multi-prompt Instruction Proposal Optimizer v2** - An advanced optimizer using LLMs to generate and refine prompts through a three-stage process.

```rust
// File: crates/dsrs/src/optimizer/mipro.rs

use dsrs::optimizer::MIPROv2;

let optimizer = MIPROv2::builder()
    .num_candidates(10)
    .max_bootstrapped_demos(3)
    .max_labeled_demos(3)
    .num_trials(20)
    .temperature(1.0)
    .track_stats(true)
    .build();

optimizer.compile(&mut module, trainset).await?;
```

### Three-Stage Process

```
┌─────────────────────────────────────────────────────────────┐
│                      Stage 1: Trace Generation              │
├─────────────────────────────────────────────────────────────┤
│ 1. Run module on training examples                          │
│ 2. Capture execution traces (inputs, outputs, scores)       │
│ 3. Build understanding of task patterns                     │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│                    Stage 2: Prompt Generation               │
├─────────────────────────────────────────────────────────────┤
│ 1. Generate program description from traces                 │
│ 2. Apply 15 prompting tips as guidance                      │
│ 3. Create candidate instructions using LLM                  │
│ 4. Pair instructions with demo examples                     │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│                 Stage 3: Evaluation & Selection             │
├─────────────────────────────────────────────────────────────┤
│ 1. Evaluate candidates in minibatches                       │
│ 2. Score using module's Evaluator metric                    │
│ 3. Select best instruction + demos combination              │
│ 4. Update module signature                                  │
└─────────────────────────────────────────────────────────────┘
```

### Configuration

| Parameter | Default | Description |
|-----------|---------|-------------|
| `num_candidates` | 10 | Number of instruction candidates to generate |
| `max_bootstrapped_demos` | 3 | Max demos from successful traces |
| `max_labeled_demos` | 3 | Max demos from labeled training data |
| `num_trials` | 20 | Number of evaluation trials |
| `minibatch_size` | 25 | Examples per evaluation batch |
| `temperature` | 1.0 | LLM temperature for generation |
| `track_stats` | false | Enable statistics tracking |

### Key Structures

```rust
/// Execution trace from a single forward pass.
pub struct Trace {
    pub inputs: Example,
    pub outputs: Prediction,
    pub score: Option<f32>,
}

/// Candidate prompt for evaluation.
pub struct PromptCandidate {
    pub instruction: String,
    pub demos: Vec<Example>,
}
```

### Prompting Tips

MIPROv2 uses a library of 15 default prompting best practices:

1. Break complex tasks into smaller steps
2. Use specific, unambiguous language
3. Provide relevant context
4. Specify output format explicitly
5. Use examples to demonstrate expected behavior
6. Ask for step-by-step reasoning (chain-of-thought)
7. Validate inputs before processing
8. Handle edge cases explicitly
9. Use consistent terminology
10. Request confidence scores
11. Avoid leading questions
12. Be explicit about constraints
13. Use structured output formats
14. Request explanations for decisions
15. Include error handling guidance

### Signatures Used

```rust
// Generate program description from traces
GenerateProgramDescription

// Generate instructions from tips and traces
GenerateInstructionFromTips
```

---

## COPRO

**Competitive Prompt Optimizer** - Iteratively generates and refines instructions through breadth-first search.

```rust
// File: crates/dsrs/src/optimizer/copro.rs

use dsrs::optimizer::COPRO;

let optimizer = COPRO::builder()
    .breadth(10)
    .depth(3)
    .init_temperature(1.4)
    .track_stats(true)
    .build();

optimizer.compile(&mut module, trainset).await?;
```

### Algorithm

```
Depth 0: [original instruction]
         │
         ▼ Generate breadth candidates
Depth 1: [candidate_1, candidate_2, ..., candidate_10]
         │
         ▼ Evaluate all, keep best
         │
         ▼ Generate improvements from best
Depth 2: [improved_1, improved_2, ..., improved_10]
         │
         ▼ Evaluate all, keep best
         │
         ▼ Generate improvements from best
Depth 3: [final_1, final_2, ..., final_10]
         │
         ▼ Select best overall
```

### Configuration

| Parameter | Default | Description |
|-----------|---------|-------------|
| `breadth` | 10 | Number of candidates per depth |
| `depth` | 3 | Optimization depth (iterations) |
| `init_temperature` | 1.4 | Initial LLM temperature |
| `track_stats` | false | Enable statistics tracking |

### Key Structures

```rust
/// Candidate instruction with evaluation score.
pub struct Candidate {
    pub score: f32,
    pub instruction: String,
    pub prefix: String,  // Output field prefix
}

/// Statistics per predictor module.
pub struct ProgramStats {
    pub best_score: f32,
    pub latest_score: f32,
    pub history: Vec<f32>,
}
```

### Signatures Used

```rust
// Initial instruction generation
BasicGenerateInstruction

// Refinement based on previous attempts
GenerateInstructionGivenAttempts
```

---

## GEPA

**Genetic-Pareto Evolutionary Optimizer** - Uses rich textual feedback and per-example dominance tracking. Based on "GEPA: Reflective Prompt Evolution Can Outperform Reinforcement Learning" (Agrawal et al., 2025).

```rust
// File: crates/dsrs/src/optimizer/gepa.rs

use dsrs::optimizer::GEPA;

let optimizer = GEPA::builder()
    .num_iterations(20)
    .minibatch_size(25)
    .num_trials(10)
    .temperature(1.0)
    .max_rollouts(Some(100))
    .max_lm_calls(Some(500))
    .track_stats(true)
    .track_best_outputs(true)
    .build();

// Requires FeedbackEvaluator (not just Evaluator)
optimizer.compile_with_feedback(&mut module, trainset).await?;
```

### Key Innovation

GEPA uses **per-example dominance tracking** to maintain a Pareto frontier of diverse, effective prompts:

```
┌─────────────────────────────────────────────────────────────┐
│                    Pareto Frontier                          │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Candidate A: Best on examples {1, 3, 5}                    │
│  Candidate B: Best on examples {2, 4}                       │
│  Candidate C: Best on examples {6, 7, 8}                    │
│                                                             │
│  → All candidates kept (each wins on some examples)         │
│  → Dominated candidates pruned                              │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Algorithm

```
1. Initialize Pareto frontier with seed candidates
2. For each iteration:
   a. Sample candidate proportional to coverage
   b. Collect execution traces with feedback
   c. Reflect on traces to identify weaknesses
   d. Propose improved instruction via mutation
   e. Evaluate new candidate on examples
   f. Add to frontier if it wins on any example
   g. Prune dominated candidates
3. Return best candidate by average score
```

### Configuration

| Parameter | Default | Description |
|-----------|---------|-------------|
| `num_iterations` | 20 | Evolution iterations |
| `minibatch_size` | 25 | Examples per evaluation |
| `num_trials` | 10 | Rollouts per candidate |
| `temperature` | 1.0 | LLM temperature |
| `max_rollouts` | None | Budget: max total rollouts |
| `max_lm_calls` | None | Budget: max LLM calls |
| `track_stats` | false | Enable statistics tracking |
| `track_best_outputs` | false | Track best outputs per example |

### Key Structures

```rust
/// GEPA candidate with per-example scores.
pub struct GEPACandidate {
    pub id: String,
    pub instruction: String,
    pub scores: HashMap<String, f32>,  // example_id → score
    pub parent_id: Option<String>,
    pub generation: usize,
}

/// Detailed optimization results.
pub struct GEPAResult {
    pub best_candidate: GEPACandidate,
    pub all_candidates: Vec<GEPACandidate>,
    pub total_rollouts: usize,
    pub total_lm_calls: usize,
    pub evolution_history: Vec<EvolutionStep>,
}
```

### Signatures Used

```rust
// Analyze execution traces for weaknesses
ReflectOnTrace

// Create improved instructions based on reflection
ProposeImprovedInstruction

// Select which module to optimize (for composite modules)
SelectModuleToImprove
```

---

## Pareto Frontier

Per-example dominance tracking for evolutionary optimization.

```rust
// File: crates/dsrs/src/optimizer/pareto.rs

use dsrs::optimizer::pareto::ParetoFrontier;

let mut frontier = ParetoFrontier::new();

// Add candidate if it wins on at least one example
frontier.add_candidate(candidate);

// Sample candidate weighted by coverage
let parent = frontier.sample_proportional_to_coverage();

// Get best by average score
let best = frontier.best_by_average();

// Get statistics
let stats = frontier.statistics();
// ParetoStatistics {
//     num_candidates: 5,
//     examples_covered: 25,
//     avg_coverage: 5.0,
//     max_coverage: 8,
//     min_coverage: 2,
// }
```

### Methods

| Method | Description |
|--------|-------------|
| `add_candidate()` | Add/update based on per-example scores |
| `prune_dominated()` | Remove candidates that don't win on any example |
| `sample_proportional_to_coverage()` | Coverage-weighted sampling |
| `best_by_average()` | Get candidate with highest average score |
| `statistics()` | Get ParetoStatistics |

---

## AutoOptimizer

Automatic optimization trigger for Adjutant.

```rust
// File: crates/adjutant/src/dspy/auto_optimizer.rs

use adjutant::dspy::auto_optimizer::{AutoOptimizer, AutoOptimizerConfig};

let config = AutoOptimizerConfig {
    enabled: true,
    min_labeled_examples: 20,
    accuracy_threshold: 0.70,
    min_hours_between_optimizations: 24,
    background_optimization: true,
    num_candidates: 10,
    num_trials: 20,
};

let optimizer = AutoOptimizer::new(config);

// Check if optimization should trigger
if optimizer.should_optimize(&performance_stats) {
    optimizer.optimize(&mut module, &labeled_examples).await?;
}
```

### Trigger Conditions

Optimization triggers when ALL conditions are met:

1. `enabled = true`
2. At least `min_labeled_examples` labeled examples
3. Current accuracy below `accuracy_threshold`
4. At least `min_hours_between_optimizations` since last run

### SelfImprover

Coordinates automatic optimization in Adjutant.

```rust
use adjutant::dspy::auto_optimizer::SelfImprover;

let improver = SelfImprover::new(config);

// Called after each session
improver.on_session_complete(session_results).await?;

// Background optimization if needed
improver.background_improve().await?;
```

---

## Usage Example

```rust
use dsrs::{
    Predict, Module, Optimizable, Evaluator,
    optimizer::MIPROv2,
    example,
};

// Define a module (must implement Module + Optimizable + Evaluator)
#[derive(Clone)]
struct QAModule {
    predictor: Predict,
}

impl Module for QAModule {
    async fn forward(&self, inputs: Example) -> Result<Prediction> {
        self.predictor.forward(inputs).await
    }
}

impl Optimizable for QAModule {
    fn get_signature(&self) -> &dyn MetaSignature {
        self.predictor.get_signature()
    }

    fn parameters(&mut self) -> IndexMap<String, &mut dyn Optimizable> {
        let mut params = IndexMap::new();
        params.insert("predictor".to_string(), &mut self.predictor as _);
        params
    }

    fn update_signature_instruction(&mut self, instruction: String) -> Result<()> {
        self.predictor.update_signature_instruction(instruction)
    }
}

impl Evaluator for QAModule {
    async fn metric(&self, example: &Example, prediction: &Prediction) -> f32 {
        // Return score 0.0 to 1.0
        let expected = example.get("answer");
        let actual = prediction.get("answer", None);
        if expected == actual { 1.0 } else { 0.0 }
    }
}

// Create optimizer
let optimizer = MIPROv2::builder()
    .num_candidates(10)
    .num_trials(20)
    .build();

// Load training data
let trainset = load_jsonl("train.jsonl").await?;

// Optimize!
optimizer.compile(&mut module, trainset).await?;

// Module is now optimized with better instruction and demos
```

---

## Optimizer Index

| Optimizer | Location | Algorithm | Use Case |
|-----------|----------|-----------|----------|
| MIPROv2 | `dsrs/src/optimizer/mipro.rs` | 3-stage LLM-guided | General purpose |
| COPRO | `dsrs/src/optimizer/copro.rs` | Breadth-first search | Fast iteration |
| GEPA | `dsrs/src/optimizer/gepa.rs` | Pareto evolution | Rich feedback, diversity |
| AutoOptimizer | `adjutant/src/dspy/auto_optimizer.rs` | Triggered | Background improvement |

---

## Integration with Other Primitives

| Primitive | Relationship |
|-----------|--------------|
| **Signatures** | Optimizers improve signature instructions |
| **Modules** | Modules must implement Optimizable trait |
| **Tools** | Tool usage patterns can be optimized |
| **Adapters** | Optimized prompts serialized by adapters |
| **Metrics** | Optimizers require Evaluator/FeedbackEvaluator |
