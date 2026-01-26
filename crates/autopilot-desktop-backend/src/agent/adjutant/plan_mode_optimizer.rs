//! Plan mode optimization loop for dsrs signatures.

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use dsrs::evaluate::FeedbackEvaluator;
use dsrs::manifest::{CompiledModuleManifest, Scorecard};
use dsrs::optimizer::{COPRO, GEPA, MIPROv2, Optimizer};
use dsrs::{Evaluator, Example, LM, Module, Optimizable, Predict, Prediction};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::config::{PlanModeOptimizationConfig, PlanModeOptimizerKind};
use super::plan_mode_metrics::{feedback_signature, score_signature};
use super::plan_mode_signatures::{sanitize_filename, PlanModeSignatureKind};
use super::plan_mode_training::PlanModeTrainingStore;
use dsrs::signatures::{
    ComplexityClassificationSignature, DeepPlanningSignature, ParallelExplorationSignature,
    PlanSynthesisSignature, ResultValidationSignature, TopicDecompositionSignature,
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlanModeOptimizationState {
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_signature_index: usize,
}

impl PlanModeOptimizationState {
    pub fn load() -> Self {
        let path = state_path();
        if let Ok(contents) = fs::read_to_string(&path) {
            serde_json::from_str(&contents).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = state_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let contents = serde_json::to_string_pretty(self)?;
        fs::write(path, contents)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
struct PlanModeOptimizationEvent {
    timestamp: DateTime<Utc>,
    signature: String,
    optimizer: String,
    examples: usize,
    baseline_score: Option<f32>,
    optimized_score: Option<f32>,
    improvement: Option<f32>,
    status: String,
    notes: Option<String>,
}

pub async fn run_plan_mode_optimization(
    config: PlanModeOptimizationConfig,
    lm: Arc<LM>,
) -> Result<()> {
    if !config.enabled {
        return Ok(());
    }

    let mut state = PlanModeOptimizationState::load();
    if !should_run(&config, &state) {
        return Ok(());
    }

    let store = PlanModeTrainingStore::load()?;
    let signatures = select_signatures(&config, &store, &mut state);

    if signatures.is_empty() {
        return Ok(());
    }

    let logger = PlanModeOptimizationLogger::new(config.log_benchmarks);

    for signature in signatures {
        if store.example_count(signature) < config.min_examples {
            continue;
        }
        let examples = store.examples_for_signature(signature);
        let minibatch = select_minibatch(&examples, config.minibatch_size);
        if minibatch.is_empty() {
            continue;
        }

        let baseline = benchmark_signature(signature, &minibatch, Arc::clone(&lm))
            .await
            .context("benchmark failed")?;

        if config.benchmark_only {
            logger.log(PlanModeOptimizationEvent {
                timestamp: Utc::now(),
                signature: signature.name().to_string(),
                optimizer: optimizer_label(&config.optimizer).to_string(),
                examples: examples.len(),
                baseline_score: Some(baseline),
                optimized_score: None,
                improvement: None,
                status: "benchmark_only".to_string(),
                notes: None,
            })?;
            continue;
        }

        let result = optimize_signature(
            signature,
            &examples,
            &minibatch,
            baseline,
            &config,
            Arc::clone(&lm),
        )
        .await;

        match result {
            Ok(optimized) => {
                logger.log(PlanModeOptimizationEvent {
                    timestamp: Utc::now(),
                    signature: signature.name().to_string(),
                    optimizer: optimizer_label(&config.optimizer).to_string(),
                    examples: examples.len(),
                    baseline_score: Some(baseline),
                    optimized_score: Some(optimized),
                    improvement: Some(optimized - baseline),
                    status: "optimized".to_string(),
                    notes: None,
                })?;
            }
            Err(err) => {
                logger.log(PlanModeOptimizationEvent {
                    timestamp: Utc::now(),
                    signature: signature.name().to_string(),
                    optimizer: optimizer_label(&config.optimizer).to_string(),
                    examples: examples.len(),
                    baseline_score: Some(baseline),
                    optimized_score: None,
                    improvement: None,
                    status: "failed".to_string(),
                    notes: Some(err.to_string()),
                })?;
            }
        }
    }

    state.last_run_at = Some(Utc::now());
    state.save()?;
    Ok(())
}

pub fn load_latest_instruction(signature: PlanModeSignatureKind) -> Option<String> {
    let path = latest_manifest_path(signature);
    let contents = fs::read_to_string(path).ok()?;
    let manifest: CompiledModuleManifest = serde_json::from_str(&contents).ok()?;
    manifest.instruction
}

fn should_run(config: &PlanModeOptimizationConfig, state: &PlanModeOptimizationState) -> bool {
    let Some(last_run) = state.last_run_at else {
        return true;
    };
    let hours_since = (Utc::now() - last_run).num_hours();
    hours_since >= config.min_hours_between_runs as i64
}

fn select_signatures(
    config: &PlanModeOptimizationConfig,
    store: &PlanModeTrainingStore,
    state: &mut PlanModeOptimizationState,
) -> Vec<PlanModeSignatureKind> {
    let mut eligible: Vec<PlanModeSignatureKind> = PlanModeSignatureKind::ALL
        .iter()
        .copied()
        .filter(|kind| store.example_count(*kind) >= config.min_examples)
        .collect();

    if config.optimize_all_signatures {
        return eligible;
    }

    if eligible.is_empty() {
        return Vec::new();
    }

    eligible.sort_by_key(|kind| store.example_count(*kind));

    let start = state.last_signature_index % eligible.len();
    let mut ordered = Vec::new();
    ordered.extend_from_slice(&eligible[start..]);
    ordered.extend_from_slice(&eligible[..start]);

    let limit = config.max_signatures_per_run.min(ordered.len());
    state.last_signature_index = start + limit;

    ordered.truncate(limit);
    ordered
}

async fn benchmark_signature(
    signature: PlanModeSignatureKind,
    examples: &[Example],
    lm: Arc<LM>,
) -> Result<f32> {
    let instruction = load_latest_instruction(signature);
    let module = PlanModeSignatureModule::new(signature, lm, instruction)?;
    Ok(module.evaluate(examples.to_vec()).await)
}

async fn optimize_signature(
    signature: PlanModeSignatureKind,
    examples: &[Example],
    evaluation_set: &[Example],
    baseline: f32,
    config: &PlanModeOptimizationConfig,
    lm: Arc<LM>,
) -> Result<f32> {
    let instruction = load_latest_instruction(signature);
    let mut module = PlanModeSignatureModule::new(signature, Arc::clone(&lm), instruction)?;

    match config.optimizer {
        PlanModeOptimizerKind::Mipro => {
            let optimizer = MIPROv2::builder()
                .num_candidates(config.num_candidates)
                .num_trials(config.num_trials)
                .minibatch_size(config.minibatch_size.min(examples.len()))
                .temperature(config.temperature)
                .prompt_model(Some((*lm).clone()))
                .build();
            optimizer.compile(&mut module, examples.to_vec()).await?;
        }
        PlanModeOptimizerKind::Copro => {
            let optimizer = COPRO::builder()
                .breadth(config.num_candidates.max(2))
                .depth(config.num_trials.max(1))
                .init_temperature(config.temperature)
                .prompt_model(Some((*lm).clone()))
                .build();
            optimizer.compile(&mut module, examples.to_vec()).await?;
        }
        PlanModeOptimizerKind::Gepa => {
            let optimizer = GEPA::builder()
                .num_iterations(config.num_trials.max(1))
                .minibatch_size(config.minibatch_size.min(examples.len()))
                .num_trials(config.num_trials.max(1))
                .temperature(config.temperature)
                .prompt_model(Some((*lm).clone()))
                .build();
            optimizer
                .compile_with_feedback(&mut module, examples.to_vec())
                .await?;
        }
    }

    let optimized_instruction = module.instruction();
    let optimized_score = module.evaluate(evaluation_set.to_vec()).await;

    if optimized_score >= baseline {
        let manifest = CompiledModuleManifest::new(signature.name(), optimizer_label(&config.optimizer))
            .with_instruction(&optimized_instruction)
            .with_scorecard(
                Scorecard::new(optimized_score)
                    .with_proxy("baseline_score", baseline)
                    .with_rollouts(config.num_trials),
            )
            .finalize()?;
        save_manifest(signature, &manifest)?;
    }

    Ok(optimized_score)
}

fn select_minibatch(examples: &[Example], minibatch_size: usize) -> Vec<Example> {
    if examples.is_empty() {
        return Vec::new();
    }
    let limit = minibatch_size.max(1).min(examples.len());
    examples.iter().take(limit).cloned().collect()
}

fn optimizer_label(kind: &PlanModeOptimizerKind) -> &'static str {
    match kind {
        PlanModeOptimizerKind::Mipro => "MIPROv2",
        PlanModeOptimizerKind::Copro => "COPRO",
        PlanModeOptimizerKind::Gepa => "GEPA",
    }
}

fn save_manifest(signature: PlanModeSignatureKind, manifest: &CompiledModuleManifest) -> Result<()> {
    let dir = manifest_dir();
    fs::create_dir_all(&dir)?;

    let compiled_id = manifest
        .compiled_id
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("manifest missing compiled_id"))?;

    let manifest_path = dir.join(format!("{}.json", sanitize_filename(compiled_id)));
    fs::write(&manifest_path, serde_json::to_string_pretty(manifest)?)?;

    let latest_path = latest_manifest_path(signature);
    fs::write(&latest_path, serde_json::to_string_pretty(manifest)?)?;

    Ok(())
}

fn manifest_dir() -> PathBuf {
    let home = dirs::home_dir().expect("No home directory");
    home.join(".openagents")
        .join("autopilot-desktop")
        .join("manifests")
        .join("plan_mode")
}

fn latest_manifest_path(signature: PlanModeSignatureKind) -> PathBuf {
    manifest_dir().join(format!("{}.latest.json", signature.filename_stem()))
}

fn state_path() -> PathBuf {
    let home = dirs::home_dir().expect("No home directory");
    home.join(".openagents")
        .join("autopilot-desktop")
        .join("optimization")
        .join("plan_mode_state.json")
}

fn log_path() -> PathBuf {
    let home = dirs::home_dir().expect("No home directory");
    home.join(".openagents")
        .join("autopilot-desktop")
        .join("optimization")
        .join("plan_mode.jsonl")
}

struct PlanModeOptimizationLogger {
    enabled: bool,
}

impl PlanModeOptimizationLogger {
    fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    fn log(&self, event: PlanModeOptimizationEvent) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        let path = log_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        let line = serde_json::to_string(&event)?;
        writeln!(file, "{}", line)?;
        Ok(())
    }
}

struct PlanModeSignatureModule {
    signature: PlanModeSignatureKind,
    predictor: Predict,
    lm: Arc<LM>,
}

impl PlanModeSignatureModule {
    fn new(
        signature: PlanModeSignatureKind,
        lm: Arc<LM>,
        instruction: Option<String>,
    ) -> Result<Self> {
        let predictor = build_predictor(signature, instruction)?;
        Ok(Self {
            signature,
            predictor,
            lm,
        })
    }

    fn instruction(&self) -> String {
        self.predictor.get_signature().instruction()
    }
}

impl Module for PlanModeSignatureModule {
    async fn forward(&self, inputs: Example) -> Result<Prediction> {
        let filtered = input_only_example(&inputs);
        self.predictor
            .forward_with_config(filtered, Arc::clone(&self.lm))
            .await
            .context("prediction failed")
    }
}

impl dsrs::Evaluator for PlanModeSignatureModule {
    async fn metric(&self, example: &Example, prediction: &Prediction) -> f32 {
        score_signature(self.signature, example, prediction)
    }
}

impl FeedbackEvaluator for PlanModeSignatureModule {
    async fn feedback_metric(&self, example: &Example, prediction: &Prediction) -> dsrs::evaluate::FeedbackMetric {
        feedback_signature(self.signature, example, prediction)
    }
}

impl Optimizable for PlanModeSignatureModule {
    fn get_signature(&self) -> &dyn dsrs::core::MetaSignature {
        self.predictor.get_signature()
    }

    fn parameters(&mut self) -> IndexMap<String, &mut dyn Optimizable> {
        let mut params: IndexMap<String, &mut dyn Optimizable> = IndexMap::new();
        params.insert(self.signature.name().to_string(), &mut self.predictor);
        params
    }

    fn update_signature_instruction(&mut self, instruction: String) -> Result<()> {
        self.predictor.update_signature_instruction(instruction)
    }
}

fn build_predictor(
    signature: PlanModeSignatureKind,
    instruction: Option<String>,
) -> Result<Predict> {
    match signature {
        PlanModeSignatureKind::TopicDecomposition => {
            let mut sig = TopicDecompositionSignature::new();
            if let Some(instruction) = instruction {
                sig.update_instruction(instruction)?;
            }
            Ok(Predict::new(sig))
        }
        PlanModeSignatureKind::ParallelExploration => {
            let mut sig = ParallelExplorationSignature::new();
            if let Some(instruction) = instruction {
                sig.update_instruction(instruction)?;
            }
            Ok(Predict::new(sig))
        }
        PlanModeSignatureKind::PlanSynthesis => {
            let mut sig = PlanSynthesisSignature::new();
            if let Some(instruction) = instruction {
                sig.update_instruction(instruction)?;
            }
            Ok(Predict::new(sig))
        }
        PlanModeSignatureKind::ComplexityClassification => {
            let mut sig = ComplexityClassificationSignature::new();
            if let Some(instruction) = instruction {
                sig.update_instruction(instruction)?;
            }
            Ok(Predict::new(sig))
        }
        PlanModeSignatureKind::DeepPlanning => {
            let mut sig = DeepPlanningSignature::new();
            if let Some(instruction) = instruction {
                sig.update_instruction(instruction)?;
            }
            Ok(Predict::new(sig))
        }
        PlanModeSignatureKind::ResultValidation => {
            let mut sig = ResultValidationSignature::new();
            if let Some(instruction) = instruction {
                sig.update_instruction(instruction)?;
            }
            Ok(Predict::new(sig))
        }
    }
}

fn input_only_example(example: &Example) -> Example {
    let mut data = HashMap::new();
    for key in &example.input_keys {
        if let Some(value) = example.data.get(key) {
            data.insert(key.clone(), value.clone());
        }
    }
    Example::new(data, example.input_keys.clone(), Vec::new())
}
