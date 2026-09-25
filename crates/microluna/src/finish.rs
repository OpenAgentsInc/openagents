//! The finish rule: a `done` finish waits for the score and a baseline run.
//!
//! A session may call `finish` with status `done` only when, since its last
//! edit, it has run the score and at least one baseline command. The rule
//! is code, not guidance: the host folds the session's own calls into a
//! [`Ledger`] as they happen, and on a `done` finish asks the ledger for a
//! [`Verdict`]. A refusal goes back to the session as the `finish` call's
//! result, naming the file it edited after its last run. It isn't a host
//! turn-back, so it doesn't count against [`crate::Persist::max_returns`].
//! After [`FinishRule::max_refusals`] refusals, the next `done` finish is
//! accepted and recorded as [`UNVERIFIED`]. A `blocked` or `failed`
//! finish is never gated.
//!
//! What counts:
//!
//! - **An edit** is an `apply_patch` or `write_file` that applied, or a
//!   command that changed a workspace file ([`changed_files`]). A
//!   command's own changes happen before it completes, and a score or
//!   baseline run's changes are that program's outputs, not edits.
//! - **A score run** is a command that doesn't only read
//!   ([`crate::tools::reads_only`]) and names one of [`FinishRule::score`],
//!   such as `score.sh`.
//! - **A baseline run** is a command that doesn't only read and runs one
//!   of [`FinishRule::baseline`]: some piece of it has the same entry point
//!   and the same set of arguments ([`Invocation`]). The host finds the
//!   baseline commands (the task's own program, issue #9633); with none,
//!   only the score is required.
//!
//! Issue #9638 and `docs/coder/design/microluna-v18.md`, change 7, record
//! why.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::tools::FinishStatus;

/// What an accepted finish that never met the rule is recorded as.
pub const UNVERIFIED: &str = "unverified";

/// The refusals a session gets before a `done` finish is accepted as
/// [`UNVERIFIED`].
pub const MAX_REFUSALS: u32 = 3;

/// The most files one workspace listing reads. A larger workspace isn't
/// listed, and a command's changes to it go unseen.
pub const LISTING_MAX: usize = 20_000;

/// The rule a session's `done` finish is held to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinishRule {
    /// What a command names to count as a score run, such as `score.sh`.
    pub score: Vec<String>,
    /// The task's baseline commands. Empty requires only the score.
    #[serde(default)]
    pub baseline: Vec<String>,
    /// Refusals before a `done` finish is accepted as [`UNVERIFIED`].
    #[serde(default = "max_refusals")]
    pub max_refusals: u32,
}

fn max_refusals() -> u32 {
    MAX_REFUSALS
}

impl FinishRule {
    /// A rule with `score` names, no baseline commands, and the default
    /// bound.
    #[must_use]
    pub fn score(score: &[&str]) -> Self {
        FinishRule {
            score: score.iter().map(|s| (*s).to_string()).collect(),
            baseline: Vec::new(),
            max_refusals: MAX_REFUSALS,
        }
    }

    /// What `command` is, as the rule counts it, with paths compared as
    /// they're written. A command that only reads is neither a score nor
    /// a baseline run, whatever it names.
    #[must_use]
    pub fn classify(&self, command: &str) -> Run {
        self.classify_at(command, None)
    }

    /// What `command` is, as the rule counts it, run from the workspace
    /// `root`: a path under `root` equals the same path relative to it.
    #[must_use]
    pub fn classify_at(&self, command: &str, root: Option<&str>) -> Run {
        if command.trim().is_empty()
            || crate::tools::reads_only("run_command", &json!({ "command": command }).to_string())
        {
            return Run::Other;
        }
        let score = self
            .score
            .iter()
            .any(|name| !name.trim().is_empty() && names_to_run(command, name.trim()));
        let runs = Invocation::all(command, root);
        let baseline = !runs.is_empty()
            && self.baseline.iter().any(|base| {
                Invocation::all(base, root)
                    .iter()
                    .any(|wanted| runs.contains(wanted))
            });
        match (score, baseline) {
            (true, true) => Run::Both,
            (true, false) => Run::Score,
            (false, true) => Run::Baseline,
            (false, false) => Run::Other,
        }
    }
}

/// One program a command runs, as the baseline match compares it: the
/// entry point and the set of arguments that aren't options.
///
/// - `python`, `python3`, and `python3.11` are one interpreter, and so are
///   `sh` and `bash`. An interpreter's entry point is its `-m` module, its
///   `-c` code, or its script's path; its own options before that are
///   left out.
/// - Any other program's entry point is the program itself; for `make`,
///   the targets are the arguments.
/// - A path is compared relative to the workspace root, so `/app/data/x`
///   and `./data/x` both equal `data/x` when the root is `/app`. Argument
///   order doesn't count, and neither do options, redirections,
///   environment assignments, or a leading `env`, `time`, `nohup`, `exec`,
///   or `timeout` wrapper.
///
/// A command is split into pieces at `;`, `&&`, `||`, `|`, `&`,
/// parentheses, and line breaks, and here-document bodies are left out.
/// The parse approximates the shell deterministically; it isn't a shell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invocation {
    /// The program, with interpreters folded (`python`, `sh`), and what it
    /// runs: `-m <module>`, `-c <code>`, or a script's path.
    pub entry: Vec<String>,
    /// The arguments that aren't options, with paths made relative to the
    /// root.
    pub args: BTreeSet<String>,
}

impl Invocation {
    /// Every program `command` runs, in order. `cd`, `export`, `set`, and
    /// `source` run none.
    #[must_use]
    pub fn all(command: &str, root: Option<&str>) -> Vec<Invocation> {
        pieces(command)
            .iter()
            .filter_map(|words| Invocation::of(words, root))
            .collect()
    }

    fn of(words: &[String], root: Option<&str>) -> Option<Invocation> {
        let mut rest = words
            .iter()
            .map(String::as_str)
            .skip_while(|w| assignment(w));
        let mut program = rest.next()?;
        loop {
            match program {
                "env" | "time" | "nohup" | "exec" | "command" => program = rest.next()?,
                // `timeout [options] duration program`.
                "timeout" => {
                    rest.find(|w| !w.starts_with('-'))?;
                    program = rest.next()?;
                }
                _ => break,
            }
            while assignment(program) {
                program = rest.next()?;
            }
        }
        let program = interpreter(&path(program, root));
        if matches!(program.as_str(), "cd" | "export" | "set" | "source" | ".") {
            return None;
        }
        let rest: Vec<&str> = rest.collect();
        let mut entry = vec![program.clone()];
        let mut at = 0;
        if matches!(program.as_str(), "python" | "sh") {
            while let Some(word) = rest.get(at).copied() {
                at += 1;
                if word == "-m" || word == "-c" {
                    entry.push(word.to_string());
                    entry.push((*rest.get(at)?).to_string());
                    at += 1;
                    break;
                }
                if word.starts_with('-') && word.len() > 1 {
                    // Options that take a value: Python's `-W` and `-X`,
                    // the shell's `-o`.
                    if matches!(
                        (program.as_str(), word),
                        ("python", "-W" | "-X") | ("sh", "-o")
                    ) {
                        at += 1;
                    }
                    continue;
                }
                entry.push(path(word, root));
                break;
            }
        }
        let args = rest
            .get(at..)
            .unwrap_or_default()
            .iter()
            .filter(|w| !(w.starts_with('-') && w.len() > 1))
            .map(|w| path(w, root))
            .collect();
        Some(Invocation { entry, args })
    }
}

/// Whether `word` is an environment assignment, `NAME=value`.
fn assignment(word: &str) -> bool {
    word.split_once('=').is_some_and(|(name, _)| {
        !name.is_empty()
            && !name.starts_with(|c: char| c.is_ascii_digit())
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    })
}

/// A program's name with interpreters folded: `python3.11` and `python3`
/// are `python`, and `bash` is `sh`. A program named by a path outside the
/// workspace, such as `/usr/bin/make`, is its file name.
fn interpreter(program: &str) -> String {
    let name = program.rsplit('/').next().unwrap_or(program);
    let python = name
        .strip_prefix("python")
        .is_some_and(|v| v.chars().all(|c| c.is_ascii_digit() || c == '.'));
    if python {
        "python".to_string()
    } else if matches!(name, "sh" | "bash" | "dash") {
        "sh".to_string()
    } else if program.starts_with('/') {
        name.to_string()
    } else {
        program.to_string()
    }
}

/// `word` as a path relative to `root` when it's under it, with `./`
/// prefixes and trailing slashes dropped.
fn path(word: &str, root: Option<&str>) -> String {
    let mut word = word;
    if let Some(root) = root
        .map(|r| r.trim_end_matches('/'))
        .filter(|r| !r.is_empty())
    {
        if word.trim_end_matches('/') == root {
            return ".".to_string();
        }
        if let Some(under) = word.strip_prefix(root).and_then(|w| w.strip_prefix('/')) {
            word = under;
        }
    }
    while let Some(under) = word.strip_prefix("./") {
        word = under;
    }
    match word.trim_end_matches('/') {
        "" => word.to_string(),
        trimmed => trimmed.to_string(),
    }
}

/// A command's words as the shell reads them so far.
#[derive(Default)]
struct Words {
    pieces: Vec<Vec<String>>,
    words: Vec<String>,
    word: String,
    /// A word has started, so an empty quoted word still counts.
    started: bool,
    /// The next word is a redirection's target: `Some(true)` for a
    /// here-document's delimiter, `Some(false)` for a file.
    target: Option<bool>,
    /// Here-document delimiters whose bodies start at the next line.
    delimiters: Vec<String>,
}

impl Words {
    fn end_word(&mut self) {
        if !self.started {
            return;
        }
        let word = std::mem::take(&mut self.word);
        match self.target.take() {
            Some(true) => self.delimiters.push(word),
            Some(false) => {}
            None => self.words.push(word),
        }
        self.started = false;
    }

    fn end_piece(&mut self) {
        self.end_word();
        if !self.words.is_empty() {
            self.pieces.push(std::mem::take(&mut self.words));
        }
    }
}

/// `command`'s pieces, each as its words: split at `;`, `&&`, `||`, `|`,
/// `&`, parentheses, and line breaks outside quotes, with quotes removed,
/// redirections and their targets dropped, and here-document bodies left
/// out.
fn pieces(command: &str) -> Vec<Vec<String>> {
    let mut s = Words::default();
    let mut chars = command.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                s.started = true;
                s.word.extend(chars.by_ref().take_while(|q| *q != '\''));
            }
            '"' => {
                s.started = true;
                while let Some(q) = chars.next() {
                    match q {
                        '"' => break,
                        '\\' => s.word.extend(chars.next()),
                        q => s.word.push(q),
                    }
                }
            }
            '\\' => match chars.next() {
                Some('\n') | None => {}
                Some(e) => {
                    s.started = true;
                    s.word.push(e);
                }
            },
            '>' | '<' => {
                // A descriptor number just before it, as in `2>`, is part
                // of the redirection.
                if !s.word.is_empty() && s.word.chars().all(|d| d.is_ascii_digit()) {
                    s.word.clear();
                    s.started = false;
                }
                s.end_word();
                let mut heredoc = false;
                if c == '<' && chars.peek() == Some(&'<') {
                    chars.next();
                    if chars.peek() == Some(&'<') {
                        chars.next();
                    } else {
                        heredoc = true;
                        if chars.peek() == Some(&'-') {
                            chars.next();
                        }
                    }
                } else if c == '>' && matches!(chars.peek(), Some('>' | '|')) {
                    chars.next();
                }
                if chars.peek() == Some(&'&') {
                    // `>&2` duplicates a descriptor and names no file.
                    chars.next();
                    while chars
                        .peek()
                        .is_some_and(|d| d.is_ascii_digit() || *d == '-')
                    {
                        chars.next();
                    }
                    continue;
                }
                s.target = Some(heredoc);
            }
            '&' if chars.peek() == Some(&'>') => s.end_word(),
            ';' | '&' | '|' | '(' | ')' | '\n' => {
                s.end_piece();
                if c == '\n' {
                    // Skip here-document bodies, each to its delimiter.
                    for delimiter in std::mem::take(&mut s.delimiters) {
                        while chars.peek().is_some() {
                            let line: String = chars.by_ref().take_while(|l| *l != '\n').collect();
                            if line.trim() == delimiter {
                                break;
                            }
                        }
                    }
                }
            }
            c if c.is_whitespace() => s.end_word(),
            c => {
                s.started = true;
                s.word.push(c);
            }
        }
    }
    s.end_piece();
    s.pieces
}

/// Whether `command` names `name` other than as a redirection's target, so
/// that writing the score script isn't running it.
fn names_to_run(command: &str, name: &str) -> bool {
    let mut after_redirect = false;
    for word in command.split_whitespace() {
        let target = after_redirect || word.starts_with('>');
        after_redirect = matches!(word, ">" | ">>");
        if word.contains(name) && !target {
            return true;
        }
    }
    false
}

/// What one command counts as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Run {
    Score,
    Baseline,
    Both,
    Other,
}

impl Run {
    fn score(self) -> bool {
        matches!(self, Run::Score | Run::Both)
    }
    fn baseline(self) -> bool {
        matches!(self, Run::Baseline | Run::Both)
    }
}

/// The session's edits and runs, in order.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ledger {
    /// Events folded so far.
    pub at: u64,
    /// When and what the session last edited.
    pub last_edit: Option<(u64, String)>,
    /// When the session last ran the score.
    pub last_score: Option<u64>,
    /// When the session last ran a baseline command.
    pub last_baseline: Option<u64>,
    /// The workspace root commands run from, which a baseline match
    /// compares paths against ([`FinishRule::classify_at`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
}

impl Ledger {
    /// An empty ledger for commands run from `root`.
    #[must_use]
    pub fn rooted(root: &Path) -> Self {
        Ledger {
            root: Some(root.display().to_string()),
            ..Ledger::default()
        }
    }

    /// Notes an edit to `path`.
    pub fn edited(&mut self, path: &str) {
        self.at += 1;
        self.last_edit = Some((self.at, path.to_string()));
    }

    /// Notes a command that ran, with the files it changed, and returns
    /// what it counted as. The changes come first: they happened before
    /// the command completed. A score or baseline run's changes are its
    /// outputs and aren't edits.
    pub fn ran(&mut self, rule: &FinishRule, command: &str, changed: &[String]) -> Run {
        let run = rule.classify_at(command, self.root.as_deref());
        self.ran_as(run, changed);
        run
    }

    /// Notes a command the caller already classified as `run`, with the
    /// files it changed, as [`Ledger::ran`] does.
    pub fn ran_as(&mut self, run: Run, changed: &[String]) {
        if run == Run::Other {
            for path in changed {
                self.edited(path);
            }
        }
        self.at += 1;
        if run.score() {
            self.last_score = Some(self.at);
        }
        if run.baseline() {
            self.last_baseline = Some(self.at);
        }
    }

    /// Why a `done` finish now would break the rule, or `None` when it
    /// meets it. A session that never edited meets it.
    #[must_use]
    pub fn missing(&self, rule: &FinishRule) -> Option<String> {
        let (edit, path) = self.last_edit.as_ref()?;
        let after = |run: Option<u64>| run.is_some_and(|at| at > *edit);
        let score = after(self.last_score);
        let baseline = rule.baseline.is_empty() || after(self.last_baseline);
        if score && baseline {
            return None;
        }
        let what = match (score, baseline) {
            (false, false) => "the score or a baseline command",
            (false, true) => "the score",
            _ => "a baseline command",
        };
        let mut runs = Vec::new();
        if !score && let Some(name) = rule.score.first() {
            runs.push(format!("the score (`{name}`)"));
        }
        if !baseline && let Some(base) = rule.baseline.first() {
            runs.push(format!("a baseline command, such as `{base}`"));
        }
        Some(format!(
            "you edited `{path}` after your last run of {what}. Run {} and read the result, then \
             call finish again",
            runs.join(" and ")
        ))
    }
}

/// What the host does with a finish.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The finish stands.
    Allowed,
    /// The finish is refused; the text goes back to the session as the
    /// call's result.
    Refused(String),
    /// The finish stands, but the rule was never met: the refusals ran
    /// out. The text says what was missing.
    Unverified(String),
}

/// The verdict on a finish with `status`, after `refusals` earlier ones.
#[must_use]
pub fn verdict(ledger: &Ledger, rule: &FinishRule, status: FinishStatus, refusals: u32) -> Verdict {
    if status != FinishStatus::Done {
        return Verdict::Allowed;
    }
    match ledger.missing(rule) {
        None => Verdict::Allowed,
        Some(why) if refusals >= rule.max_refusals => Verdict::Unverified(why),
        Some(why) => Verdict::Refused(format!(
            "The host refused this finish: {why}. This is refusal {} of {}; after that, a done \
             finish is recorded as unverified.",
            refusals + 1,
            rule.max_refusals
        )),
    }
}

/// A workspace's files, each with its size and modification time.
pub type Listing = BTreeMap<String, (u64, Option<SystemTime>)>;

fn skipped(name: &str) -> bool {
    name.starts_with('.')
        || matches!(name, "__pycache__" | "node_modules" | "target")
        || name.ends_with(".pyc")
}

/// The files under `root`, leaving out hidden entries, `__pycache__`,
/// `node_modules`, `target`, and `.pyc` files, or `None` past
/// [`LISTING_MAX`] files.
#[must_use]
pub fn listing(root: &Path) -> Option<Listing> {
    let mut files = Listing::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if skipped(&name) {
                continue;
            }
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if kind.is_dir() {
                pending.push(path);
            } else if let Ok(meta) = entry.metadata() {
                let relative = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .into_owned();
                files.insert(relative, (meta.len(), meta.modified().ok()));
                if files.len() > LISTING_MAX {
                    return None;
                }
            }
        }
    }
    Some(files)
}

/// The files added, changed, or removed between two listings.
#[must_use]
pub fn changed_files(before: &Listing, after: &Listing) -> Vec<String> {
    let mut changed: Vec<String> = after
        .iter()
        .filter(|(path, now)| before.get(*path) != Some(now))
        .map(|(path, _)| path.clone())
        .collect();
    changed.extend(before.keys().filter(|p| !after.contains_key(*p)).cloned());
    changed.sort();
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(baseline: &[&str]) -> FinishRule {
        FinishRule {
            baseline: baseline.iter().map(|s| (*s).to_string()).collect(),
            ..FinishRule::score(&["score.sh"])
        }
    }

    #[test]
    fn reading_the_score_script_is_not_a_score_run() {
        let rule = rule(&[]);
        assert_eq!(rule.classify("cat .microluna-eval/score.sh"), Run::Other);
        assert_eq!(rule.classify("sed -n 1,40p score.sh"), Run::Other);
        assert_eq!(
            rule.classify("sh .microluna-eval/score.sh | tail -3"),
            Run::Score
        );
        assert_eq!(
            rule.classify("printf 'echo SCORE 1 1' > .microluna-eval/score.sh"),
            Run::Other
        );
        assert_eq!(
            rule.classify("cat >score.sh <<'EOF'\necho hi\nEOF"),
            Run::Other
        );
    }

    #[test]
    fn a_baseline_command_matches_with_its_spaces_collapsed() {
        let rule = rule(&["python3 -m monitor --input data.csv"]);
        assert_eq!(
            rule.classify("cd /app &&  python3 -m monitor  --input data.csv"),
            Run::Baseline
        );
        assert_eq!(
            rule.classify("python3 -m monitor --input data.csv; sh score.sh"),
            Run::Both
        );
    }

    const DRIFT: &str = "python3 -m drift_monitor data/reference_embeddings.npy \
                         data/current_clear_drift.npy data/current_stable.npy \
                         data/current_with_zeros.npy";

    #[test]
    fn reordered_arguments_match() {
        let rule = rule(&[DRIFT]);
        assert_eq!(
            rule.classify(
                "python3 -m drift_monitor data/reference_embeddings.npy data/current_stable.npy \
                 data/current_with_zeros.npy data/current_clear_drift.npy; echo exit=$?"
            ),
            Run::Baseline
        );
    }

    #[test]
    fn app_paths_and_python_match_under_the_root() {
        let rule = rule(&[DRIFT]);
        let command = "pwd; ls -la; python -m drift_monitor /app/data/reference_embeddings.npy \
                       /app/data/current_stable.npy /app/data/current_clear_drift.npy \
                       ./data/current_with_zeros.npy >/tmp/out.json 2>&1";
        assert_eq!(rule.classify_at(command, Some("/app")), Run::Baseline);
        assert_eq!(rule.classify_at(command, Some("/app/")), Run::Baseline);
        // Without the root, `/app/data/x` isn't `data/x`.
        assert_eq!(rule.classify(command), Run::Other);
        // Another root isn't this one.
        assert_eq!(rule.classify_at(command, Some("/work")), Run::Other);
    }

    #[test]
    fn a_different_file_set_does_not_match() {
        let rule = rule(&[DRIFT]);
        // The stable window three times: the same entry point, fewer files.
        let command = "python -m drift_monitor /app/data/reference_embeddings.npy \
                       /app/data/current_stable.npy /app/data/current_stable.npy \
                       /app/data/current_stable.npy; true";
        assert_eq!(rule.classify_at(command, Some("/app")), Run::Other);
        let fewer =
            "python3 -m drift_monitor data/reference_embeddings.npy data/current_stable.npy";
        assert_eq!(rule.classify_at(fewer, Some("/app")), Run::Other);
        let more = format!("{DRIFT} data/extra.npy");
        assert_eq!(rule.classify_at(&more, Some("/app")), Run::Other);
    }

    #[test]
    fn a_different_module_does_not_match() {
        let rule = rule(&[DRIFT]);
        let command = DRIFT.replace("-m drift_monitor", "-m drift_monitor.cli");
        assert_eq!(rule.classify_at(&command, Some("/app")), Run::Other);
        let script = DRIFT.replace("-m drift_monitor", "drift_monitor/__main__.py");
        assert_eq!(rule.classify_at(&script, Some("/app")), Run::Other);
        assert_eq!(
            rule.classify_at("python3 -m compileall -q drift_monitor", Some("/app")),
            Run::Other
        );
    }

    #[test]
    fn a_here_document_body_is_not_a_run() {
        let rule = rule(&[DRIFT]);
        let body = format!("cat > /tmp/run.sh <<'SH'\n{DRIFT}\nSH\nsh /tmp/check.sh");
        assert_eq!(rule.classify_at(&body, Some("/app")), Run::Other);
        let after = format!("python3 - <<'PY'\nprint(1)\nPY\n{DRIFT} 2>/dev/null");
        assert_eq!(rule.classify_at(&after, Some("/app")), Run::Baseline);
    }

    #[test]
    fn entry_points_fold_interpreters_and_wrappers() {
        let root = Some("/app");
        let base = Invocation::all("python3 run.py --out out.csv in.csv", root);
        for same in [
            "python3.11 /app/run.py in.csv --out out.csv",
            "PYTHONPATH=. timeout 60 python -u ./run.py in.csv --out=x --out out.csv",
            "cd /app && env FOO=1 python3 run.py out.csv in.csv | tail -5",
        ] {
            assert!(Invocation::all(same, root).contains(&base[0]), "{same}");
        }
        assert_eq!(
            Invocation::all("make test", root),
            Invocation::all("/usr/bin/make test", root)
        );
        assert_ne!(
            Invocation::all("make test", root),
            Invocation::all("make check", root)
        );
        assert_eq!(
            Invocation::all("bash ./scripts/run.sh a", root),
            Invocation::all("sh /app/scripts/run.sh a", root)
        );
        assert!(Invocation::all("cd /app; export X=1", root).is_empty());
    }

    #[test]
    fn a_session_that_never_edited_meets_the_rule() {
        let ledger = Ledger::default();
        assert_eq!(
            verdict(&ledger, &rule(&["make"]), FinishStatus::Done, 0),
            Verdict::Allowed
        );
    }

    #[test]
    fn a_score_runs_changes_are_outputs_not_edits() {
        let rule = rule(&[]);
        let mut ledger = Ledger::default();
        ledger.edited("monitor.py");
        ledger.ran(&rule, "sh score.sh", &["out/report.json".to_string()]);
        assert_eq!(ledger.missing(&rule), None);
        ledger.ran(&rule, "python3 fix.py", &["monitor.py".to_string()]);
        assert!(ledger.missing(&rule).unwrap().contains("`monitor.py`"));
    }

    #[test]
    fn listings_show_added_changed_and_removed_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.py"), "1").unwrap();
        std::fs::write(dir.path().join("b.py"), "1").unwrap();
        std::fs::create_dir(dir.path().join("__pycache__")).unwrap();
        let before = listing(dir.path()).unwrap();
        std::fs::write(dir.path().join("a.py"), "22").unwrap();
        std::fs::remove_file(dir.path().join("b.py")).unwrap();
        std::fs::write(dir.path().join("c.py"), "1").unwrap();
        std::fs::write(dir.path().join("__pycache__/a.pyc"), "x").unwrap();
        let after = listing(dir.path()).unwrap();
        assert_eq!(changed_files(&before, &after), ["a.py", "b.py", "c.py"]);
    }
}
