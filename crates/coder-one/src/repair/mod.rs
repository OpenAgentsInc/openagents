//! `verify.repair`: one bounded repair session from a diagnostic packet,
//! then a recheck of what changed.
//!
//! All five failed v3 Luna delegates reported success and the episode
//! ended. `verify.checks` now finds such a gap and leaves a diagnostic
//! packet: the requirement, the candidate, the scenario and its expected
//! relation, the observations, and the explanations they leave open. This
//! component turns the packets into a **delta brief**, runs one **fresh**
//! executor session on it within what is left of the episode deadline,
//! and then invalidates the requirement states the changed candidate no
//! longer matches and reruns the checks.
//!
//! The brief carries only public text and the host's own observations:
//! the task, the requirement and the instruction spans it rests on, the
//! candidate's identity, and each packet. Protected verifier output
//! never reaches it, and [`Brief::check_clean`] refuses one that holds a
//! protected string.
//!
//! A repair is a new session, never a resume: it starts from the brief
//! alone, gets its own session ID, and records `resumes: null`.

pub mod cli;
pub mod scripts;
pub mod study;

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::checks::{self, Candidate, Input, Packet, Report, TaskText};
use crate::component::jev::JevMode;
use crate::deadline::Deadline;
use crate::delegate::{self, Briefing, Delegation, Executor, Mode, Reason};
use crate::minitask::MiniTask;
use crate::record::{Cost, Finish, Implementation, Outcome, Recorder, Start};

/// The schema of a repair record.
pub const SCHEMA: &str = "openagents.coder-one.repair.v1";

/// Where a run keeps its repair record, relative to its directory.
pub const FILE: &str = "verification/repair.json";

/// Where the recheck after a repair writes its report.
pub const RECHECK_FILE: &str = "verification/checks-repaired.json";

/// The heading a packet brief carries and a plain brief doesn't.
pub const PACKET_MARK: &str = "## What the check observed";

/// The most characters of one packet's observations a brief carries.
const OBSERVATION_CHARS: usize = 3_000;

/// The most packets one brief carries.
const MAX_PACKETS: usize = 3;

/// What the repair session is told.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BriefKind {
    /// The delta brief from the diagnostic packets.
    Packet,
    /// The task and a request to check the work, with no packet: the
    /// control that separates a concrete counterexample from extra
    /// sampling.
    Plain,
}

impl BriefKind {
    /// The kind as the record and the command line spell it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            BriefKind::Packet => "packet",
            BriefKind::Plain => "plain",
        }
    }
}

/// When a repair runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Trigger {
    /// Only when a check contradicted a requirement: the episode's policy.
    Detected,
    /// Only when a scenario observed a failure. A requirement that only the
    /// support judge contradicted doesn't trigger a repair: on
    /// git-leak-recovery those judgments repaired passing work three times
    /// out of three and changed nothing.
    Checked,
    /// On every candidate, to count what an unneeded repair breaks.
    Always,
}

impl Trigger {
    /// The trigger as the record and the command line spell it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Trigger::Detected => "detected",
            Trigger::Checked => "checked",
            Trigger::Always => "always",
        }
    }

    /// Parses `detected`, `checked`, or `always`.
    ///
    /// # Errors
    ///
    /// Returns a message for another word.
    pub fn parse(word: &str) -> Result<Self, String> {
        match word {
            "detected" => Ok(Trigger::Detected),
            "checked" => Ok(Trigger::Checked),
            "always" => Ok(Trigger::Always),
            other => Err(format!(
                "a trigger is detected, checked, or always, not {other}"
            )),
        }
    }
}

/// The implementation: the brief's contents and the repair count.
#[must_use]
pub fn implementation(kind: BriefKind, trigger: Trigger) -> Implementation {
    Implementation::new(
        "verify.repair",
        "one fresh session from a delta brief",
        &json!({
            "version": 1,
            "repairs": 1,
            "brief": kind,
            "trigger": trigger,
            "packets": MAX_PACKETS,
            "observation_chars": OBSERVATION_CHARS,
            "recheck": "every scenario, when the candidate changed",
        }),
    )
}

/// One requirement a check left contradicted, with the packets that
/// contradict it and `verify.support`'s answers when it judged it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Gap {
    pub requirement: String,
    pub text: String,
    pub packets: Vec<Packet>,
    /// Supports and contradicts, when `verify.support` contradicted it
    /// and no scenario did.
    pub support: Option<(Option<f64>, Option<f64>)>,
}

/// The gaps a check and a support run leave: each requirement with a
/// failing packet, then each requirement `verify.support` contradicted
/// that no packet names.
#[must_use]
pub fn gaps(report: &Report, support: Option<&crate::support::Report>) -> Vec<Gap> {
    let mut out: Vec<Gap> = Vec::new();
    for packet in &report.packets {
        match out.iter_mut().find(|g| g.requirement == packet.requirement) {
            Some(gap) => gap.packets.push(packet.clone()),
            None => out.push(Gap {
                requirement: packet.requirement.clone(),
                text: packet.requirement_text.clone(),
                packets: vec![packet.clone()],
                support: None,
            }),
        }
    }
    for state in support.map_or(&[][..], |s| s.states.as_slice()) {
        if state.state == "contradicted" && !out.iter().any(|g| g.requirement == state.id) {
            out.push(Gap {
                requirement: state.id.clone(),
                text: state.text.clone(),
                packets: Vec::new(),
                support: Some((state.judgment.supports, state.judgment.contradicts)),
            });
        }
    }
    out
}

/// A repair brief.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Brief {
    pub text: String,
    pub kind: BriefKind,
    /// The candidate revision the brief describes.
    pub candidate: String,
    /// The requirements it names.
    pub requirements: Vec<String>,
    /// The scenarios whose packets it carries.
    pub scenarios: Vec<String>,
}

impl Brief {
    /// The brief's sha256, hex.
    #[must_use]
    pub fn sha256(&self) -> String {
        Sha256::digest(self.text.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    /// The brief as a record holds it, without its text.
    #[must_use]
    pub fn record(&self, path: Option<&str>) -> Value {
        json!({
            "kind": self.kind,
            "sha256": self.sha256(),
            "chars": self.text.chars().count(),
            "candidate": self.candidate,
            "requirements": self.requirements,
            "scenarios": self.scenarios,
            "path": path,
        })
    }

    /// The brief as the executor receives it.
    #[must_use]
    pub fn briefing(&self) -> Briefing {
        Briefing {
            text: self.text.clone(),
            cap: self.text.chars().count(),
            included: vec![format!("{} repair brief", self.kind.word())],
            omitted: Vec::new(),
        }
    }

    /// Refuses a brief that holds any of `protected`, such as a
    /// verifier's test names.
    ///
    /// # Errors
    ///
    /// Returns the protected strings the brief holds.
    pub fn check_clean(&self, protected: &[String]) -> Result<(), String> {
        let found: Vec<&String> = protected
            .iter()
            .filter(|p| !p.is_empty() && self.text.contains(p.as_str()))
            .collect();
        if found.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "the brief holds protected text: {}",
                found
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        }
    }
}

fn clip(text: &str, max: usize) -> String {
    crate::judge::clip(text, max)
}

fn files_line(candidate: &Candidate) -> String {
    let mut names: Vec<&str> = candidate.files.keys().map(String::as_str).collect();
    names.truncate(20);
    if names.is_empty() {
        format!(
            "It wrote no file the host could read; it ran {} inline programs.",
            candidate.programs.len()
        )
    } else {
        format!("Its files: {}.", names.join(", "))
    }
}

/// The delta brief: each gap's requirement, the instruction spans it
/// rests on, and each packet's scenario, expected relation,
/// observations, and open explanations.
#[must_use]
pub fn packet_brief(
    task: &TaskText,
    candidate: &Candidate,
    report: &Report,
    gaps: &[Gap],
) -> Brief {
    let digest = candidate.digest();
    let mut text = String::from(
        "A host check ran your candidate and observed that it doesn't meet a \
requirement of the task. Fix what the observations show, keep what already \
works, and check the fix yourself before you finish. Don't rewrite parts the \
check didn't question.\n\n",
    );
    text.push_str(&format!("## The task\n\n{}\n\n", task.instruction.trim()));
    text.push_str(&format!(
        "## The candidate\n\nRevision {}. {}\n\n",
        &digest[..digest.len().min(12)],
        files_line(candidate)
    ));
    let mut scenarios = Vec::new();
    let mut packets = 0;
    for gap in gaps {
        text.push_str(&format!(
            "## The requirement {}\n\n{}\n\n",
            gap.requirement,
            gap.text.trim()
        ));
        let spans: Vec<&checks::SpanRef> = gap
            .packets
            .iter()
            .filter_map(|p| report.scenarios.iter().find(|s| s.id == p.scenario))
            .flat_map(|s| s.spans.iter())
            .filter(|span| span.requirement == gap.requirement)
            .collect();
        let mut seen = Vec::new();
        for span in spans {
            if !seen.contains(&&span.text) {
                seen.push(&span.text);
            }
        }
        if !seen.is_empty() {
            text.push_str("It rests on these words of the task:\n\n");
            for span in seen {
                text.push_str(&format!("> {}\n", span.trim().replace('\n', "\n> ")));
            }
            text.push('\n');
        }
        for packet in &gap.packets {
            if packets >= MAX_PACKETS {
                break;
            }
            packets += 1;
            scenarios.push(packet.scenario.clone());
            let scenario = report.scenarios.iter().find(|s| s.id == packet.scenario);
            let observed = crate::support::scrub(
                &serde_json::to_string_pretty(&packet.observations).unwrap_or_default(),
            );
            text.push_str(&format!("{PACKET_MARK}\n\n"));
            if let Some(scenario) = scenario {
                text.push_str(&format!(
                    "Scenario `{}` drove the candidate through `{}`.\n\n",
                    packet.scenario,
                    crate::support::scrub(&scenario.interface)
                ));
            }
            text.push_str(&format!(
                "Expected: {}\n\nHow the expectation was derived: {}\n\nObserved:\n\n```json\n{}\n```\n\n",
                packet.expected.statement,
                packet.expected.derivation,
                clip(&observed, OBSERVATION_CHARS)
            ));
            if !packet.hypotheses.is_empty() {
                text.push_str("Explanations the observations leave open:\n\n");
                for hypothesis in &packet.hypotheses {
                    text.push_str(&format!("- {hypothesis}\n"));
                }
                text.push('\n');
            }
        }
        if let Some((supports, contradicts)) = gap.support {
            text.push_str(&format!(
                "## What the judge read\n\nNo scenario failed here. A judgment of the requirement against the candidate's code and the observations found the evidence contradicts it (supports {}, contradicts {}).\n\n",
                supports.map_or("unknown".to_string(), |p| format!("{p:.2}")),
                contradicts.map_or("unknown".to_string(), |p| format!("{p:.2}")),
            ));
        }
    }
    text.push_str(
        "## What to do\n\nFind the cause of the observed difference in the candidate, \
fix it, and regenerate any output the task asks for from the fixed code. Then \
report what you changed.\n",
    );
    Brief {
        text,
        kind: BriefKind::Packet,
        candidate: digest,
        requirements: gaps.iter().map(|g| g.requirement.clone()).collect(),
        scenarios,
    }
}

/// The plain brief: the task and a request to check the work, with no
/// packet.
#[must_use]
pub fn plain_brief(task: &TaskText, candidate: &Candidate) -> Brief {
    let digest = candidate.digest();
    let text = format!(
        "Another session worked on this task and reported it done. Check the work \
against the task and fix anything that doesn't meet it. Keep what already works.\n\n\
## The task\n\n{}\n\n## The candidate\n\nRevision {}. {}\n\n## What to do\n\n\
Check each requirement of the task against the files, fix what's missing, and \
report what you changed.\n",
        task.instruction.trim(),
        &digest[..digest.len().min(12)],
        files_line(candidate)
    );
    Brief {
        text,
        kind: BriefKind::Plain,
        candidate: digest,
        requirements: Vec::new(),
        scenarios: Vec::new(),
    }
}

/// Where a repair works and records.
pub struct Place<'a> {
    /// The mini-task, when the repair runs on one; a scripted repair
    /// profile needs it.
    pub task: Option<&'a MiniTask>,
    /// What the recheck reads besides the workspace.
    pub subject: &'a checks::Subject,
    /// The workspace the candidate lives in, repaired in place.
    pub work: &'a Path,
    /// The run's directory: `verification/` goes under it.
    pub dir: &'a Path,
    /// Where the brief and the session's stream go.
    pub artifacts: &'a Path,
    pub recorder: &'a Recorder,
    /// The episode deadline the repair spends from.
    pub deadline: &'a Deadline,
    /// Jev, to rejudge support after the recheck; `None` skips it.
    pub jev: Option<JevMode>,
    /// The session that produced the candidate, when known. The repair
    /// never resumes it; the record names it so the two stay apart.
    pub previous_session: Option<String>,
    /// `verify.support`'s parameters for the rejudgment; the defaults when
    /// `None`.
    pub support_params: Option<crate::support::Params>,
}

/// The repair policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Policy {
    pub kind: BriefKind,
    pub trigger: Trigger,
    /// The most time the session asks of the episode deadline.
    pub allowance: Duration,
}

/// What one repair did.
#[derive(Clone, Debug)]
pub struct Repaired {
    /// The record written to `verification/repair.json`.
    pub record: Value,
    /// Whether a session ran.
    pub ran: bool,
    /// Whether the candidate changed.
    pub changed: bool,
    /// The recheck, when the candidate changed.
    pub recheck: Option<(Input, Report)>,
    /// What the session cost: zero when none ran, unknown when it may
    /// have billed and didn't say.
    pub cost_usd: Option<f64>,
    /// `verify.support`'s rejudgment of the changed candidate, when Jev
    /// was given.
    pub support_after: Option<crate::support::Report>,
}

fn session_cost(agent: &str, report: &delegate::Report) -> Cost {
    if agent == "scripted" {
        // A script runs no inference.
        return Cost::none();
    }
    match delegate::charge(report) {
        ("priced", _) => Cost {
            usd: report.summary.total_cost_usd,
            provenance: report
                .summary
                .cost_provenance
                .unwrap_or("provider_reported")
                .to_string(),
        },
        ("zero", _) => Cost::none(),
        _ => Cost::unknown(),
    }
}

/// Repairs the candidate in `place.work` once, if `policy` says to, and
/// records it as a `verify.repair` invocation: the brief, one fresh
/// executor session as a child `exec.session`, and the recheck.
/// `make` builds the executor with the deadline the episode granted.
///
/// # Errors
///
/// Returns a message when a file can't be written or the executor can't
/// be made. A repair that doesn't fix anything is not an error.
pub async fn attempt<E: Executor>(
    place: &Place<'_>,
    before: (&Input, &Report),
    support: Option<&crate::support::Report>,
    policy: Policy,
    make: impl FnOnce(Duration) -> Result<E, String>,
) -> Result<Repaired, String> {
    let (input, report) = before;
    let found = gaps(report, support);
    let invocation = place.recorder.enter(
        Start::new("verify.repair", implementation(policy.kind, policy.trigger))
            .named(&format!("{} brief", policy.kind.word()))
            .reading(&json!({
                "candidate": input.candidate.digest(),
                "gaps": found.iter().map(|g| &g.requirement).collect::<Vec<_>>(),
            }))
            .with_effects(),
    );
    let triggered = match policy.trigger {
        Trigger::Detected => !found.is_empty(),
        Trigger::Checked => found.iter().any(|gap| !gap.packets.is_empty()),
        Trigger::Always => true,
    };
    let mut record = json!({
        "schema": SCHEMA,
        "implementation": implementation(policy.kind, policy.trigger),
        "trigger": policy.trigger,
        "triggered": triggered,
        "gaps": found.iter().map(|g| json!({ "requirement": g.requirement, "scenarios": g.packets.iter().map(|p| &p.scenario).collect::<Vec<_>>(), "support": g.support })).collect::<Vec<_>>(),
        "candidate_before": input.candidate.digest(),
    });
    let skip = |why: &str, mut record: Value| -> Result<Repaired, String> {
        record["skipped"] = json!(why);
        place.recorder.end(
            &invocation,
            Finish::new(Outcome::Skipped)
                .output(json!({ "skipped": why }))
                .cost(Cost::none()),
        );
        write(place.dir, &record)?;
        Ok(Repaired {
            record,
            ran: false,
            changed: false,
            recheck: None,
            cost_usd: Some(0.0),
            support_after: None,
        })
    };
    if !triggered {
        return skip("no check contradicted a requirement", record);
    }
    let Some(granted) = place.deadline.grant("verify.repair", policy.allowance) else {
        return skip("the episode deadline left no time for a repair", record);
    };
    let brief = match policy.kind {
        BriefKind::Packet if !found.is_empty() => {
            packet_brief(&input.task, &input.candidate, report, &found)
        }
        _ => plain_brief(&input.task, &input.candidate),
    };
    let brief_path = place.artifacts.join("repair-1.brief.md");
    std::fs::create_dir_all(place.artifacts)
        .and_then(|()| std::fs::write(&brief_path, &brief.text))
        .map_err(|e| format!("cannot write {}: {e}", brief_path.display()))?;
    record["brief"] = brief.record(Some("artifacts/repair-1.brief.md"));
    if policy.kind == BriefKind::Packet && found.is_empty() {
        record["brief_note"] = json!("no packet to give: the plain brief ran instead");
    }
    record["granted_ms"] = json!(u64::try_from(granted.as_millis()).unwrap_or(u64::MAX));

    let mut executor = make(granted)?;
    let session = place.recorder.enter(
        Start::new(
            "exec.session",
            Implementation::new(
                "exec.session",
                &format!("{} {}", executor.agent(), executor.model()),
                &json!({
                    "agent": executor.agent(),
                    "model": executor.model(),
                    "deadline_sec": granted.as_secs(),
                    "describe": executor.describe(),
                    "session": "fresh",
                    "resumes": null,
                }),
            ),
        )
        .named(&format!(
            "repair · fresh session · {} ({})",
            executor.agent(),
            executor.model()
        ))
        .reading_digest(brief.sha256())
        .with_effects(),
    );
    let delegated = delegate::delegate(
        &mut executor,
        &brief.briefing(),
        &Delegation {
            mode: Mode::Always,
            reason: &Reason::Repair,
            isolation: "the episode's own workspace",
        },
        place.recorder,
        1,
    )
    .await;
    let cost = session_cost(executor.agent(), &delegated);
    let session_record = json!({
        "fresh": true,
        "resumes": null,
        "previous_session": place.previous_session,
        "session_id": delegated.summary.session_id,
        "agent": executor.agent(),
        "model": executor.model(),
        "status": delegated.status.word(),
        "turns": delegated.summary.num_turns,
        "milliseconds": delegated.milliseconds,
        "cost_usd": cost.usd,
        "cost_provenance": cost.provenance,
        "result": delegated.summary.result.as_deref().map(|r| clip(r, 500)),
    });
    place.recorder.end(
        &session,
        Finish::new(if delegated.status == delegate::Status::Answered {
            Outcome::Completed
        } else {
            Outcome::Failed
        })
        .summary(session_record.clone())
        .cost(cost.clone()),
    );
    record["session"] = session_record;

    // The candidate the repair left, and what its change invalidates.
    let after_input = place.subject.input(place.work);
    let after = after_input.candidate.digest();
    let changed = after != input.candidate.digest();
    record["candidate_after"] = json!(after);
    record["changed"] = json!(changed);
    let mut invalidated: Vec<String> = if changed {
        report
            .coverage
            .iter()
            .filter(|c| !c.scenarios.is_empty())
            .map(|c| c.id.clone())
            .collect()
    } else {
        Vec::new()
    };
    for id in support.map(|s| s.stale_for(&after)).unwrap_or_default() {
        if !invalidated.contains(&id) {
            invalidated.push(id);
        }
    }
    record["invalidated"] = json!(invalidated);
    let mut support_after = None;
    let recheck = if changed {
        let (input, report) = checks::check_subject_as(
            place.subject,
            place.work,
            place.dir,
            place.recorder,
            RECHECK_FILE,
        )
        .await;
        record["recheck"] = json!({
            "file": RECHECK_FILE,
            "summary": report.summary(),
            "gaps": gaps(&report, None).iter().map(|g| &g.requirement).collect::<Vec<_>>(),
        });
        if let Some(jev) = &place.jev {
            let judged = crate::support::judge(
                &input,
                &report,
                jev,
                place.recorder,
                place.support_params.unwrap_or_default(),
                Some(place.deadline.clone()),
            )
            .await;
            record["support_after"] = judged.summary();
            support_after = Some(judged);
        }
        Some((input, report))
    } else {
        record["recheck"] = json!({
            "skipped": "the candidate didn't change, so the earlier observations still hold",
        });
        None
    };
    let still =
        recheck.as_ref().is_some_and(|(_, r)| r.detected()) || (!changed && !found.is_empty());
    place.recorder.end(
        &invocation,
        Finish::new(if still {
            Outcome::Failed
        } else {
            Outcome::Completed
        })
        .output(json!({
            "brief": record["brief"],
            "session": { "fresh": true, "resumes": null, "session_id": record["session"]["session_id"], "status": record["session"]["status"] },
            "changed": changed,
            "invalidated": record["invalidated"],
            "recheck": record["recheck"]["summary"],
        })),
    );
    write(place.dir, &record)?;
    Ok(Repaired {
        cost_usd: cost.usd,
        record,
        ran: true,
        changed,
        recheck,
        support_after,
    })
}

/// Who repairs: a scripted repair profile, or a real CLI and model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Profile {
    /// One of [`scripts::PROFILES`].
    Scripted(String),
    /// Claude Code or Codex inside a `coder-boundary` boundary.
    Cli {
        agent: delegate::Agent,
        model: String,
    },
}

impl Profile {
    /// Parses a scripted profile, such as `fix-if-packet`, or
    /// `AGENT:MODEL`, such as `codex:gpt-6-luna`.
    ///
    /// # Errors
    ///
    /// Returns a message for an unknown profile or agent.
    pub fn parse(text: &str) -> Result<Self, String> {
        if let Some((agent, model)) = text.split_once(':') {
            return Ok(Profile::Cli {
                agent: delegate::Agent::parse(agent)?,
                model: model.to_string(),
            });
        }
        if scripts::PROFILES.contains(&text) {
            Ok(Profile::Scripted(text.to_string()))
        } else {
            Err(format!(
                "a repair profile is one of {} or AGENT:MODEL, not {text}",
                scripts::PROFILES.join(", ")
            ))
        }
    }

    /// The profile as the record and the command line spell it.
    #[must_use]
    pub fn word(&self) -> String {
        match self {
            Profile::Scripted(name) => format!("scripted:{name}"),
            Profile::Cli { agent, model } => format!("{}:{model}", agent.word()),
        }
    }
}

/// Repairs once with `profile`: a scripted session on virtual time, or
/// the CLI inside a filesystem boundary.
///
/// # Errors
///
/// Returns a message when the profile has no script for the task, the
/// executor can't be bounded, or a file can't be written.
pub async fn with_profile(
    place: &Place<'_>,
    before: (&Input, &Report),
    support: Option<&crate::support::Report>,
    policy: Policy,
    profile: &Profile,
) -> Result<Repaired, String> {
    match profile {
        Profile::Scripted(name) => {
            let task = place.task.ok_or_else(|| {
                format!("the scripted repair profile {name} runs only on a mini-task")
            })?;
            let script = scripts::script(task, name)
                .ok_or_else(|| format!("no {name} repair script for {}", task.id))?;
            attempt(place, before, support, policy, |granted| {
                let mut executor = crate::scripted::Scripted::new(script, place.work.to_path_buf());
                executor.artifacts = Some(place.artifacts.to_path_buf());
                executor.deadline = granted;
                executor.recorder = place.recorder.clone();
                // The first session wrote delegate-1; this one is the second.
                executor.runs = 1;
                Ok(executor)
            })
            .await
        }
        Profile::Cli { agent, model } => {
            attempt(place, before, support, policy, |granted| {
                crate::minitask::run::bounded_cli(
                    *agent,
                    model,
                    place.work,
                    place.artifacts,
                    granted,
                    place.deadline.clone(),
                    place.recorder,
                    &crate::session::Controls::default(),
                    1,
                )
            })
            .await
        }
    }
}

fn write(dir: &Path, record: &Value) -> Result<(), String> {
    let text = serde_json::to_string_pretty(record).map_err(|e| e.to_string())?;
    crate::record::write_atomic(&dir.join(FILE), text.as_bytes())
}

/// Copies the directory tree at `from` to `to`, `.git` included: an
/// isolated copy of the same state for one repair arm.
///
/// # Errors
///
/// Returns a message when a file can't be read or written.
pub fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| format!("cannot create {}: {e}", to.display()))?;
    let mut stack: Vec<(PathBuf, PathBuf)> = vec![(from.to_path_buf(), to.to_path_buf())];
    while let Some((src, dst)) = stack.pop() {
        for entry in std::fs::read_dir(&src)
            .map_err(|e| format!("cannot read {}: {e}", src.display()))?
            .flatten()
        {
            let path = entry.path();
            let target = dst.join(entry.file_name());
            let kind = entry
                .file_type()
                .map_err(|e| format!("cannot stat {}: {e}", path.display()))?;
            if kind.is_dir() {
                std::fs::create_dir_all(&target)
                    .map_err(|e| format!("cannot create {}: {e}", target.display()))?;
                stack.push((path, target));
            } else if kind.is_symlink() {
                let link = std::fs::read_link(&path).map_err(|e| e.to_string())?;
                std::os::unix::fs::symlink(link, &target).map_err(|e| e.to_string())?;
            } else {
                std::fs::copy(&path, &target)
                    .map_err(|e| format!("cannot copy {}: {e}", path.display()))?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
