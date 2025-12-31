//! Diff view rendering with inline comments support

use maud::{Markup, PreEscaped, html};
use nostr::Event;
use std::collections::HashMap;

/// A line in a diff with optional inline comments
#[derive(Debug, Clone)]
pub struct DiffLine {
    pub line_number: Option<usize>,
    pub content: String,
    pub line_type: DiffLineType,
    pub file_path: String,
}

/// Type of line in a diff
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineType {
    Addition,
    Deletion,
    Context,
    Header,
}

/// Comment attached to a specific line in a diff
#[derive(Debug, Clone)]
pub struct InlineComment {
    pub event: Event,
    pub file_path: String,
    pub line_number: usize,
    pub position: LinePosition,
    pub author_pubkey: String,
    pub layer_info: Option<(String, String)>, // (current_layer, total_layers)
}

/// Position of comment relative to line
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinePosition {
    Before,
    After,
}

/// Parse a unified diff into lines with metadata
pub fn parse_diff_lines(diff_text: &str) -> Vec<DiffLine> {
    let mut lines = Vec::new();
    let mut current_file: Option<String> = None;
    let mut line_number = 0;

    for line in diff_text.lines() {
        if line.starts_with("diff --git") {
            // Extract file path from: diff --git a/path/to/file b/path/to/file
            if let Some(path) = line.split_whitespace().nth(2) {
                current_file = Some(path.trim_start_matches("a/").to_string());
            }
            lines.push(DiffLine {
                line_number: None,
                content: line.to_string(),
                line_type: DiffLineType::Header,
                file_path: current_file.clone().unwrap_or_default(),
            });
            line_number = 0;
        } else if line.starts_with("@@") {
            // Hunk header
            lines.push(DiffLine {
                line_number: None,
                content: line.to_string(),
                line_type: DiffLineType::Header,
                file_path: current_file.clone().unwrap_or_default(),
            });
        } else if line.starts_with('+') && !line.starts_with("+++") {
            line_number += 1;
            lines.push(DiffLine {
                line_number: Some(line_number),
                content: line.to_string(),
                line_type: DiffLineType::Addition,
                file_path: current_file.clone().unwrap_or_default(),
            });
        } else if line.starts_with('-') && !line.starts_with("---") {
            line_number += 1;
            lines.push(DiffLine {
                line_number: Some(line_number),
                content: line.to_string(),
                line_type: DiffLineType::Deletion,
                file_path: current_file.clone().unwrap_or_default(),
            });
        } else if !line.starts_with("+++") && !line.starts_with("---") {
            // Context line
            line_number += 1;
            lines.push(DiffLine {
                line_number: Some(line_number),
                content: line.to_string(),
                line_type: DiffLineType::Context,
                file_path: current_file.clone().unwrap_or_default(),
            });
        }
    }

    lines
}

/// Extract inline comments from NIP-22 comment events with line tags
pub fn extract_inline_comments(comment_events: &[Event]) -> Vec<InlineComment> {
    let mut comments = Vec::new();

    for event in comment_events {
        // Extract layer info from event tags if present
        let layer_info = event
            .tags
            .iter()
            .find(|tag| tag.len() >= 3 && tag[0] == "layer")
            .map(|tag| (tag[1].clone(), tag[2].clone()));

        // Look for ["line", "file_path", "line_number", "position"] tag
        for tag in &event.tags {
            if tag.len() >= 4 && tag[0] == "line" {
                let file_path = tag[1].clone();
                if let Ok(line_number) = tag[2].parse::<usize>() {
                    let position = match tag[3].as_str() {
                        "before" => LinePosition::Before,
                        "after" => LinePosition::After,
                        _ => LinePosition::After,
                    };

                    comments.push(InlineComment {
                        event: event.clone(),
                        file_path,
                        line_number,
                        position,
                        author_pubkey: event.pubkey.clone(),
                        layer_info: layer_info.clone(),
                    });
                }
            }
        }
    }

    comments
}

/// Configuration for diff rendering optimization
#[derive(Debug, Clone)]
pub struct DiffRenderConfig {
    /// Maximum number of unchanged context lines before collapsing
    pub collapse_threshold: usize,
    /// Number of context lines to show around changes when collapsed
    pub context_lines: usize,
}

impl Default for DiffRenderConfig {
    fn default() -> Self {
        Self {
            collapse_threshold: 10,
            context_lines: 3,
        }
    }
}

/// A chunk of diff lines (either collapsed or expanded)
#[derive(Debug)]
enum DiffChunk {
    Expanded(Vec<DiffLine>),
    Collapsed {
        lines: Vec<DiffLine>,
        start_line: usize,
        end_line: usize,
    },
}

/// Group diff lines into chunks for optimization
fn group_into_chunks(lines: Vec<DiffLine>, config: &DiffRenderConfig) -> Vec<DiffChunk> {
    let mut chunks = Vec::new();
    let mut current_unchanged: Vec<DiffLine> = Vec::new();
    let mut current_changed: Vec<DiffLine> = Vec::new();

    for line in lines {
        match line.line_type {
            DiffLineType::Context if !matches!(line.line_number, None) => {
                // Context line - might be collapsible
                if !current_changed.is_empty() {
                    // Flush changed lines first
                    chunks.push(DiffChunk::Expanded(std::mem::take(&mut current_changed)));
                }
                current_unchanged.push(line);
            }
            _ => {
                // Changed line (addition, deletion, or header)
                if current_unchanged.len() > config.collapse_threshold {
                    // Collapse the unchanged section, keeping context around changes
                    let start = current_unchanged
                        .first()
                        .and_then(|l| l.line_number)
                        .unwrap_or(0);
                    let end = current_unchanged
                        .last()
                        .and_then(|l| l.line_number)
                        .unwrap_or(0);

                    // Keep first N context lines
                    let mut visible: Vec<DiffLine> = current_unchanged
                        .iter()
                        .take(config.context_lines)
                        .cloned()
                        .collect();

                    // Collapse middle section
                    let middle: Vec<DiffLine> = current_unchanged
                        .iter()
                        .skip(config.context_lines)
                        .take(current_unchanged.len() - 2 * config.context_lines)
                        .cloned()
                        .collect();

                    if !middle.is_empty() {
                        chunks.push(DiffChunk::Expanded(visible));
                        chunks.push(DiffChunk::Collapsed {
                            lines: middle.clone(),
                            start_line: start + config.context_lines,
                            end_line: end - config.context_lines,
                        });

                        // Keep last N context lines
                        visible = current_unchanged
                            .iter()
                            .skip(current_unchanged.len() - config.context_lines)
                            .cloned()
                            .collect();
                        chunks.push(DiffChunk::Expanded(visible));
                    } else {
                        chunks.push(DiffChunk::Expanded(current_unchanged.clone()));
                    }

                    current_unchanged.clear();
                } else if !current_unchanged.is_empty() {
                    // Not enough unchanged lines to collapse - show all
                    chunks.push(DiffChunk::Expanded(std::mem::take(&mut current_unchanged)));
                }

                current_changed.push(line);
            }
        }
    }

    // Flush remaining
    if !current_changed.is_empty() {
        chunks.push(DiffChunk::Expanded(current_changed));
    }
    if !current_unchanged.is_empty() {
        if current_unchanged.len() > config.collapse_threshold {
            let start = current_unchanged
                .first()
                .and_then(|l| l.line_number)
                .unwrap_or(0);
            let end = current_unchanged
                .last()
                .and_then(|l| l.line_number)
                .unwrap_or(0);
            chunks.push(DiffChunk::Collapsed {
                lines: current_unchanged.clone(),
                start_line: start,
                end_line: end,
            });
        } else {
            chunks.push(DiffChunk::Expanded(current_unchanged));
        }
    }

    chunks
}

/// Render diff with inline comments and optimizations for large files
pub fn render_diff_optimized(
    diff_text: &str,
    comments: &[InlineComment],
    pr_id: &str,
    repo_id: &str,
    config: DiffRenderConfig,
) -> Markup {
    let lines = parse_diff_lines(diff_text);
    let total_lines = lines.len();

    // Group into chunks for collapsing unchanged sections
    let chunks = group_into_chunks(lines, &config);

    // Group comments by file and line number
    let mut comment_map: HashMap<(String, usize), Vec<&InlineComment>> = HashMap::new();
    for comment in comments {
        comment_map
            .entry((comment.file_path.clone(), comment.line_number))
            .or_default()
            .push(comment);
    }

    let chunk_id_base = format!("diff-{}", pr_id);

    html! {
        div.diff-container data-total-lines=(total_lines) {
            @for (chunk_idx, chunk) in chunks.iter().enumerate() {
                @match chunk {
                    DiffChunk::Expanded(lines) => {
                        @for (idx, line) in lines.iter().enumerate() {
                            (render_diff_line(line, idx + chunk_idx * 100, &comment_map, pr_id, repo_id))
                        }
                    }
                    DiffChunk::Collapsed { lines, start_line, end_line } => {
                        details.collapsed-chunk {
                            summary.expand-toggle {
                                span.expand-icon { "▶" }
                                " "
                                span.expand-text {
                                    "⋯ " (lines.len()) " unchanged lines (" (start_line) "-" (end_line) ") ⋯"
                                }
                            }
                            div.collapsed-content id={(chunk_id_base) "-chunk-" (chunk_idx)} {
                                @for (idx, line) in lines.iter().enumerate() {
                                    (render_diff_line(line, idx + chunk_idx * 100 + 50000, &comment_map, pr_id, repo_id))
                                }
                            }
                        }
                    }
                }
            }
        }

        (render_diff_styles())

        // Add expand/collapse JavaScript
        script {
            (PreEscaped(r#"
document.addEventListener('DOMContentLoaded', function() {
    // Handle expand/collapse with smooth animation
    document.querySelectorAll('.collapsed-chunk').forEach(function(details) {
        details.addEventListener('toggle', function() {
            if (this.open) {
                const icon = this.querySelector('.expand-icon');
                if (icon) icon.textContent = '▼';
            } else {
                const icon = this.querySelector('.expand-icon');
                if (icon) icon.textContent = '▶';
            }
        });
    });

    // Keyboard shortcut: 'e' to expand all, 'c' to collapse all
    document.addEventListener('keydown', function(e) {
        if (e.key === 'e' && !e.ctrlKey && !e.metaKey) {
            document.querySelectorAll('.collapsed-chunk').forEach(d => d.open = true);
        } else if (e.key === 'c' && !e.ctrlKey && !e.metaKey) {
            document.querySelectorAll('.collapsed-chunk').forEach(d => d.open = false);
        }
    });
});
            "#))
        }
    }
}

/// Render a single diff line
fn render_diff_line(
    line: &DiffLine,
    idx: usize,
    comment_map: &HashMap<(String, usize), Vec<&InlineComment>>,
    pr_id: &str,
    repo_id: &str,
) -> Markup {
    let line_class = match line.line_type {
        DiffLineType::Addition => "diff-line-add",
        DiffLineType::Deletion => "diff-line-del",
        DiffLineType::Context => "diff-line-context",
        DiffLineType::Header => "diff-line-header",
    };

    html! {
        div class={"diff-line " (line_class)} data-line-idx=(idx) {
            @if let Some(ln) = line.line_number {
                span.line-number { (ln) }

                // Add comment button for reviewable lines
                @if line.line_type != DiffLineType::Header {
                    button.comment-btn
                        data-file=(line.file_path)
                        data-line=(ln)
                        hx-get={"/repo/" (repo_id) "/pulls/" (pr_id) "/comment-form?file=" (line.file_path) "&line=" (ln)}
                        hx-target={"#comment-form-" (idx)}
                        hx-swap="outerHTML"
                        title="Add comment" {
                        "💬"
                    }
                }
            }

            pre.line-content { code { (line.content) } }

            // Show inline comments for this line
            @if let Some(line_num) = line.line_number {
                @if let Some(line_comments) = comment_map.get(&(line.file_path.clone(), line_num)) {
                    div.inline-comments {
                        @for comment in line_comments {
                            div.inline-comment {
                                div.comment-header {
                                    span.comment-author { (format_pubkey(&comment.author_pubkey)) }
                                    @if let Some((current, total)) = &comment.layer_info {
                                        span.comment-layer style="margin-left: 0.5rem; padding: 2px 6px; background: #6366f1; color: white; font-size: 0.75rem;" {
                                            "Layer " (current) "/" (total)
                                        }
                                    }
                                    span.comment-position {
                                        @if comment.position == LinePosition::Before { "↑" } @else { "↓" }
                                    }
                                }
                                div.comment-body { (comment.event.content) }
                            }
                        }
                    }
                }
            }

            // Placeholder for comment form
            div id={"comment-form-" (idx)} {}
        }
    }
}

/// Render diff styles
fn render_diff_styles() -> Markup {
    html! {
        style {
            r#"
.diff-container {
    font-family: 'Monaco', 'Menlo', 'Consolas', monospace;
    font-size: 0.875rem;
    background: #1a1a1a;
    border: 1px solid #333;
}

.collapsed-chunk {
    border: none;
    margin: 0;
}

.collapsed-chunk summary {
    list-style: none;
    cursor: pointer;
    padding: 0.5rem 1rem;
    background: #2a2a2a;
    border-top: 1px solid #3a3a3a;
    border-bottom: 1px solid #3a3a3a;
    color: #9ca3af;
    font-size: 0.875rem;
    user-select: none;
}

.collapsed-chunk summary:hover {
    background: #333;
}

.collapsed-chunk summary::-webkit-details-marker {
    display: none;
}

.expand-icon {
    display: inline-block;
    transition: transform 0.2s;
    color: #6366f1;
}

.collapsed-chunk[open] .expand-icon {
    transform: rotate(90deg);
}

.collapsed-content {
    animation: slideDown 0.2s ease-out;
}

@keyframes slideDown {
    from {
        opacity: 0;
        max-height: 0;
    }
    to {
        opacity: 1;
        max-height: 5000px;
    }
}

.diff-line {
    display: flex;
    align-items: flex-start;
    position: relative;
    padding: 0.25rem 0;
    border-bottom: 1px solid #2a2a2a;
}

.diff-line:hover {
    background: #252525;
}

.diff-line-add {
    background: rgba(34, 197, 94, 0.1);
}

.diff-line-del {
    background: rgba(239, 68, 68, 0.1);
}

.diff-line-header {
    background: #2a2a2a;
    font-weight: 600;
    padding: 0.5rem;
}

.line-number {
    display: inline-block;
    width: 4rem;
    text-align: right;
    padding-right: 0.75rem;
    color: #666;
    user-select: none;
}

.comment-btn {
    display: none;
    margin-left: 0.5rem;
    padding: 0.125rem 0.375rem;
    background: #374151;
    border: 1px solid #4b5563;
    color: #9ca3af;
    cursor: pointer;
    font-size: 0.75rem;
}

.diff-line:hover .comment-btn {
    display: inline-block;
}

.comment-btn:hover {
    background: #4b5563;
    color: #e5e7eb;
}

.line-content {
    flex: 1;
    margin: 0;
    padding: 0 0.5rem;
    white-space: pre-wrap;
    word-break: break-all;
}

.line-content code {
    color: #e5e7eb;
}

.inline-comments {
    margin-left: 5rem;
    margin-top: 0.5rem;
    border-left: 3px solid #6366f1;
    padding-left: 0.75rem;
}

.inline-comment {
    background: #1e293b;
    padding: 0.75rem;
    margin-bottom: 0.5rem;
    border-left: 3px solid #6366f1;
}

.comment-header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 0.5rem;
    font-size: 0.875rem;
}

.comment-author {
    font-weight: 600;
    color: #60a5fa;
}

.comment-position {
    color: #9ca3af;
}

.comment-body {
    color: #e5e7eb;
    line-height: 1.5;
}
            "#
        }
    }
}

/// Render diff with inline comments (legacy, non-optimized)
#[allow(dead_code)]
pub fn render_diff_with_comments(
    diff_text: &str,
    comments: &[InlineComment],
    pr_id: &str,
    repo_id: &str,
) -> Markup {
    let lines = parse_diff_lines(diff_text);

    // Group comments by file and line number
    let mut comment_map: HashMap<(String, usize), Vec<&InlineComment>> = HashMap::new();
    for comment in comments {
        comment_map
            .entry((comment.file_path.clone(), comment.line_number))
            .or_default()
            .push(comment);
    }

    html! {
        div.diff-container {
            @for (idx, line) in lines.iter().enumerate() {
                @let line_class = match line.line_type {
                    DiffLineType::Addition => "diff-line-add",
                    DiffLineType::Deletion => "diff-line-del",
                    DiffLineType::Context => "diff-line-context",
                    DiffLineType::Header => "diff-line-header",
                };

                div class={"diff-line " (line_class)} data-line-idx=(idx) {
                    @if let Some(ln) = line.line_number {
                        span.line-number { (ln) }

                        // Add comment button for reviewable lines
                        @if line.line_type != DiffLineType::Header {
                            button.comment-btn
                                data-file=(line.file_path)
                                data-line=(ln)
                                hx-get={"/repo/" (repo_id) "/pulls/" (pr_id) "/comment-form?file=" (line.file_path) "&line=" (ln)}
                                hx-target={"#comment-form-" (idx)}
                                hx-swap="outerHTML"
                                title="Add comment" {
                                "💬"
                            }
                        }
                    }

                    pre.line-content { code { (line.content) } }

                    // Show inline comments for this line
                    @if let Some(line_num) = line.line_number {
                        @if let Some(line_comments) = comment_map.get(&(line.file_path.clone(), line_num)) {
                            div.inline-comments {
                                @for comment in line_comments {
                                    div.inline-comment {
                                        div.comment-header {
                                            span.comment-author { (format_pubkey(&comment.author_pubkey)) }
                                            @if let Some((current, total)) = &comment.layer_info {
                                                span.comment-layer style="margin-left: 0.5rem; padding: 2px 6px; background: #6366f1; color: white; font-size: 0.75rem;" {
                                                    "Layer " (current) "/" (total)
                                                }
                                            }
                                            span.comment-position {
                                                @if comment.position == LinePosition::Before { "↑" } @else { "↓" }
                                            }
                                        }
                                        div.comment-body { (comment.event.content) }
                                    }
                                }
                            }
                        }
                    }

                    // Placeholder for comment form
                    div id={"comment-form-" (idx)} {}
                }
            }
        }

        style {
            r#"
.diff-container {
    font-family: 'Monaco', 'Menlo', 'Consolas', monospace;
    font-size: 0.875rem;
    background: #1a1a1a;
    border: 1px solid #333;
}

.diff-line {
    display: flex;
    align-items: flex-start;
    position: relative;
    padding: 0.25rem 0;
    border-bottom: 1px solid #2a2a2a;
}

.diff-line:hover {
    background: #252525;
}

.diff-line-add {
    background: rgba(34, 197, 94, 0.1);
}

.diff-line-del {
    background: rgba(239, 68, 68, 0.1);
}

.diff-line-header {
    background: #2a2a2a;
    font-weight: 600;
    padding: 0.5rem;
}

.line-number {
    display: inline-block;
    width: 3rem;
    text-align: right;
    color: #666;
    user-select: none;
    padding-right: 0.5rem;
    flex-shrink: 0;
}

.comment-btn {
    display: none;
    background: none;
    border: none;
    cursor: pointer;
    font-size: 0.875rem;
    padding: 0 0.5rem;
    color: #888;
}

.diff-line:hover .comment-btn {
    display: inline-block;
}

.comment-btn:hover {
    color: #4a9eff;
}

.line-content {
    flex: 1;
    margin: 0;
    white-space: pre-wrap;
    word-break: break-all;
}

.inline-comments {
    margin-left: 3.5rem;
    padding: 0.5rem;
    border-left: 3px solid #4a9eff;
    background: rgba(74, 158, 255, 0.1);
}

.inline-comment {
    margin-bottom: 0.5rem;
}

.inline-comment:last-child {
    margin-bottom: 0;
}

.comment-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.25rem;
    font-size: 0.75rem;
    color: #aaa;
}

.comment-author {
    font-weight: 600;
}

.comment-body {
    color: #ddd;
    font-size: 0.875rem;
}
            "#
        }
    }
}

/// Format pubkey for display
fn format_pubkey(pubkey: &str) -> String {
    if pubkey.len() > 16 {
        format!("{}...{}", &pubkey[..8], &pubkey[pubkey.len() - 8..])
    } else {
        pubkey.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_diff_lines() {
        let diff = r#"diff --git a/src/main.rs b/src/main.rs
@@ -1,3 +1,5 @@
 fn main() {
+    println!("Hello");
+    println!("World");
 }
-// old comment
"#;

        let lines = parse_diff_lines(diff);
        assert!(lines.len() > 0);

        // Check we have additions and deletions
        let has_addition = lines.iter().any(|l| l.line_type == DiffLineType::Addition);
        let has_deletion = lines.iter().any(|l| l.line_type == DiffLineType::Deletion);
        assert!(has_addition);
        assert!(has_deletion);
    }

    #[test]
    fn test_extract_inline_comments() {
        // This would need a proper Event struct to test
        let comments: Vec<Event> = vec![];
        let inline = extract_inline_comments(&comments);
        assert_eq!(inline.len(), 0);
    }
}
