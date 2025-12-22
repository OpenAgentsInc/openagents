//! Diff view rendering with inline comments support

use maud::{html, Markup};
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
                    });
                }
            }
        }
    }

    comments
}

/// Render diff with inline comments
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
