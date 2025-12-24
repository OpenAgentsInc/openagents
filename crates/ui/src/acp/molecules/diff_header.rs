//! Diff content header component.

use maud::{Markup, html};

/// Header for diff content showing file path and change stats.
pub struct DiffHeader {
    file_path: String,
    additions: u32,
    deletions: u32,
}

impl DiffHeader {
    /// Create a new diff header.
    pub fn new(file_path: impl Into<String>) -> Self {
        Self {
            file_path: file_path.into(),
            additions: 0,
            deletions: 0,
        }
    }

    /// Set the number of additions.
    pub fn additions(mut self, count: u32) -> Self {
        self.additions = count;
        self
    }

    /// Set the number of deletions.
    pub fn deletions(mut self, count: u32) -> Self {
        self.deletions = count;
        self
    }

    /// Build the component.
    pub fn build(self) -> Markup {
        // Infer file type icon from extension
        let icon = infer_file_icon(&self.file_path);

        html! {
            div class="flex items-center gap-2 px-3 py-2 bg-secondary border-b border-border" {
                // File icon
                span class="text-xs text-muted-foreground" { (icon) }

                // File path
                span class="text-sm font-mono text-foreground flex-1 truncate" {
                    (self.file_path)
                }

                // Change stats
                @if self.additions > 0 || self.deletions > 0 {
                    div class="flex gap-2 text-xs font-mono" {
                        @if self.additions > 0 {
                            span class="text-green" { "+" (self.additions) }
                        }
                        @if self.deletions > 0 {
                            span class="text-red" { "-" (self.deletions) }
                        }
                    }
                }
            }
        }
    }
}

/// Infer a file icon from the file path extension.
fn infer_file_icon(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext {
        "rs" => ".rs",
        "ts" | "tsx" => ".ts",
        "js" | "jsx" => ".js",
        "py" => ".py",
        "go" => ".go",
        "md" => ".md",
        "json" => ".json",
        "toml" | "yaml" | "yml" => ".cfg",
        "html" | "css" => ".web",
        _ => "[-]",
    }
}
