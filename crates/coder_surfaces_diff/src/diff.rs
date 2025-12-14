//! Diff computation using the similar crate.

use similar::{ChangeTag, TextDiff};

/// The kind of change in a diff.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    /// Line was added.
    Added,
    /// Line was removed.
    Removed,
    /// Line is unchanged (context).
    Equal,
}

impl From<ChangeTag> for ChangeKind {
    fn from(tag: ChangeTag) -> Self {
        match tag {
            ChangeTag::Insert => ChangeKind::Added,
            ChangeTag::Delete => ChangeKind::Removed,
            ChangeTag::Equal => ChangeKind::Equal,
        }
    }
}

/// A single change in a diff.
#[derive(Clone, Debug)]
pub struct Change {
    /// The kind of change.
    pub kind: ChangeKind,
    /// The content of the line.
    pub content: String,
    /// Line number in the old file (None for additions).
    pub old_line: Option<usize>,
    /// Line number in the new file (None for deletions).
    pub new_line: Option<usize>,
}

impl Change {
    /// Create a new change.
    pub fn new(
        kind: ChangeKind,
        content: impl Into<String>,
        old_line: Option<usize>,
        new_line: Option<usize>,
    ) -> Self {
        Self {
            kind,
            content: content.into(),
            old_line,
            new_line,
        }
    }

    /// Check if this is an addition.
    pub fn is_added(&self) -> bool {
        self.kind == ChangeKind::Added
    }

    /// Check if this is a removal.
    pub fn is_removed(&self) -> bool {
        self.kind == ChangeKind::Removed
    }

    /// Check if this is unchanged context.
    pub fn is_equal(&self) -> bool {
        self.kind == ChangeKind::Equal
    }
}

/// A hunk of changes (contiguous group).
#[derive(Clone, Debug)]
pub struct Hunk {
    /// Starting line in old file.
    pub old_start: usize,
    /// Number of lines from old file.
    pub old_count: usize,
    /// Starting line in new file.
    pub new_start: usize,
    /// Number of lines from new file.
    pub new_count: usize,
    /// Changes in this hunk.
    pub changes: Vec<Change>,
}

impl Hunk {
    /// Create a new hunk.
    pub fn new(old_start: usize, new_start: usize) -> Self {
        Self {
            old_start,
            old_count: 0,
            new_start,
            new_count: 0,
            changes: Vec::new(),
        }
    }

    /// Add a change to this hunk.
    pub fn add_change(&mut self, change: Change) {
        match change.kind {
            ChangeKind::Added => self.new_count += 1,
            ChangeKind::Removed => self.old_count += 1,
            ChangeKind::Equal => {
                self.old_count += 1;
                self.new_count += 1;
            }
        }
        self.changes.push(change);
    }

    /// Get the header for this hunk (unified diff format).
    pub fn header(&self) -> String {
        format!(
            "@@ -{},{} +{},{} @@",
            self.old_start, self.old_count, self.new_start, self.new_count
        )
    }
}

/// Result of a diff operation.
#[derive(Clone, Debug)]
pub struct DiffResult {
    /// All changes.
    pub changes: Vec<Change>,
    /// Grouped into hunks.
    pub hunks: Vec<Hunk>,
    /// Number of additions.
    pub additions: usize,
    /// Number of deletions.
    pub deletions: usize,
}

impl DiffResult {
    /// Check if there are any changes.
    pub fn has_changes(&self) -> bool {
        self.additions > 0 || self.deletions > 0
    }

    /// Get summary string.
    pub fn summary(&self) -> String {
        format!("+{} -{}", self.additions, self.deletions)
    }
}

/// Diff between two files.
#[derive(Clone, Debug)]
pub struct FileDiff {
    /// Old file path.
    pub old_path: String,
    /// New file path.
    pub new_path: String,
    /// Diff result.
    pub diff: DiffResult,
}

impl FileDiff {
    /// Create a new file diff.
    pub fn new(old_path: impl Into<String>, new_path: impl Into<String>, diff: DiffResult) -> Self {
        Self {
            old_path: old_path.into(),
            new_path: new_path.into(),
            diff,
        }
    }
}

/// Compute a diff between two strings.
pub fn compute_diff(old: &str, new: &str) -> DiffResult {
    compute_diff_with_context(old, new, 3)
}

/// Compute a diff with specified context lines.
pub fn compute_diff_with_context(old: &str, new: &str, context: usize) -> DiffResult {
    let text_diff = TextDiff::from_lines(old, new);

    let mut changes = Vec::new();
    let mut old_line = 1usize;
    let mut new_line = 1usize;
    let mut additions = 0usize;
    let mut deletions = 0usize;

    for change in text_diff.iter_all_changes() {
        let kind = ChangeKind::from(change.tag());
        let content = change.value().trim_end_matches('\n').to_string();

        let (old_ln, new_ln) = match kind {
            ChangeKind::Equal => {
                let result = (Some(old_line), Some(new_line));
                old_line += 1;
                new_line += 1;
                result
            }
            ChangeKind::Removed => {
                deletions += 1;
                let result = (Some(old_line), None);
                old_line += 1;
                result
            }
            ChangeKind::Added => {
                additions += 1;
                let result = (None, Some(new_line));
                new_line += 1;
                result
            }
        };

        changes.push(Change::new(kind, content, old_ln, new_ln));
    }

    // Group changes into hunks
    let hunks = group_into_hunks(&changes, context);

    DiffResult {
        changes,
        hunks,
        additions,
        deletions,
    }
}

/// Group changes into hunks with context.
fn group_into_hunks(changes: &[Change], context: usize) -> Vec<Hunk> {
    if changes.is_empty() {
        return Vec::new();
    }

    let mut hunks = Vec::new();
    let mut current_hunk: Option<Hunk> = None;
    let mut context_buffer: Vec<Change> = Vec::new();
    let mut in_change = false;

    for (i, change) in changes.iter().enumerate() {
        match change.kind {
            ChangeKind::Equal => {
                if in_change {
                    // After a change, count context lines
                    context_buffer.push(change.clone());

                    if context_buffer.len() > context * 2 {
                        // End current hunk (keep only `context` trailing lines)
                        if let Some(ref mut hunk) = current_hunk {
                            for ctx in context_buffer.drain(..context) {
                                hunk.add_change(ctx);
                            }
                        }
                        if let Some(hunk) = current_hunk.take() {
                            hunks.push(hunk);
                        }
                        context_buffer.clear();
                        in_change = false;
                    }
                } else {
                    // Before a change, buffer context
                    context_buffer.push(change.clone());
                    if context_buffer.len() > context {
                        context_buffer.remove(0);
                    }
                }
            }
            ChangeKind::Added | ChangeKind::Removed => {
                if !in_change {
                    // Start new hunk
                    let old_start = change.old_line.unwrap_or(1);
                    let new_start = change.new_line.unwrap_or(1);
                    let hunk_start_old = old_start.saturating_sub(context_buffer.len());
                    let hunk_start_new = new_start.saturating_sub(context_buffer.len());

                    let mut hunk = Hunk::new(
                        hunk_start_old.max(1),
                        hunk_start_new.max(1),
                    );

                    // Add leading context
                    for ctx in context_buffer.drain(..) {
                        hunk.add_change(ctx);
                    }

                    current_hunk = Some(hunk);
                    in_change = true;
                } else if !context_buffer.is_empty() {
                    // Flush context buffer within hunk
                    if let Some(ref mut hunk) = current_hunk {
                        for ctx in context_buffer.drain(..) {
                            hunk.add_change(ctx);
                        }
                    }
                }

                if let Some(ref mut hunk) = current_hunk {
                    hunk.add_change(change.clone());
                }
            }
        }
    }

    // Finalize last hunk
    if let Some(mut hunk) = current_hunk {
        // Add remaining context (up to limit)
        for ctx in context_buffer.into_iter().take(context) {
            hunk.add_change(ctx);
        }
        hunks.push(hunk);
    }

    hunks
}

/// Compute word-level diff within a line.
pub fn compute_word_diff(old: &str, new: &str) -> Vec<(ChangeKind, String)> {
    let diff = TextDiff::from_words(old, new);

    diff.iter_all_changes()
        .map(|change| {
            (ChangeKind::from(change.tag()), change.value().to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_diff_no_changes() {
        let result = compute_diff("hello\n", "hello\n");
        assert!(!result.has_changes());
        assert_eq!(result.additions, 0);
        assert_eq!(result.deletions, 0);
    }

    #[test]
    fn test_compute_diff_addition() {
        let result = compute_diff("line1\n", "line1\nline2\n");
        assert!(result.has_changes());
        assert_eq!(result.additions, 1);
        assert_eq!(result.deletions, 0);
    }

    #[test]
    fn test_compute_diff_deletion() {
        let result = compute_diff("line1\nline2\n", "line1\n");
        assert!(result.has_changes());
        assert_eq!(result.additions, 0);
        assert_eq!(result.deletions, 1);
    }

    #[test]
    fn test_compute_diff_modification() {
        let result = compute_diff("hello\n", "world\n");
        assert!(result.has_changes());
        assert_eq!(result.additions, 1);
        assert_eq!(result.deletions, 1);
    }

    #[test]
    fn test_change_kind() {
        let added = Change::new(ChangeKind::Added, "new", None, Some(1));
        assert!(added.is_added());
        assert!(!added.is_removed());

        let removed = Change::new(ChangeKind::Removed, "old", Some(1), None);
        assert!(removed.is_removed());
        assert!(!removed.is_added());
    }

    #[test]
    fn test_hunk_creation() {
        let mut hunk = Hunk::new(1, 1);
        hunk.add_change(Change::new(ChangeKind::Equal, "context", Some(1), Some(1)));
        hunk.add_change(Change::new(ChangeKind::Removed, "old", Some(2), None));
        hunk.add_change(Change::new(ChangeKind::Added, "new", None, Some(2)));

        assert_eq!(hunk.old_count, 2); // 1 context + 1 removed
        assert_eq!(hunk.new_count, 2); // 1 context + 1 added
    }

    #[test]
    fn test_diff_summary() {
        let result = compute_diff("a\nb\nc\n", "a\nB\nc\nd\n");
        let summary = result.summary();
        assert!(summary.contains('+'));
        assert!(summary.contains('-'));
    }

    #[test]
    fn test_word_diff() {
        let changes = compute_word_diff("hello world", "hello there");
        assert!(!changes.is_empty());

        // Should have: "hello " (equal), "world" (removed), "there" (added)
        let kinds: Vec<_> = changes.iter().map(|(k, _)| *k).collect();
        assert!(kinds.contains(&ChangeKind::Equal));
    }

    #[test]
    fn test_hunks_grouping() {
        let old = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n";
        let new = "1\n2\nX\n4\n5\n6\n7\n8\nY\n10\n";

        let result = compute_diff_with_context(old, new, 2);

        // Should create separate hunks for distant changes
        assert!(!result.hunks.is_empty());
    }
}
