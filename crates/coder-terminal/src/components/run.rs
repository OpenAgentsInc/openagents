//! A Coder run as the transcript shows it, one row at a time.
//!
//! The caller maps its run events into [`RunRow`]s; this module knows no
//! protocol, no clock, and no budget. Progress says where the run is —
//! the step, an estimate of how done it is, the time it has taken — and
//! never "of N": a Coder run has no step limit to count toward.

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use super::turn::marked_style;
use super::{INDENT, cells, clip, cut, diff, indent_at, sanitize, wrap_paragraphs};
use crate::markdown::{self, Marks};
use crate::{Intensity, Ladder};

/// One row of a Coder run as the transcript shows it. The caller maps its
/// run events into these; this module knows no protocol.
#[derive(Clone, Debug, PartialEq)]
pub enum RunRow {
    /// The run started: who runs it and why. Draws
    /// "Coder · {who} · {place}" (Full) then the reason (Half), indented.
    Start {
        who: String,
        place: String,
        why: String,
    },
    /// A step's text: thinking, a tool call, a note. `mark` is the one-char
    /// lead ("·", ">", "!"). The text draws at ThreeQuarters after a Half
    /// mark; a "!" step draws at Full. A thought ("·") is the engine's
    /// Markdown and draws with its marks, never its `**` or backticks.
    Step { mark: char, text: String },
    /// A command with its exit and the last lines of its output:
    /// "$ {command}" (ThreeQuarters), then "  exit {n}" / "  timed out"
    /// (Half; Full when nonzero or timed out), then the tail lines prefixed
    /// "  │ " (Half). `exit: None` and `!timed_out` = still running or
    /// unknown: no exit row.
    Command {
        command: String,
        exit: Option<i32>,
        timed_out: bool,
        tail: Vec<String>,
    },
    /// Where the run is: "step {step}{ · ≈{percent}% done}? · {elapsed}" at
    /// Half. Never "of N" — Coder runs have no budgets.
    Progress {
        step: usize,
        percent: Option<u8>,
        seconds: u64,
    },
    /// A provider switch, in words, at ThreeQuarters with a "~" lead.
    Switched { text: String },
    /// Coder asks the person: "Coder asks: {text}" at Full, the text drawn
    /// as Markdown, then "{hint}" at Half when present (how to answer).
    Question { text: String, hint: Option<String> },
    /// The turn finished: "Coder finished · {n} file(s) changed · +{ins}
    /// -{del}" (Full; "Coder finished" alone when nothing changed), the
    /// summary as Markdown, laid out as a reply is (body at ThreeQuarters,
    /// wrapped; never its raw `**`, backticks, or list dashes), each file
    /// "  {status} {path} (+a -r)" (Half; "?" when unknown), then
    /// "worktree {path}" (Half). Collapsed, a file with a patch adds
    /// "Press Ctrl+O to see the changes." (Half) at the end; `expanded`,
    /// each file's patch follows its row as grok-build draws an edit
    /// ([`super::diff`]): numbered, syntax-highlighted, removed and added
    /// lines on red and green bands, then "{n} more lines not shown" when
    /// the patch was cut.
    Result {
        summary: String,
        files: Vec<FileRow>,
        insertions: u64,
        deletions: u64,
        worktree: String,
        expanded: bool,
    },
    /// The run failed, in its words (Full).
    Failed { text: String },
    /// The run stopped, in its words (ThreeQuarters).
    Stopped { text: String },
    /// A stretch of tool activity, condensed or expanded (#10117), as Grok
    /// Build draws it: each [`ToolRow`] one line under the turn.
    Tools(Vec<ToolRow>),
}

/// One line of a stretch of tool activity. The caller decides what shows:
/// condensed, a group's label alone; expanded, the label, then each call
/// with its output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolRow {
    /// "◈ Read 3 files, Searched 2 patterns" (mark Half, label
    /// ThreeQuarters), then " · N failed" (Full) when any member failed.
    Group { label: String, failed: usize },
    /// "◆ Run cargo test" (mark Half, line Half; the mark Full while it
    /// runs or when it failed), then " · {result}" (Full) when it failed.
    /// Expanded, `command` ("$ …", Half) and `output` (Half) follow under
    /// it, clipped rather than wrapped.
    Call {
        line: String,
        result: Option<String>,
        running: bool,
        command: Option<String>,
        output: Vec<String>,
    },
    /// "· {text}": a thought (mark Half, text Half), its first line drawn
    /// as inline Markdown.
    Thought(String),
}

/// The mark of a group's label: Grok Build's dotted diamond.
pub const GROUP_MARK: &str = "◈ ";
/// The mark of one call: Grok Build's diamond.
pub const CALL_MARK: &str = "◆ ";

/// One changed file in a finished run.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileRow {
    pub status: String,
    pub path: String,
    pub added: Option<u64>,
    pub removed: Option<u64>,
    /// The file's unified diff from its first hunk, when the run sent one.
    pub patch: Option<String>,
    /// Lines of the patch the run left out.
    pub cut: u64,
}

/// The hint under a collapsed result whose files carry patches.
pub const SHOW_CHANGES: &str = "Press Ctrl+O to see the changes.";

/// Transcript lines for one run row at `width`, indented two cells under
/// the turn, long text wrapped with a hanging indent, output tail lines
/// clipped (not wrapped) with "…".
pub fn lines(row: &RunRow, width: u16, ladder: Ladder) -> Vec<Line<'static>> {
    let block = Block {
        width: usize::from(width),
        ladder,
    };
    let half = Intensity::Half;
    let three = Intensity::ThreeQuarters;
    let full = Intensity::Full;
    let mut out = Vec::new();
    match row {
        RunRow::Start { who, place, why } => {
            let head = ["Coder", who, place]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
            block.text(&mut out, 0, "", &head, full);
            if !why.is_empty() {
                block.text(&mut out, 0, "", why, half);
            }
        }
        RunRow::Step { mark, text } => {
            let (mark_at, text_at) = if *mark == '!' {
                (full, full)
            } else {
                (half, three)
            };
            if *mark == '·' {
                block.markdown(&mut out, 0, "· ", mark_at, text, Some(text_at));
            } else {
                block.marked(&mut out, 0, &format!("{mark} "), mark_at, text, text_at);
            }
        }
        RunRow::Command {
            command,
            exit,
            timed_out,
            tail,
        } => {
            block.marked(&mut out, 0, "$ ", three, command, three);
            let status = match (timed_out, exit) {
                (true, _) => Some(("timed out".to_owned(), full)),
                (false, Some(0)) => Some(("exit 0".to_owned(), half)),
                (false, Some(code)) => Some((format!("exit {code}"), full)),
                (false, None) => None,
            };
            if let Some((status, intensity)) = status {
                block.text(&mut out, INDENT, "", &status, intensity);
            }
            for line in tail {
                block.clipped(&mut out, INDENT, "│ ", line, half);
            }
        }
        RunRow::Progress {
            step,
            percent,
            seconds,
        } => {
            block.text(&mut out, 0, "", &progress(*step, *percent, *seconds), half);
        }
        RunRow::Switched { text } => {
            block.marked(&mut out, 0, "~ ", three, text, three);
        }
        RunRow::Question { text, hint } => {
            let asked = format!("Coder asks: {text}");
            block.markdown(&mut out, 0, "", full, &asked, Some(full));
            if let Some(hint) = hint.as_deref().filter(|hint| !hint.is_empty()) {
                block.text(&mut out, 0, "", hint, half);
            }
        }
        RunRow::Result {
            summary,
            files,
            insertions,
            deletions,
            worktree,
            expanded,
        } => {
            let count = files.len();
            let noun = if count == 1 { "file" } else { "files" };
            let head = if count == 0 && *insertions == 0 && *deletions == 0 {
                "Coder finished".to_owned()
            } else {
                format!("Coder finished · {count} {noun} changed · +{insertions} -{deletions}")
            };
            block.text(&mut out, 0, "", &head, full);
            if !summary.is_empty() {
                block.markdown(&mut out, 0, "", three, summary, None);
            }
            for file in files {
                let count = |n: Option<u64>| n.map_or_else(|| "?".to_owned(), |n| n.to_string());
                let row = format!(
                    "{} {} (+{} -{})",
                    file.status,
                    file.path,
                    count(file.added),
                    count(file.removed)
                );
                block.text(&mut out, INDENT, "", &row, half);
                if *expanded {
                    block.patch(&mut out, file);
                }
            }
            if !worktree.is_empty() {
                block.text(&mut out, 0, "", &format!("worktree {worktree}"), half);
            }
            if !*expanded
                && files
                    .iter()
                    .any(|file| file.patch.is_some() || file.cut > 0)
            {
                block.text(&mut out, 0, "", SHOW_CHANGES, half);
            }
        }
        RunRow::Failed { text } => block.text(&mut out, 0, "", text, full),
        RunRow::Stopped { text } => block.text(&mut out, 0, "", text, three),
        RunRow::Tools(rows) => {
            for row in rows {
                block.tool(&mut out, row);
            }
        }
    }
    out
}

/// "9s", "1m 5s", "1h 2m".
pub fn elapsed(seconds: u64) -> String {
    match seconds {
        0..60 => format!("{seconds}s"),
        60..3600 => format!("{}m {}s", seconds / 60, seconds % 60),
        _ => format!("{}h {}m", seconds / 3600, seconds % 3600 / 60),
    }
}

/// "step 3 · ≈40% done · 1m 5s", or without the estimate when there is none.
fn progress(step: usize, percent: Option<u8>, seconds: u64) -> String {
    let mut out = format!("step {step}");
    if let Some(percent) = percent {
        out.push_str(&format!(" · ≈{}% done", percent.min(100)));
    }
    out.push_str(" · ");
    out.push_str(&elapsed(seconds));
    out
}

/// One line of Markdown as inline runs, without its marks' syntax,
/// clipped to `room` cells with "…" when it is longer.
fn inline(line: &str, room: usize) -> Vec<(String, Marks)> {
    let marked = markdown::render(&sanitize(line))
        .into_iter()
        .map(|rendered| rendered.marked)
        .find(|marked| !marked.text.trim().is_empty())
        .unwrap_or_else(|| markdown::Marked::plain(sanitize(line)));
    let runs = marked.runs_in(0..marked.text.len());
    if cells(&marked.text) <= room {
        return runs;
    }
    let mut left = room.saturating_sub(1);
    let mut out = Vec::new();
    for (run, marks) in runs {
        let kept = cut(&run, left);
        left -= cells(&kept);
        let whole = kept.len() == run.len();
        if !kept.is_empty() {
            out.push((kept, marks.clone()));
        }
        if !whole {
            break;
        }
    }
    if room > 0 {
        out.push(("…".to_owned(), Marks::default()));
    }
    out
}

/// The geometry one run row draws at.
struct Block {
    width: usize,
    ladder: Ladder,
}

impl Block {
    /// The cells left of the row's text: the turn's indent plus `extra`,
    /// never so many that no cell is left for text.
    fn lead(&self, extra: usize) -> usize {
        (indent_at(self.width) + extra).min(self.width.saturating_sub(2))
    }

    fn style(&self, intensity: Intensity) -> Style {
        self.ladder.style(intensity)
    }

    /// `text` at `intensity`, after `extra` cells beyond the indent,
    /// wrapped with continuation rows under its first.
    fn text(
        &self,
        out: &mut Vec<Line<'static>>,
        extra: usize,
        mark: &str,
        text: &str,
        intensity: Intensity,
    ) {
        self.marked(out, extra, mark, intensity, text, intensity);
    }

    /// A lead `mark` then `text`, wrapped with continuation rows hanging
    /// under the text rather than the mark.
    fn marked(
        &self,
        out: &mut Vec<Line<'static>>,
        extra: usize,
        mark: &str,
        mark_at: Intensity,
        text: &str,
        text_at: Intensity,
    ) {
        let lead = self.lead(extra);
        let room = self.width.saturating_sub(lead).max(1);
        let hang = cells(mark).min(room.saturating_sub(1));
        let rows = wrap_paragraphs(&format!("{mark}{text}"), room, hang);
        for (index, (continued, row)) in rows.into_iter().enumerate() {
            let mut spans = vec![Span::raw(" ".repeat(lead))];
            if index == 0 && !mark.is_empty() && row.starts_with(mark) {
                spans.push(Span::styled(mark.to_owned(), self.style(mark_at)));
                spans.push(Span::styled(
                    row[mark.len()..].to_owned(),
                    self.style(text_at),
                ));
            } else {
                let hang = if continued || index > 0 { hang } else { 0 };
                spans.push(Span::raw(" ".repeat(hang)));
                spans.push(Span::styled(row, self.style(text_at)));
            }
            out.push(Line::from(spans));
        }
    }

    /// A lead `mark` then `text` as Markdown, laid out as a reply is: inline
    /// marks styled, lists and code as blocks, continuation rows hanging
    /// under the text rather than the mark. Each line draws at `text_at`,
    /// or at the step the layout gives it when `None`.
    fn markdown(
        &self,
        out: &mut Vec<Line<'static>>,
        extra: usize,
        mark: &str,
        mark_at: Intensity,
        text: &str,
        text_at: Option<Intensity>,
    ) {
        let lead = self.lead(extra);
        let room = self.width.saturating_sub(lead).max(1);
        let hang = cells(mark).min(room.saturating_sub(1));
        let text = text
            .split('\n')
            .map(sanitize)
            .collect::<Vec<_>>()
            .join("\n");
        let rows = markdown::wrapped(text.trim(), room.saturating_sub(hang).max(1));
        for (index, rendered) in rows.into_iter().enumerate() {
            let base = self.style(text_at.unwrap_or(rendered.intensity));
            let mut spans = vec![Span::raw(" ".repeat(lead))];
            if index == 0 && hang > 0 {
                spans.push(Span::styled(cut(mark, hang), self.style(mark_at)));
            } else if hang > 0 {
                spans.push(Span::raw(" ".repeat(hang)));
            }
            for (run, marks) in rendered.marked.runs_in(0..rendered.marked.text.len()) {
                spans.push(Span::styled(run, marked_style(base, self.ladder, &marks)));
            }
            out.push(Line::from(spans));
        }
    }

    /// One line of a stretch of tool activity: its mark, its words, and a
    /// failure in Full, clipped to one row; an expanded call's command and
    /// output under it.
    fn tool(&self, out: &mut Vec<Line<'static>>, row: &ToolRow) {
        let half = Intensity::Half;
        let full = Intensity::Full;
        let (mark, mark_at, text, text_at, tail) = match row {
            ToolRow::Group { label, failed } => (
                GROUP_MARK,
                half,
                label.as_str(),
                Intensity::ThreeQuarters,
                (*failed > 0).then(|| format!(" · {failed} failed")),
            ),
            ToolRow::Call {
                line,
                result,
                running,
                ..
            } => (
                CALL_MARK,
                if *running || result.is_some() {
                    full
                } else {
                    half
                },
                line.as_str(),
                half,
                result.as_ref().map(|result| format!(" · {result}")),
            ),
            ToolRow::Thought(text) => ("· ", half, text.as_str(), half, None),
        };
        let lead = self.lead(0);
        let room = self.width.saturating_sub(lead);
        let mark = clip(mark, room);
        let room = room.saturating_sub(cells(&mark));
        let tail = tail.unwrap_or_default();
        let first = text
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or("");
        let space = room.saturating_sub(cells(&tail).min(room));
        // A thought is the engine's Markdown; a call or a label is a path,
        // a pattern, or a command, drawn as it is.
        let body = if matches!(row, ToolRow::Thought(_)) {
            inline(first, space)
        } else {
            vec![(clip(&sanitize(first), space), Marks::default())]
        };
        let used: usize = body.iter().map(|(run, _)| cells(run)).sum();
        let tail = clip(&tail, room.saturating_sub(used));
        let base = self.style(text_at);
        let mut spans = vec![
            Span::raw(" ".repeat(lead)),
            Span::styled(mark, self.style(mark_at)),
        ];
        spans.extend(
            body.into_iter()
                .map(|(run, marks)| Span::styled(run, marked_style(base, self.ladder, &marks))),
        );
        if !tail.is_empty() {
            spans.push(Span::styled(tail, self.style(full)));
        }
        out.push(Line::from(spans));
        if let ToolRow::Call {
            command, output, ..
        } = row
        {
            if let Some(command) = command {
                self.clipped(
                    out,
                    INDENT,
                    "$ ",
                    command.lines().next().unwrap_or(""),
                    half,
                );
            }
            for line in output {
                self.clipped(out, INDENT, "", line, half);
            }
        }
    }

    /// A changed file's patch under its row, drawn as grok-build draws an
    /// edit, then how many lines were left out.
    fn patch(&self, out: &mut Vec<Line<'static>>, file: &FileRow) {
        out.extend(diff::lines(
            file.patch.as_deref().unwrap_or(""),
            &file.path,
            self.lead(INDENT * 2)
                .saturating_sub(super::diff::INDENT_CELLS),
            self.width,
            crate::markdown::palette(),
            crate::markdown::syntax_level(self.ladder),
        ));
        if file.cut > 0 {
            let noun = if file.cut == 1 { "line" } else { "lines" };
            let note = format!("{} more {noun} not shown", file.cut);
            self.clipped(out, INDENT * 2, "", &note, Intensity::Half);
        }
    }

    /// `mark` then `text` on one row, clipped with "…" rather than wrapped.
    fn clipped(
        &self,
        out: &mut Vec<Line<'static>>,
        extra: usize,
        mark: &str,
        text: &str,
        intensity: Intensity,
    ) {
        let lead = self.lead(extra);
        let room = self.width.saturating_sub(lead);
        let row = clip(&format!("{mark}{}", sanitize(text)), room);
        out.push(Line::from(vec![
            Span::raw(" ".repeat(lead)),
            Span::styled(row, self.style(intensity)),
        ]));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(row: &RunRow, width: u16) -> Vec<String> {
        lines(row, width, Ladder::default())
            .iter()
            // A diff's band pads its rows to the width.
            .map(|line| line.to_string().trim_end().to_owned())
            .collect()
    }

    #[test]
    fn progress_never_counts_toward_a_limit() {
        for step in [0, 1, 7, 250] {
            for percent in [None, Some(0), Some(40), Some(100), Some(255)] {
                for seconds in [0, 59, 65, 3_725] {
                    let row = RunRow::Progress {
                        step,
                        percent,
                        seconds,
                    };
                    for line in text(&row, 80) {
                        assert!(!line.contains(" of "), "{line}");
                        assert!(!line.contains("/"), "{line}");
                    }
                }
            }
        }
        assert_eq!(
            text(
                &RunRow::Progress {
                    step: 3,
                    percent: Some(40),
                    seconds: 65
                },
                80
            ),
            ["  step 3 · ≈40% done · 1m 5s"]
        );
    }

    #[test]
    fn elapsed_reads_in_the_largest_two_units() {
        assert_eq!(elapsed(0), "0s");
        assert_eq!(elapsed(9), "9s");
        assert_eq!(elapsed(60), "1m 0s");
        assert_eq!(elapsed(65), "1m 5s");
        assert_eq!(elapsed(3_600), "1h 0m");
        assert_eq!(elapsed(3_720), "1h 2m");
        assert_eq!(elapsed(90_000), "25h 0m");
    }

    #[test]
    fn a_step_wraps_under_its_text_not_its_mark() {
        let row = RunRow::Step {
            mark: '·',
            text: "reading the file that matters".into(),
        };
        assert_eq!(
            text(&row, 20),
            ["  · reading the", "    file that", "    matters"]
        );
    }

    #[test]
    fn a_tail_line_clips_rather_than_wraps() {
        let row = RunRow::Command {
            command: "cargo test".into(),
            exit: None,
            timed_out: false,
            tail: vec!["x".repeat(50)],
        };
        let lines = text(&row, 20);
        assert_eq!(lines.len(), 2, "no exit row while running: {lines:?}");
        assert!(lines[1].ends_with('…'));
        assert_eq!(cells(&lines[1]), 20);
    }

    #[test]
    fn a_result_shows_its_patch_only_when_expanded() {
        let mut row = RunRow::Result {
            summary: String::new(),
            files: vec![FileRow {
                status: "modified".into(),
                path: "a.rs".into(),
                added: Some(1),
                removed: Some(1),
                patch: Some("@@ -1 +1 @@\n-old\n+new".into()),
                cut: 1,
            }],
            insertions: 1,
            deletions: 1,
            worktree: String::new(),
            expanded: false,
        };
        assert_eq!(
            text(&row, 80),
            [
                "  Coder finished · 1 file changed · +1 -1",
                "    modified a.rs (+1 -1)",
                "  Press Ctrl+O to see the changes.",
            ]
        );
        if let RunRow::Result { expanded, .. } = &mut row {
            *expanded = true;
        }
        assert_eq!(
            text(&row, 80),
            [
                "  Coder finished · 1 file changed · +1 -1",
                "    modified a.rs (+1 -1)",
                "      1  old",
                "      1  new",
                "      1 more line not shown",
            ]
        );
    }

    /// The summary a Codex run on CoderOS ended with (2026-10-02): the
    /// transcript showed its `**`, backticks, and list dashes raw.
    const CODEX_SUMMARY: &str = "Added and committed a tested Terminal improvement: **Ctrl+P opens a searchable command palette**. It searches command names and descriptions, preserves your draft, and runs the existing command handlers. Updated `/help` and the user guide.\n\n**Validation passed:**\n- 45 terminal unit tests.\n- 2 PTY tests, including rendering, filtering, and using the palette.\n- Offline release and installer tests.\n- Formatting and diff checks.\n\nCommit: `0d6afa57b4`.\n\n**Terminal 1.0.0 is not launched yet.** The public release-candidate channel still points to `1.0.0-rc.2`, and the stable channel returns HTTP 404.";

    /// Whether any drawn row still shows Markdown's syntax.
    pub(crate) fn raw_markdown(rows: &[String]) -> Vec<String> {
        rows.iter()
            .filter(|row| {
                let text = row.trim_start();
                row.contains("**")
                    || row.contains('`')
                    || text.starts_with("- ")
                    || text.starts_with("# ")
                    || text.starts_with("## ")
                    || row.contains("](")
            })
            .cloned()
            .collect()
    }

    #[test]
    fn a_result_summary_draws_as_markdown_not_its_syntax() {
        let row = RunRow::Result {
            summary: CODEX_SUMMARY.into(),
            files: Vec::new(),
            insertions: 0,
            deletions: 0,
            worktree: String::new(),
            expanded: false,
        };
        let rows = text(&row, 80);
        assert_eq!(raw_markdown(&rows), Vec::<String>::new(), "{rows:#?}");
        assert_eq!(rows[0], "  Coder finished", "nothing changed: no counts");
        assert!(
            rows.iter().any(|row| row == "  • 45 terminal unit tests."),
            "{rows:#?}"
        );
        assert!(rows.iter().any(|row| row.contains("Commit: 0d6afa57b4.")));
        // Bold and code keep their marks as styles.
        let lines = lines(&row, 80, Ladder::default());
        let bold = lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .find(|span| span.content.contains("Validation passed:"))
            .expect("the bold line");
        assert!(
            bold.style
                .add_modifier
                .contains(ratatui::style::Modifier::BOLD)
        );
    }

    #[test]
    fn a_thought_and_a_question_draw_as_markdown() {
        let thought = RunRow::Step {
            mark: '·',
            text: "**Inspecting** the `Store::open` path".into(),
        };
        assert_eq!(text(&thought, 80), ["  · Inspecting the Store::open path"]);
        let tool = RunRow::Tools(vec![ToolRow::Thought(
            "**Planning the fix**\n\nThen `cargo test`.".into(),
        )]);
        assert_eq!(text(&tool, 80), ["  · Planning the fix"]);
        let long = RunRow::Tools(vec![ToolRow::Thought(format!(
            "**{}**",
            "word ".repeat(30).trim()
        ))]);
        let rows = text(&long, 30);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].ends_with('…') && cells(&rows[0]) <= 30, "{rows:?}");
        assert!(!rows[0].contains("**"));
        let asked = RunRow::Question {
            text: "Which one?\n\n- `main`\n- **a branch**".into(),
            hint: None,
        };
        let rows = text(&asked, 80);
        assert_eq!(raw_markdown(&rows), Vec::<String>::new(), "{rows:#?}");
        assert_eq!(rows[0], "  Coder asks: Which one?");
        assert!(rows.contains(&"  • main".to_owned()), "{rows:#?}");
    }

    #[test]
    fn nothing_is_wider_than_the_width() {
        let row = RunRow::Result {
            summary: format!("a summary of the change\n\n{CODEX_SUMMARY}"),
            files: vec![FileRow {
                status: "M".into(),
                path: "crates/a/very/long/path/to/a/file.rs".into(),
                added: Some(3),
                removed: None,
                patch: Some("@@ -1 +1 @@\n-a very long removed line of code\n+x".into()),
                cut: 12,
            }],
            insertions: 3,
            deletions: 0,
            worktree: "/tmp/worktree".into(),
            expanded: true,
        };
        for width in [0u16, 1, 2, 5, 12, 40] {
            for line in lines(&row, width, Ladder::default()) {
                assert!(line.width() <= usize::from(width).max(1), "{width}: {line}");
            }
        }
    }
}
