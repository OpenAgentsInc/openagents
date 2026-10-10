//! Diagnostics fed back to the model, and line-level repair (#11113).
//!
//! When a finished block has problems, the model is told exactly what was
//! dropped and shown only the lines involved ([`Feedback::prompt`]), so it
//! does not repeat the same invalid arguments and does not rewrite the
//! whole program. Its reply is a few corrected statements, merged back by
//! name ([`repair`], the same rule as a follow-up edit) and checked again.
//! The method follows OpenUI's published repair pass (diagnostics plus the
//! broken lines to a model, then re-validate); nothing of theirs is used.

use crate::edit::{self, Edit};
use crate::embed::{self, Segment};
use crate::lex;
use crate::tree::{Diagnostic, Document};

/// What a model needs to fix a block: the problems and the lines they are
/// about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Feedback {
    pub problems: Vec<Diagnostic>,
    /// The source of each statement a problem names, in program order.
    pub lines: Vec<String>,
}

/// The feedback for `document`, parsed from `source`; `None` when it parsed
/// clean.
#[must_use]
pub fn feedback(source: &str, document: &Document) -> Option<Feedback> {
    if document.diagnostics.is_empty() {
        return None;
    }
    let named: Vec<&str> = document
        .diagnostics
        .iter()
        .filter_map(|d| d.statement.as_deref())
        .collect();
    let mut lines: Vec<String> = Vec::new();
    for (text, parsed) in lex::program(source) {
        let name = match &parsed {
            Ok(statement) => Some(statement.name.as_str()),
            Err(_) => lex::head(text),
        };
        if name.is_some_and(|name| named.contains(&name)) && !lines.iter().any(|l| l == text) {
            lines.push(text.to_owned());
        }
    }
    Some(Feedback {
        problems: document.diagnostics.clone(),
        lines,
    })
}

impl Feedback {
    /// The message to the model: each problem, the lines involved, and
    /// how to answer.
    #[must_use]
    pub fn prompt(&self) -> String {
        let mut out = String::from(
            "Your ```openui-lang block had problems, so parts of it were not shown. Problems:\n",
        );
        for problem in &self.problems {
            match &problem.statement {
                Some(name) => out.push_str(&format!("- `{name}`: {}\n", problem.message)),
                None => out.push_str(&format!("- {}\n", problem.message)),
            }
        }
        if !self.lines.is_empty() {
            out.push_str("The lines involved:\n```openui-lang\n");
            for line in &self.lines {
                out.push_str(line);
                out.push('\n');
            }
            out.push_str("```\n");
        }
        out.push_str(
            "Reply with only a ```openui-lang block of corrected statements. Keep each name; a \
             statement replaces the one with its name, and `name = null` removes one. Do not \
             repeat lines that were fine, and use only the components listed earlier.",
        );
        out
    }
}

/// Merges a model's corrections into `source` by name. `reply` is the
/// model's answer: a ```` ```openui-lang ```` block, or bare statements.
/// The caller parses [`Edit::source`] again and, if it still has problems,
/// keeps whichever version drew more.
#[must_use]
pub fn repair(source: &str, reply: &str) -> Edit {
    let block = embed::segments(reply)
        .into_iter()
        .find_map(|segment| match segment {
            Segment::Ui { source, .. } => Some(source),
            Segment::Markdown(_) => None,
        });
    edit::apply(source, block.unwrap_or(reply))
}
