//! Follow-up edits to an answer's interface, merged by name (#11113).
//!
//! A follow-up turn need not write the whole program again. It writes only
//! the statements that change, and [`apply`] merges them into the earlier
//! program:
//!
//! - a statement whose name is new is added;
//! - a statement whose name exists replaces it (merge by name);
//! - `name = null` removes `name`;
//! - a statement the edit does not mention is kept;
//! - afterwards, any statement no longer reachable from `root` is dropped.
//!
//! The result is one program, written back top-down from `root`, so it
//! streams and parses like any other. [`Edit::patch`] is the smallest
//! program that turns the earlier one into the result, which is what a
//! surface that already shows the earlier interface needs to be sent: the
//! changed statements, not the whole program again.

use std::collections::{BTreeSet, VecDeque};

use crate::lex::{self, Expr, Statement};
use crate::tree::Diagnostic;

/// The program after an edit, and what the edit did, compared with the
/// earlier program.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Edit {
    /// The merged program: `root` first, then each statement in the order
    /// it is first reached, one per line.
    pub source: String,
    /// Names the result has that the earlier program did not.
    pub added: Vec<String>,
    /// Names whose statement now reads differently.
    pub replaced: Vec<String>,
    /// Names of the earlier program the edit removed with `name = null`.
    pub removed: Vec<String>,
    /// Names of the earlier program that `root` no longer reaches.
    pub dropped: Vec<String>,
    /// Lines of either program that could not be read, removals of names
    /// that were not there, and new statements nothing reaches. Never
    /// shown to a reader.
    pub diagnostics: Vec<Diagnostic>,
}

impl Edit {
    /// Whether the edit changed the program at all.
    #[must_use]
    pub fn changed(&self) -> bool {
        !(self.added.is_empty()
            && self.replaced.is_empty()
            && self.removed.is_empty()
            && self.dropped.is_empty())
    }

    /// The statements a surface showing the earlier program needs: each
    /// added or replaced statement as it now reads, then `name = null` for
    /// each name removed or dropped. Applying it to the earlier program
    /// gives [`Edit::source`].
    #[must_use]
    pub fn patch(&self) -> String {
        let statements = lex::program(&self.source);
        let mut out = String::new();
        for (_, parsed) in &statements {
            if let Ok(statement) = parsed
                && (self.added.contains(&statement.name) || self.replaced.contains(&statement.name))
            {
                out.push_str(&statement.to_string());
                out.push('\n');
            }
        }
        for name in self.removed.iter().chain(&self.dropped) {
            out.push_str(name);
            out.push_str(" = null\n");
        }
        out
    }
}

/// Merges `change` into `previous` by name, then drops what `root` no
/// longer reaches (see the module docs). Both are whole programs: no
/// streaming rules apply.
#[must_use]
pub fn apply(previous: &str, change: &str) -> Edit {
    let mut edit = Edit::default();
    let mut before: Vec<Statement> = Vec::new();
    for (text, parsed) in lex::program(previous) {
        match parsed {
            Ok(statement) => put(&mut before, statement),
            Err(message) => edit.diagnostics.push(Diagnostic {
                statement: lex::head(text).map(str::to_owned),
                message: format!("a line of the earlier program was dropped: {message}"),
            }),
        }
    }
    let mut table = before.clone();
    for (text, parsed) in lex::program(change) {
        let statement = match parsed {
            Ok(statement) => statement,
            Err(message) => {
                let name = lex::head(text).map(str::to_owned);
                let kept = name
                    .as_deref()
                    .filter(|name| table.iter().any(|s| s.name == *name))
                    .map(|name| format!("; the earlier `{name}` is kept"))
                    .unwrap_or_default();
                edit.diagnostics.push(Diagnostic {
                    statement: name,
                    message: format!("a line of the edit was dropped: {message}{kept}"),
                });
                continue;
            }
        };
        if matches!(statement.expr, Expr::Null) {
            let at = table.iter().position(|s| s.name == statement.name);
            match at {
                Some(at) => {
                    table.remove(at);
                }
                None => edit.diagnostics.push(Diagnostic {
                    statement: Some(statement.name.clone()),
                    message: format!("`{}` was removed but never defined", statement.name),
                }),
            }
        } else {
            put(&mut table, statement);
        }
    }

    let kept: Vec<&Statement> = match reachable(&table) {
        Some(order) => order
            .iter()
            .filter_map(|name| table.iter().find(|s| s.name == *name))
            .collect(),
        None => {
            if !table.is_empty() {
                edit.diagnostics.push(Diagnostic {
                    statement: None,
                    message: "there is no `root` statement, so nothing was pruned".into(),
                });
            }
            table.iter().collect()
        }
    };
    let is_kept = |name: &str| kept.iter().any(|s| s.name == name);
    for statement in &kept {
        edit.source.push_str(&statement.to_string());
        edit.source.push('\n');
        match before.iter().find(|s| s.name == statement.name) {
            None => edit.added.push(statement.name.clone()),
            Some(earlier) if earlier.expr != statement.expr => {
                edit.replaced.push(statement.name.clone());
            }
            Some(_) => {}
        }
    }
    for statement in &before {
        if is_kept(&statement.name) {
            continue;
        }
        if table.iter().any(|s| s.name == statement.name) {
            edit.dropped.push(statement.name.clone());
        } else {
            edit.removed.push(statement.name.clone());
        }
    }
    for statement in &table {
        if !is_kept(&statement.name) && !before.iter().any(|s| s.name == statement.name) {
            edit.diagnostics.push(Diagnostic {
                statement: Some(statement.name.clone()),
                message: format!(
                    "`{}` was added but nothing from `root` uses it, so it was dropped",
                    statement.name
                ),
            });
        }
    }
    edit
}

/// Adds `statement`, or replaces the one with its name in place.
fn put(table: &mut Vec<Statement>, statement: Statement) {
    match table.iter_mut().find(|s| s.name == statement.name) {
        Some(slot) => *slot = statement,
        None => table.push(statement),
    }
}

/// The names reachable from `root`, breadth first in the order written;
/// `None` when there is no `root`. A reference to a name never defined is
/// skipped here; validation reports it.
fn reachable(table: &[Statement]) -> Option<Vec<&str>> {
    table.iter().find(|s| s.name == "root")?;
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut order = Vec::new();
    let mut queue = VecDeque::from(["root"]);
    while let Some(name) = queue.pop_front() {
        if !seen.insert(name) {
            continue;
        }
        let Some(statement) = table.iter().find(|s| s.name == name) else {
            continue;
        };
        order.push(statement.name.as_str());
        let mut next = Vec::new();
        lex::refs(&statement.expr, &mut next);
        queue.extend(next);
    }
    Some(order)
}
