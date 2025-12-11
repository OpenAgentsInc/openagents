//! Terminal wrapper for ACP tool execution.
//!
//! This module provides a simple terminal abstraction for displaying
//! tool execution output from Claude Code.

use agent_client_protocol as acp;
use std::path::PathBuf;

/// Terminal output state.
#[derive(Clone, Debug)]
pub struct TerminalOutput {
    /// Terminal ID.
    pub id: acp::TerminalId,
    /// Command label.
    pub label: String,
    /// Working directory.
    pub cwd: Option<PathBuf>,
    /// Captured output.
    pub output: String,
    /// Exit status.
    pub exit_status: Option<acp::TerminalExitStatus>,
    /// Whether the terminal has exited.
    pub exited: bool,
}

impl TerminalOutput {
    /// Create a new terminal output.
    pub fn new(id: acp::TerminalId, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            cwd: None,
            output: String::new(),
            exit_status: None,
            exited: false,
        }
    }

    /// Set the working directory.
    pub fn with_cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// Append output data.
    pub fn append(&mut self, data: &[u8]) {
        if let Ok(text) = std::str::from_utf8(data) {
            self.output.push_str(text);
        } else {
            // Try lossy conversion for non-UTF8 data
            self.output.push_str(&String::from_utf8_lossy(data));
        }
    }

    /// Set the exit status.
    pub fn set_exit(&mut self, status: acp::TerminalExitStatus) {
        self.exit_status = Some(status);
        self.exited = true;
    }

    /// Check if the terminal has exited.
    pub fn has_exited(&self) -> bool {
        self.exited
    }

    /// Get the exit code if available.
    pub fn exit_code(&self) -> Option<i32> {
        self.exit_status
            .as_ref()
            .and_then(|s| s.exit_code.map(|c| c as i32))
    }

    /// Check if the terminal exited successfully.
    pub fn succeeded(&self) -> bool {
        self.exit_code() == Some(0)
    }

    /// Get the output as lines.
    pub fn lines(&self) -> impl Iterator<Item = &str> {
        self.output.lines()
    }

    /// Get the last N lines of output.
    pub fn last_lines(&self, n: usize) -> Vec<&str> {
        self.output.lines().rev().take(n).collect::<Vec<_>>().into_iter().rev().collect()
    }

    /// Get the output length in bytes.
    pub fn output_len(&self) -> usize {
        self.output.len()
    }

    /// Clear the output.
    pub fn clear(&mut self) {
        self.output.clear();
    }
}

/// Events from a terminal.
#[derive(Clone, Debug)]
pub enum TerminalEvent {
    /// Output received.
    Output(Vec<u8>),
    /// Terminal exited.
    Exited(acp::TerminalExitStatus),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_output() {
        let mut terminal = TerminalOutput::new(acp::TerminalId::new("test-1"), "echo hello");

        terminal.append(b"hello\n");
        terminal.append(b"world\n");

        assert_eq!(terminal.output, "hello\nworld\n");
        assert!(!terminal.has_exited());

        terminal.set_exit(acp::TerminalExitStatus::new().exit_code(Some(0)));

        assert!(terminal.has_exited());
        assert!(terminal.succeeded());
        assert_eq!(terminal.exit_code(), Some(0));
    }

    #[test]
    fn test_terminal_lines() {
        let mut terminal = TerminalOutput::new(acp::TerminalId::new("test-2"), "test");
        terminal.append(b"line 1\nline 2\nline 3\nline 4\nline 5\n");

        let last_2 = terminal.last_lines(2);
        assert_eq!(last_2, vec!["line 4", "line 5"]);
    }
}
