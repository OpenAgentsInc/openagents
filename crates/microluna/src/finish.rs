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
//! - **A baseline run** is a command that doesn't only read and contains
//!   one of [`FinishRule::baseline`], spaces collapsed. The host finds the
//!   baseline commands (the task's own program, issue #9633); with none,
//!   only the score is required.
//!
//! Issue #9638 and `docs/coder/design/microluna-v18.md`, change 7, record
//! why.

use std::collections::BTreeMap;
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

    /// What `command` is, as the rule counts it. A command that only reads
    /// is neither a score nor a baseline run, whatever it names.
    #[must_use]
    pub fn classify(&self, command: &str) -> Run {
        if command.trim().is_empty()
            || crate::tools::reads_only("run_command", &json!({ "command": command }).to_string())
        {
            return Run::Other;
        }
        let flat = collapse(command);
        let score = self
            .score
            .iter()
            .any(|name| !name.trim().is_empty() && names_to_run(command, name.trim()));
        let baseline = self
            .baseline
            .iter()
            .any(|base| !base.trim().is_empty() && flat.contains(&collapse(base)));
        match (score, baseline) {
            (true, true) => Run::Both,
            (true, false) => Run::Score,
            (false, true) => Run::Baseline,
            (false, false) => Run::Other,
        }
    }
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

fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
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
}

impl Ledger {
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
        let run = rule.classify(command);
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
