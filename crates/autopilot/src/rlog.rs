//! rlog format writer for trajectory output

use crate::trajectory::{Step, StepType, Trajectory};
use serde_json::Value;

/// Writer for rlog format
pub struct RlogWriter {
    lines: Vec<String>,
}

impl RlogWriter {
    pub fn new() -> Self {
        Self { lines: Vec::new() }
    }

    /// Write trajectory to rlog format string
    pub fn write(&mut self, traj: &Trajectory) -> String {
        self.lines.clear();
        self.write_header(traj);
        self.write_start(traj);
        self.write_steps(traj);
        self.write_end(traj);
        self.lines.join("\n")
    }

    fn write_header(&mut self, traj: &Trajectory) {
        self.lines.push("---".to_string());
        self.lines.push("format: rlog/1".to_string());
        self.lines.push(format!("id: {}", traj.session_id));
        self.lines.push(format!("repo_sha: {}", traj.repo_sha));

        if let Some(ref branch) = traj.branch {
            self.lines.push(format!("branch: {}", branch));
        }

        self.lines.push(format!("model: {}", traj.model));
        self.lines.push(format!("cwd: {}", traj.cwd));
        self.lines.push("agent: autopilot".to_string());
        self.lines
            .push(format!("version: {}", env!("CARGO_PKG_VERSION")));

        // Token summaries
        self.lines.push(format!(
            "tokens_total_in: {}",
            traj.usage.input_tokens
        ));
        self.lines.push(format!(
            "tokens_total_out: {}",
            traj.usage.output_tokens
        ));
        self.lines.push(format!(
            "tokens_cached: {}",
            traj.usage.cache_read_tokens
        ));

        self.lines.push("---".to_string());
        self.lines.push(String::new());
    }

    fn write_start(&mut self, traj: &Trajectory) {
        let id = if traj.session_id.len() > 8 {
            &traj.session_id[..8]
        } else {
            &traj.session_id
        };
        self.lines.push(format!(
            "@start id={} ts={}",
            id,
            traj.started_at.format("%Y-%m-%dT%H:%M:%SZ")
        ));
    }

    fn write_steps(&mut self, traj: &Trajectory) {
        for step in &traj.steps {
            self.lines.push(self.step_to_line(step));
        }
    }

    fn write_end(&mut self, traj: &Trajectory) {
        self.lines.push(format!(
            "@end tokens_in={} tokens_out={} cost_usd={:.4}",
            traj.usage.input_tokens, traj.usage.output_tokens, traj.usage.cost_usd
        ));
    }

    fn step_to_line(&self, step: &Step) -> String {
        let mut line = match &step.step_type {
            StepType::User { content } => {
                format!("u: {}", truncate(content, 200))
            }
            StepType::Assistant { content } => {
                format!("a: {}", truncate(content, 200))
            }
            StepType::Thinking { content, signature } => {
                let mut l = format!("th: {}", truncate(content, 150));
                if let Some(sig) = signature {
                    let sig_short = if sig.len() > 20 { &sig[..20] } else { sig };
                    l.push_str(&format!(" sig={}...", sig_short));
                }
                l
            }
            StepType::ToolCall { tool, tool_id, input } => {
                let args = format_tool_args(tool, input);
                let id_short = if tool_id.len() > 8 {
                    &tool_id[..8]
                } else {
                    tool_id
                };
                format!("t!:{} id={} {} → [running]", tool, id_short, args)
            }
            StepType::ToolResult {
                tool_id,
                success,
                output,
            } => {
                let status = if *success { "[ok]" } else { "[error]" };
                let id_short = if tool_id.len() > 8 {
                    &tool_id[..8]
                } else {
                    tool_id
                };
                let content = output.as_deref().unwrap_or("");
                format!("o: id={} → {} {}", id_short, status, truncate(content, 100))
            }
            StepType::SystemInit { model } => {
                format!("@init model={}", model)
            }
            StepType::SystemStatus { status } => {
                format!("# status: {}", status)
            }
        };

        // Append token metadata
        if let Some(t) = step.tokens_in {
            line.push_str(&format!(" tokens_in={}", t));
        }
        if let Some(t) = step.tokens_out {
            line.push_str(&format!(" tokens_out={}", t));
        }
        if let Some(t) = step.tokens_cached {
            if t > 0 {
                line.push_str(&format!(" tokens_cached={}", t));
            }
        }

        line
    }
}

impl Default for RlogWriter {
    fn default() -> Self {
        Self::new()
    }
}

fn truncate(s: &str, max: usize) -> String {
    let first_line = s.lines().next().unwrap_or("");
    if first_line.chars().count() <= max {
        first_line.to_string()
    } else {
        format!(
            "{}...",
            first_line.chars().take(max - 3).collect::<String>()
        )
    }
}

fn format_tool_args(tool_name: &str, input: &Value) -> String {
    match tool_name {
        "Read" => input
            .get("file_path")
            .and_then(|v| v.as_str())
            .map(|p| format!("file_path={}", p))
            .unwrap_or_default(),
        "Bash" => input
            .get("command")
            .and_then(|v| v.as_str())
            .map(|c| format!("cmd=\"{}\"", truncate(c, 50)))
            .unwrap_or_default(),
        "Edit" | "Write" => input
            .get("file_path")
            .and_then(|v| v.as_str())
            .map(|p| format!("file_path={}", p))
            .unwrap_or_default(),
        "Glob" => input
            .get("pattern")
            .and_then(|v| v.as_str())
            .map(|p| format!("pattern=\"{}\"", p))
            .unwrap_or_default(),
        "Grep" => input
            .get("pattern")
            .and_then(|v| v.as_str())
            .map(|p| format!("pattern=\"{}\"", truncate(p, 30)))
            .unwrap_or_default(),
        "Task" => input
            .get("description")
            .and_then(|v| v.as_str())
            .map(|d| format!("desc=\"{}\"", truncate(d, 40)))
            .unwrap_or_default(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trajectory::{TokenUsage, TrajectoryResult};
    use chrono::Utc;

    #[test]
    fn test_rlog_output() {
        let mut traj = Trajectory::new(
            "Test prompt".to_string(),
            "claude-sonnet".to_string(),
            "/test/cwd".to_string(),
            "abc123".to_string(),
            Some("main".to_string()),
        );
        traj.session_id = "sess_test123".to_string();
        traj.add_step(StepType::User {
            content: "Hello".to_string(),
        });
        traj.add_step(StepType::Assistant {
            content: "Hi there!".to_string(),
        });
        traj.usage = TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            cache_read_tokens: 10,
            cache_creation_tokens: 0,
            cost_usd: 0.01,
        };

        let mut writer = RlogWriter::new();
        let output = writer.write(&traj);

        assert!(output.contains("format: rlog/1"));
        assert!(output.contains("id: sess_test123"));
        assert!(output.contains("u: Hello"));
        assert!(output.contains("a: Hi there!"));
        assert!(output.contains("@end tokens_in=100"));
    }
}
