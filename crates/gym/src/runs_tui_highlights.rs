//! The Highlights view in the Runs pane: `h` lists the candidate claims
//! `gym runs highlights` computes, strongest first, with the selected
//! claim's numbers, sample size, and caveats below the list.
//!
//! `Enter` opens the cited runs: the list shows only the runs the claim
//! cites, or the run itself when it cites one. `h` or `Esc` goes back to
//! the list. Nothing here posts anywhere; a person picks, edits, and posts.

use std::cell::Cell;
use std::collections::HashMap;

use coder_terminal::Intensity;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use super::{Key, Pane, Reply, Tab};
use crate::runs::Filter;
use crate::runs_highlights::{Highlight, Inputs, compute};
use crate::runs_learning::Answer;
use crate::runs_story;

/// The claims as the pane holds them.
#[derive(Default)]
pub(super) struct Highlighting {
    pub(super) showing: bool,
    pub(super) claims: Vec<Highlight>,
    cursor: usize,
    scroll: Cell<usize>,
}

impl Pane {
    /// Whether the Highlights view is showing.
    #[must_use]
    pub fn showing_highlights(&self) -> bool {
        self.highlighting.showing
    }

    /// Computes the claims from the runs, Jev's answers, the leaderboard,
    /// and the marks as they stand.
    fn compute_highlights(&mut self) {
        let answers: HashMap<String, &Answer> = self
            .learning
            .answers
            .iter()
            .map(|(id, answer)| (id.clone(), answer))
            .collect();
        self.highlighting.claims = compute(&Inputs {
            runs: &self.catalog.runs,
            answers: &answers,
            reference: self.learning.reference.as_ref(),
            marks: &self.marking.marks,
        });
        self.highlighting.cursor = 0;
        self.highlighting.scroll.set(0);
    }

    /// Handles `h` from the list, and every key while the view shows.
    /// Returns the reply when it took the key.
    pub(super) fn highlight_key(&mut self, key: Key) -> Option<Reply> {
        if !self.highlighting.showing {
            if key == Key::Char('h') && self.open.is_none() {
                self.compute_highlights();
                self.highlighting.showing = true;
                return Some(Reply::Handled);
            }
            return None;
        }
        let last = self.highlighting.claims.len().saturating_sub(1);
        let cursor = &mut self.highlighting.cursor;
        match key {
            Key::Up | Key::Char('k') => *cursor = cursor.saturating_sub(1),
            Key::Down | Key::Char('j') => *cursor = (*cursor + 1).min(last),
            Key::PageUp => *cursor = cursor.saturating_sub(10),
            Key::PageDown => *cursor = (*cursor + 10).min(last),
            Key::Home | Key::Char('g') => *cursor = 0,
            Key::End | Key::Char('G') => *cursor = last,
            Key::Back | Key::Char('h') => self.highlighting.showing = false,
            Key::Char('q') => return Some(Reply::Quit),
            Key::Enter => self.open_cited(),
            _ => {}
        }
        Some(Reply::Handled)
    }

    /// Shows the runs the selected claim cites, and opens the run when it
    /// cites one.
    fn open_cited(&mut self) {
        let Some(claim) = self.highlighting.claims.get(self.highlighting.cursor) else {
            return;
        };
        self.filter = Filter {
            runs: claim.runs.clone(),
            cited_by: Some(claim.key.clone()),
            ..Filter::default()
        };
        self.highlighting.showing = false;
        self.cursor = 0;
        if self.visible().len() == 1 {
            self.open_selected(Tab::Summary);
        }
    }

    pub(super) fn render_highlights(&self, area: Rect, buf: &mut Buffer) {
        let claims = &self.highlighting.claims;
        let n1 = claims.iter().filter(|claim| claim.n1).count();
        self.header(
            area,
            buf,
            &[("Terminal-Bench runs".to_owned(), Intensity::Full)],
            &format!("{} runs", self.catalog.runs.len()),
        );
        let title = format!(
            "Highlights · {} candidate claims, strongest first",
            claims.len()
        );
        let hint = format!("{n1} rest on one run · computed by rules · nothing posts");
        let inner = self.framed(
            area,
            buf,
            (&title, &hint),
            (
                "↑↓ move · enter open the cited runs · h or esc back · q quit",
                "↑↓ enter h q",
            ),
        );
        if inner.height < 6 {
            return;
        }
        let width = usize::from(inner.width);
        if claims.is_empty() {
            buf.set_stringn(
                inner.left(),
                inner.top(),
                "No claim meets a rule yet. Rank runs with l, or run more arms of the same tasks.",
                width,
                self.style(Intensity::Half),
            );
            return;
        }
        // The list, a rule, and the selected claim in full.
        let detail_rows = (inner.height / 2).max(4);
        let list_rows = usize::from(inner.height - detail_rows - 1);
        let cursor = self.highlighting.cursor.min(claims.len() - 1);
        let mut scroll = self.highlighting.scroll.get();
        if cursor < scroll {
            scroll = cursor;
        } else if cursor >= scroll + list_rows {
            scroll = cursor + 1 - list_rows;
        }
        self.highlighting.scroll.set(scroll);
        for (offset, claim) in claims.iter().enumerate().skip(scroll).take(list_rows) {
            let y = inner.top() + (offset - scroll) as u16;
            let selected = offset == cursor;
            let mut style = self.style(if selected {
                Intensity::Full
            } else if claim.n1 {
                Intensity::Half
            } else {
                Intensity::ThreeQuarters
            });
            if selected {
                style = style.bg(self.ladder.selection());
                for x in inner.left() - 1..inner.right() {
                    buf[(x, y)].set_style(ratatui::style::Style::new().bg(self.ladder.selection()));
                }
                buf[(inner.left() - 1, y)].set_char('▸').set_style(
                    self.ladder
                        .style(Intensity::Full)
                        .bg(self.ladder.selection()),
                );
            }
            let line = format!(
                "{:>2}  {:<14} {:<5} {}",
                offset + 1,
                claim.rule.word(),
                if claim.n1 {
                    "n=1".to_owned()
                } else {
                    format!("n={}", claim.sample)
                },
                claim.claim
            );
            buf.set_stringn(inner.left(), y, line, width, style);
        }
        let rule_y = inner.top() + list_rows as u16;
        for x in inner.left()..inner.right() {
            buf[(x, rule_y)]
                .set_char('─')
                .set_style(self.style(Intensity::Quarter));
        }
        let claim = &claims[cursor];
        let mut lines: Vec<(String, Intensity)> = runs_story::wrap(&claim.claim, width)
            .into_iter()
            .map(|line| (line, Intensity::Full))
            .collect();
        lines.push((
            format!(
                "{} · {} · {} cited runs · score {:.2}",
                claim.key,
                claim.sample_label(),
                claim.runs.len(),
                claim.score
            ),
            Intensity::Half,
        ));
        for caveat in &claim.caveats {
            for (index, line) in runs_story::wrap(caveat, width.saturating_sub(2))
                .into_iter()
                .enumerate()
            {
                let lead = if index == 0 { "– " } else { "  " };
                lines.push((format!("{lead}{line}"), Intensity::ThreeQuarters));
            }
        }
        for (offset, (line, intensity)) in lines.iter().take(usize::from(detail_rows)).enumerate() {
            buf.set_stringn(
                inner.left(),
                rule_y + 1 + offset as u16,
                line,
                width,
                self.style(*intensity),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::pane;
    use super::*;
    use crate::runs_highlights::Rule;

    fn claim(key: &str, runs: Vec<String>, n1: bool) -> Highlight {
        Highlight {
            key: key.to_owned(),
            rule: Rule::Cost,
            claim: format!("Claim {key}: an arm passed for 20% of what another spent."),
            task: None,
            runs,
            numbers: Vec::new(),
            sample: if n1 { 1 } else { 3 },
            n1,
            caveats: vec![
                "The arms ran in different batches (tb4 and panel), not side by side.".to_owned(),
            ],
            strength: 0.5,
            score: 0.5,
        }
    }

    #[test]
    fn h_lists_the_claims_and_enter_opens_the_cited_runs() {
        let (_fixtures, mut pane) = pane();
        let ids: Vec<String> = pane.catalog.runs.iter().map(|run| run.id()).collect();
        assert!(ids.len() >= 3, "{ids:?}");
        assert_eq!(pane.key(Key::Char('h')), Reply::Handled);
        assert!(pane.showing_highlights());
        pane.highlighting.claims = vec![
            claim("cost-aaaa", vec![ids[0].clone(), ids[1].clone()], false),
            claim("cost-bbbb", vec![ids[2].clone()], true),
        ];
        let text = pane.to_text(140, 30);
        assert!(text.contains("Highlights · 2 candidate claims"), "{text}");
        assert!(text.contains("1 rest on one run"), "{text}");
        assert!(text.contains("Claim cost-aaaa"), "{text}");
        assert!(text.contains("different batches"), "{text}");
        assert!(text.contains("enter open the cited runs"), "{text}");
        // Keys that mark or ask don't reach the list under the view.
        pane.key(Key::Char('x'));
        assert!(!pane.composing());

        // Enter shows the two cited runs.
        pane.key(Key::Enter);
        assert!(!pane.showing_highlights());
        assert_eq!(pane.visible().len(), 2);
        let text = pane.to_text(140, 30);
        assert!(text.contains("the 2 runs cost-aaaa cites"), "{text}");
        pane.key(Key::Char('c'));
        assert_eq!(pane.visible().len(), ids.len());

        // A claim that cites one run opens it.
        pane.key(Key::Char('h'));
        pane.highlighting.claims = vec![
            claim("cost-aaaa", vec![ids[0].clone(), ids[1].clone()], false),
            claim("cost-bbbb", vec![ids[2].clone()], true),
        ];
        pane.key(Key::Down);
        pane.key(Key::Enter);
        assert!(pane.is_open());
        pane.key(Key::Back);
        assert!(!pane.is_open());

        // Esc and h leave the view.
        pane.key(Key::Char('h'));
        pane.key(Key::Back);
        assert!(!pane.showing_highlights());
    }

    #[test]
    fn with_no_claim_the_view_says_why() {
        let (_fixtures, mut pane) = pane();
        pane.key(Key::Char('h'));
        pane.highlighting.claims.clear();
        let text = pane.to_text(140, 30);
        assert!(text.contains("No claim meets a rule yet"), "{text}");
    }
}
