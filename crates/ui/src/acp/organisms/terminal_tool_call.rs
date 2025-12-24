//! Terminal tool call component with embedded terminal output.

use maud::{Markup, html, PreEscaped};
use crate::acp::atoms::{ToolKind, ToolStatus};
use crate::acp::molecules::{ToolHeader, TerminalHeader, ExitStatus, PermissionBar};
use crate::acp::styles::{ACP_TOOL_CALL_CLASS, ACP_TERMINAL_CLASS};

/// Terminal tool call with command output.
pub struct TerminalToolCall {
    command: String,
    output: String,
    working_dir: Option<String>,
    exit_status: ExitStatus,
    entry_id: String,
    waiting_for_permission: bool,
    truncated: bool,
}

impl TerminalToolCall {
    /// Create a new terminal tool call.
    pub fn new(command: impl Into<String>, entry_id: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            output: String::new(),
            working_dir: None,
            exit_status: ExitStatus::Running,
            entry_id: entry_id.into(),
            waiting_for_permission: false,
            truncated: false,
        }
    }

    /// Set the output.
    pub fn output(mut self, output: impl Into<String>) -> Self {
        self.output = output.into();
        self
    }

    /// Set the working directory.
    pub fn working_dir(mut self, dir: impl Into<String>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    /// Set exit status to success.
    pub fn success(mut self) -> Self {
        self.exit_status = ExitStatus::Success;
        self
    }

    /// Set exit status to failed.
    pub fn failed(mut self, code: i32) -> Self {
        self.exit_status = ExitStatus::Failed(code);
        self
    }

    /// Mark output as truncated.
    pub fn truncated(mut self) -> Self {
        self.truncated = true;
        self
    }

    /// Mark as waiting for permission.
    pub fn waiting_for_permission(mut self) -> Self {
        self.waiting_for_permission = true;
        self
    }

    /// Build the component.
    pub fn build(self) -> Markup {
        let tool_status = match &self.exit_status {
            ExitStatus::Running => ToolStatus::Running,
            ExitStatus::Success => ToolStatus::Success,
            ExitStatus::Failed(code) => ToolStatus::Error(format!("exit {}", code)),
        };

        // Build the terminal header outside the html! macro
        let mut header = TerminalHeader::new(&self.command)
            .exit_status(self.exit_status);
        if let Some(dir) = &self.working_dir {
            header = header.working_dir(dir);
        }
        let header_markup = header.build();

        html! {
            div class=(ACP_TOOL_CALL_CLASS) data-entry-id=(self.entry_id) {
                details open {
                    summary class="cursor-pointer list-none" {
                        (ToolHeader::new(ToolKind::Execute, "Bash")
                            .status(tool_status)
                            .build())
                    }

                    // Terminal header with command
                    (header_markup)

                    // Terminal output
                    @if !self.output.is_empty() {
                        div class=(ACP_TERMINAL_CLASS) {
                            pre class="whitespace-pre-wrap" {
                                (PreEscaped(&self.output))
                            }
                            @if self.truncated {
                                div class="mt-2 text-xs text-muted-foreground italic" {
                                    "Output truncated..."
                                }
                            }
                        }
                    }

                    // Permission bar
                    @if self.waiting_for_permission {
                        (PermissionBar::new().build())
                    }
                }
            }
        }
    }
}
