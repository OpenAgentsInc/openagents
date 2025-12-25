//! Context loss analysis for autopilot compaction
//!
//! Analyzes trajectory logs to detect what context is lost during summarization
//! and which lost context most impacts task completion.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Types of context that can be lost during compaction
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContextType {
    /// File paths and locations mentioned
    FilePaths,
    /// Function/struct/type names
    SymbolNames,
    /// Error messages and failure reasons
    ErrorDetails,
    /// Directory structure and navigation
    WorkingDirectory,
    /// Dependency names and versions
    Dependencies,
    /// Test results and validation outcomes
    TestResults,
    /// TODO items and pending tasks
    TodoItems,
    /// Git branch and commit information
    GitContext,
    /// Issue numbers and directive IDs
    IssueContext,
    /// Architecture patterns and design decisions
    ArchitectureDecisions,
    /// Constraints and requirements
    Constraints,
}

impl ContextType {
    /// Get a human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            ContextType::FilePaths => "File paths and locations",
            ContextType::SymbolNames => "Function, struct, and type names",
            ContextType::ErrorDetails => "Error messages and failure reasons",
            ContextType::WorkingDirectory => "Directory structure and navigation",
            ContextType::Dependencies => "Dependency names and versions",
            ContextType::TestResults => "Test results and validation outcomes",
            ContextType::TodoItems => "TODO items and pending tasks",
            ContextType::GitContext => "Git branch and commit information",
            ContextType::IssueContext => "Issue numbers and directive IDs",
            ContextType::ArchitectureDecisions => "Architecture patterns and design decisions",
            ContextType::Constraints => "Constraints and requirements",
        }
    }
}

/// Evidence of context loss
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextLossInstance {
    /// Type of context that was lost
    pub context_type: ContextType,
    /// Session ID where loss occurred
    pub session_id: String,
    /// Specific evidence (e.g., "file path mentioned before compaction but not after")
    pub evidence: String,
    /// Impact on task completion (1-10, 10 being severe)
    pub impact_severity: u8,
    /// Whether this led to a failure or error
    pub caused_failure: bool,
}

/// Results of context loss analysis
#[derive(Debug, Serialize, Deserialize)]
pub struct ContextLossReport {
    /// All detected instances of context loss
    pub instances: Vec<ContextLossInstance>,
    /// Frequency of each context type being lost
    pub frequency_by_type: HashMap<ContextType, usize>,
    /// Average impact severity by context type
    pub avg_impact_by_type: HashMap<ContextType, f64>,
    /// Most critical context types (high frequency + high impact)
    pub critical_types: Vec<ContextType>,
}

/// Analyzer for context loss in trajectories
pub struct ContextLossAnalyzer;

impl ContextLossAnalyzer {
    /// Analyze a session before and after compaction
    pub fn analyze_session(
        session_id: &str,
        before_content: &str,
        after_content: &str,
    ) -> Result<Vec<ContextLossInstance>> {
        let mut instances = Vec::new();

        // Analyze file paths
        let paths_before = Self::extract_file_paths(before_content);
        let paths_after = Self::extract_file_paths(after_content);
        let lost_paths: Vec<_> = paths_before
            .iter()
            .filter(|p| !paths_after.contains(p))
            .collect();

        if !lost_paths.is_empty() {
            instances.push(ContextLossInstance {
                context_type: ContextType::FilePaths,
                session_id: session_id.to_string(),
                evidence: format!("{} file paths lost: {:?}", lost_paths.len(), lost_paths),
                impact_severity: if lost_paths.len() > 5 { 8 } else { 6 },
                caused_failure: false, // Cannot determine without further analysis
            });
        }

        // Analyze error messages
        let errors_before = Self::extract_error_messages(before_content);
        let errors_after = Self::extract_error_messages(after_content);
        let lost_errors: Vec<_> = errors_before
            .iter()
            .filter(|e| !errors_after.contains(e))
            .collect();

        if !lost_errors.is_empty() {
            instances.push(ContextLossInstance {
                context_type: ContextType::ErrorDetails,
                session_id: session_id.to_string(),
                evidence: format!("{} error messages lost", lost_errors.len()),
                impact_severity: 9, // Error details are critical
                caused_failure: false,
            });
        }

        // Analyze issue/directive references
        let refs_before = Self::extract_issue_refs(before_content);
        let refs_after = Self::extract_issue_refs(after_content);
        let lost_refs: Vec<_> = refs_before
            .iter()
            .filter(|r| !refs_after.contains(r))
            .collect();

        if !lost_refs.is_empty() {
            instances.push(ContextLossInstance {
                context_type: ContextType::IssueContext,
                session_id: session_id.to_string(),
                evidence: format!(
                    "{} issue/directive refs lost: {:?}",
                    lost_refs.len(),
                    lost_refs
                ),
                impact_severity: 7,
                caused_failure: false,
            });
        }

        // Analyze test result mentions
        let test_keywords = [
            "test passed",
            "test failed",
            "tests passed",
            "tests failed",
            "cargo test",
        ];
        let tests_before = test_keywords
            .iter()
            .filter(|&&kw| before_content.contains(kw))
            .count();
        let tests_after = test_keywords
            .iter()
            .filter(|&&kw| after_content.contains(kw))
            .count();

        if tests_before > 0 && tests_after == 0 {
            instances.push(ContextLossInstance {
                context_type: ContextType::TestResults,
                session_id: session_id.to_string(),
                evidence: "Test results mentioned before but not after compaction".to_string(),
                impact_severity: 7,
                caused_failure: false,
            });
        }

        // Analyze git context
        let git_keywords = ["git checkout", "git commit", "git push", "branch"];
        let git_before = git_keywords
            .iter()
            .filter(|&&kw| before_content.contains(kw))
            .count();
        let git_after = git_keywords
            .iter()
            .filter(|&&kw| after_content.contains(kw))
            .count();

        if git_before > 0 && git_after == 0 {
            instances.push(ContextLossInstance {
                context_type: ContextType::GitContext,
                session_id: session_id.to_string(),
                evidence: "Git operations mentioned before but lost after compaction".to_string(),
                impact_severity: 5,
                caused_failure: false,
            });
        }

        Ok(instances)
    }

    /// Detect file paths in content
    fn extract_file_paths(content: &str) -> Vec<String> {
        // Simple regex for common file paths
        let mut paths = Vec::new();
        for line in content.lines() {
            if line.contains(".rs")
                || line.contains(".toml")
                || line.contains(".md")
                || line.contains(".json")
            {
                // Extract path-like strings
                for word in line.split_whitespace() {
                    if word.contains('/') && (word.contains(".rs") || word.contains(".toml")) {
                        paths.push(
                            word.trim_matches(|c: char| {
                                !c.is_alphanumeric() && c != '/' && c != '.' && c != '_' && c != '-'
                            })
                            .to_string(),
                        );
                    }
                }
            }
        }
        paths
    }

    /// Detect error messages in content
    fn extract_error_messages(content: &str) -> Vec<String> {
        let mut errors = Vec::new();
        for line in content.lines() {
            if line.to_lowercase().contains("error:")
                || line.contains("Error:")
                || line.contains("failed")
                || line.contains("Failed")
            {
                errors.push(line.trim().to_string());
            }
        }
        errors
    }

    /// Detect issue/directive references
    fn extract_issue_refs(content: &str) -> Vec<String> {
        let mut refs = Vec::new();
        for word in content.split_whitespace() {
            if word.starts_with("#") && word.len() > 1 {
                refs.push(word.to_string());
            }
            if word.starts_with("d-") && word.len() > 2 {
                refs.push(word.to_string());
            }
        }
        refs
    }

    /// Generate report from instances
    pub fn generate_report(instances: Vec<ContextLossInstance>) -> ContextLossReport {
        let mut frequency_by_type: HashMap<ContextType, usize> = HashMap::new();
        let mut impact_sum_by_type: HashMap<ContextType, u64> = HashMap::new();

        for instance in &instances {
            *frequency_by_type
                .entry(instance.context_type.clone())
                .or_insert(0) += 1;
            *impact_sum_by_type
                .entry(instance.context_type.clone())
                .or_insert(0) += instance.impact_severity as u64;
        }

        let avg_impact_by_type: HashMap<ContextType, f64> = frequency_by_type
            .iter()
            .map(|(ctx_type, freq)| {
                let sum = *impact_sum_by_type.get(ctx_type).unwrap_or(&0);
                (ctx_type.clone(), sum as f64 / *freq as f64)
            })
            .collect();

        // Identify critical types: frequency >= 3 AND avg_impact >= 6
        let critical_types: Vec<ContextType> = frequency_by_type
            .iter()
            .filter(|(ctx_type, freq)| {
                **freq >= 3
                    && avg_impact_by_type
                        .get(*ctx_type)
                        .map(|&avg| avg >= 6.0)
                        .unwrap_or(false)
            })
            .map(|(ctx_type, _)| ctx_type.clone())
            .collect();

        ContextLossReport {
            instances,
            frequency_by_type,
            avg_impact_by_type,
            critical_types,
        }
    }
}

/// Generate improved compaction instructions based on analysis
pub fn generate_improved_compaction_instructions(report: &ContextLossReport) -> String {
    let mut instructions =
        String::from("Create a handoff-ready summary for autonomous continuation:\n\n");

    instructions.push_str("## Core Requirements\n\n");
    instructions.push_str("1. Tasks completed (mark clearly as DONE)\n");
    instructions.push_str("2. Current task in progress (if any, with specific next steps)\n");
    instructions.push_str("3. Pending tasks from the backlog\n");
    instructions.push_str("4. Any blockers or decisions needed\n\n");

    if !report.critical_types.is_empty() {
        instructions.push_str("## Critical Context (MUST PRESERVE)\n\n");
        instructions.push_str("Based on analysis, the following context is frequently lost and severely impacts task completion:\n\n");

        for ctx_type in &report.critical_types {
            let freq = report.frequency_by_type.get(ctx_type).unwrap_or(&0);
            let impact = report.avg_impact_by_type.get(ctx_type).unwrap_or(&0.0);

            instructions.push_str(&format!(
                "- **{}**: Lost in {} sessions with avg impact {:.1}/10\n",
                ctx_type.description(),
                freq,
                impact
            ));

            match ctx_type {
                ContextType::FilePaths => {
                    instructions.push_str("  → ALWAYS include specific file paths with line numbers (e.g., `src/main.rs:142`)\n");
                }
                ContextType::SymbolNames => {
                    instructions.push_str("  → ALWAYS preserve exact function/struct/type names\n");
                }
                ContextType::ErrorDetails => {
                    instructions
                        .push_str("  → ALWAYS include full error messages and stack traces\n");
                }
                ContextType::IssueContext => {
                    instructions
                        .push_str("  → ALWAYS mention active issue numbers and directive IDs\n");
                }
                ContextType::TestResults => {
                    instructions.push_str("  → ALWAYS state which tests passed/failed and why\n");
                }
                ContextType::ArchitectureDecisions => {
                    instructions.push_str(
                        "  → ALWAYS explain why certain approaches were chosen over alternatives\n",
                    );
                }
                _ => {}
            }
        }
        instructions.push_str("\n");
    }

    instructions.push_str("## Key Context Details\n\n");
    instructions.push_str("- Active files and their purpose (with full paths)\n");
    instructions.push_str("- Established patterns to follow\n");
    instructions.push_str("- Constraints or requirements\n");
    instructions.push_str("- Recent error messages (if any)\n\n");

    instructions.push_str(
        "Format as a clear action plan that another agent can immediately pick up and continue.",
    );

    instructions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_file_paths() {
        let content = "Modified src/main.rs and crates/foo/Cargo.toml";
        let paths = ContextLossAnalyzer::extract_file_paths(content);
        assert!(paths.iter().any(|p| p.contains("main.rs")));
        assert!(paths.iter().any(|p| p.contains("Cargo.toml")));
    }

    #[test]
    fn test_extract_error_messages() {
        let content = "Error: file not found\nSuccess\nFailed to compile";
        let errors = ContextLossAnalyzer::extract_error_messages(content);
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn test_extract_issue_refs() {
        let content = "Working on #1014 for d-004";
        let refs = ContextLossAnalyzer::extract_issue_refs(content);
        assert!(refs.contains(&"#1014".to_string()));
        assert!(refs.contains(&"d-004".to_string()));
    }

    #[test]
    fn test_generate_report() {
        let instances = vec![
            ContextLossInstance {
                context_type: ContextType::FilePaths,
                session_id: "test-1".to_string(),
                evidence: "Path lost".to_string(),
                impact_severity: 8,
                caused_failure: true,
            },
            ContextLossInstance {
                context_type: ContextType::FilePaths,
                session_id: "test-2".to_string(),
                evidence: "Another path lost".to_string(),
                impact_severity: 6,
                caused_failure: false,
            },
            ContextLossInstance {
                context_type: ContextType::ErrorDetails,
                session_id: "test-3".to_string(),
                evidence: "Error message lost".to_string(),
                impact_severity: 9,
                caused_failure: true,
            },
        ];

        let report = ContextLossAnalyzer::generate_report(instances);
        assert_eq!(report.instances.len(), 3);
        assert_eq!(
            *report
                .frequency_by_type
                .get(&ContextType::FilePaths)
                .unwrap(),
            2
        );
        assert_eq!(
            *report
                .frequency_by_type
                .get(&ContextType::ErrorDetails)
                .unwrap(),
            1
        );
    }

    #[test]
    fn test_generate_improved_instructions() {
        let instances = vec![
            ContextLossInstance {
                context_type: ContextType::FilePaths,
                session_id: "test-1".to_string(),
                evidence: "Path lost".to_string(),
                impact_severity: 8,
                caused_failure: true,
            },
            ContextLossInstance {
                context_type: ContextType::FilePaths,
                session_id: "test-2".to_string(),
                evidence: "Path lost".to_string(),
                impact_severity: 7,
                caused_failure: true,
            },
            ContextLossInstance {
                context_type: ContextType::FilePaths,
                session_id: "test-3".to_string(),
                evidence: "Path lost".to_string(),
                impact_severity: 9,
                caused_failure: true,
            },
        ];

        let report = ContextLossAnalyzer::generate_report(instances);
        let instructions = generate_improved_compaction_instructions(&report);

        assert!(instructions.contains("Critical Context"));
        assert!(instructions.contains("File paths"));
        assert!(instructions.contains("ALWAYS include specific file paths"));
    }
}
