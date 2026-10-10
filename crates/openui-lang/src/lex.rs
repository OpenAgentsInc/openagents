//! Statements, auto-closing, and the expression parser.
//!
//! A program is one statement per line, `name = Expression`. A newline ends
//! a statement only outside a string and outside brackets, so a long call
//! may span lines. `//` starts a comment outside a string.

/// An expression, as written.
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Str(String),
    Num(f64),
    Bool(bool),
    Null,
    /// A name another statement defines, maybe later (a forward reference).
    Ref(String),
    Array(Vec<Expr>),
    Object(Vec<(String, Expr)>),
    /// `Name(positional, ..., key = value, ...)`. Our profile of the spec
    /// accepts named arguments (`key = value` or `key: value`) beside the
    /// positional ones.
    Call {
        name: String,
        args: Vec<Expr>,
        named: Vec<(String, Expr)>,
    },
}

/// One parsed statement.
#[derive(Clone, Debug, PartialEq)]
pub struct Statement {
    pub name: String,
    pub expr: Expr,
}

/// The finished statements' source texts and the unfinished tail, which is
/// empty when the text ends at a statement boundary.
pub(crate) fn split(source: &str) -> (Vec<&str>, &str, usize) {
    let mut finished = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut comment = false;
    let mut chars = source.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        if comment {
            if c == '\n' {
                comment = false;
            } else {
                continue;
            }
        }
        if let Some(open) = quote {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == open {
                quote = None;
            }
            // A newline inside a string still ends the line: a string never
            // spans lines, so an unclosed one must not swallow the program.
            if c == '\n' {
                quote = None;
                escaped = false;
            } else {
                continue;
            }
        }
        match c {
            '"' | '\'' => quote = Some(c),
            '/' if chars.peek().is_some_and(|(_, next)| *next == '/') => comment = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            '\n' if depth == 0 => {
                let text = &source[start..at];
                if !text.trim().is_empty() {
                    finished.push(text);
                }
                start = at + 1;
            }
            _ => {}
        }
    }
    (finished, &source[start..], start)
}

/// `text` cut before a string still being written, so no half-written
/// text or link ever shows, with its open brackets closed; and whether it
/// needed closing.
pub(crate) fn autoclose(text: &str) -> (String, bool) {
    match open_string(text) {
        Some(at) => (close(&text[..at]).0, true),
        None => close(text),
    }
}

/// Where a string left open at the end of `text` starts.
fn open_string(text: &str) -> Option<usize> {
    let mut quote: Option<(char, usize)> = None;
    let mut escaped = false;
    let mut comment = false;
    let mut chars = text.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        if comment {
            if c == '\n' {
                comment = false;
            }
            continue;
        }
        if let Some((open, _)) = quote {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == open {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => quote = Some((c, at)),
            '/' if chars.peek().is_some_and(|(_, n)| *n == '/') => comment = true,
            _ => {}
        }
    }
    quote.map(|(_, at)| at)
}

fn close(text: &str) -> (String, bool) {
    let mut stack = Vec::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut comment = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if comment {
            if c == '\n' {
                comment = false;
            }
            continue;
        }
        if let Some(open) = quote {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == open {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => quote = Some(c),
            '/' if chars.peek() == Some(&'/') => comment = true,
            '(' => stack.push(')'),
            '[' => stack.push(']'),
            '{' => stack.push('}'),
            ')' | ']' | '}' => {
                if stack.last() == Some(&c) {
                    stack.pop();
                }
            }
            _ => {}
        }
    }
    let incomplete = quote.is_some() || !stack.is_empty();
    if !incomplete {
        return (text.to_owned(), false);
    }
    let mut out = text.to_owned();
    if let Some(open) = quote {
        if escaped {
            out.pop();
        }
        out.push(open);
    }
    // A dangling separator (`Card("a", ` or `href=`) cannot close cleanly;
    // drop it so the rest parses.
    loop {
        let trimmed = out.trim_end();
        if let Some(rest) = trimmed
            .strip_suffix(',')
            .or_else(|| trimmed.strip_suffix('='))
            .or_else(|| trimmed.strip_suffix(':'))
        {
            out = rest.to_owned();
        } else {
            break;
        }
    }
    // A named argument cut after its name (`href`) is dropped with the
    // comma before it; a bare name stays, since it may be a reference.
    while let Some(close) = stack.pop() {
        out.push(close);
    }
    (out, true)
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Ident(String),
    Str(String),
    Num(f64),
    Open(char),
    Close(char),
    Comma,
    Equals,
    Colon,
}

fn tokens(text: &str) -> Result<Vec<Token>, String> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(&c) = chars.peek() {
        match c {
            c if c.is_whitespace() => {
                chars.next();
            }
            '/' => {
                chars.next();
                if chars.peek() == Some(&'/') {
                    for c in chars.by_ref() {
                        if c == '\n' {
                            break;
                        }
                    }
                } else {
                    return Err("a stray `/`".into());
                }
            }
            '(' | '[' | '{' => {
                chars.next();
                out.push(Token::Open(c));
            }
            ')' | ']' | '}' => {
                chars.next();
                out.push(Token::Close(c));
            }
            ',' => {
                chars.next();
                out.push(Token::Comma);
            }
            '=' => {
                chars.next();
                out.push(Token::Equals);
            }
            ':' => {
                chars.next();
                out.push(Token::Colon);
            }
            '"' | '\'' => {
                chars.next();
                out.push(Token::Str(string(&mut chars, c)?));
            }
            c if c.is_ascii_digit() || c == '-' || c == '.' => {
                let mut number = String::new();
                while let Some(&d) = chars.peek() {
                    if d.is_ascii_digit() || matches!(d, '-' | '+' | '.' | 'e' | 'E') {
                        number.push(d);
                        chars.next();
                    } else {
                        break;
                    }
                }
                let value = number
                    .parse::<f64>()
                    .map_err(|_| format!("`{number}` is not a number"))?;
                out.push(Token::Num(value));
            }
            c if c.is_alphabetic() || c == '_' || c == '$' => {
                let mut ident = String::new();
                while let Some(&d) = chars.peek() {
                    if d.is_alphanumeric() || d == '_' || d == '$' {
                        ident.push(d);
                        chars.next();
                    } else {
                        break;
                    }
                }
                out.push(Token::Ident(ident));
            }
            other => return Err(format!("an unexpected `{other}`")),
        }
    }
    Ok(out)
}

fn string(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    quote: char,
) -> Result<String, String> {
    let mut out = String::new();
    while let Some(c) = chars.next() {
        match c {
            c if c == quote => return Ok(out),
            '\\' => match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('u') => {
                    let hex: String = chars.by_ref().take(4).collect();
                    let code = u32::from_str_radix(&hex, 16)
                        .ok()
                        .and_then(char::from_u32)
                        .ok_or_else(|| format!("a bad escape `\\u{hex}`"))?;
                    out.push(code);
                }
                Some(other) => out.push(other),
                None => return Err("a string ends in `\\`".into()),
            },
            '\n' => return Err("a string runs past the end of its line".into()),
            c => out.push(c),
        }
    }
    Err("a string is not closed".into())
}

/// Every statement of a whole program, as its source text (trimmed) and
/// the parse of it; blank and comment-only stretches are skipped.
pub(crate) fn program(source: &str) -> Vec<(&str, Result<Statement, String>)> {
    let (mut texts, tail, _) = split(source);
    texts.push(tail);
    texts
        .into_iter()
        .filter(|text| !blank(text))
        .map(|text| (text.trim(), statement(text)))
        .collect()
}

/// The name a statement's text starts with, even when the rest fails to
/// parse: the word before its first `=`.
pub(crate) fn head(text: &str) -> Option<&str> {
    text.split('=')
        .next()
        .map(str::trim)
        .filter(|name| !name.is_empty() && !name.contains(char::is_whitespace))
}

/// The names `expr` refers to, in the order written.
pub(crate) fn refs<'e>(expr: &'e Expr, out: &mut Vec<&'e str>) {
    match expr {
        Expr::Ref(name) => out.push(name),
        Expr::Array(items) => items.iter().for_each(|item| refs(item, out)),
        Expr::Object(fields) => fields.iter().for_each(|(_, value)| refs(value, out)),
        Expr::Call { args, named, .. } => {
            args.iter().for_each(|arg| refs(arg, out));
            named.iter().for_each(|(_, value)| refs(value, out));
        }
        Expr::Str(_) | Expr::Num(_) | Expr::Bool(_) | Expr::Null => {}
    }
}

/// Whether `text` holds nothing but spaces and comments.
pub(crate) fn blank(text: &str) -> bool {
    tokens(text).is_ok_and(|tokens| tokens.is_empty())
}

/// Parses one statement's text.
pub(crate) fn statement(text: &str) -> Result<Statement, String> {
    let tokens = tokens(text)?;
    let mut parser = Parser { tokens, at: 0 };
    let name = match parser.next() {
        Some(Token::Ident(name)) => name,
        _ => return Err("a statement starts with a name".into()),
    };
    if parser.next() != Some(Token::Equals) {
        return Err(format!("`{name}` is not followed by `=`"));
    }
    let expr = parser.expr()?;
    if parser.at != parser.tokens.len() {
        return Err(format!("`{name}` has text after its expression"));
    }
    Ok(Statement { name, expr })
}

struct Parser {
    tokens: Vec<Token>,
    at: usize,
}

impl Parser {
    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.at).cloned();
        self.at += 1;
        token
    }

    fn peek(&self, ahead: usize) -> Option<&Token> {
        self.tokens.get(self.at + ahead)
    }

    fn expr(&mut self) -> Result<Expr, String> {
        match self.next() {
            Some(Token::Str(text)) => Ok(Expr::Str(text)),
            Some(Token::Num(value)) => Ok(Expr::Num(value)),
            Some(Token::Ident(word)) => match word.as_str() {
                "true" => Ok(Expr::Bool(true)),
                "false" => Ok(Expr::Bool(false)),
                "null" => Ok(Expr::Null),
                _ if self.peek(0) == Some(&Token::Open('(')) => {
                    self.at += 1;
                    self.call(word)
                }
                _ => Ok(Expr::Ref(word)),
            },
            Some(Token::Open('[')) => {
                let mut items = Vec::new();
                loop {
                    if self.peek(0) == Some(&Token::Close(']')) {
                        self.at += 1;
                        return Ok(Expr::Array(items));
                    }
                    items.push(self.expr()?);
                    match self.next() {
                        Some(Token::Comma) => {}
                        Some(Token::Close(']')) => return Ok(Expr::Array(items)),
                        _ => return Err("an array item is not followed by `,` or `]`".into()),
                    }
                }
            }
            Some(Token::Open('{')) => {
                let mut fields = Vec::new();
                loop {
                    if self.peek(0) == Some(&Token::Close('}')) {
                        self.at += 1;
                        return Ok(Expr::Object(fields));
                    }
                    let key = match self.next() {
                        Some(Token::Ident(key) | Token::Str(key)) => key,
                        _ => return Err("an object key is missing".into()),
                    };
                    if self.next() != Some(Token::Colon) {
                        return Err(format!("the object key `{key}` has no `:`"));
                    }
                    fields.push((key, self.expr()?));
                    match self.next() {
                        Some(Token::Comma) => {}
                        Some(Token::Close('}')) => return Ok(Expr::Object(fields)),
                        _ => return Err("an object field is not followed by `,` or `}`".into()),
                    }
                }
            }
            Some(other) => Err(format!("an unexpected {other:?}")),
            None => Err("an expression is missing".into()),
        }
    }

    fn call(&mut self, name: String) -> Result<Expr, String> {
        let mut args = Vec::new();
        let mut named = Vec::new();
        loop {
            if self.peek(0) == Some(&Token::Close(')')) {
                self.at += 1;
                return Ok(Expr::Call { name, args, named });
            }
            let key = match (self.peek(0), self.peek(1)) {
                (Some(Token::Ident(key)), Some(Token::Equals | Token::Colon)) => Some(key.clone()),
                _ => None,
            };
            if let Some(key) = key {
                self.at += 2;
                named.push((key, self.expr()?));
            } else if named.is_empty() {
                args.push(self.expr()?);
            } else {
                return Err(format!(
                    "a positional argument of {name} follows a named one"
                ));
            }
            match self.next() {
                Some(Token::Comma) => {}
                Some(Token::Close(')')) => return Ok(Expr::Call { name, args, named }),
                _ => {
                    return Err(format!(
                        "an argument of {name} is not followed by `,` or `)`"
                    ));
                }
            }
        }
    }
}
