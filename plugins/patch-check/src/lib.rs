//! Validate a unified diff against a file's current content, as a
//! `packet-v0` guest plugin.
//!
//! Pure computation: the diff and the content both arrive in the input
//! packet, nothing is read from disk, and the answer is a placement report
//! — does each hunk still apply, where, and with how much drift — plus an
//! optional preview of the post-application content. The point is to check
//! whether a patch applies *before* anyone claims it does.
//!
//! Placement is per hunk, in order. A hunk is first tried at its declared
//! old-side position against the current content (with the line shift the
//! previously applied hunks introduced accounted for); on a mismatch the
//! old side is searched within `fuzz` lines of that position, and a match
//! found off-position reports its signed `drift_lines`. A hunk whose old
//! side matches nowhere in the window fails as `context_not_found`; one
//! that matches at two or more candidate positions fails as `ambiguous`.
//! Failures quote the first mismatching context line so the caller can see
//! what the file no longer says.

use openagents_pdk::Refusal;
use serde::{Deserialize, Serialize};

const DEFAULT_FUZZ: i64 = 200;
const FUZZ_CAP: i64 = 1_000;
const DEFAULT_MAX_PREVIEW_CHARS: usize = 20_000;
const MAX_PREVIEW_CHARS_CAP: usize = 100_000;
/// A quoted mismatching line is bounded to this many characters.
const MISMATCH_QUOTE_CHARS: usize = 200;

#[derive(Deserialize)]
pub struct Input {
    /// The unified diff to check: one file, one or more hunks.
    pub diff: String,
    /// The file's current text, checked as-is.
    pub content: String,
    /// Max line-offset drift searched when exact placement fails.
    #[serde(default)]
    pub fuzz: Option<i64>,
    /// When true, context lines match ignoring trailing whitespace.
    #[serde(default)]
    pub whitespace_lenient: bool,
    /// When true and every hunk applies, return the post-application text.
    #[serde(default)]
    pub include_preview: bool,
    /// Character ceiling for the preview; longer previews are elided in
    /// the middle.
    #[serde(default)]
    pub max_preview_chars: Option<usize>,
}

#[derive(Serialize, Debug)]
pub struct Output {
    /// True when every hunk placed.
    pub applies: bool,
    pub hunks: Vec<HunkReport>,
    pub applied_hunks: usize,
    pub failed_hunks: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview_truncated: Option<bool>,
}

#[derive(Serialize, Debug)]
pub struct HunkReport {
    /// Zero-based position of the hunk in the diff.
    pub index: usize,
    pub applies: bool,
    /// Where the hunk's old side sits, 1-based in the current content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at_line: Option<usize>,
    /// Signed line offset from the declared position, when placed off it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drift_lines: Option<i64>,
    /// `context_not_found` or `ambiguous`, when the hunk did not place.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The first old-side line that no longer matches, bounded to 200 chars.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mismatch: Option<String>,
}

/// One parsed hunk body line.
#[derive(Debug, Clone, PartialEq)]
enum HunkLine {
    Context(String),
    Remove(String),
    Add(String),
}

#[derive(Debug)]
struct Hunk {
    /// 1-based old-side start from the `@@` header (0 for a pure insertion
    /// into an empty region: "insert after line N" convention).
    old_start: usize,
    lines: Vec<HunkLine>,
    /// A `\ No newline at end of file` marker followed an old-side line.
    no_newline_old: bool,
    /// A `\ No newline at end of file` marker followed a new-side line.
    no_newline_new: bool,
}

impl Hunk {
    /// The old side: context and removal lines, with a context flag.
    fn old_side(&self) -> Vec<(&str, bool)> {
        self.lines
            .iter()
            .filter_map(|line| match line {
                HunkLine::Context(text) => Some((text.as_str(), true)),
                HunkLine::Remove(text) => Some((text.as_str(), false)),
                HunkLine::Add(_) => None,
            })
            .collect()
    }

    /// The new side: context and addition lines.
    fn new_side(&self) -> Vec<&str> {
        self.lines
            .iter()
            .filter_map(|line| match line {
                HunkLine::Context(text) => Some(text.as_str()),
                HunkLine::Add(text) => Some(text.as_str()),
                HunkLine::Remove(_) => None,
            })
            .collect()
    }
}

/// Parse `@@ -a[,b] +c[,d] @@ ...` into `(a, b, c, d)`.
fn parse_hunk_header(line: &str) -> Option<(usize, usize, usize, usize)> {
    let rest = line.strip_prefix("@@ -")?;
    let end = rest.find(" @@")?;
    let (old, new) = rest[..end].split_once(" +")?;
    let (a, b) = parse_range(old)?;
    let (c, d) = parse_range(new)?;
    Some((a, b, c, d))
}

fn parse_range(text: &str) -> Option<(usize, usize)> {
    match text.split_once(',') {
        Some((start, count)) => Some((start.parse().ok()?, count.parse().ok()?)),
        None => Some((text.parse().ok()?, 1)),
    }
}

fn malformed_refusal() -> Refusal {
    Refusal::unsupported(
        "no hunks parsed: a unified diff has optional file headers (`--- a/file`, \
         `+++ b/file`) and one or more hunks, each starting with a header like \
         `@@ -start,count +start,count @@` followed by body lines prefixed with \
         ' ' (context), '-' (removal), or '+' (addition)",
    )
}

/// Parse the diff into hunks, refusing a multi-file diff, a malformed
/// diff with no hunks, or a hunk whose body contradicts its header counts.
fn parse_diff(diff: &str) -> Result<Vec<Hunk>, Refusal> {
    let lines: Vec<&str> = diff.lines().collect();
    let mut hunks: Vec<Hunk> = Vec::new();
    let mut git_headers = 0usize;
    let mut old_file_headers = 0usize;
    let mut i = 0usize;

    while i < lines.len() {
        let line = lines[i];
        if line.starts_with("diff --git ") {
            git_headers += 1;
            i += 1;
            continue;
        }
        if line.starts_with("--- ") {
            old_file_headers += 1;
            i += 1;
            continue;
        }
        let Some((old_start, old_count, _new_start, new_count)) = parse_hunk_header(line) else {
            // File headers, index lines, mode lines, prose: skipped.
            i += 1;
            continue;
        };
        i += 1;

        let mut need_old = old_count;
        let mut need_new = new_count;
        let mut body: Vec<HunkLine> = Vec::new();
        let mut no_newline_old = false;
        let mut no_newline_new = false;

        loop {
            // A `\ No newline at end of file` marker binds to the line
            // before it, on that line's side(s).
            if i < lines.len() && lines[i].starts_with('\\') {
                match body.last() {
                    Some(HunkLine::Add(_)) => no_newline_new = true,
                    Some(HunkLine::Remove(_)) => no_newline_old = true,
                    Some(HunkLine::Context(_)) => {
                        no_newline_old = true;
                        no_newline_new = true;
                    }
                    None => {}
                }
                i += 1;
                continue;
            }
            if need_old == 0 && need_new == 0 {
                break;
            }
            let Some(&raw) = lines.get(i) else {
                return Err(Refusal::unsupported(format!(
                    "hunk {} is truncated: its header promised {} old and {} new \
                     lines but the diff ends {} old and {} new lines short",
                    hunks.len() + 1,
                    old_count,
                    new_count,
                    need_old,
                    need_new,
                )));
            };
            let parsed = if let Some(text) = raw.strip_prefix(' ') {
                (HunkLine::Context(text.to_string()), true, true)
            } else if let Some(text) = raw.strip_prefix('-') {
                (HunkLine::Remove(text.to_string()), true, false)
            } else if let Some(text) = raw.strip_prefix('+') {
                (HunkLine::Add(text.to_string()), false, true)
            } else if raw.is_empty() {
                // Some tools strip the single space from an empty context
                // line; tolerate it.
                (HunkLine::Context(String::new()), true, true)
            } else {
                return Err(Refusal::unsupported(format!(
                    "hunk {} has a body line with no ' ', '-', or '+' prefix \
                     before its header counts were satisfied: {:?}",
                    hunks.len() + 1,
                    bound_chars(raw, MISMATCH_QUOTE_CHARS),
                )));
            };
            let (kind, uses_old, uses_new) = parsed;
            if (uses_old && need_old == 0) || (uses_new && need_new == 0) {
                return Err(Refusal::unsupported(format!(
                    "hunk {} has more body lines than its header counts declare",
                    hunks.len() + 1,
                )));
            }
            if uses_old {
                need_old -= 1;
            }
            if uses_new {
                need_new -= 1;
            }
            body.push(kind);
            i += 1;
        }

        hunks.push(Hunk { old_start, lines: body, no_newline_old, no_newline_new });
    }

    let files = git_headers.max(old_file_headers);
    if files >= 2 {
        return Err(Refusal::unsupported(format!(
            "the diff names {files} files, and this check takes one file per \
             call — split the diff and pass each file's hunks with that \
             file's content",
        )));
    }
    if hunks.is_empty() {
        return Err(malformed_refusal());
    }
    Ok(hunks)
}

/// Truncate to at most `max` characters, on a char boundary.
fn bound_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        text.chars().take(max).collect()
    }
}

/// Does the old side match the buffer at `pos`?
fn matches_at(buffer: &[String], pos: usize, pattern: &[(&str, bool)], lenient: bool) -> bool {
    if pos + pattern.len() > buffer.len() {
        return false;
    }
    pattern.iter().enumerate().all(|(k, &(text, is_context))| {
        let actual = buffer[pos + k].as_str();
        if lenient && is_context {
            actual.trim_end() == text.trim_end()
        } else {
            actual == text
        }
    })
}

/// The first old-side line that fails to match at `pos`, quoted bounded.
fn first_mismatch(
    buffer: &[String],
    pos: i64,
    pattern: &[(&str, bool)],
    lenient: bool,
) -> String {
    if pos >= 0 {
        let pos = pos as usize;
        for (k, &(text, is_context)) in pattern.iter().enumerate() {
            match buffer.get(pos + k) {
                Some(actual) => {
                    let matched = if lenient && is_context {
                        actual.trim_end() == text.trim_end()
                    } else {
                        actual.as_str() == text
                    };
                    if !matched {
                        return bound_chars(text, MISMATCH_QUOTE_CHARS);
                    }
                }
                None => return bound_chars(text, MISMATCH_QUOTE_CHARS),
            }
        }
    }
    // Declared position out of range, or (ambiguous off-position) every
    // line matched: quote the first old-side line.
    pattern
        .first()
        .map(|&(text, _)| bound_chars(text, MISMATCH_QUOTE_CHARS))
        .unwrap_or_default()
}

fn handle(input: Input) -> Result<Output, Refusal> {
    let fuzz = input.fuzz.unwrap_or(DEFAULT_FUZZ).clamp(0, FUZZ_CAP);
    let max_preview_chars = input
        .max_preview_chars
        .unwrap_or(DEFAULT_MAX_PREVIEW_CHARS)
        .min(MAX_PREVIEW_CHARS_CAP);
    let lenient = input.whitespace_lenient;

    let hunks = parse_diff(&input.diff)?;

    let content_ends_nl = input.content.ends_with('\n');
    let mut buffer: Vec<String> = if input.content.is_empty() {
        Vec::new()
    } else {
        let mut lines: Vec<String> = input.content.split('\n').map(str::to_string).collect();
        if content_ends_nl {
            lines.pop();
        }
        lines
    };

    let mut reports: Vec<HunkReport> = Vec::new();
    // Net lines the applied hunks have added to (or removed from) the
    // working buffer ahead of the cursor: later hunks' declared old-side
    // positions are original-file coordinates, so their expected position
    // in the working buffer is `declared + shift`.
    let mut shift: i64 = 0;
    let mut ends_nl = content_ends_nl;

    for (index, hunk) in hunks.iter().enumerate() {
        let pattern = hunk.old_side();
        let new_side: Vec<String> = hunk.new_side().iter().map(|s| s.to_string()).collect();

        if pattern.is_empty() {
            // A pure insertion (`-N,0`) carries no verifiable old side:
            // the header means "insert after old line N". Accept it there.
            let at = ((hunk.old_start as i64) + shift).clamp(0, buffer.len() as i64) as usize;
            let added = new_side.len() as i64;
            buffer.splice(at..at, new_side);
            shift += added;
            if hunk.no_newline_new {
                ends_nl = false;
            }
            reports.push(HunkReport {
                index,
                applies: true,
                at_line: Some(hunk.old_start + 1),
                drift_lines: None,
                reason: None,
                mismatch: None,
            });
            continue;
        }

        let expected: i64 = hunk.old_start as i64 - 1 + shift;
        let placed: Option<(usize, i64)> = if expected >= 0
            && matches_at(&buffer, expected as usize, &pattern, lenient)
        {
            Some((expected as usize, 0))
        } else {
            // Search ±fuzz lines around the expected position for a
            // unique match.
            let last_valid = buffer.len() as i64 - pattern.len() as i64;
            let lo = (expected - fuzz).max(0);
            let hi = (expected + fuzz).min(last_valid);
            let mut found: Vec<i64> = Vec::new();
            let mut position = lo;
            while position <= hi {
                if position != expected && matches_at(&buffer, position as usize, &pattern, lenient)
                {
                    found.push(position);
                    if found.len() > 1 {
                        break;
                    }
                }
                position += 1;
            }
            match found.len() {
                1 => Some((found[0] as usize, found[0] - expected)),
                0 => {
                    reports.push(HunkReport {
                        index,
                        applies: false,
                        at_line: None,
                        drift_lines: None,
                        reason: Some("context_not_found".to_string()),
                        mismatch: Some(first_mismatch(&buffer, expected, &pattern, lenient)),
                    });
                    None
                }
                _ => {
                    reports.push(HunkReport {
                        index,
                        applies: false,
                        at_line: None,
                        drift_lines: None,
                        reason: Some("ambiguous".to_string()),
                        mismatch: Some(first_mismatch(&buffer, expected, &pattern, lenient)),
                    });
                    None
                }
            }
        };

        let Some((pos, drift)) = placed else { continue };

        // Report in current-content (original) coordinates.
        let at_line = (pos as i64 - shift + 1).max(1) as usize;
        let old_len = pattern.len() as i64;
        let new_len = new_side.len() as i64;
        buffer.splice(pos..pos + pattern.len(), new_side);
        shift += new_len - old_len;
        if hunk.no_newline_new {
            ends_nl = false;
        } else if hunk.no_newline_old {
            // The old side lacked the trailing newline and the new side
            // does not: the patch adds it back.
            ends_nl = true;
        }
        reports.push(HunkReport {
            index,
            applies: true,
            at_line: Some(at_line),
            drift_lines: (drift != 0).then_some(drift),
            reason: None,
            mismatch: None,
        });
    }

    let applied_hunks = reports.iter().filter(|report| report.applies).count();
    let failed_hunks = reports.len() - applied_hunks;
    let applies = failed_hunks == 0;

    let (preview, preview_truncated) = if input.include_preview && applies {
        let mut text = buffer.join("\n");
        if ends_nl && !buffer.is_empty() {
            text.push('\n');
        }
        let total = text.chars().count();
        if total > max_preview_chars {
            let head_chars = max_preview_chars / 2;
            let tail_chars = max_preview_chars - head_chars;
            let elided = total - max_preview_chars;
            let head: String = text.chars().take(head_chars).collect();
            let tail: String = text
                .chars()
                .skip(total - tail_chars)
                .collect();
            (
                Some(format!("{head}\n[{elided} chars elided]\n{tail}")),
                Some(true),
            )
        } else {
            (Some(text), None)
        }
    } else {
        (None, None)
    };

    Ok(Output {
        applies,
        hunks: reports,
        applied_hunks,
        failed_hunks,
        preview,
        preview_truncated,
    })
}

openagents_pdk::plugin_entry!(handle);

#[cfg(test)]
mod tests;
