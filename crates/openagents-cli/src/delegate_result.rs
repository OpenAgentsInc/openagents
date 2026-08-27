//! Structured `delegate` tool results.
//!
//! The parent model reads JSON. The TUI box still shows the S2/S3 trailer
//! line, built from these fields rather than stored as prose.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// How one delegated agent finished.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DelegateStatus {
    Done,
    Failed,
    Cancelled,
}

/// A worktree left in place because the child changed it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorktreeRef {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
}

/// Isolation closeout. Missing from JSON when the run never made a worktree;
/// `null` when it made one and removed it; an object when it kept one.
///
/// A live #245 session had to `ls` the temp path to tell those last two
/// apart, because omitted and removed used to look the same.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum WorktreeOutcome {
    #[default]
    Unused,
    Removed,
    Kept(WorktreeRef),
}

impl WorktreeOutcome {
    fn is_unused(&self) -> bool {
        matches!(self, Self::Unused)
    }

    pub fn as_kept(&self) -> Option<&WorktreeRef> {
        match self {
            Self::Kept(worktree) => Some(worktree),
            _ => None,
        }
    }
}

impl Serialize for WorktreeOutcome {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Unused | Self::Removed => serializer.serialize_none(),
            Self::Kept(worktree) => worktree.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for WorktreeOutcome {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match Option::<WorktreeRef>::deserialize(deserializer)? {
            None => Self::Removed,
            Some(worktree) => Self::Kept(worktree),
        })
    }
}

/// One child's outcome, matching the Task-tool S4 envelope.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DelegateAgentResult {
    pub status: DelegateStatus,
    pub agent: String,
    pub total_tool_uses: u64,
    pub duration_ms: u64,
    pub total_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub report: String,
    #[serde(default, skip_serializing_if = "WorktreeOutcome::is_unused")]
    pub worktree: WorktreeOutcome,
}

/// Fan-out (`count` > 1, no `agent`): the old one-line header plus one record
/// per child.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DelegateFanoutResult {
    pub header: String,
    pub results: Vec<DelegateAgentResult>,
}

impl DelegateAgentResult {
    pub fn parse(text: &str) -> Option<Self> {
        let value: serde_json::Value = serde_json::from_str(text.trim()).ok()?;
        if value.get("status").is_none() || value.get("results").is_some() {
            return None;
        }
        serde_json::from_value(value).ok()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| self.report.clone())
    }

    /// The S2/S3 trailer the box still prints.
    pub fn render(&self) -> String {
        let status = match self.status {
            DelegateStatus::Done => "Done",
            DelegateStatus::Failed => "Failed",
            DelegateStatus::Cancelled => "Cancelled",
        };
        let mut bits = vec![
            status.to_string(),
            format!("{} tool uses", self.total_tool_uses),
            format!("{}s", self.duration_ms / 1000),
        ];
        if self.total_tokens > 0 {
            bits.push(format_tokens(self.total_tokens));
        }
        if let Some(model) = &self.model {
            bits.push(model.clone());
        }
        let mut text = bits.join(" · ");
        if !self.report.trim().is_empty() {
            text.push('\n');
            text.push_str(self.report.trim());
        }
        match &self.worktree {
            WorktreeOutcome::Kept(worktree) => {
                let branch = worktree.branch.as_deref().unwrap_or("HEAD");
                text.push_str(&format!(
                    "\nworktree kept: {} (branch {branch})",
                    worktree.path
                ));
            }
            WorktreeOutcome::Removed => {
                text.push_str("\nworktree removed (no changes)");
            }
            WorktreeOutcome::Unused => {}
        }
        text
    }
}

impl DelegateFanoutResult {
    pub fn parse(text: &str) -> Option<Self> {
        let value: serde_json::Value = serde_json::from_str(text.trim()).ok()?;
        if value.get("results").is_none() || value.get("header").is_none() {
            return None;
        }
        serde_json::from_value(value).ok()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| self.header.clone())
    }

    pub fn render(&self) -> String {
        let mut text = self.header.clone();
        for result in &self.results {
            text.push_str("\n\n");
            text.push_str(&result.render());
        }
        text
    }
}

/// The text a tool box should show: the trailer when `raw` is a result record,
/// otherwise the raw output unchanged.
pub fn displayed_delegate_output(raw: &str) -> String {
    if let Some(one) = DelegateAgentResult::parse(raw) {
        return one.render();
    }
    if let Some(many) = DelegateFanoutResult::parse(raw) {
        return many.render();
    }
    raw.to_string()
}

fn format_tokens(total: u64) -> String {
    if total >= 1000 {
        let thousands = total as f64 / 1000.0;
        format!("{thousands:.1}k tokens")
    } else {
        format!("{total} tokens")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_result(worktree: WorktreeOutcome) -> DelegateAgentResult {
        DelegateAgentResult {
            status: DelegateStatus::Done,
            agent: "coder-mini".to_string(),
            total_tool_uses: 1,
            duration_ms: 1000,
            total_tokens: 10,
            model: None,
            session_id: None,
            report: "ok".to_string(),
            worktree,
        }
    }

    #[test]
    fn render_keeps_the_true_tool_use_count_when_a_box_would_clip() {
        let result = DelegateAgentResult {
            status: DelegateStatus::Done,
            agent: "coder-mini".to_string(),
            total_tool_uses: 14,
            duration_ms: 96_000,
            total_tokens: 41_234,
            model: Some("glm-5.3-flash".to_string()),
            session_id: Some("th_mini".to_string()),
            report: "the finding".to_string(),
            worktree: WorktreeOutcome::Unused,
        };
        let shown = result.render();
        assert!(shown.starts_with("Done · 14 tool uses · 96s · 41.2k tokens · glm-5.3-flash"));
        assert!(shown.contains("the finding"));
        let parsed = DelegateAgentResult::parse(&result.to_json()).unwrap();
        assert_eq!(parsed.total_tool_uses, 14);
        assert_eq!(parsed.total_tokens, 41_234);
        assert_eq!(parsed.session_id.as_deref(), Some("th_mini"));
    }

    #[test]
    fn a_fan_out_envelope_carries_one_record_per_child() {
        let envelope = DelegateFanoutResult {
            header: "3 of 3 children completed on openagents (this process, the OpenAgents proxy)."
                .to_string(),
            results: (1..=3)
                .map(|id| DelegateAgentResult {
                    status: DelegateStatus::Done,
                    agent: "openagents".to_string(),
                    total_tool_uses: 0,
                    duration_ms: id * 10,
                    total_tokens: 0,
                    model: None,
                    session_id: None,
                    report: format!("child {id}"),
                    worktree: WorktreeOutcome::Unused,
                })
                .collect(),
        };
        assert_eq!(envelope.results.len(), 3);
        let parsed = DelegateFanoutResult::parse(&envelope.to_json()).unwrap();
        assert_eq!(parsed.results.len(), 3);
        assert_eq!(parsed.results[0].report, "child 1");
        assert_eq!(parsed.results[2].report, "child 3");
        let shown = envelope.render();
        assert!(shown.starts_with("3 of 3 children completed"));
        assert!(shown.contains("child 2"));
    }

    #[test]
    fn prose_output_is_not_mistaken_for_a_result_record() {
        assert!(DelegateAgentResult::parse("Done · 1 tool uses · 0s\nhello").is_none());
        assert_eq!(
            displayed_delegate_output("ordinary tool output"),
            "ordinary tool output"
        );
    }

    #[test]
    fn unused_worktree_is_omitted_removed_is_null() {
        let unused = sample_result(WorktreeOutcome::Unused).to_json();
        let unused_json: serde_json::Value = serde_json::from_str(&unused).unwrap();
        assert!(unused_json.get("worktree").is_none(), "{unused}");

        let removed = sample_result(WorktreeOutcome::Removed);
        let removed_json: serde_json::Value = serde_json::from_str(&removed.to_json()).unwrap();
        assert!(removed_json.get("worktree").unwrap().is_null());
        assert!(removed.render().contains("worktree removed (no changes)"));
        let parsed = DelegateAgentResult::parse(&removed.to_json()).unwrap();
        assert_eq!(parsed.worktree, WorktreeOutcome::Removed);

        let kept = sample_result(WorktreeOutcome::Kept(WorktreeRef {
            path: "/tmp/wt".to_string(),
            branch: Some("agent-x".to_string()),
        }));
        let kept_json: serde_json::Value = serde_json::from_str(&kept.to_json()).unwrap();
        assert_eq!(kept_json["worktree"]["path"], "/tmp/wt");
        assert_eq!(kept_json["worktree"]["branch"], "agent-x");
        assert!(
            kept.render()
                .contains("worktree kept: /tmp/wt (branch agent-x)")
        );
    }
}
