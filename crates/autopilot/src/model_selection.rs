//! Model selection based on task complexity analysis
//!
//! This module implements intelligent model selection for autopilot runs by analyzing:
//! - Historical performance of each model on similar tasks
//! - Task complexity indicators (keywords, issue type, directive complexity)
//! - Cost vs performance tradeoffs
//!
//! # Task Complexity Indicators
//!
//! ## Low Complexity (Haiku)
//! - Simple documentation updates
//! - Typo fixes
//! - Running existing tests
//! - Simple refactoring (rename, move files)
//! - Tasks with clear, specific instructions
//!
//! ## Medium Complexity (Sonnet - default)
//! - Feature implementation (most cases)
//! - Bug fixes requiring investigation
//! - Writing tests
//! - Moderate refactoring
//! - Integration work
//!
//! ## High Complexity (Opus)
//! - Architectural design decisions
//! - Complex debugging (race conditions, memory issues)
//! - Cross-cutting refactors affecting multiple crates
//! - Protocol implementations (NIPs, cryptography)
//! - Performance optimization requiring deep analysis

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::metrics::{MetricsDb, SessionMetrics, SessionStatus};

/// Available models ranked by capability/cost
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Model {
    /// Fast, cheap model for simple tasks
    Haiku,
    /// Balanced model for most tasks (default)
    Sonnet,
    /// Most capable model for complex tasks
    Opus,
}

impl Model {
    pub fn as_str(&self) -> &'static str {
        match self {
            Model::Haiku => "haiku",
            Model::Sonnet => "sonnet",
            Model::Opus => "opus",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "haiku" => Some(Model::Haiku),
            "sonnet" => Some(Model::Sonnet),
            "opus" => Some(Model::Opus),
            _ => None,
        }
    }

    /// Relative cost multiplier (haiku = 1x baseline)
    pub fn cost_multiplier(&self) -> f64 {
        match self {
            Model::Haiku => 1.0,
            Model::Sonnet => 3.0,
            Model::Opus => 15.0,
        }
    }
}

/// Task complexity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum ComplexityLevel {
    #[default]
    Low,
    Medium,
    High,
}

/// Task complexity indicators extracted from issue/prompt
#[derive(Debug, Clone, Default)]
pub struct TaskComplexity {
    /// Overall complexity level
    pub level: ComplexityLevel,
    /// Individual indicator scores
    pub indicators: HashMap<String, f64>,
    /// Confidence in the assessment (0.0 to 1.0)
    pub confidence: f64,
}

impl TaskComplexity {
    /// Calculate complexity from task description
    pub fn analyze(title: &str, description: &str, directive_id: Option<&str>) -> Self {
        let mut indicators = HashMap::new();
        let full_text = format!("{} {}", title, description).to_lowercase();

        // Simple task indicators (favor Haiku)
        let simple_score = Self::score_keywords(
            &full_text,
            &[
                "typo",
                "documentation",
                "readme",
                "comment",
                "rename",
                "format",
                "clippy",
                "test run",
                "simple",
                "quick",
                "trivial",
            ],
        );
        indicators.insert("simple_keywords".to_string(), simple_score);

        // Complex task indicators (favor Opus)
        let complex_score = Self::score_keywords(
            &full_text,
            &[
                "architecture",
                "design",
                "protocol",
                "cryptography",
                "race condition",
                "memory leak",
                "performance",
                "optimization",
                "security",
                "nip-",
                "refactor",
                "cross-cutting",
                "investigate",
                "debug",
            ],
        );
        indicators.insert("complex_keywords".to_string(), complex_score);

        // Implementation task indicators (favor Sonnet)
        let implementation_score = Self::score_keywords(
            &full_text,
            &[
                "implement",
                "feature",
                "add",
                "create",
                "build",
                "fix",
                "bug",
                "integrate",
                "test",
            ],
        );
        indicators.insert("implementation_keywords".to_string(), implementation_score);

        // Directive complexity (certain directives are inherently complex)
        // When no directive, use a low default so simple tasks stay Low complexity
        let directive_score = directive_id
            .map(|id| match id {
                "d-001" | "d-002" | "d-006" | "d-007" => 1.0, // Protocol/crypto work
                "d-004" | "d-012" | "d-013" => 0.8,           // Quality/testing
                "d-003" | "d-008" | "d-009" => 0.6,           // Feature work
                _ => 0.4,
            })
            .unwrap_or(0.2);
        indicators.insert("directive_complexity".to_string(), directive_score);

        // Text length indicator (longer = more complex)
        let length_score = (full_text.len() as f64 / 500.0).min(1.0);
        indicators.insert("text_length".to_string(), length_score);

        // Calculate overall complexity level
        let weighted_score = simple_score * -1.0 // Simple keywords reduce complexity
            + complex_score * 2.0                 // Complex keywords strongly increase
            + implementation_score * 0.5          // Implementation is moderate
            + directive_score * 1.5               // Directive context is important
            + length_score * 0.3; // Length has minor influence

        let level = if weighted_score < 0.3 {
            ComplexityLevel::Low
        } else if weighted_score < 1.5 {
            ComplexityLevel::Medium
        } else {
            ComplexityLevel::High
        };

        // Confidence is higher when signals are clear
        let signal_strength = (simple_score + complex_score + implementation_score) / 3.0;
        let confidence = signal_strength.max(0.3); // Minimum 30% confidence

        TaskComplexity {
            level,
            indicators,
            confidence,
        }
    }

    /// Score how many keywords from the list appear in text
    fn score_keywords(text: &str, keywords: &[&str]) -> f64 {
        let matches = keywords.iter().filter(|kw| text.contains(*kw)).count();
        (matches as f64 / keywords.len() as f64).min(1.0)
    }
}

/// Model performance statistics for a specific complexity level
#[derive(Debug, Clone)]
pub struct ModelPerformance {
    pub model: Model,
    pub sessions_count: usize,
    pub completion_rate: f64,
    pub avg_duration_seconds: f64,
    pub avg_cost_usd: f64,
    pub avg_tool_error_rate: f64,
}

/// Model selector that chooses optimal model based on task and historical data
pub struct ModelSelector<'a> {
    db: &'a MetricsDb,
}

impl<'a> ModelSelector<'a> {
    pub fn new(db: &'a MetricsDb) -> Self {
        Self { db }
    }

    /// Select the best model for a given task
    ///
    /// # Arguments
    /// * `title` - Issue title
    /// * `description` - Issue description
    /// * `directive_id` - Optional directive ID
    /// * `prefer_cost` - Prefer lower cost models when quality is similar
    ///
    /// # Returns
    /// Recommended model with confidence score and rationale
    pub fn select_model(
        &self,
        title: &str,
        description: &str,
        directive_id: Option<&str>,
        prefer_cost: bool,
    ) -> Result<ModelRecommendation> {
        // Analyze task complexity
        let complexity = TaskComplexity::analyze(title, description, directive_id);

        // Get historical performance for each model
        let performances = self.get_model_performances()?;

        // Make recommendation based on complexity and performance
        let model = match complexity.level {
            ComplexityLevel::Low => {
                // For simple tasks, use Haiku if it has decent performance, otherwise Sonnet
                if let Some(haiku_perf) = performances.get(&Model::Haiku) {
                    if haiku_perf.completion_rate > 0.7 && haiku_perf.avg_tool_error_rate < 0.3 {
                        Model::Haiku
                    } else {
                        Model::Sonnet
                    }
                } else {
                    Model::Haiku // No data, try Haiku for simple tasks
                }
            }
            ComplexityLevel::Medium => {
                // For medium tasks, Sonnet is usually best
                // But if prefer_cost and Haiku performs well, suggest Haiku
                if prefer_cost {
                    if let Some(haiku_perf) = performances.get(&Model::Haiku) {
                        if haiku_perf.completion_rate > 0.8
                            && haiku_perf.avg_tool_error_rate < 0.2
                        {
                            Model::Haiku
                        } else {
                            Model::Sonnet
                        }
                    } else {
                        Model::Sonnet
                    }
                } else {
                    Model::Sonnet
                }
            }
            ComplexityLevel::High => {
                // For complex tasks, use Opus unless its performance is poor
                if let Some(opus_perf) = performances.get(&Model::Opus) {
                    if opus_perf.completion_rate < 0.5 {
                        // Opus struggling, fall back to Sonnet
                        Model::Sonnet
                    } else {
                        Model::Opus
                    }
                } else {
                    Model::Opus // No data, try Opus for complex tasks
                }
            }
        };

        // Generate rationale
        let confidence = complexity.confidence;
        let rationale = self.generate_rationale(&model, &complexity, &performances);

        Ok(ModelRecommendation {
            model,
            complexity,
            confidence,
            rationale,
            estimated_cost: self.estimate_cost(&model, &performances),
        })
    }

    /// Get performance statistics for each model from historical data
    fn get_model_performances(&self) -> Result<HashMap<Model, ModelPerformance>> {
        let mut performances = HashMap::new();

        // Get recent sessions (last 100)
        let sessions = self.db.get_recent_sessions(100)?;

        for model_type in &[Model::Haiku, Model::Sonnet, Model::Opus] {
            let model_sessions: Vec<&SessionMetrics> = sessions
                .iter()
                .filter(|s| {
                    Model::from_str(&s.model)
                        .map(|m| m == *model_type)
                        .unwrap_or(false)
                })
                .collect();

            if model_sessions.is_empty() {
                continue;
            }

            let completed = model_sessions
                .iter()
                .filter(|s| s.final_status == SessionStatus::Completed)
                .count();

            let completion_rate = completed as f64 / model_sessions.len() as f64;

            let avg_duration_seconds = model_sessions
                .iter()
                .map(|s| s.duration_seconds)
                .sum::<f64>()
                / model_sessions.len() as f64;

            let avg_cost_usd = model_sessions.iter().map(|s| s.cost_usd).sum::<f64>()
                / model_sessions.len() as f64;

            let avg_tool_error_rate = model_sessions
                .iter()
                .filter(|s| s.tool_calls > 0)
                .map(|s| s.tool_errors as f64 / s.tool_calls as f64)
                .sum::<f64>()
                / model_sessions
                    .iter()
                    .filter(|s| s.tool_calls > 0)
                    .count()
                    .max(1) as f64;

            performances.insert(
                *model_type,
                ModelPerformance {
                    model: *model_type,
                    sessions_count: model_sessions.len(),
                    completion_rate,
                    avg_duration_seconds,
                    avg_cost_usd,
                    avg_tool_error_rate,
                },
            );
        }

        Ok(performances)
    }

    fn generate_rationale(
        &self,
        model: &Model,
        complexity: &TaskComplexity,
        performances: &HashMap<Model, ModelPerformance>,
    ) -> String {
        let mut parts = Vec::new();

        // Complexity reasoning
        parts.push(format!(
            "Task complexity: {:?} (confidence: {:.0}%)",
            complexity.level,
            complexity.confidence * 100.0
        ));

        // Model performance reasoning
        if let Some(perf) = performances.get(model) {
            parts.push(format!(
                "{} has {:.0}% completion rate with {:.1}% error rate across {} sessions",
                model.as_str(),
                perf.completion_rate * 100.0,
                perf.avg_tool_error_rate * 100.0,
                perf.sessions_count
            ));
        }

        // Key indicators
        let top_indicators: Vec<_> = complexity
            .indicators
            .iter()
            .filter(|(_, score)| **score > 0.3)
            .map(|(name, score)| format!("{}: {:.1}%", name, score * 100.0))
            .collect();

        if !top_indicators.is_empty() {
            parts.push(format!("Key indicators: {}", top_indicators.join(", ")));
        }

        parts.join(". ")
    }

    fn estimate_cost(
        &self,
        model: &Model,
        performances: &HashMap<Model, ModelPerformance>,
    ) -> f64 {
        if let Some(perf) = performances.get(model) {
            perf.avg_cost_usd
        } else {
            // Rough estimates if no data
            match model {
                Model::Haiku => 0.01,
                Model::Sonnet => 0.05,
                Model::Opus => 0.25,
            }
        }
    }
}

/// Model recommendation output
#[derive(Debug, Serialize, Deserialize)]
pub struct ModelRecommendation {
    /// Recommended model
    pub model: Model,
    /// Task complexity analysis
    pub complexity: TaskComplexity,
    /// Confidence in the recommendation (0.0 to 1.0)
    pub confidence: f64,
    /// Human-readable rationale
    pub rationale: String,
    /// Estimated cost in USD
    pub estimated_cost: f64,
}

impl TaskComplexity {
    // Implement Serialize/Deserialize manually since HashMap doesn't derive it automatically
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "level": format!("{:?}", self.level),
            "indicators": self.indicators,
            "confidence": self.confidence,
        })
    }
}

impl Serialize for TaskComplexity {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("TaskComplexity", 3)?;
        state.serialize_field("level", &format!("{:?}", self.level))?;
        state.serialize_field("indicators", &self.indicators)?;
        state.serialize_field("confidence", &self.confidence)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for TaskComplexity {
    fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // TaskComplexity is serialization-only; deserialization returns default
        // This is used for JSON output in analysis results
        Ok(TaskComplexity::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complexity_simple_task() {
        let complexity = TaskComplexity::analyze(
            "Fix typo in README",
            "Simple documentation typo fix",
            None,
        );

        assert_eq!(complexity.level, ComplexityLevel::Low);
        assert!(complexity.indicators.get("simple_keywords").unwrap() > &0.0);
    }

    #[test]
    fn test_complexity_medium_task() {
        let complexity = TaskComplexity::analyze(
            "Implement user authentication",
            "Add JWT-based authentication to the API endpoints",
            Some("d-003"),
        );

        assert_eq!(complexity.level, ComplexityLevel::Medium);
        assert!(complexity.indicators.get("implementation_keywords").unwrap() > &0.0);
    }

    #[test]
    fn test_complexity_high_task() {
        let complexity = TaskComplexity::analyze(
            "Implement NIP-SA protocol",
            "Design and implement cryptographic threshold signature protocol for sovereign agents",
            Some("d-006"),
        );

        assert_eq!(complexity.level, ComplexityLevel::High);
        assert!(complexity.indicators.get("complex_keywords").unwrap() > &0.0);
        assert!(complexity.indicators.get("directive_complexity").unwrap() > &0.8);
    }

    #[test]
    fn test_model_from_str() {
        assert_eq!(Model::from_str("haiku"), Some(Model::Haiku));
        assert_eq!(Model::from_str("SONNET"), Some(Model::Sonnet));
        assert_eq!(Model::from_str("opus"), Some(Model::Opus));
        assert_eq!(Model::from_str("invalid"), None);
    }

    #[test]
    fn test_cost_multipliers() {
        assert_eq!(Model::Haiku.cost_multiplier(), 1.0);
        assert_eq!(Model::Sonnet.cost_multiplier(), 3.0);
        assert_eq!(Model::Opus.cost_multiplier(), 15.0);
    }
}
