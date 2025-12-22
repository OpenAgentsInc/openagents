//! rlog format writer for trajectory output

use crate::redact::redact_secrets;
use crate::trajectory::{Step, StepType, Trajectory};
use serde_json::Value;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;

/// Writer for rlog format
pub struct RlogWriter {
    lines: Vec<String>,
    /// Optional file writer for streaming mode
    file_writer: Option<BufWriter<File>>,
}

impl RlogWriter {
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            file_writer: None,
        }
    }

    /// Create a new writer in streaming mode that writes to a file
    pub fn new_streaming(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let path = path.as_ref();
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?;

        Ok(Self {
            lines: Vec::new(),
            file_writer: Some(BufWriter::new(file)),
        })
    }

    /// Write header to the file (for streaming mode)
    pub fn write_header(&mut self, traj: &Trajectory) -> std::io::Result<()> {
        let mut header_lines = Vec::new();
        header_lines.push("---".to_string());
        header_lines.push("format: rlog/1".to_string());
        header_lines.push(format!("id: {}", traj.session_id));
        header_lines.push(format!("repo_sha: {}", traj.repo_sha));

        if let Some(ref branch) = traj.branch {
            header_lines.push(format!("branch: {}", branch));
        }

        header_lines.push(format!("model: {}", traj.model));
        header_lines.push(format!("cwd: {}", traj.cwd));
        header_lines.push("agent: autopilot".to_string());
        header_lines.push(format!("version: {}", env!("CARGO_PKG_VERSION")));

        // Token summaries (will be updated at end)
        header_lines.push(format!(
            "tokens_total_in: {}",
            traj.usage.input_tokens
        ));
        header_lines.push(format!(
            "tokens_total_out: {}",
            traj.usage.output_tokens
        ));
        header_lines.push(format!(
            "tokens_cached: {}",
            traj.usage.cache_read_tokens
        ));

        header_lines.push("---".to_string());
        header_lines.push(String::new());

        // Write start marker
        let id = if traj.session_id.len() > 8 {
            &traj.session_id[..8]
        } else {
            &traj.session_id
        };
        header_lines.push(format!(
            "@start id={} ts={}",
            id,
            traj.started_at.format("%Y-%m-%dT%H:%M:%SZ")
        ));

        if let Some(writer) = &mut self.file_writer {
            for line in &header_lines {
                writeln!(writer, "{}", line)?;
            }
            writer.flush()?;
        }

        Ok(())
    }

    /// Update header with session ID and current token totals (rewrites the file)
    /// This is called after the SDK Init message provides the session_id
    pub fn update_header(&mut self, path: impl AsRef<Path>, traj: &Trajectory) -> std::io::Result<()> {
        use std::io::{BufRead, BufReader};

        // Read existing content after the header
        let existing_steps = if let Ok(file) = File::open(path.as_ref()) {
            let reader = BufReader::new(file);
            let mut lines = Vec::new();
            let mut in_header = true;
            let mut header_end_count = 0;

            for line in reader.lines() {
                let line = line?;
                if in_header {
                    if line == "---" {
                        header_end_count += 1;
                        if header_end_count == 2 {
                            in_header = false;
                        }
                    }
                } else {
                    // Skip the @start line, we'll rewrite it
                    if !line.starts_with("@start") {
                        lines.push(line);
                    }
                }
            }
            lines
        } else {
            Vec::new()
        };

        // Reopen file for writing (truncate)
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path.as_ref())?;

        self.file_writer = Some(BufWriter::new(file));

        // Write updated header
        self.write_header(traj)?;

        // Write back existing steps
        if let Some(writer) = &mut self.file_writer {
            for line in existing_steps {
                writeln!(writer, "{}", line)?;
            }
            writer.flush()?;
        }

        Ok(())
    }

    /// Append a single step to the file (for streaming mode)
    pub fn append_step(&mut self, step: &Step) -> std::io::Result<()> {
        let line = self.step_to_line(step);

        if let Some(writer) = &mut self.file_writer {
            writeln!(writer, "{}", line)?;
            writer.flush()?;
        }

        Ok(())
    }

    /// Write footer to the file (for streaming mode)
    pub fn write_footer(&mut self, traj: &Trajectory) -> std::io::Result<()> {
        let footer = format!(
            "@end tokens_in={} tokens_out={} cost_usd={:.4}",
            traj.usage.input_tokens, traj.usage.output_tokens, traj.usage.cost_usd
        );

        if let Some(writer) = &mut self.file_writer {
            writeln!(writer, "{}", footer)?;
            writer.flush()?;
        }

        Ok(())
    }

    /// Close the file writer (for streaming mode)
    pub fn close(&mut self) -> std::io::Result<()> {
        if let Some(mut writer) = self.file_writer.take() {
            writer.flush()?;
        }
        Ok(())
    }

    /// Write trajectory to rlog format string (batch mode)
    pub fn write(&mut self, traj: &Trajectory) -> String {
        self.lines.clear();
        self.write_header_batch(traj);
        self.write_start_batch(traj);
        self.write_steps_batch(traj);
        self.write_end_batch(traj);
        self.lines.join("\n")
    }

    fn write_header_batch(&mut self, traj: &Trajectory) {
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

    fn write_start_batch(&mut self, traj: &Trajectory) {
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

    fn write_steps_batch(&mut self, traj: &Trajectory) {
        for step in &traj.steps {
            self.lines.push(self.step_to_line(step));
        }
    }

    fn write_end_batch(&mut self, traj: &Trajectory) {
        self.lines.push(format!(
            "@end tokens_in={} tokens_out={} cost_usd={:.4}",
            traj.usage.input_tokens, traj.usage.output_tokens, traj.usage.cost_usd
        ));
    }

    fn step_to_line(&self, step: &Step) -> String {
        let mut line = match &step.step_type {
            StepType::User { content } => {
                let redacted = redact_secrets(content);
                format!("u: {}", truncate(&redacted, 200))
            }
            StepType::Assistant { content } => {
                let redacted = redact_secrets(content);
                format!("a: {}", truncate(&redacted, 200))
            }
            StepType::Thinking { content, signature } => {
                let redacted = redact_secrets(content);
                let mut l = format!("th: {}", truncate(&redacted, 150));
                if let Some(sig) = signature {
                    let sig_short = if sig.len() > 20 { &sig[..20] } else { sig };
                    l.push_str(&format!(" sig={}...", sig_short));
                }
                l
            }
            StepType::ToolCall { tool, tool_id, input } => {
                let args = format_tool_args(tool, input);
                let id_short = if tool_id.len() > 8 {
                    &tool_id[tool_id.len() - 8..]
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
                    &tool_id[tool_id.len() - 8..]
                } else {
                    tool_id
                };
                let content = output.as_deref().unwrap_or("");
                let redacted = redact_secrets(content);
                format!("o: id={} → {} {}", id_short, status, truncate(&redacted, 100))
            }
            StepType::SystemInit { model } => {
                format!("@init model={}", model)
            }
            StepType::SystemStatus { status } => {
                format!("# status: {}", status)
            }
            StepType::Subagent {
                agent_id,
                agent_type,
                status,
                summary,
            } => {
                use crate::trajectory::SubagentStatus;
                let id_short = if agent_id.len() > 8 {
                    &agent_id[agent_id.len() - 8..]
                } else {
                    agent_id
                };
                let status_str = match status {
                    SubagentStatus::Started => "[started]",
                    SubagentStatus::Done => "[done]",
                    SubagentStatus::Error => "[error]",
                };
                let mut l = format!("x:{} id={} → {}", agent_type, id_short, status_str);
                if let Some(s) = summary {
                    let redacted = redact_secrets(s);
                    l.push_str(&format!(" summary=\"{}\"", truncate(&redacted, 80)));
                }
                l
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
            .map(|c| {
                let redacted = redact_secrets(c);
                format!("cmd=\"{}\"", truncate(&redacted, 50))
            })
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
    use crate::trajectory::TokenUsage;

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

    #[test]
    fn test_all_step_types() {
        let mut traj = Trajectory::new(
            "Test all steps".to_string(),
            "claude-sonnet-4".to_string(),
            "/test".to_string(),
            "abc".to_string(),
            None,
        );
        traj.session_id = "test_id".to_string();

        // Add all step types
        traj.add_step(StepType::SystemInit {
            model: "claude-sonnet-4".to_string(),
        });
        traj.add_step(StepType::User {
            content: "User message".to_string(),
        });
        traj.add_step(StepType::Thinking {
            content: "Thinking content".to_string(),
            signature: Some("sig123".to_string()),
        });
        traj.add_step(StepType::ToolCall {
            tool: "Read".to_string(),
            tool_id: "toolu_abc123".to_string(),
            input: serde_json::json!({"file_path": "/test/file.rs"}),
        });
        traj.add_step(StepType::ToolResult {
            tool_id: "toolu_abc123".to_string(),
            success: true,
            output: Some("File contents".to_string()),
        });
        traj.add_step(StepType::SystemStatus {
            status: "Complete".to_string(),
        });

        let mut writer = RlogWriter::new();
        let output = writer.write(&traj);

        // Verify all step types are formatted
        assert!(output.contains("@init model=claude-sonnet-4"));
        assert!(output.contains("u: User message"));
        assert!(output.contains("th: Thinking content"));
        assert!(output.contains("t!:Read"));
        assert!(output.contains("file_path=/test/file.rs"));
        assert!(output.contains("o:"));
        assert!(output.contains("[ok]"));
        assert!(output.contains("# status: Complete"));
    }

    #[test]
    fn test_truncation() {
        let mut traj = Trajectory::new(
            "Test truncation".to_string(),
            "claude".to_string(),
            "/".to_string(),
            "a".to_string(),
            None,
        );
        traj.session_id = "t".to_string();

        // Add very long content that should be truncated
        let long_content = "x".repeat(500);
        traj.add_step(StepType::User {
            content: long_content,
        });

        let mut writer = RlogWriter::new();
        let output = writer.write(&traj);

        // Should be truncated to ~200 chars plus "..."
        let user_line = output.lines().find(|l| l.starts_with("u:")).unwrap();
        assert!(user_line.len() < 210);
        assert!(user_line.contains("..."));
    }

    #[test]
    fn test_tool_result_formatting() {
        let mut traj = Trajectory::new("Test".to_string(), "m".to_string(), "/".to_string(), "a".to_string(), None);
        traj.session_id = "t".to_string();

        // Success result
        traj.add_step(StepType::ToolResult {
            tool_id: "tool_success".to_string(),
            success: true,
            output: Some("Success output".to_string()),
        });

        // Error result
        traj.add_step(StepType::ToolResult {
            tool_id: "tool_error".to_string(),
            success: false,
            output: Some("Error message".to_string()),
        });

        let mut writer = RlogWriter::new();
        let output = writer.write(&traj);

        // Check both success and error formatting
        assert!(output.contains("[ok]"));
        assert!(output.contains("[error]"));
    }

    #[test]
    fn test_header_fields() {
        let mut traj = Trajectory::new(
            "Test header".to_string(),
            "claude-opus-4".to_string(),
            "/home/test".to_string(),
            "commit_sha".to_string(),
            Some("feature-branch".to_string()),
        );
        traj.session_id = "session_abc".to_string();
        traj.usage = TokenUsage {
            input_tokens: 1000,
            output_tokens: 500,
            cache_read_tokens: 200,
            cache_creation_tokens: 100,
            cost_usd: 0.05,
        };

        let mut writer = RlogWriter::new();
        let output = writer.write(&traj);

        // Verify all header fields
        assert!(output.contains("format: rlog/1"));
        assert!(output.contains("id: session_abc"));
        assert!(output.contains("repo_sha: commit_sha"));
        assert!(output.contains("branch: feature-branch"));
        assert!(output.contains("model: claude-opus-4"));
        assert!(output.contains("cwd: /home/test"));
        assert!(output.contains("agent: autopilot"));
        assert!(output.contains("tokens_total_in: 1000"));
        assert!(output.contains("tokens_total_out: 500"));
        assert!(output.contains("tokens_cached: 200"));
    }

    #[test]
    fn test_special_characters() {
        let mut traj = Trajectory::new("Test".to_string(), "m".to_string(), "/".to_string(), "a".to_string(), None);
        traj.session_id = "t".to_string();

        // Add content with special characters
        traj.add_step(StepType::User {
            content: "Content with \"quotes\" and 'apostrophes' and <brackets>".to_string(),
        });

        let mut writer = RlogWriter::new();
        let output = writer.write(&traj);

        // Should preserve special characters
        assert!(output.contains("\"quotes\""));
        assert!(output.contains("'apostrophes'"));
        assert!(output.contains("<brackets>"));
    }
}
