use crate as dsrs;
use crate::{
    Evaluator, Example, LM, Module, Optimizable, Optimizer, Predict, Prediction, Predictor,
    example, get_lm,
};
use anyhow::Result;
use bon::Builder;
use dsrs_macros::Signature;
use futures::future::join_all;
use std::sync::Arc;
use std::{collections::HashMap, future::Future, pin::Pin, sync::LazyLock};

#[Signature]
struct BasicGenerateInstruction {
    /// You are an instruction optimizer for large language models. I will give you a ``signature`` of fields (inputs and outputs) in English. Your task is to propose an instruction that will lead a good language model to perform the task well. Don't be afraid to be creative.

    #[input(desc = "The initial instructions before optimization")]
    pub basic_instruction: String,
    #[output(desc = "The improved instructions for the language model")]
    pub proposed_instruction: String,
}

#[Signature]
struct GenerateInstructionGivenAttempts {
    /// You are an instruction optimizer for large language models. I will give some task instructions I've tried, along with their corresponding validation scores. The instructions are arranged in increasing order based on their scores, where higher scores indicate better quality.
    ///
    /// Your task is to propose a new instruction that will lead a good language model to perform the task even better. Don't be afraid to be creative.

    #[input(
        desc = "The instructions I've tried, along with their corresponding validation scores"
    )]
    pub attempted_instructions: Vec<String>,
    #[output(desc = "The improved instructions for the language model")]
    pub proposed_instruction: String,
}

#[derive(Clone)]
struct Candidate {
    pub score: f32,
    pub instruction: String,
    pub prefix: String,
}

#[derive(Clone)]
struct ProgramStats {
    pub results_best: HashMap<String, Vec<f32>>,
    pub results_latest: HashMap<String, Vec<f32>>,
    pub total_calls: usize,
}

#[derive(Builder)]
pub struct COPRO {
    #[builder(default = 10)]
    pub breadth: usize,
    #[builder(default = 3)]
    pub depth: usize,
    #[builder(default = 1.4)]
    pub init_temperature: f32,
    #[builder(default = false)]
    pub track_stats: bool,
    pub prompt_model: Option<LM>,
}

static BASIC_GENERATOR: LazyLock<Predict> =
    LazyLock::new(|| Predict::new(BasicGenerateInstruction::new()));
static REFINEMENT_GENERATOR: LazyLock<Predict> =
    LazyLock::new(|| Predict::new(GenerateInstructionGivenAttempts::new()));

impl COPRO {
    fn get_output_field_prefix(&self, predictor: &dyn Optimizable) -> String {
        // Get the last output field's prefix/desc
        let output_fields = predictor.get_signature().output_fields();
        if let Some(obj) = output_fields.as_object()
            && let Some((_, field)) = obj.iter().next_back()
            && let Some(desc) = field.get("desc")
        {
            return desc.as_str().unwrap_or("").to_string();
        }
        "".to_string()
    }
}

impl Optimizer for COPRO {
    async fn compile<M: Module + Optimizable + Evaluator>(
        &self,
        module: &mut M,
        trainset: Vec<Example>,
    ) -> Result<()> {
        if self.breadth <= 1 {
            return Err(anyhow::anyhow!("Breadth must be greater than 1"));
        }

        // Collect predictor information first
        let predictor_info: Vec<(String, String, String)> = {
            let named_predictors = module.parameters();
            named_predictors
                .iter()
                .map(|(name, predictor)| {
                    let basic_instruction = predictor.get_signature().instruction();
                    let basic_prefix = self.get_output_field_prefix(*predictor);
                    (name.clone(), basic_instruction, basic_prefix)
                })
                .collect()
        };

        let mut all_candidates: HashMap<String, Vec<(String, String)>> = HashMap::new();
        let mut latest_candidates: HashMap<String, Vec<(String, String)>> = HashMap::new();
        let mut evaluated_candidates: HashMap<String, HashMap<(String, String), Candidate>> =
            HashMap::new();

        let mut stats = ProgramStats {
            results_best: HashMap::new(),
            results_latest: HashMap::new(),
            total_calls: 0,
        };

        // Seed with initial instructions - generate breadth-1 new + 1 original
        for (predictor_name, basic_instruction, basic_prefix) in &predictor_info {
            let mut candidates = Vec::new();

            // Generate new candidates
            if self.breadth > 1 {
                let mut futures: Vec<Pin<Box<dyn Future<Output = Result<Prediction>> + Send>>> =
                    Vec::new();

                for _ in 0..self.breadth - 1 {
                    let inst = basic_instruction.clone();
                    if let Some(mut prompt_model) = self.prompt_model.clone() {
                        prompt_model.temperature = self.init_temperature;
                        futures.push(Box::pin(async move {
                            BASIC_GENERATOR
                                .forward_with_config(
                                    example! {
                                        "basic_instruction": "input" => inst
                                    },
                                    Arc::new(prompt_model),
                                )
                                .await
                        }));
                    } else {
                        futures.push(Box::pin(async move {
                            BASIC_GENERATOR
                                .forward_with_config(
                                    example! {
                                        "basic_instruction": "input" => inst
                                    },
                                    Arc::clone(&get_lm()),
                                )
                                .await
                        }));
                    }
                }

                let results = join_all(futures).await;
                let predictions = results.into_iter().collect::<Result<Vec<_>>>()?;

                for pred in predictions {
                    let instruction = pred
                        .data
                        .get("proposed_instruction")
                        .and_then(|v| v.as_str())
                        .unwrap_or(basic_instruction)
                        .to_string();
                    let prefix = pred
                        .data
                        .get("proposed_prefix_for_output_field")
                        .and_then(|v| v.as_str())
                        .unwrap_or(basic_prefix)
                        .to_string();
                    candidates.push((instruction, prefix));
                }
            }

            candidates.push((basic_instruction.clone(), basic_prefix.clone()));

            all_candidates.insert(predictor_name.clone(), candidates.clone());
            latest_candidates.insert(predictor_name.clone(), candidates);
            evaluated_candidates.insert(predictor_name.clone(), HashMap::new());

            if self.track_stats {
                stats
                    .results_best
                    .insert(predictor_name.clone(), Vec::new());
                stats
                    .results_latest
                    .insert(predictor_name.clone(), Vec::new());
            }
        }

        // Main optimization loop
        for d in 0..self.depth {
            println!("Iteration Depth: {}/{}", d + 1, self.depth);

            // Evaluate candidates for each predictor
            for (p_i, (predictor_name, _, _)) in predictor_info.iter().enumerate() {
                // Determine which candidates to evaluate
                let candidates_to_eval = if predictor_info.len() > 1 {
                    // Re-evaluate all candidates when multiple predictors
                    all_candidates.get(predictor_name).unwrap().clone()
                } else {
                    // Just evaluate latest candidates
                    latest_candidates.get(predictor_name).unwrap().clone()
                };

                let mut latest_scores = Vec::new();

                for (c_i, (instruction, prefix)) in candidates_to_eval.iter().enumerate() {
                    // Check if already evaluated
                    let key = (instruction.clone(), prefix.clone());

                    let score = if let Some(existing) = evaluated_candidates
                        .get(predictor_name)
                        .and_then(|m| m.get(&key))
                    {
                        // Skip if already evaluated with same or better score
                        existing.score
                    } else {
                        // Update predictor with candidate
                        {
                            let mut module_predictors = module.parameters();
                            if let Some(predictor) = module_predictors.get_mut(predictor_name) {
                                predictor.update_signature_instruction(instruction.clone())?;
                                // Note: We can't update prefix without modifying the signature system
                                // This would require extending MetaSignature trait
                            }
                        }

                        println!(
                            "At Depth {}/{}, Evaluating Prompt Candidate #{}/{} for Predictor {} of {}",
                            d + 1,
                            self.depth,
                            c_i + 1,
                            candidates_to_eval.len(),
                            p_i + 1,
                            predictor_info.len()
                        );

                        // Evaluate
                        let score = module.evaluate(trainset.clone()).await;
                        stats.total_calls += 1;

                        // Store evaluated candidate
                        evaluated_candidates
                            .get_mut(predictor_name)
                            .unwrap()
                            .insert(
                                key,
                                Candidate {
                                    score,
                                    instruction: instruction.clone(),
                                    prefix: prefix.clone(),
                                },
                            );

                        score
                    };

                    // Track latest scores for stats
                    if candidates_to_eval.len() - self.breadth <= c_i {
                        latest_scores.push(score);
                    }
                }

                // Update to best candidate for this predictor
                if let Some(best) = evaluated_candidates.get(predictor_name).and_then(|m| {
                    m.values()
                        .max_by(|a, b| a.score.partial_cmp(&b.score).unwrap())
                }) {
                    {
                        let mut module_predictors = module.parameters();
                        if let Some(predictor) = module_predictors.get_mut(predictor_name) {
                            predictor.update_signature_instruction(best.instruction.clone())?;
                        }
                    }

                    println!(
                        "Updating Predictor {} to best candidate with score {:.3}",
                        predictor_name, best.score
                    );
                }

                // Track stats
                if self.track_stats && !latest_scores.is_empty() {
                    let avg = latest_scores.iter().sum::<f32>() / latest_scores.len() as f32;
                    stats
                        .results_latest
                        .get_mut(predictor_name)
                        .unwrap()
                        .push(avg);

                    // Track best scores
                    let mut best_scores: Vec<f32> = evaluated_candidates
                        .get(predictor_name)
                        .unwrap()
                        .values()
                        .map(|c| c.score)
                        .collect();
                    best_scores.sort_by(|a, b| b.partial_cmp(a).unwrap());
                    best_scores.truncate(10);

                    if !best_scores.is_empty() {
                        let best_avg = best_scores.iter().sum::<f32>() / best_scores.len() as f32;
                        stats
                            .results_best
                            .get_mut(predictor_name)
                            .unwrap()
                            .push(best_avg);
                    }
                }
            }

            // Skip generation on last iteration
            if d == self.depth - 1 {
                break;
            }

            // Generate new candidates based on attempts
            let mut new_latest_candidates = HashMap::new();

            for (predictor_name, _, _) in &predictor_info {
                // Build few-shot examples from best attempts
                let mut attempts_list = Vec::new();
                let mut best_candidates: Vec<_> = evaluated_candidates
                    .get(predictor_name)
                    .unwrap()
                    .values()
                    .cloned()
                    .collect();
                best_candidates.sort_by(|a, b| a.score.partial_cmp(&b.score).unwrap());

                // Take up to breadth best candidates
                let num_examples = std::cmp::min(self.breadth, best_candidates.len());
                for (i, candidate) in best_candidates.iter().take(num_examples).enumerate() {
                    attempts_list.push(format!(
                        "Instruction #{}: {}",
                        i + 1,
                        candidate.instruction
                    ));
                    attempts_list.push(format!("Prefix #{}: {}", i + 1, candidate.prefix));
                    attempts_list.push(format!(
                        "Resulting Score #{}: {:.3}",
                        i + 1,
                        candidate.score
                    ));
                }

                let attempts_str = attempts_list.join("\n");

                // Generate new candidates
                let results = if let Some(mut prompt_model) = self.prompt_model.clone() {
                    prompt_model.temperature = self.init_temperature;
                    let attempts = attempts_str.clone();

                    REFINEMENT_GENERATOR
                        .batch_with_config(
                            (0..self.breadth)
                                .map(|_| {
                                    example! {
                                        "attempted_instructions": "input" => attempts.clone()
                                    }
                                })
                                .collect(),
                            Arc::new(prompt_model),
                        )
                        .await
                } else {
                    let attempts = attempts_str.clone();
                    REFINEMENT_GENERATOR
                        .batch_with_config(
                            (0..self.breadth)
                                .map(|_| {
                                    example! {
                                        "attempted_instructions": "input" => attempts.clone()
                                    }
                                })
                                .collect(),
                            Arc::clone(&get_lm()),
                        )
                        .await
                };

                if let Ok(predictions) = results {
                    let mut new_candidates = Vec::new();

                    for pred in predictions {
                        // Handle both single and multiple completions
                        let instructions = if let Some(arr) = pred
                            .data
                            .get("proposed_instruction")
                            .and_then(|v| v.as_array())
                        {
                            arr.iter()
                                .filter_map(|v| v.as_str())
                                .map(|s| s.to_string())
                                .collect()
                        } else if let Some(s) = pred
                            .data
                            .get("proposed_instruction")
                            .and_then(|v| v.as_str())
                        {
                            vec![s.to_string()]
                        } else {
                            vec![]
                        };

                        let prefixes = if let Some(arr) = pred
                            .data
                            .get("proposed_prefix_for_output_field")
                            .and_then(|v| v.as_array())
                        {
                            arr.iter()
                                .filter_map(|v| v.as_str())
                                .map(|s| s.to_string())
                                .collect()
                        } else if let Some(s) = pred
                            .data
                            .get("proposed_prefix_for_output_field")
                            .and_then(|v| v.as_str())
                        {
                            vec![s.to_string()]
                        } else {
                            vec![]
                        };

                        for (inst, pref) in instructions.iter().zip(prefixes.iter()) {
                            new_candidates.push((inst.clone(), pref.clone()));
                        }
                    }

                    // Add to all candidates
                    all_candidates
                        .get_mut(predictor_name)
                        .unwrap()
                        .extend(new_candidates.clone());
                    new_latest_candidates.insert(predictor_name.clone(), new_candidates);
                }
            }

            latest_candidates = new_latest_candidates;
        }

        // Find best overall candidate and update module
        let mut best_overall: Option<(String, Candidate)> = None;

        for (predictor_name, candidates_map) in &evaluated_candidates {
            if let Some(best) = candidates_map
                .values()
                .max_by(|a, b| a.score.partial_cmp(&b.score).unwrap())
                && (best_overall.is_none() || best.score > best_overall.as_ref().unwrap().1.score)
            {
                best_overall = Some((predictor_name.clone(), best.clone()));
            }
        }

        // Update original module with best candidates
        if let Some((_, best_candidate)) = best_overall {
            let module_predictors = module.parameters();
            for (predictor_name, predictor) in module_predictors {
                if let Some(best) = evaluated_candidates.get(&predictor_name).and_then(|m| {
                    m.values()
                        .max_by(|a, b| a.score.partial_cmp(&b.score).unwrap())
                }) {
                    predictor.update_signature_instruction(best.instruction.clone())?;
                }
            }

            if self.track_stats {
                println!("\n=== Optimization Complete ===");
                println!("Total calls: {}", stats.total_calls);
                println!("Best score: {:.3}", best_candidate.score);
                println!("Best instruction: {}", best_candidate.instruction);
                if !best_candidate.prefix.is_empty() {
                    println!("Best prefix: {}", best_candidate.prefix);
                }
            }
        }

        Ok(())
    }
}
