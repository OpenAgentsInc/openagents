//! Experiment matrix for running all method × dataset combinations.
//!
//! Provides batch experiment execution with checkpointing and parallel execution.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// An experiment configuration (method, dataset pair).
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExperimentKey {
    pub method: String,
    pub dataset: String,
}

impl ExperimentKey {
    pub fn new(method: impl Into<String>, dataset: impl Into<String>) -> Self {
        Self {
            method: method.into(),
            dataset: dataset.into(),
        }
    }
}

/// Checkpoint for tracking experiment progress.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixCheckpoint {
    /// Completed experiments.
    pub completed: Vec<ExperimentKey>,
    /// Currently in-progress experiment (if any).
    pub in_progress: Option<ExperimentKey>,
    /// Remaining experiments to run.
    pub remaining: Vec<ExperimentKey>,
}

impl MatrixCheckpoint {
    /// Create a new checkpoint with all experiments pending.
    pub fn new(experiments: Vec<ExperimentKey>) -> Self {
        Self {
            completed: Vec::new(),
            in_progress: None,
            remaining: experiments,
        }
    }

    /// Load checkpoint from file, or create new if file doesn't exist.
    pub fn load_or_create(path: &Path, experiments: Vec<ExperimentKey>) -> std::io::Result<Self> {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let checkpoint: MatrixCheckpoint = serde_json::from_str(&content)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            Ok(checkpoint)
        } else {
            Ok(Self::new(experiments))
        }
    }

    /// Save checkpoint to file.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, content)
    }

    /// Mark an experiment as in progress.
    pub fn start_experiment(&mut self, key: &ExperimentKey) {
        self.remaining.retain(|k| k != key);
        self.in_progress = Some(key.clone());
    }

    /// Mark an experiment as completed.
    pub fn complete_experiment(&mut self, key: &ExperimentKey) {
        self.in_progress = None;
        if !self.completed.contains(key) {
            self.completed.push(key.clone());
        }
    }

    /// Get the next experiment to run.
    pub fn next_experiment(&self) -> Option<&ExperimentKey> {
        self.remaining.first()
    }

    /// Check if all experiments are done.
    pub fn is_complete(&self) -> bool {
        self.remaining.is_empty() && self.in_progress.is_none()
    }

    /// Get progress as (completed, total).
    pub fn progress(&self) -> (usize, usize) {
        let total = self.completed.len() + self.remaining.len() + if self.in_progress.is_some() { 1 } else { 0 };
        (self.completed.len(), total)
    }
}

/// Configuration for the experiment matrix.
#[derive(Debug, Clone)]
pub struct MatrixConfig {
    /// Methods to run.
    pub methods: Vec<String>,
    /// Datasets to run.
    pub datasets: Vec<String>,
    /// Output directory for results.
    pub output_dir: PathBuf,
    /// Model to use.
    pub model: String,
    /// Maximum tasks per experiment (for testing).
    pub max_tasks: Option<usize>,
    /// Data directory for real data (if any).
    pub data_dir: Option<PathBuf>,
}

impl MatrixConfig {
    /// Create experiment keys for all method × dataset combinations.
    pub fn experiment_keys(&self) -> Vec<ExperimentKey> {
        let mut keys = Vec::new();
        for method in &self.methods {
            for dataset in &self.datasets {
                keys.push(ExperimentKey::new(method, dataset));
            }
        }
        keys
    }

    /// Get output directory for a specific experiment.
    pub fn experiment_output_dir(&self, key: &ExperimentKey) -> PathBuf {
        self.output_dir.join(format!("{}-{}", key.method, key.dataset))
    }
}

/// Predefined experiment configurations.
pub mod presets {
    use super::*;

    /// Table 1 from the RLM paper: all 5 methods × all 5 datasets.
    pub fn table1(output_dir: PathBuf, model: String) -> MatrixConfig {
        MatrixConfig {
            methods: vec![
                "Base".to_string(),
                "Summary".to_string(),
                "CodeactBm25".to_string(),
                "Rlm".to_string(),
                "RlmNoSubcalls".to_string(),
            ],
            datasets: vec![
                "SNiah".to_string(),
                "Browsecomp".to_string(),
                "OolongTrec".to_string(),
                "OolongPairs".to_string(),
                "Codeqa".to_string(),
            ],
            output_dir,
            model,
            max_tasks: None,
            data_dir: None,
        }
    }

    /// Quick test: Base and RLM on S-NIAH only.
    pub fn quick_test(output_dir: PathBuf, model: String) -> MatrixConfig {
        MatrixConfig {
            methods: vec!["Base".to_string(), "Rlm".to_string()],
            datasets: vec!["SNiah".to_string()],
            output_dir,
            model,
            max_tasks: Some(5),
            data_dir: None,
        }
    }

    /// Ablation: RLM vs RLM-NoSubcalls on all datasets.
    pub fn ablation(output_dir: PathBuf, model: String) -> MatrixConfig {
        MatrixConfig {
            methods: vec!["Rlm".to_string(), "RlmNoSubcalls".to_string()],
            datasets: vec![
                "SNiah".to_string(),
                "Browsecomp".to_string(),
                "OolongTrec".to_string(),
                "OolongPairs".to_string(),
                "Codeqa".to_string(),
            ],
            output_dir,
            model,
            max_tasks: None,
            data_dir: None,
        }
    }
}

/// Print experiment matrix status.
pub fn print_matrix_status(checkpoint: &MatrixCheckpoint) {
    let (completed, total) = checkpoint.progress();
    println!("\n=== Experiment Matrix Status ===");
    println!("Progress: {}/{} experiments", completed, total);

    if !checkpoint.completed.is_empty() {
        println!("\nCompleted:");
        for key in &checkpoint.completed {
            println!("  ✓ {} × {}", key.method, key.dataset);
        }
    }

    if let Some(ref key) = checkpoint.in_progress {
        println!("\nIn Progress:");
        println!("  → {} × {}", key.method, key.dataset);
    }

    if !checkpoint.remaining.is_empty() {
        println!("\nRemaining:");
        for key in &checkpoint.remaining {
            println!("  ○ {} × {}", key.method, key.dataset);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_experiment_key() {
        let key = ExperimentKey::new("RLM", "S-NIAH");
        assert_eq!(key.method, "RLM");
        assert_eq!(key.dataset, "S-NIAH");
    }

    #[test]
    fn test_matrix_checkpoint() {
        let experiments = vec![
            ExperimentKey::new("Base", "S-NIAH"),
            ExperimentKey::new("RLM", "S-NIAH"),
        ];

        let mut checkpoint = MatrixCheckpoint::new(experiments);
        assert_eq!(checkpoint.progress(), (0, 2));

        let key = checkpoint.next_experiment().unwrap().clone();
        checkpoint.start_experiment(&key);
        assert!(checkpoint.in_progress.is_some());

        checkpoint.complete_experiment(&key);
        assert_eq!(checkpoint.progress(), (1, 2));
        assert!(!checkpoint.is_complete());
    }

    #[test]
    fn test_matrix_config() {
        let config = presets::table1(PathBuf::from("./results"), "test".to_string());
        let keys = config.experiment_keys();
        assert_eq!(keys.len(), 25); // 5 methods × 5 datasets
    }
}
