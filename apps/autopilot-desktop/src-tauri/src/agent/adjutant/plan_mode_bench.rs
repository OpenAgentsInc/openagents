//! Benchmark harness for plan mode signatures.

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;
use chrono::{DateTime, Utc};
use dsrs::evaluate::{EvalTask, EvalTaskSet, Metric, MetricScore, MetricTier, Scorer, ScorerBuilder};
use dsrs::evaluate::metrics::truth::LlmJudgeMetric;
use dsrs::{Example, LmUsage, Prediction};
use serde::Serialize;
use serde_json::Value;

use super::config::PlanModeOptimizationConfig;
use super::plan_mode_metrics::score_signature;
use super::plan_mode_signatures::PlanModeSignatureKind;

pub const PLAN_MODE_PROXY_METRIC: &str = "plan_mode_proxy";

#[derive(Debug, Clone, Serialize)]
pub struct PlanModeBenchmarkEvent {
    pub timestamp: DateTime<Utc>,
    pub signature: String,
    pub optimizer: String,
    pub training_examples: usize,
    pub eval_examples: usize,
    pub baseline_scorecard: Option<dsrs::evaluate::ScorecardResult>,
    pub candidate_scorecard: Option<dsrs::evaluate::ScorecardResult>,
    pub baseline_manifest_id: Option<String>,
    pub candidate_manifest_id: Option<String>,
    pub promotion_result: Option<dsrs::evaluate::PromotionResult>,
    pub promotion_state: Option<dsrs::evaluate::PromotionState>,
    pub delta_over_baseline: Option<f64>,
    pub config: PlanModeOptimizationConfig,
}

pub struct PlanModeBenchmarkLogger {
    enabled: bool,
}

impl PlanModeBenchmarkLogger {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    pub fn log(&self, event: PlanModeBenchmarkEvent) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        let path = benchmark_log_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        let line = serde_json::to_string(&event)?;
        writeln!(file, "{}", line)?;
        Ok(())
    }
}

pub fn split_examples_for_eval(
    examples: &[Example],
    eval_split_size: usize,
) -> (Vec<Example>, Vec<Example>) {
    if examples.is_empty() || eval_split_size == 0 {
        return (examples.to_vec(), Vec::new());
    }
    let eval_count = eval_split_size.min(examples.len());
    let split_index = examples.len() - eval_count;
    let train = examples[..split_index].to_vec();
    let eval = examples[split_index..].to_vec();
    (train, eval)
}

pub fn build_eval_task_set(
    signature: PlanModeSignatureKind,
    eval_examples: &[Example],
) -> EvalTaskSet {
    let mut tasks = Vec::new();
    for (idx, example) in eval_examples.iter().enumerate() {
        if let Some(goal) = encode_goal_from_example(example) {
            let task_id = format!("plan_mode_{}_{}", signature.filename_stem(), idx);
            let task = EvalTask::new(task_id, goal)
                .with_tag("plan_mode")
                .with_tag(signature.filename_stem());
            tasks.push(task);
        }
    }
    EvalTaskSet::new(format!("plan_mode_{}", signature.filename_stem()))
        .with_description("Plan mode eval tasks derived from training examples")
        .with_tasks(tasks)
}

pub fn scorer_for_signature(signature: PlanModeSignatureKind, config: &PlanModeOptimizationConfig) -> Scorer {
    let rollouts = config.num_trials.clamp(1, 3);
    ScorerBuilder::new()
        .metric(PlanModeProxyMetric::new(signature))
        .metric(truth_metric_for_signature(signature))
        .proxy_threshold(f64::from(config.min_proxy_score))
        .rollouts(rollouts)
        .build()
}

pub fn truth_metric_name(signature: PlanModeSignatureKind) -> String {
    format!("plan_mode_truth_{}", signature.filename_stem())
}

pub fn decode_goal_example(example: &Example) -> Option<Example> {
    let goal = example.data.get("goal")?.as_str()?;
    let parsed: HashMap<String, Value> = serde_json::from_str(goal).ok()?;
    if parsed.is_empty() {
        return None;
    }
    let mut data = HashMap::new();
    let mut keys = Vec::new();
    for (key, value) in parsed {
        keys.push(key.clone());
        data.insert(key, value);
    }
    keys.sort();
    Some(Example::new(data, keys, Vec::new()))
}

fn encode_goal_from_example(example: &Example) -> Option<String> {
    if example.input_keys.is_empty() {
        return None;
    }
    let mut map = HashMap::new();
    for key in &example.input_keys {
        if let Some(value) = example.data.get(key) {
            map.insert(key.clone(), value.clone());
        }
    }
    serde_json::to_string(&map).ok()
}

fn truth_metric_for_signature(signature: PlanModeSignatureKind) -> LlmJudgeMetric {
    let criteria = match signature {
        PlanModeSignatureKind::TopicDecomposition => vec![
            "Topic clarity".to_string(),
            "Reasonable topic count".to_string(),
            "Coverage of user prompt".to_string(),
        ],
        PlanModeSignatureKind::ParallelExploration => vec![
            "Findings relevance".to_string(),
            "Evidence cited".to_string(),
            "Coverage of focus area".to_string(),
        ],
        PlanModeSignatureKind::PlanSynthesis => vec![
            "Step quality".to_string(),
            "Completeness".to_string(),
            "Practical sequencing".to_string(),
        ],
        PlanModeSignatureKind::ComplexityClassification => vec![
            "Routing appropriateness".to_string(),
            "Reasoning clarity".to_string(),
            "Consistency with task".to_string(),
        ],
        PlanModeSignatureKind::DeepPlanning => vec![
            "Strategic depth".to_string(),
            "Risk coverage".to_string(),
            "Execution readiness".to_string(),
        ],
        PlanModeSignatureKind::ResultValidation => vec![
            "Format correctness".to_string(),
            "Classification sanity".to_string(),
            "Actionable feedback".to_string(),
        ],
    };

    LlmJudgeMetric::new()
        .with_name(truth_metric_name(signature))
        .with_criteria(criteria)
}

fn benchmark_log_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".openagents")
        .join("autopilot-desktop")
        .join("benchmarks")
        .join("plan_mode.jsonl")
}

struct PlanModeProxyMetric {
    signature: PlanModeSignatureKind,
}

impl PlanModeProxyMetric {
    fn new(signature: PlanModeSignatureKind) -> Self {
        Self { signature }
    }
}

#[async_trait::async_trait]
impl Metric for PlanModeProxyMetric {
    fn name(&self) -> &str {
        PLAN_MODE_PROXY_METRIC
    }

    fn tier(&self) -> MetricTier {
        MetricTier::Proxy
    }

    fn cost_estimate(&self) -> u64 {
        0
    }

    async fn evaluate(&self, input: &Example, output: &Example) -> Result<MetricScore> {
        let input_example = decode_goal_example(input).unwrap_or_default();
        let prediction = Prediction::new(output.data.clone(), LmUsage::default());
        let score = f64::from(score_signature(self.signature, &input_example, &prediction));
        Ok(MetricScore::new(score))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn goal_encoding_roundtrip() {
        let mut data = HashMap::new();
        data.insert("user_prompt".to_string(), Value::String("Test".to_string()));
        data.insert("file_tree".to_string(), Value::String("src/".to_string()));
        let example = Example::new(
            data,
            vec!["user_prompt".to_string(), "file_tree".to_string()],
            Vec::new(),
        );
        let encoded = encode_goal_from_example(&example).unwrap_or_default();
        let mut input = Example::default();
        input.data.insert("goal".to_string(), Value::String(encoded));
        let decoded = decode_goal_example(&input).unwrap_or_default();
        assert_eq!(
            decoded.data.get("user_prompt").and_then(Value::as_str),
            Some("Test")
        );
    }
}
