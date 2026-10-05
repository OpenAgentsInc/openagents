//! Decides on this computer whether a prompt line is a shell command or a
//! request. No shell line leaves the machine to be classified.
//!
//! The rules follow the smart terminal's design (`docs/terminal/smart-terminal.md`,
//! "How Enter decides"): an explicit prefix wins; a path, an assignment, or a
//! first word the shell does not resolve decides structurally; a resolving
//! first word followed by prose is scored locally. No local decision model
//! answers rule 4 yet, so the score is the whole decision there.

/// What the shell's own command table says the first word is, as zsh's
/// `whence -w` reports it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Word {
    Alias,
    Builtin,
    Function,
    Command,
    Reserved,
    /// The shell resolves nothing by that name.
    Missing,
    /// No report yet, or a shell without the hook.
    #[default]
    Unknown,
}

impl Word {
    /// Reads `whence -w` output: `name: kind`, or the bare kind.
    #[must_use]
    pub fn parse(report: &str) -> Self {
        match report.rsplit(": ").next().unwrap_or("").trim() {
            "alias" => Self::Alias,
            "builtin" => Self::Builtin,
            "function" => Self::Function,
            "command" | "hashed" => Self::Command,
            "reserved" => Self::Reserved,
            "none" => Self::Missing,
            _ => Self::Unknown,
        }
    }
    fn resolves(self) -> bool {
        matches!(
            self,
            Self::Alias | Self::Builtin | Self::Function | Self::Command | Self::Reserved
        )
    }
}

/// zsh's builtins and reserved words, which never appear on `PATH`.
const BUILTINS: &[&str] = &[
    ".",
    ":",
    "[",
    "alias",
    "autoload",
    "bg",
    "bindkey",
    "break",
    "builtin",
    "bye",
    "cd",
    "chdir",
    "command",
    "compadd",
    "continue",
    "declare",
    "dirs",
    "disable",
    "disown",
    "echo",
    "emulate",
    "enable",
    "eval",
    "exec",
    "exit",
    "export",
    "false",
    "fc",
    "fg",
    "float",
    "functions",
    "getopts",
    "hash",
    "history",
    "integer",
    "jobs",
    "kill",
    "let",
    "limit",
    "local",
    "log",
    "logout",
    "noglob",
    "popd",
    "print",
    "printf",
    "pushd",
    "pwd",
    "r",
    "read",
    "readonly",
    "rehash",
    "return",
    "set",
    "setopt",
    "shift",
    "source",
    "suspend",
    "test",
    "times",
    "trap",
    "true",
    "ttyctl",
    "type",
    "typeset",
    "ulimit",
    "umask",
    "unalias",
    "unfunction",
    "unhash",
    "unlimit",
    "unset",
    "unsetopt",
    "vared",
    "wait",
    "whence",
    "where",
    "which",
    "zcompile",
    "zle",
    "zmodload",
    "zparseopts",
    "zstyle",
];
const RESERVED: &[&str] = &[
    "!",
    "[[",
    "case",
    "coproc",
    "do",
    "done",
    "elif",
    "else",
    "end",
    "esac",
    "fi",
    "for",
    "foreach",
    "function",
    "if",
    "nocorrect",
    "repeat",
    "select",
    "then",
    "time",
    "until",
    "while",
    "{",
    "}",
];

/// The shell's command table, as its hook reports it at each prompt:
/// alias and function names, and `PATH`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Table {
    pub aliases: std::collections::BTreeSet<String>,
    pub functions: std::collections::BTreeSet<String>,
    pub path: Option<String>,
}

impl Table {
    /// Reads the hook's report: lines `p:PATH`, `a:NAMES`, and `f:NAMES`.
    #[must_use]
    pub fn parse(report: &str) -> Self {
        let mut table = Self::default();
        for line in report.lines() {
            if let Some(path) = line.strip_prefix("p:") {
                table.path = Some(path.to_owned());
            } else if let Some(names) = line.strip_prefix("a:") {
                table.aliases = names.split_whitespace().map(str::to_owned).collect();
            } else if let Some(names) = line.strip_prefix("f:") {
                table.functions = names.split_whitespace().map(str::to_owned).collect();
            }
        }
        table
    }

    /// What `name` resolves to. Before the shell reports, `PATH` is this
    /// process's own.
    #[must_use]
    pub fn word(&self, name: &str) -> Word {
        if name.is_empty() {
            return Word::Unknown;
        }
        if self.aliases.contains(name) {
            return Word::Alias;
        }
        if self.functions.contains(name) {
            return Word::Function;
        }
        if RESERVED.contains(&name) {
            return Word::Reserved;
        }
        if BUILTINS.contains(&name) {
            return Word::Builtin;
        }
        if name.contains('/') {
            return Word::Unknown;
        }
        let path = self
            .path
            .clone()
            .or_else(|| std::env::var("PATH").ok())
            .unwrap_or_default();
        for dir in path.split(':').filter(|dir| !dir.is_empty()) {
            let candidate = std::path::Path::new(dir).join(name);
            if executable(&candidate) {
                return Word::Command;
            }
        }
        Word::Missing
    }

    /// Classifies `line` against this table.
    #[must_use]
    pub fn classify(&self, line: &str) -> Decision {
        let first = words(line.trim()).0.into_iter().next().unwrap_or_default();
        classify(line, self.word(&first))
    }
}

#[cfg(unix)]
fn executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}
#[cfg(not(unix))]
fn executable(path: &std::path::Path) -> bool {
    path.is_file()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    Shell,
    Ask,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decision {
    pub route: Route,
    /// False when only the local score decided (rule 4).
    pub sure: bool,
}

impl Decision {
    const fn sure(route: Route) -> Self {
        Self { route, sure: true }
    }
    #[must_use]
    pub fn label(self) -> &'static str {
        match (self.route, self.sure) {
            (Route::Shell, true) => "shell",
            (Route::Shell, false) => "shell?",
            (Route::Ask, true) => "ask",
            (Route::Ask, false) => "ask?",
        }
    }
}

const QUESTION: &[&str] = &[
    "why",
    "how",
    "what",
    "what's",
    "whats",
    "when",
    "where",
    "who",
    "which",
    "can",
    "could",
    "should",
    "would",
    "is",
    "are",
    "does",
    "do",
    "did",
    "will",
    "explain",
    "please",
    "help",
    "tell",
    "show",
    "summarize",
    "describe",
];
/// Words that make a line read as prose when they follow a command name.
const STRONG: &[&str] = &[
    "the", "a", "an", "my", "this", "that", "these", "those", "it", "its", "it's", "me", "all",
    "every", "our", "your", "i", "we", "you", "why", "how", "what", "faster", "slower", "better",
    "wrong", "broken",
];
const WEAK: &[&str] = &[
    "in", "of", "to", "for", "with", "and", "from", "into", "about", "so", "is", "are", "was",
    "did", "does", "not",
];

/// Splits outside quotes, and reports whether quoting was balanced.
fn words(line: &str) -> (Vec<String>, bool, bool) {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut quoted = false;
    for c in line.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '\'' || c == '"' => {
                quote = Some(c);
                quoted = true;
            }
            None if c.is_whitespace() => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            None => current.push(c),
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    (out, quote.is_none(), quoted)
}

fn operators(line: &str) -> bool {
    let (mut single, mut double) = (false, false);
    let chars: Vec<char> = line.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        match c {
            '\'' if !double => single = !single,
            '"' if !single => double = !double,
            _ if single || double => {}
            '|' | '>' | '<' | ';' | '`' | '$' => return true,
            '&' if chars.get(i + 1) == Some(&'&') => return true,
            '*' | '[' if i > 0 && !chars[i - 1].is_whitespace() => return true,
            '*' if chars.get(i + 1).is_some_and(|n| !n.is_whitespace()) => return true,
            _ => {}
        }
    }
    false
}

fn assignment(word: &str) -> bool {
    word.split_once('=').is_some_and(|(name, _)| {
        !name.is_empty()
            && name
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    })
}

fn path(word: &str) -> bool {
    word.starts_with("./")
        || word.starts_with("../")
        || word.starts_with("~/")
        || word == "~"
        || word.starts_with('/')
}

/// Argument shapes that prose rarely has: flags, paths, files, and keys.
fn technical(word: &str) -> bool {
    word.starts_with('-')
        || word.contains('/')
        || word.contains('=')
        || word.contains(':')
        || word.chars().any(|c| c.is_ascii_digit())
        || (word.contains('.') && !word.ends_with('.') && !word.ends_with("..."))
}

/// Decides one prompt line. `first` is the shell's report on its first word.
#[must_use]
pub fn classify(line: &str, first: Word) -> Decision {
    let line = line.trim();
    if line.is_empty() {
        return Decision::sure(Route::Shell);
    }
    // The explicit prefix: a shell comment, so nothing is lost.
    if line.starts_with("# ") || line == "#" {
        return Decision::sure(Route::Ask);
    }
    let (words, balanced, quoted) = words(line);
    let Some(head) = words.first() else {
        return Decision::sure(Route::Shell);
    };
    if path(head) || assignment(head) {
        return Decision::sure(Route::Shell);
    }
    let syntax = operators(line);
    let lower: Vec<String> = words
        .iter()
        .map(|w| {
            w.trim_end_matches(['?', '.', '!', ','])
                .to_ascii_lowercase()
        })
        .collect();
    let question = line.ends_with('?');
    // An apostrophe in prose ("what's failing") leaves a quote open.
    if !balanced && !syntax {
        return Decision::sure(Route::Ask);
    }
    if !first.resolves() {
        // The shell would only print `command not found`; a lone word is
        // more likely a typo of a command than a question.
        let looks_like_command = syntax || quoted || words.iter().skip(1).any(|w| technical(w));
        if words.len() == 1 && !question || looks_like_command && !question {
            return Decision::sure(Route::Shell);
        }
        return Decision::sure(Route::Ask);
    }
    // Rule 4: the first word resolves. Score the rest locally.
    let mut shell: u32 = 2;
    let mut ask: u32 = 0;
    if words.len() <= 2 {
        shell += 2;
    }
    if syntax {
        shell += 3;
    }
    if quoted {
        shell += 2;
    }
    if words.iter().skip(1).any(|w| technical(w)) {
        shell += 3;
    }
    if question {
        ask += 3;
    }
    if QUESTION.contains(&lower[0].as_str()) {
        ask += 2;
    }
    let mut strong = 0;
    let mut weak = 0;
    for word in lower.iter().skip(1) {
        if STRONG.contains(&word.as_str()) {
            strong += 1;
        } else if WEAK.contains(&word.as_str()) {
            weak += 1;
        }
    }
    ask += 2 * strong.min(2) + weak.min(2);
    if words.len() >= 4
        && words.iter().all(|w| {
            w.trim_end_matches(['?', '.', '!', ','])
                .chars()
                .all(|c| c.is_alphabetic() || c == '\'')
        })
    {
        ask += 1;
    }
    if line.ends_with('.') && !syntax {
        ask += 1;
    }
    let route = if ask > shell {
        Route::Ask
    } else {
        Route::Shell
    };
    Decision {
        route,
        sure: ask.abs_diff(shell) >= 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_caret_routes_each_example_line_locally() {
        use Route::{Ask, Shell};
        use Word::{Builtin, Command, Missing};
        let table: &[(&str, Word, Route)] = &[
            ("git status", Command, Shell),
            ("ls -la", Command, Shell),
            ("cargo test", Command, Shell),
            ("./run.sh", Missing, Shell),
            ("cd ~/x", Builtin, Shell),
            ("FOO=1 make", Missing, Shell),
            ("make it faster", Command, Ask),
            ("why did that fail", Missing, Ask),
            ("how do I undo my last commit", Missing, Ask),
            ("fix the failing test", Missing, Ask),
            ("fix the failing test", Command, Ask),
            ("explain this error", Missing, Ask),
            ("find all TODOs in src", Command, Ask),
            ("echo \"hi\"", Builtin, Shell),
            ("python3 -c 'print(1)'", Command, Shell),
            ("which python", Builtin, Shell),
            ("which file handles auth?", Builtin, Ask),
            ("where is the config?", Builtin, Ask),
            ("what's failing here", Missing, Ask),
            ("git push origin main", Command, Shell),
            ("git commit -m 'fix the failing test'", Command, Shell),
            ("make install", Command, Shell),
            ("find . -name '*.rs'", Command, Shell),
            ("cat Cargo.toml | grep version", Command, Shell),
            ("gti", Missing, Shell),
            ("gti status --short", Missing, Shell),
            ("# why did that fail", Missing, Ask),
            ("", Word::Unknown, Shell),
        ];
        for (line, word, route) in table {
            assert_eq!(classify(line, *word).route, *route, "{line:?} ({word:?})");
        }
    }

    #[test]
    fn structural_rules_are_sure_and_scores_are_marked() {
        assert!(classify("./run.sh", Word::Missing).sure);
        assert!(classify("why did that fail", Word::Missing).sure);
        assert!(!classify("make it faster", Word::Command).sure);
        assert_eq!(classify("make it faster", Word::Command).label(), "ask?");
    }

    #[test]
    fn whence_reports_parse() {
        assert_eq!(Word::parse("ls: command"), Word::Command);
        assert_eq!(Word::parse("cd: builtin"), Word::Builtin);
        assert_eq!(Word::parse("ll: alias"), Word::Alias);
        assert_eq!(Word::parse("why: none"), Word::Missing);
        assert_eq!(Word::parse("hashed"), Word::Command);
        assert_eq!(Word::parse(""), Word::Unknown);
    }
}
