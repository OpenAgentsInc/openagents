//! Terminal output header component.

use maud::{Markup, html};

/// Exit status of a terminal command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExitStatus {
    /// Command is still running
    Running,
    /// Command succeeded with exit code 0
    Success,
    /// Command failed with non-zero exit code
    Failed(i32),
}

/// Header for terminal command output.
pub struct TerminalHeader {
    command: String,
    working_dir: Option<String>,
    exit_status: ExitStatus,
}

impl TerminalHeader {
    /// Create a new terminal header.
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            working_dir: None,
            exit_status: ExitStatus::Running,
        }
    }

    /// Set the working directory.
    pub fn working_dir(mut self, dir: impl Into<String>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    /// Set the exit status.
    pub fn exit_status(mut self, status: ExitStatus) -> Self {
        self.exit_status = status;
        self
    }

    /// Build the component.
    pub fn build(self) -> Markup {
        let (status_icon, status_class, status_text) = match &self.exit_status {
            ExitStatus::Running => ("*", "text-blue animate-pulse", "running"),
            ExitStatus::Success => ("+", "text-green", "exit 0"),
            ExitStatus::Failed(_code) => ("x", "text-red", ""),
        };

        html! {
            div class="px-3 py-2 bg-background border-b border-border" {
                // Command preview
                div class="flex items-center gap-2" {
                    span class="text-green text-xs" { "$" }
                    code class="text-sm font-mono text-foreground flex-1 truncate" {
                        (self.command)
                    }

                    // Exit status
                    span class={ "text-xs font-mono " (status_class) } {
                        (status_icon)
                        " "
                        @if let ExitStatus::Failed(code) = &self.exit_status {
                            "exit " (code)
                        } @else {
                            (status_text)
                        }
                    }
                }

                // Working directory
                @if let Some(dir) = &self.working_dir {
                    div class="text-xs text-muted-foreground font-mono mt-1 truncate" {
                        "in " (dir)
                    }
                }
            }
        }
    }
}
