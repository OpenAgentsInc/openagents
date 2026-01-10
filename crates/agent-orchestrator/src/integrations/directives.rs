use crate::error::{Error, Result};
use dsrs::{example, Predict, Prediction, Predictor, Signature, GLOBAL_SETTINGS};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

// ============================================================================
// DSPy Signatures
// ============================================================================

#[Signature]
struct DirectiveStatusParser {
    /// Parse directive status from text.

    /// Directive content (including frontmatter if available)
    #[input]
    directive_text: String,

    /// Status: Active/Pending/Complete/Blocked
    #[output]
    status: String,

    /// Confidence in the classification
    #[output]
    confidence: f32,
}

#[Signature]
struct DirectivePriorityClassifier {
    /// Classify directive priority from context.

    /// Directive content
    #[input]
    directive_text: String,

    /// Additional context (frontmatter, title, metadata)
    #[input]
    context: String,

    /// Priority: Critical/High/Medium/Low
    #[output]
    priority: String,

    /// Rationale for the priority
    #[output]
    reasoning: String,
}

#[Signature]
struct DirectiveMatchingSignature {
    /// Semantic matching between directive and a query.

    /// Directive title or summary
    #[input]
    directive_text: String,

    /// Query or keyword list
    #[input]
    query: String,

    /// Whether this directive matches the query
    #[output]
    matches: bool,

    /// Confidence in the match
    #[output]
    confidence: f32,

    /// Matching rationale
    #[output]
    reasoning: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DirectiveStatus {
    #[default]
    Active,
    Completed,
    Blocked,
    Paused,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DirectivePriority {
    Critical,
    High,
    #[default]
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectiveSummary {
    pub id: String,
    pub title: String,
    pub status: DirectiveStatus,
    pub priority: DirectivePriority,
    pub created: Option<String>,
    pub updated: Option<String>,
    pub file_path: PathBuf,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DirectiveInjectionConfig {
    pub include_active: bool,
    pub include_related: bool,
    pub max_directives: usize,
    pub priority_filter: Option<DirectivePriority>,
}

impl DirectiveInjectionConfig {
    pub fn new() -> Self {
        Self {
            include_active: true,
            include_related: true,
            max_directives: 5,
            priority_filter: None,
        }
    }

    pub fn with_max(mut self, max: usize) -> Self {
        self.max_directives = max;
        self
    }

    pub fn active_only(mut self) -> Self {
        self.include_related = false;
        self
    }

    pub fn with_priority(mut self, priority: DirectivePriority) -> Self {
        self.priority_filter = Some(priority);
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct DirectiveContext {
    pub active_directives: Vec<DirectiveSummary>,
    pub current_directive: Option<String>,
    directive_map: HashMap<String, DirectiveSummary>,
}

impl DirectiveContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn load(workspace: &Path) -> Result<Self> {
        let directives_path = workspace.join(".openagents").join("directives");

        if !directives_path.exists() {
            return Ok(Self::default());
        }

        let mut context = Self::new();
        context.scan_directives(&directives_path).await?;
        Ok(context)
    }

    async fn scan_directives(&mut self, dir: &Path) -> Result<()> {
        let entries = std::fs::read_dir(dir).map_err(Error::Io)?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "md")
                && let Ok(summary) = self.parse_directive(&path).await
            {
                self.directive_map
                    .insert(summary.id.clone(), summary.clone());
                if summary.status == DirectiveStatus::Active {
                    self.active_directives.push(summary);
                }
            }
        }

        self.active_directives.sort_by(|a, b| {
            let priority_ord = priority_to_ord(&a.priority).cmp(&priority_to_ord(&b.priority));
            if priority_ord != std::cmp::Ordering::Equal {
                return priority_ord;
            }
            a.id.cmp(&b.id)
        });

        Ok(())
    }

    async fn parse_directive(&self, path: &Path) -> Result<DirectiveSummary> {
        let content = std::fs::read_to_string(path).map_err(Error::Io)?;

        let frontmatter = extract_yaml_frontmatter(&content)?;

        let id = frontmatter
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        let title = frontmatter
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        let status_fallback = frontmatter
            .get("status")
            .and_then(|v| v.as_str())
            .map(parse_status)
            .unwrap_or_default();

        let priority_fallback = frontmatter
            .get("priority")
            .and_then(|v| v.as_str())
            .map(parse_priority)
            .unwrap_or_default();

        let status = match Self::classify_status(&content).await {
            Some((status, confidence)) if confidence >= 0.6 => status,
            _ => status_fallback,
        };

        let priority_context = Self::build_priority_context(&title, &content, &frontmatter);
        let priority = Self::classify_priority(&content, &priority_context)
            .await
            .unwrap_or(priority_fallback);

        let created = frontmatter
            .get("created")
            .and_then(|v| v.as_str())
            .map(String::from);

        let updated = frontmatter
            .get("updated")
            .and_then(|v| v.as_str())
            .map(String::from);

        Ok(DirectiveSummary {
            id,
            title,
            status,
            priority,
            created,
            updated,
            file_path: path.to_path_buf(),
        })
    }

    async fn classify_status(content: &str) -> Option<(DirectiveStatus, f32)> {
        if !dspy_ready() {
            return None;
        }

        let parser = Predict::new(DirectiveStatusParser::new());
        let example = example! {
            "directive_text": "input" => content.to_string(),
        };

        let prediction = parser.forward(example).await.ok()?;
        let status_label = get_string(&prediction, "status");
        let confidence = get_f32(&prediction, "confidence");

        Some((status_from_label(&status_label), confidence))
    }

    async fn classify_priority(content: &str, context: &str) -> Option<DirectivePriority> {
        if !dspy_ready() {
            return None;
        }

        let classifier = Predict::new(DirectivePriorityClassifier::new());
        let example = example! {
            "directive_text": "input" => content.to_string(),
            "context": "input" => context.to_string(),
        };

        let prediction = classifier.forward(example).await.ok()?;
        let priority_label = get_string(&prediction, "priority");

        Some(priority_from_label(&priority_label))
    }

    fn build_priority_context(
        title: &str,
        _content: &str,
        frontmatter: &HashMap<String, serde_yaml::Value>,
    ) -> String {
        let frontmatter_text = serde_yaml::to_string(frontmatter).unwrap_or_default();
        format!("title: {}\nfrontmatter:\n{}", title, frontmatter_text)
    }

    pub fn format_for_context(&self) -> String {
        if self.active_directives.is_empty() {
            return String::new();
        }

        let mut output = String::from("## Active Directives\n\n");

        for directive in &self.active_directives {
            output.push_str(&format!(
                "- **{}**: {} (priority: {:?})\n",
                directive.id, directive.title, directive.priority
            ));
        }

        if let Some(ref current) = self.current_directive {
            output.push_str(&format!("\n**Current focus**: {}\n", current));
        }

        output
    }

    pub fn format_with_config(&self, config: &DirectiveInjectionConfig) -> String {
        let mut directives: Vec<_> = self.active_directives.iter().collect();

        if let Some(ref priority) = config.priority_filter {
            directives.retain(|d| &d.priority == priority);
        }

        if !config.include_active {
            directives.clear();
        }

        directives.truncate(config.max_directives);

        if directives.is_empty() {
            return String::new();
        }

        let mut output = String::from("## Directives\n\n");
        for directive in directives {
            output.push_str(&format!(
                "- **{}**: {} [{:?}]\n",
                directive.id, directive.title, directive.priority
            ));
        }
        output
    }

    pub fn find_related(&self, keywords: &[&str]) -> Option<&DirectiveSummary> {
        let query = keywords.join(" ");
        if let Some(found) = self.find_related_dspy(&query) {
            return Some(found);
        }

        self.active_directives.iter().find(|d| {
            let title_lower = d.title.to_lowercase();
            keywords
                .iter()
                .any(|k| title_lower.contains(&k.to_lowercase()))
        })
    }

    fn find_related_dspy(&self, query: &str) -> Option<&DirectiveSummary> {
        if !dspy_ready() {
            return None;
        }

        let matcher = Predict::new(DirectiveMatchingSignature::new());
        for directive in &self.active_directives {
            let example = example! {
                "directive_text": "input" => directive.title.clone(),
                "query": "input" => query.to_string(),
            };

            let prediction = run_prediction(matcher.forward(example))?;
            let matches = prediction_bool(&prediction, "matches")?;
            let confidence = get_f32(&prediction, "confidence");

            if matches && confidence >= 0.5 {
                return Some(directive);
            }
        }

        None
    }

    pub fn get_by_id(&self, id: &str) -> Option<&DirectiveSummary> {
        self.directive_map.get(id)
    }

    pub fn set_current(&mut self, directive_id: Option<String>) {
        self.current_directive = directive_id;
    }

    pub fn active_count(&self) -> usize {
        self.active_directives.len()
    }
}

fn dspy_ready() -> bool {
    GLOBAL_SETTINGS.read().unwrap().is_some()
}

fn run_prediction<F>(future: F) -> Option<Prediction>
where
    F: Future<Output = std::result::Result<Prediction, anyhow::Error>>,
{
    if !dspy_ready() {
        return None;
    }

    let result = if let Ok(handle) = tokio::runtime::Handle::try_current() {
        catch_unwind(AssertUnwindSafe(|| {
            tokio::task::block_in_place(|| handle.block_on(future))
        }))
    } else if let Ok(runtime) = tokio::runtime::Runtime::new() {
        catch_unwind(AssertUnwindSafe(|| runtime.block_on(future)))
    } else {
        return None;
    };

    match result {
        Ok(Ok(prediction)) => Some(prediction),
        _ => None,
    }
}

fn get_string(prediction: &Prediction, key: &str) -> String {
    let val = prediction.get(key, None);
    if let Some(s) = val.as_str() {
        s.to_string()
    } else {
        val.to_string().trim_matches('"').to_string()
    }
}

fn get_f32(prediction: &Prediction, key: &str) -> f32 {
    let val = prediction.get(key, None);
    if let Some(n) = val.as_f64() {
        n as f32
    } else if let Some(s) = val.as_str() {
        s.parse().unwrap_or(0.0)
    } else {
        0.0
    }
}

fn prediction_bool(prediction: &Prediction, key: &str) -> Option<bool> {
    let val = prediction.get(key, None);
    if let Some(b) = val.as_bool() {
        Some(b)
    } else if let Some(s) = val.as_str() {
        match s.to_lowercase().as_str() {
            "true" | "yes" | "1" => Some(true),
            "false" | "no" | "0" => Some(false),
            _ => None,
        }
    } else {
        None
    }
}

fn extract_yaml_frontmatter(content: &str) -> Result<HashMap<String, serde_yaml::Value>> {
    let lines: Vec<&str> = content.lines().collect();

    let start = lines
        .iter()
        .position(|l| l.trim() == "---")
        .ok_or_else(|| Error::Config {
            message: "Missing YAML frontmatter start".into(),
        })?;

    let end = lines
        .iter()
        .skip(start + 1)
        .position(|l| l.trim() == "---")
        .map(|p| p + start + 1)
        .ok_or_else(|| Error::Config {
            message: "Missing YAML frontmatter end".into(),
        })?;

    let yaml_content = lines[start + 1..end].join("\n");
    serde_yaml::from_str(&yaml_content).map_err(|e| Error::Config {
        message: format!("YAML parse error: {}", e),
    })
}

fn parse_status(s: &str) -> DirectiveStatus {
    status_from_label(s)
}

fn parse_priority(s: &str) -> DirectivePriority {
    priority_from_label(s)
}

fn status_from_label(s: &str) -> DirectiveStatus {
    match s.to_lowercase().as_str() {
        "active" | "pending" => DirectiveStatus::Active,
        "complete" | "completed" | "done" => DirectiveStatus::Completed,
        "blocked" => DirectiveStatus::Blocked,
        "paused" | "on-hold" => DirectiveStatus::Paused,
        _ => DirectiveStatus::Active,
    }
}

fn priority_from_label(s: &str) -> DirectivePriority {
    match s.to_lowercase().as_str() {
        "critical" | "urgent" => DirectivePriority::Critical,
        "high" => DirectivePriority::High,
        "medium" | "normal" => DirectivePriority::Medium,
        "low" => DirectivePriority::Low,
        _ => DirectivePriority::Medium,
    }
}

fn priority_to_ord(p: &DirectivePriority) -> u8 {
    match p {
        DirectivePriority::Critical => 0,
        DirectivePriority::High => 1,
        DirectivePriority::Medium => 2,
        DirectivePriority::Low => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_directive(dir: &Path, id: &str, title: &str, status: &str, priority: &str) {
        let content = format!(
            r#"---
id: {}
title: "{}"
status: {}
priority: {}
created: 2025-12-24
---

# {}

Test directive content.
"#,
            id, title, status, priority, title
        );

        let filename = format!("{}.md", id);
        std::fs::write(dir.join(filename), content).unwrap();
    }

    #[tokio::test]
    async fn test_load_empty_workspace() {
        let temp = TempDir::new().unwrap();
        let context = DirectiveContext::load(temp.path()).await.unwrap();
        assert!(context.active_directives.is_empty());
    }

    #[tokio::test]
    async fn test_load_directives() {
        let temp = TempDir::new().unwrap();
        let directives_dir = temp.path().join(".openagents").join("directives");
        std::fs::create_dir_all(&directives_dir).unwrap();

        create_test_directive(
            &directives_dir,
            "d-001",
            "First Directive",
            "active",
            "high",
        );
        create_test_directive(
            &directives_dir,
            "d-002",
            "Second Directive",
            "active",
            "medium",
        );
        create_test_directive(
            &directives_dir,
            "d-003",
            "Third Directive",
            "completed",
            "low",
        );

        let context = DirectiveContext::load(temp.path()).await.unwrap();

        assert_eq!(context.active_count(), 2);
        assert_eq!(context.active_directives[0].id, "d-001");
        assert_eq!(context.active_directives[1].id, "d-002");
    }

    #[tokio::test]
    async fn test_priority_sorting() {
        let temp = TempDir::new().unwrap();
        let directives_dir = temp.path().join(".openagents").join("directives");
        std::fs::create_dir_all(&directives_dir).unwrap();

        create_test_directive(&directives_dir, "d-001", "Low Priority", "active", "low");
        create_test_directive(&directives_dir, "d-002", "Critical", "active", "critical");
        create_test_directive(&directives_dir, "d-003", "Medium", "active", "medium");

        let context = DirectiveContext::load(temp.path()).await.unwrap();

        assert_eq!(
            context.active_directives[0].priority,
            DirectivePriority::Critical
        );
        assert_eq!(
            context.active_directives[1].priority,
            DirectivePriority::Medium
        );
        assert_eq!(
            context.active_directives[2].priority,
            DirectivePriority::Low
        );
    }

    #[test]
    fn test_format_for_context() {
        let mut context = DirectiveContext::new();
        context.active_directives.push(DirectiveSummary {
            id: "d-001".into(),
            title: "Test Directive".into(),
            status: DirectiveStatus::Active,
            priority: DirectivePriority::High,
            created: None,
            updated: None,
            file_path: PathBuf::new(),
        });

        let output = context.format_for_context();
        assert!(output.contains("d-001"));
        assert!(output.contains("Test Directive"));
        assert!(output.contains("High"));
    }

    #[test]
    fn test_format_with_config() {
        let mut context = DirectiveContext::new();
        for i in 1..=10 {
            context.active_directives.push(DirectiveSummary {
                id: format!("d-{:03}", i),
                title: format!("Directive {}", i),
                status: DirectiveStatus::Active,
                priority: DirectivePriority::Medium,
                created: None,
                updated: None,
                file_path: PathBuf::new(),
            });
        }

        let config = DirectiveInjectionConfig::new().with_max(3);
        let output = context.format_with_config(&config);

        let directive_count = output.matches("d-").count();
        assert_eq!(directive_count, 3);
    }

    #[test]
    fn test_find_related() {
        let mut context = DirectiveContext::new();
        context.active_directives.push(DirectiveSummary {
            id: "d-001".into(),
            title: "Bitcoin Payment Integration".into(),
            status: DirectiveStatus::Active,
            priority: DirectivePriority::High,
            created: None,
            updated: None,
            file_path: PathBuf::new(),
        });
        context.active_directives.push(DirectiveSummary {
            id: "d-002".into(),
            title: "Nostr Protocol".into(),
            status: DirectiveStatus::Active,
            priority: DirectivePriority::Medium,
            created: None,
            updated: None,
            file_path: PathBuf::new(),
        });

        let found = context.find_related(&["bitcoin", "lightning"]);
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "d-001");

        let not_found = context.find_related(&["database", "sql"]);
        assert!(not_found.is_none());
    }

    #[test]
    fn test_parse_status() {
        assert_eq!(parse_status("active"), DirectiveStatus::Active);
        assert_eq!(parse_status("completed"), DirectiveStatus::Completed);
        assert_eq!(parse_status("done"), DirectiveStatus::Completed);
        assert_eq!(parse_status("blocked"), DirectiveStatus::Blocked);
        assert_eq!(parse_status("paused"), DirectiveStatus::Paused);
        assert_eq!(parse_status("on-hold"), DirectiveStatus::Paused);
        assert_eq!(parse_status("unknown"), DirectiveStatus::Active);
    }

    #[test]
    fn test_parse_priority() {
        assert_eq!(parse_priority("critical"), DirectivePriority::Critical);
        assert_eq!(parse_priority("urgent"), DirectivePriority::Critical);
        assert_eq!(parse_priority("high"), DirectivePriority::High);
        assert_eq!(parse_priority("medium"), DirectivePriority::Medium);
        assert_eq!(parse_priority("normal"), DirectivePriority::Medium);
        assert_eq!(parse_priority("low"), DirectivePriority::Low);
    }

    #[test]
    fn test_set_current() {
        let mut context = DirectiveContext::new();
        assert!(context.current_directive.is_none());

        context.set_current(Some("d-001".into()));
        assert_eq!(context.current_directive, Some("d-001".into()));

        context.set_current(None);
        assert!(context.current_directive.is_none());
    }

    #[test]
    fn test_injection_config_builder() {
        let config = DirectiveInjectionConfig::new()
            .with_max(10)
            .active_only()
            .with_priority(DirectivePriority::High);

        assert_eq!(config.max_directives, 10);
        assert!(!config.include_related);
        assert_eq!(config.priority_filter, Some(DirectivePriority::High));
    }
}
