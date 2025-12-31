//! Coalition compute types for parallel execution across multiple providers

use serde::{Deserialize, Serialize};

/// Parallelism strategy for task execution
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ParallelismStrategy {
    /// All subtasks run simultaneously
    Parallel,
    /// Output of one task feeds into the next (pipeline)
    Pipeline,
    /// Run same task on N providers and compare results
    Redundant {
        /// Number of redundant executions
        count: u32,
    },
    /// System decides based on task characteristics
    Adaptive,
}

impl ParallelismStrategy {
    /// Check if strategy involves redundant execution
    pub fn is_redundant(&self) -> bool {
        matches!(self, ParallelismStrategy::Redundant { .. })
    }

    /// Get redundancy count if applicable
    pub fn redundancy_count(&self) -> Option<u32> {
        match self {
            ParallelismStrategy::Redundant { count } => Some(*count),
            _ => None,
        }
    }
}

/// Aggregation strategy for combining results
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AggregationStrategy {
    /// Join all outputs in order
    Concatenate,
    /// Combine with deduplication
    Merge,
    /// Majority vote for redundant execution
    Vote,
    /// Custom aggregation with provided prompt
    Custom {
        /// Prompt describing how to aggregate
        prompt: String,
    },
}

impl AggregationStrategy {
    /// Create a custom aggregation strategy
    pub fn custom(prompt: impl Into<String>) -> Self {
        Self::Custom {
            prompt: prompt.into(),
        }
    }
}

/// Inference parameters for a subtask
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InferenceParams {
    /// Temperature (0.0-2.0)
    pub temperature: f32,
    /// Maximum tokens to generate
    pub max_tokens: u32,
    /// Top-p sampling
    pub top_p: Option<f32>,
    /// Stop sequences
    pub stop_sequences: Vec<String>,
}

impl Default for InferenceParams {
    fn default() -> Self {
        Self {
            temperature: 1.0,
            max_tokens: 1024,
            top_p: None,
            stop_sequences: Vec::new(),
        }
    }
}

impl InferenceParams {
    /// Create new params with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Set temperature
    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = temp.clamp(0.0, 2.0);
        self
    }

    /// Set max tokens
    pub fn with_max_tokens(mut self, max: u32) -> Self {
        self.max_tokens = max;
        self
    }

    /// Set top-p
    pub fn with_top_p(mut self, p: f32) -> Self {
        self.top_p = Some(p.clamp(0.0, 1.0));
        self
    }

    /// Add a stop sequence
    pub fn with_stop(mut self, seq: impl Into<String>) -> Self {
        self.stop_sequences.push(seq.into());
        self
    }
}

/// A subtask within a decomposable task
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Subtask {
    /// Unique subtask ID
    pub id: String,
    /// Prompt for this subtask
    pub prompt: String,
    /// Inference parameters
    pub params: InferenceParams,
    /// Estimated tokens for this subtask
    pub estimated_tokens: u64,
}

impl Subtask {
    /// Create a new subtask
    pub fn new(id: impl Into<String>, prompt: impl Into<String>, estimated_tokens: u64) -> Self {
        Self {
            id: id.into(),
            prompt: prompt.into(),
            params: InferenceParams::default(),
            estimated_tokens,
        }
    }

    /// Set inference parameters
    pub fn with_params(mut self, params: InferenceParams) -> Self {
        self.params = params;
        self
    }
}

/// Decomposable task with subtasks and dependencies
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecomposableTask {
    /// Task description
    pub description: String,
    /// List of subtasks
    pub subtasks: Vec<Subtask>,
    /// Dependencies between subtasks (from_index, to_index)
    pub dependencies: Vec<(usize, usize)>,
}

impl DecomposableTask {
    /// Create a new decomposable task
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
            subtasks: Vec::new(),
            dependencies: Vec::new(),
        }
    }

    /// Add a subtask
    pub fn add_subtask(&mut self, subtask: Subtask) -> usize {
        self.subtasks.push(subtask);
        self.subtasks.len() - 1
    }

    /// Add a dependency between subtasks
    pub fn add_dependency(&mut self, from_idx: usize, to_idx: usize) -> Result<(), String> {
        if from_idx >= self.subtasks.len() {
            return Err(format!("Invalid from_idx: {}", from_idx));
        }
        if to_idx >= self.subtasks.len() {
            return Err(format!("Invalid to_idx: {}", to_idx));
        }
        if from_idx == to_idx {
            return Err("Cannot create self-dependency".to_string());
        }

        self.dependencies.push((from_idx, to_idx));
        Ok(())
    }

    /// Get subtasks that have no dependencies
    pub fn root_subtasks(&self) -> Vec<usize> {
        (0..self.subtasks.len())
            .filter(|&idx| !self.dependencies.iter().any(|(_, to)| *to == idx))
            .collect()
    }

    /// Get subtasks that depend on a given subtask
    pub fn dependent_subtasks(&self, idx: usize) -> Vec<usize> {
        self.dependencies
            .iter()
            .filter(|(from, _)| *from == idx)
            .map(|(_, to)| *to)
            .collect()
    }
}

/// Coalition compute request
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoalitionComputeRequest {
    /// Unique request ID
    pub id: String,
    /// Decomposable task to execute
    pub task: DecomposableTask,
    /// Budget in satoshis
    pub budget_sats: u64,
    /// Parallelism strategy
    pub parallelism: ParallelismStrategy,
    /// Aggregation strategy
    pub aggregation: AggregationStrategy,
}

impl CoalitionComputeRequest {
    /// Create a new compute request
    pub fn new(id: impl Into<String>, task: DecomposableTask, budget_sats: u64) -> Self {
        Self {
            id: id.into(),
            task,
            budget_sats,
            parallelism: ParallelismStrategy::Parallel,
            aggregation: AggregationStrategy::Concatenate,
        }
    }

    /// Set parallelism strategy
    pub fn with_parallelism(mut self, strategy: ParallelismStrategy) -> Self {
        self.parallelism = strategy;
        self
    }

    /// Set aggregation strategy
    pub fn with_aggregation(mut self, strategy: AggregationStrategy) -> Self {
        self.aggregation = strategy;
        self
    }
}

/// Result from a subtask execution
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubtaskResult {
    /// Subtask ID
    pub subtask_id: String,
    /// Provider who executed this subtask
    pub provider_id: String,
    /// Output from the subtask
    pub output: String,
    /// Tokens processed
    pub tokens_processed: u64,
    /// Cost in satoshis
    pub cost_sats: u64,
}

impl SubtaskResult {
    /// Create a new subtask result
    pub fn new(
        subtask_id: impl Into<String>,
        provider_id: impl Into<String>,
        output: impl Into<String>,
        tokens_processed: u64,
        cost_sats: u64,
    ) -> Self {
        Self {
            subtask_id: subtask_id.into(),
            provider_id: provider_id.into(),
            output: output.into(),
            tokens_processed,
            cost_sats,
        }
    }
}

/// Provider contribution to coalition compute
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderContribution {
    /// Provider ID
    pub provider_id: String,
    /// Subtask IDs completed by this provider
    pub subtasks_completed: Vec<String>,
    /// Total tokens processed
    pub tokens_processed: u64,
    /// Total cost in satoshis
    pub cost_sats: u64,
    /// Share percentage of total work
    pub share_pct: f32,
}

impl ProviderContribution {
    /// Create a new provider contribution
    pub fn new(provider_id: impl Into<String>) -> Self {
        Self {
            provider_id: provider_id.into(),
            subtasks_completed: Vec::new(),
            tokens_processed: 0,
            cost_sats: 0,
            share_pct: 0.0,
        }
    }

    /// Add a completed subtask
    pub fn add_subtask(&mut self, subtask_id: impl Into<String>, tokens: u64, cost_sats: u64) {
        self.subtasks_completed.push(subtask_id.into());
        self.tokens_processed += tokens;
        self.cost_sats += cost_sats;
    }

    /// Calculate share percentage based on total work
    pub fn calculate_share(&mut self, total_tokens: u64) {
        if total_tokens > 0 {
            self.share_pct = (self.tokens_processed as f64 / total_tokens as f64) as f32;
        }
    }
}

/// Coalition compute result
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoalitionResult {
    /// Request ID this result is for
    pub request_id: String,
    /// Results from all subtasks
    pub subtask_results: Vec<SubtaskResult>,
    /// Aggregated final output
    pub aggregated_output: String,
    /// Total cost in satoshis
    pub total_cost_sats: u64,
    /// Contributions from each provider
    pub provider_contributions: Vec<ProviderContribution>,
}

impl CoalitionResult {
    /// Create a new coalition result
    pub fn new(request_id: impl Into<String>, aggregated_output: impl Into<String>) -> Self {
        Self {
            request_id: request_id.into(),
            subtask_results: Vec::new(),
            aggregated_output: aggregated_output.into(),
            total_cost_sats: 0,
            provider_contributions: Vec::new(),
        }
    }

    /// Add a subtask result
    pub fn add_subtask_result(&mut self, result: SubtaskResult) {
        self.total_cost_sats += result.cost_sats;
        self.subtask_results.push(result);
    }

    /// Finalize contributions by calculating shares
    pub fn finalize_contributions(&mut self) {
        let total_tokens: u64 = self
            .subtask_results
            .iter()
            .map(|r| r.tokens_processed)
            .sum();

        for contribution in &mut self.provider_contributions {
            contribution.calculate_share(total_tokens);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parallelism_strategy_is_redundant() {
        assert!(!ParallelismStrategy::Parallel.is_redundant());
        assert!(!ParallelismStrategy::Pipeline.is_redundant());
        assert!(ParallelismStrategy::Redundant { count: 3 }.is_redundant());
        assert!(!ParallelismStrategy::Adaptive.is_redundant());
    }

    #[test]
    fn test_parallelism_strategy_redundancy_count() {
        assert_eq!(ParallelismStrategy::Parallel.redundancy_count(), None);
        assert_eq!(
            ParallelismStrategy::Redundant { count: 3 }.redundancy_count(),
            Some(3)
        );
    }

    #[test]
    fn test_aggregation_strategy_custom() {
        let strategy = AggregationStrategy::custom("Combine the outputs intelligently");
        match strategy {
            AggregationStrategy::Custom { prompt } => {
                assert_eq!(prompt, "Combine the outputs intelligently");
            }
            _ => panic!("Expected Custom variant"),
        }
    }

    #[test]
    fn test_inference_params_builder() {
        let params = InferenceParams::new()
            .with_temperature(0.7)
            .with_max_tokens(2048)
            .with_top_p(0.9)
            .with_stop("END");

        assert_eq!(params.temperature, 0.7);
        assert_eq!(params.max_tokens, 2048);
        assert_eq!(params.top_p, Some(0.9));
        assert_eq!(params.stop_sequences, vec!["END"]);
    }

    #[test]
    fn test_inference_params_temperature_clamping() {
        let params = InferenceParams::new().with_temperature(3.0);
        assert_eq!(params.temperature, 2.0);

        let params = InferenceParams::new().with_temperature(-1.0);
        assert_eq!(params.temperature, 0.0);
    }

    #[test]
    fn test_subtask_new() {
        let subtask = Subtask::new("task1", "Write a function", 500);
        assert_eq!(subtask.id, "task1");
        assert_eq!(subtask.prompt, "Write a function");
        assert_eq!(subtask.estimated_tokens, 500);
    }

    #[test]
    fn test_decomposable_task_add_subtask() {
        let mut task = DecomposableTask::new("Complex task");
        let idx = task.add_subtask(Subtask::new("sub1", "Step 1", 100));
        assert_eq!(idx, 0);
        assert_eq!(task.subtasks.len(), 1);
    }

    #[test]
    fn test_decomposable_task_add_dependency() {
        let mut task = DecomposableTask::new("Pipeline task");
        task.add_subtask(Subtask::new("sub1", "Step 1", 100));
        task.add_subtask(Subtask::new("sub2", "Step 2", 100));
        task.add_subtask(Subtask::new("sub3", "Step 3", 100));

        assert!(task.add_dependency(0, 1).is_ok());
        assert!(task.add_dependency(1, 2).is_ok());
        assert_eq!(task.dependencies.len(), 2);

        // Invalid dependencies
        assert!(task.add_dependency(0, 0).is_err()); // Self-dependency
        assert!(task.add_dependency(5, 1).is_err()); // Invalid from_idx
        assert!(task.add_dependency(0, 5).is_err()); // Invalid to_idx
    }

    #[test]
    fn test_decomposable_task_root_subtasks() {
        let mut task = DecomposableTask::new("Pipeline task");
        task.add_subtask(Subtask::new("sub1", "Step 1", 100));
        task.add_subtask(Subtask::new("sub2", "Step 2", 100));
        task.add_subtask(Subtask::new("sub3", "Step 3", 100));

        task.add_dependency(0, 1).unwrap();
        task.add_dependency(1, 2).unwrap();

        let roots = task.root_subtasks();
        assert_eq!(roots, vec![0]); // Only sub1 has no dependencies
    }

    #[test]
    fn test_decomposable_task_dependent_subtasks() {
        let mut task = DecomposableTask::new("Pipeline task");
        task.add_subtask(Subtask::new("sub1", "Step 1", 100));
        task.add_subtask(Subtask::new("sub2", "Step 2", 100));
        task.add_subtask(Subtask::new("sub3", "Step 3", 100));

        task.add_dependency(0, 1).unwrap();
        task.add_dependency(0, 2).unwrap();

        let dependents = task.dependent_subtasks(0);
        assert_eq!(dependents.len(), 2);
        assert!(dependents.contains(&1));
        assert!(dependents.contains(&2));
    }

    #[test]
    fn test_coalition_compute_request_builder() {
        let task = DecomposableTask::new("Test task");
        let request = CoalitionComputeRequest::new("req1", task, 10_000)
            .with_parallelism(ParallelismStrategy::Redundant { count: 3 })
            .with_aggregation(AggregationStrategy::Vote);

        assert_eq!(request.id, "req1");
        assert_eq!(request.budget_sats, 10_000);
        assert!(request.parallelism.is_redundant());
        assert_eq!(request.aggregation, AggregationStrategy::Vote);
    }

    #[test]
    fn test_subtask_result_new() {
        let result = SubtaskResult::new("sub1", "provider1", "output data", 500, 1_000);
        assert_eq!(result.subtask_id, "sub1");
        assert_eq!(result.provider_id, "provider1");
        assert_eq!(result.output, "output data");
        assert_eq!(result.tokens_processed, 500);
        assert_eq!(result.cost_sats, 1_000);
    }

    #[test]
    fn test_provider_contribution() {
        let mut contrib = ProviderContribution::new("provider1");
        contrib.add_subtask("sub1", 100, 200);
        contrib.add_subtask("sub2", 150, 300);

        assert_eq!(contrib.subtasks_completed.len(), 2);
        assert_eq!(contrib.tokens_processed, 250);
        assert_eq!(contrib.cost_sats, 500);

        contrib.calculate_share(1000);
        assert_eq!(contrib.share_pct, 0.25);
    }

    #[test]
    fn test_coalition_result() {
        let mut result = CoalitionResult::new("req1", "Final output");

        result.add_subtask_result(SubtaskResult::new("sub1", "provider1", "out1", 100, 200));
        result.add_subtask_result(SubtaskResult::new("sub2", "provider2", "out2", 150, 300));

        assert_eq!(result.subtask_results.len(), 2);
        assert_eq!(result.total_cost_sats, 500);

        let mut contrib1 = ProviderContribution::new("provider1");
        contrib1.add_subtask("sub1", 100, 200);
        let mut contrib2 = ProviderContribution::new("provider2");
        contrib2.add_subtask("sub2", 150, 300);

        result.provider_contributions = vec![contrib1, contrib2];
        result.finalize_contributions();

        assert_eq!(result.provider_contributions[0].share_pct, 0.4);
        assert_eq!(result.provider_contributions[1].share_pct, 0.6);
    }

    #[test]
    fn test_coalition_compute_serde() {
        let task = DecomposableTask::new("Test task");
        let request = CoalitionComputeRequest::new("req1", task, 10_000);

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: CoalitionComputeRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, request.id);
        assert_eq!(deserialized.budget_sats, request.budget_sats);
    }
}
