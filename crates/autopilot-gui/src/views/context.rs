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
                h1 class="text-2xl font-bold text-gray-900 dark:text-white" { "Context Inspector" }
                div class="flex items-center gap-2" {
                    (token_usage_badge(&info.token_usage))
                    button
                        class="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700"
                        onclick="compactConversation()"
                    {
                        "Compact Conversation"
                    }
                }
            }

            // Working Directory
            div class="bg-white dark:bg-gray-800 rounded-lg shadow p-4" {
                h2 class="text-lg font-semibold mb-2 text-gray-900 dark:text-white" { "Working Directory" }
                p class="text-sm text-gray-600 dark:text-gray-400 font-mono" { (info.cwd) }
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
            div class="bg-white dark:bg-gray-800 rounded-lg p-6 max-w-md" {
                h3 class="text-lg font-semibold mb-4 text-gray-900 dark:text-white" {
                    "Compact Conversation?"
                }
                p class="text-gray-600 dark:text-gray-400 mb-4" {
                    "This will remove older messages and tool results to free up context space. "
                    "The conversation will be summarized."
                }
                div class="flex gap-2 justify-end" {
                    button
                        class="px-4 py-2 border border-gray-300 rounded hover:bg-gray-50"
                        onclick="closeCompactDialog()"
                    {
                        "Cancel"
                    }
                    button
                        class="px-4 py-2 bg-red-600 text-white rounded hover:bg-red-700"
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
        div class="bg-white dark:bg-gray-800 rounded-lg shadow p-4" {
            h2 class="text-lg font-semibold mb-4 text-gray-900 dark:text-white flex items-center gap-2" {
                svg class="w-5 h-5" fill="currentColor" viewBox="0 0 20 20" {
                    path d="M10 2a8 8 0 100 16 8 8 0 000-16zM9 5a1 1 0 112 0v4a1 1 0 11-2 0V5zm1 9a1 1 0 100-2 1 1 0 000 2z" {}
                }
                "Git Status"
            }

            @if let Some(git) = status {
                div class="space-y-3" {
                    // Branch info
                    div class="flex items-center gap-2" {
                        span class="text-sm font-medium text-gray-600 dark:text-gray-400" { "Branch:" }
                        code class="text-sm bg-gray-100 dark:bg-gray-700 px-2 py-1 rounded" {
                            (git.branch)
                        }
                        @if git.ahead > 0 {
                            span class="text-xs text-green-600" { "↑" (git.ahead) }
                        }
                        @if git.behind > 0 {
                            span class="text-xs text-red-600" { "↓" (git.behind) }
                        }
                    }

                    // Modified files
                    @if !git.modified_files.is_empty() {
                        div {
                            span class="text-sm font-medium text-yellow-600" {
                                "Modified (" (git.modified_files.len()) ")"
                            }
                            ul class="mt-1 space-y-1" {
                                @for file in git.modified_files.iter().take(5) {
                                    li class="text-sm text-gray-600 dark:text-gray-400 font-mono truncate" {
                                        (file)
                                    }
                                }
                            }
                        }
                    }

                    // Added files
                    @if !git.added_files.is_empty() {
                        div {
                            span class="text-sm font-medium text-green-600" {
                                "Added (" (git.added_files.len()) ")"
                            }
                            ul class="mt-1 space-y-1" {
                                @for file in git.added_files.iter().take(5) {
                                    li class="text-sm text-gray-600 dark:text-gray-400 font-mono truncate" {
                                        (file)
                                    }
                                }
                            }
                        }
                    }

                    // Deleted files
                    @if !git.deleted_files.is_empty() {
                        div {
                            span class="text-sm font-medium text-red-600" {
                                "Deleted (" (git.deleted_files.len()) ")"
                            }
                            ul class="mt-1 space-y-1" {
                                @for file in git.deleted_files.iter().take(5) {
                                    li class="text-sm text-gray-600 dark:text-gray-400 font-mono truncate" {
                                        (file)
                                    }
                                }
                            }
                        }
                    }

                    // Recent commits
                    @if !git.commits.is_empty() {
                        div class="mt-4" {
                            h3 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2" {
                                "Recent Commits"
                            }
                            ul class="space-y-2" {
                                @for commit in git.commits.iter() {
                                    li class="text-xs" {
                                        div class="flex items-start gap-2" {
                                            code class="text-blue-600 dark:text-blue-400" {
                                                (commit.hash[..7].to_string())
                                            }
                                            span class="text-gray-600 dark:text-gray-400 flex-1" {
                                                (commit.message)
                                            }
                                        }
                                        div class="text-gray-500 ml-12" {
                                            (commit.author) " • " (commit.timestamp)
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            } @else {
                p class="text-sm text-gray-500 dark:text-gray-400" {
                    "Not a git repository"
                }
            }
        }
    }
}

/// Token usage panel with gauge
fn token_usage_panel(usage: &TokenUsage) -> Markup {
    let color_class = if usage.percent >= 80.0 {
        "text-red-600"
    } else if usage.percent >= 60.0 {
        "text-yellow-600"
    } else {
        "text-green-600"
    };

    html! {
        div class="bg-white dark:bg-gray-800 rounded-lg shadow p-4" {
            h2 class="text-lg font-semibold mb-4 text-gray-900 dark:text-white" { "Token Usage" }

            // Gauge
            div class="mb-4" {
                div class="flex justify-between mb-2" {
                    span class="text-sm font-medium text-gray-700 dark:text-gray-300" {
                        (format!("{} / {} tokens", usage.used, usage.max))
                    }
                    span class={"text-sm font-bold " (color_class)} {
                        (format!("{:.1}%", usage.percent))
                    }
                }
                div class="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-4" {
                    div
                        class={"h-4 rounded-full transition-all " @if usage.percent >= 80.0 {
                            "bg-red-600"
                        } @else if usage.percent >= 60.0 {
                            "bg-yellow-500"
                        } @else {
                            "bg-green-500"
                        }}
                        style=(format!("width: {}%", usage.percent.min(100.0)))
                    {}
                }
            }

            // Warning thresholds
            @if usage.percent >= 80.0 {
                div class="p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded mb-4" {
                    p class="text-sm text-red-800 dark:text-red-300" {
                        "⚠️ Critical: Context nearly full. Consider compacting conversation."
                    }
                }
            } @else if usage.percent >= 60.0 {
                div class="p-3 bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800 rounded mb-4" {
                    p class="text-sm text-yellow-800 dark:text-yellow-300" {
                        "⚠️ Warning: Approaching context limit."
                    }
                }
            }

            // Token breakdown
            div class="space-y-2" {
                @for item in &usage.breakdown {
                    div class="flex justify-between text-sm" {
                        span class="text-gray-600 dark:text-gray-400" { (item.source) }
                        span class="text-gray-900 dark:text-white font-mono" {
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
        ("bg-red-100 dark:bg-red-900/20", "text-red-700 dark:text-red-300")
    } else if usage.percent >= 60.0 {
        ("bg-yellow-100 dark:bg-yellow-900/20", "text-yellow-700 dark:text-yellow-300")
    } else {
        ("bg-green-100 dark:bg-green-900/20", "text-green-700 dark:text-green-300")
    };

    html! {
        div class={(bg_class) " px-3 py-1 rounded-full"} {
            span class={"text-sm font-medium " (text_class)} {
                (format!("{:.0}% tokens used", usage.percent))
            }
        }
    }
}

/// CLAUDE.md panel
fn claude_md_panel(content: Option<String>) -> Markup {
    html! {
        div class="bg-white dark:bg-gray-800 rounded-lg shadow p-4" {
            h2 class="text-lg font-semibold mb-4 text-gray-900 dark:text-white" { "CLAUDE.md" }

            @if let Some(md) = content {
                div class="prose prose-sm dark:prose-invert max-w-none" {
                    div class="bg-gray-50 dark:bg-gray-900 p-4 rounded font-mono text-sm overflow-x-auto" {
                        pre { (md) }
                    }
                }
            } @else {
                p class="text-sm text-gray-500 dark:text-gray-400 italic" {
                    "No CLAUDE.md file found in working directory"
                }
            }
        }
    }
}

/// Directory tree panel
fn directory_tree_panel(root: &FileEntry) -> Markup {
    html! {
        div class="bg-white dark:bg-gray-800 rounded-lg shadow p-4" {
            h2 class="text-lg font-semibold mb-4 text-gray-900 dark:text-white" { "Directory Structure" }

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
            div class="flex items-center gap-2 py-1 hover:bg-gray-100 dark:hover:bg-gray-700 rounded px-2" {
                span class="text-gray-500" { (indent) }

                @if entry.is_dir {
                    button
                        class="text-blue-600 dark:text-blue-400 hover:underline cursor-pointer"
                        onclick=(format!("toggleDirectory('{}')", entry.path.replace("'", "\\'")))
                    {
                        "📁 " (entry.name)
                    }
                } @else {
                    span class="text-gray-700 dark:text-gray-300" {
                        (file_icon(&entry.name)) " " (entry.name)
                    }
                    span class="text-gray-500 text-xs ml-auto" {
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
        div class="bg-white dark:bg-gray-800 rounded-lg shadow p-4" {
            h2 class="text-lg font-semibold mb-4 text-gray-900 dark:text-white" {
                "Context Size Breakdown"
            }

            div class="overflow-x-auto" {
                table class="min-w-full divide-y divide-gray-200 dark:divide-gray-700" {
                    thead class="bg-gray-50 dark:bg-gray-900" {
                        tr {
                            th class="px-4 py-2 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider" {
                                "Source"
                            }
                            th class="px-4 py-2 text-right text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider" {
                                "Tokens"
                            }
                            th class="px-4 py-2 text-right text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider" {
                                "Bytes"
                            }
                            th class="px-4 py-2 text-right text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider" {
                                "% of Total"
                            }
                        }
                    }
                    tbody class="bg-white dark:bg-gray-800 divide-y divide-gray-200 dark:divide-gray-700" {
                        @for item in &usage.breakdown {
                            tr class="hover:bg-gray-50 dark:hover:bg-gray-700" {
                                td class="px-4 py-2 text-sm text-gray-900 dark:text-white" {
                                    (item.source)
                                }
                                td class="px-4 py-2 text-sm text-gray-600 dark:text-gray-400 text-right font-mono" {
                                    (format!("{}", item.tokens))
                                }
                                td class="px-4 py-2 text-sm text-gray-600 dark:text-gray-400 text-right font-mono" {
                                    (format_bytes(item.bytes as u64))
                                }
                                td class="px-4 py-2 text-sm text-gray-600 dark:text-gray-400 text-right" {
                                    (format!("{:.1}%", (item.tokens as f64 / usage.used as f64) * 100.0))
                                }
                            }
                        }
                        // Total row
                        tr class="bg-gray-100 dark:bg-gray-900 font-semibold" {
                            td class="px-4 py-2 text-sm text-gray-900 dark:text-white" {
                                "Total"
                            }
                            td class="px-4 py-2 text-sm text-gray-900 dark:text-white text-right font-mono" {
                                (format!("{}", usage.used))
                            }
                            td class="px-4 py-2 text-sm text-gray-900 dark:text-white text-right font-mono" {
                                (format_bytes(usage.breakdown.iter().map(|b| b.bytes as u64).sum()))
                            }
                            td class="px-4 py-2 text-sm text-gray-900 dark:text-white text-right" {
                                "100%"
                            }
                        }
                    }
                }
            }
        }
    }
}
