//! Diff tool call component with file changes.

use maud::{Markup, html};
use crate::acp::atoms::{ToolKind, ToolStatus};
use crate::acp::molecules::{ToolHeader, DiffHeader, PermissionBar};
use crate::acp::styles::{ACP_TOOL_CALL_CLASS, ACP_DIFF_CLASS, ACP_DIFF_ADD_CLASS, ACP_DIFF_DEL_CLASS, ACP_DIFF_CONTEXT_CLASS};

/// A line in a diff.
#[derive(Debug, Clone)]
pub enum DiffLine {
    /// Added line
    Add(String),
    /// Deleted line
    Del(String),
    /// Context line (unchanged)
    Context(String),
    /// Hunk header (@@...@@)
    Hunk(String),
}

/// Diff tool call with inline file changes.
pub struct DiffToolCall {
    file_path: String,
    lines: Vec<DiffLine>,
    additions: u32,
    deletions: u32,
    status: ToolStatus,
    entry_id: String,
    waiting_for_permission: bool,
}

impl DiffToolCall {
    /// Create a new diff tool call.
    pub fn new(file_path: impl Into<String>, entry_id: impl Into<String>) -> Self {
        Self {
            file_path: file_path.into(),
            lines: Vec::new(),
            additions: 0,
            deletions: 0,
            status: ToolStatus::Running,
            entry_id: entry_id.into(),
            waiting_for_permission: false,
        }
    }

    /// Set the diff lines.
    pub fn lines(mut self, lines: Vec<DiffLine>) -> Self {
        // Count additions and deletions
        for line in &lines {
            match line {
                DiffLine::Add(_) => self.additions += 1,
                DiffLine::Del(_) => self.deletions += 1,
                _ => {}
            }
        }
        self.lines = lines;
        self
    }

    /// Set the status.
    pub fn status(mut self, status: ToolStatus) -> Self {
        self.status = status;
        self
    }

    /// Mark as success.
    pub fn success(mut self) -> Self {
        self.status = ToolStatus::Success;
        self
    }

    /// Mark as waiting for permission.
    pub fn waiting_for_permission(mut self) -> Self {
        self.waiting_for_permission = true;
        self.status = ToolStatus::WaitingForConfirmation;
        self
    }

    /// Build the component.
    pub fn build(self) -> Markup {
        html! {
            div class=(ACP_TOOL_CALL_CLASS) data-entry-id=(self.entry_id) {
                details open {
                    summary class="cursor-pointer list-none" {
                        (ToolHeader::new(ToolKind::Edit, "Edit")
                            .status(self.status)
                            .build())
                    }

                    // Diff header with file info
                    (DiffHeader::new(&self.file_path)
                        .additions(self.additions)
                        .deletions(self.deletions)
                        .build())

                    // Diff content
                    @if !self.lines.is_empty() {
                        div class=(ACP_DIFF_CLASS) {
                            @for line in &self.lines {
                                @match line {
                                    DiffLine::Add(content) => {
                                        div class=(ACP_DIFF_ADD_CLASS) {
                                            span class="select-none pr-2" { "+" }
                                            (content)
                                        }
                                    }
                                    DiffLine::Del(content) => {
                                        div class=(ACP_DIFF_DEL_CLASS) {
                                            span class="select-none pr-2" { "-" }
                                            (content)
                                        }
                                    }
                                    DiffLine::Context(content) => {
                                        div class=(ACP_DIFF_CONTEXT_CLASS) {
                                            span class="select-none pr-2" { " " }
                                            (content)
                                        }
                                    }
                                    DiffLine::Hunk(header) => {
                                        div class="text-cyan bg-cyan/10 px-2" {
                                            (header)
                                        }
                                    }
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
