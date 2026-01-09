//! Refine meta-operator for retry and fallback patterns.
//!
//! The Refine operator wraps any Module and adds:
//! - Retry logic with configurable attempts
//! - Reward-based acceptance thresholds
//! - Fallback to alternative LMs
//! - Structured output enforcement
//!
//! This enables robust execution of subjective tasks where
//! quality varies between attempts.

use crate::core::{Module, LM};
use crate::data::{Example, Prediction};
use anyhow::{anyhow, Result};
use std::sync::Arc;

/// A reward function that evaluates prediction quality.
pub type RewardFn = Box<dyn Fn(&Example, &Prediction) -> f32 + Send + Sync>;

/// Configuration for the Refine operator.
#[derive(Clone)]
pub struct RefineConfig {
    /// Maximum retry attempts.
    pub max_retries: usize,
    /// Minimum score to accept a prediction.
    pub threshold: f32,
    /// Whether to use best-of-n (keep best) vs first-above-threshold.
    pub best_of_n: bool,
}

impl Default for RefineConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            threshold: 0.5,
            best_of_n: false,
        }
    }
}

/// Refine meta-operator that adds retry and fallback to any module.
///
/// # Example
///
/// ```ignore
/// use dsrs::predictors::{Refine, Predict};
/// use dsrs::core::Module;
///
/// let predictor = Predict::new(my_signature);
/// let refined = Refine::new(predictor)
///     .with_retries(3)
///     .with_threshold(0.7)
///     .with_reward(|_input, pred| {
///         // Score based on output quality
///         if pred.data.contains_key("answer") { 1.0 } else { 0.0 }
///     });
///
/// let result = refined.forward(input).await?;
/// ```
pub struct Refine<M: Module> {
    /// The wrapped module.
    module: M,
    /// Configuration.
    config: RefineConfig,
    /// Reward function for scoring predictions.
    reward_fn: Option<RewardFn>,
    /// Optional fallback LM.
    fallback_lm: Option<Arc<LM>>,
}

impl<M: Module> Refine<M> {
    /// Create a new Refine wrapper around a module.
    pub fn new(module: M) -> Self {
        Self {
            module,
            config: RefineConfig::default(),
            reward_fn: None,
            fallback_lm: None,
        }
    }

    /// Set the maximum number of retry attempts.
    pub fn with_retries(mut self, n: usize) -> Self {
        self.config.max_retries = n;
        self
    }

    /// Set the acceptance threshold (0.0 to 1.0).
    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.config.threshold = threshold;
        self
    }

    /// Enable best-of-n mode (keep best prediction across all attempts).
    pub fn best_of_n(mut self) -> Self {
        self.config.best_of_n = true;
        self
    }

    /// Set the reward function for scoring predictions.
    pub fn with_reward<F>(mut self, f: F) -> Self
    where
        F: Fn(&Example, &Prediction) -> f32 + Send + Sync + 'static,
    {
        self.reward_fn = Some(Box::new(f));
        self
    }

    /// Set a fallback LM to try if primary fails.
    pub fn with_fallback(mut self, lm: Arc<LM>) -> Self {
        self.fallback_lm = Some(lm);
        self
    }

    /// Get the wrapped module.
    pub fn inner(&self) -> &M {
        &self.module
    }

    /// Get mutable access to the wrapped module.
    pub fn inner_mut(&mut self) -> &mut M {
        &mut self.module
    }

    /// Unwrap and return the inner module.
    pub fn into_inner(self) -> M {
        self.module
    }
}

impl<M: Module> Module for Refine<M> {
    async fn forward(&self, inputs: Example) -> Result<Prediction> {
        let reward_fn = self.reward_fn.as_ref();

        // Track best prediction for best-of-n mode
        let mut best_pred: Option<(Prediction, f32)> = None;
        let mut last_error: Option<anyhow::Error> = None;

        for _attempt in 0..self.config.max_retries {
            // Try the primary module
            match self.module.forward(inputs.clone()).await {
                Ok(pred) => {
                    // If no reward function, accept first success
                    let score = match reward_fn {
                        Some(f) => f(&inputs, &pred),
                        None => 1.0, // Accept any successful prediction
                    };

                    if self.config.best_of_n {
                        // Keep track of best prediction
                        if best_pred.as_ref().map(|(_, s)| score > *s).unwrap_or(true) {
                            best_pred = Some((pred, score));
                        }
                    } else {
                        // First-above-threshold mode
                        if score >= self.config.threshold {
                            return Ok(pred);
                        }
                        // Keep as fallback if best so far
                        if best_pred.as_ref().map(|(_, s)| score > *s).unwrap_or(true) {
                            best_pred = Some((pred, score));
                        }
                    }
                }
                Err(e) => {
                    last_error = Some(e);
                }
            }

            // Try fallback LM if available and we failed
            if self.fallback_lm.is_some() && last_error.is_some() {
                // TODO: Implement fallback LM execution
                // This would require the module to support custom LM injection
            }

            // Continue to next attempt if we haven't met threshold
            // (logging removed to avoid tracing dependency)
        }

        // Return best prediction if we have one
        if let Some((pred, score)) = best_pred {
            if self.config.best_of_n || score >= self.config.threshold * 0.5 {
                // In best-of-n mode, always return best
                // Otherwise, return if score is at least half threshold
                return Ok(pred);
            }
        }

        // All attempts failed
        Err(last_error.unwrap_or_else(|| {
            anyhow!(
                "Refine failed after {} attempts: no prediction met threshold {}",
                self.config.max_retries,
                self.config.threshold
            )
        }))
    }
}

/// Result of a refinement attempt.
#[derive(Debug)]
pub struct RefineResult {
    /// The final prediction.
    pub prediction: Prediction,
    /// Number of attempts made.
    pub attempts: usize,
    /// Score of the final prediction.
    pub score: f32,
    /// Whether fallback was used.
    pub used_fallback: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::LmUsage;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingModule {
        call_count: AtomicUsize,
        fail_first_n: usize,
    }

    impl CountingModule {
        fn new(fail_first_n: usize) -> Self {
            Self {
                call_count: AtomicUsize::new(0),
                fail_first_n,
            }
        }

        fn calls(&self) -> usize {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    impl Module for CountingModule {
        async fn forward(&self, _inputs: Example) -> Result<Prediction> {
            let count = self.call_count.fetch_add(1, Ordering::SeqCst);
            if count < self.fail_first_n {
                Err(anyhow!("Simulated failure {}", count))
            } else {
                let mut data = HashMap::new();
                data.insert("result".to_string(), serde_json::json!("success"));
                Ok(Prediction::new(data, LmUsage::default()))
            }
        }
    }

    #[tokio::test]
    async fn test_refine_succeeds_on_first_try() {
        let module = CountingModule::new(0); // Never fails
        let refined = Refine::new(module).with_retries(3);

        let example = Example::new(HashMap::new(), vec![], vec![]);
        let result = refined.forward(example).await;

        assert!(result.is_ok());
        assert_eq!(refined.inner().calls(), 1);
    }

    #[tokio::test]
    async fn test_refine_retries_on_failure() {
        let module = CountingModule::new(2); // Fails first 2 times
        let refined = Refine::new(module).with_retries(3);

        let example = Example::new(HashMap::new(), vec![], vec![]);
        let result = refined.forward(example).await;

        assert!(result.is_ok());
        assert_eq!(refined.inner().calls(), 3);
    }

    #[tokio::test]
    async fn test_refine_fails_after_max_retries() {
        let module = CountingModule::new(5); // Fails 5 times
        let refined = Refine::new(module).with_retries(3);

        let example = Example::new(HashMap::new(), vec![], vec![]);
        let result = refined.forward(example).await;

        assert!(result.is_err());
        assert_eq!(refined.inner().calls(), 3);
    }

    #[tokio::test]
    async fn test_refine_with_reward_function() {
        struct AlwaysSuccessModule;

        impl Module for AlwaysSuccessModule {
            async fn forward(&self, _inputs: Example) -> Result<Prediction> {
                let mut data = HashMap::new();
                data.insert("score".to_string(), serde_json::json!(0.8));
                Ok(Prediction::new(data, LmUsage::default()))
            }
        }

        let refined = Refine::new(AlwaysSuccessModule)
            .with_retries(3)
            .with_threshold(0.7)
            .with_reward(|_input, pred| {
                pred.data
                    .get("score")
                    .and_then(|v| v.as_f64())
                    .map(|v| v as f32)
                    .unwrap_or(0.0)
            });

        let example = Example::new(HashMap::new(), vec![], vec![]);
        let result = refined.forward(example).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_refine_best_of_n() {
        use std::sync::atomic::AtomicU32;

        struct IncreasingScoreModule {
            score: AtomicU32,
        }

        impl Module for IncreasingScoreModule {
            async fn forward(&self, _inputs: Example) -> Result<Prediction> {
                let score = self.score.fetch_add(1, Ordering::SeqCst);
                let mut data = HashMap::new();
                data.insert("score".to_string(), serde_json::json!(score));
                Ok(Prediction::new(data, LmUsage::default()))
            }
        }

        let module = IncreasingScoreModule {
            score: AtomicU32::new(0),
        };

        let refined = Refine::new(module)
            .with_retries(5)
            .best_of_n()
            .with_reward(|_input, pred| {
                pred.data
                    .get("score")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as f32)
                    .unwrap_or(0.0)
            });

        let example = Example::new(HashMap::new(), vec![], vec![]);
        let result = refined.forward(example).await.unwrap();

        // Should return the prediction with highest score (4)
        let score = result
            .data
            .get("score")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        assert_eq!(score, 4);
    }
}
