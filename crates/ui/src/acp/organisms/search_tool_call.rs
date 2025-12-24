//! Search tool call component with file list.

use maud::{Markup, html};
use crate::acp::atoms::{ToolKind, ToolStatus};
use crate::acp::molecules::{ToolHeader, PermissionBar};
use crate::acp::styles::{ACP_TOOL_CALL_CLASS, ACP_CONTENT_CLASS};

/// A search result file.
pub struct SearchResult {
    pub path: String,
    pub preview: Option<String>,
    pub line_number: Option<u32>,
}

/// Search tool call with file list results.
pub struct SearchToolCall {
    query: String,
    results: Vec<SearchResult>,
    status: ToolStatus,
    entry_id: String,
    waiting_for_permission: bool,
}

impl SearchToolCall {
    /// Create a new search tool call.
    pub fn new(query: impl Into<String>, entry_id: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            results: Vec::new(),
            status: ToolStatus::Running,
            entry_id: entry_id.into(),
            waiting_for_permission: false,
        }
    }

    /// Set the search results.
    pub fn results(mut self, results: Vec<SearchResult>) -> Self {
        self.results = results;
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
        let result_count = self.results.len();

        html! {
            div class=(ACP_TOOL_CALL_CLASS) data-entry-id=(self.entry_id) {
                details open {
                    summary class="cursor-pointer list-none" {
                        (ToolHeader::new(ToolKind::Search, format!("Search: {}", self.query))
                            .status(self.status)
                            .build())
                    }

                    // Results list
                    div class=(ACP_CONTENT_CLASS) {
                        @if self.results.is_empty() {
                            p class="text-sm text-muted-foreground italic" {
                                "No results found"
                            }
                        } @else {
                            p class="text-xs text-muted-foreground mb-2" {
                                (result_count) " file(s) found"
                            }
                            ul class="space-y-1" {
                                @for result in &self.results {
                                    li class="flex items-start gap-2" {
                                        // File icon
                                        span class="text-xs text-muted-foreground" { "[-]" }

                                        div class="flex-1 min-w-0" {
                                            // File path
                                            div class="text-sm font-mono text-foreground truncate" {
                                                (result.path)
                                                @if let Some(line) = result.line_number {
                                                    span class="text-muted-foreground" {
                                                        ":" (line)
                                                    }
                                                }
                                            }

                                            // Preview
                                            @if let Some(preview) = &result.preview {
                                                div class="text-xs text-muted-foreground truncate mt-0.5" {
                                                    (preview)
                                                }
                                            }
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
