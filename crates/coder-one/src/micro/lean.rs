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
use crate::accept::authority::{self, Authority, Powers};
use crate::checks::contract::executed;

/// File-content identity for candidate evidence. Match the merge inventory's
/// exclusions, but refuse incomplete reads instead of comparing partial trees.
/// Git metadata, Python bytecode, and the named cache directories are excluded.
pub(super) fn evidence_tree(dir: &Path) -> Result<BTreeMap<String, String>, String> {
    let mut tree = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        for entry in std::fs::read_dir(at).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            let kind = entry.file_type().map_err(|e| e.to_string())?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if kind.is_dir() {
                if !parallel::UNMERGED.contains(&name.as_ref()) {
                    stack.push(path);
                }
                continue;
            }
            if name.ends_with(".pyc") {
                continue;
            }
            let relative = path.strip_prefix(dir).map_err(|e| e.to_string())?;
            let relative = relative.to_str().ok_or("file path is not UTF-8")?;
            let bytes = if kind.is_symlink() {
                let target = std::fs::read_link(&path).map_err(|e| e.to_string())?;
                format!(
                    "link:{}",
                    target.to_str().ok_or("link target is not UTF-8")?
                )
                .into_bytes()
            } else if kind.is_file() {
                std::fs::read(&path).map_err(|e| e.to_string())?
            } else {
                return Err(format!("unsupported candidate entry: {}", path.display()));
            };
            tree.insert(relative.to_string(), crate::accept::sha256(&bytes));
        }
    }
    Ok(tree)
}

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
    /// Keep candidate snapshots and the evaluator in the artifacts, prefer
    /// an earlier tie, and validate the submitted workspace again.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub protect_candidates: bool,
    /// Retain every candidate, each sequential session's and each lane's,
    /// and the evaluator without changing selection, review permissions,
    /// or stopping. Records observation only, except that a retained lane
    /// is scored by a fresh copy of the frozen scorer.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub retain_candidates: bool,
    /// The final review reads files and host-recorded evidence only. It
    /// cannot run commands or modify the candidate.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub observe_review: bool,
    /// Scan the source for comments that give a reason for a choice
    /// (`accept::rationale_choices`), have Jev rank each as a likely
    /// defect against the task, and put the likely ones in every brief as
    /// suspects each session must decide on.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub rationale: bool,
    /// `evidence.departures`: more suspect sources beside `rationale`'s
    /// comments ([`crate::departures`], issue #9634), each row showing its
    /// kind. Only the sources the offline measurement admitted
    /// ([`crate::departures::ADMITTED`]) are accepted. Empty, the
    /// default, leaves the v13 suspects as they were.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub departures: Vec<crate::departures::Source>,
    /// Attempts at the first work session that run at once, each in its
    /// own copy of the workspace with a different approach, before the
    /// sequential sessions: the host keeps the best by the frozen score.
    /// Above 1 it needs `keep_best`, and a scorer session writes the
    /// evaluation script first. 0 or 1 runs one first session in place.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub lanes: u32,
    /// The lanes' wall-time bound in seconds; 0 leaves the loop's.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub lane_sec: u64,
    /// Put the tail of the frozen score's last output, the failures it
    /// names, in each later brief, and add the failure practice
    /// ([`FAILURES`]).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub failures: bool,
    /// Add the structure practice ([`STRUCTURE`]).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub structure: bool,
    /// Stall detection and a typed next-step choice between work
    /// sessions ([`crate::stall`], issue #9627). Absent, as in every
    /// manifest before it, the loop runs as it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detect: Option<crate::stall::Detect>,
    /// Hold a work session's `done` finish until the score and a baseline
    /// command ran after its last edit ([`microluna::finish`], issue
    /// #9638). Needs `keep_best`, whose `score.sh` is the score. Absent, as
    /// in every manifest before it, every finish stands.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finish_rule: Option<LeanFinishRule>,
    /// `evidence.baseline`: before session 1, run the task's own entry
    /// points once in a bounded scratch copy, put what they did in every
    /// brief as "Baseline behavior", record each run in
    /// `executed-commands.jsonl`, and give the commands to the finish rule
    /// ([`crate::baseline`], issue #9633). Absent, as in every manifest
    /// before it, nothing runs before the session.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub baseline: bool,
    /// `verify.executed` (issue #9636): after every session, rerun the
    /// baseline commands, the commands the instruction names, and a
    /// compile or import of the package on a scratch copy of the
    /// candidate ([`crate::checks::contract::executed`]), and reject a
    /// candidate on which a command that exited 0 on the untouched
    /// workspace now fails. A rejected candidate isn't kept, and the next
    /// session is told why. Needs `keep_best`. Absent, as in every
    /// manifest before it, no command runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executed: Option<LeanExecuted>,
    /// A classified acceptance suite beside the loop, each test's power
    /// set by its authority class ([`Tiered`], issue #9629). Absent, as in
    /// every manifest before it, the loop runs as it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tiered: Option<Box<Tiered>>,
    /// `control.review`: start the self-check only when a trigger fires
    /// ([`crate::review_rule`], issue #9637), and have it report concerns
    /// by requirement ID. Needs `self_check`. Absent, as in every manifest
    /// before it, the self-check always runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_rule: Option<crate::review_rule::Params>,
}

/// `executor.microluna.lean.executed`: the bounds of the commands the host
/// reruns after every session.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LeanExecuted {
    /// Each command's wall-time bound, in seconds.
    #[serde(default = "executed_command_sec")]
    pub command_sec: u64,
    /// All of one run's commands together, in seconds.
    #[serde(default = "executed_budget_sec")]
    pub budget_sec: u64,
}

fn executed_command_sec() -> u64 {
    crate::checks::contract::executed::COMMAND_SEC
}

fn executed_budget_sec() -> u64 {
    300
}

/// `executor.microluna.lean.finish_rule`: how a `done` finish is held to
/// the score and the baseline commands.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LeanFinishRule {
    /// Refusals before a `done` finish is accepted as unverified.
    #[serde(default = "max_refusals")]
    pub max_refusals: u32,
}

fn max_refusals() -> u32 {
    microluna::finish::MAX_REFUSALS
}

/// What a command names to count as a score run: the evaluation script,
/// wherever its copy lives.
pub const SCORE_NAME: &str = "score.sh";

/// The finish rule for a lean session, when the manifest turns it on.
/// `baseline` is the task's baseline commands (`evidence.baseline`, issue
/// #9633), empty when the manifest doesn't run it, so the rule then
/// requires the score alone.
fn finish_rule(lean: &Lean, baseline: &[String]) -> Option<microluna::FinishRule> {
    lean.finish_rule.as_ref().map(|rule| microluna::FinishRule {
        score: vec![SCORE_NAME.to_string()],
        baseline: baseline.to_vec(),
        max_refusals: rule.max_refusals,
    })
}

/// `executor.microluna.lean.tiered`: an acceptance suite, written and
/// frozen before the first session, whose tests carry authority classes
/// (`accept::authority`). The suite runs after every work session, and
/// each class has only the power the policy grants it:
///
/// - A red test of a class in `hold` turns a finish back: the loop can't
///   stop on it. Its green is necessary, never sufficient: the loop's own
///   stopping rules still apply. Only an executed contract or an
///   independently supported test may hold.
/// - A class in `rank` breaks ties between candidates for keep-best, after
///   the holding classes and the frozen score. It never makes the host
///   restore an earlier workspace over the last one, and the sessions
///   never see its tests.
/// - A guard that turns red is a suspect: the next session reads it as
///   possibly pinning the defect, never as an order to restore it.
///
/// A class may hold or rank only once it has passed the offline bar
/// (`accept::authority::PROMOTED`). With `hold` and `rank` empty the suite
/// only observes: it is written, classified, run, and recorded, and a red
/// guard is named as a suspect.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tiered {
    /// How `accept.define` writes the suite.
    pub writer: SuiteWriter,
    /// The classes whose red tests hold the loop.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hold: Vec<Authority>,
    /// The classes that break ties between candidates.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rank: Vec<Authority>,
}

impl Tiered {
    fn validate(&self, lean: &Lean) -> Vec<String> {
        let mut problems = self.writer.validate();
        for class in &self.hold {
            if !class.can_hold() {
                problems.push(format!(
                    "executor.microluna.lean.tiered.hold: a {} test can't hold the loop",
                    class.word()
                ));
            }
        }
        for class in &self.rank {
            if !class.can_rank() {
                problems.push(format!(
                    "executor.microluna.lean.tiered.rank: a {} test can't rank candidates",
                    class.word()
                ));
            }
        }
        for class in self.hold.iter().chain(&self.rank) {
            if !crate::accept::authority::PROMOTED.contains(class) {
                problems.push(format!(
                    "executor.microluna.lean.tiered: {} hasn't passed the offline bar, so it \
                     can't hold power in a policy",
                    class.word()
                ));
            }
        }
        if !self.rank.is_empty() && !lean.keep_best {
            problems.push("executor.microluna.lean.tiered.rank requires keep_best".to_string());
        }
        if lean.lanes > 1 {
            problems.push("executor.microluna.lean.tiered doesn't run with lanes".to_string());
        }
        problems
    }
}

/// Added with `structure`. Written after reading the two search tasks'
/// traces beside Fable's passes, so it is in-sample for them: every Luna
/// run on one filtered the input to letters before looking for the
/// transformation's structure, which lived in the unfiltered positions;
/// runs on the other never considered intermediate forms between ordered
/// steps.
pub const STRUCTURE: &str = "Before you simplify the input for analysis, such as dropping \
punctuation, spacing, or case, check that what you drop plays no role in the transformation: \
positions may count every character. When the transformation is a sequence of ordered steps, a \
later step can act on an intermediate form that appears in neither the input nor the output, so \
reason about the order of steps from the cases that still fail.";

/// Added with `failures`. Sessions on the search tasks edited toward
/// the score without the score's own list of what still failed.
pub const FAILURES: &str = "Work from the failures. After each change, look at what the \
evaluation still fails, group the failures by the smallest difference they share, and fix the \
largest group first; make your evaluation print the failing cases so you can see them.";

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero_u32(n: &u32) -> bool {
    *n == 0
}

/// What the scorer session is told: write the evaluation script only.
#[must_use]
pub fn scorer_guidance(eval: &Path) -> String {
    format!(
        "This session writes only the evaluation script; other sessions write the solution \
         afterwards. Don't change the solution or the task's files. Read the task and the \
         evidence, then write `{eval}/score.sh`. It runs the solution in the workspace the way \
         the task will be judged and prints, as its last line, `SCORE <passed> <total>`: how \
         many of the task's stated checks pass, or how many held-out examples come out exactly \
         right. Make it fine-grained, so partial progress raises the score, keep it under 60 \
         seconds, and make it score the untouched workspace low without failing to run. Run it \
         once on the untouched workspace, then call finish.",
        eval = eval.display()
    )
}

/// The approach each lane is told to take, by lane.
pub const LANE_APPROACHES: [&str; 4] = [
    "Take the approach you judge most likely to work.",
    "Take a substantially different approach from the most obvious one.",
    "Question the most natural assumption about the task, and take the approach that follows \
     if it is wrong.",
    "Start from the smallest example the task gives, get it exactly right, and generalize from \
     there.",
];

/// What a lane session is told about its copy.
#[must_use]
pub fn lane_note(k: usize, n: usize, lane: &Path, real: &Path, scorer: &Path) -> String {
    format!(
        "{n} sessions attempt this task at the same time, each in a private copy of the \
         workspace, and the host keeps the best by the frozen evaluation score. You are attempt \
         {k} of {n}, and you work in `{lane}`, a copy of `{real}`: wherever the task or the \
         evidence names `{real}`, use `{lane}` instead, and never write under `{real}`. The \
         evaluation script for your copy is `{scorer}/score.sh`; run `sh {scorer}/score.sh` to \
         see your score. {}",
        LANE_APPROACHES[(k - 1) % LANE_APPROACHES.len()],
        lane = lane.display(),
        real = real.display(),
        scorer = scorer.display(),
    )
}

/// `text` with every whole-path mention of `from` changed to `to`: `from`
/// followed by a path separator, a quote, a space, a bracket, or the end.
#[must_use]
pub fn rebase_text(text: &str, from: &str, to: &str) -> String {
    if from.is_empty() {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(from) {
        let after = rest[at + from.len()..].chars().next();
        out.push_str(&rest[..at]);
        if after.is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == '-' || c == '.')) {
            out.push_str(to);
        } else {
            out.push_str(from);
        }
        rest = &rest[at + from.len()..];
    }
    out.push_str(rest);
    out
}

/// A copy of the frozen scorer in `to` that scores `lane` instead of
/// `real`.
fn rebase_scorer(frozen: &Path, to: &Path, real: &Path, lane: &Path) -> Result<(), String> {
    crate::handoff::copy_tree(frozen, to)?;
    let (real, lane) = (real.display().to_string(), lane.display().to_string());
    for file in parallel::workspace_files(to) {
        let path = to.join(&file);
        if let Ok(text) = std::fs::read_to_string(&path) {
            let rebased = rebase_text(&text, &real, &lane);
            if rebased != text {
                std::fs::write(&path, rebased).map_err(|e| format!("{}: {e}", path.display()))?;
            }
        }
    }
    Ok(())
}

/// Jev's question on each comment that gives a reason for a choice.
#[must_use]
pub fn suspect_question(j: usize) -> String {
    format!(
        "Could the behavior that the comment in `comments[{j}]` describes or justifies cause one \
         of the problems the task in `task` describes, or depart from what the task asks?"
    )
}

/// Above this probability a comment is a likely defect.
pub const SUSPECT_P: f64 = 0.5;

/// What a session is told about the ranked suspects.
pub const SUSPECTS_NOTE: &str = "The comments below give a reason for a choice in the code, and \
Jev read each as a likely cause of a problem the task describes. A comment is a claim, not a \
specification. Decide each one explicitly against the task: fix it when it causes a described \
problem or departs from the standard form, and say in your finish summary what you decided for \
each.";

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
        if self.protect_candidates && !self.keep_best {
            problems.push("protect_candidates requires keep_best".to_string());
        }
        if self.finish_rule.is_some() && !self.keep_best {
            problems.push("finish_rule requires keep_best".to_string());
        }
        if let Some(executed) = &self.executed {
            if !self.keep_best {
                problems.push("executed requires keep_best".to_string());
            }
            if executed.command_sec == 0 || executed.budget_sec == 0 {
                problems.push(
                    "executor.microluna.lean.executed bounds must be at least 1 second".to_string(),
                );
            }
        }
        if self.review_rule.is_some() && !self.self_check {
            problems.push("review_rule requires self_check".to_string());
        }
        for source in &self.departures {
            if !crate::departures::ADMITTED.contains(source) {
                problems.push(format!(
                    "executor.microluna.lean.departures names {}, which the offline measurement \
                     didn't admit",
                    source.word()
                ));
            }
        }
        if !self.departures.is_empty() && !self.rationale {
            problems.push("executor.microluna.lean.departures requires rationale".to_string());
        }
        if let Some(tiered) = &self.tiered {
            problems.extend(tiered.validate(self));
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

/// The host enforces this review with file reads and finish only.
pub const OBSERVE_GUIDANCE: &str = "Review the candidate against every requirement in the task. \
You can read files and finish; commands and edits are disabled. The host's score is a limited \
self-test, not evidence that every requirement is met. Identify missing requirements, unsupported \
assumptions, and checks whose expected values merely repeat current behavior. Cite the task text \
and source file for each finding. If the evidence is incomplete, finish as blocked and say what \
would need to be tested. Do not call the task verified from a green self-test alone.";

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

impl Micro {
    /// The comments that give a reason for a choice, ranked by Jev as
    /// likely defects: the evidence for the likely ones, the record, and
    /// Jev's cost.
    async fn rank_suspects(&self, prepared: &Prepared) -> (Option<Evidence>, Value, f64) {
        let comments = crate::accept::rationale_choices(&self.workdir);
        if comments.is_empty() {
            return (None, Value::Null, 0.0);
        }
        let mut questions = jev::Questions::new();
        for j in 0..comments.len() {
            questions = questions.with(
                format!("suspect_{j}"),
                jev::Noul::new(suspect_question(j).as_str()),
            );
        }
        let asked = jev_component::ask(
            &prepared.jev,
            &self.recorder,
            jev_component::Ask {
                component: "microluna.lean",
                name: "jev_suspects",
                id: format!("jev-suspects-{}", self.dispatch()),
                state: json!({
                    "task": clip_lines(&prepared.instruction, 4_000),
                    "comments": comments,
                }),
                questions,
                parent: None,
                deadline: prepared.deadline.clone(),
            },
        )
        .await;
        let usd = asked.input_tokens.map_or(0.0, |t| {
            t as f64 * jev_component::USD_PER_MILLION_INPUT / 1_000_000.0
        });
        let mut ranked: Vec<(f64, &String)> = comments
            .iter()
            .enumerate()
            .map(|(j, c)| (asked.noul(&format!("suspect_{j}")).unwrap_or(0.0), c))
            .collect();
        ranked.sort_by(|a, b| b.0.total_cmp(&a.0));
        let likely: Vec<String> = ranked
            .iter()
            .filter(|(p, _)| *p >= SUSPECT_P)
            .take(8)
            .map(|(p, c)| format!("{c} (p = {p:.2})"))
            .collect();
        let record = json!({
            "kind": "lean.suspects",
            "comments": ranked.iter().map(|(p, c)| json!({"comment": c, "p": p})).collect::<Vec<_>>(),
            "likely": likely.len(),
            "jev": { "how": asked.how, "error": asked.error },
            "jev_usd": usd,
        });
        let evidence = (!likely.is_empty()).then(|| Evidence {
            label: "Likely defects: comments that justify a choice".to_string(),
            text: format!("{SUSPECTS_NOTE}\n\n{}", likely.join("\n")),
        });
        (evidence, record, usd)
    }

    /// The v13 comments and the admitted departure sources, ranked by Jev
    /// ([`crate::departures`]): the evidence for the listed rows, each
    /// with its kind, the record, and Jev's cost.
    async fn rank_departures(
        &self,
        prepared: &Prepared,
        extra: &[crate::departures::Source],
    ) -> (Option<Evidence>, Value, f64) {
        use crate::departures::{self, Source};
        let mut sources = vec![Source::Rationale];
        sources.extend(extra.iter().copied().filter(|s| *s != Source::Rationale));
        let ranked = departures::rank(
            &prepared.jev,
            &self.recorder,
            &departures::Context {
                component: "microluna.lean",
                id: format!("jev-departures-{}", self.dispatch()),
                deadline: prepared.deadline.clone(),
            },
            &prepared.instruction,
            &self.workdir,
            &sources,
        )
        .await;
        let listed = departures::listed(&ranked.rows);
        let record = json!({
            "kind": "lean.suspects",
            "sources": sources.iter().map(|s| s.word()).collect::<Vec<_>>(),
            "implementation": departures::implementation(&sources),
            "rows": ranked.rows,
            "likely": listed.len(),
            "jev": ranked.calls,
            "jev_usd": ranked.usd,
        });
        (departures::evidence(&listed), record, ranked.usd)
    }

    /// `evidence.baseline` ([`crate::baseline`], issue #9633): runs the
    /// task's entry points once in scratch copies, bounded by the loop's
    /// time left, and appends a host-executed command record per run to
    /// `executed-commands.jsonl` in the lean group's artifacts. Returns the
    /// briefing section, the loop's record, and the baseline commands.
    async fn run_baseline(
        &self,
        prepared: &Prepared,
        left: Duration,
    ) -> (Option<Evidence>, Value, Vec<String>) {
        let wall = crate::baseline::WALL.min(left);
        if wall < Duration::from_secs(5) {
            return (
                None,
                json!({"kind": "lean.baseline", "none": "too little time left to run it"}),
                Vec::new(),
            );
        }
        let setup = crate::baseline::Setup {
            root: self.workdir.clone(),
            alias: self.workdir.display().to_string(),
            wall,
            container: self.isolation == Isolation::TaskContainer,
        };
        let found = crate::baseline::run(&prepared.instruction, &setup).await;
        let file = self
            .artifacts
            .join(format!("lean-{}", self.dispatch()))
            .join(crate::baseline::EXECUTED_FILE);
        let mut record = found.summary();
        record["records"] = match found.append_records(&file) {
            Ok(()) if !found.runs.is_empty() => json!(file.display().to_string()),
            Ok(()) => Value::Null,
            Err(error) => json!({ "error": error }),
        };
        (found.evidence(), record, found.commands())
    }
}

/// A candidate identity: each file's SHA-256 by relative path.
type Identity = BTreeMap<String, String>;

/// A lane's outcome: its session, its copy, score, flag, and, when
/// candidates are retained, its snapshot.
struct LaneRun {
    ran: Ran,
    dir: PathBuf,
    score: Option<(u64, u64)>,
    tail: String,
    flagged: bool,
    /// The retained snapshot's path, its identity or why it couldn't be
    /// kept, and the copy's milliseconds.
    snapshot: Option<(PathBuf, Result<Identity, String>, u128)>,
}

/// What `retain_candidates` or `protect_candidates` keeps of each lane.
struct LaneKeep<'a> {
    /// The directory candidates are retained in.
    retained: &'a Path,
    /// Only a lane with a retained snapshot can be kept.
    protect: bool,
    /// The frozen evaluator's identity.
    evaluator: Option<&'a BTreeMap<String, String>>,
}

/// The lane the host put in the workspace.
struct KeptLane {
    lane: usize,
    session: u32,
    score: Option<(u64, u64)>,
    dir: PathBuf,
    /// Whether `dir` is the retained candidate, which the loop keeps.
    retained: bool,
}

/// Why the lean loop keeps the lane it keeps.
pub const LANE_SELECTION: &str = "the highest frozen score among lanes that aren't flagged as \
hard-coded; a tie goes to a done finish, then to the lower lane";

/// Why protected lanes keep the lane they keep.
pub const PROTECTED_LANE_SELECTION: &str = "the highest frozen score among lanes that aren't \
flagged as hard-coded and have a retained snapshot; a tie goes to a done finish, then to the \
lower lane";

impl Micro {
    /// Runs `lanes` first attempts at once, each in a copy of the
    /// workspace with a different approach, scores each copy with the
    /// frozen scorer rebased to it, and puts the best one in the workspace.
    /// With `keep`, each lane is also retained as a candidate with its own
    /// identity, score, and lane number, and scored with a fresh copy of
    /// the frozen scorer. Returns the sessions, the records (each retained
    /// lane's, then the lanes' summary), and the kept lane.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    async fn lean_lanes(
        &self,
        prepared: &Prepared,
        lean: &Lean,
        lanes: usize,
        general: &str,
        rules: &str,
        evidence: &[Evidence],
        samples: &[Evidence],
        baseline: &[String],
        frozen: &Path,
        fields: &BTreeMap<String, BTreeSet<String>>,
        start_tree: &BTreeMap<String, String>,
        wall_left: &dyn Fn() -> Option<Duration>,
        spent: f64,
        scope: &candidate::Scope,
        keep: Option<&LaneKeep<'_>>,
        score_total: &mut Option<u64>,
    ) -> (Vec<Ran>, Vec<Value>, Option<KeptLane>) {
        let real_before = parallel::tree(&self.workdir);
        let mut dirs = Vec::new();
        for _ in 0..lanes {
            let dir = scratch("lean-lane");
            let scorer = scratch("lean-lane-score");
            if scope.snapshot(&self.workdir, &dir).is_err()
                || rebase_scorer(frozen, &scorer, &self.workdir, &dir).is_err()
            {
                let _ = std::fs::remove_dir_all(&dir);
                let _ = std::fs::remove_dir_all(&scorer);
                continue;
            }
            dirs.push((dir, scorer));
        }
        let n = dirs.len();
        let real = self.workdir.display().to_string();
        let deadline = match (wall_left(), lean.lane_sec) {
            (Some(left), 0) => Some(left),
            (Some(left), sec) => Some(left.min(Duration::from_secs(sec))),
            (None, 0) => None,
            (None, sec) => Some(Duration::from_secs(sec)),
        };
        let share = (self.policy.spend_usd - spent).max(0.0) / n.max(1) as f64;
        let numbers: Vec<u32> = (0..n).map(|k| 2 + u32::try_from(k).unwrap_or(0)).collect();
        let runs =
            futures_util::future::join_all(dirs.iter().enumerate().map(|(k, (dir, scorer))| {
                let mut guidance = general.to_string();
                guidance.push_str("\n\n");
                guidance.push_str(&lane_note(k + 1, n, dir, &self.workdir, scorer));
                guidance.push_str(rules);
                let mut lane_evidence = evidence.to_vec();
                lane_evidence.extend(self.sources_within(dir, &[], lean.source_chars));
                lane_evidence.extend(samples.iter().cloned());
                let brief = Brief {
                    task: prepared.instruction.clone(),
                    guidance,
                    evidence: lane_evidence,
                    state: vec![format!("Attempt {} of {n}, session {}.", k + 1, numbers[k])],
                };
                let persist = lean.persist.as_ref().map(|gate| microluna::Persist {
                    max_returns: gate.max_returns,
                    not_done: gate.not_done,
                    score_command: Some(format!("sh {}/score.sh", scorer.display())),
                    reserve_turns: gate.reserve_turns,
                    reserve_sec: gate.reserve_sec,
                });
                let place = Place {
                    workdir: Some(dir.clone()),
                    group: Some(format!("attempt {} of {n}", k + 1)),
                    batch: "lanes".to_string(),
                    parallel_with: numbers
                        .iter()
                        .copied()
                        .filter(|m| *m != numbers[k])
                        .collect(),
                    persist,
                    finish_rule: finish_rule(
                        lean,
                        &baseline
                            .iter()
                            .map(|c| rebase_text(c, &real, &dir.display().to_string()))
                            .collect::<Vec<_>>(),
                    ),
                    deadline,
                    spend_usd: lean.session_spend.then_some(share),
                    command_max: (lean.command_sec > 0)
                        .then(|| Duration::from_secs(lean.command_sec)),
                    ..Place::default()
                };
                let number = numbers[k];
                async move {
                    self.session_at(
                        number,
                        &[format!("attempt {}", k + 1)],
                        "the lean loop runs several first attempts at once",
                        &brief,
                        false,
                        place,
                    )
                    .await
                }
            }))
            .await;
        let mut outcomes = Vec::new();
        for (ran, (dir, scorer)) in runs.into_iter().zip(dirs.iter()) {
            let remaining = wall_left().unwrap_or(Duration::MAX);
            let (mut score, mut tail) = if let Some(keep) = keep {
                // A lane session can edit its own scorer copy, so a retained
                // lane is scored by a fresh copy of the intact frozen one.
                let intact = keep
                    .evaluator
                    .is_some_and(|d| evidence_tree(frozen).as_ref() == Ok(d));
                let fresh = scratch("lean-lane-score");
                let scored = if intact && rebase_scorer(frozen, &fresh, &self.workdir, dir).is_ok()
                {
                    self.lean_score_in(&fresh, lean, dir, remaining).await
                } else {
                    (
                        None,
                        "The evaluator is missing or changed; its result is unknown.".to_string(),
                    )
                };
                let _ = std::fs::remove_dir_all(&fresh);
                scored
            } else {
                self.lean_score_in(scorer, lean, dir, remaining).await
            };
            if keep.is_some()
                && let Some((_, total)) = score
            {
                if score_total.is_some_and(|expected| expected != total) {
                    score = None;
                    tail.push_str("\nThe score total changed; candidates are not comparable.");
                } else {
                    *score_total = Some(total);
                }
            }
            let flagged = !literal_examples(dir, start_tree, fields).is_empty();
            let _ = std::fs::remove_dir_all(scorer);
            let snapshot = keep.map(|keep| {
                let candidate = keep.retained.join(format!("session-{}", ran.number));
                let started = Instant::now();
                let identity = scope
                    .bound(dir)
                    .and_then(|()| scope.identity(dir))
                    .and_then(|before| {
                        scope.snapshot(dir, &candidate)?;
                        if evidence_tree(&candidate)? != before {
                            return Err("candidate copy differs from the lane".to_string());
                        }
                        Ok(before)
                    });
                (candidate, identity, started.elapsed().as_millis())
            });
            outcomes.push(LaneRun {
                ran,
                dir: dir.clone(),
                score,
                tail,
                flagged,
                snapshot,
            });
        }
        let leaked = parallel::tree(&self.workdir) != real_before;
        let protect = keep.is_some_and(|k| k.protect);
        let excluded = |o: &LaneRun| -> Option<&'static str> {
            if o.flagged {
                Some("the workspace looks hard-coded")
            } else if protect && !matches!(o.snapshot, Some((_, Ok(_), _))) {
                Some("its snapshot could not be retained")
            } else {
                None
            }
        };
        // The best lane: eligible, then the score, then a done finish, then
        // the lower lane.
        let chosen = outcomes
            .iter()
            .enumerate()
            .filter(|(_, o)| excluded(o).is_none())
            .max_by(|(ia, a), (ib, b)| {
                fraction(a.score)
                    .total_cmp(&fraction(b.score))
                    .then((a.ran.status() == "done").cmp(&(b.ran.status() == "done")))
                    .then(ib.cmp(ia))
            })
            .map(|(k, _)| k);
        let mut kept = None;
        let mut place_error = None;
        if let Some(k) = chosen {
            let o = &outcomes[k];
            // A protected workspace becomes the retained candidate itself,
            // so the submitted identity can be checked against it.
            let from = match (&o.snapshot, protect) {
                (Some((candidate, _, _)), true) => candidate.clone(),
                _ => o.dir.clone(),
            };
            let placed = scope.restore(&self.workdir, &from).and_then(|()| {
                if protect {
                    return Ok(from.clone());
                }
                let snapshot = scratch("lean-best");
                scope.snapshot(&self.workdir, &snapshot).map(|()| snapshot)
            });
            match placed {
                Ok(dir) => {
                    kept = Some(KeptLane {
                        lane: k + 1,
                        session: o.ran.number,
                        score: o.score,
                        dir,
                        retained: protect,
                    });
                }
                Err(error) => place_error = Some(error),
            }
        }
        let matches = kept.as_ref().filter(|k| k.retained).map(|k| {
            let submitted = scope.identity(&self.workdir);
            submitted.is_ok() && submitted == evidence_tree(&k.dir)
        });
        let mut records = Vec::new();
        if let Some(keep) = keep {
            for (k, o) in outcomes.iter().enumerate() {
                let Some((candidate, identity, ms)) = &o.snapshot else {
                    continue;
                };
                records.push(json!({
                    "kind": "lean",
                    "after_session": o.ran.number,
                    "lane": k + 1,
                    "batch": "lanes",
                    "self_check": false,
                    "status": o.ran.status(),
                    "score": o.score.map(|(p, t)| json!({"passed": p, "total": t})),
                    "score_tail": o.tail,
                    "hardcoded": {"flagged": o.flagged},
                    "kept": kept.as_ref().is_some_and(|kl| kl.lane == k + 1),
                    "cost_usd": o.ran.cost_usd,
                    "candidate": candidate.display().to_string(),
                    "snapshot_error": identity.as_ref().err(),
                    "snapshot_ms": ms,
                    "workspace_files": identity.as_ref().ok(),
                    "evaluator_files": keep.evaluator,
                }));
            }
        }
        records.push(json!({
            "kind": "lean.lanes",
            "lanes": outcomes.iter().enumerate().map(|(k, o)| json!({
                "lane": k + 1,
                "session": o.ran.number,
                "status": o.ran.status(),
                "score": o.score.map(|(p, t)| json!({"passed": p, "total": t})),
                "flagged": o.flagged,
                "cost_usd": o.ran.cost_usd,
                "excluded": excluded(o),
            })).collect::<Vec<_>>(),
            "kept": kept.as_ref().map(|k| k.lane),
            "leaked": leaked,
            "selected": chosen.map(|k| json!({
                "lane": k + 1,
                "session": outcomes[k].ran.number,
                "score": outcomes[k].score.map(|(p, t)| json!({"passed": p, "total": t})),
                "status": outcomes[k].ran.status(),
                "reason": if protect { PROTECTED_LANE_SELECTION } else { LANE_SELECTION },
            })),
            "place_error": place_error,
            "selection_matches_workspace": matches,
            "retained": keep.is_some(),
        }));
        let failed_snapshots = outcomes
            .iter()
            .filter_map(|o| match &o.snapshot {
                Some((_, Err(error), _)) => Some(format!("attempt {}: {error}", o.ran.number - 1)),
                _ => None,
            })
            .collect::<Vec<_>>();
        crate::say::line(&format!(
            "  microluna ▸ lean: {n} attempts scored {}; kept {}{}{}",
            outcomes
                .iter()
                .map(|o| o
                    .score
                    .map_or("none".to_string(), |(p, t)| format!("{p}/{t}")))
                .collect::<Vec<_>>()
                .join(", "),
            kept.as_ref()
                .map_or("none".to_string(), |k| format!("attempt {}", k.lane)),
            place_error.as_ref().map_or(String::new(), |e| format!(
                "; the chosen attempt couldn't be put in the workspace: {e}"
            )),
            if failed_snapshots.is_empty() {
                String::new()
            } else {
                format!(
                    "; the host couldn't keep a snapshot of {}",
                    failed_snapshots.join("; ")
                )
            }
        ));
        let mut ran = Vec::new();
        for o in outcomes {
            let _ = std::fs::remove_dir_all(&o.dir);
            ran.push(o.ran);
        }
        (ran, records, kept)
    }
}

/// The review rule's decision on session `session`'s candidate: its score
/// and hard-coding answer from the loop's record, the grades and executed
/// commands from the first of `dirs` that has them, and the requirement
/// map.
fn review_decision(
    prepared: &Prepared,
    lean: &Lean,
    params: crate::review_rule::Params,
    session: u32,
    moves: &[Value],
    dirs: &[&Path],
) -> crate::review_rule::Decision {
    use crate::review_rule as rule;
    let record = moves
        .iter()
        .rev()
        .find(|m| m["kind"] == "lean" && m["after_session"].as_u64() == Some(u64::from(session)));
    let score = record.and_then(|m| {
        Some((
            m["score"]["passed"].as_u64()?,
            m["score"]["total"].as_u64()?,
        ))
    });
    let hardcoded = if lean.hardcode_check {
        record.and_then(|m| rule::hardcoded_of(&m["hardcoded"]))
    } else {
        None
    };
    rule::decide(
        &rule::Evidence {
            session,
            score,
            grades: rule::read_grades(dirs),
            executed: rule::read_executed(dirs),
            hardcoded,
            targets: rule::targets(&prepared.requirements),
        },
        params,
    )
}

/// The review rule's decision as a loop record.
fn review_record(decision: &crate::review_rule::Decision) -> Value {
    let mut record = serde_json::to_value(decision).unwrap_or(Value::Null);
    record["kind"] = json!("lean.review_rule");
    record["trigger"] = json!(decision.words());
    record
}

/// The best workspace so far.
struct Best {
    session: u32,
    score: Option<(u64, u64)>,
    dir: PathBuf,
    /// The tiered suite's holding tests red, and the share of its ranking
    /// tests green, when a tiered suite ran on this candidate.
    held: usize,
    rank: f64,
}

/// The tiered suite beside the lean loop, and how it runs.
struct Tier {
    suite: crate::accept::AcceptanceSuite,
    runner: crate::accept::Local,
    hold: Vec<Authority>,
    rank: Vec<Authority>,
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

    /// Writes, freezes, and classifies the tiered suite before the first
    /// session. Returns the suite, or `None` when it can't be written or
    /// has no tests, and the move that records it with its cost in `usd`.
    async fn tiered_suite(&self, prepared: &Prepared, tiered: &Tiered) -> (Option<Tier>, Value) {
        let Ok(wire) = &self.wire else {
            return (
                None,
                json!({"kind": "lean.tiered_suite", "error": "no transport", "usd": 0.0}),
            );
        };
        let base = self
            .artifacts
            .parent()
            .map_or_else(|| self.artifacts.clone(), Path::to_path_buf);
        let suite_dir = base.join(format!("accept-tiered-{}", self.dispatch()));
        let task = crate::accept::Task {
            title: prepared.title.clone(),
            instruction: prepared.instruction.clone(),
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
        let key = format!("microluna-{}", &sha256(&prepared.instruction)[..16]);
        let w = &tiered.writer;
        let writer = crate::accept::MicrolunaWriter {
            transport: wire,
            config: Config {
                model: self.model.clone(),
                effort: w.effort.clone().or_else(|| self.effort.clone()),
                max_turns: w.turns,
                cache_key: format!("{key}-tiered-writer"),
                deadline: Some(Duration::from_secs(self.policy.session_sec).min(self.deadline)),
                orient_effort: None,
                parallel_tools: self.policy.parallel_tools,
                persist: None,
                spend_usd: None,
                finish_rule: None,
            },
            isolation: self.isolation,
            seal: self.seal.clone(),
            traces: Some(self.artifacts.clone()),
            echo: false,
        };
        let confine = match self.isolation {
            Isolation::TaskContainer => crate::accept::Confine::TaskContainer,
            Isolation::ReadOnly => crate::accept::Confine::ReadOnly,
            Isolation::Boundary => crate::accept::Confine::Writing,
        };
        let runner = crate::accept::Local {
            confine,
            test_sec: 120,
            jobs: self.policy.test_jobs as usize,
        };
        let real = self.workdir.display().to_string();
        let inputs = crate::accept::Inputs {
            task: &task,
            requirements: &prepared.requirements,
            evidence: &evidence,
            workspace: &self.workdir,
            suite_dir: &suite_dir,
            workspace_note: format!(
                "the task's workspace, {real}, which your commands can read but you must not \
                 change"
            ),
            target: None,
        };
        let mut options = w.options();
        options.test_jobs = self.policy.test_jobs as usize;
        options.authority = true;
        crate::say::line("  microluna ▸ writing and classifying the tiered acceptance suite");
        let suite = crate::accept::define(
            &inputs,
            &writer,
            &runner,
            &prepared.jev,
            &self.recorder,
            &options,
        )
        .await;
        let mut classes: BTreeMap<&str, usize> = BTreeMap::new();
        for classified in suite.authority.values() {
            *classes.entry(classified.class.word()).or_default() += 1;
        }
        crate::say::line(&format!(
            "  microluna ▸ {}; classes {}",
            suite.headline(),
            classes
                .iter()
                .map(|(k, n)| format!("{k} {n}"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        let record = json!({
            "kind": "lean.tiered_suite",
            "headline": suite.headline(),
            "digest": suite.digest,
            "tests": suite.tests.len(),
            "classes": classes,
            "hold": tiered.hold,
            "rank": tiered.rank,
            "record": crate::accept::AcceptanceSuite::record_path(&suite_dir),
            "usd": suite.writer_usd + suite.jev_usd,
        });
        let tier = (!suite.tests.is_empty()).then(|| Tier {
            suite,
            runner,
            hold: tiered.hold.clone(),
            rank: tiered.rank.clone(),
        });
        (tier, record)
    }

    /// Runs the tiered suite on the workspace after session `number`: the
    /// powers its result carries, the record, and the lines the next
    /// session reads. A suite edited since its freeze holds nothing and
    /// says so.
    async fn tier_run(&self, tier: &Tier, number: u32) -> (Powers, Value, Vec<String>) {
        match crate::accept::run(
            &tier.suite,
            &self.workdir,
            &tier.runner,
            Some(&self.recorder),
            &format!("after session {number}"),
        )
        .await
        {
            Ok(result) => {
                let powers = Powers::of(&tier.suite, &result, &tier.hold, &tier.rank);
                let notes = authority::brief_lines(&tier.suite, &result, &powers);
                let record = json!({
                    "passed": result.passed,
                    "total": result.total,
                    "held": powers.held,
                    "hold_green": powers.hold_green,
                    "hold_total": powers.hold_total,
                    "rank_green": powers.rank_green,
                    "rank_total": powers.rank_total,
                    "suspects": powers.suspects,
                    "unpowered_red": powers.unpowered_red,
                });
                (powers, record, notes)
            }
            Err(tampered) => (
                Powers::default(),
                json!({"error": tampered.to_string(), "held": []}),
                Vec::new(),
            ),
        }
    }

    /// Runs the frozen score script against the workspace.
    pub(super) async fn lean_score(
        &self,
        frozen: &Path,
        lean: &Lean,
        remaining: Duration,
    ) -> (Option<(u64, u64)>, String) {
        self.lean_score_in(frozen, lean, &self.workdir, remaining)
            .await
    }

    /// Runs the score script in `frozen` with `dir` as its working directory.
    async fn lean_score_in(
        &self,
        frozen: &Path,
        lean: &Lean,
        dir: &Path,
        remaining: Duration,
    ) -> (Option<(u64, u64)>, String) {
        let script = frozen.join("score.sh");
        if !script.is_file() {
            return (None, "no score script".to_string());
        }
        let wall = Duration::from_secs(lean.score_sec).min(remaining);
        if wall.is_zero() {
            return (
                None,
                "No time remains to evaluate the workspace.".to_string(),
            );
        }
        let ended = match self.isolation {
            Isolation::TaskContainer => {
                let mut command = std::process::Command::new("/bin/sh");
                command.arg(&script).current_dir(dir);
                microluna::tools::withhold_credentials(&mut command);
                supervise::Job::from_command(command)
                    .bounded(supervise::Limits::within(wall).keeping(64 * 1024))
                    .run()
                    .await
            }
            _ => {
                let spec = if self.isolation == Isolation::ReadOnly {
                    coder_boundary::Boundary::readonly()
                } else {
                    coder_boundary::Boundary::writing(dir)
                };
                let boundary = match spec.owned_scratch_under(std::env::temp_dir()).build() {
                    Ok(boundary) => boundary,
                    Err(error) => return (None, format!("no enforced boundary: {error}")),
                };
                let mut command = match boundary.command("/bin/sh", [script.as_os_str()]) {
                    Ok(command) => command,
                    Err(error) => return (None, error.to_string()),
                };
                command.current_dir(dir);
                microluna::tools::withhold_credentials(&mut command);
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
        let score = (ended.ending.success() && !ended.stdout.truncated)
            .then(|| parse_score(&stdout))
            .flatten();
        (
            score,
            format!("exit {:?}; {:?}\n{tail}", ended.ending.code(), ended.ending),
        )
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
            (lean.failures, FAILURES),
            (lean.structure, STRUCTURE),
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
        let mut spent_before = 0.0;
        let mut samples = data_samples(&self.workdir, lean.sample_chars);
        if lean.defended {
            let defended = crate::accept::defended_choices_general(&self.workdir);
            if !defended.is_empty() {
                samples.insert(0, crate::accept::defended_evidence(&defended, true));
            }
        }
        let mut suspects_record = Value::Null;
        let mut suspects_listed = false;
        if lean.rationale {
            let (evidence, record, usd) = if lean.departures.is_empty() {
                self.rank_suspects(prepared).await
            } else {
                self.rank_departures(prepared, &lean.departures).await
            };
            if let Some(evidence) = evidence {
                samples.insert(0, evidence);
                suspects_listed = true;
            }
            suspects_record = record;
            spent_before += usd;
        }
        let mut baseline: Vec<String> = Vec::new();
        let mut baseline_record = Value::Null;
        if lean.baseline {
            let (evidence, record, commands) = self.run_baseline(prepared, time_left()).await;
            if let Some(evidence) = evidence {
                // After the suspects, when there are any.
                samples.insert(usize::from(suspects_listed), evidence);
            }
            baseline_record = record;
            baseline = commands;
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
        // What a candidate is: the whole workspace, or in a Git work tree
        // the files Git doesn't ignore.
        let scope = candidate::Scope::of(&self.workdir);
        // A copy of the untouched workspace, for a real diff.
        let base = parallel::copyable(&self.workdir).then(|| scratch("lean-base"));
        let base = base.filter(|dir| crate::handoff::copy_tree(&self.workdir, dir).is_ok());
        let _base_cleanup = Cleanup(base.clone());
        let eval = eval_dir(&self.workdir, self.isolation);
        let retained = self.artifacts.join(format!("lean-{}", self.dispatch()));
        let keep_evidence = lean.protect_candidates || lean.retain_candidates;
        let frozen = if keep_evidence {
            retained.join("evaluator")
        } else {
            scratch("microluna-eval-frozen")
        };
        let _frozen_cleanup = Cleanup((!keep_evidence).then(|| frozen.clone()));
        let _eval_cleanup = Cleanup(
            (self.isolation != Isolation::TaskContainer || lean.keep_best).then(|| eval.clone()),
        );
        if lean.keep_best {
            let _ = std::fs::create_dir_all(&eval);
        }
        let mut have_score = false;
        let mut evaluator_digest = None;
        let mut score_total = None;
        let mut best: Option<Best> = None;
        let mut best_cleanup: Vec<PathBuf> = Vec::new();
        let mut sessions: Vec<Ran> = Vec::new();
        let mut moves: Vec<Value> = Vec::new();
        let mut spent = spent_before;
        if !suspects_record.is_null() {
            moves.push(suspects_record);
        }
        if !baseline_record.is_null() {
            moves.push(baseline_record);
        }
        // `verify.executed` (issue #9636): the commands to rerun after every
        // session, each with its outcome on the untouched workspace. The
        // records go beside `evidence.baseline`'s, whose finished commands
        // (`baseline`) come first.
        let executed_file = self
            .artifacts
            .join(format!("lean-{}", self.dispatch()))
            .join(crate::baseline::EXECUTED_FILE);
        let executed_place = |rule: &LeanExecuted, left: Duration| executed::Place {
            workdir: self.workdir.clone(),
            contained: self.isolation == Isolation::TaskContainer,
            wall: Duration::from_secs(rule.command_sec),
            budget: Duration::from_secs(rule.budget_sec).min(left),
        };
        let mut executed_plan: Option<executed::Baseline> = None;
        if let Some(rule) = lean.executed.as_ref().filter(|_| lean.keep_best) {
            match &base {
                Some(untouched) => {
                    let known = executed::read(&executed_file);
                    let from_records = known
                        .iter()
                        .filter(|r| r.stage == executed::Stage::Baseline)
                        .count();
                    let place = executed_place(rule, time_left());
                    let (plan, records) = executed::prepare(
                        &prepared.instruction,
                        untouched,
                        &place,
                        &baseline,
                        &known,
                    )
                    .await;
                    let error = executed::append(&executed_file, &records).err();
                    moves.push(json!({
                        "kind": "lean.executed_baseline",
                        "commands": plan.commands,
                        "from_records": from_records,
                        "ran": records.len(),
                        "error": error,
                    }));
                    executed_plan = Some(plan);
                }
                None => moves.push(json!({
                    "kind": "lean.executed_refused",
                    "error": "the workspace couldn't be copied, so no command runs after a session",
                })),
            }
        }
        let mut reject_note: Option<String> = None;
        let mut history: Vec<String> = Vec::new();
        let mut flag_note: Option<String> = None;
        let mut last_tail: Option<String> = None;
        // The stall detector's notes for the next session, and whether it
        // re-briefed after the last one (issue #9627).
        let mut detect_notes: Vec<String> = Vec::new();
        let mut rebriefed = false;
        let mut stopped = String::new();
        let mut halted = false;
        // The review rule's decision, once the loop reaches the self-check.
        let mut review: Option<crate::review_rule::Decision> = None;
        let lane_bound = (lean.keep_best && lean.lanes > 1).then(|| scope.bound(&self.workdir));
        let lanes = if matches!(lane_bound, Some(Ok(()))) {
            lean.lanes as usize
        } else {
            1
        };
        if let Some(Err(error)) = &lane_bound {
            crate::say::line(&format!(
                "  microluna ▸ lean: the {} attempts can't have copies of the workspace, so one \
                 first attempt runs: {error}",
                lean.lanes
            ));
            moves.push(json!({"kind": "lean.lanes_refused", "lanes": lean.lanes, "error": error}));
        }
        let tier = match &lean.tiered {
            Some(tiered) => {
                let (tier, record) = self.tiered_suite(prepared, tiered).await;
                spent += record["usd"].as_f64().unwrap_or(0.0);
                moves.push(record);
                tier
            }
            None => None,
        };
        let mut tier_notes: Vec<String> = Vec::new();
        let mut offset = 0u32;
        if lanes > 1 {
            // The scorer session: the evaluation script only.
            let mut guidance = scorer_guidance(&eval);
            guidance.push_str(&rules);
            let mut scorer_evidence = evidence.clone();
            scorer_evidence.extend(self.sources_within(&self.workdir, &named, lean.source_chars));
            scorer_evidence.extend(samples.iter().cloned());
            let ran = self
                .session_at(
                    1,
                    &["the evaluation script".to_string()],
                    "the lean loop's scorer writes the evaluation script before any attempt",
                    &Brief {
                        task: prepared.instruction.clone(),
                        guidance,
                        evidence: scorer_evidence,
                        state: vec!["Session 1: the scorer.".to_string()],
                    },
                    false,
                    Place {
                        group: Some("the evaluation script".to_string()),
                        deadline: wall_left(),
                        command_max: (lean.command_sec > 0)
                            .then(|| Duration::from_secs(lean.command_sec)),
                        ..Place::default()
                    },
                )
                .await;
            spent += ran.cost_usd.unwrap_or(0.0);
            sessions.push(ran);
            offset = 1;
            if eval.join("score.sh").is_file() {
                have_score = crate::handoff::copy_tree(&eval, &frozen).is_ok();
                if have_score {
                    evaluator_digest = evidence_tree(&frozen).ok();
                }
            }
            let (untouched, _) = if have_score {
                self.lean_score(
                    &frozen,
                    lean,
                    time_left().min(wall_left().unwrap_or(Duration::MAX)),
                )
                .await
            } else {
                (None, String::new())
            };
            moves.push(json!({
                "kind": "lean.scorer",
                "frozen": have_score,
                "untouched": untouched.map(|(p, t)| json!({"passed": p, "total": t})),
            }));
            if have_score {
                let (ran_lanes, record, kept) = self
                    .lean_lanes(
                        prepared,
                        lean,
                        lanes,
                        &general,
                        &rules,
                        &evidence,
                        &samples,
                        &baseline,
                        &frozen,
                        &fields,
                        &start_tree,
                        &|| Some(time_left().min(wall_left().unwrap_or(Duration::MAX))),
                        spent,
                        &scope,
                        keep_evidence
                            .then_some(LaneKeep {
                                retained: &retained,
                                protect: lean.protect_candidates,
                                evaluator: evaluator_digest.as_ref(),
                            })
                            .as_ref(),
                        &mut score_total,
                    )
                    .await;
                for ran in &ran_lanes {
                    spent += ran.cost_usd.unwrap_or(0.0);
                }
                offset += u32::try_from(ran_lanes.len()).unwrap_or(0);
                sessions.extend(ran_lanes);
                if let Some(kept) = kept {
                    history.push(format!(
                        "{lanes} attempts ran at once; the host kept attempt {}'s workspace, \
                         which scored {}.",
                        kept.lane,
                        kept.score
                            .map_or("nothing".to_string(), |(p, t)| format!("{p} of {t}"))
                    ));
                    if !kept.retained {
                        best_cleanup.push(kept.dir.clone());
                    }
                    best = Some(Best {
                        // Recording modes name the kept lane's own session;
                        // v14 keeps naming the last lane's.
                        session: if keep_evidence { kept.session } else { offset },
                        score: kept.score,
                        dir: kept.dir,
                        held: 0,
                        rank: 0.0,
                    });
                }
                moves.extend(record);
                if keep_evidence {
                    let evidence = serde_json::to_vec_pretty(&moves).unwrap_or_default();
                    if let Err(error) =
                        crate::record::write_atomic(&retained.join("selection.json"), &evidence)
                    {
                        moves.push(json!({"kind": "lean.evidence_error", "error": error}));
                        if lean.protect_candidates {
                            stopped = format!("could not retain candidate evidence: {error}");
                            halted = true;
                        }
                    }
                }
            }
        }
        let total = offset + lean.sessions + u32::from(lean.self_check);
        let mut number = offset;
        let mut checking = false;
        loop {
            if halted {
                break;
            }
            number += 1;
            if !checking && number > offset + lean.sessions {
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
            // The review rule (issue #9637): decide once, before the
            // self-check, whether anything disagrees.
            if checking
                && review.is_none()
                && let Some(params) = lean.review_rule
            {
                // The candidate the review reads: the selected one, which
                // protected candidates restore before the review, or else
                // the workspace as the last work session left it.
                let session = best
                    .as_ref()
                    .filter(|_| lean.protect_candidates)
                    .map_or(number - 1, |b| b.session);
                let decision = review_decision(
                    prepared,
                    lean,
                    params,
                    session,
                    &moves,
                    &[&retained, &self.artifacts],
                );
                crate::say::line(&format!("  microluna ▸ review rule: {}", decision.reason));
                moves.push(review_record(&decision));
                let skip = !decision.review;
                if skip {
                    stopped.push_str(&format!(
                        "{}{}",
                        if stopped.is_empty() { "" } else { "; " },
                        decision.reason
                    ));
                }
                review = Some(decision);
                if skip {
                    break;
                }
            }
            if checking
                && lean.protect_candidates
                && let Some(candidate) = &best
            {
                if let Err(error) = scope.restore(&self.workdir, &candidate.dir) {
                    stopped =
                        format!("could not prepare the selected candidate for review: {error}");
                    break;
                }
                history.push(format!("The host selected session {} for review; equal scores preserve the earlier candidate.", candidate.session));
            }
            let mut guidance = if checking && lean.observe_review {
                OBSERVE_GUIDANCE.to_string()
            } else if checking {
                CHECK_GUIDANCE.to_string()
            } else if sessions.is_empty() {
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
            if checking && let Some(decision) = &review {
                guidance.push_str("\n\n");
                guidance.push_str(&crate::review_rule::guidance(decision));
            }
            guidance.push_str(&rules);
            let mut session_evidence = evidence.clone();
            session_evidence.extend(self.sources_within(&self.workdir, &named, lean.source_chars));
            session_evidence.extend(samples.iter().cloned());
            let mut state = vec![format!(
                "Session {number} of at most {total}{}.",
                if checking { ", the self-check" } else { "" }
            )];
            if !sessions.is_empty() {
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
                if let Some(note) = &reject_note {
                    state.push(note.clone());
                }
                if let Some(tail) = &last_tail {
                    state.push(tail.clone());
                }
                if !checking {
                    state.extend(detect_notes.iter().cloned());
                }
                state.extend(tier_notes.iter().cloned());
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
            if checking && review.is_some() {
                state.push(format!(
                    "The task's requirements, by ID:\n{}",
                    crate::review_rule::requirement_lines(&prepared.requirements).join("\n")
                ));
            }
            let brief = Brief {
                task: prepared.instruction.clone(),
                guidance,
                evidence: session_evidence,
                state,
            };
            let why = if checking {
                "the self-check reviews the result on a fresh context".to_string()
            } else if sessions.is_empty() {
                "the lean loop's first session takes the whole task".to_string()
            } else {
                "the lean loop continues the task".to_string()
            };
            // The in-session stall check (issue #9627), on work sessions
            // only; inert unless `detect.in_session` is on.
            let mut watch = self.in_session(
                prepared,
                lean.detect.as_ref().filter(|_| !checking),
                &sessions,
                number,
            );
            let ran = self
                .session_watched(
                    number,
                    &[if checking {
                        "the self-check".to_string()
                    } else {
                        "the whole task".to_string()
                    }],
                    &why,
                    &brief,
                    checking && lean.observe_review,
                    Place {
                        observe_only: checking && lean.observe_review,
                        persist: persist(lean, checking, have_score, &eval, &frozen),
                        finish_rule: finish_rule(lean, &baseline),
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
                    &mut watch,
                )
                .await;
            spent += ran.cost_usd.unwrap_or(0.0) + watch.usd;
            let in_session = watch.record();
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
                if have_score {
                    evaluator_digest = evidence_tree(&frozen).ok();
                }
            }
            let intact = evaluator_digest
                .as_ref()
                .is_some_and(|d| evidence_tree(&frozen).as_ref() == Ok(d));
            let (mut score, mut score_tail) = if have_score && intact {
                self.lean_score(
                    &frozen,
                    lean,
                    time_left().min(wall_left().unwrap_or(Duration::MAX)),
                )
                .await
            } else {
                (
                    None,
                    "The evaluator is missing or changed; its result is unknown.".to_string(),
                )
            };
            if evaluator_digest
                .as_ref()
                .is_some_and(|d| evidence_tree(&frozen).as_ref() != Ok(d))
            {
                score = None;
                score_tail =
                    "The evaluator changed after freezing; its result is unknown.".to_string();
            }
            if let Some((_, total)) = score {
                if score_total.is_some_and(|expected| expected != total) {
                    score = None;
                    score_tail
                        .push_str("\nThe score total changed; candidates are not comparable.");
                } else {
                    score_total = Some(total);
                }
            }
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
            // `verify.executed`: rerun the commands on a scratch copy of the
            // candidate. Any that exited 0 on the untouched workspace and
            // fails now rejects it, whatever it scores.
            let (rejected, executed_record, executed_records) =
                match (lean.executed.as_ref(), executed_plan.as_ref()) {
                    (Some(rule), Some(plan)) => {
                        let place = executed_place(
                            rule,
                            time_left().min(wall_left().unwrap_or(Duration::MAX)),
                        );
                        let candidate_dir = keep_evidence
                            .then(|| format!("lean-{}/session-{number}", self.dispatch()));
                        match executed::after_session(plan, &place, number, candidate_dir).await {
                            Ok(records) => {
                                let rejected = executed::rejects(&records);
                                let error = executed::append(&executed_file, &records).err();
                                let count = |v: executed::Verdict| {
                                    records.iter().filter(|r| r.verdict == Some(v)).count()
                                };
                                let record = json!({
                                    "rejected": rejected,
                                    "regressed": records
                                        .iter()
                                        .filter(|r| r.verdict == Some(executed::Verdict::Regressed))
                                        .map(|r| json!({"command": r.command, "rule": r.rule}))
                                        .collect::<Vec<_>>(),
                                    "ok": count(executed::Verdict::Ok),
                                    "not_a_regression": count(executed::Verdict::NotARegression),
                                    "unknown": count(executed::Verdict::Unknown),
                                    "error": error,
                                });
                                (rejected, record, records)
                            }
                            Err(error) => (
                                false,
                                json!({"rejected": false, "error": error}),
                                Vec::new(),
                            ),
                        }
                    }
                    _ => (false, Value::Null, Vec::new()),
                };
            let powers = match &tier {
                Some(tier) => {
                    let (powers, record, notes) = self.tier_run(tier, number).await;
                    tier_notes = notes;
                    Some((powers, record))
                }
                None => None,
            };
            let held = powers.as_ref().map_or(0, |(p, _)| p.held.len());
            let rank = powers.as_ref().map_or(0.0, |(p, _)| p.rank_fraction());
            if lean.failures && have_score {
                last_tail = Some(format!(
                    "The host's evaluation output after session {number}, its tail:\n{}",
                    crate::judge::clip(&score_tail, 1_500)
                ));
            }
            if let Some((p, t)) = score {
                history.push(format!(
                    "After session {number} the host's score was {p} of {t}{}.",
                    if flagged {
                        ", but the workspace was flagged"
                    } else if rejected {
                        ", but the host rejected the workspace: a command regressed"
                    } else {
                        ""
                    }
                ));
            } else if lean.keep_best {
                history.push(format!(
                    "After session {number} evaluation was missing or invalid; completion is unknown:\n{}",
                    crate::judge::clip(&score_tail, 600)
                ));
            }
            let candidate = retained.join(format!("session-{number}"));
            let snapshot_started = Instant::now();
            let bound = if keep_evidence || lean.keep_best {
                scope.bound(&self.workdir)
            } else {
                Ok(())
            };
            let snapshot = if keep_evidence {
                bound.clone().and_then(|()| {
                    let before = scope.identity(&self.workdir)?;
                    scope.snapshot(&self.workdir, &candidate)?;
                    if evidence_tree(&candidate)? != before {
                        return Err("candidate copy differs from the workspace".to_string());
                    }
                    Ok(())
                })
            } else {
                Ok(())
            };
            let snapshot_ms = snapshot_started.elapsed().as_millis();
            // Protected selection keeps the earliest tied candidate. The
            // scalar score cannot establish that a later edit is better.
            let mut kept = false;
            let mut keep_error = None;
            if lean.keep_best
                && !flagged
                && !rejected
                && (!lean.protect_candidates || snapshot.is_ok())
            {
                let better = best.as_ref().is_none_or(|b| {
                    if tier.is_some() {
                        authority::ranks_at_least(
                            (held, fraction(score), rank),
                            (b.held, fraction(b.score), b.rank),
                            lean.protect_candidates,
                        )
                    } else if lean.protect_candidates {
                        fraction(score) > fraction(b.score)
                    } else {
                        fraction(score) >= fraction(b.score)
                    }
                });
                if better {
                    let dir = if lean.protect_candidates {
                        candidate.clone()
                    } else {
                        scratch("lean-best")
                    };
                    let copied = if lean.protect_candidates {
                        Ok(())
                    } else {
                        bound
                            .clone()
                            .and_then(|()| scope.snapshot(&self.workdir, &dir))
                    };
                    match copied {
                        Ok(()) => {
                            if !lean.protect_candidates {
                                best_cleanup.push(dir.clone());
                            }
                            best = Some(Best {
                                session: number,
                                score,
                                dir,
                                held,
                                rank,
                            });
                            kept = true;
                        }
                        Err(error) => {
                            let _ = std::fs::remove_dir_all(&dir);
                            keep_error = Some(error);
                        }
                    }
                }
            }
            let not_kept = match (&snapshot, &keep_error) {
                (Err(error), _) => format!("; the host couldn't keep a snapshot: {error}"),
                (Ok(()), Some(error)) => {
                    format!("; the best so far, but the host couldn't keep a snapshot: {error}")
                }
                _ => String::new(),
            };
            crate::say::line(&format!(
                "  microluna ▸ session {number} {status}; {}; {}{}{}",
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
                } else if rejected {
                    "; rejected: a command that ran on the untouched workspace fails now"
                } else {
                    ""
                },
                not_kept
            ));
            reject_note = rejected.then(|| {
                executed::note(number, &executed_records, best.as_ref().map(|b| b.session))
            });
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
                "candidate": keep_evidence.then(|| candidate.display().to_string()),
                "snapshot_error": snapshot.err(),
                "keep_error": keep_error,
                "snapshot_ms": keep_evidence.then_some(snapshot_ms),
                "workspace_files": keep_evidence.then(|| scope.identity(&self.workdir).ok()),
                "evaluator_files": evaluator_digest,
            }));
            if let (Some((_, record)), Some(last)) = (&powers, moves.last_mut()) {
                last["tiered"] = record.clone();
            }
            if let (Some(record), Some(last)) = (in_session, moves.last_mut()) {
                last["in_session"] = record;
            }
            if let (false, Some(last)) = (executed_record.is_null(), moves.last_mut()) {
                last["executed"] = executed_record;
            }
            if keep_evidence {
                let evidence = serde_json::to_vec_pretty(&moves).unwrap_or_default();
                if let Err(error) =
                    crate::record::write_atomic(&retained.join("selection.json"), &evidence)
                {
                    moves.push(json!({"kind": "lean.evidence_error", "error": error}));
                    if lean.protect_candidates {
                        stopped = format!("could not retain candidate evidence: {error}");
                        break;
                    }
                }
            }
            if checking {
                stopped.push_str(&format!(
                    "{}the self-check ended {status}",
                    if stopped.is_empty() { "" } else { "; " }
                ));
                if review.is_some()
                    && let Some(ran) = sessions.last()
                {
                    moves.push(crate::review_rule::concerns_record(
                        number,
                        &ran.summary(),
                        &crate::review_rule::targets(&prepared.requirements),
                    ));
                }
                break;
            }
            let full = score.is_some_and(|(p, t)| p >= t);
            // A holding test's red keeps the loop going; its green is
            // necessary, never sufficient.
            let settled =
                status == "done" && !flagged && !rejected && (!lean.keep_best || full) && held == 0;
            if settled {
                stopped = format!("session {number} ended done");
                if !lean.self_check {
                    break;
                }
                checking = true;
            }
            if let Some(detect) = lean.detect.as_ref()
                && !checking
                && number < offset + lean.sessions
            {
                let detected = self
                    .detect_after(prepared, detect, &sessions, number, rebriefed)
                    .await;
                spent += detected.usd;
                if let Some(last) = moves.last_mut() {
                    last["detect"] = detected.record;
                }
                rebriefed = detected.action == crate::stall::Action::Rebrief;
                detect_notes = detected.rebrief.into_iter().chain(detected.next).collect();
                if detected.action == crate::stall::Action::Stop {
                    stopped =
                        format!("the stall check stopped the work sessions after session {number}");
                    if !lean.self_check {
                        break;
                    }
                    checking = true;
                }
            }
        }
        // Finish on the best workspace when the last one scores lower or is
        // flagged.
        if let Some(b) = &best {
            let last = moves
                .iter()
                .rev()
                .find(|m| m["kind"] != "lean.review_rule" && m["kind"] != "lean.review_concerns");
            let last_flagged = last.is_some_and(|m| {
                m["hardcoded"]["flagged"] == true || m["executed"]["rejected"] == true
            });
            let last_score = last.and_then(|m| {
                Some((
                    m["score"]["passed"].as_u64()?,
                    m["score"]["total"].as_u64()?,
                ))
            });
            // A tiered suite's ranking tests never restore an earlier
            // workspace: only the holding tests and the score may.
            let last_held = last.and_then(|m| m["tiered"]["held"].as_array().map(Vec::len));
            let beaten = match (&tier, last_held) {
                (Some(_), Some(last_held)) => {
                    authority::beats(b.held, fraction(b.score), last_held, fraction(last_score))
                }
                _ => fraction(b.score) > fraction(last_score),
            };
            if Some(b.session) != sessions.last().map(|r| r.number)
                && (lean.protect_candidates || last_flagged || beaten)
            {
                match scope.restore(&self.workdir, &b.dir) {
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
        if lean.retain_candidates && !lean.protect_candidates {
            let submitted = scope.identity(&self.workdir);
            let selected = moves.iter().rev().find(|item| {
                let Some(path) = item["candidate"].as_str() else {
                    return false;
                };
                item["snapshot_error"].is_null()
                    && submitted.is_ok()
                    && submitted == evidence_tree(Path::new(path))
            });
            let selected_session = selected.map(|item| item["after_session"].clone());
            let score = selected.map(|item| item["score"].clone());
            let selected_lane = selected.map(|item| item["lane"].clone());
            moves.push(json!({
                "kind": "lean.submitted",
                "selected_session": selected_session,
                "selected_lane": selected_lane,
                "candidate_scope": scope.name(),
                "selection_matches_workspace": selected_session.is_some(),
                "result": "observed_without_revalidation",
                "score": score,
                "evaluation_rerun": false,
                "review_status": moves.iter().rev().find(|item| item["self_check"] == true).map(|item| item["status"].clone()),
                "workspace_files": submitted.as_ref().ok(),
                "workspace_error": submitted.as_ref().err(),
                "identity_scope": scope.describe(),
                "benchmark_outcome": Value::Null,
            }));
            if let Ok(bytes) = serde_json::to_vec_pretty(&moves)
                && let Err(error) =
                    crate::record::write_atomic(&retained.join("selection.json"), &bytes)
            {
                moves.push(json!({"kind": "lean.evidence_error", "error": error}));
            }
        }
        if lean.protect_candidates {
            let intact = evaluator_digest
                .as_ref()
                .is_some_and(|d| evidence_tree(&frozen).as_ref() == Ok(d));
            let (mut score, output) = if have_score && intact {
                self.lean_score(
                    &frozen,
                    lean,
                    time_left().min(wall_left().unwrap_or(Duration::MAX)),
                )
                .await
            } else {
                (
                    None,
                    "The frozen evaluator is missing or changed.".to_string(),
                )
            };
            if evaluator_digest
                .as_ref()
                .is_some_and(|d| evidence_tree(&frozen).as_ref() != Ok(d))
            {
                score = None;
            }
            if score.is_some_and(|(_, total)| score_total != Some(total)) {
                score = None;
            }
            let submitted_files = scope.identity(&self.workdir);
            let selection_available = best.as_ref().is_some_and(|b| {
                submitted_files.is_ok() && submitted_files == evidence_tree(&b.dir)
            });
            let selected_session = best
                .as_ref()
                .filter(|_| selection_available)
                .map(|b| b.session);
            let result = if !selection_available || score.is_none() {
                "unknown"
            } else if score.is_some_and(|(p, t)| p == t) {
                "local_checks_passed"
            } else {
                "local_checks_failed"
            };
            stopped.push_str(&format!(
                "; submitted evidence: {result} (benchmark outcome not known)"
            ));
            moves.push(json!({
                "kind": "lean.submitted",
                "selected_session": selected_session,
                "selected_lane": selected_session.and_then(|n| moves.iter().find(|m| {
                    m["kind"] == "lean" && m["after_session"].as_u64() == Some(u64::from(n))
                }).map(|m| m["lane"].clone())),
                "candidate_scope": scope.name(),
                "selection_matches_workspace": selection_available,
                "result": result,
                "score": score.map(|(p, t)| json!({"passed": p, "total": t})),
                "output": output,
                "review_status": sessions.last().filter(|r| r.read_only).map(Ran::status),
                "workspace_files": submitted_files.as_ref().ok(),
                "workspace_error": submitted_files.as_ref().err(),
                "identity_scope": scope.describe(),
                "benchmark_outcome": Value::Null,
            }));
            if let Ok(bytes) = serde_json::to_vec_pretty(&moves)
                && let Err(error) =
                    crate::record::write_atomic(&retained.join("selection.json"), &bytes)
            {
                stopped.push_str(&format!("; could not retain submitted evidence: {error}"));
            }
        }
        (sessions, moves, stopped)
    }
}
