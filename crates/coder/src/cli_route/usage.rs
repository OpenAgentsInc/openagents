//! Reading the `openagents` help text into typed command forms.
//!
//! Each group's `USAGE` string lists its commands as two-space-indented
//! rows: the command words, then positionals (`HOST`), options
//! (`--limit N`, `[--to all|near]`), switches (`[--follow]`), and a
//! trailing command (`-- CMD [ARGS...]`), with the summary after a gap of
//! two or more spaces or on the deeper-indented lines that follow. This
//! module turns those rows into [`Form`]s. It reads only the text the
//! command prints for `--help`, so a command appears here when it appears
//! there.

use serde::{Deserialize, Serialize};

/// One element of a command form, in command-line order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Token {
    /// A command word that must appear as written: `list`, `move`.
    Literal { word: String },
    /// A positional value: `HOST`, `TEXT`, `X,Y,Z`.
    Positional {
        name: String,
        #[serde(default, skip_serializing_if = "is_false")]
        optional: bool,
        #[serde(default, skip_serializing_if = "is_false")]
        repeated: bool,
    },
    /// `--name VALUE` (`value` is the metavar) or a `--name` switch
    /// (`value` is `None`). `choices` lists the lowercase values a closed
    /// enum takes (`--to all|ads|zone`); `open` is true when the value may
    /// also be something else (`standard|admin|all|LIST`).
    Option {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        choices: Vec<String>,
        #[serde(default, skip_serializing_if = "is_false")]
        open: bool,
        #[serde(default, skip_serializing_if = "is_false")]
        optional: bool,
        #[serde(default, skip_serializing_if = "is_false")]
        repeated: bool,
    },
    /// `-- CMD [ARGS...]`: a command line passed through.
    Rest {
        #[serde(default, skip_serializing_if = "is_false")]
        optional: bool,
    },
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_false(value: &bool) -> bool {
    !*value
}

impl Token {
    fn mark_repeated(&mut self) {
        match self {
            Token::Positional { repeated, .. } | Token::Option { repeated, .. } => *repeated = true,
            Token::Literal { .. } | Token::Rest { .. } => {}
        }
    }
}

/// One way to write a command: its tokens in order.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Form {
    pub tokens: Vec<Token>,
}

impl Form {
    /// The command words, in order.
    #[must_use]
    pub fn words(&self) -> Vec<String> {
        self.tokens
            .iter()
            .filter_map(|token| match token {
                Token::Literal { word } => Some(word.clone()),
                _ => None,
            })
            .collect()
    }
}

/// One command row: the words that name it, its forms, and its summary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub path: Vec<String>,
    /// The usage text of each form, as the help prints it.
    pub usage: Vec<String>,
    pub forms: Vec<Form>,
    pub summary: String,
}

/// One row of the top-level help table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupRow {
    pub name: String,
    pub summary: String,
}

/// The groups the top-level help lists, in order: every two-space-indented
/// row whose first word is a lowercase command name, with continuation
/// lines joined. The same reading `openagents mcp serve` uses.
#[must_use]
pub fn groups(usage: &str) -> Vec<GroupRow> {
    let mut groups: Vec<GroupRow> = Vec::new();
    for line in usage.lines() {
        let Some(row) = line.strip_prefix("  ") else {
            continue;
        };
        if row.starts_with(' ') {
            if let Some(last) = groups.last_mut() {
                last.summary.push(' ');
                last.summary.push_str(row.trim());
            }
            continue;
        }
        let Some((name, summary)) = row.split_once(char::is_whitespace) else {
            continue;
        };
        if !name.starts_with(|c: char| c.is_ascii_lowercase())
            || !name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        {
            continue;
        }
        groups.push(GroupRow {
            name: name.to_owned(),
            summary: summary.trim().to_owned(),
        });
    }
    groups
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Lex {
    Open,
    Close,
    OpenGroup,
    CloseGroup,
    Bar,
    Ellipsis,
    Word(String),
}

fn lex(text: &str) -> Vec<Lex> {
    let mut out = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    let flush = |word: &mut String, out: &mut Vec<Lex>| {
        if word.is_empty() {
            return;
        }
        let mut text = std::mem::take(word);
        let ellipsis = text.len() > 3 && text.ends_with("...");
        if ellipsis {
            text.truncate(text.len() - 3);
        }
        match text.as_str() {
            "..." => out.push(Lex::Ellipsis),
            "|" | "/" => out.push(Lex::Bar),
            _ => out.push(Lex::Word(text)),
        }
        if ellipsis {
            out.push(Lex::Ellipsis);
        }
    };
    for c in text.chars() {
        if quoted {
            word.push(c);
            if c == '"' {
                quoted = false;
            }
            continue;
        }
        match c {
            '"' => {
                word.push(c);
                quoted = true;
            }
            '[' | ']' | '(' | ')' => {
                flush(&mut word, &mut out);
                out.push(match c {
                    '[' => Lex::Open,
                    ']' => Lex::Close,
                    '(' => Lex::OpenGroup,
                    _ => Lex::CloseGroup,
                });
            }
            c if c.is_whitespace() => flush(&mut word, &mut out),
            c => word.push(c),
        }
    }
    flush(&mut word, &mut out);
    out
}

#[derive(Clone, Debug)]
enum Part {
    Word(String),
    Group {
        alternatives: Vec<Vec<Part>>,
        optional: bool,
    },
    Ellipsis,
}

fn parse(lexes: &[Lex], at: &mut usize, close: Option<&Lex>) -> Result<Vec<Vec<Part>>, String> {
    let mut alternatives = vec![Vec::new()];
    while *at < lexes.len() {
        let lex = &lexes[*at];
        *at += 1;
        let current = alternatives.last_mut().expect("one alternative");
        match lex {
            Lex::Word(word) => current.push(Part::Word(word.clone())),
            Lex::Ellipsis => current.push(Part::Ellipsis),
            Lex::Bar => alternatives.push(Vec::new()),
            Lex::Open | Lex::OpenGroup => {
                let optional = *lex == Lex::Open;
                let closing = if optional {
                    Lex::Close
                } else {
                    Lex::CloseGroup
                };
                let inner = parse(lexes, at, Some(&closing))?;
                alternatives
                    .last_mut()
                    .expect("one alternative")
                    .push(Part::Group {
                        alternatives: inner,
                        optional,
                    });
            }
            Lex::Close | Lex::CloseGroup => {
                return if close == Some(lex) {
                    Ok(alternatives)
                } else {
                    Err(format!(
                        "unbalanced `{}`",
                        if *lex == Lex::Close { "]" } else { ")" }
                    ))
                };
            }
        }
    }
    match close {
        None => Ok(alternatives),
        Some(_) => Err("an unclosed bracket".to_string()),
    }
}

/// A word that names a command: lowercase letters, digits, and dashes,
/// with `|` between alternatives (`install|uninstall|status`).
fn is_literal(word: &str) -> bool {
    word.starts_with(|c: char| c.is_ascii_lowercase())
        && word
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '|')
}

/// A value's metavar split into the enum values it lists and whether it is
/// open: `all|ads|zone` is closed, `standard|admin|all|LIST` and `N` are
/// open.
fn choices_of(value: &str) -> (Vec<String>, bool) {
    if value.starts_with('"') || value.starts_with(|c: char| c.is_ascii_digit()) {
        return (Vec::new(), true);
    }
    let mut choices = Vec::new();
    let mut open = false;
    for part in value.split('|') {
        if part.chars().any(|c| c.is_ascii_uppercase()) || part.is_empty() {
            open = true;
        } else {
            choices.push(part.to_string());
        }
    }
    if choices.is_empty() {
        open = true;
    }
    (choices, open)
}

fn flatten(alternatives: &[Vec<Part>], optional: bool, out: &mut Vec<Token>) {
    let optional = optional || alternatives.len() > 1;
    for parts in alternatives {
        flatten_seq(parts, optional, out);
    }
}

fn flatten_seq(parts: &[Part], optional: bool, out: &mut Vec<Token>) {
    let mut index = 0;
    while index < parts.len() {
        let repeated = matches!(parts.get(index + 1), Some(Part::Ellipsis));
        match &parts[index] {
            Part::Word(word) if word == "--" => {
                out.push(Token::Rest { optional });
                // `CMD [ARGS...]` is the passed-through command itself.
                return;
            }
            Part::Word(word) if word.starts_with("--") && word.len() > 2 => {
                let name = &word[2..];
                let (name, inline) = match name.split_once('=') {
                    Some((name, value)) => (name, Some(value.to_string())),
                    None => (name, None),
                };
                let mut value = inline;
                if value.is_none()
                    && let Some(Part::Word(next)) = parts.get(index + 1)
                    && !next.starts_with("--")
                {
                    value = Some(next.clone());
                    index += 1;
                }
                let (choices, open) = value.as_deref().map_or((Vec::new(), false), choices_of);
                let repeated = matches!(parts.get(index + 1), Some(Part::Ellipsis));
                out.push(Token::Option {
                    name: name.to_string(),
                    value,
                    choices,
                    open,
                    optional,
                    repeated,
                });
            }
            Part::Word(word) if is_literal(word) => out.push(Token::Literal { word: word.clone() }),
            Part::Word(word) => out.push(Token::Positional {
                name: word.clone(),
                optional,
                repeated,
            }),
            Part::Group {
                alternatives,
                optional: bracket,
            } => {
                let start = out.len();
                flatten(alternatives, optional || *bracket, out);
                if repeated {
                    for token in &mut out[start..] {
                        token.mark_repeated();
                    }
                }
            }
            Part::Ellipsis => {
                // `request|list|show ...`: the rest of the words pass through.
                if matches!(out.last(), Some(Token::Literal { .. }) | None) {
                    out.push(Token::Rest { optional: true });
                }
            }
        }
        index += 1;
    }
}

/// Parse one form's usage text, such as `show HOST` or
/// `list [--limit N]`.
///
/// # Errors
///
/// Names an unbalanced bracket.
pub fn form(text: &str) -> Result<Form, String> {
    let lexes = lex(text);
    let mut at = 0;
    let alternatives = parse(&lexes, &mut at, None)?;
    let mut tokens = Vec::new();
    flatten(&alternatives, false, &mut tokens);
    Ok(Form { tokens })
}

/// Split a row's usage into its top-level alternatives, each parsed. An
/// alternative that starts with a command word names another command
/// (`enable HOST | disable HOST`); one that does not is another form of
/// the command before it (`PUBKEY:SLUG | --author PUBKEY SLUG`).
fn alternatives(text: &str) -> Result<Vec<(String, Form)>, String> {
    let lexes = lex(text);
    let mut at = 0;
    let top = parse(&lexes, &mut at, None)?;
    let mut out: Vec<(String, Form)> = Vec::new();
    for parts in &top {
        let mut tokens = Vec::new();
        flatten_seq(parts, false, &mut tokens);
        let starts_command = matches!(tokens.first(), Some(Token::Literal { .. }));
        match out.last() {
            Some((_, previous)) if !starts_command => {
                // Carry the command words over so the form is complete.
                let mut form: Vec<Token> = previous
                    .tokens
                    .iter()
                    .take_while(|token| matches!(token, Token::Literal { .. }))
                    .cloned()
                    .collect();
                form.extend(tokens);
                out.push((text.to_string(), Form { tokens: form }));
            }
            _ => out.push((text.to_string(), Form { tokens })),
        }
    }
    Ok(out)
}

/// Expand command words written with alternatives
/// (`service install|uninstall|status`) into one form per command.
fn expand(form: &Form) -> Vec<Form> {
    let mut forms = vec![Form::default()];
    for token in &form.tokens {
        match token {
            Token::Literal { word } if word.contains('|') => {
                let mut next = Vec::new();
                for form in &forms {
                    for alternative in word.split('|').filter(|w| !w.is_empty()) {
                        let mut form = form.clone();
                        form.tokens.push(Token::Literal {
                            word: alternative.to_string(),
                        });
                        next.push(form);
                    }
                }
                forms = next;
            }
            token => {
                for form in &mut forms {
                    form.tokens.push(token.clone());
                }
            }
        }
    }
    forms
}

/// Split a row into usage and summary: at the first run of two or more
/// spaces, or earlier at the first capitalized word outside brackets
/// (`build [--timeout SECONDS] Fly every part`), since a metavar is all
/// capitals and a command word all lowercase.
fn split_gap(text: &str) -> (&str, &str) {
    let mut at = text.find("  ").unwrap_or(text.len());
    let mut depth = 0i32;
    let mut start = true;
    for (index, c) in text.char_indices() {
        if index >= at {
            break;
        }
        match c {
            '[' | '(' => depth += 1,
            ']' | ')' => depth -= 1,
            _ => {}
        }
        if start && depth == 0 && c.is_ascii_uppercase() {
            let mut rest = text[index + 1..].chars();
            if rest.next().is_some_and(|next| next.is_ascii_lowercase()) {
                at = index;
                break;
            }
        }
        start = c.is_whitespace();
    }
    (text[..at].trim_end(), text[at..].trim())
}

struct Draft {
    usage: String,
    summary: String,
}

/// The command rows of one group's `USAGE` text.
///
/// Rows are the two-space-indented lines that start with a command word;
/// deeper-indented lines continue the row above (usage while they start
/// with `[`, or with `(`, `-`, or `|` before any summary; summary
/// otherwise). The first unindented line after the header ends the rows,
/// so option and verb notes below it are not read as commands. A row
/// written as `coder task submit …` or `openagents GROUP …` has that
/// prefix dropped.
///
/// # Errors
///
/// Names the row whose usage does not parse.
pub fn rows(group: &str, usage: &str) -> Result<Vec<Row>, String> {
    let mut drafts: Vec<Draft> = Vec::new();
    let mut open = false;
    for (index, line) in usage.lines().enumerate() {
        if index == 0 && line.trim_start().to_ascii_lowercase().starts_with("usage:") {
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        if indent == 0 {
            break;
        }
        let text = line.trim_start();
        if indent == 2 {
            let mut text = text;
            for prefix in [format!("coder {group} "), format!("openagents {group} ")] {
                if let Some(rest) = text.strip_prefix(prefix.as_str()) {
                    text = rest;
                }
            }
            if text.starts_with(|c: char| c.is_ascii_lowercase()) {
                let (usage, summary) = split_gap(text);
                drafts.push(Draft {
                    usage: usage.to_string(),
                    summary: summary.to_string(),
                });
                open = true;
            } else {
                open = false;
            }
            continue;
        }
        if !open {
            continue;
        }
        let draft = drafts.last_mut().expect("an open row");
        let usage_line = text.starts_with('[')
            || (draft.summary.is_empty() && text.starts_with(['(', '-', '|']));
        if usage_line {
            let (usage, summary) = split_gap(text);
            draft.usage.push(' ');
            draft.usage.push_str(usage);
            if !summary.is_empty() {
                draft.summary = summary.to_string();
            }
        } else {
            if !draft.summary.is_empty() {
                draft.summary.push(' ');
            }
            draft.summary.push_str(text);
        }
    }
    let mut rows: Vec<Row> = Vec::new();
    for draft in drafts {
        let parsed =
            alternatives(&draft.usage).map_err(|error| format!("`{}`: {error}", draft.usage))?;
        for (text, form) in parsed {
            for form in expand(&form) {
                let path = form.words();
                if path.is_empty() {
                    return Err(format!("`{text}` names no command"));
                }
                match rows.iter_mut().find(|row| row.path == path) {
                    Some(row) => {
                        row.usage.push(text.clone());
                        row.forms.push(form);
                        if row.summary.is_empty() {
                            row.summary.clone_from(&draft.summary);
                        }
                    }
                    None => rows.push(Row {
                        path,
                        usage: vec![text.clone()],
                        forms: vec![form],
                        summary: draft.summary.clone(),
                    }),
                }
            }
        }
    }
    Ok(rows)
}

/// The form of a group that has no command rows, from its header line
/// (`usage: openagents discover [OPTIONS]`) and its option lines.
///
/// # Errors
///
/// Names a header that does not parse.
pub fn bare(group: &str, usage: &str) -> Result<Form, String> {
    let header = usage.lines().next().unwrap_or_default();
    let after = header
        .split_once(&format!("openagents {group}"))
        .map_or("", |(_, rest)| rest)
        .replace("[OPTIONS]", "")
        .replace("[ARGS]", "");
    let mut form = form(&after)?;
    for option in options(usage) {
        if !form
            .tokens
            .iter()
            .any(|token| matches!(token, Token::Option { name, .. } if option_name(&option) == Some(name)))
        {
            form.tokens.push(option);
        }
    }
    Ok(form)
}

fn option_name(token: &Token) -> Option<&String> {
    match token {
        Token::Option { name, .. } => Some(name),
        _ => None,
    }
}

/// A metavar: an uppercase name, possibly with punctuation
/// (`SECONDS`, `NODE_ID@HOST:PORT`), or a `|`-list of values.
fn metavar(word: &str) -> bool {
    (word.starts_with(|c: char| c.is_ascii_uppercase())
        && !word.chars().any(|c| c.is_ascii_lowercase()))
        || (word.contains('|') && !word.starts_with('-'))
}

/// Every option the whole text mentions, rows and notes alike, each once:
/// `--name METAVAR` as an option with a value, a bare `--name` as a switch
/// unless it takes a value somewhere else. These are the group-wide
/// options a command may carry (`--relay URL`, `--as PROFILE`).
#[must_use]
pub fn options(usage: &str) -> Vec<Token> {
    let words: Vec<&str> = usage
        .lines()
        .skip(1)
        .flat_map(str::split_whitespace)
        .map(|word| word.trim_matches(|c: char| "[](),.;`'".contains(c)))
        .collect();
    let mut out: Vec<Token> = Vec::new();
    for (index, word) in words.iter().enumerate() {
        let Some(name) = word.strip_prefix("--") else {
            continue;
        };
        if name.is_empty() || !name.starts_with(|c: char| c.is_ascii_lowercase()) {
            continue;
        }
        let (name, inline) = match name.split_once('=') {
            Some((name, value)) => (name, Some(value.to_string())),
            None => (name, None),
        };
        let value = inline.or_else(|| {
            words
                .get(index + 1)
                .filter(|next| metavar(next))
                .map(|next| (*next).to_string())
        });
        let (choices, open) = value.as_deref().map_or((Vec::new(), false), choices_of);
        match out
            .iter_mut()
            .find(|token| option_name(token).is_some_and(|known| known == name))
        {
            Some(Token::Option {
                value: known,
                choices: known_choices,
                open: known_open,
                ..
            }) if known.is_none() && value.is_some() => {
                *known = value;
                *known_choices = choices;
                *known_open = open;
            }
            Some(_) => {}
            None => out.push(Token::Option {
                name: name.to_string(),
                value,
                choices,
                open,
                optional: true,
                repeated: true,
            }),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(text: &str) -> Vec<Token> {
        form(text).unwrap().tokens
    }

    fn positional(name: &str, optional: bool, repeated: bool) -> Token {
        Token::Positional {
            name: name.into(),
            optional,
            repeated,
        }
    }

    #[test]
    fn reads_positionals_options_switches_and_rest() {
        assert_eq!(
            tokens("tail HOST PATH [--lines N] [--follow]"),
            vec![
                Token::Literal {
                    word: "tail".into()
                },
                positional("HOST", false, false),
                positional("PATH", false, false),
                Token::Option {
                    name: "lines".into(),
                    value: Some("N".into()),
                    choices: vec![],
                    open: true,
                    optional: true,
                    repeated: false
                },
                Token::Option {
                    name: "follow".into(),
                    value: None,
                    choices: vec![],
                    open: false,
                    optional: true,
                    repeated: false
                },
            ]
        );
        let exec = tokens("exec HOST [--timeout S] -- CMD [ARGS...]");
        assert_eq!(exec.last(), Some(&Token::Rest { optional: false }));
    }

    #[test]
    fn reads_enums_and_repeats() {
        let say = tokens("say TEXT [--to all|ads|zone|near|here]");
        assert!(
            matches!(&say[2], Token::Option { choices, open: false, .. } if choices.len() == 5)
        );
        let rights = tokens("invite HOST [--rights standard|admin|all|LIST]");
        assert!(
            matches!(&rights[2], Token::Option { open: true, choices, .. } if choices.len() == 3)
        );
        let xp = tokens("xp [--pubkey KEY]...");
        assert!(matches!(&xp[1], Token::Option { repeated: true, .. }));
        let prompt = tokens("steer HOST TASK --revision N PROMPT...");
        assert_eq!(prompt.last(), Some(&positional("PROMPT", false, true)));
    }

    #[test]
    fn splits_command_alternatives_and_expands_words() {
        let usage = "usage: openagents x COMMAND
  enable HOST | disable HOST | retry HOST
  service install|uninstall|status [--binary PATH]
                          Run the service.
  describe PUBKEY:SLUG | --author PUBKEY SLUG
        Print one head.";
        let rows = rows("x", usage).unwrap();
        let paths: Vec<String> = rows.iter().map(|row| row.path.join(" ")).collect();
        assert_eq!(
            paths,
            [
                "enable",
                "disable",
                "retry",
                "service install",
                "service uninstall",
                "service status",
                "describe"
            ]
        );
        assert_eq!(rows[6].forms.len(), 2);
        assert_eq!(rows[3].summary, "Run the service.");
    }

    #[test]
    fn notes_after_the_rows_are_not_commands() {
        let usage = "usage: openagents zone COMMAND
  build [--timeout SECONDS] Fly every part.
Verbs: fly X,Y,Z | grab
  install                   Carry the held part.";
        let rows = rows("zone", usage).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].summary, "Fly every part.");
    }

    #[test]
    fn nested_words_follow_a_positional() {
        let form = form("control ENTITY move X,Y,Z [--yaw DEGREES]").unwrap();
        assert_eq!(form.words(), ["control", "move"]);
    }

    #[test]
    fn group_notes_supply_shared_options() {
        let options = options(
            "usage: openagents cap COMMAND\n  list\nOptions:\n  --relay URL  Relay.\n  --same-machine  Allow loopback.",
        );
        assert!(
            options.iter().any(
                |t| matches!(t, Token::Option { name, value: Some(_), .. } if name == "relay")
            )
        );
        assert!(options.iter().any(
            |t| matches!(t, Token::Option { name, value: None, .. } if name == "same-machine")
        ));
    }

    #[test]
    fn quoted_values_stay_whole() {
        let form = form("verify --issue N --fix-build \"1.0.0 (16)\" --verified yes|no").unwrap();
        assert!(form.tokens.iter().any(
            |t| matches!(t, Token::Option { name, value: Some(v), .. } if name == "fix-build" && v == "\"1.0.0 (16)\"")
        ));
    }
}
