//! Training data collection and storage for Adjutant DSPy optimization.
//!
//! Collects training examples from successful task executions and stores
//! them for use with MIPROv2 optimization.

use anyhow::Result;
use dsrs::Example;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Storage location for training data.
pub fn training_data_path() -> PathBuf {
    let home = dirs::home_dir().expect("No home directory");
    home.join(".openagents/adjutant/training")
}

// ============================================================================
// Training Example Types
// ============================================================================

/// Data for a single subtask.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtaskData {
    pub id: String,
    pub action: String,
    pub target: String,
    pub instruction: String,
}

/// Training example for subtask planning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanningTrainingExample {
    // Inputs
    pub task_title: String,
    pub task_description: String,
    pub context: String,
    // Expected outputs (from successful executions)
    pub expected_subtasks: Vec<SubtaskData>,
    pub success: bool,
}

impl PlanningTrainingExample {
    /// Convert to DSPy Example for optimization.
    pub fn to_example(&self) -> Example {
        let mut data = HashMap::new();
        data.insert(
            "task_title".to_string(),
            serde_json::json!(self.task_title),
        );
        data.insert(
            "task_description".to_string(),
            serde_json::json!(self.task_description),
        );
        data.insert("context".to_string(), serde_json::json!(self.context));
        data.insert(
            "subtasks".to_string(),
            serde_json::json!(serde_json::to_string(&self.expected_subtasks).unwrap_or_default()),
        );

        Example::new(
            data,
            vec![
                "task_title".to_string(),
                "task_description".to_string(),
                "context".to_string(),
            ],
            vec!["subtasks".to_string()],
        )
    }
}

/// Training example for subtask execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTrainingExample {
    // Inputs
    pub action: String,
    pub target: String,
    pub instruction: String,
    pub file_context: String,
    // Expected outputs
    pub expected_result: serde_json::Value,
    pub success: bool,
}

impl ExecutionTrainingExample {
    /// Convert to DSPy Example for optimization.
    pub fn to_example(&self) -> Example {
        let mut data = HashMap::new();
        data.insert("action".to_string(), serde_json::json!(self.action));
        data.insert("target".to_string(), serde_json::json!(self.target));
        data.insert(
            "instruction".to_string(),
            serde_json::json!(self.instruction),
        );
        data.insert(
            "file_context".to_string(),
            serde_json::json!(self.file_context),
        );
        data.insert(
            "result".to_string(),
            serde_json::json!(serde_json::to_string(&self.expected_result).unwrap_or_default()),
        );
        data.insert("success".to_string(), serde_json::json!(self.success));

        Example::new(
            data,
            vec![
                "action".to_string(),
                "target".to_string(),
                "instruction".to_string(),
                "file_context".to_string(),
            ],
            vec!["result".to_string(), "success".to_string()],
        )
    }
}

/// Training example for result synthesis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynthesisTrainingExample {
    // Inputs
    pub task_title: String,
    pub subtask_results: String,
    // Expected outputs
    pub expected_success: bool,
    pub expected_summary: String,
    pub expected_modified_files: Vec<String>,
}

impl SynthesisTrainingExample {
    /// Convert to DSPy Example for optimization.
    pub fn to_example(&self) -> Example {
        let mut data = HashMap::new();
        data.insert(
            "task_title".to_string(),
            serde_json::json!(self.task_title),
        );
        data.insert(
            "subtask_results".to_string(),
            serde_json::json!(self.subtask_results),
        );
        data.insert(
            "success".to_string(),
            serde_json::json!(self.expected_success),
        );
        data.insert(
            "summary".to_string(),
            serde_json::json!(self.expected_summary),
        );
        data.insert(
            "modified_files".to_string(),
            serde_json::json!(
                serde_json::to_string(&self.expected_modified_files).unwrap_or_default()
            ),
        );

        Example::new(
            data,
            vec!["task_title".to_string(), "subtask_results".to_string()],
            vec![
                "success".to_string(),
                "summary".to_string(),
                "modified_files".to_string(),
            ],
        )
    }
}

// ============================================================================
// Training Dataset
// ============================================================================

/// Complete training dataset for Adjutant optimization.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AdjutantTrainingDataset {
    pub planning_examples: Vec<PlanningTrainingExample>,
    pub execution_examples: Vec<ExecutionTrainingExample>,
    pub synthesis_examples: Vec<SynthesisTrainingExample>,
}

impl AdjutantTrainingDataset {
    /// Create a new empty dataset.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load dataset from disk.
    pub fn load() -> Result<Self> {
        let path = training_data_path().join("dataset.json");
        if path.exists() {
            let contents = std::fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&contents)?)
        } else {
            Ok(Self::new())
        }
    }

    /// Save dataset to disk.
    pub fn save(&self) -> Result<()> {
        let path = training_data_path();
        std::fs::create_dir_all(&path)?;
        let file_path = path.join("dataset.json");
        let contents = serde_json::to_string_pretty(self)?;
        std::fs::write(file_path, contents)?;
        Ok(())
    }

    /// Add a planning example.
    pub fn add_planning_example(&mut self, example: PlanningTrainingExample) {
        self.planning_examples.push(example);
    }

    /// Add an execution example.
    pub fn add_execution_example(&mut self, example: ExecutionTrainingExample) {
        self.execution_examples.push(example);
    }

    /// Add a synthesis example.
    pub fn add_synthesis_example(&mut self, example: SynthesisTrainingExample) {
        self.synthesis_examples.push(example);
    }

    /// Convert planning examples to DSPy Examples.
    pub fn planning_as_examples(&self) -> Vec<Example> {
        self.planning_examples.iter().map(|e| e.to_example()).collect()
    }

    /// Convert execution examples to DSPy Examples.
    pub fn execution_as_examples(&self) -> Vec<Example> {
        self.execution_examples.iter().map(|e| e.to_example()).collect()
    }

    /// Convert synthesis examples to DSPy Examples.
    pub fn synthesis_as_examples(&self) -> Vec<Example> {
        self.synthesis_examples.iter().map(|e| e.to_example()).collect()
    }

    /// Get total number of examples.
    pub fn len(&self) -> usize {
        self.planning_examples.len()
            + self.execution_examples.len()
            + self.synthesis_examples.len()
    }

    /// Check if dataset is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// ============================================================================
// Training Collector
// ============================================================================

/// Collector that records successful executions for training.
pub struct TrainingCollector {
    dataset: AdjutantTrainingDataset,
    auto_save: bool,
}

impl TrainingCollector {
    /// Create a new collector.
    ///
    /// If `auto_save` is true, saves after each new example.
    pub fn new(auto_save: bool) -> Result<Self> {
        let dataset = AdjutantTrainingDataset::load().unwrap_or_default();
        Ok(Self { dataset, auto_save })
    }

    /// Record a planning example (only if successful).
    pub fn record_planning(&mut self, example: PlanningTrainingExample) -> Result<()> {
        if example.success {
            self.dataset.add_planning_example(example);
            if self.auto_save {
                self.dataset.save()?;
            }
        }
        Ok(())
    }

    /// Record an execution example (only if successful).
    pub fn record_execution(&mut self, example: ExecutionTrainingExample) -> Result<()> {
        if example.success {
            self.dataset.add_execution_example(example);
            if self.auto_save {
                self.dataset.save()?;
            }
        }
        Ok(())
    }

    /// Record a synthesis example (only if successful).
    pub fn record_synthesis(&mut self, example: SynthesisTrainingExample) -> Result<()> {
        if example.expected_success {
            self.dataset.add_synthesis_example(example);
            if self.auto_save {
                self.dataset.save()?;
            }
        }
        Ok(())
    }

    /// Get access to the underlying dataset.
    pub fn dataset(&self) -> &AdjutantTrainingDataset {
        &self.dataset
    }

    /// Force save the dataset.
    pub fn save(&self) -> Result<()> {
        self.dataset.save()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_planning_example_to_dsrs() {
        let example = PlanningTrainingExample {
            task_title: "Add hello world".to_string(),
            task_description: "Add a hello world function".to_string(),
            context: "// lib.rs\npub fn main() {}".to_string(),
            expected_subtasks: vec![SubtaskData {
                id: "1".to_string(),
                action: "edit".to_string(),
                target: "src/lib.rs".to_string(),
                instruction: "Add hello function".to_string(),
            }],
            success: true,
        };

        let dsrs_example = example.to_example();
        assert!(dsrs_example.data.contains_key("task_title"));
        assert!(dsrs_example.data.contains_key("subtasks"));
        assert!(dsrs_example.input_keys.contains(&"task_title".to_string()));
        assert!(dsrs_example.output_keys.contains(&"subtasks".to_string()));
    }

    #[test]
    fn test_dataset_roundtrip() {
        let mut dataset = AdjutantTrainingDataset::new();
        dataset.add_planning_example(PlanningTrainingExample {
            task_title: "Test".to_string(),
            task_description: "Test task".to_string(),
            context: "".to_string(),
            expected_subtasks: vec![],
            success: true,
        });

        let json = serde_json::to_string(&dataset).unwrap();
        let loaded: AdjutantTrainingDataset = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.planning_examples.len(), 1);
    }
}
