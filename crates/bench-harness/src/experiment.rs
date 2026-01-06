//! Experiment runner with checkpointing and reporting.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::method::Method;
use crate::metrics::Metric;
use crate::task::{GroundTruth, TaskInstance};
use crate::trajectory::TrajectoryWriter;
use lm_router::LmUsage;

/// Configuration for an experiment run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentConfig {
    /// Experiment name.
    pub name: String,
    /// Output directory for results.
    pub output_dir: PathBuf,
    /// Whether to resume from checkpoint.
    pub resume: bool,
    /// Maximum tasks to run (for debugging).
    pub max_tasks: Option<usize>,
    /// Random seed.
    pub seed: Option<u64>,
}

impl ExperimentConfig {
    /// Create a new experiment config.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            output_dir: PathBuf::from("./results"),
            resume: true,
            max_tasks: None,
            seed: None,
        }
    }

    /// Set the output directory.
    pub fn output_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.output_dir = path.into();
        self
    }

    /// Set whether to resume from checkpoint.
    pub fn resume(mut self, resume: bool) -> Self {
        self.resume = resume;
        self
    }

    /// Set maximum tasks to run.
    pub fn max_tasks(mut self, max: usize) -> Self {
        self.max_tasks = Some(max);
        self
    }

    /// Set random seed.
    pub fn seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }
}

/// Result from running a single task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    /// Task ID.
    pub task_id: String,
    /// Method name.
    pub method: String,
    /// The predicted answer.
    pub answer: String,
    /// Ground truth.
    pub ground_truth: GroundTruth,
    /// Metric scores.
    pub scores: HashMap<String, f64>,
    /// Token usage.
    pub usage: LmUsage,
    /// Duration in milliseconds.
    pub duration_ms: u64,
    /// Whether this task succeeded.
    pub success: bool,
    /// Error message if failed.
    pub error: Option<String>,
}

impl TaskResult {
    /// Create a successful task result.
    pub fn success(
        task_id: impl Into<String>,
        method: impl Into<String>,
        answer: impl Into<String>,
        ground_truth: GroundTruth,
        usage: LmUsage,
        duration_ms: u64,
    ) -> Self {
        Self {
            task_id: task_id.into(),
            method: method.into(),
            answer: answer.into(),
            ground_truth,
            scores: HashMap::new(),
            usage,
            duration_ms,
            success: true,
            error: None,
        }
    }

    /// Create a failed task result.
    pub fn failure(
        task_id: impl Into<String>,
        method: impl Into<String>,
        ground_truth: GroundTruth,
        error: impl Into<String>,
    ) -> Self {
        Self {
            task_id: task_id.into(),
            method: method.into(),
            answer: String::new(),
            ground_truth,
            scores: HashMap::new(),
            usage: LmUsage::default(),
            duration_ms: 0,
            success: false,
            error: Some(error.into()),
        }
    }

    /// Add metric scores.
    pub fn with_scores(mut self, scores: HashMap<String, f64>) -> Self {
        self.scores = scores;
        self
    }
}

/// Aggregated results for a method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodResults {
    /// Method name.
    pub method: String,
    /// Primary metric score.
    pub primary_score: f64,
    /// Standard deviation of primary score.
    pub std_dev: f64,
    /// Per-metric aggregated scores.
    pub metric_scores: HashMap<String, f64>,
    /// Total tasks.
    pub total_tasks: usize,
    /// Successful tasks.
    pub successful_tasks: usize,
    /// Total token usage.
    pub total_usage: LmUsage,
    /// Mean token usage per task.
    pub mean_usage: LmUsage,
    /// Mean duration in milliseconds.
    pub mean_duration_ms: f64,
}

impl MethodResults {
    /// Compute method results from individual task results.
    pub fn from_tasks(method: &str, results: &[TaskResult], primary_metric: &str) -> Self {
        let successful: Vec<_> = results.iter().filter(|r| r.success).collect();

        // Aggregate primary score
        let primary_scores: Vec<f64> = successful
            .iter()
            .filter_map(|r| r.scores.get(primary_metric).copied())
            .collect();

        let primary_score = if primary_scores.is_empty() {
            0.0
        } else {
            primary_scores.iter().sum::<f64>() / primary_scores.len() as f64
        };

        let std_dev = if primary_scores.len() < 2 {
            0.0
        } else {
            let mean = primary_score;
            let variance = primary_scores
                .iter()
                .map(|x| (x - mean).powi(2))
                .sum::<f64>()
                / (primary_scores.len() - 1) as f64;
            variance.sqrt()
        };

        // Aggregate all metrics
        let mut metric_scores: HashMap<String, f64> = HashMap::new();
        for result in &successful {
            for (metric, score) in &result.scores {
                *metric_scores.entry(metric.clone()).or_insert(0.0) += score;
            }
        }
        for score in metric_scores.values_mut() {
            if !successful.is_empty() {
                *score /= successful.len() as f64;
            }
        }

        // Aggregate usage
        let total_usage = successful.iter().fold(LmUsage::default(), |mut acc, r| {
            acc.prompt_tokens += r.usage.prompt_tokens;
            acc.completion_tokens += r.usage.completion_tokens;
            acc.total_tokens += r.usage.total_tokens;
            acc
        });

        let mean_usage = if successful.is_empty() {
            LmUsage::default()
        } else {
            LmUsage::new(
                total_usage.prompt_tokens / successful.len(),
                total_usage.completion_tokens / successful.len(),
            )
        };

        let total_duration: u64 = successful.iter().map(|r| r.duration_ms).sum();
        let mean_duration_ms = if successful.is_empty() {
            0.0
        } else {
            total_duration as f64 / successful.len() as f64
        };

        Self {
            method: method.to_string(),
            primary_score,
            std_dev,
            metric_scores,
            total_tasks: results.len(),
            successful_tasks: successful.len(),
            total_usage,
            mean_usage,
            mean_duration_ms,
        }
    }
}

/// Complete experiment results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentResults {
    /// Experiment config.
    pub config: ExperimentConfig,
    /// Results per method.
    pub per_method: HashMap<String, MethodResults>,
    /// All task results.
    pub task_results: Vec<TaskResult>,
    /// Total usage across all methods.
    pub total_usage: LmUsage,
}

impl ExperimentResults {
    /// Create empty results.
    pub fn new(config: ExperimentConfig) -> Self {
        Self {
            config,
            per_method: HashMap::new(),
            task_results: Vec::new(),
            total_usage: LmUsage::default(),
        }
    }
}

/// Checkpoint for resumable experiments.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Checkpoint {
    /// Completed task IDs per method.
    completed: HashMap<String, Vec<String>>,
}

impl Checkpoint {
    fn new() -> Self {
        Self {
            completed: HashMap::new(),
        }
    }

    fn is_completed(&self, method: &str, task_id: &str) -> bool {
        self.completed
            .get(method)
            .map(|ids| ids.contains(&task_id.to_string()))
            .unwrap_or(false)
    }

    fn mark_completed(&mut self, method: &str, task_id: &str) {
        self.completed
            .entry(method.to_string())
            .or_default()
            .push(task_id.to_string());
    }
}

/// Experiment runner.
pub struct ExperimentRunner<T: TaskInstance> {
    config: ExperimentConfig,
    tasks: Vec<T>,
    methods: HashMap<String, Arc<dyn Method>>,
    metrics: Vec<Box<dyn Metric>>,
    primary_metric: String,
    checkpoint: Checkpoint,
    results: Vec<TaskResult>,
    trajectory_writer: Option<TrajectoryWriter>,
}

impl<T: TaskInstance> ExperimentRunner<T> {
    /// Create a new experiment runner.
    pub fn new(config: ExperimentConfig, tasks: Vec<T>) -> Self {
        Self {
            config,
            tasks,
            methods: HashMap::new(),
            metrics: Vec::new(),
            primary_metric: "exact_match".to_string(),
            checkpoint: Checkpoint::new(),
            results: Vec::new(),
            trajectory_writer: None,
        }
    }

    /// Add a method to the experiment.
    pub fn add_method(&mut self, method: Arc<dyn Method>) {
        self.methods.insert(method.name().to_string(), method);
    }

    /// Add a metric.
    pub fn add_metric(&mut self, metric: Box<dyn Metric>) {
        self.metrics.push(metric);
    }

    /// Set the primary metric for scoring.
    pub fn set_primary_metric(&mut self, name: impl Into<String>) {
        self.primary_metric = name.into();
    }

    /// Initialize the experiment (create directories, load checkpoint).
    pub fn init(&mut self) -> Result<()> {
        // Create output directory
        fs::create_dir_all(&self.config.output_dir)?;

        // Load checkpoint if resuming
        if self.config.resume {
            self.load_checkpoint()?;
        }

        // Create trajectory writer
        let trajectory_path = self
            .config
            .output_dir
            .join(format!("{}_trajectories.jsonl", self.config.name));
        self.trajectory_writer = Some(TrajectoryWriter::new(trajectory_path)?);

        Ok(())
    }

    /// Load checkpoint from disk.
    fn load_checkpoint(&mut self) -> Result<()> {
        let checkpoint_path = self
            .config
            .output_dir
            .join(format!("{}_checkpoint.json", self.config.name));

        if checkpoint_path.exists() {
            let content = fs::read_to_string(&checkpoint_path)?;
            self.checkpoint = serde_json::from_str(&content)?;

            // Also load existing results
            let results_path = self
                .config
                .output_dir
                .join(format!("{}_results.jsonl", self.config.name));
            if results_path.exists() {
                let file = File::open(&results_path)?;
                let reader = BufReader::new(file);
                for line in reader.lines() {
                    let line = line?;
                    if !line.trim().is_empty() {
                        if let Ok(result) = serde_json::from_str::<TaskResult>(&line) {
                            self.results.push(result);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Save checkpoint to disk.
    fn save_checkpoint(&self) -> Result<()> {
        let checkpoint_path = self
            .config
            .output_dir
            .join(format!("{}_checkpoint.json", self.config.name));
        let content = serde_json::to_string_pretty(&self.checkpoint)?;
        fs::write(checkpoint_path, content)?;
        Ok(())
    }

    /// Save a task result to disk.
    fn save_result(&self, result: &TaskResult) -> Result<()> {
        let results_path = self
            .config
            .output_dir
            .join(format!("{}_results.jsonl", self.config.name));
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(results_path)?;
        let json = serde_json::to_string(result)?;
        writeln!(file, "{}", json)?;
        Ok(())
    }

    /// Compute metrics for a prediction.
    fn compute_metrics(&self, prediction: &str, ground_truth: &GroundTruth) -> HashMap<String, f64> {
        self.metrics
            .iter()
            .map(|m| {
                let value = m.compute(prediction, ground_truth);
                (value.name, value.score)
            })
            .collect()
    }

    /// Run the experiment.
    pub async fn run(&mut self) -> Result<ExperimentResults> {
        self.init()?;

        let max_tasks = self.config.max_tasks.unwrap_or(self.tasks.len());

        // Collect method names and indices to avoid borrow conflicts
        let method_info: Vec<_> = self
            .methods
            .iter()
            .map(|(name, method)| (name.clone(), Arc::clone(method)))
            .collect();

        for (method_name, method) in method_info {
            for task_idx in 0..max_tasks.min(self.tasks.len()) {
                // Get task ID upfront to avoid borrow conflicts
                let task_id = self.tasks[task_idx].id().to_string();

                // Skip if already completed
                if self.checkpoint.is_completed(&method_name, &task_id) {
                    continue;
                }

                // Run the task
                let result = {
                    let task = &self.tasks[task_idx];
                    self.run_single_task_immut(method.as_ref(), task).await
                };

                // Save result
                self.save_result(&result)?;
                self.results.push(result);

                // Update checkpoint
                self.checkpoint.mark_completed(&method_name, &task_id);
                self.save_checkpoint()?;
            }
        }

        // Aggregate results
        let mut experiment_results = ExperimentResults::new(self.config.clone());

        for (method_name, _) in &self.methods {
            let method_results: Vec<_> = self
                .results
                .iter()
                .filter(|r| r.method == *method_name)
                .cloned()
                .collect();

            let aggregated =
                MethodResults::from_tasks(method_name, &method_results, &self.primary_metric);
            experiment_results
                .per_method
                .insert(method_name.clone(), aggregated);
        }

        experiment_results.task_results = self.results.clone();

        // Calculate total usage
        for result in &experiment_results.task_results {
            experiment_results.total_usage.prompt_tokens += result.usage.prompt_tokens;
            experiment_results.total_usage.completion_tokens += result.usage.completion_tokens;
            experiment_results.total_usage.total_tokens += result.usage.total_tokens;
        }

        // Save final results
        let final_path = self
            .config
            .output_dir
            .join(format!("{}_final.json", self.config.name));
        let content = serde_json::to_string_pretty(&experiment_results)?;
        fs::write(final_path, content)?;

        Ok(experiment_results)
    }

    /// Run a single task with a method (immutable self, returns trajectory to be written later).
    async fn run_single_task_immut(&self, method: &dyn Method, task: &T) -> TaskResult {
        let start = std::time::Instant::now();

        match method.solve(task).await {
            Ok(method_result) => {
                let duration_ms = start.elapsed().as_millis() as u64;

                // Compute metrics
                let scores = self.compute_metrics(&method_result.answer, task.ground_truth());

                TaskResult::success(
                    task.id(),
                    method.name(),
                    method_result.answer,
                    task.ground_truth().clone(),
                    method_result.usage,
                    duration_ms,
                )
                .with_scores(scores)
            }
            Err(e) => TaskResult::failure(
                task.id(),
                method.name(),
                task.ground_truth().clone(),
                e.to_string(),
            ),
        }
    }

    /// Get current results.
    pub fn results(&self) -> &[TaskResult] {
        &self.results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_experiment_config() {
        let config = ExperimentConfig::new("test-exp")
            .output_dir("/tmp/results")
            .max_tasks(10)
            .seed(42);

        assert_eq!(config.name, "test-exp");
        assert_eq!(config.output_dir, PathBuf::from("/tmp/results"));
        assert_eq!(config.max_tasks, Some(10));
        assert_eq!(config.seed, Some(42));
    }

    #[test]
    fn test_task_result() {
        let result = TaskResult::success(
            "task-1",
            "base",
            "42",
            GroundTruth::exact("42"),
            LmUsage::new(100, 50),
            1000,
        );

        assert!(result.success);
        assert_eq!(result.task_id, "task-1");
        assert_eq!(result.answer, "42");
    }

    #[test]
    fn test_method_results_aggregation() {
        let results = vec![
            TaskResult::success(
                "task-1",
                "base",
                "42",
                GroundTruth::exact("42"),
                LmUsage::new(100, 50),
                1000,
            )
            .with_scores(HashMap::from([("exact_match".to_string(), 1.0)])),
            TaskResult::success(
                "task-2",
                "base",
                "wrong",
                GroundTruth::exact("right"),
                LmUsage::new(200, 100),
                2000,
            )
            .with_scores(HashMap::from([("exact_match".to_string(), 0.0)])),
        ];

        let method_results = MethodResults::from_tasks("base", &results, "exact_match");

        assert_eq!(method_results.primary_score, 0.5);
        assert_eq!(method_results.total_tasks, 2);
        assert_eq!(method_results.successful_tasks, 2);
        assert_eq!(method_results.total_usage.prompt_tokens, 300);
    }
}
