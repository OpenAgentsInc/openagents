//! Proxy metrics - cheap, fast, run frequently.
//!
//! These metrics are designed to quickly filter out bad outputs before
//! expensive truth metrics are run. They should complete in <10ms.

use super::{Metric, MetricScore, MetricTier};
use crate::data::example::Example;
use anyhow::Result;
use async_trait::async_trait;

/// Metric that checks if output is valid JSON format.
#[derive(Debug, Clone, Default)]
pub struct FormatMetric {
    /// Name of the metric.
    name: String,
    /// Whether to check for valid JSON.
    check_json: bool,
    /// Whether to check for non-empty output.
    check_non_empty: bool,
}

impl FormatMetric {
    /// Create a new format metric.
    pub fn new() -> Self {
        Self {
            name: "format".into(),
            check_json: false,
            check_non_empty: true,
        }
    }

    /// Enable JSON validation.
    pub fn with_json_check(mut self) -> Self {
        self.check_json = true;
        self
    }

    /// Set custom name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
}

#[async_trait]
impl Metric for FormatMetric {
    fn name(&self) -> &str {
        &self.name
    }

    fn tier(&self) -> MetricTier {
        MetricTier::Proxy
    }

    fn cost_estimate(&self) -> u64 {
        0 // Free - purely local computation
    }

    async fn evaluate(&self, _input: &Example, output: &Example) -> Result<MetricScore> {
        // Check if output has any content
        if self.check_non_empty {
            let has_content = output.data.iter().any(|(_, v)| {
                if let Some(s) = v.as_str() {
                    !s.trim().is_empty()
                } else {
                    true
                }
            });

            if !has_content {
                return Ok(MetricScore::fail("Output is empty"));
            }
        }

        // Check JSON validity if required
        if self.check_json {
            for (field, value) in output.data.iter() {
                if let Some(s) = value.as_str() {
                    // Try to parse as JSON if it looks like JSON
                    if s.trim().starts_with('{') || s.trim().starts_with('[') {
                        if serde_json::from_str::<serde_json::Value>(s).is_err() {
                            return Ok(MetricScore::fail(format!(
                                "Field '{}' contains invalid JSON",
                                field
                            )));
                        }
                    }
                }
            }
        }

        Ok(MetricScore::perfect())
    }
}

/// Metric that checks for required keywords in output.
#[derive(Debug, Clone)]
pub struct KeywordMetric {
    /// Name of the metric.
    name: String,
    /// Required keywords (all must be present for score 1.0).
    required: Vec<String>,
    /// Forbidden keywords (any present gives score 0.0).
    forbidden: Vec<String>,
    /// Field to check (None = check all fields).
    field: Option<String>,
}

impl KeywordMetric {
    /// Create a new keyword metric.
    pub fn new() -> Self {
        Self {
            name: "keywords".into(),
            required: Vec::new(),
            forbidden: Vec::new(),
            field: None,
        }
    }

    /// Add required keywords.
    pub fn require(mut self, keywords: Vec<impl Into<String>>) -> Self {
        self.required = keywords.into_iter().map(|k| k.into()).collect();
        self
    }

    /// Add forbidden keywords.
    pub fn forbid(mut self, keywords: Vec<impl Into<String>>) -> Self {
        self.forbidden = keywords.into_iter().map(|k| k.into()).collect();
        self
    }

    /// Check only a specific field.
    pub fn in_field(mut self, field: impl Into<String>) -> Self {
        self.field = Some(field.into());
        self
    }

    /// Set custom name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
}

impl Default for KeywordMetric {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Metric for KeywordMetric {
    fn name(&self) -> &str {
        &self.name
    }

    fn tier(&self) -> MetricTier {
        MetricTier::Proxy
    }

    fn cost_estimate(&self) -> u64 {
        0 // Free - purely local computation
    }

    async fn evaluate(&self, _input: &Example, output: &Example) -> Result<MetricScore> {
        // Collect text to search
        let text: String = if let Some(ref field) = self.field {
            output
                .data
                .get(field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase()
        } else {
            output
                .data
                .iter()
                .filter_map(|(_, v)| v.as_str())
                .collect::<Vec<_>>()
                .join(" ")
                .to_lowercase()
        };

        // Check for forbidden keywords first
        for kw in &self.forbidden {
            if text.contains(&kw.to_lowercase()) {
                return Ok(MetricScore::fail(format!(
                    "Contains forbidden keyword: {}",
                    kw
                )));
            }
        }

        // Check for required keywords
        if self.required.is_empty() {
            return Ok(MetricScore::perfect());
        }

        let mut found = 0;
        let mut missing = Vec::new();

        for kw in &self.required {
            if text.contains(&kw.to_lowercase()) {
                found += 1;
            } else {
                missing.push(kw.clone());
            }
        }

        let score = found as f64 / self.required.len() as f64;

        if missing.is_empty() {
            Ok(MetricScore::new(score))
        } else {
            Ok(MetricScore::new(score).with_details(format!("Missing keywords: {:?}", missing)))
        }
    }
}

/// Metric that checks output length bounds.
#[derive(Debug, Clone)]
pub struct LengthMetric {
    /// Name of the metric.
    name: String,
    /// Minimum length (characters).
    min_chars: Option<usize>,
    /// Maximum length (characters).
    max_chars: Option<usize>,
    /// Field to check (None = check total length).
    field: Option<String>,
}

impl LengthMetric {
    /// Create a new length metric.
    pub fn new() -> Self {
        Self {
            name: "length".into(),
            min_chars: None,
            max_chars: None,
            field: None,
        }
    }

    /// Set minimum length.
    pub fn min(mut self, chars: usize) -> Self {
        self.min_chars = Some(chars);
        self
    }

    /// Set maximum length.
    pub fn max(mut self, chars: usize) -> Self {
        self.max_chars = Some(chars);
        self
    }

    /// Set both min and max.
    pub fn between(mut self, min: usize, max: usize) -> Self {
        self.min_chars = Some(min);
        self.max_chars = Some(max);
        self
    }

    /// Check only a specific field.
    pub fn in_field(mut self, field: impl Into<String>) -> Self {
        self.field = Some(field.into());
        self
    }

    /// Set custom name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
}

impl Default for LengthMetric {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Metric for LengthMetric {
    fn name(&self) -> &str {
        &self.name
    }

    fn tier(&self) -> MetricTier {
        MetricTier::Proxy
    }

    fn cost_estimate(&self) -> u64 {
        0 // Free - purely local computation
    }

    async fn evaluate(&self, _input: &Example, output: &Example) -> Result<MetricScore> {
        // Calculate length
        let length: usize = if let Some(ref field) = self.field {
            output
                .data
                .get(field)
                .and_then(|v| v.as_str())
                .map(|s| s.len())
                .unwrap_or(0)
        } else {
            output
                .data
                .iter()
                .filter_map(|(_, v)| v.as_str())
                .map(|s| s.len())
                .sum()
        };

        // Check bounds
        if let Some(min) = self.min_chars {
            if length < min {
                return Ok(MetricScore::fail(format!(
                    "Output too short: {} < {} chars",
                    length, min
                )));
            }
        }

        if let Some(max) = self.max_chars {
            if length > max {
                return Ok(MetricScore::fail(format!(
                    "Output too long: {} > {} chars",
                    length, max
                )));
            }
        }

        Ok(MetricScore::perfect())
    }
}

/// Metric that checks if code output has valid syntax.
#[derive(Debug, Clone)]
pub struct SyntaxMetric {
    /// Name of the metric.
    name: String,
    /// Expected language (for syntax checking).
    language: Option<String>,
    /// Field containing code.
    field: Option<String>,
}

impl SyntaxMetric {
    /// Create a new syntax metric.
    pub fn new() -> Self {
        Self {
            name: "syntax".into(),
            language: None,
            field: None,
        }
    }

    /// Set the expected language.
    pub fn language(mut self, lang: impl Into<String>) -> Self {
        self.language = Some(lang.into());
        self
    }

    /// Check only a specific field.
    pub fn in_field(mut self, field: impl Into<String>) -> Self {
        self.field = Some(field.into());
        self
    }

    /// Set custom name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Check basic syntax validity (balanced brackets, quotes).
    fn check_basic_syntax(code: &str) -> Result<MetricScore> {
        let mut stack: Vec<char> = Vec::new();
        let mut in_string = false;
        let mut string_char = '"';
        let mut prev_char = ' ';

        for ch in code.chars() {
            if in_string {
                if ch == string_char && prev_char != '\\' {
                    in_string = false;
                }
            } else {
                match ch {
                    '"' | '\'' | '`' => {
                        in_string = true;
                        string_char = ch;
                    }
                    '(' | '[' | '{' => stack.push(ch),
                    ')' => {
                        if stack.pop() != Some('(') {
                            return Ok(MetricScore::fail("Unbalanced parentheses"));
                        }
                    }
                    ']' => {
                        if stack.pop() != Some('[') {
                            return Ok(MetricScore::fail("Unbalanced brackets"));
                        }
                    }
                    '}' => {
                        if stack.pop() != Some('{') {
                            return Ok(MetricScore::fail("Unbalanced braces"));
                        }
                    }
                    _ => {}
                }
            }
            prev_char = ch;
        }

        if in_string {
            return Ok(MetricScore::fail("Unclosed string"));
        }

        if !stack.is_empty() {
            return Ok(MetricScore::fail(format!("Unclosed brackets: {:?}", stack)));
        }

        Ok(MetricScore::perfect())
    }
}

impl Default for SyntaxMetric {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Metric for SyntaxMetric {
    fn name(&self) -> &str {
        &self.name
    }

    fn tier(&self) -> MetricTier {
        MetricTier::Proxy
    }

    fn cost_estimate(&self) -> u64 {
        0 // Free - purely local computation
    }

    async fn evaluate(&self, _input: &Example, output: &Example) -> Result<MetricScore> {
        // Get code to check
        let code: String = if let Some(ref field) = self.field {
            output
                .data
                .get(field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        } else {
            // Look for common code field names
            for field_name in &["code", "output", "result", "content"] {
                if let Some(v) = output.data.get(*field_name) {
                    if let Some(s) = v.as_str() {
                        return Self::check_basic_syntax(s);
                    }
                }
            }
            // Fall back to first string field
            output
                .data
                .iter()
                .find_map(|(_, v)| v.as_str().map(|s| s.to_string()))
                .unwrap_or_default()
        };

        if code.is_empty() {
            return Ok(MetricScore::fail("No code found"));
        }

        Self::check_basic_syntax(&code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_example(fields: &[(&str, &str)]) -> Example {
        let mut ex = Example::default();
        for (k, v) in fields {
            ex.data
                .insert(k.to_string(), serde_json::Value::String(v.to_string()));
        }
        ex
    }

    #[tokio::test]
    async fn test_format_metric_empty() {
        let metric = FormatMetric::new();
        let input = Example::default();
        let output = Example::default();

        let score = metric.evaluate(&input, &output).await.unwrap();
        assert_eq!(score.value, 0.0);
    }

    #[tokio::test]
    async fn test_format_metric_non_empty() {
        let metric = FormatMetric::new();
        let input = Example::default();
        let output = make_example(&[("answer", "Hello world")]);

        let score = metric.evaluate(&input, &output).await.unwrap();
        assert_eq!(score.value, 1.0);
    }

    #[tokio::test]
    async fn test_format_metric_json() {
        let metric = FormatMetric::new().with_json_check();
        let input = Example::default();

        // Valid JSON
        let output = make_example(&[("data", "{\"key\": \"value\"}")]);
        let score = metric.evaluate(&input, &output).await.unwrap();
        assert_eq!(score.value, 1.0);

        // Invalid JSON
        let output = make_example(&[("data", "{invalid json}")]);
        let score = metric.evaluate(&input, &output).await.unwrap();
        assert_eq!(score.value, 0.0);
    }

    #[tokio::test]
    async fn test_keyword_metric_required() {
        let metric = KeywordMetric::new().require(vec!["function", "return"]);
        let input = Example::default();

        // All keywords present
        let output = make_example(&[("code", "function foo() { return 42; }")]);
        let score = metric.evaluate(&input, &output).await.unwrap();
        assert_eq!(score.value, 1.0);

        // Only one keyword
        let output = make_example(&[("code", "function foo() { }")]);
        let score = metric.evaluate(&input, &output).await.unwrap();
        assert_eq!(score.value, 0.5);
    }

    #[tokio::test]
    async fn test_keyword_metric_forbidden() {
        let metric = KeywordMetric::new().forbid(vec!["TODO", "FIXME"]);
        let input = Example::default();

        // No forbidden keywords
        let output = make_example(&[("code", "function complete() { return 42; }")]);
        let score = metric.evaluate(&input, &output).await.unwrap();
        assert_eq!(score.value, 1.0);

        // Has forbidden keyword
        let output = make_example(&[("code", "function incomplete() { // TODO: fix this }")]);
        let score = metric.evaluate(&input, &output).await.unwrap();
        assert_eq!(score.value, 0.0);
    }

    #[tokio::test]
    async fn test_length_metric() {
        let metric = LengthMetric::new().between(10, 100);
        let input = Example::default();

        // Within bounds
        let output = make_example(&[("text", "This is exactly the right length of text.")]);
        let score = metric.evaluate(&input, &output).await.unwrap();
        assert_eq!(score.value, 1.0);

        // Too short
        let output = make_example(&[("text", "Short")]);
        let score = metric.evaluate(&input, &output).await.unwrap();
        assert_eq!(score.value, 0.0);
    }

    #[tokio::test]
    async fn test_syntax_metric() {
        let metric = SyntaxMetric::new().in_field("code");
        let input = Example::default();

        // Valid syntax
        let output = make_example(&[("code", "function foo() { return { a: 1 }; }")]);
        let score = metric.evaluate(&input, &output).await.unwrap();
        assert_eq!(score.value, 1.0);

        // Unbalanced braces
        let output = make_example(&[("code", "function foo() { return { a: 1 }; ")]);
        let score = metric.evaluate(&input, &output).await.unwrap();
        assert_eq!(score.value, 0.0);

        // Unclosed string
        let output = make_example(&[("code", "let x = \"unclosed string")]);
        let score = metric.evaluate(&input, &output).await.unwrap();
        assert_eq!(score.value, 0.0);
    }
}
