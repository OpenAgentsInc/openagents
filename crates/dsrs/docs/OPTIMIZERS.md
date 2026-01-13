# Optimizers

- **Status:** Accurate
- **Last verified:** d44f9cd3f
- **Source of truth:** `crates/dsrs/src/optimizer/`
- **Doc owner:** dsrs
- **If this doc conflicts with code, code wins.**

Learning without training models.

## Overview

An **Optimizer** improves your DSPy program automatically by:

- Trying different prompt strategies
- Adjusting intermediate reasoning
- Selecting better decompositions

This is **ML-style optimization**, but applied to *program structure*, not weights. No fine-tuning required.

- **Mental model:** "The system learns how to ask better questions."

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

## Policy Bundle Format

Optimized prompts are stored in versioned policy bundles.

### Bundle Directory Structure

```
.adjutant/policies/
├── v1.2.3/
│   ├── manifest.json          # Bundle metadata
│   ├── signatures/
│   │   ├── ToolCallSignature.json
│   │   ├── SubtaskPlanningSignature.json
│   │   └── ResultSynthesisSignature.json
│   └── demos/
│       ├── tool_call_demos.jsonl
│       ├── planning_demos.jsonl
│       └── synthesis_demos.jsonl
├── v1.2.2/
│   └── ...
└── current -> v1.2.3/         # Symlink to active version
```

### manifest.json Format

```json
{
  "version": "v1.2.3",
  "created_at": "2025-01-13T10:00:00Z",
  "parent_version": "v1.2.2",
  "optimization_method": "MIPROv2",
  "optimization_config": {
    "num_candidates": 10,
    "num_trials": 20,
    "max_bootstrapped_demos": 3,
    "temperature": 1.0
  },
  "training_dataset": "labeled_examples_v3.jsonl",
  "training_examples": 150,
  "validation_score": 0.87,
  "signatures": [
    {
      "name": "ToolCallSignature",
      "instruction_hash": "sha256:abc123...",
      "demo_count": 5
    },
    {
      "name": "SubtaskPlanningSignature",
      "instruction_hash": "sha256:def456...",
      "demo_count": 3
    }
  ],
  "locked": false,
  "notes": "Improved tool selection accuracy by 12%"
}
```

### Signature File Format

```json
{
  "name": "ToolCallSignature",
  "instruction": "Given a plan step and available tools, select the best tool and generate valid parameters...",
  "instruction_hash": "sha256:abc123...",
  "input_fields": ["step", "step_intent", "available_tools", "execution_history", "file_context"],
  "output_fields": ["tool", "params", "expected_outcome", "progress", "needs_user_input"],
  "demos": [
    {
      "inputs": { "step": "Read the authentication module", "step_intent": "Investigate" },
      "outputs": { "tool": "file_read", "params": {"path": "src/auth.rs"} }
    }
  ],
  "optimization_history": [
    { "version": "v1.2.2", "score": 0.82 },
    { "version": "v1.2.3", "score": 0.87 }
  ]
}
```

### Loading Policy Bundles

```rust
// File: crates/dsrs/src/optimizer/policy.rs

pub struct PolicyBundle {
    pub manifest: PolicyManifest,
    pub signatures: HashMap<String, OptimizedSignature>,
}

impl PolicyBundle {
    /// Load from directory
    pub fn load(path: &Path) -> Result<Self> {
        let manifest_path = path.join("manifest.json");
        let manifest: PolicyManifest = serde_json::from_reader(File::open(manifest_path)?)?;

        let mut signatures = HashMap::new();
        for sig_entry in &manifest.signatures {
            let sig_path = path.join("signatures").join(format!("{}.json", sig_entry.name));
            let sig: OptimizedSignature = serde_json::from_reader(File::open(sig_path)?)?;
            signatures.insert(sig_entry.name.clone(), sig);
        }

        Ok(Self { manifest, signatures })
    }

    /// Apply to a module
    pub fn apply_to<M: Optimizable>(&self, module: &mut M) -> Result<()> {
        for (name, params) in module.parameters() {
            if let Some(sig) = self.signatures.get(&name) {
                params.update_signature_instruction(sig.instruction.clone())?;
                // Apply demos...
            }
        }
        Ok(())
    }

    /// Save after optimization
    pub fn save(&self, path: &Path) -> Result<()> {
        fs::create_dir_all(path)?;
        // Save manifest
        let manifest_path = path.join("manifest.json");
        serde_json::to_writer_pretty(File::create(manifest_path)?, &self.manifest)?;
        // Save signatures
        let sig_dir = path.join("signatures");
        fs::create_dir_all(&sig_dir)?;
        for (name, sig) in &self.signatures {
            let sig_path = sig_dir.join(format!("{}.json", name));
            serde_json::to_writer_pretty(File::create(sig_path)?, sig)?;
        }
        Ok(())
    }
}
```

---

## Training Datasets

Datasets used for optimizer compilation.

### Dataset Sources

| Dataset | Location | Examples | Description |
|---------|----------|----------|-------------|
| **Tool Call Training** | `.adjutant/datasets/tool_calls.jsonl` | ~500 | Labeled tool selection decisions |
| **Planning Training** | `.adjutant/datasets/planning.jsonl` | ~200 | Task → PlanIR examples |
| **Synthesis Training** | `.adjutant/datasets/synthesis.jsonl` | ~150 | Subtask results → PR summary |
| **Issue Validation** | `.adjutant/datasets/issue_validation.jsonl` | ~100 | Issue + context → valid/stale |

### Dataset Format

```jsonl
{"inputs":{"step":"Read auth module","step_intent":"Investigate"},"outputs":{"tool":"file_read","params":{"path":"src/auth.rs"}},"labels":{"step_utility":0.9,"was_repeated":false}}
{"inputs":{"step":"Run tests","step_intent":"Verify"},"outputs":{"tool":"shell","params":{"command":"cargo test"}},"labels":{"step_utility":0.8,"verification_delta":3}}
```

### Dataset Collection

Datasets are collected from production sessions:

```rust
// File: crates/adjutant/src/dspy/dataset_collector.rs

pub struct DatasetCollector {
    tool_calls: Vec<LabeledToolCall>,
    plans: Vec<LabeledPlan>,
}

impl DatasetCollector {
    /// Record a tool call with outcome labels
    pub fn record_tool_call(&mut self, call: LabeledToolCall) {
        self.tool_calls.push(call);
    }

    /// Export to JSONL for training
    pub fn export_tool_calls(&self, path: &Path) -> Result<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        for call in &self.tool_calls {
            let line = serde_json::to_string(call)?;
            writeln!(writer, "{}", line)?;
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
pub struct LabeledToolCall {
    pub inputs: ToolCallInputs,
    pub outputs: ToolCallOutputs,
    pub labels: ToolCallLabels,
}

#[derive(Serialize, Deserialize)]
pub struct ToolCallLabels {
    pub step_utility: f32,          // From ToolResultSignature
    pub verification_delta: i32,     // Test delta after step
    pub was_repeated: bool,          // Did we repeat this exact call?
    pub cost_tokens: u64,            // Tokens consumed
    pub cost_tool_calls: u64,        // Number of tool invocations
}
```

### Dataset Statistics

```bash
# View dataset statistics
adjutant dataset stats

# Output:
# Dataset: tool_calls.jsonl
#   Examples: 523
#   Avg step_utility: 0.72
#   Repeated calls: 8.2%
#   Avg tokens/call: 1,450
#
# Dataset: planning.jsonl
#   Examples: 198
#   Avg confidence: 0.81
#   Avg steps/plan: 4.2
```

### Train/Test Split

```rust
/// Split dataset for optimization
pub fn split_dataset(
    examples: Vec<Example>,
    train_ratio: f32,
    seed: u64,
) -> (Vec<Example>, Vec<Example>) {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut shuffled = examples;
    shuffled.shuffle(&mut rng);

    let split_idx = (shuffled.len() as f32 * train_ratio) as usize;
    let train = shuffled[..split_idx].to_vec();
    let test = shuffled[split_idx..].to_vec();

    (train, test)
}

// Usage in optimization
let (trainset, valset) = split_dataset(examples, 0.8, 42);
optimizer.compile_with_validation(&mut module, trainset, valset).await?;
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
