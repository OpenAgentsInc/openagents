//! The lean loop (`executor.microluna.lean`): a strong first session, a
//! few bounded continuations, and a fresh self-check, with no acceptance
//! suite that can reverse a fix.
//!
//! Three Microluna trials, v6 to v8, reached a workspace that passes the
//! verifier and then lost it to their own acceptance suite: a guard written
//! from the untouched code sent a session back to the defect, a writer's
//! wrong expected value made an audit revert a fix, and a green suite
//! accepted a lookup table of the training pairs. One well-briefed session
//! reached the passing workspace on its own. The lean loop keeps that
//! session and adds only what can't reverse it:
//!
//! - **The brief.** The task, general guidance, the task's constraints,
//!   the workspace's current source, and the head of each data file, in
//!   the cached prefix.
//! - **The score** (`keep_best`). The first session writes an evaluation
//!   script before its first change to the solution. The host freezes it,
//!   runs it after every session, and keeps a snapshot of the best-scoring
//!   workspace. The score only chooses among workspaces: nothing is ever
//!   reverted to make it pass.
//! - **Hard-coding** (`hardcode_check`). Code counts the provided data's
//!   fields that a changed file repeats literally, and Jev reads the diff
//!   for a solution that looks up the examples instead of implementing the
//!   rule. A flagged workspace can't be the best, and the next session is
//!   told why.
//! - **The self-check** (`self_check`). A last session on a fresh context
//!   reviews the result against the task. It may not undo a change the
//!   task's words don't show wrong.

use std::collections::{BTreeMap, BTreeSet};

use super::*;

/// `executor.microluna.lean`: the lean loop's shape.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lean {
    /// Work sessions at most, the self-check not counted.
    pub sessions: u32,
    /// Characters of the workspace's current source files in the prefix.
    pub source_chars: usize,
    /// Characters of data-file heads in the prefix.
    #[serde(default)]
    pub sample_chars: usize,
    /// End with a self-check session on a fresh context.
    #[serde(default)]
    pub self_check: bool,
    /// Tell each session to hold out part of any provided examples and to
    /// measure on the held-out part.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub holdout: bool,
    /// Scan each session's changes for hard-coded examples, in code and
    /// with one Jev question.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hardcode_check: bool,
    /// Have the first session write an evaluation script, freeze it, score
    /// the workspace after every session, and finish on the best snapshot.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub keep_best: bool,
    /// The score script's wall-time bound, in seconds.
    #[serde(default = "score_sec")]
    pub score_sec: u64,
    /// When the host turns a work session's `finish` back, or `None` to let
    /// every finish stand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persist: Option<LeanPersist>,
    /// The loop's wall-time bound in seconds, the self-check included: no
    /// session starts in its last minute, and each session ends by it. 0
    /// leaves only the dispatch's deadline.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub wall_sec: u64,
    /// Add the working practices ([`PRACTICES`]) to the guidance.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub practices: bool,
    /// Put the comments that defend a design choice in the evidence, as
    /// suspects (`accept::defended_choices_general`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub defended: bool,
    /// Bound each session's spend by what is left of the dispatch's.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub session_spend: bool,
    /// Count whole records, an input with its answer, in the literal scan
    /// ([`data_records`]) instead of single fields, so a provided word list
    /// a solution may use isn't read as hard-coded examples.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub records: bool,
    /// Every command's wall-time bound in seconds, below the tool's own
    /// 600. 0 keeps the tool's.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub command_sec: u64,
    /// Add the symptom practice alone ([`SYMPTOMS`]), without the search
    /// practice `practices` also carries.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub symptoms: bool,
    /// Add the worked-example practice ([`EXAMPLE_FIRST`]).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub example_first: bool,
    /// Add the standard-form practice ([`STANDARD_FORMS`]).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub standard_forms: bool,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(n: &u64) -> bool {
    *n == 0
}

/// `executor.microluna.lean.persist`: when a work session's finish is
/// turned back ([`microluna::Persist`]). With `keep_best`, a finish also
/// goes back while the evaluation script scores below full.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LeanPersist {
    /// The most finishes turned back in one session.
    pub max_returns: u32,
    /// Turn back a finish whose status isn't `done`.
    pub not_done: bool,
    /// Turns and seconds of the session that must be left for a finish to
    /// go back.
    pub reserve_turns: usize,
    pub reserve_sec: u64,
}

fn score_sec() -> u64 {
    120
}

impl Lean {
    pub(super) fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if !(1..=12).contains(&self.sessions) {
            problems.push("executor.microluna.lean.sessions must be from 1 to 12".to_string());
        }
        if self.score_sec == 0 {
            problems.push("executor.microluna.lean.score_sec must be at least 1".to_string());
        }
        problems
    }
}

/// What every lean session is told, before the task's constraints.
pub const LEAN_GUIDANCE: &str = "You work on the whole task in this session. The evidence below \
holds the workspace's current source files and the head of each data file, so read it before \
you open files. Then:\n\n\
1. Work out exactly what the task requires: every rule, file, format, and command it states, \
and every property the code's own documentation states.\n\
2. Before you change the solution, write a quick check you can rerun that measures what the \
task asks for: run the program on the task's inputs or examples and compare with what the task \
states.\n\
3. Make the change, run your check, and keep iterating until it passes. A comment that defends \
a simplification or a shortcut may describe the defect itself: check it against the task.\n\
4. Don't stop at a partial result while you have turns left and a way to improve it. Use \
`blocked` only when a tool or a piece of information is missing, not because the task is hard. \
Call finish with status `done` only when your check shows the task met, and say in the summary \
what your check measured, with numbers.";

/// Added with `practices`: two working practices the dev-set traces
/// lacked. v9 spent 176 turns on one task adjusting rules by hand, a few
/// pairs per session; and two runs on another fixed five defects each
/// without reproducing the task's symptoms per component, and missed the
/// two the verifier checks.
pub const PRACTICES: &str = "When the task asks you to find rules, parameters, a key, or any \
model that fits data, write a program that searches for them automatically and scores each \
candidate, and improve the search, rather than adjusting the answer by hand one piece at a time.\n\n\
When the task describes symptoms, reproduce each one on the untouched code with a small script \
before you change anything, trace it to the component responsible, and confirm your change \
removes it. Check each component on its own against what it should compute, not only the \
end-to-end result.";

/// Added with `symptoms`: the second half of [`PRACTICES`].
pub const SYMPTOMS: &str = "When the task describes symptoms, reproduce each one on the \
untouched code with a small script before you change anything, trace it to the component \
responsible, and confirm your change removes it. Check each component on its own against what \
it should compute, not only the end-to-end result.";

/// Added with `example_first`. Every Luna run on one dev task guessed the
/// mechanism behind a provided input and output pair and never compared
/// the two; Fable's passing runs compared them first and read the
/// mechanism off the difference.
pub const EXAMPLE_FIRST: &str = "When the task provides an input together with its expected \
output, work out the exact transformation from that pair before you design anything: compute \
what turns each part of the input into the output, and look for the structure in that mapping, \
such as what repeats, what depends on what came before, and what differs between positions. Test \
each hypothesis against the pair, and drop the ones it contradicts.";

/// Added with `standard_forms`. A session kept a well-known statistic in a
/// variant its docstring defended, and the verifier required the
/// standard form.
pub const STANDARD_FORMS: &str = "The standard definition of a well-known method the code \
implements, such as a statistic, an algorithm, a protocol, or a format, is part of what the task \
asks. Where the code or a comment chooses a variant of it, treat the choice as a suspect, and use \
the standard form unless the task says otherwise.";

/// Added with `holdout`: examples are a sample of a rule, not the answer.
pub const HOLDOUT_GUIDANCE: &str = "When the task provides examples, training data, or a sample \
with its answer, treat them as a sample of a general rule, not as the answer. The task is judged \
on inputs you haven't seen. Set part of the examples aside as held out, such as every fifth one, \
develop on the rest, and measure on the held-out part. Never copy the examples, their answers, \
or a table of them into the solution, and never have the solution read a provided answer file.";

/// Added with `keep_best` until the score script exists: where to write it.
#[must_use]
pub fn score_guidance(eval: &Path) -> String {
    format!(
        "Before your first change to the solution, write an evaluation script at \
         `{eval}/score.sh`. It runs the solution in the workspace the way the task will be \
         judged and prints, as its last line, `SCORE <passed> <total>`: how many of the task's \
         stated checks pass, or how many held-out examples come out exactly right. Keep it under \
         60 seconds, and make it score the untouched workspace low. The host freezes a copy of \
         it after this session, runs it after every session, and keeps the workspace that scores \
         best, so a later change that lowers the score is dropped.",
        eval = eval.display()
    )
}

/// What a continuation session is told about the loop.
pub const CONTINUE_GUIDANCE: &str = "Earlier sessions worked on this task; the state below says \
what they did, what the host measured, and what is still open. Continue from the workspace as it \
is now. Don't undo an earlier change unless the task's words show it wrong.";

/// The self-check session's guidance.
pub const CHECK_GUIDANCE: &str = "You are reviewing a finished attempt at this task with fresh \
eyes. The evidence holds the workspace as the attempt left it, and the state holds the changes \
and what the host measured. Check the result against the task, requirement by requirement, by \
running it: every file, format, command, and stated rule. Fix what the task's words show is \
wrong or missing. Don't undo a change unless the task's words show it wrong, and never restore \
behavior only because an old comment, docstring, or test expects it. If everything holds, change \
nothing and call finish with status `done`.";

/// Jev's question on hard-coding.
pub const HARDCODE_QUESTION: &str = "Does the change in `diff` hard-code the task's provided \
examples, their answers, or a table of them, or read a provided answer file, instead of \
implementing the general rule or method the task in `task` describes? `literal` counts the \
provided data's fields each changed file repeats word for word.";

/// A changed file that repeats this many distinct data fields is flagged.
pub const LITERAL_MIN: usize = 20;

/// Above this probability Jev's answer flags hard-coding.
pub const HARDCODE_P: f64 = 0.6;

const DATA_EXTENSIONS: [&str; 7] = ["tsv", "csv", "txt", "json", "jsonl", "dat", "psv"];

/// Whether `path` names a data file: a text table or list, not source.
fn is_data(path: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, ext)| DATA_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
        && !path.to_ascii_lowercase().contains("requirements")
}

/// The provided data's distinct fields, by data file: each line split on
/// tabs, commas, semicolons, and bars, keeping fields of 4 to 200
/// characters that hold a letter.
#[must_use]
pub fn data_fields(workdir: &Path) -> BTreeMap<String, BTreeSet<String>> {
    let mut out = BTreeMap::new();
    for path in parallel::workspace_files(workdir) {
        if !is_data(&path) {
            continue;
        }
        let full = workdir.join(&path);
        if std::fs::metadata(&full).map_or(true, |m| m.len() > 4 * 1024 * 1024) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&full) else {
            continue;
        };
        let mut fields = BTreeSet::new();
        for line in text.lines() {
            for field in line.split(['\t', ',', ';', '|']) {
                let field = field.trim().trim_matches('"');
                let n = field.chars().count();
                if (4..=200).contains(&n) && field.chars().any(char::is_alphabetic) {
                    fields.insert(field.to_string());
                }
            }
        }
        if fields.len() >= LITERAL_MIN {
            out.insert(path, fields);
        }
    }
    out
}

/// Separates a record's fields in [`data_records`]' entries.
const UNIT: char = '\u{1f}';

/// The provided data's distinct records, by data file: each line split on
/// tabs, commas, semicolons, and bars into two or more fields of 2 to 200
/// characters, one of them with a letter. An input with its answer is a
/// record; a word list isn't, since its lines have one field each. A
/// changed file matches a record when it repeats every one of its fields.
#[must_use]
pub fn data_records(workdir: &Path) -> BTreeMap<String, BTreeSet<String>> {
    let mut out = BTreeMap::new();
    for path in parallel::workspace_files(workdir) {
        if !is_data(&path) {
            continue;
        }
        let full = workdir.join(&path);
        if std::fs::metadata(&full).map_or(true, |m| m.len() > 4 * 1024 * 1024) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&full) else {
            continue;
        };
        let mut records = BTreeSet::new();
        for line in text.lines() {
            let fields: Vec<&str> = line
                .split(['\t', ',', ';', '|'])
                .map(|f| f.trim().trim_matches('"'))
                .filter(|f| (2..=200).contains(&f.chars().count()))
                .collect();
            if fields.len() >= 2 && fields.iter().any(|f| f.chars().any(char::is_alphabetic)) {
                records.insert(fields.join(&UNIT.to_string()));
            }
        }
        if records.len() >= LITERAL_MIN {
            out.insert(path, records);
        }
    }
    out
}

/// Whether `text` repeats `entry`: a field, or every field of a record.
fn repeats(text: &str, entry: &str) -> bool {
    entry.split(UNIT).all(|part| text.contains(part))
}

/// For each file changed since `start`, the most distinct fields or
/// records of one data file it repeats literally, when that is at least
/// [`LITERAL_MIN`].
#[must_use]
pub fn literal_examples(
    workdir: &Path,
    start: &BTreeMap<String, String>,
    fields: &BTreeMap<String, BTreeSet<String>>,
) -> Vec<(String, String, usize)> {
    if fields.is_empty() {
        return Vec::new();
    }
    let now = parallel::tree(workdir);
    let mut out = Vec::new();
    for (path, digest) in &now {
        if start.get(path) == Some(digest) || fields.contains_key(path) {
            continue;
        }
        let full = workdir.join(path);
        if std::fs::metadata(&full).map_or(true, |m| m.len() > 8 * 1024 * 1024) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&full) else {
            continue;
        };
        let best = fields
            .iter()
            .map(|(data, set)| (data, set.iter().filter(|f| repeats(&text, f)).count()))
            .max_by_key(|(_, n)| *n);
        if let Some((data, n)) = best
            && n >= LITERAL_MIN
        {
            out.push((path.clone(), data.clone(), n));
        }
    }
    out
}

/// The heads of the workspace's data files, at most `chars` in all.
fn data_samples(workdir: &Path, chars: usize) -> Vec<Evidence> {
    let mut out = Vec::new();
    let mut left = chars;
    for path in parallel::workspace_files(workdir) {
        if !is_data(&path) || left < 300 {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(workdir.join(&path)) else {
            continue;
        };
        let lines = text.lines().count();
        let head: String = text.lines().take(15).collect::<Vec<_>>().join("\n");
        let head = crate::judge::clip(&head, left.min(2_000));
        left = left.saturating_sub(head.chars().count());
        out.push(Evidence {
            label: format!("The head of {path} ({lines} lines)"),
            text: head,
        });
    }
    out
}

/// The last `SCORE <passed> <total>` line in `output`.
#[must_use]
pub fn parse_score(output: &str) -> Option<(u64, u64)> {
    microluna::session::parse_score(output)
}

/// A score as a fraction, for comparing workspaces.
fn fraction(score: Option<(u64, u64)>) -> f64 {
    score.map_or(-1.0, |(p, t)| p as f64 / t as f64)
}

/// Where the first session writes its evaluation script: outside the
/// workspace in a task container, and inside it, hidden, where a boundary
/// lets a session write only the workspace.
#[must_use]
pub fn eval_dir(workdir: &Path, isolation: Isolation) -> PathBuf {
    if isolation == Isolation::TaskContainer {
        std::env::temp_dir().join(format!(
            "microluna-eval-{}",
            &sha256(&workdir.display().to_string())[..12]
        ))
    } else {
        workdir.join(".microluna-eval")
    }
}

/// The finish gate for a session: none for the self-check; with
/// `keep_best`, the frozen score once it exists, and the session's own
/// script before.
fn persist(
    lean: &Lean,
    checking: bool,
    have_score: bool,
    eval: &Path,
    frozen: &Path,
) -> Option<microluna::Persist> {
    let gate = lean.persist.as_ref().filter(|_| !checking)?;
    let score_command = lean.keep_best.then(|| {
        if have_score {
            format!("sh {}/score.sh", frozen.display())
        } else {
            format!(
                "test -f {e}/score.sh && sh {e}/score.sh",
                e = eval.display()
            )
        }
    });
    Some(microluna::Persist {
        max_returns: gate.max_returns,
        not_done: gate.not_done,
        score_command,
        reserve_turns: gate.reserve_turns,
        reserve_sec: gate.reserve_sec,
    })
}

/// The best workspace so far.
struct Best {
    session: u32,
    score: Option<(u64, u64)>,
    dir: PathBuf,
}

impl Micro {
    /// The workspace's source files, those in `first` first, at most
    /// `chars` characters.
    fn sources_within(&self, workdir: &Path, first: &[String], chars: usize) -> Vec<Evidence> {
        let sources = crate::accept::source_files(workdir, 200);
        let mut ordered: Vec<&String> = first.iter().filter(|f| sources.contains(f)).collect();
        ordered.extend(sources.iter().filter(|f| !first.contains(f)));
        let mut left = chars;
        let mut out = Vec::new();
        for path in ordered {
            let Ok(text) = std::fs::read_to_string(workdir.join(path)) else {
                continue;
            };
            let size = text.chars().count();
            if size > left {
                continue;
            }
            left -= size;
            out.push(Evidence {
                label: format!("The current {path}"),
                text,
            });
            if left < 200 {
                break;
            }
        }
        out
    }

    /// Runs the frozen score script against the workspace.
    async fn lean_score(&self, frozen: &Path, lean: &Lean) -> (Option<(u64, u64)>, String) {
        let script = frozen.join("score.sh");
        if !script.is_file() {
            return (None, "no score script".to_string());
        }
        let wall = Duration::from_secs(lean.score_sec);
        let ended = match self.isolation {
            Isolation::TaskContainer => {
                let mut command = std::process::Command::new("/bin/sh");
                command.arg(&script).current_dir(&self.workdir);
                supervise::Job::from_command(command)
                    .bounded(supervise::Limits::within(wall).keeping(64 * 1024))
                    .run()
                    .await
            }
            _ => {
                let spec = if self.isolation == Isolation::ReadOnly {
                    coder_boundary::Boundary::readonly()
                } else {
                    coder_boundary::Boundary::writing(&self.workdir)
                };
                let boundary = match spec.owned_scratch_under(std::env::temp_dir()).build() {
                    Ok(boundary) => boundary,
                    Err(error) => return (None, format!("no enforced boundary: {error}")),
                };
                let mut command = match boundary.command("/bin/sh", [script.as_os_str()]) {
                    Ok(command) => command,
                    Err(error) => return (None, error.to_string()),
                };
                command.current_dir(&self.workdir);
                supervise::Job::from_command(command)
                    .bounded(supervise::Limits::within(wall).keeping(64 * 1024))
                    .run_holding(boundary.hold())
                    .await
            }
        };
        let stdout = ended.stdout.marked();
        let stderr = ended.stderr.marked();
        let tail = crate::judge::clip(
            &format!("{}\n{}", stdout.trim_end(), stderr.trim_end()),
            1_200,
        );
        (parse_score(&stdout), tail)
    }

    /// Whether the workspace hard-codes the examples: the literal scan, and
    /// Jev over the diff.
    async fn hardcoded(
        &self,
        prepared: &Prepared,
        base: Option<&Path>,
        literal: &[(String, String, usize)],
        number: u32,
    ) -> (bool, Value, f64) {
        let diff = match base {
            Some(base) => crate::delegate::changes_since(base, &self.workdir),
            None => crate::delegate::changes(&self.workdir, None),
        };
        let state = json!({
            "task": clip_lines(&prepared.instruction, 4_000),
            "diff": crate::judge::clip(&diff, 8_000),
            "literal": literal.iter().map(|(file, data, n)| format!("{file} repeats {n} distinct fields of {data}")).collect::<Vec<_>>(),
        });
        let asked = jev_component::ask(
            &prepared.jev,
            &self.recorder,
            jev_component::Ask {
                component: "microluna.lean",
                name: "jev_hardcoded",
                id: format!("jev-hardcoded-{}-{number}", self.dispatch()),
                state,
                questions: jev::Questions::new()
                    .with("hardcoded", jev::Noul::new(HARDCODE_QUESTION)),
                parent: None,
                deadline: prepared.deadline.clone(),
            },
        )
        .await;
        let p = asked.noul("hardcoded");
        let usd = asked.input_tokens.map_or(0.0, |t| {
            t as f64 * jev_component::USD_PER_MILLION_INPUT / 1_000_000.0
        });
        let flagged = !literal.is_empty() || p.is_some_and(|p| p >= HARDCODE_P);
        (
            flagged,
            json!({
                "literal": literal.iter().map(|(file, data, n)| json!({"file": file, "data": data, "fields": n})).collect::<Vec<_>>(),
                "p": p,
                "flagged": flagged,
                "jev": { "how": asked.how, "error": asked.error },
            }),
            usd,
        )
    }

    /// The lean loop. Returns the sessions, the record of each step, and
    /// why it stopped.
    #[allow(clippy::too_many_lines)]
    pub(super) async fn lean_loop(
        &self,
        prepared: &Prepared,
        lean: &Lean,
    ) -> (Vec<Ran>, Vec<Value>, String) {
        let started = Instant::now();
        let time_left = || {
            self.episode
                .allowance()
                .map_or(self.deadline, |left| left.min(self.deadline))
                .saturating_sub(started.elapsed())
        };
        let everything = Group {
            ids: prepared
                .requirements
                .requirements
                .iter()
                .map(|r| r.id.clone())
                .collect(),
            lines: Vec::new(),
        };
        let evidence = evidence_for(prepared, &everything, self.policy.evidence_chars);
        let mut general = LEAN_GUIDANCE.to_string();
        if lean.practices {
            general.push_str("\n\n");
            general.push_str(PRACTICES);
        }
        for (on, text) in [
            (lean.symptoms, SYMPTOMS),
            (lean.example_first, EXAMPLE_FIRST),
            (lean.standard_forms, STANDARD_FORMS),
        ] {
            if on {
                general.push_str("\n\n");
                general.push_str(text);
            }
        }
        if lean.holdout {
            general.push_str("\n\n");
            general.push_str(HOLDOUT_GUIDANCE);
        }
        if lean.command_sec > 0 {
            general.push_str(&format!(
                "\n\nEvery command ends after {} seconds, whatever bound you ask for, so bound \
                 each search or long run by time and have it print its best result so far.",
                lean.command_sec
            ));
        }
        let facts = constraints(&prepared.requirements);
        let rules = if facts.is_empty() {
            String::new()
        } else {
            format!(
                "\n\nThe task's constraints hold throughout; honor each one exactly:\n\n{}",
                facts.join("\n")
            )
        };
        let files = parallel::workspace_files(&self.workdir);
        let named = parallel::files_named(&prepared.instruction, &files);
        let mut samples = data_samples(&self.workdir, lean.sample_chars);
        if lean.defended {
            let defended = crate::accept::defended_choices_general(&self.workdir);
            if !defended.is_empty() {
                samples.insert(0, crate::accept::defended_evidence(&defended, true));
            }
        }
        let wall = (lean.wall_sec > 0).then(|| Duration::from_secs(lean.wall_sec));
        let wall_left = || wall.map(|w| w.saturating_sub(started.elapsed()));
        let fields = if lean.hardcode_check && lean.records {
            data_records(&self.workdir)
        } else if lean.hardcode_check {
            data_fields(&self.workdir)
        } else {
            BTreeMap::new()
        };
        let start_tree = parallel::tree(&self.workdir);
        // A copy of the untouched workspace, for a real diff.
        let base = parallel::copyable(&self.workdir).then(|| scratch("lean-base"));
        let base = base.filter(|dir| crate::handoff::copy_tree(&self.workdir, dir).is_ok());
        let _base_cleanup = Cleanup(base.clone());
        let eval = eval_dir(&self.workdir, self.isolation);
        let frozen = scratch("microluna-eval-frozen");
        let _frozen_cleanup = Cleanup(Some(frozen.clone()));
        let _eval_cleanup = Cleanup(
            (self.isolation != Isolation::TaskContainer || lean.keep_best).then(|| eval.clone()),
        );
        if lean.keep_best {
            let _ = std::fs::create_dir_all(&eval);
        }
        let mut have_score = false;
        let mut best: Option<Best> = None;
        let mut best_cleanup: Vec<PathBuf> = Vec::new();
        let mut sessions: Vec<Ran> = Vec::new();
        let mut moves: Vec<Value> = Vec::new();
        let mut spent = 0.0;
        let mut history: Vec<String> = Vec::new();
        let mut flag_note: Option<String> = None;
        let mut stopped = String::new();
        let total = lean.sessions + u32::from(lean.self_check);
        let mut number = 0;
        let mut checking = false;
        loop {
            number += 1;
            if !checking && number > lean.sessions {
                stopped = format!("the lean loop used its {} sessions", lean.sessions);
                if !lean.self_check {
                    break;
                }
                checking = true;
            }
            if spent >= self.policy.spend_usd {
                stopped = format!("the spend bound ${:.2} was reached", self.policy.spend_usd);
                break;
            }
            if time_left() < Duration::from_secs(60)
                || wall_left().is_some_and(|w| w < Duration::from_secs(60))
            {
                stopped = "the dispatch's time ran out".to_string();
                break;
            }
            let mut guidance = if checking {
                CHECK_GUIDANCE.to_string()
            } else if number == 1 {
                general.clone()
            } else {
                format!("{general}\n\n{CONTINUE_GUIDANCE}")
            };
            if lean.keep_best && !have_score && !checking {
                guidance.push_str("\n\n");
                guidance.push_str(&score_guidance(&eval));
            }
            if checking && lean.holdout {
                guidance.push_str("\n\n");
                guidance.push_str(HOLDOUT_GUIDANCE);
            }
            guidance.push_str(&rules);
            let mut session_evidence = evidence.clone();
            session_evidence.extend(self.sources_within(&self.workdir, &named, lean.source_chars));
            session_evidence.extend(samples.iter().cloned());
            let mut state = vec![format!(
                "Session {number} of at most {total}{}.",
                if checking { ", the self-check" } else { "" }
            )];
            if number > 1 {
                for ran in sessions.iter().rev().take(3).rev() {
                    state.push(format!(
                        "Session {} ended {}: {}",
                        ran.number,
                        ran.status(),
                        crate::judge::clip(&ran.summary(), 600)
                    ));
                }
                state.extend(history.iter().cloned());
                if let Some(note) = &flag_note {
                    state.push(note.clone());
                }
                let changes = match &base {
                    Some(base) => crate::delegate::changes_since(base, &self.workdir),
                    None => crate::delegate::changes(&self.workdir, None),
                };
                state.push(format!(
                    "What the workspace shows as changed since the start:\n{}",
                    crate::judge::clip(&changes, 3_000)
                ));
                if have_score {
                    state.push(format!(
                        "The host's frozen evaluation script is {}/score.sh; run `sh {}/score.sh` \
                         from the workspace to see the score the host sees.",
                        frozen.display(),
                        frozen.display()
                    ));
                }
            }
            let brief = Brief {
                task: prepared.instruction.clone(),
                guidance,
                evidence: session_evidence,
                state,
            };
            let why = if checking {
                "the self-check reviews the result on a fresh context".to_string()
            } else if number == 1 {
                "the lean loop's first session takes the whole task".to_string()
            } else {
                "the lean loop continues the task".to_string()
            };
            let ran = self
                .session_at(
                    number,
                    &[if checking {
                        "the self-check".to_string()
                    } else {
                        "the whole task".to_string()
                    }],
                    &why,
                    &brief,
                    false,
                    Place {
                        persist: persist(lean, checking, have_score, &eval, &frozen),
                        deadline: wall_left(),
                        command_max: (lean.command_sec > 0)
                            .then(|| Duration::from_secs(lean.command_sec)),
                        spend_usd: lean
                            .session_spend
                            .then(|| (self.policy.spend_usd - spent).max(0.0)),
                        group: Some(if checking {
                            "the self-check".to_string()
                        } else {
                            "the whole task".to_string()
                        }),
                        ..Place::default()
                    },
                )
                .await;
            spent += ran.cost_usd.unwrap_or(0.0);
            let status = ran.status();
            let lost = matches!(ran.ending, Ending::Transport(_));
            sessions.push(ran);
            if lost {
                stopped = format!("session {number} lost its provider");
                break;
            }
            // Freeze the score script the first time it exists.
            if lean.keep_best && !have_score && eval.join("score.sh").is_file() {
                have_score = crate::handoff::copy_tree(&eval, &frozen).is_ok();
            }
            let (score, score_tail) = if have_score {
                self.lean_score(&frozen, lean).await
            } else {
                (None, String::new())
            };
            let (flagged, flag_record, jev_usd) = if lean.hardcode_check {
                let literal = literal_examples(&self.workdir, &start_tree, &fields);
                self.hardcoded(prepared, base.as_deref(), &literal, number)
                    .await
            } else {
                (false, Value::Null, 0.0)
            };
            spent += jev_usd;
            flag_note = flagged.then(|| {
                format!(
                    "The host flagged the workspace after session {number} as hard-coding the \
                     task's examples ({}). A flagged workspace can't be kept. Replace any table \
                     or copy of the examples with the general rule the task describes, and \
                     check it on examples you held out.",
                    flag_record["literal"]
                        .as_array()
                        .filter(|l| !l.is_empty())
                        .map_or_else(
                            || "Jev read the diff as a lookup".to_string(),
                            |l| l
                                .iter()
                                .map(|x| format!(
                                    "{} repeats {} fields of {}",
                                    x["file"].as_str().unwrap_or(""),
                                    x["fields"],
                                    x["data"].as_str().unwrap_or("")
                                ))
                                .collect::<Vec<_>>()
                                .join("; ")
                        )
                )
            });
            if let Some((p, t)) = score {
                history.push(format!(
                    "After session {number} the host's score was {p} of {t}{}.",
                    if flagged {
                        ", but the workspace was flagged"
                    } else {
                        ""
                    }
                ));
            } else if have_score {
                history.push(format!(
                    "After session {number} the host's score script printed no SCORE line:\n{}",
                    crate::judge::clip(&score_tail, 600)
                ));
            }
            // Keep the best: a flagged workspace never counts, and a tie
            // goes to the later session.
            let mut kept = false;
            if lean.keep_best && !flagged {
                let better = best
                    .as_ref()
                    .is_none_or(|b| fraction(score) >= fraction(b.score));
                if better && parallel::copyable(&self.workdir) {
                    let dir = scratch("lean-best");
                    if crate::handoff::copy_tree(&self.workdir, &dir).is_ok() {
                        best_cleanup.push(dir.clone());
                        best = Some(Best {
                            session: number,
                            score,
                            dir,
                        });
                        kept = true;
                    }
                }
            }
            crate::say::line(&format!(
                "  microluna ▸ session {number} {status}; {}; {}{}",
                score.map_or("no tests ran".to_string(), |(p, t)| format!(
                    "{p} of {t} tests pass"
                )),
                if flagged {
                    "the answer looks hard-coded"
                } else {
                    "the answer doesn't look hard-coded"
                },
                if kept {
                    "; kept as the best so far"
                } else {
                    ""
                }
            ));
            moves.push(json!({
                "kind": "lean",
                "after_session": number,
                "self_check": checking,
                "status": status,
                "score": score.map(|(p, t)| json!({"passed": p, "total": t})),
                "score_tail": score_tail,
                "hardcoded": flag_record,
                "kept": kept,
                "spent_usd": spent,
            }));
            if checking {
                stopped.push_str(&format!(
                    "{}the self-check ended {status}",
                    if stopped.is_empty() { "" } else { "; " }
                ));
                break;
            }
            let full = score.is_some_and(|(p, t)| p >= t);
            let settled = status == "done" && !flagged && (!lean.keep_best || !have_score || full);
            if settled {
                stopped = format!("session {number} ended done");
                if !lean.self_check {
                    break;
                }
                checking = true;
            }
        }
        // Finish on the best workspace when the last one scores lower or is
        // flagged.
        if let Some(b) = &best {
            let last = moves.last();
            let last_flagged = last.is_some_and(|m| m["hardcoded"]["flagged"] == true);
            let last_score = last.and_then(|m| {
                Some((
                    m["score"]["passed"].as_u64()?,
                    m["score"]["total"].as_u64()?,
                ))
            });
            if b.session != sessions.len() as u32
                && (last_flagged || fraction(b.score) > fraction(last_score))
            {
                match crate::compose::replace_contents(&self.workdir, &b.dir) {
                    Ok(()) => {
                        stopped.push_str(&format!(
                            "; the host restored session {}'s workspace, the best by score",
                            b.session
                        ));
                        moves.push(json!({"kind": "lean.restore", "session": b.session, "score": b.score.map(|(p, t)| json!({"passed": p, "total": t}))}));
                    }
                    Err(error) => {
                        stopped
                            .push_str(&format!("; restoring the best workspace failed: {error}"));
                    }
                }
            }
        }
        for dir in best_cleanup {
            let _ = std::fs::remove_dir_all(dir);
        }
        (sessions, moves, stopped)
    }
}
