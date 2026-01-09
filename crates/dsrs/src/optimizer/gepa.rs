/// GEPA (Genetic-Pareto) Optimizer Implementation
///
/// GEPA is a reflective prompt optimizer that uses:
/// 1. Rich textual feedback (not just scores)
/// 2. Pareto-based candidate selection
/// 3. LLM-driven reflection and mutation
/// 4. Per-example dominance tracking
///
/// Reference: "GEPA: Reflective Prompt Evolution Can Outperform Reinforcement Learning"
/// (Agrawal et al., 2025, arxiv:2507.19457)
use anyhow::{Context, Result};
use bon::Builder;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate as dsrs;
use crate::{
    Example, LM, Module, Optimizable, Optimizer, Predict, Prediction, Predictor,
    evaluate::FeedbackEvaluator, example,
};
use dsrs_macros::Signature;

use super::pareto::ParetoFrontier;

// ============================================================================
// Core Data Structures
// ============================================================================

/// A candidate program in the evolutionary process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GEPACandidate {
    /// Unique identifier
    pub id: usize,

    /// The instruction/prompt for this candidate
    pub instruction: String,

    /// Name of the module this candidate targets
    pub module_name: String,

    /// Scores achieved on each evaluation example
    pub example_scores: Vec<f32>,

    /// Parent candidate ID (for lineage tracking)
    pub parent_id: Option<usize>,

    /// Generation number in the evolutionary process
    pub generation: usize,
}

impl GEPACandidate {
    /// Create a new candidate from a predictor
    pub fn from_predictor(predictor: &dyn Optimizable, module_name: impl Into<String>) -> Self {
        Self {
            id: 0,
            instruction: predictor.get_signature().instruction(),
            module_name: module_name.into(),
            example_scores: Vec::new(),
            parent_id: None,
            generation: 0,
        }
    }

    /// Calculate average score across all examples
    pub fn average_score(&self) -> f32 {
        if self.example_scores.is_empty() {
            return 0.0;
        }
        self.example_scores.iter().sum::<f32>() / self.example_scores.len() as f32
    }

    /// Create a mutated child candidate
    pub fn mutate(&self, new_instruction: String, generation: usize) -> Self {
        Self {
            id: 0, // Will be assigned by frontier
            instruction: new_instruction,
            module_name: self.module_name.clone(),
            example_scores: Vec::new(),
            parent_id: Some(self.id),
            generation,
        }
    }
}

/// Detailed results from GEPA optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GEPAResult {
    /// Best candidate found
    pub best_candidate: GEPACandidate,

    /// All candidates evaluated during optimization
    pub all_candidates: Vec<GEPACandidate>,

    /// Total number of rollouts performed
    pub total_rollouts: usize,

    /// Total LM calls made during optimization
    pub total_lm_calls: usize,

    /// Evolution history: generation -> best score at that generation
    pub evolution_history: Vec<(usize, f32)>,

    /// Highest score achieved on each validation task
    pub highest_score_achieved_per_val_task: Vec<f32>,

    /// Best outputs on validation set (if tracked)
    pub best_outputs_valset: Option<Vec<Prediction>>,

    /// Pareto frontier statistics over time
    pub frontier_history: Vec<ParetoStatistics>,
}

/// Statistics about Pareto frontier (re-exported from pareto module)
pub use super::pareto::ParetoStatistics;

// ============================================================================
// LLM Signatures for Reflection and Mutation
// ============================================================================

#[Signature]
struct ReflectOnTrace {
    /// You are an expert at analyzing program execution traces and identifying
    /// areas for improvement. Given the module instruction, example traces showing
    /// inputs, outputs, and feedback, identify specific weaknesses and suggest
    /// targeted improvements.

    #[input(desc = "The current instruction for the module")]
    pub current_instruction: String,

    #[input(desc = "Execution traces showing inputs, outputs, and evaluation feedback")]
    pub traces: String,

    #[input(desc = "Description of what the module should accomplish")]
    pub task_description: String,

    #[output(desc = "Analysis of weaknesses and specific improvement suggestions")]
    pub reflection: String,
}

#[Signature]
struct ProposeImprovedInstruction {
    /// You are an expert prompt engineer. Given the current instruction, execution
    /// traces, feedback, and reflection on weaknesses, propose an improved instruction
    /// that addresses the identified issues. Be creative and consider various prompting
    /// techniques.

    #[input(desc = "The current instruction")]
    pub current_instruction: String,

    #[input(desc = "Reflection on weaknesses and improvement suggestions")]
    pub reflection: String,

    #[input(desc = "Execution traces and feedback from recent rollouts")]
    pub traces_and_feedback: String,

    #[output(desc = "An improved instruction that addresses the identified weaknesses")]
    pub improved_instruction: String,
}

#[Signature]
struct SelectModuleToImprove {
    /// Given multiple modules in a program and their performance feedback, select which
    /// module would benefit most from optimization. Consider which module's errors are
    /// most impactful and addressable through instruction changes.

    #[input(desc = "List of modules with their current instructions and performance")]
    pub module_summary: String,

    #[input(desc = "Recent execution traces showing module interactions")]
    pub execution_traces: String,

    #[output(desc = "Name of the module to optimize and reasoning")]
    pub selected_module: String,
}

// ============================================================================
// GEPA Optimizer
// ============================================================================

/// GEPA Optimizer Configuration
#[derive(Builder)]
pub struct GEPA {
    /// Maximum number of evolutionary iterations
    #[builder(default = 20)]
    pub num_iterations: usize,

    /// Size of minibatch for each rollout
    #[builder(default = 25)]
    pub minibatch_size: usize,

    /// Number of trials per candidate evaluation
    #[builder(default = 10)]
    pub num_trials: usize,

    /// Temperature for LLM-based mutations
    #[builder(default = 1.0)]
    pub temperature: f32,

    /// Track detailed statistics
    #[builder(default = true)]
    pub track_stats: bool,

    /// Track best outputs on validation set (for inference-time search)
    #[builder(default = false)]
    pub track_best_outputs: bool,

    /// Maximum total rollouts (budget control)
    pub max_rollouts: Option<usize>,

    /// Maximum LM calls (budget control)
    pub max_lm_calls: Option<usize>,

    /// Optional separate LM for meta-prompting (instruction generation)
    pub prompt_model: Option<LM>,

    /// Validation set for Pareto evaluation (if None, uses trainset)
    pub valset: Option<Vec<Example>>,
}

impl GEPA {
    /// Initialize the Pareto frontier with the seed program
    async fn initialize_frontier<M>(
        &self,
        module: &mut M,
        trainset: &[Example],
    ) -> Result<ParetoFrontier>
    where
        M: Module + Optimizable + FeedbackEvaluator,
    {
        let mut frontier = ParetoFrontier::new();

        // Collect predictor information first (to release mutable borrow)
        let candidate_infos: Vec<GEPACandidate> = {
            let predictors = module.parameters();
            predictors
                .into_iter()
                .map(|(name, predictor)| GEPACandidate::from_predictor(predictor, name))
                .collect()
        };

        // Now evaluate each candidate (module is no longer borrowed mutably)
        for candidate in candidate_infos {
            let scores = self
                .evaluate_candidate(module, trainset, &candidate)
                .await?;
            frontier.add_candidate(candidate, &scores);
        }

        Ok(frontier)
    }

    /// Evaluate a candidate on a set of examples (in parallel for speed)
    async fn evaluate_candidate<M>(
        &self,
        module: &M,
        examples: &[Example],
        _candidate: &GEPACandidate,
    ) -> Result<Vec<f32>>
    where
        M: Module + FeedbackEvaluator,
    {
        use futures::future::join_all;

        let futures: Vec<_> = examples
            .iter()
            .map(|example| async move {
                let prediction = module.forward(example.clone()).await?;
                let feedback = module.feedback_metric(example, &prediction).await;
                Ok::<f32, anyhow::Error>(feedback.score)
            })
            .collect();

        let results = join_all(futures).await;
        results.into_iter().collect()
    }

    /// Collect execution traces with feedback
    async fn collect_traces<M>(
        &self,
        module: &M,
        minibatch: &[Example],
    ) -> Result<Vec<(Example, Prediction, String)>>
    where
        M: Module + FeedbackEvaluator,
    {
        let mut traces = Vec::with_capacity(minibatch.len());

        for example in minibatch {
            let prediction = module.forward(example.clone()).await?;
            let feedback = module.feedback_metric(example, &prediction).await;

            // Format trace for LLM reflection
            let trace_text = format!(
                "Input: {:?}\nOutput: {:?}\nScore: {:.3}\nFeedback: {}",
                example, prediction, feedback.score, feedback.feedback
            );

            traces.push((example.clone(), prediction, trace_text));
        }

        Ok(traces)
    }

    /// Generate improved instruction through LLM reflection
    async fn generate_mutation(
        &self,
        current_instruction: &str,
        traces: &[(Example, Prediction, String)],
        task_description: &str,
    ) -> Result<String> {
        // Combine traces into a single string
        let traces_text = traces
            .iter()
            .enumerate()
            .map(|(i, (_, _, trace))| format!("=== Trace {} ===\n{}\n", i + 1, trace))
            .collect::<Vec<_>>()
            .join("\n");

        // First, reflect on the traces
        let reflect_predictor = Predict::new(ReflectOnTrace::new());
        let reflection_input = example! {
            "current_instruction": "input" => current_instruction,
            "traces": "input" => traces_text.clone(),
            "task_description": "input" => task_description
        };

        let reflection_output = if let Some(mut prompt_model) = self.prompt_model.clone() {
            prompt_model.temperature = self.temperature;
            reflect_predictor
                .forward_with_config(reflection_input, Arc::new(prompt_model))
                .await?
        } else {
            reflect_predictor.forward(reflection_input).await?
        };

        let reflection = reflection_output
            .get("reflection", None)
            .as_str()
            .unwrap_or("")
            .to_string();

        // Then, propose improved instruction
        let propose_predictor = Predict::new(ProposeImprovedInstruction::new());
        let proposal_input = example! {
            "current_instruction": "input" => current_instruction,
            "reflection": "input" => reflection.clone(),
            "traces_and_feedback": "input" => traces_text.clone()
        };

        let proposal_output = if let Some(mut prompt_model) = self.prompt_model.clone() {
            prompt_model.temperature = self.temperature;
            propose_predictor
                .forward_with_config(proposal_input, Arc::new(prompt_model))
                .await?
        } else {
            propose_predictor.forward(proposal_input).await?
        };

        let improved = proposal_output
            .get("improved_instruction", None)
            .as_str()
            .unwrap_or(current_instruction)
            .to_string();

        Ok(improved)
    }
}

impl Optimizer for GEPA {
    async fn compile<M>(&self, _module: &mut M, _trainset: Vec<Example>) -> Result<()>
    where
        M: Module + Optimizable + crate::Evaluator,
    {
        // GEPA requires FeedbackEvaluator, not just Evaluator
        // This is a compilation error that guides users to implement the right trait
        anyhow::bail!(
            "GEPA requires the module to implement FeedbackEvaluator trait. \
             Please implement feedback_metric() method that returns FeedbackMetric."
        )
    }
}

impl GEPA {
    /// Compile method specifically for FeedbackEvaluator modules
    pub async fn compile_with_feedback<M>(
        &self,
        module: &mut M,
        trainset: Vec<Example>,
    ) -> Result<GEPAResult>
    where
        M: Module + Optimizable + FeedbackEvaluator,
    {
        println!("GEPA: Starting reflective prompt optimization");
        println!("  Iterations: {}", self.num_iterations);
        println!("  Minibatch size: {}", self.minibatch_size);

        // Use valset if provided, otherwise use trainset for Pareto evaluation
        let eval_set = self.valset.as_ref().unwrap_or(&trainset);

        // Initialize frontier with seed program
        let mut frontier = self.initialize_frontier(&mut *module, eval_set).await?;
        println!("  Initialized frontier with {} candidates", frontier.len());

        // Track statistics
        let mut all_candidates = Vec::new();
        let mut evolution_history = Vec::new();
        let mut frontier_history = Vec::new();
        let mut total_rollouts = 0;
        let mut total_lm_calls = 0;

        // Main evolutionary loop
        for generation in 0..self.num_iterations {
            println!("\nGeneration {}/{}", generation + 1, self.num_iterations);

            // Check budget constraints
            if let Some(max_rollouts) = self.max_rollouts
                && total_rollouts >= max_rollouts
            {
                println!("  Budget limit reached: max rollouts");
                break;
            }

            // Sample candidate from frontier (proportional to coverage)
            let parent = frontier
                .sample_proportional_to_coverage()
                .context("Failed to sample from frontier")?
                .clone();

            println!(
                "  Sampled parent (ID {}): avg score {:.3}",
                parent.id,
                parent.average_score()
            );

            // Sample minibatch
            let minibatch: Vec<Example> =
                trainset.iter().take(self.minibatch_size).cloned().collect();

            // Apply parent instruction to module
            {
                let mut predictors = module.parameters();
                if let Some(predictor) = predictors.get_mut(&parent.module_name) {
                    predictor.update_signature_instruction(parent.instruction.clone())?;
                }
            }

            // Collect execution traces
            let traces = self.collect_traces(module, &minibatch).await?;
            total_rollouts += traces.len();

            // Generate mutation through LLM reflection
            let task_desc = "Perform the task as specified";
            let new_instruction = self
                .generate_mutation(&parent.instruction, &traces, task_desc)
                .await?;

            total_lm_calls += 2; // Reflection + proposal

            println!("  Generated new instruction through reflection");

            // Create child candidate
            let child = parent.mutate(new_instruction.clone(), generation + 1);

            // Apply child instruction and evaluate
            {
                let mut predictors = module.parameters();
                if let Some(predictor) = predictors.get_mut(&child.module_name) {
                    predictor.update_signature_instruction(child.instruction.clone())?;
                }
            }

            let child_scores = self.evaluate_candidate(module, eval_set, &child).await?;
            total_rollouts += child_scores.len();

            let child_avg = child_scores.iter().sum::<f32>() / child_scores.len() as f32;
            println!("  Child avg score: {:.3}", child_avg);

            // Add to frontier
            let added = frontier.add_candidate(child.clone(), &child_scores);
            if added {
                println!("  Added to Pareto frontier");
            } else {
                println!("  Dominated, not added");
            }

            // Track statistics
            if self.track_stats {
                all_candidates.push(child);
                let best_avg = frontier
                    .best_by_average()
                    .map(|c| c.average_score())
                    .unwrap_or(0.0);
                evolution_history.push((generation, best_avg));
                frontier_history.push(frontier.statistics());
            }

            println!("  Frontier size: {}", frontier.len());
        }

        // Get best candidate
        let best_candidate = frontier
            .best_by_average()
            .context("No candidates on frontier")?
            .clone();

        println!("\nGEPA optimization complete");
        println!(
            "  Best average score: {:.3}",
            best_candidate.average_score()
        );
        println!("  Total rollouts: {}", total_rollouts);
        println!("  Total LM calls: {}", total_lm_calls);

        // Apply best instruction to module
        {
            let mut predictors = module.parameters();
            if let Some(predictor) = predictors.get_mut(&best_candidate.module_name) {
                predictor.update_signature_instruction(best_candidate.instruction.clone())?;
            }
        }

        Ok(GEPAResult {
            best_candidate,
            all_candidates,
            total_rollouts,
            total_lm_calls,
            evolution_history,
            highest_score_achieved_per_val_task: vec![], // TODO: Track per-task bests
            best_outputs_valset: None, // TODO: Implement if track_best_outputs is true
            frontier_history,
        })
    }
}
