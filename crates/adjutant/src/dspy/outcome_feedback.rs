//! Outcome feedback - links task outcomes to decision training data.
//!
//! After an autopilot session completes, this module evaluates whether
//! each decision made during the session was "correct" based on the
//! actual outcome, and creates labeled training examples for optimization.

use super::sessions::{AutopilotSession, DecisionRecord, SessionOutcome};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Result of processing outcome feedback.
#[derive(Debug, Default)]
pub struct FeedbackResult {
    /// Number of decisions evaluated
    pub decisions_evaluated: usize,
    /// Number marked as correct
    pub correct_count: usize,
    /// Number marked as incorrect
    pub incorrect_count: usize,
    /// New labeled examples created
    pub examples_created: usize,
}

/// Training example with ground-truth outcome label.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabeledExample {
    /// Original training example type (complexity, delegation, rlm)
    pub example_type: String,
    /// The input that was provided to the decision pipeline
    pub input_summary: String,
    /// The output from the decision pipeline
    pub output: serde_json::Value,
    /// Predicted confidence at decision time
    pub predicted_confidence: f32,
    /// Ground truth: was the prediction correct?
    pub was_correct: bool,
    /// Session it came from
    pub session_id: String,
    /// Actual outcome that determined correctness
    pub outcome_summary: String,
    /// Iterations used in the session
    pub iterations_used: usize,
    /// Timestamp when labeled
    pub labeled_at: DateTime<Utc>,
}

/// Labeled examples storage by type.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LabeledExamplesStore {
    pub complexity: Vec<LabeledExample>,
    pub delegation: Vec<LabeledExample>,
    pub rlm_trigger: Vec<LabeledExample>,
    pub updated_at: DateTime<Utc>,
}

impl LabeledExamplesStore {
    /// Load from disk or create new.
    pub fn load() -> anyhow::Result<Self> {
        let path = Self::storage_path()?;
        if path.exists() {
            let content = fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&content).unwrap_or_default())
        } else {
            Ok(Self::default())
        }
    }

    /// Save to disk.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::storage_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    /// Get storage path.
    fn storage_path() -> anyhow::Result<PathBuf> {
        Ok(dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("No home directory"))?
            .join(".openagents")
            .join("adjutant")
            .join("training")
            .join("labeled_examples.json"))
    }

    /// Add a labeled example.
    pub fn add(&mut self, example: LabeledExample) {
        match example.example_type.as_str() {
            "complexity" => self.complexity.push(example),
            "delegation" => self.delegation.push(example),
            "rlm_trigger" | "rlm" => self.rlm_trigger.push(example),
            _ => tracing::warn!("Unknown example type: {}", example.example_type),
        }
        self.updated_at = Utc::now();
    }

    /// Get total count of all examples.
    pub fn total_count(&self) -> usize {
        self.complexity.len() + self.delegation.len() + self.rlm_trigger.len()
    }

    /// Get count by type.
    pub fn count_by_type(&self, example_type: &str) -> usize {
        match example_type {
            "complexity" => self.complexity.len(),
            "delegation" => self.delegation.len(),
            "rlm_trigger" | "rlm" => self.rlm_trigger.len(),
            _ => 0,
        }
    }

    /// Get examples since last optimization (for triggering new optimization).
    pub fn examples_since(&self, since: Option<DateTime<Utc>>) -> usize {
        let cutoff = since.unwrap_or_else(|| DateTime::UNIX_EPOCH.into());
        self.complexity.iter().filter(|e| e.labeled_at > cutoff).count()
            + self.delegation.iter().filter(|e| e.labeled_at > cutoff).count()
            + self.rlm_trigger.iter().filter(|e| e.labeled_at > cutoff).count()
    }

    /// Get accuracy for a decision type.
    pub fn accuracy(&self, example_type: &str) -> Option<f32> {
        let examples: &[LabeledExample] = match example_type {
            "complexity" => &self.complexity,
            "delegation" => &self.delegation,
            "rlm_trigger" | "rlm" => &self.rlm_trigger,
            _ => return None,
        };

        if examples.is_empty() {
            return None;
        }

        let correct = examples.iter().filter(|e| e.was_correct).count();
        Some(correct as f32 / examples.len() as f32)
    }
}

/// Links autopilot outcomes back to decision training data.
pub struct OutcomeFeedback {
    labeled_store: LabeledExamplesStore,
}

impl OutcomeFeedback {
    /// Create a new outcome feedback processor.
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            labeled_store: LabeledExamplesStore::load()?,
        })
    }

    /// Process a completed session and create labeled examples.
    pub fn process_session(&mut self, session: &AutopilotSession) -> anyhow::Result<FeedbackResult> {
        let outcome = session.outcome.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Session has no outcome"))?;

        let mut result = FeedbackResult::default();

        for decision in &session.decisions {
            let was_correct = self.evaluate_decision_correctness(
                decision,
                outcome,
                session.iterations_used,
            );

            result.decisions_evaluated += 1;
            if was_correct {
                result.correct_count += 1;
            } else {
                result.incorrect_count += 1;
            }

            // Create labeled example
            let labeled = LabeledExample {
                example_type: decision.decision_type.clone(),
                input_summary: decision.input_summary.clone(),
                output: decision.output.clone(),
                predicted_confidence: decision.predicted_confidence,
                was_correct,
                session_id: session.id.clone(),
                outcome_summary: self.summarize_outcome(outcome),
                iterations_used: session.iterations_used,
                labeled_at: Utc::now(),
            };

            self.labeled_store.add(labeled);
            result.examples_created += 1;
        }

        // Save updated store
        self.labeled_store.save()?;

        Ok(result)
    }

    /// Evaluate if a decision was correct based on the session outcome.
    fn evaluate_decision_correctness(
        &self,
        decision: &DecisionRecord,
        outcome: &SessionOutcome,
        iterations_used: usize,
    ) -> bool {
        match decision.decision_type.as_str() {
            "complexity" => self.is_complexity_correct(decision, outcome, iterations_used),
            "delegation" => self.is_delegation_correct(decision, outcome),
            "rlm_trigger" | "rlm" => self.is_rlm_correct(decision, outcome),
            _ => true, // Unknown decision types are considered correct
        }
    }

    /// Evaluate if complexity classification was correct.
    ///
    /// Logic:
    /// - Success within expected iterations for that complexity = correct
    /// - MaxIterationsReached when we predicted Low/Medium = incorrect
    fn is_complexity_correct(
        &self,
        decision: &DecisionRecord,
        outcome: &SessionOutcome,
        iterations_used: usize,
    ) -> bool {
        let predicted = decision.output.get("complexity")
            .and_then(|v| v.as_str())
            .unwrap_or("Medium");

        match outcome {
            SessionOutcome::Success { .. } => {
                // Correct if we didn't need too many iterations for the predicted complexity
                match predicted.to_lowercase().as_str() {
                    "low" => iterations_used <= 2,
                    "medium" => iterations_used <= 4,
                    "high" => iterations_used <= 7,
                    "veryhigh" | "very_high" => true, // Any success is good for VeryHigh
                    _ => true,
                }
            }
            SessionOutcome::MaxIterationsReached { .. } => {
                // Incorrect if we underestimated complexity
                matches!(predicted.to_lowercase().as_str(), "veryhigh" | "very_high")
            }
            SessionOutcome::Failed { .. } => {
                // Task failed - can't really judge complexity
                // But if we predicted Low for something that failed, likely wrong
                !matches!(predicted.to_lowercase().as_str(), "low")
            }
            _ => true, // Can't evaluate on interruption/error
        }
    }

    /// Evaluate if delegation decision was correct.
    ///
    /// Logic:
    /// - Success = delegation decision was correct (whatever we chose worked)
    /// - Failure when we didn't delegate = should have delegated
    fn is_delegation_correct(
        &self,
        decision: &DecisionRecord,
        outcome: &SessionOutcome,
    ) -> bool {
        let should_delegate = decision.output.get("should_delegate")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        match outcome {
            SessionOutcome::Success { .. } => {
                // Success means the decision was correct
                true
            }
            SessionOutcome::Failed { .. } | SessionOutcome::MaxIterationsReached { .. } => {
                // If we failed and didn't delegate, we probably should have
                // If we delegated but still failed, the task was just hard
                should_delegate // Delegated failures are less "wrong"
            }
            _ => true,
        }
    }

    /// Evaluate if RLM trigger decision was correct.
    ///
    /// Logic:
    /// - Success = RLM decision was correct
    /// - Large context + failure without RLM = should have used RLM
    fn is_rlm_correct(
        &self,
        decision: &DecisionRecord,
        outcome: &SessionOutcome,
    ) -> bool {
        let use_rlm = decision.output.get("use_rlm")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Try to get estimated tokens from input
        let estimated_tokens: usize = decision.input_summary
            .split("estimated_tokens")
            .nth(1)
            .and_then(|s| s.chars().filter(|c| c.is_ascii_digit()).take(10).collect::<String>().parse().ok())
            .unwrap_or(0);

        match outcome {
            SessionOutcome::Success { .. } => {
                // Success means the decision was correct
                true
            }
            SessionOutcome::Failed { .. } | SessionOutcome::MaxIterationsReached { .. } => {
                // If context was large and we didn't use RLM, likely incorrect
                if estimated_tokens > 50_000 && !use_rlm {
                    false
                } else {
                    true
                }
            }
            _ => true,
        }
    }

    /// Create a summary of the outcome for the labeled example.
    fn summarize_outcome(&self, outcome: &SessionOutcome) -> String {
        match outcome {
            SessionOutcome::Success { summary, verification_passed, .. } => {
                format!("Success (verified: {}): {}", verification_passed,
                    summary.chars().take(100).collect::<String>())
            }
            SessionOutcome::Failed { reason, .. } => {
                format!("Failed: {}", reason.chars().take(100).collect::<String>())
            }
            SessionOutcome::MaxIterationsReached { last_summary } => {
                format!("MaxIterations: {}",
                    last_summary.as_deref().unwrap_or("no summary").chars().take(100).collect::<String>())
            }
            SessionOutcome::UserInterrupted => "UserInterrupted".to_string(),
            SessionOutcome::Error(e) => format!("Error: {}", e.chars().take(100).collect::<String>()),
        }
    }

    /// Get the labeled examples store.
    pub fn store(&self) -> &LabeledExamplesStore {
        &self.labeled_store
    }

    /// Get mutable reference to the store.
    pub fn store_mut(&mut self) -> &mut LabeledExamplesStore {
        &mut self.labeled_store
    }
}

impl Default for OutcomeFeedback {
    fn default() -> Self {
        Self {
            labeled_store: LabeledExamplesStore::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dspy::sessions::DecisionRecord;

    #[test]
    fn test_complexity_correctness_success() {
        let feedback = OutcomeFeedback::default();

        // Low complexity, succeeded in 1 iteration = correct
        let decision = DecisionRecord::new(
            "complexity",
            "task: fix typo",
            serde_json::json!({"complexity": "Low", "confidence": 0.9}),
            0.9,
        );
        let outcome = SessionOutcome::Success {
            summary: "Fixed typo".into(),
            modified_files: vec!["file.rs".into()],
            verification_passed: true,
        };
        assert!(feedback.is_complexity_correct(&decision, &outcome, 1));

        // Low complexity, took 5 iterations = incorrect
        assert!(!feedback.is_complexity_correct(&decision, &outcome, 5));
    }

    #[test]
    fn test_delegation_correctness() {
        let feedback = OutcomeFeedback::default();

        // Didn't delegate, succeeded = correct
        let decision = DecisionRecord::new(
            "delegation",
            "task: simple fix",
            serde_json::json!({"should_delegate": false, "target": "local_tools"}),
            0.85,
        );
        let outcome = SessionOutcome::Success {
            summary: "Fixed".into(),
            modified_files: vec![],
            verification_passed: true,
        };
        assert!(feedback.is_delegation_correct(&decision, &outcome));

        // Didn't delegate, failed = incorrect (should have delegated)
        let failed_outcome = SessionOutcome::Failed {
            reason: "Too complex".into(),
            error: None,
        };
        assert!(!feedback.is_delegation_correct(&decision, &failed_outcome));
    }

    #[test]
    fn test_labeled_store_accuracy() {
        let mut store = LabeledExamplesStore::default();

        // Add some examples
        store.add(LabeledExample {
            example_type: "complexity".into(),
            input_summary: "test".into(),
            output: serde_json::json!({}),
            predicted_confidence: 0.9,
            was_correct: true,
            session_id: "s1".into(),
            outcome_summary: "success".into(),
            iterations_used: 1,
            labeled_at: Utc::now(),
        });
        store.add(LabeledExample {
            example_type: "complexity".into(),
            input_summary: "test2".into(),
            output: serde_json::json!({}),
            predicted_confidence: 0.8,
            was_correct: false,
            session_id: "s2".into(),
            outcome_summary: "failed".into(),
            iterations_used: 10,
            labeled_at: Utc::now(),
        });

        assert_eq!(store.accuracy("complexity"), Some(0.5));
        assert_eq!(store.count_by_type("complexity"), 2);
    }
}
