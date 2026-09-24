//! `control.best_of`: several candidates of the first executor at once,
//! each in its own copy of the workspace, and one kept.
//!
//! A GPT-6 Luna attempt costs cents, so the composition can run N of them
//! in parallel and keep the best. Each candidate works in a private copy
//! of the task's workspace, with every mention of the workspace's path in
//! its briefing rewritten to its copy's. When every candidate has ended,
//! the host puts each copy into the real workspace in turn, runs
//! `verify.checks` on it there, and asks the calibrated verdict
//! (`checks::verdict`) about the candidate's final report. The kept
//! candidate is the one with the best verdict (pass, then unknown, then
//! fail; a session that ended without an answer ranks below every
//! answered one), then the best check scorecard (fewer failed scenarios,
//! then more confirmed requirements), then the lowest cost, then the
//! lowest number. Its copy becomes the workspace, and the rest of the
//! composition (repair, `verify.second`, `control.persist`) continues
//! from it.
//!
//! Every candidate is a `delegate` call in the trajectory and a
//! `candidate-N` dispatch in `artifacts/composition.json`, so each one is
//! charged. The record's `best_of` holds each candidate's status, cost,
//! time, checks, verdict, and score, which one was kept, and why. With
//! `archive` on, each candidate's workspace is also saved as
//! `best-of/candidate-N.tar.gz` in the episode, so the task's verifier can
//! grade every candidate afterwards (`tbench verify --candidate`), which
//! separates what the selection loses from what the candidates lack.
//!
//! Isolation is a copy, not a boundary. A candidate that writes to the
//! real workspace path anyway changes what the others read; the record's
//! `leaked` says whether the real workspace changed while the candidates
//! ran, and the kept candidate's copy replaces it either way. A task whose
//! state lives outside the workspace, such as a service container, is not
//! isolated at all.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use super::{Exec, Factory, Setup, Standing, VerifyPolicy, claimed, replace_contents, row};
use crate::checks::{self, Subject, generic};
use crate::delegate::{
    self, Briefing, Delegation, Executor, Mode, Prepared, Reason, Report, Status,
};
use crate::handoff::Tier;
use crate::record::Recorder;

/// The component's ID in decisions and the record.
pub const COMPONENT: &str = "control.best_of";

/// Where each candidate's archived workspace goes, under the episode.
pub const DIR: &str = "best-of";

/// The most candidates a policy may ask for.
pub const MAX_N: usize = 8;

/// The order candidates are ranked in, as the record states it.
pub const ORDER: [&str; 4] = ["verdict", "scorecard", "cost", "number"];

fn max_copy_mb() -> u64 {
    256
}
fn yes() -> bool {
    true
}

/// `control.best_of`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BestOfPolicy {
    /// How many candidates run at once, from 2 to [`MAX_N`].
    pub n: usize,
    /// The largest workspace the host copies, in MiB. A larger one runs
    /// one candidate, as if `best_of` were absent, and the record says so.
    #[serde(default = "max_copy_mb")]
    pub max_copy_mb: u64,
    /// Save each candidate's workspace under `best-of/` in the episode, so
    /// the verifier can grade every candidate afterwards.
    #[serde(default = "yes")]
    pub archive: bool,
}

impl BestOfPolicy {
    /// Refuses a policy this build can't run.
    #[must_use]
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.n < 2 || self.n > MAX_N {
            problems.push(format!(
                "control.best_of.n must be from 2 to {MAX_N}, not {}",
                self.n
            ));
        }
        if self.max_copy_mb == 0 {
            problems.push("control.best_of.max_copy_mb must be at least 1".to_string());
        }
        problems
    }
}

/// `text` with every mention of the path `from` as a whole path, or as the
/// start of one, replaced by `to`. `/app` and `/app/x` change; `/apple`,
/// `/opt/app`, and `app` don't.
#[must_use]
pub fn rebase(text: &str, from: &Path, to: &Path) -> String {
    let from = from.to_string_lossy();
    let from = from.trim_end_matches('/');
    let to = to.to_string_lossy();
    let to = to.trim_end_matches('/');
    if from.is_empty() || from == to {
        return text.to_string();
    }
    let path_char = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/');
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    let mut previous: Option<char> = None;
    while let Some(at) = rest.find(from) {
        let before = rest[..at].chars().next_back().or(previous);
        let mut after = rest[at + from.len()..].chars();
        let starts = before.is_none_or(|c| !path_char(c));
        // A period ends the path when it ends a sentence, not a name.
        let ends = match after.next() {
            None | Some('/') => true,
            Some('.') => after.next().is_none_or(|c| !path_char(c)),
            Some(c) => !path_char(c),
        };
        out.push_str(&rest[..at]);
        out.push_str(if starts && ends { to } else { from });
        previous = from.chars().next_back();
        rest = &rest[at + from.len()..];
    }
    out.push_str(rest);
    out
}

/// The briefing one candidate reads: the workspace's path rewritten to
/// its copy's, and a closing section that says where it works.
#[must_use]
pub fn candidate_briefing(briefing: &Briefing, workdir: &Path, copy: &Path) -> Briefing {
    let mut text = rebase(&briefing.text, workdir, copy);
    text.push_str(&format!(
        "\n\n## Your workspace\n\nYou work in a private copy of the task's workspace at `{copy}`. \
         Where the task or this briefing names `{workdir}`, use `{copy}` instead: other attempts \
         at this task share `{workdir}`, so don't read or write it. If your result is chosen, \
         the host copies it to `{workdir}`.\n",
        copy = copy.display(),
        workdir = workdir.display(),
    ));
    let mut out = briefing.clone();
    out.text = text;
    out.included.push("best-of workspace".to_string());
    out
}

/// What a briefing was packed from, rebased to a candidate's copy.
#[must_use]
pub fn candidate_prepared(prepared: &Prepared, workdir: &Path, copy: &Path) -> Prepared {
    let mut out = prepared.clone();
    out.instruction = rebase(&prepared.instruction, workdir, copy);
    out.directions = rebase(&prepared.directions, workdir, copy);
    for item in &mut out.items {
        item.label = rebase(&item.label, workdir, copy);
        item.text = rebase(&item.text, workdir, copy);
    }
    out
}

/// Points an executor at a directory and a recorder of its own.
fn aim(exec: &mut Exec, dir: &Path, recorder: &Recorder) {
    match exec {
        Exec::Cli(cli) => {
            cli.workdir = dir.to_path_buf();
            cli.control.recorder = Some(recorder.clone());
        }
        Exec::Scripted(scripted) => {
            scripted.workdir = dir.to_path_buf();
            scripted.recorder = recorder.clone();
        }
        Exec::Micro(micro) => {
            micro.workdir = dir.to_path_buf();
            micro.recorder = recorder.clone();
        }
    }
}

/// One candidate's executor, copy, and result.
pub struct Candidate {
    pub exec: Exec,
    /// Its private copy of the workspace.
    pub dir: PathBuf,
    /// Where its session's events go until they join the episode's.
    pub recorder: Recorder,
    /// Why its copy couldn't be made, when it couldn't.
    pub refused: Option<String>,
    /// Its report, once it ran, with its copy's path rewritten back to the
    /// workspace's.
    pub report: Option<Report>,
    /// Its session's host-loop record.
    pub last: Value,
    /// The commands its session reported, rewritten to the workspace.
    pub claimed: Vec<generic::Claimed>,
}

/// The N candidates, run as one executor: [`Executor::execute`] runs them
/// all at once and returns the first's report. The composition then reads
/// every candidate from [`Fan::candidates`].
pub struct Fan {
    pub workdir: PathBuf,
    /// The episode's recorder, which each candidate's events join.
    pub recorder: Recorder,
    pub candidates: Vec<Candidate>,
    /// Dispatches made before the fan.
    pub runs_before: u32,
    /// The workspace's fingerprint before the candidates ran.
    pub before: Option<String>,
    /// Whether the workspace changed while they ran.
    pub leaked: Option<bool>,
}

/// Why a workspace can't be copied `n` times, or `None` when it can.
fn too_large(workdir: &Path, max_mb: u64) -> Option<String> {
    match super::tree_size(workdir, 20_000) {
        Some((_, bytes)) if bytes <= max_mb * 1024 * 1024 => None,
        _ => Some(format!(
            "the workspace is over {max_mb} MiB or 20,000 files, too large to copy for each candidate"
        )),
    }
}

impl Fan {
    /// N executors of `tier`, each to run in its own copy, or why one
    /// candidate runs instead.
    ///
    /// # Errors
    ///
    /// Returns why the tier can't run here, as [`Factory::make`] does.
    pub fn prepare<F: Factory>(
        setup: &Setup<'_>,
        policy: &BestOfPolicy,
        tier: &Tier,
        deadline: Duration,
        runs: u32,
        factory: &mut F,
    ) -> Result<Result<Fan, String>, String> {
        if let Some(why) = too_large(setup.workdir, policy.max_copy_mb) {
            return Ok(Err(why));
        }
        static MADE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let stamp = format!(
            "{}-{}-{}",
            std::process::id(),
            atif::now_ms(),
            MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        let mut candidates = Vec::with_capacity(policy.n);
        for i in 0..policy.n {
            let offset = u32::try_from(i).unwrap_or(u32::MAX);
            let mut exec = factory.make(tier, deadline, runs.saturating_add(offset))?;
            let dir = std::env::temp_dir().join(format!("coder-one-best-of-{stamp}-{}", i + 1));
            let recorder = Recorder::default();
            aim(&mut exec, &dir, &recorder);
            candidates.push(Candidate {
                exec,
                dir,
                recorder,
                refused: None,
                report: None,
                last: Value::Null,
                claimed: Vec::new(),
            });
        }
        Ok(Ok(Fan {
            workdir: setup.workdir.to_path_buf(),
            recorder: setup.recorder.clone(),
            candidates,
            runs_before: runs,
            before: None,
            leaked: None,
        }))
    }

    /// Dispatches run so far, the fan's included.
    #[must_use]
    pub fn runs(&self) -> u32 {
        self.candidates
            .iter()
            .map(|c| c.exec.runs())
            .max()
            .unwrap_or(self.runs_before)
    }

    /// Removes every candidate's copy.
    pub fn discard(&self) {
        for candidate in &self.candidates {
            let _ = std::fs::remove_dir_all(&candidate.dir);
        }
    }
}

/// A report that says why a candidate never started.
fn harness(why: String) -> Report {
    Report {
        status: Status::Harness(why),
        summary: delegate::Summary::default(),
        milliseconds: 0,
        stderr: String::new(),
        stream: None,
    }
}

/// `report` with the copy's path rewritten to the workspace's.
fn rebased_report(report: &Report, copy: &Path, workdir: &Path) -> Report {
    let mut out = report.clone();
    out.summary.result = report
        .summary
        .result
        .as_ref()
        .map(|r| rebase(r, copy, workdir));
    out.stderr = rebase(&report.stderr, copy, workdir);
    out
}

impl Executor for Fan {
    fn agent(&self) -> &str {
        self.candidates[0].exec.agent()
    }
    fn cost_provenance(&self) -> &'static str {
        self.candidates[0].exec.cost_provenance()
    }
    fn model(&self) -> &str {
        self.candidates[0].exec.model()
    }
    fn deadline(&self) -> Duration {
        self.candidates[0].exec.deadline()
    }
    fn describe(&self) -> Map<String, Value> {
        let mut out = self.candidates[0].exec.describe();
        out.insert(
            "best_of".to_string(),
            json!({ "n": self.candidates.len(), "candidate": 1 }),
        );
        out
    }
    async fn execute(&mut self, briefing: &Briefing) -> Report {
        self.before = delegate::fingerprint(&self.workdir);
        for candidate in &mut self.candidates {
            if let Err(error) = crate::handoff::copy_tree(&self.workdir, &candidate.dir) {
                candidate.refused = Some(format!("cannot copy the workspace: {error}"));
            }
        }
        let workdir = self.workdir.clone();
        let runs = futures_util::future::join_all(self.candidates.iter_mut().map(|candidate| {
            let brief = candidate_briefing(briefing, &workdir, &candidate.dir);
            async move {
                let report = match &candidate.refused {
                    Some(why) => harness(why.clone()),
                    None => candidate.exec.execute(&brief).await,
                };
                (brief, report)
            }
        }))
        .await;
        self.leaked = Some(delegate::fingerprint(&self.workdir) != self.before);
        let n = self.candidates.len();
        for (i, (candidate, (brief, report))) in self.candidates.iter_mut().zip(runs).enumerate() {
            // Each session's events join the episode's, candidate by
            // candidate, so the trajectory holds all of them.
            for step in candidate.recorder.steps() {
                self.recorder.push(step);
            }
            candidate.claimed = claimed(&candidate.recorder)
                .into_iter()
                .map(|c| generic::Claimed {
                    command: rebase(&c.command, &candidate.dir, &workdir),
                    exit_code: c.exit_code,
                })
                .collect();
            candidate.last = candidate.exec.last();
            // The first candidate's call is the one the dispatch records;
            // every other candidate is a call of its own.
            if i > 0 {
                let reason = Reason::Handoff(format!("{COMPONENT}: candidate {} of {n}", i + 1));
                let number = candidate.exec.runs();
                self.recorder.push(delegate::record(
                    &candidate.exec,
                    &brief,
                    &Delegation {
                        mode: Mode::Always,
                        reason: &reason,
                        isolation: "a private copy of the workspace",
                    },
                    &report,
                    number,
                ));
            }
            candidate.report = Some(report);
        }
        self.candidates[0]
            .report
            .clone()
            .unwrap_or_else(|| harness("the first candidate left no report".to_string()))
    }
    fn system_options(&self) -> Vec<String> {
        self.candidates[0].exec.system_options()
    }
    fn select_system(&mut self, answers: Vec<(String, Option<f64>)>) {
        for candidate in &mut self.candidates {
            candidate.exec.select_system(answers.clone());
        }
    }
    fn take_evidence(&mut self, prepared: &Prepared) {
        for candidate in &mut self.candidates {
            let rebased = candidate_prepared(prepared, &self.workdir, &candidate.dir);
            candidate.exec.take_evidence(&rebased);
        }
    }
}

/// One candidate's place in the ranking.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Score {
    /// 1-based.
    pub number: usize,
    /// Whether its session ended with an answer.
    pub answered: bool,
    /// The verdict's call: `pass`, `unknown`, or `fail`; `None` when none
    /// was asked.
    pub verdict: Option<String>,
    /// Failed scenarios and contradicted requirements.
    pub failures: usize,
    /// Requirements a scenario observed or support read as supported.
    pub confirmed: usize,
    /// The dispatch's priced cost, when known.
    pub cost_usd: Option<f64>,
}

impl Score {
    /// The verdict's rank: pass 0, unknown or none 1, fail 2, and 3 for a
    /// session that ended without an answer, whatever the verdict said.
    #[must_use]
    pub fn verdict_rank(&self) -> u8 {
        if !self.answered {
            return 3;
        }
        match self.verdict.as_deref() {
            Some("pass") => 0,
            Some("fail") => 2,
            _ => 1,
        }
    }

    fn key(&self) -> (u8, usize, std::cmp::Reverse<usize>, u64, usize) {
        // An unknown cost ranks after every known one.
        let cost = self.cost_usd.map_or(u64::MAX, |usd| (usd * 1e9) as u64);
        (
            self.verdict_rank(),
            self.failures,
            std::cmp::Reverse(self.confirmed),
            cost,
            self.number,
        )
    }
}

/// The kept candidate's index in `scores`, and why, in a sentence. The
/// verdict first, then the scorecard, then cost, then the lowest number.
///
/// # Panics
///
/// Panics when `scores` is empty.
#[must_use]
pub fn pick(scores: &[Score]) -> (usize, String) {
    let mut order: Vec<usize> = (0..scores.len()).collect();
    order.sort_by_key(|&i| scores[i].key());
    let best = *order.first().expect("at least one candidate");
    let kept = &scores[best];
    let decided_by = match order.get(1).map(|&j| scores[j].key()) {
        None => "it is the only candidate",
        Some(next) => {
            let own = kept.key();
            if own.0 != next.0 {
                "its verdict is the best"
            } else if (own.1, own.2) != (next.1, next.2) {
                "its verdict ties and its checks are the best"
            } else if own.3 != next.3 {
                "its verdict and checks tie and it cost the least"
            } else {
                "it ties on every key and has the lowest number"
            }
        }
    };
    let why = format!(
        "candidate {} kept: {decided_by} (verdict {}, {} failure(s), {} confirmed, {})",
        kept.number,
        if kept.answered {
            kept.verdict.as_deref().unwrap_or("not asked")
        } else {
            "no answer"
        },
        kept.failures,
        kept.confirmed,
        kept.cost_usd
            .map_or_else(|| "cost unknown".to_string(), |usd| format!("${usd:.4}")),
    );
    (best, why)
}

/// What the selection leaves for the rest of the composition.
pub struct Selected {
    /// The kept candidate's report, rebased to the workspace.
    pub report: Report,
    pub last: Value,
    /// Its checks, on the real workspace.
    pub checked: Option<(checks::Input, checks::Report)>,
    /// Its verdict, when `verify.verdict` asked.
    pub verdict: Option<(crate::checks::verdict::Verdict, Value)>,
    /// A dispatch row per candidate.
    pub branches: Vec<Value>,
    /// A checks-log entry per candidate.
    pub checks_log: Vec<Value>,
    /// `best_of` in the composition record.
    pub record: Value,
}

/// The checks file of candidate `number`.
#[must_use]
pub fn checks_file(number: usize) -> String {
    format!("verification/checks-candidate-{number}.json")
}

/// Archives `copy` as `best-of/candidate-N.tar.gz` under the episode.
fn archive(dir: &Path, copy: &Path, number: usize) -> Value {
    let target = dir.join(DIR);
    if let Err(error) = std::fs::create_dir_all(&target) {
        return json!({ "error": error.to_string() });
    }
    let file = format!("{DIR}/candidate-{number}.tar.gz");
    let out = std::process::Command::new("tar")
        .arg("-czf")
        .arg(dir.join(&file))
        .arg("-C")
        .arg(copy)
        .arg(".")
        .output();
    match out {
        Ok(out) if out.status.code().is_some_and(|code| code <= 1) => json!({
            "path": file,
            "bytes": std::fs::metadata(dir.join(&file)).map(|m| m.len()).unwrap_or(0),
        }),
        Ok(out) => json!({
            "error": format!(
                "tar exited {}: {}",
                out.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&out.stderr).trim()
            )
        }),
        Err(error) => json!({ "error": format!("cannot run tar: {error}") }),
    }
}

/// Checks each candidate in the real workspace, asks the verdict about
/// each, keeps one, and leaves its copy as the workspace.
#[allow(clippy::too_many_lines)]
pub async fn select(
    setup: &Setup<'_>,
    subject: &Subject,
    verify: &VerifyPolicy,
    policy: &BestOfPolicy,
    fan: &mut Fan,
    tier: &Tier,
    granted: u64,
) -> Selected {
    let workdir = setup.workdir;
    let mut scores = Vec::new();
    let mut checked_all = Vec::new();
    let mut verdicts = Vec::new();
    let mut rows = Vec::new();
    let mut entries = Vec::new();
    let mut checks_log = Vec::new();
    for (i, candidate) in fan.candidates.iter_mut().enumerate() {
        let number = i + 1;
        let report = rebased_report(
            &candidate
                .report
                .clone()
                .unwrap_or_else(|| harness("the candidate never ran".to_string())),
            &candidate.dir,
            workdir,
        );
        candidate.report = Some(report.clone());
        let mut branch = row(
            &format!("candidate-{number}"),
            tier,
            &report,
            &candidate.last,
            granted,
        );
        let placed = if candidate.refused.is_some() {
            Err("its copy was never made".to_string())
        } else {
            replace_contents(workdir, &candidate.dir)
        };
        let (checked, standing) = match (&placed, verify.checks) {
            (Ok(()), true) => {
                let mut subject = subject.clone();
                if let Some(live) = &mut subject.live {
                    live.claimed.clone_from(&candidate.claimed);
                    live.report = Some(report.output());
                }
                let file = checks_file(number);
                let done =
                    checks::check_subject_as(&subject, workdir, setup.dir, setup.recorder, &file)
                        .await;
                let standing = Standing::of(&done.1, None);
                checks_log.push(json!({
                    "after": format!("candidate-{number}"),
                    "file": file,
                    "summary": done.1.summary(),
                    "self_report": super::self_reported(&done.1),
                }));
                (Some(done), Some(standing))
            }
            _ => (None, None),
        };
        let verdict = if verify.verdict && report.status == Status::Answered {
            Some(
                super::assess_verdict(setup, &report.output(), &format!("candidate-{number}"))
                    .await,
            )
        } else {
            None
        };
        let archived = match (&placed, policy.archive) {
            (Ok(()), true) => archive(setup.dir, &candidate.dir, number),
            _ => Value::Null,
        };
        let (charge, _) = delegate::charge(&report);
        let score = Score {
            number,
            answered: report.status == Status::Answered,
            verdict: verdict.as_ref().map(|(v, _)| v.call.clone()),
            failures: standing.map_or(usize::MAX / 2, |s| s.failed + s.contradicted),
            confirmed: standing.map_or(0, |s| s.confirmed),
            cost_usd: (charge == "priced")
                .then_some(report.summary.total_cost_usd)
                .flatten(),
        };
        entries.push(json!({
            "number": number,
            "status": report.status.word(),
            "cost_usd": score.cost_usd,
            "charge": charge,
            "milliseconds": report.milliseconds,
            "refused": candidate.refused,
            "placed": placed.as_ref().err(),
            "checks_file": checked.as_ref().map(|_| checks_file(number)),
            "checks": checked.as_ref().map(|(_, r)| r.summary()),
            "standing": standing,
            "verdict": verdict.as_ref().map(|(_, record)| record.clone()),
            "score": score,
            "archive": archived,
            "report_chars": report.output().chars().count(),
        }));
        branch["candidate"] = json!(number);
        rows.push(branch);
        scores.push(score);
        checked_all.push(checked);
        verdicts.push(verdict);
    }
    let (best, why) = pick(&scores);
    let kept = &fan.candidates[best];
    let restored = if kept.refused.is_none() {
        replace_contents(workdir, &kept.dir).err()
    } else {
        None
    };
    // The kept candidate's checks are the composition's first checks, so
    // readers of `verification/checks.json` find them where they always
    // have.
    if checked_all[best].is_some() {
        let _ = std::fs::copy(
            setup.dir.join(checks_file(best + 1)),
            setup.dir.join(checks::COVERAGE_FILE),
        );
    }
    for (i, row) in rows.iter_mut().enumerate() {
        row["kept"] = json!(i == best);
    }
    let oracle_verdicts: Vec<Value> = scores
        .iter()
        .map(|s| {
            json!(s.verdict.as_deref().unwrap_or(if s.answered {
                "not asked"
            } else {
                "no answer"
            }))
        })
        .collect();
    let mut record = json!({
        "n": fan.candidates.len(),
        "order": ORDER,
        "kept": best + 1,
        "why": why,
        "verdicts": oracle_verdicts,
        "leaked": fan.leaked,
        "candidates": entries,
        "policy": policy,
    });
    if let Some(error) = &restored {
        record["restore_error"] = json!(error);
    }
    super::record_decision(setup.recorder, COMPONENT, "kept", &record);
    println!("  best of {} ▸ {why}", fan.candidates.len());
    let report = kept
        .report
        .clone()
        .unwrap_or_else(|| harness("no report".to_string()));
    let last = kept.last.clone();
    fan.discard();
    Selected {
        report,
        last,
        checked: checked_all.swap_remove(best),
        verdict: verdicts.swap_remove(best),
        branches: rows,
        checks_log,
        record,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn score(number: usize, verdict: &str, failures: usize, confirmed: usize, usd: f64) -> Score {
        Score {
            number,
            answered: true,
            verdict: Some(verdict.to_string()),
            failures,
            confirmed,
            cost_usd: Some(usd),
        }
    }

    #[test]
    fn the_verdict_decides_before_the_checks_and_the_checks_before_cost() {
        // A passing verdict beats cleaner checks and a lower cost.
        let scores = [
            score(1, "unknown", 0, 5, 0.01),
            score(2, "pass", 2, 1, 0.09),
            score(3, "fail", 0, 9, 0.001),
        ];
        let (kept, why) = pick(&scores);
        assert_eq!(kept, 1, "{why}");
        assert!(why.contains("its verdict is the best"), "{why}");
        // With the verdicts tied, fewer failures win, then more confirmed.
        let scores = [
            score(1, "unknown", 1, 9, 0.001),
            score(2, "unknown", 0, 1, 0.05),
            score(3, "unknown", 0, 2, 0.09),
        ];
        let (kept, why) = pick(&scores);
        assert_eq!(kept, 2, "{why}");
        assert!(why.contains("checks are the best"), "{why}");
        // With the verdicts and checks tied, the cheapest wins.
        let scores = [score(1, "pass", 0, 2, 0.05), score(2, "pass", 0, 2, 0.02)];
        let (kept, why) = pick(&scores);
        assert_eq!(kept, 1, "{why}");
        assert!(why.contains("cost the least"), "{why}");
        // A full tie keeps the lowest number.
        let scores = [score(1, "pass", 0, 2, 0.02), score(2, "pass", 0, 2, 0.02)];
        let (kept, why) = pick(&scores);
        assert_eq!(kept, 0, "{why}");
        assert!(why.contains("lowest number"), "{why}");
    }

    #[test]
    fn a_session_without_an_answer_ranks_below_a_failed_verdict() {
        let mut silent = score(1, "pass", 0, 9, 0.0);
        silent.answered = false;
        let scores = [silent, score(2, "fail", 3, 0, 0.5)];
        let (kept, why) = pick(&scores);
        assert_eq!(kept, 1, "{why}");
        // No verdict asked ranks with unknown, ahead of fail.
        let mut unasked = score(1, "", 1, 0, 0.1);
        unasked.verdict = None;
        let (kept, _) = pick(&[score(2, "fail", 0, 3, 0.0), unasked]);
        assert_eq!(kept, 1);
        // An unknown cost ranks after a known one.
        let mut unpriced = score(1, "pass", 0, 2, 0.0);
        unpriced.cost_usd = None;
        let (kept, _) = pick(&[unpriced, score(2, "pass", 0, 2, 0.4)]);
        assert_eq!(kept, 1);
    }

    #[test]
    fn rebasing_changes_whole_paths_only() {
        let from = Path::new("/app");
        let to = Path::new("/tmp/c-1");
        assert_eq!(
            rebase("Write /app/out.json from `/app`, then cd /app.", from, to),
            "Write /tmp/c-1/out.json from `/tmp/c-1`, then cd /tmp/c-1."
        );
        assert_eq!(
            rebase("/apple /opt/app app /app-data /app_x /app.bak", from, to),
            "/apple /opt/app app /app-data /app_x /app.bak"
        );
        assert_eq!(
            rebase("(/app)\n\"/app/\"", from, to),
            "(/tmp/c-1)\n\"/tmp/c-1/\""
        );
        // And back, for a report that names its copy.
        assert_eq!(rebase("/tmp/c-1/x /tmp/c-10", to, from), "/app/x /tmp/c-10");
    }

    #[test]
    fn a_candidate_briefing_names_its_copy_and_keeps_the_rest() {
        let briefing = Briefing {
            text: "Fix /app/main.py.".to_string(),
            cap: 100,
            included: vec!["task".to_string()],
            omitted: Vec::new(),
        };
        let out = candidate_briefing(&briefing, Path::new("/app"), Path::new("/tmp/c-2"));
        assert!(
            out.text.starts_with("Fix /tmp/c-2/main.py."),
            "{}",
            out.text
        );
        assert!(
            out.text
                .contains("private copy of the task's workspace at `/tmp/c-2`")
        );
        assert!(
            out.text
                .contains("other attempts at this task share `/app`")
        );
        assert_eq!(out.included, ["task", "best-of workspace"]);
    }

    #[test]
    fn a_policy_asks_for_two_to_eight_candidates() {
        let policy = |n| BestOfPolicy {
            n,
            max_copy_mb: 256,
            archive: true,
        };
        assert!(policy(1).validate()[0].contains("from 2 to 8"));
        assert!(policy(9).validate()[0].contains("from 2 to 8"));
        assert!(policy(3).validate().is_empty());
        let parsed: BestOfPolicy = serde_json::from_value(json!({ "n": 5 })).unwrap();
        assert_eq!(
            parsed,
            BestOfPolicy {
                n: 5,
                max_copy_mb: 256,
                archive: true
            }
        );
        assert!(serde_json::from_value::<BestOfPolicy>(json!({ "n": 5, "mode": "x" })).is_err());
    }
}
