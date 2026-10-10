//! OpenUI Lang, the declarative subset our chat answers use for inline
//! components (#11113, phase 1).
//!
//! An answer is Markdown prose with one fenced ```` ```openui-lang ```` block
//! after a one-line lead. The block is one statement per line:
//!
//! ```text
//! root = Columns([web, computer])
//! web = Card("On the web", [Text("Connect GitHub and pick a repository."), Button("Connect GitHub", href="/projects")])
//! computer = Card("On your computer", [Command("curl -fsSL https://openagents.com/cli/install.sh | bash", windows="irm https://openagents.com/cli/install.ps1 | iex")])
//! ```
//!
//! The syntax is OpenUI Lang (spec v0.1, from Thesys's MIT-licensed
//! `openui`; our profile also accepts named arguments, `key = value` or
//! `key: value`, beside positional ones). This crate is our own Rust
//! implementation of its rules, not a port of their code:
//!
//! - A newline outside a string and outside brackets ends a statement. A
//!   name may be used before the statement that defines it (a forward
//!   reference); until then it draws nothing. A later statement with the
//!   same name replaces the earlier one (merge by name).
//! - While streaming ([`Stream`]), finished statements are parsed once and
//!   cached; only the unfinished tail is parsed again on each update. The
//!   tail is cut before a string still being written and its brackets are
//!   closed, so no half-written text or link shows. A finished statement
//!   is never replaced by a tail that still needed closing.
//! - Validation is against [`catalog::CATALOG`]: an unknown component or
//!   argument is dropped, an invalid list item is pruned and its siblings
//!   kept, a bad choice falls back to the default, and an unsafe link is
//!   dropped. Missing required arguments and names never defined are
//!   reported only once the program is complete. Every fix is a
//!   [`Diagnostic`], never shown to a reader.
//! - Nothing here runs code or calls tools: there is no `Query`,
//!   `Mutation`, state, or script. A button is a link to a page that
//!   starts the flow there.
//!
//! [`embed`] finds the blocks in Markdown and writes the Markdown fallback
//! (prose with links, numbered steps, and commands as code blocks) for
//! surfaces that cannot draw the components.
//!
//! [`edit`] merges a follow-up turn's statements into the earlier program
//! by name (`name = null` removes one; what `root` no longer reaches is
//! dropped) and gives the patch a surface showing the earlier interface
//! needs. [`feedback`] turns a block's diagnostics and only the lines they
//! name into a message for the model, and merges its corrections back.

pub mod catalog;
pub mod edit;
pub mod embed;
pub mod feedback;
mod lex;
mod print;
mod tree;

pub use catalog::prompt;
pub use edit::Edit;
pub use feedback::Feedback;
pub use lex::{Expr, Statement};
pub use tree::{Audience, ButtonStyle, Diagnostic, Document, Node, Step, Tab, safe_href};

/// The fence's info string.
pub const LANG: &str = "openui-lang";

/// Parses a whole program.
#[must_use]
pub fn parse(source: &str) -> Document {
    let mut stream = Stream::default();
    stream.finish(source)
}

/// Parses a program still being written: [`parse`] with the streaming
/// rules (see the crate docs).
#[must_use]
pub fn parse_partial(source: &str) -> Document {
    let mut stream = Stream::default();
    stream.update(source)
}

/// A program read as it streams in: finished statements are parsed once.
#[derive(Clone, Debug, Default)]
pub struct Stream {
    /// The source up to the end of the last finished statement.
    settled: String,
    table: tree::Table,
    /// Diagnostics from parsing the finished statements.
    diagnostics: Vec<Diagnostic>,
}

impl Stream {
    /// The document for `source`, the whole program so far. Text that
    /// extends what came before reuses the finished statements; any other
    /// text starts over.
    pub fn update(&mut self, source: &str) -> Document {
        self.read(source, false)
    }

    /// The document for the whole program.
    pub fn finish(&mut self, source: &str) -> Document {
        self.read(source, true)
    }

    fn read(&mut self, source: &str, complete: bool) -> Document {
        if !source.starts_with(&self.settled) {
            *self = Self::default();
        }
        let base = self.settled.len();
        let (finished, tail, tail_at) = lex::split(&source[base..]);
        tree::admit(&finished, &mut self.table, &mut self.diagnostics);
        self.settled = source[..base + tail_at].to_owned();
        let mut diagnostics = self.diagnostics.clone();
        let mut table = self.table.clone();
        if !tail.trim().is_empty() {
            if complete {
                tree::admit(&[tail], &mut table, &mut diagnostics);
            } else {
                let (closed, needed) = lex::autoclose(tail);
                // A tail that fails to parse yet is still being written.
                if let Ok(statement) = lex::Statement::parse(&closed)
                    && !(needed && table.contains_key(&statement.name))
                {
                    table.insert(statement.name.clone(), statement);
                }
            }
        }
        let root = tree::materialize(&table, complete, &mut diagnostics);
        Document { root, diagnostics }
    }
}

impl Statement {
    fn parse(text: &str) -> Result<Self, String> {
        lex::statement(text)
    }
}

#[cfg(test)]
mod tests;
