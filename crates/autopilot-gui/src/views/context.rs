//! Context inspector view for autopilot GUI
//!
//! Shows what the agent can see: git status, directory structure,
//! CLAUDE.md content, token usage, and context size breakdown.

use maud::{html, Markup, PreEscaped};
use serde::{Deserialize, Serialize};

/// Git status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitStatus {
    pub branch: String,
    pub modified_files: Vec<String>,
    pub added_files: Vec<String>,
    pub deleted_files: Vec<String>,
    pub commits: Vec<GitCommit>,
    pub ahead: usize,
    pub behind: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitCommit {
    pub hash: String,
    pub message: String,
    pub author: String,
    pub timestamp: String,
}

/// Directory entry in file tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub children: Vec<FileEntry>,
}

/// Token usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub used: usize,
    pub max: usize,
    pub percent: f64,
    pub breakdown: Vec<TokenBreakdown>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBreakdown {
    pub source: String,
    pub tokens: usize,
    pub bytes: usize,
}

/// Complete context information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextInfo {
    pub git_status: Option<GitStatus>,
    pub claude_md: Option<String>,
    pub directory_tree: FileEntry,
    pub token_usage: TokenUsage,
    pub cwd: String,
}

/// Render the context inspector view
pub fn context_inspector(info: ContextInfo) -> Markup {
    html! {
        div class="p-6 space-y-6" {
            // Header
            div class="flex items-center justify-between" {
                h1 class="text-2xl font-bold text-foreground" { "Context Inspector" }
                div class="flex items-center gap-2" {
                    (token_usage_badge(&info.token_usage))
                    button
                        class="px-4 py-2 bg-primary text-primary-foreground hover:bg-primary/90"
                        onclick="compactConversation()"
                    {
                        "Compact Conversation"
                    }
                }
            }

            // Working Directory
            div class="bg-card shadow p-4" {
                h2 class="text-lg font-semibold mb-2 text-foreground" { "Working Directory" }
                p class="text-sm text-muted-foreground font-mono" { (info.cwd) }
            }

            div class="grid grid-cols-1 lg:grid-cols-2 gap-6" {
                // Git Status Panel
                div {
                    (git_status_panel(info.git_status))
                }

                // Token Usage Panel
                div {
                    (token_usage_panel(&info.token_usage))
                }
            }

            // CLAUDE.md Display
            div {
                (claude_md_panel(info.claude_md))
            }

            // Directory Structure
            div {
                (directory_tree_panel(&info.directory_tree))
            }

            // Context Size Breakdown
            div {
                (context_breakdown_panel(&info.token_usage))
            }
        }

        // Compact confirmation dialog (hidden by default)
        div
            id="compact-dialog"
            class="hidden fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
        {
            div class="bg-card p-6 max-w-md" {
                h3 class="text-lg font-semibold mb-4 text-foreground" {
                    "Compact Conversation?"
                }
                p class="text-muted-foreground mb-4" {
                    "This will remove older messages and tool results to free up context space. "
                    "The conversation will be summarized."
                }
                div class="flex gap-2 justify-end" {
                    button
                        class="px-4 py-2 border border-border hover:bg-muted"
                        onclick="closeCompactDialog()"
                    {
                        "Cancel"
                    }
                    button
                        class="px-4 py-2 bg-destructive text-destructive-foreground hover:bg-destructive/90"
                        onclick="executeCompact()"
                    {
                        "Compact"
                    }
                }
            }
        }

        script {
            (PreEscaped(r#"
function compactConversation() {
    document.getElementById('compact-dialog').classList.remove('hidden');
}

function closeCompactDialog() {
    document.getElementById('compact-dialog').classList.add('hidden');
}

function executeCompact() {
    // Send compact request via WebSocket
    if (window.ws && window.ws.readyState === WebSocket.OPEN) {
        window.ws.send(JSON.stringify({ type: 'compact' }));
    }
    closeCompactDialog();
}

function toggleDirectory(path) {
    const element = document.getElementById('dir-' + path);
    if (element) {
        element.classList.toggle('hidden');
    }
}
            "#))
        }
    }
}

/// Git status panel
fn git_status_panel(status: Option<GitStatus>) -> Markup {
    html! {
        div class="bg-card shadow p-4" {
            h2 class="text-lg font-semibold mb-4 text-foreground flex items-center gap-2" {
                svg class="w-5 h-5" fill="currentColor" viewBox="0 0 20 20" {
                    path d="M10 2a8 8 0 100 16 8 8 0 000-16zM9 5a1 1 0 112 0v4a1 1 0 11-2 0V5zm1 9a1 1 0 100-2 1 1 0 000 2z" {}
                }
                "Git Status"
            }

            @if let Some(git) = status {
                div class="space-y-3" {
                    // Branch info
                    div class="flex items-center gap-2" {
                        span class="text-sm font-medium text-muted-foreground" { "Branch:" }
                        code class="text-sm bg-muted px-2 py-1" {
                            (git.branch)
                        }
                        @if git.ahead > 0 {
                            span class="text-xs text-accent" { "↑" (git.ahead) }
                        }
                        @if git.behind > 0 {
                            span class="text-xs text-destructive" { "↓" (git.behind) }
                        }
                    }

                    // Modified files
                    @if !git.modified_files.is_empty() {
                        div {
                            span class="text-sm font-medium text-accent" {
                                "Modified (" (git.modified_files.len()) ")"
                            }
                            ul class="mt-1 space-y-1" {
                                @for file in git.modified_files.iter().take(5) {
                                    li class="text-sm text-muted-foreground font-mono truncate" {
                                        (file)
                                    }
                                }
                            }
                        }
                    }

                    // Added files
                    @if !git.added_files.is_empty() {
                        div {
                            span class="text-sm font-medium text-accent" {
                                "Added (" (git.added_files.len()) ")"
                            }
                            ul class="mt-1 space-y-1" {
                                @for file in git.added_files.iter().take(5) {
                                    li class="text-sm text-muted-foreground font-mono truncate" {
                                        (file)
                                    }
                                }
                            }
                        }
                    }

                    // Deleted files
                    @if !git.deleted_files.is_empty() {
                        div {
                            span class="text-sm font-medium text-destructive" {
                                "Deleted (" (git.deleted_files.len()) ")"
                            }
                            ul class="mt-1 space-y-1" {
                                @for file in git.deleted_files.iter().take(5) {
                                    li class="text-sm text-muted-foreground font-mono truncate" {
                                        (file)
                                    }
                                }
                            }
                        }
                    }

                    // Recent commits
                    @if !git.commits.is_empty() {
                        div class="mt-4" {
                            h3 class="text-sm font-medium text-foreground mb-2" {
                                "Recent Commits"
                            }
                            ul class="space-y-2" {
                                @for commit in git.commits.iter() {
                                    li class="text-xs" {
                                        div class="flex items-start gap-2" {
                                            code class="text-primary" {
                                                (commit.hash[..7].to_string())
                                            }
                                            span class="text-muted-foreground flex-1" {
                                                (commit.message)
                                            }
                                        }
                                        div class="text-muted-foreground ml-12" {
                                            (commit.author) " • " (commit.timestamp)
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            } @else {
                p class="text-sm text-muted-foreground" {
                    "Not a git repository"
                }
            }
        }
    }
}

/// Token usage panel with gauge
fn token_usage_panel(usage: &TokenUsage) -> Markup {
    let color_class = if usage.percent >= 80.0 {
        "text-destructive"
    } else {
        "text-accent"
    };

    html! {
        div class="bg-card shadow p-4" {
            h2 class="text-lg font-semibold mb-4 text-foreground" { "Token Usage" }

            // Gauge
            div class="mb-4" {
                div class="flex justify-between mb-2" {
                    span class="text-sm font-medium text-foreground" {
                        (format!("{} / {} tokens", usage.used, usage.max))
                    }
                    span class={"text-sm font-bold " (color_class)} {
                        (format!("{:.1}%", usage.percent))
                    }
                }
                div class="w-full bg-muted h-4" {
                    div
                        class={"h-4 transition-all " @if usage.percent >= 80.0 {
                            "bg-destructive"
                        } @else {
                            "bg-accent"
                        }}
                        style=(format!("width: {}%", usage.percent.min(100.0)))
                    {}
                }
            }

            // Warning thresholds
            @if usage.percent >= 80.0 {
                div class="p-3 bg-destructive/10 border border-destructive mb-4" {
                    p class="text-sm text-destructive" {
                        "⚠️ Critical: Context nearly full. Consider compacting conversation."
                    }
                }
            } @else if usage.percent >= 60.0 {
                div class="p-3 bg-accent/10 border border-accent mb-4" {
                    p class="text-sm text-accent" {
                        "⚠️ Warning: Approaching context limit."
                    }
                }
            }

            // Token breakdown
            div class="space-y-2" {
                @for item in &usage.breakdown {
                    div class="flex justify-between text-sm" {
                        span class="text-muted-foreground" { (item.source) }
                        span class="text-foreground font-mono" {
                            (format!("{} tokens", item.tokens))
                        }
                    }
                }
            }
        }
    }
}

/// Token usage badge for header
fn token_usage_badge(usage: &TokenUsage) -> Markup {
    let (bg_class, text_class) = if usage.percent >= 80.0 {
        ("bg-destructive/10", "text-destructive")
    } else {
        ("bg-accent/10", "text-accent")
    };

    html! {
        div class={(bg_class) " px-3 py-1"} {
            span class={"text-sm font-medium " (text_class)} {
                (format!("{:.0}% tokens used", usage.percent))
            }
        }
    }
}

/// CLAUDE.md panel
fn claude_md_panel(content: Option<String>) -> Markup {
    html! {
        div class="bg-card shadow p-4" {
            h2 class="text-lg font-semibold mb-4 text-foreground" { "CLAUDE.md" }

            @if let Some(md) = content {
                div class="prose prose-sm dark:prose-invert max-w-none" {
                    div class="bg-muted p-4 font-mono text-sm overflow-x-auto" {
                        pre { (md) }
                    }
                }
            } @else {
                p class="text-sm text-muted-foreground italic" {
                    "No CLAUDE.md file found in working directory"
                }
            }
        }
    }
}

/// Directory tree panel
fn directory_tree_panel(root: &FileEntry) -> Markup {
    html! {
        div class="bg-card shadow p-4" {
            h2 class="text-lg font-semibold mb-4 text-foreground" { "Directory Structure" }

            div class="font-mono text-sm" {
                (render_file_tree(root, 0))
            }
        }
    }
}

/// Render file tree recursively
fn render_file_tree(entry: &FileEntry, depth: usize) -> Markup {
    let indent = "  ".repeat(depth);

    html! {
        div {
            div class="flex items-center gap-2 py-1 hover:bg-muted px-2" {
                span class="text-muted-foreground" { (indent) }

                @if entry.is_dir {
                    button
                        class="text-primary hover:underline cursor-pointer"
                        onclick=(format!("toggleDirectory('{}')", entry.path.replace("'", "\\'")))
                    {
                        "📁 " (entry.name)
                    }
                } @else {
                    span class="text-foreground" {
                        (file_icon(&entry.name)) " " (entry.name)
                    }
                    span class="text-muted-foreground text-xs ml-auto" {
                        (format_bytes(entry.size))
                    }
                }
            }

            @if entry.is_dir && !entry.children.is_empty() {
                div id=(format!("dir-{}", entry.path)) class="hidden" {
                    @for child in &entry.children {
                        (render_file_tree(child, depth + 1))
                    }
                }
            }
        }
    }
}

/// Get file icon based on extension
fn file_icon(name: &str) -> &'static str {
    if name.ends_with(".rs") {
        "🦀"
    } else if name.ends_with(".ts") || name.ends_with(".tsx") {
        "📘"
    } else if name.ends_with(".js") || name.ends_with(".jsx") {
        "📜"
    } else if name.ends_with(".md") {
        "📝"
    } else if name.ends_with(".json") {
        "📋"
    } else if name.ends_with(".toml") {
        "⚙️"
    } else {
        "📄"
    }
}

/// Format bytes as human readable
fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Context breakdown panel
fn context_breakdown_panel(usage: &TokenUsage) -> Markup {
    html! {
        div class="bg-card shadow p-4" {
            h2 class="text-lg font-semibold mb-4 text-foreground" {
                "Context Size Breakdown"
            }

            div class="overflow-x-auto" {
                table class="min-w-full divide-y divide-border" {
                    thead class="bg-muted" {
                        tr {
                            th class="px-4 py-2 text-left text-xs font-medium text-muted-foreground uppercase tracking-wider" {
                                "Source"
                            }
                            th class="px-4 py-2 text-right text-xs font-medium text-muted-foreground uppercase tracking-wider" {
                                "Tokens"
                            }
                            th class="px-4 py-2 text-right text-xs font-medium text-muted-foreground uppercase tracking-wider" {
                                "Bytes"
                            }
                            th class="px-4 py-2 text-right text-xs font-medium text-muted-foreground uppercase tracking-wider" {
                                "% of Total"
                            }
                        }
                    }
                    tbody class="bg-card divide-y divide-border" {
                        @for item in &usage.breakdown {
                            tr class="hover:bg-muted" {
                                td class="px-4 py-2 text-sm text-foreground" {
                                    (item.source)
                                }
                                td class="px-4 py-2 text-sm text-muted-foreground text-right font-mono" {
                                    (format!("{}", item.tokens))
                                }
                                td class="px-4 py-2 text-sm text-muted-foreground text-right font-mono" {
                                    (format_bytes(item.bytes as u64))
                                }
                                td class="px-4 py-2 text-sm text-muted-foreground text-right" {
                                    (format!("{:.1}%", (item.tokens as f64 / usage.used as f64) * 100.0))
                                }
                            }
                        }
                        // Total row
                        tr class="bg-muted font-semibold" {
                            td class="px-4 py-2 text-sm text-foreground" {
                                "Total"
                            }
                            td class="px-4 py-2 text-sm text-foreground text-right font-mono" {
                                (format!("{}", usage.used))
                            }
                            td class="px-4 py-2 text-sm text-foreground text-right font-mono" {
                                (format_bytes(usage.breakdown.iter().map(|b| b.bytes as u64).sum()))
                            }
                            td class="px-4 py-2 text-sm text-foreground text-right" {
                                "100%"
                            }
                        }
                    }
                }
            }
        }
    }
}
