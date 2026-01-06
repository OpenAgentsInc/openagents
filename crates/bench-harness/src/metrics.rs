//! Metrics for evaluating benchmark results.

use std::collections::HashSet;

use crate::task::GroundTruth;

/// Result of a metric evaluation.
#[derive(Debug, Clone)]
pub struct MetricValue {
    /// Name of the metric.
    pub name: String,
    /// Score (typically 0.0-1.0).
    pub score: f64,
    /// Additional details.
    pub details: Option<String>,
}

impl MetricValue {
    /// Create a new metric value.
    pub fn new(name: impl Into<String>, score: f64) -> Self {
        Self {
            name: name.into(),
            score,
            details: None,
        }
    }

    /// Add details to the metric value.
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }
}

/// Trait for evaluation metrics.
pub trait Metric: Send + Sync {
    /// Name of the metric.
    fn name(&self) -> &str;

    /// Compute the metric score for a prediction against ground truth.
    fn compute(&self, prediction: &str, truth: &GroundTruth) -> MetricValue;
}

/// Exact match metric (case-insensitive, trimmed).
pub struct ExactMatchMetric;

impl Metric for ExactMatchMetric {
    fn name(&self) -> &str {
        "exact_match"
    }

    fn compute(&self, prediction: &str, truth: &GroundTruth) -> MetricValue {
        let pred_normalized = normalize_text(prediction);

        let score = match truth {
            GroundTruth::ExactMatch(expected) => {
                let exp_normalized = normalize_text(expected);
                // Check for exact match OR if the expected answer is contained in prediction
                // This handles cases where the LLM returns "The answer is: XYZ" when we expect "XYZ"
                if pred_normalized == exp_normalized || pred_normalized.contains(&exp_normalized) {
                    1.0
                } else {
                    0.0
                }
            }
            GroundTruth::MultipleChoice { answer, .. } => {
                // Extract answer letter from prediction
                let pred_char = extract_answer_letter(&pred_normalized);
                if pred_char == Some(*answer) {
                    1.0
                } else {
                    0.0
                }
            }
            _ => 0.0, // Not applicable for other types
        };

        MetricValue::new(self.name(), score)
    }
}

/// Multiple choice accuracy metric.
pub struct MultipleChoiceAccuracy;

impl Metric for MultipleChoiceAccuracy {
    fn name(&self) -> &str {
        "multiple_choice_accuracy"
    }

    fn compute(&self, prediction: &str, truth: &GroundTruth) -> MetricValue {
        let score = match truth {
            GroundTruth::MultipleChoice { answer, choices } => {
                let pred_normalized = normalize_text(prediction);

                // First try to extract letter directly
                if let Some(pred_char) = extract_answer_letter(&pred_normalized) {
                    if pred_char == *answer {
                        return MetricValue::new(self.name(), 1.0);
                    }
                }

                // If no letter found, try to match the answer content to choices
                if let Some(matched) = match_answer_to_choices(prediction, choices) {
                    if matched == *answer {
                        return MetricValue::new(self.name(), 1.0);
                    }
                }

                0.0
            }
            _ => 0.0,
        };

        MetricValue::new(self.name(), score)
    }
}

/// Numeric decay metric: exp(-|prediction - truth|).
pub struct NumericDecayMetric;

impl Metric for NumericDecayMetric {
    fn name(&self) -> &str {
        "numeric_decay"
    }

    fn compute(&self, prediction: &str, truth: &GroundTruth) -> MetricValue {
        let score = match truth {
            GroundTruth::NumericRange { value, tolerance } => {
                // Try to parse prediction as number
                if let Some(pred_value) = parse_number(prediction) {
                    let diff = (pred_value - value).abs();
                    if diff <= *tolerance {
                        1.0
                    } else {
                        // Exponential decay beyond tolerance
                        (-diff).exp()
                    }
                } else {
                    0.0
                }
            }
            _ => 0.0,
        };

        MetricValue::new(self.name(), score)
    }
}

/// F1 metric for set-based evaluation.
pub struct F1Metric;

impl Metric for F1Metric {
    fn name(&self) -> &str {
        "f1"
    }

    fn compute(&self, prediction: &str, truth: &GroundTruth) -> MetricValue {
        let score = match truth {
            GroundTruth::StringSet(expected) => {
                // Parse prediction as comma-separated or newline-separated items
                let pred_set: HashSet<String> = prediction
                    .split(|c| c == ',' || c == '\n')
                    .map(|s| normalize_text(s))
                    .filter(|s| !s.is_empty())
                    .collect();

                let expected_normalized: HashSet<String> =
                    expected.iter().map(|s| normalize_text(s)).collect();

                compute_f1(&pred_set, &expected_normalized)
            }
            _ => 0.0,
        };

        MetricValue::new(self.name(), score)
    }
}

/// Normalize text for comparison.
fn normalize_text(text: &str) -> String {
    text.trim().to_lowercase()
}

/// Extract answer letter (A, B, C, D) from a response.
fn extract_answer_letter(text: &str) -> Option<char> {
    let text = text.trim().to_uppercase();

    // Try direct single letter
    if text.len() == 1 {
        let c = text.chars().next()?;
        if matches!(c, 'A' | 'B' | 'C' | 'D') {
            return Some(c);
        }
    }

    // Try "Answer: X" pattern
    if let Some(rest) = text.strip_prefix("ANSWER:") {
        let rest = rest.trim();
        if let Some(c) = rest.chars().next() {
            if matches!(c, 'A' | 'B' | 'C' | 'D') {
                return Some(c);
            }
        }
    }

    // Try "(X)" pattern
    for pattern in ["(A)", "(B)", "(C)", "(D)"] {
        if text.contains(pattern) {
            return Some(pattern.chars().nth(1)?);
        }
    }

    // Try "X)" or "X." at start
    if text.len() >= 2 {
        let first = text.chars().next()?;
        let second = text.chars().nth(1)?;
        if matches!(first, 'A' | 'B' | 'C' | 'D') && (second == ')' || second == '.') {
            return Some(first);
        }
    }

    None
}

/// Try to match a prediction to one of the multiple choice options.
/// Returns the letter (A, B, C, D) if the prediction contains or matches one of the choices.
fn match_answer_to_choices(text: &str, choices: &[String]) -> Option<char> {
    let text_lower = text.to_lowercase();
    let letters = ['A', 'B', 'C', 'D'];

    for (i, choice) in choices.iter().enumerate() {
        if i >= 4 { break; }

        // Extract the value part of the choice (after "A. ", "B. ", etc.)
        let value = if choice.len() > 2 && choice.chars().nth(1) == Some('.') {
            choice[2..].trim().to_lowercase()
        } else {
            choice.to_lowercase()
        };

        // Check if the prediction contains this value
        if text_lower.contains(&value) {
            return Some(letters[i]);
        }

        // Also try matching numbers directly
        // e.g., if choice is "B. 1" and text says "returns 1"
        if let Some(num_str) = value.split_whitespace().last() {
            if text_lower.contains(num_str) {
                // Be more careful here - only match if it's a clear value match
                // Check if the number appears as a standalone word
                let num_pattern = format!(" {} ", num_str);
                let num_pattern_end = format!(" {}.", num_str);
                let num_pattern_end2 = format!(" {}", num_str);
                if text_lower.contains(&num_pattern)
                    || text_lower.ends_with(num_str)
                    || text_lower.contains(&num_pattern_end)
                    || text_lower.ends_with(&num_pattern_end2) {
                    return Some(letters[i]);
                }
            }
        }
    }

    None
}

/// Parse a number from text.
fn parse_number(text: &str) -> Option<f64> {
    let text = text.trim();

    // Try direct parse
    if let Ok(n) = text.parse::<f64>() {
        return Some(n);
    }

    // Try extracting number from text (e.g., "The answer is 42")
    for word in text.split_whitespace() {
        if let Ok(n) = word.parse::<f64>() {
            return Some(n);
        }
    }

    None
}

/// Compute F1 score between two sets.
fn compute_f1(predicted: &HashSet<String>, expected: &HashSet<String>) -> f64 {
    if predicted.is_empty() && expected.is_empty() {
        return 1.0;
    }
    if predicted.is_empty() || expected.is_empty() {
        return 0.0;
    }

    let true_positives = predicted.intersection(expected).count() as f64;
    let precision = true_positives / predicted.len() as f64;
    let recall = true_positives / expected.len() as f64;

    if precision + recall == 0.0 {
        0.0
    } else {
        2.0 * precision * recall / (precision + recall)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        let metric = ExactMatchMetric;

        // Exact match
        let result = metric.compute("Hello", &GroundTruth::exact("hello"));
        assert_eq!(result.score, 1.0);

        // No match
        let result = metric.compute("Hi", &GroundTruth::exact("hello"));
        assert_eq!(result.score, 0.0);

        // Whitespace normalization
        let result = metric.compute("  hello  ", &GroundTruth::exact("hello"));
        assert_eq!(result.score, 1.0);
    }

    #[test]
    fn test_multiple_choice() {
        let metric = MultipleChoiceAccuracy;
        let truth = GroundTruth::multiple_choice(
            'B',
            vec![
                "Option A".to_string(),
                "Option B".to_string(),
                "Option C".to_string(),
            ],
        );

        assert_eq!(metric.compute("B", &truth).score, 1.0);
        assert_eq!(metric.compute("b", &truth).score, 1.0);
        assert_eq!(metric.compute("Answer: B", &truth).score, 1.0);
        assert_eq!(metric.compute("(B)", &truth).score, 1.0);
        assert_eq!(metric.compute("A", &truth).score, 0.0);
    }

    #[test]
    fn test_numeric_decay() {
        let metric = NumericDecayMetric;
        let truth = GroundTruth::numeric_with_tolerance(100.0, 5.0);

        // Within tolerance
        assert_eq!(metric.compute("100", &truth).score, 1.0);
        assert_eq!(metric.compute("102", &truth).score, 1.0);
        assert_eq!(metric.compute("95", &truth).score, 1.0);

        // Outside tolerance - exponential decay
        let result = metric.compute("110", &truth);
        assert!(result.score < 1.0);
        assert!(result.score > 0.0);
    }

    #[test]
    fn test_f1() {
        let metric = F1Metric;

        // Perfect match
        let truth = GroundTruth::string_set(vec!["apple", "banana", "cherry"]);
        let result = metric.compute("apple, banana, cherry", &truth);
        assert_eq!(result.score, 1.0);

        // Partial match
        let result = metric.compute("apple, banana", &truth);
        assert!(result.score > 0.5);
        assert!(result.score < 1.0);

        // No match
        let result = metric.compute("grape, orange", &truth);
        assert_eq!(result.score, 0.0);
    }

    #[test]
    fn test_extract_answer_letter() {
        assert_eq!(extract_answer_letter("A"), Some('A'));
        assert_eq!(extract_answer_letter("b"), Some('B'));
        assert_eq!(extract_answer_letter("Answer: C"), Some('C'));
        assert_eq!(extract_answer_letter("(D)"), Some('D'));
        assert_eq!(extract_answer_letter("B)"), Some('B'));
        assert_eq!(extract_answer_letter("C."), Some('C'));
        assert_eq!(extract_answer_letter("random text"), None);
    }
}
