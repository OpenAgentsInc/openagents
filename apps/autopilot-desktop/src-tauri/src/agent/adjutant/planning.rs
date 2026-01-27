//! Plan Mode Pipeline for Adjutant Agent
//!
//! Implements the core planning workflow: topic decomposition → parallel exploration → synthesis

use super::config::{PlanModeConfig, PlanModeOptimizationConfig};
use super::lm_config::{build_dsrs_lm, load_ai_gateway_config};
use super::plan_mode_optimizer::{load_latest_instruction, run_plan_mode_optimization};
use super::plan_mode_signatures::PlanModeSignatureKind;
use super::plan_mode_training::{
    ComplexityClassificationExample, DeepPlanningExample, ParallelExplorationExample,
    PlanModeTrace, PlanModeTrainingStore, PlanSynthesisExample, ResultValidationExample,
    TopicDecompositionExample,
};
use dsrs::core::MetaSignature;
use dsrs::signatures::{
    ComplexityClassificationSignature, DeepPlanningSignature, ExplorationTopic,
    ParallelExplorationSignature, PlanSynthesisSignature, ResultValidationSignature,
    TopicDecompositionSignature, TopicsResponse,
};
use dsrs::{LM, Predict, Predictor, example};
use futures::future::join_all;
use serde_json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::process::Command;
use tracing::{debug, error, info, warn};

// ============================================================================
// Pipeline Implementation
// ============================================================================

/// Result from a single exploration agent
#[derive(Debug, Clone)]
pub struct ExplorationResult {
    pub topic: String,
    pub focus: String,
    pub files_examined: Vec<String>,
    pub key_findings: String,
}

fn apply_optimized_instruction<S: MetaSignature>(
    mut signature: S,
    kind: PlanModeSignatureKind,
    config: &PlanModeOptimizationConfig,
) -> S {
    if config.apply_optimized_instructions
        && let Some(instruction) = load_latest_instruction(kind)
            && let Err(err) = signature.update_instruction(instruction) {
                warn!(
                    "Failed to apply optimized instruction for {}: {}",
                    kind.name(),
                    err
                );
            }
    signature
}

/// Final plan result
#[derive(Debug, Clone)]
pub struct PlanResult {
    pub implementation_plan: String,
    pub topics_explored: Vec<String>,
    pub files_examined: Vec<String>,
    #[expect(dead_code)]
    pub confidence: f32,
}

/// Plan Mode Pipeline - orchestrates the full planning workflow
pub struct PlanModePipeline {
    config: PlanModeConfig,
    repo_path: PathBuf,
    // DSPy predictors for each signature
    topic_decomposer: Predict,
    synthesis_predictor: Predict,
    complexity_classifier: Predict,
    deep_planner: Option<Predict>,
    validator: Predict,
    // Language Model
    lm: Option<Arc<LM>>,
}

impl PlanModePipeline {
    pub fn new(repo_path: PathBuf, config: PlanModeConfig) -> Self {
        // Initialize DSPy predictors with each signature
        let topic_decomposer = Predict::new(apply_optimized_instruction(
            TopicDecompositionSignature::new(),
            PlanModeSignatureKind::TopicDecomposition,
            &config.optimization,
        ));
        let synthesis_predictor = Predict::new(apply_optimized_instruction(
            PlanSynthesisSignature::new(),
            PlanModeSignatureKind::PlanSynthesis,
            &config.optimization,
        ));
        let complexity_classifier = Predict::new(apply_optimized_instruction(
            ComplexityClassificationSignature::new(),
            PlanModeSignatureKind::ComplexityClassification,
            &config.optimization,
        ));
        let validator = Predict::new(apply_optimized_instruction(
            ResultValidationSignature::new(),
            PlanModeSignatureKind::ResultValidation,
            &config.optimization,
        ));

        let deep_planner = if config.enable_deep_planning {
            Some(Predict::new(apply_optimized_instruction(
                DeepPlanningSignature::new(),
                PlanModeSignatureKind::DeepPlanning,
                &config.optimization,
            )))
        } else {
            None
        };

        Self {
            config,
            repo_path,
            topic_decomposer,
            synthesis_predictor,
            complexity_classifier,
            deep_planner,
            validator,
            lm: None,
        }
    }

    /// Set the language model for DSPy predictions
    #[expect(dead_code)]
    pub fn with_lm(mut self, lm: Arc<LM>) -> Self {
        self.lm = Some(lm);
        self
    }

    /// Create pipeline with automatic LM configuration
    pub async fn with_auto_lm(mut self) -> Self {
        if let Ok(config) = load_ai_gateway_config() {
            let server_url = config.server_url();
            match build_dsrs_lm(&config).await {
                Ok(lm) => {
                    info!(
                        "✅ AI Gateway LM initialized and connected to {}",
                        server_url
                    );
                    self.lm = Some(Arc::new(lm));
                }
                Err(err) => {
                    warn!("Failed to initialize AI Gateway LM: {}", err);
                }
            }
        }
        self
    }

    /// Execute the full plan mode pipeline
    pub async fn execute_plan_mode(&self, user_prompt: &str) -> Result<PlanResult, String> {
        let mut trace = PlanModeTrace::default();

        // 1. Get repository context
        let file_tree = self.get_file_tree().await?;

        // 2. Classify complexity
        let complexity = self
            .classify_complexity(user_prompt, &file_tree, &mut trace)
            .await?;

        // 3. Topic decomposition
        let topics = self
            .decompose_into_topics(user_prompt, &file_tree, &mut trace)
            .await?;

        // 4. Parallel exploration
        let exploration_results = self.run_parallel_exploration(topics, &mut trace).await?;

        // 5. Plan synthesis
        let plan = if complexity > self.config.deep_planning_threshold {
            self.deep_plan_synthesis(user_prompt, &exploration_results, &mut trace)
                .await?
        } else {
            self.synthesize_plan(user_prompt, &exploration_results, &mut trace)
                .await?
        };

        // 6. Validation (if enabled)
        if self.config.enable_validation {
            self.validate_plan(user_prompt, &plan, &mut trace).await?;
        }

        self.persist_training(trace).await;

        Ok(plan)
    }

    /// Topic Decomposition - break user prompt into 2-4 exploration topics
    async fn decompose_into_topics(
        &self,
        user_prompt: &str,
        file_tree: &str,
        trace: &mut PlanModeTrace,
    ) -> Result<Vec<ExplorationTopic>, String> {
        info!("Decomposing prompt into exploration topics...");

        if let Some(lm) = &self.lm {
            let inputs = example! {
                "user_prompt": "input" => user_prompt.to_string(),
                "file_tree": "input" => file_tree.to_string(),
            };

            let result = self
                .topic_decomposer
                .forward_with_config(inputs, Arc::clone(lm))
                .await
                .map_err(|e| format!("DSPy decomposition failed: {}", e))?;

            let topics_json = result
                .data
                .get("topics")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'topics' output")?;

            let topics_response: TopicsResponse = Self::parse_json_from_output(topics_json)?;

            if self.config.optimization.record_training {
                let serialized_topics = serde_json::to_string(&topics_response.topics)
                    .unwrap_or_else(|_| topics_json.to_string());
                if topics_response.topics.len() >= 2 && topics_response.topics.len() <= 4 {
                    trace.topic_decomposition = Some(TopicDecompositionExample {
                        user_prompt: user_prompt.to_string(),
                        file_tree: file_tree.to_string(),
                        topics: serialized_topics,
                    });
                }
            }

            return Ok(topics_response.topics);
        }

        // Mock decomposition as fallback
        Ok(vec![
            ExplorationTopic {
                name: "Core Architecture".to_string(),
                focus: "Understand the main application structure and patterns".to_string(),
                patterns: vec!["main".to_string(), "app".to_string(), "mod".to_string()],
            },
            ExplorationTopic {
                name: "Data Layer".to_string(),
                focus: "Examine data models and database interactions".to_string(),
                patterns: vec!["model".to_string(), "db".to_string(), "data".to_string()],
            },
        ])
    }

    /// Parallel Exploration
    async fn run_parallel_exploration(
        &self,
        topics: Vec<ExplorationTopic>,
        trace: &mut PlanModeTrace,
    ) -> Result<Vec<ExplorationResult>, String> {
        info!("Launching {} explore agents...", topics.len());
        struct ExplorationRun {
            result: ExplorationResult,
            training: Option<ParallelExplorationExample>,
        }

        let mut exploration_tasks = Vec::new();

        for (i, topic) in topics.into_iter().enumerate() {
            let agent_num = i + 1;
            let lm = self.lm.clone();
            let repo_path = self.repo_path.clone();
            let max_files = self.config.max_tool_calls_per_agent.max(3);

            exploration_tasks.push(async move {
                debug!("[Agent {}] Starting: {}", agent_num, topic.name);

                let (files_examined, file_context) =
                    Self::gather_exploration_context(&repo_path, &topic.patterns, max_files)
                        .await?;

                if let Some(lm) = lm {
                    let predictor = Predict::new(apply_optimized_instruction(
                        ParallelExplorationSignature::new(),
                        PlanModeSignatureKind::ParallelExploration,
                        &self.config.optimization,
                    ));
                    let inputs = example! {
                        "topic": "input" => topic.name.clone(),
                        "focus": "input" => topic.focus.clone(),
                        "patterns": "input" => serde_json::to_string(&topic.patterns).unwrap_or_default(),
                        "repo_path": "input" => repo_path.to_string_lossy().to_string(),
                        "file_context": "input" => file_context.clone(),
                    };

                    let result = predictor.forward_with_config(inputs, lm).await;

                    match result {
                        Ok(prediction) => {
                            let findings = prediction
                                .data
                                .get("findings")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let files_from_model = prediction
                                .data
                                .get("files_examined")
                                .and_then(|v| v.as_str())
                                .and_then(|raw| serde_json::from_str::<Vec<String>>(raw).ok())
                                .unwrap_or_default();

                            let training = if !findings.trim().is_empty() && !files_from_model.is_empty() {
                                Some(ParallelExplorationExample {
                                    topic: topic.name.clone(),
                                    focus: topic.focus.clone(),
                                    patterns: serde_json::to_string(&topic.patterns).unwrap_or_default(),
                                    repo_path: repo_path.to_string_lossy().to_string(),
                                    file_context: file_context.clone(),
                                    findings: findings.clone(),
                                    files_examined: serde_json::to_string(&files_from_model).unwrap_or_default(),
                                })
                            } else {
                                None
                            };

                            let files_examined = if files_examined.is_empty() {
                                files_from_model
                            } else {
                                files_examined
                            };

                            Ok(ExplorationRun {
                                result: ExplorationResult {
                                    topic: topic.name,
                                    focus: topic.focus,
                                    files_examined,
                                    key_findings: findings,
                                },
                                training,
                            })
                        }
                        Err(e) => Err(e.to_string()),
                    }
                } else {
                    let summary = Self::fallback_findings(&topic, &files_examined);
                    Ok(ExplorationRun {
                        result: ExplorationResult {
                            topic: topic.name,
                            focus: topic.focus,
                            files_examined,
                            key_findings: summary,
                        },
                        training: None,
                    })
                }
            });
        }

        let results = join_all(exploration_tasks).await;
        let mut exploration_results = Vec::new();
        for run in results.into_iter().flatten() {
            exploration_results.push(run.result);
            if self.config.optimization.record_training
                && let Some(example) = run.training {
                    trace.parallel_exploration.push(example);
                }
        }

        Ok(exploration_results)
    }

    /// Plan Synthesis
    async fn synthesize_plan(
        &self,
        user_prompt: &str,
        exploration_results: &[ExplorationResult],
        trace: &mut PlanModeTrace,
    ) -> Result<PlanResult, String> {
        info!("Synthesizing plan from exploration...");

        let combined_findings = Self::format_exploration_results(exploration_results);
        let all_files: Vec<String> = exploration_results
            .iter()
            .flat_map(|r| r.files_examined.clone())
            .collect();

        if let Some(lm) = &self.lm {
            let inputs = example! {
                "user_prompt": "input" => user_prompt.to_string(),
                "exploration_results": "input" => combined_findings.clone(),
                "repo_context": "input" => format!("Files examined: {}", all_files.join(", ")),
            };

            let result = self
                .synthesis_predictor
                .forward_with_config(inputs, Arc::clone(lm))
                .await
                .map_err(|e| format!("Plan synthesis failed: {}", e))?;

            let implementation_plan = result
                .data
                .get("implementation_plan")
                .and_then(|v| v.as_str())
                .unwrap_or("Failed to generate plan")
                .to_string();

            if self.config.optimization.record_training && implementation_plan.len() >= 60 {
                trace.plan_synthesis = Some(PlanSynthesisExample {
                    user_prompt: user_prompt.to_string(),
                    exploration_results: combined_findings.clone(),
                    repo_context: format!("Files examined: {}", all_files.join(", ")),
                    implementation_plan: implementation_plan.clone(),
                });
            }

            return Ok(PlanResult {
                implementation_plan,
                topics_explored: exploration_results
                    .iter()
                    .map(|r| r.topic.clone())
                    .collect(),
                files_examined: all_files,
                confidence: 0.9,
            });
        }

        Ok(PlanResult {
            implementation_plan: format!("Mock plan for: {}", user_prompt),
            topics_explored: exploration_results
                .iter()
                .map(|r| r.topic.clone())
                .collect(),
            files_examined: all_files,
            confidence: 0.5,
        })
    }

    /// Deep Planning
    async fn deep_plan_synthesis(
        &self,
        user_prompt: &str,
        exploration_results: &[ExplorationResult],
        trace: &mut PlanModeTrace,
    ) -> Result<PlanResult, String> {
        if let Some(lm) = &self.lm
            && let Some(planner) = &self.deep_planner {
                let inputs = example! {
                    "complex_request": "input" => user_prompt.to_string(),
                    "codebase_analysis": "input" => Self::format_exploration_results(exploration_results),
                    "constraints": "input" => "Follow project conventions and ensure type safety.".to_string(),
                };

                let result = planner
                    .forward_with_config(inputs, Arc::clone(lm))
                    .await
                    .map_err(|e| format!("Deep planning failed: {}", e))?;

                let implementation_plan = result
                    .data
                    .get("implementation_plan")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Failed to generate deep plan")
                    .to_string();

                if self.config.optimization.record_training && implementation_plan.len() >= 80 {
                    let reasoning = result
                        .data
                        .get("reasoning")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let strategy = result
                        .data
                        .get("strategy")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let risk_assessment = result
                        .data
                        .get("risk_assessment")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    trace.deep_planning = Some(DeepPlanningExample {
                        complex_request: user_prompt.to_string(),
                        codebase_analysis: Self::format_exploration_results(exploration_results),
                        constraints: "Follow project conventions and ensure type safety."
                            .to_string(),
                        reasoning,
                        strategy,
                        implementation_plan: implementation_plan.clone(),
                        risk_assessment,
                    });
                }

                return Ok(PlanResult {
                    implementation_plan,
                    topics_explored: exploration_results
                        .iter()
                        .map(|r| r.topic.clone())
                        .collect(),
                    files_examined: Vec::new(),
                    confidence: 0.95,
                });
            }
        self.synthesize_plan(user_prompt, exploration_results, trace)
            .await
    }

    /// Complexity Classification
    async fn classify_complexity(
        &self,
        user_prompt: &str,
        file_tree: &str,
        trace: &mut PlanModeTrace,
    ) -> Result<f32, String> {
        if let Some(lm) = &self.lm {
            let repo_indicators = format!("File tree lines: {}", file_tree.lines().count());
            let domain_signals = "N/A".to_string();

            let inputs = example! {
                "task_description": "input" => user_prompt.to_string(),
                "repo_indicators": "input" => repo_indicators.clone(),
                "domain_signals": "input" => domain_signals.clone(),
            };

            let result = self
                .complexity_classifier
                .forward_with_config(inputs, Arc::clone(lm))
                .await
                .map_err(|e| format!("Complexity classification failed: {}", e))?;

            let complexity_str = result
                .data
                .get("complexity")
                .and_then(|v| v.as_str())
                .unwrap_or("Medium");

            if self.config.optimization.record_training {
                let routing_decision = result
                    .data
                    .get("routing_decision")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let reasoning = result
                    .data
                    .get("reasoning")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                if !routing_decision.is_empty() && !reasoning.is_empty() {
                    trace.complexity_classification = Some(ComplexityClassificationExample {
                        task_description: user_prompt.to_string(),
                        repo_indicators,
                        domain_signals,
                        complexity: complexity_str.to_string(),
                        routing_decision,
                        reasoning,
                    });
                }
            }

            match complexity_str.to_lowercase().as_str() {
                "low" => Ok(0.2),
                "medium" => Ok(0.5),
                "high" => Ok(0.8),
                "veryhigh" | "very high" => Ok(1.0),
                _ => Ok(0.5),
            }
        } else {
            Ok(0.5)
        }
    }

    /// Get repository file tree
    async fn get_file_tree(&self) -> Result<String, String> {
        let mut tree = String::new();
        tree.push_str(&format!("Repository: {}\n", self.repo_path.display()));
        let mut entries = Vec::new();
        self.collect_files(&self.repo_path, &mut entries, 0, 100)
            .await?;
        for (name, depth) in entries {
            tree.push_str(&format!("{}• {}\n", "  ".repeat(depth), name));
        }
        Ok(tree)
    }

    #[async_recursion::async_recursion]
    async fn collect_files(
        &self,
        path: &Path,
        entries: &mut Vec<(String, usize)>,
        depth: usize,
        max: usize,
    ) -> Result<(), String> {
        if entries.len() >= max || depth > 3 {
            return Ok(());
        }
        let mut read_dir = tokio::fs::read_dir(path).await.map_err(|e| e.to_string())?;
        while let Ok(Some(entry)) = read_dir.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.')
                || ["node_modules", "target", "dist", ".git"].contains(&name.as_str())
            {
                continue;
            }
            let rel = entry
                .path()
                .strip_prefix(&self.repo_path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or(name);
            entries.push((rel, depth));
            if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
                self.collect_files(&entry.path(), entries, depth + 1, max)
                    .await?;
            }
            if entries.len() >= max {
                break;
            }
        }
        Ok(())
    }

    /// Plan Validation
    async fn validate_plan(
        &self,
        user_prompt: &str,
        plan: &PlanResult,
        trace: &mut PlanModeTrace,
    ) -> Result<(), String> {
        info!("Validating plan quality...");

        if let Some(lm) = &self.lm {
            let inputs = example! {
                "original_request": "input" => user_prompt.to_string(),
                "generated_output": "input" => plan.implementation_plan.clone(),
                "criteria": "input" => "Completeness, feasibility, and adherence to requirements.".to_string(),
            };

            let result = self
                .validator
                .forward_with_config(inputs, Arc::clone(lm))
                .await
                .map_err(|e| format!("Validation failed: {}", e))?;

            let issues = result
                .data
                .get("issues")
                .and_then(|v| v.as_str())
                .unwrap_or("None");
            let confidence_str = result
                .data
                .get("confidence")
                .and_then(|v| v.as_str())
                .unwrap_or("0.8");
            let confidence = confidence_str.parse::<f32>().unwrap_or(0.8);

            if self.config.optimization.record_training && confidence_str.parse::<f32>().is_ok() {
                let quality_assessment = result
                    .data
                    .get("quality_assessment")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                trace.result_validation = Some(ResultValidationExample {
                    original_request: user_prompt.to_string(),
                    generated_output: plan.implementation_plan.clone(),
                    criteria: "Completeness, feasibility, and adherence to requirements."
                        .to_string(),
                    quality_assessment,
                    issues: issues.to_string(),
                    confidence: confidence_str.to_string(),
                });
            }

            info!(
                "Validation result: issues={}, confidence={}",
                issues, confidence
            );

            if confidence < 0.5 {
                return Err(format!("Plan validation failed: {}", issues));
            }
        }

        Ok(())
    }

    async fn persist_training(&self, trace: PlanModeTrace) {
        if !self.config.optimization.record_training {
            return;
        }
        if trace.is_empty() {
            return;
        }

        let mut store = match PlanModeTrainingStore::load() {
            Ok(store) => store,
            Err(err) => {
                warn!("Failed to load plan mode training store: {}", err);
                return;
            }
        };

        store.append_trace(trace, self.config.optimization.max_examples);
        if let Err(err) = store.save() {
            warn!("Failed to save plan mode training store: {}", err);
        }

        if !self.config.optimization.enabled {
            return;
        }

        let Some(lm) = self.lm.clone() else {
            warn!("Optimization skipped: no LM available");
            return;
        };

        let config = self.config.optimization.clone();
        if config.background_optimization {
            tokio::task::spawn_blocking(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build();
                match runtime {
                    Ok(runtime) => {
                        if let Err(err) = runtime.block_on(run_plan_mode_optimization(config, lm)) {
                            error!("Plan mode optimization failed: {}", err);
                        }
                    }
                    Err(err) => {
                        error!("Plan mode optimization runtime init failed: {}", err);
                    }
                }
            });
        } else if let Err(err) = run_plan_mode_optimization(config, lm).await {
            error!("Plan mode optimization failed: {}", err);
        }
    }

    fn parse_json_from_output<T: serde::de::DeserializeOwned>(output: &str) -> Result<T, String> {
        let json_str = if let Some(start) = output.find("```json") {
            let after_start = &output[start + 7..];
            after_start.split("```").next().unwrap_or(after_start)
        } else if let Some(start) = output.find('{') {
            &output[start..output.rfind('}').map_or(output.len(), |i| i + 1)]
        } else {
            output
        };
        serde_json::from_str(json_str.trim()).map_err(|e| format!("JSON error: {}", e))
    }

    fn format_exploration_results(results: &[ExplorationResult]) -> String {
        let mut output = String::new();
        for result in results {
            use std::fmt::Write;
            let _ = writeln!(
                output,
                "## Topic: {}\nFocus: {}\nFiles: {}\nFindings:\n{}\n",
                result.topic,
                result.focus,
                result.files_examined.join(", "),
                result.key_findings
            );
            output.push('\n');
        }
        output
    }

    async fn gather_exploration_context(
        repo_path: &Path,
        patterns: &[String],
        max_files: usize,
    ) -> Result<(Vec<String>, String), String> {
        let mut files = Vec::new();
        for pattern in patterns {
            if files.len() >= max_files {
                break;
            }
            let mut matches = match Self::rg_files_for_pattern(repo_path, pattern).await {
                Ok(matches) => matches,
                Err(err) => {
                    warn!("Pattern search failed for '{}': {}", pattern, err);
                    Vec::new()
                }
            };
            matches.retain(|path| !files.contains(path));
            files.extend(matches);
        }

        if files.is_empty() {
            files = match Self::rg_list_repo_files(repo_path, max_files).await {
                Ok(list) => list,
                Err(err) => {
                    warn!("rg --files failed: {}, falling back to manual listing", err);
                    Self::list_repo_files_fallback(repo_path, max_files).await?
                }
            };
        }

        let mut context = String::new();
        for file in files.iter().take(max_files) {
            let file_path = repo_path.join(file);
            if let Ok(bytes) = tokio::fs::read(&file_path).await {
                if bytes.contains(&0) {
                    continue;
                }
                let text = String::from_utf8_lossy(&bytes);
                context.push_str(&format!("FILE: {}\n", file));
                context.push_str(&Self::truncate_text(&text, 4000));
                context.push_str("\n\n");
            }
        }

        Ok((files, context))
    }

    async fn rg_files_for_pattern(repo_path: &Path, pattern: &str) -> Result<Vec<String>, String> {
        let output = Command::new("rg")
            .arg("-l")
            .arg("-i")
            .arg("--no-messages")
            .arg(pattern)
            .current_dir(repo_path)
            .output()
            .await;

        match output {
            Ok(output) if output.status.success() || output.status.code() == Some(1) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                Ok(stdout
                    .lines()
                    .map(|line| line.trim().to_string())
                    .filter(|line| !line.is_empty())
                    .collect())
            }
            Ok(output) => Err(format!(
                "rg failed for pattern '{}': {}",
                pattern,
                String::from_utf8_lossy(&output.stderr)
            )),
            Err(e) => Err(format!("rg failed to run: {}", e)),
        }
    }

    async fn rg_list_repo_files(repo_path: &Path, max_files: usize) -> Result<Vec<String>, String> {
        let output = Command::new("rg")
            .arg("--files")
            .arg("--no-messages")
            .current_dir(repo_path)
            .output()
            .await;

        match output {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                Ok(stdout
                    .lines()
                    .map(|line| line.trim().to_string())
                    .filter(|line| !line.is_empty())
                    .take(max_files)
                    .collect())
            }
            Ok(output) => Err(format!(
                "rg --files failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )),
            Err(e) => Err(format!("rg --files failed to run: {}", e)),
        }
    }

    async fn list_repo_files_fallback(
        repo_path: &Path,
        max_files: usize,
    ) -> Result<Vec<String>, String> {
        let mut results = Vec::new();
        let mut stack = vec![repo_path.to_path_buf()];

        while let Some(dir) = stack.pop() {
            if results.len() >= max_files {
                break;
            }
            let mut read_dir = tokio::fs::read_dir(&dir).await.map_err(|e| e.to_string())?;
            while let Ok(Some(entry)) = read_dir.next_entry().await {
                let file_type = entry.file_type().await.map_err(|e| e.to_string())?;
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.')
                    || ["node_modules", "target", "dist", ".git"].contains(&name.as_str())
                {
                    continue;
                }

                if file_type.is_dir() {
                    stack.push(entry.path());
                    continue;
                }

                let rel = entry
                    .path()
                    .strip_prefix(repo_path)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or(name);
                results.push(rel);
                if results.len() >= max_files {
                    break;
                }
            }
        }

        Ok(results)
    }

    fn truncate_text(text: &str, max_chars: usize) -> String {
        text.chars().take(max_chars).collect()
    }

    fn fallback_findings(topic: &ExplorationTopic, files_examined: &[String]) -> String {
        if files_examined.is_empty() {
            format!(
                "No matching files found for patterns: {}. Consider adjusting search terms.",
                topic.patterns.join(", ")
            )
        } else {
            format!(
                "Examined {} files for patterns [{}]. Summarize key areas manually if needed.",
                files_examined.len(),
                topic.patterns.join(", ")
            )
        }
    }
}
