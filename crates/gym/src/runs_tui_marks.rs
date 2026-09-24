//! Marking in the Runs pane: `x` marks the selected run, or the selected
//! step of a transcript, as bad; `v` clears a run; `u` removes a mark.
//!
//! `x` opens a composer over the pane. What you type is the note; the
//! arrow keys move through the `runs-learning-v1` judgments and `Tab`
//! toggles one as a tag; `Enter` saves and `Esc` cancels. Marks go to the
//! same append-only store `gym runs mark` writes, so they survive a
//! restart and the command line sees them.

use std::collections::BTreeSet;

use coder_terminal::{Intensity, frame, rail};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;

use super::{Key, Pane, Tab};
use crate::runs;
use crate::runs_learning::{Context, JUDGMENTS};
use crate::runs_marks::{Mark, Marks, NewMark, Verdict, evidence_key};
use crate::runs_story::Detail;

/// A mark being written.
#[derive(Clone, Debug)]
pub(super) struct Composer {
    run: String,
    task: String,
    step: Option<usize>,
    note: String,
    tags: BTreeSet<&'static str>,
    /// The judgment the arrow keys are on.
    cursor: usize,
}

/// The marks as the pane holds them.
pub(super) struct Marking {
    pub(super) marks: Marks,
    pub(super) author: String,
    pub(super) composer: Option<Composer>,
    /// What the last mark, clear, or removal did, until the next key.
    pub(super) notice: Option<String>,
}

impl Marking {
    pub(super) fn in_memory() -> Self {
        Marking {
            marks: Marks::open(None),
            author: crate::runs_marks::default_author(),
            composer: None,
            notice: None,
        }
    }
}

impl Pane {
    /// The same pane with `marks`, the store `x`, `v`, and `u` write to,
    /// and `author`, the name each mark carries.
    #[must_use]
    pub fn with_marks(mut self, marks: Marks, author: String) -> Self {
        self.marking.marks = marks;
        self.marking.author = author;
        self
    }

    /// The marks the pane holds.
    #[must_use]
    pub fn marks(&self) -> &Marks {
        &self.marking.marks
    }

    /// Whether the mark composer is open.
    #[must_use]
    pub fn composing(&self) -> bool {
        self.marking.composer.is_some()
    }

    /// The run and step `x`, `v`, and `u` act on: the selected step in a
    /// transcript, else the open or selected run.
    fn mark_target(&self) -> Option<(String, String, Option<usize>)> {
        match &self.open {
            Some(open) => {
                let step = (open.tab == Tab::Transcript && !open.blocks().is_empty())
                    .then(|| open.selected.min(open.blocks().len() - 1) + 1);
                Some((open.id.clone(), open.detail.run.task.clone(), step))
            }
            None => self
                .selected_run()
                .map(|run| (run.id(), run.task.clone(), None)),
        }
    }

    /// Handles `x`, `v`, and `u`, and every key while the composer is
    /// open. Returns whether it took the key.
    pub(super) fn mark_key(&mut self, key: Key) -> bool {
        if self.marking.composer.is_some() {
            self.composer_key(key);
            return true;
        }
        self.marking.notice = None;
        match key {
            Key::Char('x') => {
                if let Some((run, task, step)) = self.mark_target() {
                    let existing = self.marking.marks.get(&run, step);
                    let bad = existing.filter(|mark| mark.verdict == Verdict::Bad);
                    let tags = bad
                        .map(|mark| {
                            mark.tags
                                .iter()
                                .filter_map(|tag| {
                                    JUDGMENTS.iter().find(|j| j.id == tag).map(|j| j.id)
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    let note = bad.and_then(|mark| mark.note.clone()).unwrap_or_default();
                    self.marking.composer = Some(Composer {
                        run,
                        task,
                        step,
                        note,
                        tags,
                        cursor: 0,
                    });
                }
                true
            }
            Key::Char('v') => {
                if let Some((run, _, step)) = self.mark_target() {
                    if step.is_some() {
                        self.marking.notice = Some(
                            "only a whole run can be cleared; press t for the summary".to_owned(),
                        );
                    } else {
                        self.save_mark(run, None, Verdict::Clear, Vec::new(), None);
                    }
                }
                true
            }
            Key::Char('u') => {
                if let Some((run, _, step)) = self.mark_target() {
                    let now = runs::now_ms();
                    let author = self.marking.author.clone();
                    self.marking.notice =
                        Some(match self.marking.marks.unmark(&run, step, &author, now) {
                            Ok(true) => match step {
                                Some(step) => format!("removed the mark on step {step}"),
                                None => "removed the run's mark".to_owned(),
                            },
                            Ok(false) => "nothing to remove here".to_owned(),
                            Err(error) => error,
                        });
                    self.after_marking();
                }
                true
            }
            _ => false,
        }
    }

    fn composer_key(&mut self, key: Key) {
        let Some(composer) = &mut self.marking.composer else {
            return;
        };
        match key {
            Key::Char(c) => composer.note.push(c),
            Key::Backspace => {
                composer.note.pop();
            }
            Key::Up => composer.cursor = composer.cursor.saturating_sub(1),
            Key::Down => composer.cursor = (composer.cursor + 1).min(JUDGMENTS.len() - 1),
            Key::PageUp => composer.cursor = composer.cursor.saturating_sub(6),
            Key::PageDown => composer.cursor = (composer.cursor + 6).min(JUDGMENTS.len() - 1),
            Key::Left | Key::Right => {}
            Key::Home => composer.cursor = 0,
            Key::End => composer.cursor = JUDGMENTS.len() - 1,
            Key::Tab => {
                let id = JUDGMENTS[composer.cursor].id;
                if !composer.tags.insert(id) {
                    composer.tags.remove(id);
                }
            }
            Key::Back => self.marking.composer = None,
            Key::Enter => {
                if let Some(composer) = self.marking.composer.take() {
                    let tags = JUDGMENTS
                        .iter()
                        .filter(|j| composer.tags.contains(j.id))
                        .map(|j| j.id.to_owned())
                        .collect();
                    let note = Some(composer.note).filter(|note| !note.trim().is_empty());
                    self.save_mark(composer.run, composer.step, Verdict::Bad, tags, note);
                }
            }
        }
    }

    fn save_mark(
        &mut self,
        run: String,
        step: Option<usize>,
        verdict: Verdict,
        tags: Vec<String>,
        note: Option<String>,
    ) {
        let evidence = self.evidence_of(&run);
        let result = self.marking.marks.mark(NewMark {
            run,
            step,
            verdict,
            tags,
            note,
            evidence,
            author: self.marking.author.clone(),
            at_ms: runs::now_ms(),
        });
        self.marking.notice = Some(match result {
            Ok(mark) => format!("{}; u removes it", mark.describe()),
            Err(error) => error,
        });
        self.after_marking();
    }

    /// The digest of the evidence Jev reads for `run`, as it stands.
    fn evidence_of(&self, run: &str) -> Option<String> {
        let context = Context::new(&self.catalog, self.learning.reference.clone());
        if let Some(open) = self.open.as_ref().filter(|open| open.id == run) {
            return Some(evidence_key(&open.detail, &context));
        }
        let run = self.catalog.runs.iter().find(|r| r.id() == run)?;
        Some(evidence_key(&Detail::load(run), &context))
    }

    /// Reads the marks again, so marks made from the command line show.
    pub(super) fn reload_marks(&mut self) {
        let before = self.marking.marks.records().len();
        self.marking.marks.reload();
        if self.marking.marks.records().len() != before {
            self.after_marking();
        }
    }

    fn after_marking(&mut self) {
        if let Some(open) = &self.open {
            open.invalidate();
        }
    }

    /// The flag the list draws before a marked run's task.
    pub(super) fn mark_flag(&self, run: &str) -> Option<char> {
        self.marking.marks.verdict(run).map(Verdict::flag)
    }

    /// The marks on `run`, in words.
    pub(super) fn mark_lines(&self, run: &str) -> Vec<String> {
        self.marking
            .marks
            .of_run(run)
            .into_iter()
            .map(|mark| format!("{} {}", mark.verdict.flag(), mark.describe()))
            .collect()
    }

    /// The mark on step `index` of the open run, counted from 0, in words.
    pub(super) fn step_mark(&self, run: &str, index: usize) -> Option<String> {
        self.marking
            .marks
            .get(run, Some(index + 1))
            .map(|mark: &Mark| format!("{} {}", mark.verdict.flag(), mark.describe()))
    }

    /// Draws the composer over the lower part of `area`.
    pub(super) fn render_composer(&self, area: Rect, buf: &mut Buffer) {
        let Some(composer) = &self.marking.composer else {
            return;
        };
        let height = area.height.saturating_sub(4).clamp(6, 16);
        if area.height < height + 2 || area.width < 40 {
            return;
        }
        let boxed = Rect::new(
            area.left() + 1,
            area.bottom() - height - 1,
            area.width - 2,
            height,
        );
        let field = self.style(Intensity::Half);
        for y in boxed.top()..boxed.bottom() {
            for x in boxed.left()..boxed.right() {
                buf[(x, y)].reset();
                buf[(x, y)].set_style(field);
            }
        }
        frame(boxed, buf, self.style(Intensity::ThreeQuarters));
        let title = match composer.step {
            Some(step) => format!("Mark step {step} of {} as bad", composer.task),
            None => format!("Mark {} as bad", composer.task),
        };
        rail(
            boxed,
            buf,
            0,
            Some((&title, self.style(Intensity::Full))),
            Some((
                &format!("{} tagged", composer.tags.len()),
                self.style(Intensity::Half),
            )),
        );
        rail(
            boxed,
            buf,
            boxed.height - 1,
            Some((
                "type a note · ↑↓ judgment · tab tags it · enter saves · esc cancels",
                self.style(Intensity::ThreeQuarters),
            )),
            None,
        );
        let inner = Rect::new(
            boxed.left() + 2,
            boxed.top() + 1,
            boxed.width.saturating_sub(4),
            boxed.height.saturating_sub(2),
        );
        let width = usize::from(inner.width);
        buf.set_stringn(
            inner.left(),
            inner.top(),
            format!("note: {}▏", composer.note),
            width,
            self.style(Intensity::Full),
        );
        buf.set_stringn(
            inner.left(),
            inner.top() + 1,
            "What went wrong? Tag the judgments it shows:",
            width,
            self.style(Intensity::Half),
        );
        let rows = usize::from(inner.height.saturating_sub(2));
        if rows == 0 {
            return;
        }
        let first = composer
            .cursor
            .saturating_sub(rows / 2)
            .min(JUDGMENTS.len().saturating_sub(rows));
        for (offset, (index, judgment)) in JUDGMENTS
            .iter()
            .enumerate()
            .skip(first)
            .take(rows)
            .enumerate()
        {
            let here = index == composer.cursor;
            let tagged = composer.tags.contains(judgment.id);
            let line = format!(
                "{} [{}] {:<18} {}",
                if here { '▸' } else { ' ' },
                if tagged { 'x' } else { ' ' },
                judgment.id,
                judgment.tag
            );
            let mut style = self.style(if here || tagged {
                Intensity::Full
            } else {
                Intensity::ThreeQuarters
            });
            if here {
                style = style.bg(self.ladder.selection());
            }
            if tagged {
                style = style.add_modifier(Modifier::BOLD);
            }
            buf.set_stringn(
                inner.left(),
                inner.top() + 2 + offset as u16,
                line,
                width,
                style,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::pane;
    use super::*;

    fn type_text(pane: &mut Pane, text: &str) {
        for c in text.chars() {
            pane.key(Key::Char(c));
        }
    }

    /// Moves the list's cursor to the run whose task is `task`.
    fn select(pane: &mut Pane, task: &str) {
        pane.key(Key::Home);
        for _ in 0..pane.catalog.runs.len() {
            if pane.selected_run().is_some_and(|run| run.task == task) {
                return;
            }
            pane.key(Key::Down);
        }
        panic!("no {task} run in the list");
    }

    #[test]
    fn x_marks_a_run_and_a_step_and_the_marks_survive_a_restart() {
        let state = tempfile::tempdir().unwrap();
        let store = || Marks::open(Some(state.path().to_path_buf()));
        let (_fixtures, pane) = pane();
        let mut pane = pane.with_marks(store(), "tester".to_owned());

        // Mark a run from the list, with a note and two tags.
        select(&mut pane, "wal-recovery-ordering");
        pane.key(Key::Char('x'));
        assert!(pane.composing());
        let text = pane.to_text(140, 40);
        assert!(text.contains("Mark wal-recovery-ordering as bad"), "{text}");
        assert!(text.contains("tab tags it"), "{text}");
        assert!(text.contains("[ ] near_miss"), "{text}");
        // Letters, j and k among them, go into the note.
        type_text(&mut pane, "said the tests passed; jk");
        pane.key(Key::Backspace);
        pane.key(Key::Backspace);
        pane.key(Key::Tab);
        for _ in 0..6 {
            pane.key(Key::Down);
        }
        pane.key(Key::Tab);
        let text = pane.to_text(140, 40);
        assert!(text.contains("note: said the tests passed; ▏"), "{text}");
        assert!(text.contains("[x] near_miss"), "{text}");
        assert!(text.contains("[x] unearned_success"), "{text}");
        assert!(text.contains("2 tagged"), "{text}");
        pane.key(Key::Enter);
        assert!(!pane.composing());
        let wal = pane.selected_run().unwrap().id();
        let mark = pane.marks().get(&wal, None).unwrap().clone();
        assert_eq!(mark.tags, vec!["near_miss", "unearned_success"]);
        assert_eq!(mark.note.as_deref(), Some("said the tests passed;"));
        assert_eq!(mark.author, "tester");
        assert!(mark.evidence.is_some());
        let text = pane.to_text(140, 30);
        assert!(text.contains("⚑ wal-recovery-ordering"), "{text}");
        assert!(
            text.contains("Marked bad: near_miss, unearned_success"),
            "{text}"
        );
        assert!(text.contains("u removes it"), "{text}");

        // Esc cancels a composer without a mark.
        select(&mut pane, "coq-block-bound");
        let coq = pane.selected_run().unwrap().id();
        pane.key(Key::Char('x'));
        type_text(&mut pane, "never mind");
        pane.key(Key::Back);
        assert!(!pane.composing());
        assert!(pane.marks().get(&coq, None).is_none());

        // v clears a run in one keystroke.
        pane.key(Key::Char('v'));
        assert_eq!(pane.marks().verdict(&coq), Some(Verdict::Clear));
        assert!(pane.to_text(140, 30).contains("⚐ coq-block-bound"));

        // In a transcript, x marks the selected step.
        pane.key(Key::Char('t'));
        pane.key(Key::Down);
        pane.key(Key::Char('x'));
        let text = pane.to_text(140, 40);
        assert!(
            text.contains("Mark step 2 of coq-block-bound as bad"),
            "{text}"
        );
        pane.key(Key::End);
        pane.key(Key::Tab);
        pane.key(Key::Enter);
        assert_eq!(
            pane.marks().get(&coq, Some(2)).unwrap().tags,
            vec!["surprise"]
        );
        let text = pane.to_text(140, 40);
        assert!(
            text.contains("⚑ Step 2 marked bad: surprise (tester,"),
            "{text}"
        );
        // v on a step says only a run can be cleared.
        pane.key(Key::Char('v'));
        assert!(
            pane.to_text(140, 40)
                .contains("only a whole run can be cleared")
        );
        // The summary lists both marks.
        pane.key(Key::Char('t'));
        let text = pane.to_text(140, 80);
        assert!(text.contains("Marks"), "{text}");
        assert!(text.contains("Cleared: nothing wrong (tester,"), "{text}");
        assert!(text.contains("Step 2 marked bad: surprise"), "{text}");

        // A new pane over the same store has the marks.
        let (_again, pane) = pane_again();
        let mut pane = pane.with_marks(store(), "tester".to_owned());
        assert_eq!(pane.marks().len(), 3);
        assert!(pane.to_text(140, 30).contains("⚑ wal-recovery-ordering"));

        // u removes the step's mark in the transcript, and the run's in the
        // list.
        select(&mut pane, "coq-block-bound");
        pane.key(Key::Char('t'));
        pane.key(Key::Down);
        pane.key(Key::Char('u'));
        assert!(pane.to_text(140, 40).contains("removed the mark on step 2"));
        pane.key(Key::Back);
        pane.key(Key::Char('u'));
        assert!(pane.to_text(140, 30).contains("removed the run's mark"));
        pane.key(Key::Char('u'));
        assert!(pane.to_text(140, 30).contains("nothing to remove here"));
        assert_eq!(store().len(), 1);
    }

    fn pane_again() -> (tempfile::TempDir, Pane) {
        pane()
    }

    #[test]
    fn a_search_takes_x_as_a_letter() {
        let (_fixtures, mut pane) = pane();
        pane.key(Key::Char('/'));
        pane.key(Key::Char('x'));
        assert!(!pane.composing());
        assert_eq!(pane.filter.search, "x");
    }
}
