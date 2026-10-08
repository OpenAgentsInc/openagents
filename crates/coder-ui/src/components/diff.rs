//! Typed hunk-only diff rows reimplemented from the public terminal component.
//! The source component retains grok-build's Apache-2.0 attribution.

use crate::{
    components::{run, syntax, truncate, wrap},
    source_theme as t,
};
use rust_native::view::RichRun;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Equal,
    Delete,
    Insert,
}
#[derive(Clone, Debug)]
pub struct DiffLine {
    pub text: String,
    pub old: usize,
    pub new: usize,
    pub change: Change,
}

pub fn hunks(patch: &str) -> Vec<Vec<DiffLine>> {
    let mut out: Vec<Vec<DiffLine>> = Vec::new();
    let (mut old, mut new) = (0, 0);
    for line in patch.lines() {
        if let Some(header) = line.strip_prefix("@@") {
            for part in header.split_whitespace() {
                let number = |value: &str| {
                    value
                        .split(',')
                        .next()
                        .and_then(|n| n.parse().ok())
                        .unwrap_or(0)
                };
                if let Some(value) = part.strip_prefix('-') {
                    old = number(value);
                } else if let Some(value) = part.strip_prefix('+') {
                    new = number(value);
                }
            }
            out.push(Vec::new());
            continue;
        }
        let Some(hunk) = out.last_mut() else {
            continue;
        };
        let (change, text) = match line.as_bytes().first() {
            Some(b'+') => (Change::Insert, &line[1..]),
            Some(b'-') => (Change::Delete, &line[1..]),
            Some(b' ') => (Change::Equal, &line[1..]),
            Some(b'\\') => continue,
            _ => (Change::Equal, line),
        };
        hunk.push(DiffLine {
            text: text.into(),
            old,
            new,
            change,
        });
        if change != Change::Insert {
            old += 1;
        }
        if change != Change::Delete {
            new += 1;
        }
    }
    out
}

pub fn counts(patch: &str) -> (usize, usize) {
    hunks(patch)
        .iter()
        .flatten()
        .fold((0, 0), |(added, removed), line| match line.change {
            Change::Insert => (added + 1, removed),
            Change::Delete => (added, removed + 1),
            Change::Equal => (added, removed),
        })
}

pub fn lines(patch: &str, path: &str, width: usize) -> Vec<Vec<RichRun>> {
    let hunks = hunks(patch);
    let mut out = Vec::new();
    for (h, hunk) in hunks.iter().enumerate() {
        if h > 0 && !out.is_empty() {
            let previous = hunks[h - 1]
                .iter()
                .rev()
                .find(|l| l.change != Change::Delete)
                .map(|l| l.new);
            let next = hunk
                .iter()
                .find(|l| l.change != Change::Delete)
                .map(|l| l.new);
            let gap = previous
                .zip(next)
                .and_then(|(p, n)| n.checked_sub(p + 1))
                .filter(|n| *n > 0);
            let label = match gap {
                Some(1) => "… 1 unchanged line".into(),
                Some(n) => format!("… {n} unchanged lines"),
                None => "…".into(),
            };
            out.push(vec![run(truncate(&format!("  {label}"), width), t::GRAY)]);
        }
        let gutter = hunk
            .iter()
            .map(|l| l.old.max(l.new).max(1))
            .max()
            .unwrap_or(1)
            .ilog10() as usize
            + 1;
        let total = gutter + 4;
        let content_width = width.saturating_sub(total).max(1);
        let old = hunk
            .iter()
            .filter(|l| l.change != Change::Insert)
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let new = hunk
            .iter()
            .filter(|l| l.change != Change::Delete)
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let old_rows = syntax::lines(&old, path);
        let new_rows = syntax::lines(&new, path);
        let (mut oi, mut ni) = (0, 0);
        for line in hunk {
            let background = match line.change {
                Change::Delete => Some(t::DIFF_DELETE_BG),
                Change::Insert => Some(t::DIFF_INSERT_BG),
                Change::Equal => None,
            };
            let foreground = match line.change {
                Change::Delete => t::DIFF_DELETE_FG,
                Change::Insert => t::DIFF_INSERT_FG,
                Change::Equal => t::GRAY,
            };
            let highlighted = if line.change == Change::Delete {
                old_rows.get(oi)
            } else {
                new_rows.get(ni)
            };
            let mut content = highlighted
                .cloned()
                .unwrap_or_else(|| vec![run(&line.text, foreground)]);
            // Grok Night keeps changed unknown-language content neutral on a
            // colored band; only its gutter uses the change's foreground.
            if !syntax::known(path) {
                for r in &mut content {
                    r.foreground = Some(if line.change == Change::Equal {
                        t::GRAY
                    } else {
                        t::TEXT_PRIMARY
                    });
                }
            }
            for (i, mut body) in wrap(&content, content_width).into_iter().enumerate() {
                let used = body.iter().map(|r| r.text.width()).sum::<usize>();
                let mut row = if i == 0 {
                    vec![
                        run("  ", t::TEXT_SECONDARY),
                        run(
                            format!(
                                "{:>gutter$}",
                                if line.change == Change::Delete {
                                    line.old
                                } else {
                                    line.new
                                }
                            ),
                            foreground,
                        ),
                        run("  ", t::TEXT_SECONDARY),
                    ]
                } else {
                    vec![run(" ".repeat(total), t::TEXT_SECONDARY)]
                };
                for r in &mut body {
                    r.background = background;
                }
                row.extend(body);
                if let Some(bg) = background {
                    let mut pad = run(
                        " ".repeat(content_width.saturating_sub(used)),
                        t::TEXT_SECONDARY,
                    );
                    pad.background = Some(bg);
                    row.push(pad);
                }
                out.push(row);
            }
            if line.change != Change::Insert {
                oi += 1;
            }
            if line.change != Change::Delete {
                ni += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counts_headers_numbers_change_bands_and_hunk_gap() {
        let patch = "--- a/file\n+++ b/file\n@@ -10,2 +10,2 @@\n-old\n+new\n same\n@@ -20 +20 @@\n+later\n\\ No newline at end of file";
        assert_eq!(counts(patch), (2, 1));
        let rows = lines(patch, "unknown", 24);
        assert!(
            rows.iter()
                .flatten()
                .any(|r| r.background == Some(t::DIFF_DELETE_BG))
        );
        assert!(
            rows.iter()
                .flatten()
                .any(|r| r.background == Some(t::DIFF_INSERT_BG))
        );
        assert!(
            rows.iter()
                .flatten()
                .any(|r| r.text.contains("8 unchanged lines"))
        );
        assert_eq!(
            rows[0]
                .iter()
                .map(|r| r.text.as_str())
                .collect::<String>()
                .trim_end(),
            "  10  old"
        );
    }
}
