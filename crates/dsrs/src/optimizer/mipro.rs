use crate as dsrs;
/// MIPROv2 Optimizer Implementation
///
/// Multi-prompt Instruction Proposal Optimizer (MIPROv2) is an advanced optimizer
/// that automatically generates and evaluates candidate prompts using LLMs.
///
/// ## Three-Stage Process
///
/// 1. **Trace Generation**: Runs the module with training data to generate execution traces
/// 2. **Prompt Generation**: Uses an LLM to generate candidate prompts based on:
///    - Program descriptions (LLM-generated)
///    - Execution traces
///    - Prompting tips library
/// 3. **Evaluation & Combination**: Evaluates candidates in batches and combines best components
use crate::{
    Evaluator, Example, LM, Module, Optimizable, Optimizer, Predict, Prediction, Predictor,
    example, get_lm,
};
use anyhow::{Context, Result};
use bon::Builder;
use dsrs_macros::Signature;
use std::sync::Arc;

// ============================================================================
// Signature Definitions for LLM-based Prompt Generation
// ============================================================================

#[Signature]
struct GenerateProgramDescription {
    /// You are an expert at understanding and describing programs. Given a task signature with input and output fields, and some example traces, generate a clear and concise description of what the program does.

    #[input(desc = "The task signature showing input and output fields")]
    pub signature_fields: String,

    #[input(desc = "Example input-output traces from the program")]
    pub example_traces: String,

    #[output(desc = "A clear description of what the program does")]
    pub program_description: String,
}

#[Signature]
struct GenerateInstructionFromTips {
    /// You are an expert prompt engineer. Given a program description, example traces, and a collection of prompting best practices, generate an effective instruction that will help a language model perform this task well.
    ///
    /// Be creative and consider various prompting techniques like chain-of-thought, few-shot examples, role-playing, and output formatting.

    #[input(desc = "Description of what the program should do")]
    pub program_description: String,

    #[input(desc = "Example input-output traces showing desired behavior")]
    pub example_traces: String,

    #[input(desc = "Best practices and tips for writing effective prompts")]
    pub prompting_tips: String,

    #[output(desc = "An optimized instruction for the language model")]
    pub instruction: String,
}

// ============================================================================
// Core Data Structures
// ============================================================================

/// Represents a single execution trace of the program
#[derive(Clone, Debug)]
pub struct Trace {
    /// Input example
    pub inputs: Example,
    /// Output prediction
    pub outputs: Prediction,
    /// Evaluation score (if available)
    pub score: Option<f32>,
}

impl Trace {
    /// Creates a new trace
    pub fn new(inputs: Example, outputs: Prediction, score: Option<f32>) -> Self {
        Self {
            inputs,
            outputs,
            score,
        }
    }

    /// Formats the trace as a human-readable string for LLM consumption
    pub fn format_for_prompt(&self) -> String {
        let mut result = String::new();
        result.push_str("Input:\n");

        for (key, value) in &self.inputs.data {
            result.push_str(&format!("  {}: {}\n", key, value));
        }

        result.push_str("Output:\n");
        for (key, value) in &self.outputs.data {
            result.push_str(&format!("  {}: {}\n", key, value));
        }

        if let Some(score) = self.score {
            result.push_str(&format!("Score: {:.3}\n", score));
        }

        result
    }
}

/// Represents a candidate prompt with its associated examples and score
#[derive(Clone, Debug)]
pub struct PromptCandidate {
    /// The instruction text
    pub instruction: String,
    /// Few-shot demonstration examples (reserved for future enhancement)
    #[allow(dead_code)]
    pub demos: Vec<Example>,
    /// Evaluation score
    pub score: f32,
}

impl PromptCandidate {
    /// Creates a new candidate with default score
    pub fn new(instruction: String, demos: Vec<Example>) -> Self {
        Self {
            instruction,
            demos,
            score: 0.0,
        }
    }

    /// Updates the candidate's score
    pub fn with_score(mut self, score: f32) -> Self {
        self.score = score;
        self
    }
}

/// Library of prompting tips and best practices
pub struct PromptingTips {
    pub tips: Vec<String>,
}

impl PromptingTips {
    /// Creates a new prompting tips library with default tips
    pub fn default_tips() -> Self {
        Self {
            tips: vec![
                "Use clear and specific language".to_string(),
                "Provide context about the task domain".to_string(),
                "Specify the desired output format".to_string(),
                "Use chain-of-thought reasoning for complex tasks".to_string(),
                "Include few-shot examples when helpful".to_string(),
                "Break down complex instructions into steps".to_string(),
                "Use role-playing (e.g., 'You are an expert...') when appropriate".to_string(),
                "Specify constraints and edge cases".to_string(),
                "Request explanations or reasoning when needed".to_string(),
                "Use structured output formats (JSON, lists, etc.) when applicable".to_string(),
                "Consider the model's strengths and limitations".to_string(),
                "Be explicit about what to avoid or exclude".to_string(),
                "Use positive framing (what to do vs. what not to do)".to_string(),
                "Provide examples of both correct and incorrect outputs when useful".to_string(),
                "Use delimiters or markers to separate different sections".to_string(),
            ],
        }
    }

    /// Formats tips as a string for LLM consumption
    pub fn format_for_prompt(&self) -> String {
        self.tips
            .iter()
            .enumerate()
            .map(|(i, tip)| format!("{}. {}", i + 1, tip))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// ============================================================================
// MIPROv2 Optimizer
// ============================================================================

/// MIPROv2 (Multi-prompt Instruction Proposal Optimizer v2)
///
/// An advanced optimizer that uses LLMs to automatically generate and refine
/// prompts based on program traces, descriptions, and prompting best practices.
#[derive(Builder)]
pub struct MIPROv2 {
    /// Number of candidate prompts to generate per iteration
    #[builder(default = 10)]
    pub num_candidates: usize,

    /// Maximum number of bootstrapped (generated) demos to include
    #[builder(default = 3)]
    pub max_bootstrapped_demos: usize,

    /// Maximum number of labeled demos to include from training set
    #[builder(default = 3)]
    pub max_labeled_demos: usize,

    /// Number of evaluation trials (iterations)
    #[builder(default = 20)]
    pub num_trials: usize,

    /// Size of minibatch for evaluation
    #[builder(default = 25)]
    pub minibatch_size: usize,

    /// Temperature for prompt generation
    #[builder(default = 1.0)]
    pub temperature: f32,

    /// Optional separate LM for prompt generation (defaults to global LM)
    pub prompt_model: Option<LM>,

    /// Track and display statistics
    #[builder(default = true)]
    pub track_stats: bool,

    /// Random seed for reproducibility
    pub seed: Option<u64>,
}

impl MIPROv2 {
    // ========================================================================
    // Stage 1: Trace Generation
    // ========================================================================

    /// Generates execution traces by running the module on training examples
    async fn generate_traces<M>(&self, module: &M, examples: &[Example]) -> Result<Vec<Trace>>
    where
        M: Module + Evaluator,
    {
        let mut traces = Vec::with_capacity(examples.len());

        println!(
            "Stage 1: Generating traces from {} examples",
            examples.len()
        );

        for (idx, example) in examples.iter().enumerate() {
            if idx % 10 == 0 {
                println!("  Processing example {}/{}", idx + 1, examples.len());
            }

            // Run forward pass
            let prediction = module
                .forward(example.clone())
                .await
                .context("Failed to generate prediction for trace")?;

            // Evaluate the prediction
            let score = module.metric(example, &prediction).await;

            traces.push(Trace::new(example.clone(), prediction, Some(score)));
        }

        println!("Generated {} traces", traces.len());
        Ok(traces)
    }

    /// Selects the best traces based on their scores
    pub fn select_best_traces(&self, traces: &[Trace], num_select: usize) -> Vec<Trace> {
        let mut scored_traces: Vec<_> = traces
            .iter()
            .filter(|t| t.score.is_some())
            .cloned()
            .collect();

        // Sort by score descending
        scored_traces.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        scored_traces.into_iter().take(num_select).collect()
    }

    // ========================================================================
    // Stage 2: Candidate Prompt Generation
    // ========================================================================

    /// Generates a program description using an LLM
    async fn generate_program_description(
        &self,
        signature_desc: &str,
        traces: &[Trace],
    ) -> Result<String> {
        let description_generator = Predict::new(GenerateProgramDescription::new());

        // Format traces for the prompt
        let traces_str = traces
            .iter()
            .take(5) // Use first 5 traces
            .map(|t| t.format_for_prompt())
            .collect::<Vec<_>>()
            .join("\n---\n");

        let input = example! {
            "signature_fields": "input" => signature_desc.to_string(),
            "example_traces": "input" => traces_str,
        };

        let prediction = if let Some(mut pm) = self.prompt_model.clone() {
            pm.temperature = 0.7;
            description_generator
                .forward_with_config(input, Arc::new(pm))
                .await?
        } else {
            let lm = get_lm();
            description_generator.forward_with_config(input, lm).await?
        };

        Ok(prediction
            .data
            .get("program_description")
            .and_then(|v| v.as_str())
            .unwrap_or("Generate accurate outputs for the given inputs.")
            .to_string())
    }

    /// Generates candidate instructions using LLM with prompting tips
    async fn generate_candidate_instructions(
        &self,
        program_description: &str,
        traces: &[Trace],
        num_candidates: usize,
    ) -> Result<Vec<String>> {
        let instruction_generator = Predict::new(GenerateInstructionFromTips::new());
        let tips = PromptingTips::default_tips();

        // Format traces
        let traces_str = traces
            .iter()
            .take(8)
            .map(|t| t.format_for_prompt())
            .collect::<Vec<_>>()
            .join("\n---\n");

        println!(
            "Stage 2: Generating {} candidate instructions",
            num_candidates
        );

        let mut candidates = Vec::new();

        // Generate candidates sequentially (simpler and avoids lifetime issues)
        for i in 0..num_candidates {
            let input = example! {
                "program_description": "input" => program_description.to_string(),
                "example_traces": "input" => traces_str.clone(),
                "prompting_tips": "input" => tips.format_for_prompt(),
            };

            let result = if let Some(mut pm) = self.prompt_model.clone() {
                pm.temperature = self.temperature;
                instruction_generator
                    .forward_with_config(input, Arc::new(pm))
                    .await
            } else {
                let lm = get_lm();
                instruction_generator.forward_with_config(input, lm).await
            };

            if let Ok(pred) = result
                && let Some(instruction) = pred.data.get("instruction").and_then(|v| v.as_str())
            {
                candidates.push(instruction.to_string());
            }

            if (i + 1) % 3 == 0 || i == num_candidates - 1 {
                println!(
                    "  Generated {}/{} candidates",
                    candidates.len(),
                    num_candidates
                );
            }
        }

        println!(
            "Generated {} total candidate instructions",
            candidates.len()
        );
        Ok(candidates)
    }

    /// Creates prompt candidates by pairing instructions with demo selections
    pub fn create_prompt_candidates(
        &self,
        instructions: Vec<String>,
        traces: &[Trace],
    ) -> Vec<PromptCandidate> {
        let best_traces = self.select_best_traces(traces, self.max_labeled_demos);
        let demo_examples: Vec<Example> = best_traces.into_iter().map(|t| t.inputs).collect();

        instructions
            .into_iter()
            .map(|inst| PromptCandidate::new(inst, demo_examples.clone()))
            .collect()
    }

    // ========================================================================
    // Stage 3: Evaluation and Selection
    // ========================================================================

    /// Evaluates a single prompt candidate
    async fn evaluate_candidate<M>(
        &self,
        module: &mut M,
        candidate: &PromptCandidate,
        eval_examples: &[Example],
        predictor_name: &str,
    ) -> Result<f32>
    where
        M: Module + Optimizable + Evaluator,
    {
        // Update module with candidate instruction
        {
            let mut params = module.parameters();
            if let Some(predictor) = params.get_mut(predictor_name) {
                predictor.update_signature_instruction(candidate.instruction.clone())?;

                // Note: Demo setting would require mutable signature access
                // This is a design consideration for future enhancement
            }
        }

        // Evaluate on minibatch
        let minibatch: Vec<Example> = eval_examples
            .iter()
            .take(self.minibatch_size)
            .cloned()
            .collect();

        let score = module.evaluate(minibatch).await;
        Ok(score)
    }

    /// Evaluates all candidates and returns the best one
    async fn evaluate_and_select_best<M>(
        &self,
        module: &mut M,
        candidates: Vec<PromptCandidate>,
        eval_examples: &[Example],
        predictor_name: &str,
    ) -> Result<PromptCandidate>
    where
        M: Module + Optimizable + Evaluator,
    {
        println!(
            "Stage 3: Evaluating {} candidates on minibatch of {} examples",
            candidates.len(),
            self.minibatch_size.min(eval_examples.len())
        );

        let mut evaluated_candidates = Vec::new();

        for (idx, candidate) in candidates.into_iter().enumerate() {
            println!("  Evaluating candidate {}/{}", idx + 1, self.num_candidates);

            let score = self
                .evaluate_candidate(module, &candidate, eval_examples, predictor_name)
                .await?;

            evaluated_candidates.push(candidate.with_score(score));

            if self.track_stats {
                println!("    Score: {:.3}", score);
            }
        }

        // Find best candidate
        let best = evaluated_candidates
            .into_iter()
            .max_by(|a, b| {
                a.score
                    .partial_cmp(&b.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .context("No candidates to evaluate")?;

        println!("Best candidate score: {:.3}", best.score);
        Ok(best)
    }

    // ========================================================================
    // Helper Methods
    // ========================================================================

    /// Formats signature fields as a string
    pub fn format_signature_fields(&self, signature: &dyn crate::core::MetaSignature) -> String {
        let mut result = String::new();

        result.push_str("Input Fields:\n");
        if let Some(obj) = signature.input_fields().as_object() {
            for (name, field) in obj {
                let desc = field
                    .get("desc")
                    .and_then(|v| v.as_str())
                    .unwrap_or("No description");
                result.push_str(&format!("  - {}: {}\n", name, desc));
            }
        }

        result.push_str("\nOutput Fields:\n");
        if let Some(obj) = signature.output_fields().as_object() {
            for (name, field) in obj {
                let desc = field
                    .get("desc")
                    .and_then(|v| v.as_str())
                    .unwrap_or("No description");
                result.push_str(&format!("  - {}: {}\n", name, desc));
            }
        }

        result
    }
}

// ============================================================================
// Optimizer Trait Implementation
// ============================================================================

impl Optimizer for MIPROv2 {
    async fn compile<M>(&self, module: &mut M, trainset: Vec<Example>) -> Result<()>
    where
        M: Module + Optimizable + Evaluator,
    {
        println!("\n=== MIPROv2 Optimization Started ===");
        println!("Configuration:");
        println!("  Candidates: {}", self.num_candidates);
        println!("  Trials: {}", self.num_trials);
        println!("  Minibatch size: {}", self.minibatch_size);
        println!("  Training examples: {}", trainset.len());

        // Get predictor information
        let predictor_names: Vec<String> = module.parameters().keys().cloned().collect();

        if predictor_names.is_empty() {
            return Err(anyhow::anyhow!("No optimizable parameters found in module"));
        }

        println!(
            "  Optimizing {} predictor(s): {:?}\n",
            predictor_names.len(),
            predictor_names
        );

        // Optimize each predictor
        for predictor_name in predictor_names {
            println!("--- Optimizing predictor: {} ---", predictor_name);

            // Get signature for this predictor
            let signature_desc = {
                let params = module.parameters();
                if let Some(predictor) = params.get(&predictor_name) {
                    self.format_signature_fields(predictor.get_signature())
                } else {
                    continue;
                }
            };

            // Stage 1: Generate traces
            let traces = self.generate_traces(module, &trainset).await?;

            // Stage 2: Generate candidates
            let program_description = self
                .generate_program_description(&signature_desc, &traces)
                .await?;

            println!("Generated program description: {}", program_description);

            let instructions = self
                .generate_candidate_instructions(&program_description, &traces, self.num_candidates)
                .await?;

            let candidates = self.create_prompt_candidates(instructions, &traces);

            // Stage 3: Evaluate and select best
            let best_candidate = self
                .evaluate_and_select_best(module, candidates, &trainset, &predictor_name)
                .await?;

            // Apply best candidate
            {
                let mut params = module.parameters();
                if let Some(predictor) = params.get_mut(&predictor_name) {
                    predictor.update_signature_instruction(best_candidate.instruction.clone())?;
                    // Note: Demo setting would require mutable signature access
                    // This is a design consideration for future enhancement
                }
            }

            println!(
                "✓ Optimized {} with score {:.3}",
                predictor_name, best_candidate.score
            );
            println!("  Instruction: {}\n", best_candidate.instruction);
        }

        println!("=== MIPROv2 Optimization Complete ===\n");
        Ok(())
    }
}
