//! The transcript's rows, and how the chat client's typed events become
//! them. Every row draws with one of `coder-terminal`'s components.

use coder_terminal::components::card::Card;
use coder_terminal::components::run::{self, FileRow, RunRow, ToolRow};
use coder_terminal::components::turn::{self, Who};
use coder_terminal::{Intensity, Ladder};
use openagents_chat::coder_events::{self, CoderEvent, StepKind};
use openagents_chat::tool_groups::{Entry, Item, Shown, Stretch};
use ratatui::text::Line;

/// One entry in the transcript.
#[derive(Clone, Debug, PartialEq)]
pub enum Row {
    /// A message, or a finished reply.
    Turn(Who, String),
    /// A line under a turn: an offer or a notice.
    Note(String, Intensity),
    /// A framed card: welcome, help, pairing.
    Card(Card),
    /// One row of a Coder run.
    Run(RunRow),
    /// A Coder run's consecutive tool calls and thoughts, grouped as Grok
    /// Build groups them (#10117): condensed unless `expanded`.
    Tools { stretch: Stretch, expanded: bool },
}

impl Row {
    /// A quiet note.
    pub fn note(text: impl Into<String>) -> Self {
        Row::Note(text.into(), Intensity::Half)
    }

    /// A note the person must see: a refusal, a failure.
    pub fn loud(text: impl Into<String>) -> Self {
        Row::Note(text.into(), Intensity::Full)
    }

    /// The row's text as a test or a pipe reads it, one line per drawn row
    /// at `width`.
    pub fn text(&self, width: u16, ladder: Ladder) -> Vec<String> {
        lines(self, width, ladder)
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect()
            })
            .collect()
    }
}

/// The row's drawn lines at `width`.
///
/// As grok-build spaces its entries, one blank row follows a turn, a
/// stretch of tool calls, and a run's ending (its result, failure, stop,
/// or question); the rows inside a stretch keep no gap.
pub fn lines(row: &Row, width: u16, ladder: Ladder) -> Vec<Line<'static>> {
    let mut lines = match row {
        Row::Turn(who, text) => return turn::turn(*who, text, width, ladder),
        Row::Note(text, intensity) => return turn::note(text, *intensity, width, ladder),
        Row::Card(card) => return card.lines(width, ladder),
        Row::Run(row) => run::lines(row, width, ladder),
        Row::Tools { stretch, expanded } => {
            run::lines(&RunRow::Tools(tool_rows(stretch, *expanded)), width, ladder)
        }
    };
    let gap = matches!(
        row,
        Row::Tools { .. }
            | Row::Run(
                RunRow::Result { .. }
                    | RunRow::Failed { .. }
                    | RunRow::Stopped { .. }
                    | RunRow::Question { .. }
            )
    );
    if gap && !lines.is_empty() {
        lines.push(Line::default());
    }
    lines
}

/// A stretch's lines: condensed, each group's label, each call one line,
/// and a long run folded; expanded, every call with its command and
/// output under its group's label.
pub fn tool_rows(stretch: &Stretch, expanded: bool) -> Vec<ToolRow> {
    let mut rows = Vec::new();
    for item in stretch.items(!expanded) {
        push_item(&mut rows, &item, expanded);
    }
    rows
}

fn push_item(rows: &mut Vec<ToolRow>, item: &Item<'_>, expanded: bool) {
    match item {
        Item::Group { label, members, .. } => {
            rows.push(ToolRow::Group {
                label: label.text.clone(),
                failed: label.failed,
            });
            if expanded {
                for member in members {
                    rows.push(match member {
                        Entry::Call(shown) => call_row(shown, true),
                        Entry::Thought { text, .. } => ToolRow::Thought(text.clone()),
                    });
                }
            }
        }
        Item::Call(shown) => rows.push(call_row(shown, expanded)),
        Item::Thought { text, .. } => rows.push(ToolRow::Thought((*text).to_owned())),
        Item::More { label, .. } => rows.push(ToolRow::Group {
            label: label.text.clone(),
            failed: label.failed,
        }),
    }
}

fn call_row(shown: &Shown, expanded: bool) -> ToolRow {
    ToolRow::Call {
        verb: shown.verb().map(str::to_owned),
        line: shown.target(),
        result: shown.result(),
        running: shown.running,
        // The command shows under its line only when the line names what
        // it does in words; a line that is the command already shows it
        // (grok-build: `$ command` is the description's second line).
        command: shown
            .command()
            .filter(|_| expanded && shown.call.about.is_some())
            .map(str::to_owned),
        output: if expanded {
            shown.output.lines().map(str::to_owned).collect()
        } else {
            Vec::new()
        },
    }
}

/// The run row for one Coder event, or `None` for an event the transcript
/// shows elsewhere: the start (one short note, #10115), a reply step (the
/// result carries it), and progress, which the screen keeps as one live
/// line rather than a row per step.
pub fn run_row(event: &CoderEvent) -> Option<RunRow> {
    Some(match event {
        CoderEvent::CoderStarted(_) => return None,
        CoderEvent::Step(step) => {
            let mark = match step.kind {
                StepKind::Thinking => '·',
                StepKind::ToolCall => '>',
                StepKind::Note => '!',
                StepKind::Observation => ' ',
                // A command shows with its output; a message and the reply
                // show in the result.
                StepKind::Command | StepKind::Message | StepKind::Reply => return None,
            };
            let text = first_line(&step.text);
            if text.is_empty() {
                return None;
            }
            RunRow::Step { mark, text }
        }
        CoderEvent::Output(output) => {
            let lines: Vec<String> = output.text.lines().map(str::to_owned).collect();
            let tail = lines[lines.len().saturating_sub(3)..].to_vec();
            RunRow::Command {
                command: first_line(&output.command),
                exit: output.exit,
                timed_out: output.timed_out,
                tail,
            }
        }
        CoderEvent::ProviderSwitched(_) => RunRow::Switched {
            text: coder_events::text(event)
                .unwrap_or_default()
                .trim_start_matches([' ', '~'])
                .to_owned(),
        },
        CoderEvent::Question(asked) | CoderEvent::Approval(asked) => RunRow::Question {
            text: asked.text.trim().to_owned(),
            hint: asked.answer.clone(),
        },
        CoderEvent::Progress(_) | CoderEvent::Status(_) => return None,
        CoderEvent::Result(result) => RunRow::Result {
            summary: result.summary.trim().to_owned(),
            files: result
                .files_changed
                .iter()
                .map(|file| FileRow {
                    status: file.status.clone(),
                    path: file.path.clone(),
                    added: file.added,
                    removed: file.removed,
                    patch: file.patch.clone(),
                    cut: file.patch_cut,
                })
                .collect(),
            insertions: result.insertions,
            deletions: result.deletions,
            worktree: crate::app::home_relative(&result.worktree),
            cost_microusd: result.cost_microusd,
            expanded: false,
        },
        CoderEvent::Failure(_) => RunRow::Failed {
            text: coder_events::text(event).unwrap_or_default(),
        },
        CoderEvent::Stopped(stopped) => RunRow::Stopped {
            text: stopped.message.clone(),
        },
    })
}

/// The live progress row: "step N · ≈X% done · 9s", never a budget.
pub fn progress_row(progress: &coder_events::Progress) -> RunRow {
    RunRow::Progress {
        step: progress.step,
        percent: progress
            .complete
            .map(|complete| (complete.clamp(0.0, 1.0) * 100.0).round() as u8),
        seconds: progress.seconds.max(0.0) as u64,
    }
}

/// The provider's product name for its word.
pub fn provider(word: &str) -> String {
    coder_events::provider_name(&serde_json::Value::String(word.to_owned()))
}

fn first_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_chat::coder_events::{Output, Progress, Step};

    #[test]
    fn a_reply_step_and_progress_draw_no_row_of_their_own() {
        let reply = CoderEvent::Step(Step {
            turn: 1,
            step_id: 1,
            kind: StepKind::Reply,
            source: "agent".into(),
            text: "done".into(),
            call: None,
        });
        assert_eq!(run_row(&reply), None);
        let progress = CoderEvent::Progress(Progress {
            turn: 1,
            step: 2,
            seconds: 3.0,
            done: None,
            complete: Some(0.4),
        });
        assert_eq!(run_row(&progress), None);
    }

    #[test]
    fn a_command_keeps_its_exit_and_last_three_lines() {
        let output = CoderEvent::Output(Output {
            turn: 1,
            step_id: 2,
            command: "cargo test\n".into(),
            exit: Some(1),
            timed_out: false,
            seconds: 1.0,
            text: "a\nb\nc\nd".into(),
            truncated: false,
        });
        assert_eq!(
            run_row(&output),
            Some(RunRow::Command {
                command: "cargo test".into(),
                exit: Some(1),
                timed_out: false,
                tail: vec!["b".into(), "c".into(), "d".into()],
            })
        );
    }

    #[test]
    fn progress_is_an_estimate_never_a_budget() {
        let row = progress_row(&Progress {
            turn: 1,
            step: 5,
            seconds: 9.4,
            done: None,
            complete: Some(0.4),
        });
        assert_eq!(
            row,
            RunRow::Progress {
                step: 5,
                percent: Some(40),
                seconds: 9
            }
        );
        let text = Row::Run(row).text(80, Ladder::new(coder_terminal::Colors::None));
        assert!(text.iter().all(|line| !line.contains(" of ")), "{text:?}");
    }
}
