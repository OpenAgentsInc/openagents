//! Statements written back as source: one line each, in the form the parser
//! reads, so a program can be merged, pruned, and sent again.

use std::fmt::{self, Write as _};

use crate::lex::{Expr, Statement};

impl fmt::Display for Statement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} = {}", self.name, self.expr)
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Str(text) => quoted(f, text),
            Expr::Num(value) => write!(f, "{value}"),
            Expr::Bool(value) => write!(f, "{value}"),
            Expr::Null => f.write_str("null"),
            Expr::Ref(name) => f.write_str(name),
            Expr::Array(items) => {
                f.write_char('[')?;
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{item}")?;
                }
                f.write_char(']')
            }
            Expr::Object(fields) => {
                f.write_char('{')?;
                for (index, (key, value)) in fields.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    if ident(key) {
                        f.write_str(key)?;
                    } else {
                        quoted(f, key)?;
                    }
                    write!(f, ": {value}")?;
                }
                f.write_char('}')
            }
            Expr::Call { name, args, named } => {
                write!(f, "{name}(")?;
                let mut first = true;
                for arg in args {
                    if !first {
                        f.write_str(", ")?;
                    }
                    first = false;
                    write!(f, "{arg}")?;
                }
                for (key, value) in named {
                    if !first {
                        f.write_str(", ")?;
                    }
                    first = false;
                    write!(f, "{key}={value}")?;
                }
                f.write_char(')')
            }
        }
    }
}

/// Whether `word` reads back as a name (an object key needing no quotes).
fn ident(word: &str) -> bool {
    let mut chars = word.chars();
    chars
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$')
        && chars.all(|c| c.is_alphanumeric() || c == '_' || c == '$')
        && !matches!(word, "true" | "false" | "null")
}

/// `text` in double quotes, escaped so it stays on one line.
fn quoted(f: &mut fmt::Formatter<'_>, text: &str) -> fmt::Result {
    f.write_char('"')?;
    for c in text.chars() {
        match c {
            '"' => f.write_str("\\\"")?,
            '\\' => f.write_str("\\\\")?,
            '\n' => f.write_str("\\n")?,
            '\t' => f.write_str("\\t")?,
            '\r' => f.write_str("\\r")?,
            c if c.is_control() => write!(f, "\\u{:04x}", u32::from(c))?,
            c => f.write_char(c)?,
        }
    }
    f.write_char('"')
}
