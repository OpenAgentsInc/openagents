use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Summary metrics for the entire trajectory.
///
/// All fields are optional as this entire object is optional.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FinalMetrics {
    /// Sum of all prompt tokens across all steps in the trajectory
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_prompt_tokens: Option<i64>,

    /// Sum of all completion tokens across all steps in the trajectory
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_completion_tokens: Option<i64>,

    /// Sum of all cached tokens across all steps in the trajectory
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_cached_tokens: Option<i64>,

    /// Total real monetary cost for the entire trajectory in USD
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_cost_usd: Option<f64>,

    /// Total number of steps
    ///
    /// Can be unequal to length of steps array if explained in notes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_steps: Option<i64>,

    /// Custom aggregate metrics not covered by the core schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<Value>,
}

impl FinalMetrics {
    /// Create new empty final metrics
    pub fn new() -> Self {
        Self {
            total_prompt_tokens: None,
            total_completion_tokens: None,
            total_cached_tokens: None,
            total_cost_usd: None,
            total_steps: None,
            extra: None,
        }
    }

    /// Calculate total cost using the standard formula
    ///
    /// ```
    /// non_cached_total = total_prompt_tokens - total_cached_tokens
    /// total_cost_usd = (non_cached_total × cost_per_input_token) +
    ///                  (total_cached_tokens × cost_per_cached_token) +
    ///                  (total_completion_tokens × cost_per_completion_token)
    /// ```
    pub fn calculate_cost(
        &self,
        cost_per_input_token: f64,
        cost_per_cached_token: f64,
        cost_per_completion_token: f64,
    ) -> Option<f64> {
        let prompt_tokens = self.total_prompt_tokens? as f64;
        let completion_tokens = self.total_completion_tokens.unwrap_or(0) as f64;
        let cached_tokens = self.total_cached_tokens.unwrap_or(0) as f64;

        let non_cached_total = prompt_tokens - cached_tokens;
        let cost = (non_cached_total * cost_per_input_token)
            + (cached_tokens * cost_per_cached_token)
            + (completion_tokens * cost_per_completion_token);

        Some(cost)
    }
}

impl Default for FinalMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cost_calculation() {
        let metrics = FinalMetrics {
            total_prompt_tokens: Some(1000),
            total_completion_tokens: Some(500),
            total_cached_tokens: Some(200),
            total_cost_usd: None,
            total_steps: Some(10),
            extra: None,
        };

        let cost = metrics.calculate_cost(0.001, 0.0001, 0.002).unwrap();
        // (800 * 0.001) + (200 * 0.0001) + (500 * 0.002) = 0.8 + 0.02 + 1.0 = 1.82
        assert!((cost - 1.82).abs() < 0.0001);
    }
}
