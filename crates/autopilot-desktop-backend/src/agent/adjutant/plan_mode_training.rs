//! Training data collection for plan mode signatures.

use anyhow::Result;
use chrono::{DateTime, Utc};
use dsrs::Example;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use super::plan_mode_signatures::PlanModeSignatureKind;

const TRAINING_FILE: &str = "plan_mode.json";

pub fn training_data_dir() -> PathBuf {
    let home = dirs::home_dir().expect("No home directory");
    home.join(".openagents/autopilot-desktop/training")
}

fn training_data_path() -> PathBuf {
    training_data_dir().join(TRAINING_FILE)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicDecompositionExample {
    pub user_prompt: String,
    pub file_tree: String,
    pub topics: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelExplorationExample {
    pub topic: String,
    pub focus: String,
    pub patterns: String,
    pub repo_path: String,
    pub file_context: String,
    pub findings: String,
    pub files_examined: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanSynthesisExample {
    pub user_prompt: String,
    pub exploration_results: String,
    pub repo_context: String,
    pub implementation_plan: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityClassificationExample {
    pub task_description: String,
    pub repo_indicators: String,
    pub domain_signals: String,
    pub complexity: String,
    pub routing_decision: String,
    pub reasoning: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepPlanningExample {
    pub complex_request: String,
    pub codebase_analysis: String,
    pub constraints: String,
    pub reasoning: String,
    pub strategy: String,
    pub implementation_plan: String,
    pub risk_assessment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultValidationExample {
    pub original_request: String,
    pub generated_output: String,
    pub criteria: String,
    pub quality_assessment: String,
    pub issues: String,
    pub confidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlanModeTrainingStore {
    pub topic_decomposition: Vec<TopicDecompositionExample>,
    pub parallel_exploration: Vec<ParallelExplorationExample>,
    pub plan_synthesis: Vec<PlanSynthesisExample>,
    pub complexity_classification: Vec<ComplexityClassificationExample>,
    pub deep_planning: Vec<DeepPlanningExample>,
    pub result_validation: Vec<ResultValidationExample>,
    pub updated_at: DateTime<Utc>,
}

impl PlanModeTrainingStore {
    pub fn load() -> Result<Self> {
        let path = training_data_path();
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&content).unwrap_or_default())
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = training_data_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn record_topic_decomposition(&mut self, example: TopicDecompositionExample) {
        self.topic_decomposition.push(example);
        self.updated_at = Utc::now();
    }

    pub fn record_parallel_exploration(&mut self, example: ParallelExplorationExample) {
        self.parallel_exploration.push(example);
        self.updated_at = Utc::now();
    }

    pub fn record_plan_synthesis(&mut self, example: PlanSynthesisExample) {
        self.plan_synthesis.push(example);
        self.updated_at = Utc::now();
    }

    pub fn record_complexity_classification(&mut self, example: ComplexityClassificationExample) {
        self.complexity_classification.push(example);
        self.updated_at = Utc::now();
    }

    pub fn record_deep_planning(&mut self, example: DeepPlanningExample) {
        self.deep_planning.push(example);
        self.updated_at = Utc::now();
    }

    pub fn record_result_validation(&mut self, example: ResultValidationExample) {
        self.result_validation.push(example);
        self.updated_at = Utc::now();
    }

    pub fn append_trace(&mut self, trace: PlanModeTrace, max_examples: usize) {
        if let Some(example) = trace.topic_decomposition {
            self.record_topic_decomposition(example);
        }
        if let Some(example) = trace.plan_synthesis {
            self.record_plan_synthesis(example);
        }
        if let Some(example) = trace.complexity_classification {
            self.record_complexity_classification(example);
        }
        if let Some(example) = trace.deep_planning {
            self.record_deep_planning(example);
        }
        if let Some(example) = trace.result_validation {
            self.record_result_validation(example);
        }
        for example in trace.parallel_exploration {
            self.record_parallel_exploration(example);
        }

        trim_examples(&mut self.topic_decomposition, max_examples);
        trim_examples(&mut self.parallel_exploration, max_examples);
        trim_examples(&mut self.plan_synthesis, max_examples);
        trim_examples(&mut self.complexity_classification, max_examples);
        trim_examples(&mut self.deep_planning, max_examples);
        trim_examples(&mut self.result_validation, max_examples);
    }

    pub fn examples_for_signature(&self, signature: PlanModeSignatureKind) -> Vec<Example> {
        match signature {
            PlanModeSignatureKind::TopicDecomposition => self
                .topic_decomposition
                .iter()
                .map(topic_decomposition_example)
                .collect(),
            PlanModeSignatureKind::ParallelExploration => self
                .parallel_exploration
                .iter()
                .map(parallel_exploration_example)
                .collect(),
            PlanModeSignatureKind::PlanSynthesis => self
                .plan_synthesis
                .iter()
                .map(plan_synthesis_example)
                .collect(),
            PlanModeSignatureKind::ComplexityClassification => self
                .complexity_classification
                .iter()
                .map(complexity_classification_example)
                .collect(),
            PlanModeSignatureKind::DeepPlanning => self
                .deep_planning
                .iter()
                .map(deep_planning_example)
                .collect(),
            PlanModeSignatureKind::ResultValidation => self
                .result_validation
                .iter()
                .map(result_validation_example)
                .collect(),
        }
    }

    pub fn example_count(&self, signature: PlanModeSignatureKind) -> usize {
        match signature {
            PlanModeSignatureKind::TopicDecomposition => self.topic_decomposition.len(),
            PlanModeSignatureKind::ParallelExploration => self.parallel_exploration.len(),
            PlanModeSignatureKind::PlanSynthesis => self.plan_synthesis.len(),
            PlanModeSignatureKind::ComplexityClassification => self.complexity_classification.len(),
            PlanModeSignatureKind::DeepPlanning => self.deep_planning.len(),
            PlanModeSignatureKind::ResultValidation => self.result_validation.len(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PlanModeTrace {
    pub topic_decomposition: Option<TopicDecompositionExample>,
    pub parallel_exploration: Vec<ParallelExplorationExample>,
    pub plan_synthesis: Option<PlanSynthesisExample>,
    pub complexity_classification: Option<ComplexityClassificationExample>,
    pub deep_planning: Option<DeepPlanningExample>,
    pub result_validation: Option<ResultValidationExample>,
}

impl PlanModeTrace {
    pub fn is_empty(&self) -> bool {
        self.topic_decomposition.is_none()
            && self.parallel_exploration.is_empty()
            && self.plan_synthesis.is_none()
            && self.complexity_classification.is_none()
            && self.deep_planning.is_none()
            && self.result_validation.is_none()
    }
}

fn trim_examples<T>(examples: &mut Vec<T>, max_examples: usize) {
    if max_examples == 0 {
        examples.clear();
        return;
    }
    if examples.len() > max_examples {
        let drain_count = examples.len() - max_examples;
        examples.drain(0..drain_count);
    }
}

fn topic_decomposition_example(example: &TopicDecompositionExample) -> Example {
    let mut data = HashMap::new();
    data.insert("user_prompt".to_string(), serde_json::json!(example.user_prompt));
    data.insert("file_tree".to_string(), serde_json::json!(example.file_tree));
    data.insert("topics".to_string(), serde_json::json!(example.topics));

    Example::new(
        data,
        vec!["user_prompt".to_string(), "file_tree".to_string()],
        vec!["topics".to_string()],
    )
}

fn parallel_exploration_example(example: &ParallelExplorationExample) -> Example {
    let mut data = HashMap::new();
    data.insert("topic".to_string(), serde_json::json!(example.topic));
    data.insert("focus".to_string(), serde_json::json!(example.focus));
    data.insert("patterns".to_string(), serde_json::json!(example.patterns));
    data.insert("repo_path".to_string(), serde_json::json!(example.repo_path));
    data.insert(
        "file_context".to_string(),
        serde_json::json!(example.file_context),
    );
    data.insert("findings".to_string(), serde_json::json!(example.findings));
    data.insert(
        "files_examined".to_string(),
        serde_json::json!(example.files_examined),
    );

    Example::new(
        data,
        vec![
            "topic".to_string(),
            "focus".to_string(),
            "patterns".to_string(),
            "repo_path".to_string(),
            "file_context".to_string(),
        ],
        vec!["findings".to_string(), "files_examined".to_string()],
    )
}

fn plan_synthesis_example(example: &PlanSynthesisExample) -> Example {
    let mut data = HashMap::new();
    data.insert("user_prompt".to_string(), serde_json::json!(example.user_prompt));
    data.insert(
        "exploration_results".to_string(),
        serde_json::json!(example.exploration_results),
    );
    data.insert("repo_context".to_string(), serde_json::json!(example.repo_context));
    data.insert(
        "implementation_plan".to_string(),
        serde_json::json!(example.implementation_plan),
    );

    Example::new(
        data,
        vec![
            "user_prompt".to_string(),
            "exploration_results".to_string(),
            "repo_context".to_string(),
        ],
        vec!["implementation_plan".to_string()],
    )
}

fn complexity_classification_example(example: &ComplexityClassificationExample) -> Example {
    let mut data = HashMap::new();
    data.insert(
        "task_description".to_string(),
        serde_json::json!(example.task_description),
    );
    data.insert(
        "repo_indicators".to_string(),
        serde_json::json!(example.repo_indicators),
    );
    data.insert(
        "domain_signals".to_string(),
        serde_json::json!(example.domain_signals),
    );
    data.insert("complexity".to_string(), serde_json::json!(example.complexity));
    data.insert(
        "routing_decision".to_string(),
        serde_json::json!(example.routing_decision),
    );
    data.insert("reasoning".to_string(), serde_json::json!(example.reasoning));

    Example::new(
        data,
        vec![
            "task_description".to_string(),
            "repo_indicators".to_string(),
            "domain_signals".to_string(),
        ],
        vec![
            "complexity".to_string(),
            "routing_decision".to_string(),
            "reasoning".to_string(),
        ],
    )
}

fn deep_planning_example(example: &DeepPlanningExample) -> Example {
    let mut data = HashMap::new();
    data.insert(
        "complex_request".to_string(),
        serde_json::json!(example.complex_request),
    );
    data.insert(
        "codebase_analysis".to_string(),
        serde_json::json!(example.codebase_analysis),
    );
    data.insert("constraints".to_string(), serde_json::json!(example.constraints));
    data.insert("reasoning".to_string(), serde_json::json!(example.reasoning));
    data.insert("strategy".to_string(), serde_json::json!(example.strategy));
    data.insert(
        "implementation_plan".to_string(),
        serde_json::json!(example.implementation_plan),
    );
    data.insert(
        "risk_assessment".to_string(),
        serde_json::json!(example.risk_assessment),
    );

    Example::new(
        data,
        vec![
            "complex_request".to_string(),
            "codebase_analysis".to_string(),
            "constraints".to_string(),
        ],
        vec![
            "reasoning".to_string(),
            "strategy".to_string(),
            "implementation_plan".to_string(),
            "risk_assessment".to_string(),
        ],
    )
}

fn result_validation_example(example: &ResultValidationExample) -> Example {
    let mut data = HashMap::new();
    data.insert(
        "original_request".to_string(),
        serde_json::json!(example.original_request),
    );
    data.insert(
        "generated_output".to_string(),
        serde_json::json!(example.generated_output),
    );
    data.insert("criteria".to_string(), serde_json::json!(example.criteria));
    data.insert(
        "quality_assessment".to_string(),
        serde_json::json!(example.quality_assessment),
    );
    data.insert("issues".to_string(), serde_json::json!(example.issues));
    data.insert(
        "confidence".to_string(),
        serde_json::json!(example.confidence),
    );

    Example::new(
        data,
        vec![
            "original_request".to_string(),
            "generated_output".to_string(),
            "criteria".to_string(),
        ],
        vec![
            "quality_assessment".to_string(),
            "issues".to_string(),
            "confidence".to_string(),
        ],
    )
}
