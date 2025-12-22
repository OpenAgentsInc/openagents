//! Self-improvement learning system for autopilot
//!
//! This module implements Phase 5 (Learning Application) from d-004. It analyzes
//! autopilot trajectories and metrics to:
//! - Identify instruction adherence failures
//! - Detect patterns in tool errors
//! - Generate CLAUDE.md updates
//! - Propose hook improvements
//! - Create issues for detected problems

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::metrics::{MetricsDb, SessionMetrics};

/// Type of improvement detected
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImprovementType {
    /// Instruction adherence failure (agent ignored instructions)
    InstructionFailure,
    /// Repeated tool error pattern
    ToolErrorPattern,
    /// Safety violation detected
    SafetyViolation,
    /// Context loss after compaction
    ContextLoss,
    /// Inefficient tool usage
    Inefficiency,
}

/// A detected improvement opportunity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Improvement {
    /// Type of improvement
    pub improvement_type: ImprovementType,
    /// Human-readable description
    pub description: String,
    /// Evidence (session IDs, tool calls, etc.)
    pub evidence: Vec<String>,
    /// Proposed fix
    pub proposed_fix: String,
    /// Severity (1-10, 10 being critical)
    pub severity: u8,
    /// Whether this should create an issue
    pub create_issue: bool,
}

/// Proposed change to CLAUDE.md or other prompt files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptChange {
    /// File to modify
    pub file_path: String,
    /// Section to modify
    pub section: String,
    /// Current text (if replacing)
    pub current_text: Option<String>,
    /// New text to add or replace with
    pub new_text: String,
    /// Rationale for the change
    pub rationale: String,
}

/// Proposed change to hooks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookChange {
    /// Hook name (e.g., "user-prompt-submit-hook")
    pub hook_name: String,
    /// Current hook command (if replacing)
    pub current_command: Option<String>,
    /// New hook command
    pub new_command: String,
    /// Rationale for the change
    pub rationale: String,
}

/// Analyzer for instruction adherence failures
pub struct InstructionAnalyzer<'a> {
    db: &'a MetricsDb,
}

impl<'a> InstructionAnalyzer<'a> {
    pub fn new(db: &'a MetricsDb) -> Self {
        Self { db }
    }

    /// Analyze instruction failures across multiple sessions
    pub fn analyze(&self, session_ids: &[String]) -> Result<Vec<Improvement>> {
        let mut improvements = Vec::new();

        // Get all sessions
        let sessions: Vec<_> = session_ids
            .iter()
            .filter_map(|id| self.db.get_session(id).ok().flatten())
            .collect();

        // Pattern 1: High tool error rate suggests ignoring instructions
        let high_error_sessions = self.find_high_error_rate_sessions(&sessions);
        if !high_error_sessions.is_empty() {
            improvements.push(Improvement {
                improvement_type: ImprovementType::InstructionFailure,
                description: format!(
                    "{} sessions with >20% tool error rate - likely ignoring read-before-edit",
                    high_error_sessions.len()
                ),
                evidence: high_error_sessions.iter().map(|s| s.id.clone()).collect(),
                proposed_fix: "Add stronger emphasis to read-before-edit instruction in CLAUDE.md".to_string(),
                severity: 7,
                create_issue: true,
            });
        }

        // Pattern 2: Detect unsafe operations (from tool call analysis)
        let unsafe_ops = self.detect_unsafe_operations(session_ids)?;
        if !unsafe_ops.is_empty() {
            improvements.push(Improvement {
                improvement_type: ImprovementType::SafetyViolation,
                description: format!(
                    "{} unsafe operations detected (sqlite3 writes, git force push)",
                    unsafe_ops.len()
                ),
                evidence: unsafe_ops,
                proposed_fix: "Add pre-submit hook to block unsafe operations".to_string(),
                severity: 10,
                create_issue: true,
            });
        }

        // Pattern 3: Repeated failures on same tool
        let tool_failures = self.analyze_tool_failure_patterns(session_ids)?;
        for (tool, count) in tool_failures {
            if count > 5 {
                improvements.push(Improvement {
                    improvement_type: ImprovementType::ToolErrorPattern,
                    description: format!("{} tool failed {} times across sessions", tool, count),
                    evidence: vec![format!("{} failures", count)],
                    proposed_fix: format!(
                        "Add specific guidance for {} tool usage in CLAUDE.md",
                        tool
                    ),
                    severity: 5,
                    create_issue: true,
                });
            }
        }

        Ok(improvements)
    }

    fn find_high_error_rate_sessions<'b>(&self, sessions: &'b [SessionMetrics]) -> Vec<&'b SessionMetrics> {
        sessions
            .iter()
            .filter(|s| {
                if s.tool_calls == 0 {
                    return false;
                }
                let error_rate = s.tool_errors as f64 / s.tool_calls as f64;
                error_rate > 0.20
            })
            .collect()
    }

    fn detect_unsafe_operations(&self, session_ids: &[String]) -> Result<Vec<String>> {
        let mut unsafe_ops = Vec::new();

        for session_id in session_ids {
            let tool_calls = self.db.get_tool_calls(session_id)?;

            for call in tool_calls {
                // Check for unsafe Bash commands
                if call.tool_name == "Bash" && !call.success {
                    if let Some(ref error) = call.error_type {
                        // Look for patterns that suggest unsafe operations
                        if error.contains("sqlite3") && error.contains("INSERT") {
                            unsafe_ops.push(format!(
                                "Session {} attempted direct sqlite3 write",
                                session_id
                            ));
                        }
                        if error.contains("git") && error.contains("force") {
                            unsafe_ops.push(format!(
                                "Session {} attempted git force operation",
                                session_id
                            ));
                        }
                    }
                }
            }
        }

        Ok(unsafe_ops)
    }

    fn analyze_tool_failure_patterns(&self, session_ids: &[String]) -> Result<HashMap<String, usize>> {
        let mut tool_failures: HashMap<String, usize> = HashMap::new();

        for session_id in session_ids {
            let tool_calls = self.db.get_tool_calls(session_id)?;

            for call in tool_calls {
                if !call.success {
                    *tool_failures.entry(call.tool_name.clone()).or_insert(0) += 1;
                }
            }
        }

        Ok(tool_failures)
    }
}

/// Generator for CLAUDE.md updates
pub struct PromptUpdateGenerator;

impl PromptUpdateGenerator {
    /// Generate prompt updates based on detected improvements
    pub fn generate(improvements: &[Improvement]) -> Vec<PromptChange> {
        let mut changes = Vec::new();

        for improvement in improvements {
            match improvement.improvement_type {
                ImprovementType::InstructionFailure => {
                    if improvement.description.contains("read-before-edit") {
                        changes.push(PromptChange {
                            file_path: "CLAUDE.md".to_string(),
                            section: "Database Operations".to_string(),
                            current_text: None,
                            new_text: "**CRITICAL: You MUST read a file before editing it. The Edit tool will fail if you haven't read the file first.**".to_string(),
                            rationale: format!("High error rate detected: {}", improvement.description),
                        });
                    }
                }
                ImprovementType::ToolErrorPattern => {
                    changes.push(PromptChange {
                        file_path: "CLAUDE.md".to_string(),
                        section: "Tool Usage Guidelines".to_string(),
                        current_text: None,
                        new_text: format!("**{} Tool**: {}",
                            improvement.description.split_whitespace().next().unwrap_or("Unknown"),
                            improvement.proposed_fix
                        ),
                        rationale: improvement.description.clone(),
                    });
                }
                _ => {}
            }
        }

        changes
    }
}

/// Generator for hook improvements
pub struct HookUpdateGenerator;

impl HookUpdateGenerator {
    /// Generate hook updates based on safety violations
    pub fn generate(improvements: &[Improvement]) -> Vec<HookChange> {
        let mut changes = Vec::new();

        for improvement in improvements {
            if matches!(improvement.improvement_type, ImprovementType::SafetyViolation) {
                if improvement.description.contains("sqlite3") {
                    changes.push(HookChange {
                        hook_name: "user-prompt-submit-hook".to_string(),
                        current_command: None,
                        new_command: "if grep -q 'sqlite3.*INSERT\\|UPDATE\\|DELETE' <<< \"$PROMPT\"; then echo 'BLOCKED: Direct sqlite3 writes not allowed. Use the API instead.'; exit 1; fi".to_string(),
                        rationale: "Block direct sqlite3 writes to prevent bypassing counters and triggers".to_string(),
                    });
                }

                if improvement.description.contains("git force") {
                    changes.push(HookChange {
                        hook_name: "user-prompt-submit-hook".to_string(),
                        current_command: None,
                        new_command: "if grep -q 'git.*--force\\|git.*-f' <<< \"$PROMPT\"; then echo 'BLOCKED: Git force operations require explicit user approval'; exit 1; fi".to_string(),
                        rationale: "Prevent accidental force push to main branch".to_string(),
                    });
                }
            }
        }

        changes
    }
}

/// Issue creator for improvement opportunities
pub struct ImprovementIssueCreator;

impl ImprovementIssueCreator {
    /// Create an issue for an improvement
    pub fn create_issue(improvement: &Improvement) -> Result<String> {
        // In production, this would call the MCP issues tool
        // For now, return a formatted issue description

        let title = match improvement.improvement_type {
            ImprovementType::InstructionFailure => {
                format!("Fix instruction adherence: {}", improvement.description.split_whitespace().take(5).collect::<Vec<_>>().join(" "))
            }
            ImprovementType::ToolErrorPattern => {
                format!("Reduce {} errors", improvement.description.split_whitespace().next().unwrap_or("tool"))
            }
            ImprovementType::SafetyViolation => {
                "Block unsafe operations in autopilot".to_string()
            }
            ImprovementType::ContextLoss => {
                "Improve context retention after compaction".to_string()
            }
            ImprovementType::Inefficiency => {
                format!("Optimize: {}", improvement.description.split_whitespace().take(5).collect::<Vec<_>>().join(" "))
            }
        };

        let description = format!(
            "## Problem\n\n{}\n\n## Evidence\n\n{}\n\n## Proposed Fix\n\n{}\n\n## Severity\n\n{}/10\n\nDetected by autopilot learning system.",
            improvement.description,
            improvement.evidence.join("\n- "),
            improvement.proposed_fix,
            improvement.severity
        );

        Ok(format!("title: {}\n\n{}", title, description))
    }
}

/// Main learning pipeline
pub struct LearningPipeline<'a> {
    db: &'a MetricsDb,
}

impl<'a> LearningPipeline<'a> {
    pub fn new(db: &'a MetricsDb) -> Self {
        Self { db }
    }

    /// Run the full learning pipeline
    pub fn run(&self, session_ids: &[String]) -> Result<LearningReport> {
        let analyzer = InstructionAnalyzer::new(self.db);
        let improvements = analyzer.analyze(session_ids)?;

        let prompt_updates = PromptUpdateGenerator::generate(&improvements);
        let hook_updates = HookUpdateGenerator::generate(&improvements);

        let mut issues_created = Vec::new();
        for improvement in &improvements {
            if improvement.create_issue {
                let issue = ImprovementIssueCreator::create_issue(improvement)?;
                issues_created.push(issue);
            }
        }

        Ok(LearningReport {
            improvements,
            prompt_updates,
            hook_updates,
            issues_created,
        })
    }
}

/// Report of learning pipeline results
#[derive(Debug, Serialize, Deserialize)]
pub struct LearningReport {
    pub improvements: Vec<Improvement>,
    pub prompt_updates: Vec<PromptChange>,
    pub hook_updates: Vec<HookChange>,
    pub issues_created: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::{SessionStatus};
    use chrono::Utc;

    #[test]
    fn test_detect_high_error_rate() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test_learning.db");
        let db = MetricsDb::open(&db_path).unwrap();

        // Create session with high error rate
        let session = SessionMetrics {
            id: "high-error-session".to_string(),
            timestamp: Utc::now(),
            model: "sonnet".to_string(),
            prompt: "Test".to_string(),
            duration_seconds: 100.0,
            tokens_in: 1000,
            tokens_out: 500,
            tokens_cached: 200,
            cost_usd: 0.05,
            issues_claimed: 1,
            issues_completed: 0,
            tool_calls: 20,
            tool_errors: 8, // 40% error rate
            final_status: SessionStatus::Completed,
            messages: 10,
            apm: None,
            source: "autopilot".to_string(),
        };
        db.store_session(&session).unwrap();

        let analyzer = InstructionAnalyzer::new(&db);
        let improvements = analyzer.analyze(&[session.id]).unwrap();

        assert!(improvements.iter().any(|i| matches!(
            i.improvement_type,
            ImprovementType::InstructionFailure
        )));
    }

    #[test]
    fn test_generate_prompt_updates() {
        let improvement = Improvement {
            improvement_type: ImprovementType::InstructionFailure,
            description: "High error rate detected - likely ignoring read-before-edit".to_string(),
            evidence: vec!["session-1".to_string()],
            proposed_fix: "Add stronger emphasis to read-before-edit instruction".to_string(),
            severity: 7,
            create_issue: true,
        };

        let changes = PromptUpdateGenerator::generate(&[improvement]);

        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].file_path, "CLAUDE.md");
        assert!(changes[0].new_text.contains("CRITICAL"));
    }

    #[test]
    fn test_generate_hook_updates() {
        let improvement = Improvement {
            improvement_type: ImprovementType::SafetyViolation,
            description: "3 unsafe operations detected (sqlite3 writes)".to_string(),
            evidence: vec!["session-1".to_string()],
            proposed_fix: "Add pre-submit hook to block unsafe operations".to_string(),
            severity: 10,
            create_issue: true,
        };

        let changes = HookUpdateGenerator::generate(&[improvement]);

        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].hook_name, "user-prompt-submit-hook");
        assert!(changes[0].new_command.contains("sqlite3"));
    }

    #[test]
    fn test_create_issue_format() {
        let improvement = Improvement {
            improvement_type: ImprovementType::ToolErrorPattern,
            description: "Read tool failed 10 times across sessions".to_string(),
            evidence: vec!["10 failures".to_string()],
            proposed_fix: "Add specific guidance for Read tool usage".to_string(),
            severity: 5,
            create_issue: true,
        };

        let issue = ImprovementIssueCreator::create_issue(&improvement).unwrap();

        assert!(issue.contains("title:"));
        assert!(issue.contains("## Problem"));
        assert!(issue.contains("## Evidence"));
        assert!(issue.contains("## Proposed Fix"));
        assert!(issue.contains("5/10"));
    }
}
