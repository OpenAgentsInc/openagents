//! `evidence.error_context`: the source lines a failing command's output
//! names, printed from the workspace with a bounded window.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::parse::{self, Location};

/// A failing command and its output, as the host saw it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Failure {
    /// The command, or what ran it, such as `the host's score`.
    pub command: String,
    /// Standard output and standard error as the reader saw them.
    pub output: String,
    pub exit: Option<i64>,
    pub timed_out: bool,
    /// Wall time, in milliseconds, when known.
    #[serde(default)]
    pub milliseconds: u64,
}

/// The bounds of one rendering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    /// Lines shown before and after each named line.
    pub window: usize,
    /// Regions at most.
    pub regions: usize,
    /// Characters at most, across the regions.
    pub chars: usize,
}

impl Default for Bounds {
    fn default() -> Self {
        Bounds {
            window: WINDOW,
            regions: 6,
            chars: 6_000,
        }
    }
}

/// Lines shown on each side of a named line, by default.
pub const WINDOW: usize = 6;

/// One region to print.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Region {
    /// The workspace file, relative to its root.
    pub file: String,
    /// The first and last lines shown, from 1.
    pub first: usize,
    pub last: usize,
    /// The lines the output named inside it.
    pub named: Vec<usize>,
    /// The command whose output named it.
    pub command: String,
    /// The rule that found the first named line.
    pub rule: String,
}

/// The resolved references of `failures`, most recent failure first and,
/// within one output, in [`parse::parse`]'s order: `(file, location,
/// command)`.
#[must_use]
pub fn located(
    failures: &[Failure],
    files: &BTreeSet<String>,
    root: &str,
) -> Vec<(String, Location, String)> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for failure in failures.iter().rev() {
        for location in parse::parse(&failure.output) {
            let Some(file) = parse::resolve(&location, files, root) else {
                continue;
            };
            if seen.insert((file.clone(), location.line)) {
                out.push((file, location, failure.command.clone()));
            }
        }
    }
    out
}

/// The regions to print for `located`, reading each file with `read`:
/// each named line with `bounds.window` lines around it, windows in the
/// same file merged when they overlap, at most `bounds.regions`, in the
/// order of `located`. A named line past the end of its file is skipped.
pub fn regions(
    located: &[(String, Location, String)],
    bounds: Bounds,
    mut read: impl FnMut(&str) -> Option<String>,
) -> Vec<Region> {
    let mut out: Vec<Region> = Vec::new();
    for (file, location, command) in located {
        let Some(text) = read(file) else {
            continue;
        };
        let lines = text.lines().count();
        let Ok(line) = usize::try_from(location.line) else {
            continue;
        };
        if line == 0 || line > lines {
            continue;
        }
        let first = line.saturating_sub(bounds.window).max(1);
        let last = (line + bounds.window).min(lines);
        if let Some(region) = out
            .iter_mut()
            .find(|r| r.file == *file && first <= r.last + 1 && r.first <= last + 1)
        {
            region.first = region.first.min(first);
            region.last = region.last.max(last);
            if !region.named.contains(&line) {
                region.named.push(line);
            }
            continue;
        }
        if out.len() >= bounds.regions {
            break;
        }
        out.push(Region {
            file: file.clone(),
            first,
            last,
            named: vec![line],
            command: command.clone(),
            rule: location.rule.clone(),
        });
    }
    out
}

/// The evidence text for `regions`, within `bounds.chars`, or `None` when
/// there's nothing to show.
pub fn render(
    regions: &[Region],
    bounds: Bounds,
    mut read: impl FnMut(&str) -> Option<String>,
) -> Option<String> {
    let mut text = String::new();
    let mut shown = 0;
    for region in regions {
        let Some(source) = read(&region.file) else {
            continue;
        };
        let mut block = format!(
            "`{}` lines {}-{}, named by `{}`:\n",
            region.file,
            region.first,
            region.last,
            crate::judge::clip(&region.command, 120)
        );
        for (index, line) in source
            .lines()
            .enumerate()
            .skip(region.first - 1)
            .take(region.last + 1 - region.first)
        {
            let number = index + 1;
            let mark = if region.named.contains(&number) {
                '>'
            } else {
                ' '
            };
            block.push_str(&format!("{mark}{number:>6}  {line}\n"));
        }
        if text.chars().count() + block.chars().count() > bounds.chars {
            break;
        }
        text.push_str(&block);
        text.push('\n');
        shown += 1;
    }
    (shown > 0).then(|| {
        format!(
            "The source lines the failing output names, most recent first. A line marked `>` \
             is the one the output points at.\n\n{}",
            text.trim_end()
        )
    })
}
