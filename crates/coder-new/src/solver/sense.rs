//! Issue Sense (#11484): System One judgments at the solver's turning
//! points, asked at the run's decision door (our hosted Clef pool by
//! default, keyless and free), so a cheap typed answer settles what would
//! otherwise cost a Claude session or a guess.
//!
//! - **Gate**: the queue's per-issue answers (landed, current, actionable)
//!   stop a run before any hand works when they are clear.
//! - **Failure triage**: each failing check is asked what caused it; a
//!   confident answer routes the hand-back without a Tester session.
//! - **Hunk review**: once the checks pass, each hunk of the change is
//!   asked whether it serves the issue; a clear stray hunk sends the Coder
//!   one round back naming it.
//! - **Landed watch** (`coder solver watch`): each new commit on main is
//!   asked whether it did the work of an open issue it touches.
//!
//! Every answer is a judgment row (#11387) in the run's summary, joined to
//! what the run came to.

use std::path::Path;

use serde_json::{Value, json};

use super::queue;
use crate::issue_run::{
    Decider, Transcript,
    decide::{self, Briefing, judgment},
    setup::{Base, output},
    verdict::{Outcome, Status},
};

/// The probability that the work already landed at or above which the gate
/// stops a run.
pub const GATE_LANDED: f64 = 0.8;
/// The probability that the issue is current, or actionable, below which
/// the gate stops a run. Far below the queue's midpoint: a stopped run is
/// work not done.
pub const GATE_NO: f64 = 0.2;
/// The probability at or above which a triage or review answer acts.
pub const SURE: f64 = 0.9;
/// The most hunks one review asks about.
pub const MAX_HUNKS: usize = 40;
/// How many questions run at once against the door.
const AT_ONCE: usize = 4;

/// The gate's outcome for a run whose issue the door judged: stopped when
/// the work clearly landed, the issue is clearly no longer current, or it
/// clearly cannot be acted on. `None` lets the run go on, as does an entry
/// no door judged.
#[must_use]
pub fn gate(entry: &queue::Entry) -> Option<Outcome> {
    let judged = entry.judged.as_ref()?;
    let p = |id: &str| judged[id].as_f64();
    if let Some(landed) = p("landed").filter(|p| *p >= GATE_LANDED) {
        return Some(Outcome::stopped(
            Status::AlreadyDone,
            format!(
                "{} judged the work already on main (p={landed:.2}); run with --no-gate to work it anyway.",
                judged["door"].as_str().unwrap_or("The door")
            ),
        ));
    }
    let why = match (p("current"), p("actionable")) {
        (Some(current), _) if current < GATE_NO => format!(
            "judged replaced by later work or about code that no longer exists (p current={current:.2})"
        ),
        (_, Some(actionable)) if actionable < GATE_NO => format!(
            "judged not actionable now: it waits on a decision or another issue (p actionable={actionable:.2})"
        ),
        _ => return None,
    };
    Some(Outcome::stopped(
        Status::Gated,
        format!(
            "{} {why}; run with --no-gate to work it anyway.",
            judged["door"].as_str().unwrap_or("The door")
        ),
    ))
}

/// The gate's judgment rows, one per question the door answered.
#[must_use]
pub fn gate_rows(entry: &queue::Entry) -> Vec<Value> {
    let Some(judged) = entry.judged.as_ref() else {
        return Vec::new();
    };
    let door = judged["door"].as_str().unwrap_or_default();
    let model = judged["model"].as_str().unwrap_or_default();
    [
        ("landed", GATE_LANDED, true),
        ("current", GATE_NO, false),
        ("actionable", GATE_NO, false),
    ]
    .iter()
    .filter_map(|(id, at, stops_above)| {
        let p = judged[*id].as_f64()?;
        let stops = if *stops_above { p >= *at } else { p < *at };
        Some(judgment(
            &format!("gate_{id}"),
            None,
            door,
            model,
            p,
            Some(*at),
            if stops { "stop" } else { "go" },
        ))
    })
    .collect()
}

/// What one failing check was judged to be.
#[derive(Clone, Debug, PartialEq)]
pub struct Triaged {
    pub check: String,
    /// `change`, `environment`, `flaky` or `unrelated`.
    pub cause: String,
    pub p: f64,
    /// The probability that the fix needs a file the briefing does not list.
    pub outside: f64,
}

/// Where triage sends a failed round, when it is sure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route {
    Coder(String),
    Briefer(String),
    /// The computer failed, not the change; the run stops.
    Environment(String),
}

const CAUSES: [(&str, &str); 4] = [
    (
        "change",
        "The change made in this run caused this failure: a compile error, a failing assertion or a broken test in code it touched",
    ),
    (
        "environment",
        "The computer or toolchain failed: a full disk, a dead linker, a network error, a missing tool",
    ),
    (
        "flaky",
        "A timing-dependent or flaky test that would likely pass on a rerun",
    ),
    (
        "unrelated",
        "A failure in code the change did not touch that was already there",
    ),
];

/// The route triage settles on, or `None` to ask the Tester. Sure means
/// every failing check was answered at or above [`SURE`]: one environment
/// failure stops the run; all caused by the change go to the Coder, or to
/// the Briefer when one needs a file outside the briefing.
#[must_use]
pub fn route(triaged: &[Triaged], failing: usize) -> Option<Route> {
    if triaged.is_empty() || triaged.len() < failing {
        return None;
    }
    if let Some(fault) = triaged
        .iter()
        .find(|t| t.cause == "environment" && t.p >= SURE)
    {
        return Some(Route::Environment(format!(
            "Clef judged {} a failure of the computer, not the change (p={:.2}).",
            fault.check, fault.p
        )));
    }
    if !triaged.iter().all(|t| t.cause == "change" && t.p >= SURE) {
        return None;
    }
    let checks = triaged
        .iter()
        .map(|t| t.check.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    if triaged.iter().any(|t| t.outside >= SURE) {
        return Some(Route::Briefer(format!(
            "Clef judged {checks} failed because of the change, and the fix needs a file the briefing does not list."
        )));
    }
    Some(Route::Coder(format!(
        "Clef judged {checks} failed because of the change; fix the code it names."
    )))
}

/// The tail of a check's output a question reads.
fn tail(text: &str, lines: usize, chars: usize) -> String {
    let kept: Vec<&str> = text.lines().rev().take(lines).collect();
    let joined = kept.into_iter().rev().collect::<Vec<_>>().join("\n");
    let skip = joined.chars().count().saturating_sub(chars);
    joined.chars().skip(skip).collect()
}

fn issue_text(base: &Base) -> String {
    let body: String = base.body.chars().take(3000).collect();
    format!("Issue #{}: {}\n\n{body}", base.issue, base.title)
}

/// Asks the door about each failing check. Returns what was answered and
/// adds a judgment row per answer.
pub async fn triage(
    decider: Decider,
    base: &Base,
    briefing: &Briefing,
    outputs: &[(String, bool, String)],
    rows: &mut Vec<Value>,
    transcript: &mut Transcript,
) -> Vec<Triaged> {
    let failing: Vec<&(String, bool, String)> = outputs.iter().filter(|(_, ok, _)| !ok).collect();
    if decider == Decider::Off || failing.is_empty() {
        return Vec::new();
    }
    let stat = output("git", &["diff", "--stat", &base.base], &base.worktree)
        .await
        .unwrap_or_default();
    let files = briefing.files.join("\n");
    let requests: Vec<(String, jev::SystemOneRequest)> = failing
        .iter()
        .map(|(id, _, text)| {
            let state = format!(
                "{}\n\nFiles the briefing lists:\n{files}\n\nWhat the change touched:\n{}\n\nFailing check: {id}\n\nIts output, last lines:\n{}",
                issue_text(base),
                tail(&stat, 30, 2000),
                tail(text, 60, 4000),
            );
            let mut cause = jev::Choice::default();
            for (name, about) in CAUSES {
                cause = cause.option(name, about);
            }
            cause.instructions = Some("What caused this check to fail?".into());
            let questions = jev::Questions::new().with("cause", cause).with(
                "outside",
                jev::Noul::new(
                    "Does fixing this failure need to change a file the briefing does not list?",
                ),
            );
            (id.clone(), jev::SystemOneRequest::new(state, questions))
        })
        .collect();
    let mut card = json!({
        "kind": "model",
        "title": "What made each check fail?",
        "question": "What caused this check to fail? Does the fix need a file outside the briefing?",
        "used_for": format!("At p ≥ {SURE} for every check the hand-back skips the Tester"),
        "files": failing.iter().map(|(id, _, _)| json!({"name": id})).collect::<Vec<_>>(),
    });
    let index = transcript.card(card.clone());
    let started = std::time::Instant::now();
    let answers = ask_all(decider, requests).await;
    let mut triaged = Vec::new();
    let mut shown = Vec::new();
    for (id, answer) in answers {
        let Ok((response, door)) = answer else {
            shown.push(json!({"name": id, "error": answer.err()}));
            continue;
        };
        let (Ok(cause), Ok(outside)) = (response.choice("cause"), response.noul("outside")) else {
            shown.push(json!({"name": id, "error": "the answer had no cause"}));
            continue;
        };
        let p = cause
            .probabilities
            .get(&cause.choice)
            .copied()
            .unwrap_or(0.0);
        rows.push(judgment(
            "failure_cause",
            Some(&id),
            &door,
            &response.model,
            p,
            Some(SURE),
            &cause.choice,
        ));
        rows.push(judgment(
            "failure_outside_briefing",
            Some(&id),
            &door,
            &response.model,
            outside.noul,
            Some(SURE),
            if outside.noul >= SURE {
                "rebrief"
            } else {
                "keep"
            },
        ));
        card["door"] = json!(door);
        shown.push(json!({"name": id, "p": p, "chosen": cause.choice, "outside": outside.noul}));
        triaged.push(Triaged {
            check: id,
            cause: cause.choice.clone(),
            p,
            outside: outside.noul,
        });
    }
    card["files"] = json!(shown);
    card["ms"] = json!(started.elapsed().as_millis() as u64);
    transcript.finish(index, card, json!({"ok": true}));
    triaged
}

/// One hunk of a unified diff.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hunk {
    pub file: String,
    /// The `@@ ... @@` line.
    pub header: String,
    pub text: String,
}

impl Hunk {
    /// How a hand-back names it.
    #[must_use]
    pub fn name(&self) -> String {
        format!("{} {}", self.file, self.header)
    }
}

/// The hunks of `diff`, each cut to 1,500 characters, at most
/// [`MAX_HUNKS`].
#[must_use]
pub fn hunks(diff: &str) -> Vec<Hunk> {
    let mut out: Vec<Hunk> = Vec::new();
    let mut file = String::new();
    for line in diff.lines() {
        if let Some(rest) = line.strip_prefix("diff --git a/") {
            file = rest
                .split_once(" b/")
                .map_or(rest, |(_, path)| path)
                .to_owned();
        } else if line.starts_with("@@") {
            out.push(Hunk {
                file: file.clone(),
                header: line
                    .split("@@")
                    .nth(1)
                    .map_or_else(|| line.to_owned(), |h| format!("@@{h}@@")),
                text: String::new(),
            });
        } else if let Some(hunk) = out.last_mut() {
            if !line.starts_with("+++") && !line.starts_with("---") && hunk.text.len() < 1500 {
                hunk.text.push_str(line);
                hunk.text.push('\n');
            }
        }
    }
    out.truncate(MAX_HUNKS);
    out
}

/// One hunk's review answers.
#[derive(Clone, Debug, PartialEq)]
pub struct Reviewed {
    pub hunk: Hunk,
    pub serves: f64,
    pub unrequested: f64,
    pub stray: f64,
}

impl Reviewed {
    /// Whether the hunk clearly does not belong in the change.
    #[must_use]
    pub fn flagged(&self) -> bool {
        self.stray >= SURE || (self.serves <= 1.0 - SURE && self.unrequested >= SURE)
    }
}

/// The hand-back a review asks for: the flagged hunks, named; `None` when
/// none is flagged.
#[must_use]
pub fn review_notes(reviewed: &[Reviewed]) -> Option<String> {
    let flagged: Vec<String> = reviewed
        .iter()
        .filter(|r| r.flagged())
        .map(|r| {
            format!(
                "- {} (serves the issue p={:.2}, unrequested change p={:.2}, stray p={:.2})",
                r.hunk.name(),
                r.serves,
                r.unrequested,
                r.stray
            )
        })
        .collect();
    (!flagged.is_empty()).then(|| {
        format!(
            "The checks pass, but Clef judged these hunks outside what the issue asks for. Remove each one, or keep it only if the fix needs it:\n{}",
            flagged.join("\n")
        )
    })
}

/// Asks the door about each hunk of the change.
pub async fn review(
    decider: Decider,
    base: &Base,
    diff: &str,
    rows: &mut Vec<Value>,
    transcript: &mut Transcript,
) -> Vec<Reviewed> {
    let hunks = hunks(diff);
    if decider == Decider::Off || hunks.is_empty() {
        return Vec::new();
    }
    let requests: Vec<(String, jev::SystemOneRequest)> = hunks
        .iter()
        .map(|hunk| {
            let state = format!(
                "{}\n\nOne hunk of the change made for this issue, in {}:\n{}\n{}",
                issue_text(base),
                hunk.file,
                hunk.header,
                hunk.text
            );
            let questions = jev::Questions::new()
                .with(
                    "serves",
                    jev::Noul::new(
                        "Does this hunk do work the issue asks for, or work that work needs (a test, an import, a mod line, a doc update)?",
                    ),
                )
                .with(
                    "unrequested",
                    jev::Noul::new("Does this hunk change behavior the issue did not ask to change?"),
                )
                .with(
                    "stray",
                    jev::Noul::new(
                        "Is this hunk leftover debug output, commented-out code, or an accidental edit?",
                    ),
                );
            (hunk.name(), jev::SystemOneRequest::new(state, questions))
        })
        .collect();
    let mut card = json!({
        "kind": "model",
        "title": "Does each hunk serve the issue?",
        "question": "Does this hunk serve the issue? Does it change unrequested behavior? Is it stray?",
        "used_for": format!("A hunk judged stray, or unrequested and not serving, at p ≥ {SURE} sends the Coder one round back"),
        "files": hunks.iter().map(|h| json!({"name": h.name()})).collect::<Vec<_>>(),
    });
    let index = transcript.card(card.clone());
    let started = std::time::Instant::now();
    let answers = ask_all(decider, requests).await;
    let mut reviewed = Vec::new();
    let mut shown = Vec::new();
    for ((name, answer), hunk) in answers.into_iter().zip(hunks) {
        let Ok((response, door)) = answer else {
            shown.push(json!({"name": name, "error": answer.err()}));
            continue;
        };
        let p = |id: &str| response.noul(id).ok().map(|a| a.noul);
        let (Some(serves), Some(unrequested), Some(stray)) =
            (p("serves"), p("unrequested"), p("stray"))
        else {
            shown.push(json!({"name": name, "error": "the answer was incomplete"}));
            continue;
        };
        let one = Reviewed {
            hunk,
            serves,
            unrequested,
            stray,
        };
        let flagged = one.flagged();
        for (id, value) in [
            ("serves", serves),
            ("unrequested", unrequested),
            ("stray", stray),
        ] {
            rows.push(judgment(
                &format!("hunk_{id}"),
                Some(&name),
                &door,
                &response.model,
                value,
                Some(SURE),
                if flagged { "flag" } else { "keep" },
            ));
        }
        card["door"] = json!(door);
        shown.push(json!({
            "name": name,
            "p": serves,
            "chosen": if flagged { "flag" } else { "keep" },
        }));
        reviewed.push(one);
    }
    card["files"] = json!(shown);
    card["ms"] = json!(started.elapsed().as_millis() as u64);
    transcript.finish(index, card, json!({"ok": true}));
    reviewed
}

/// Asks every request at the door, [`AT_ONCE`] at a time, in order. Each
/// answer carries the name of the door that gave it.
async fn ask_all(
    decider: Decider,
    requests: Vec<(String, jev::SystemOneRequest)>,
) -> Vec<(String, Result<(jev::SystemOneResponse, String), String>)> {
    use futures::StreamExt;
    futures::stream::iter(
        requests
            .into_iter()
            .map(|(name, request)| async move { (name, decide::ask(decider, request).await) }),
    )
    .buffered(AT_ONCE)
    .collect()
    .await
}

/// The judgment rows with what the run came to: its status, and for a row
/// about a file or a hunk, whether the final change touched that file.
#[must_use]
pub fn labeled(rows: &[Value], status: &Value, changed: &Value) -> Vec<Value> {
    let changed: Vec<&str> = changed
        .as_array()
        .map(|list| list.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    rows.iter()
        .cloned()
        .map(|mut row| {
            row["outcome"] = status.clone();
            let set = row["question_set"].as_str().unwrap_or_default();
            if set == "file_relevance" || set.starts_with("hunk_") {
                let file = row["subject"]
                    .as_str()
                    .unwrap_or_default()
                    .split(" @@")
                    .next()
                    .unwrap_or_default();
                row["in_change"] = json!(changed.contains(&file));
            }
            row
        })
        .collect()
}

/// `coder solver watch`: asks, for each commit on `origin/main` since the
/// last watch, whether it did the work of an open issue whose text names a
/// file or crate it touched. One JSON line per commit and issue asked.
pub async fn watch(args: &[String]) -> Result<String, String> {
    let mut repo = std::env::current_dir().map_err(|error| error.to_string())?;
    let mut state = super::default_state();
    let mut since: Option<String> = None;
    let mut decider = Decider::Gateway;
    let mut limit = 200usize;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().cloned().ok_or(format!("{name} needs a value"));
        match arg.as_str() {
            "--repo" => repo = value("--repo")?.into(),
            "--state" => state = value("--state")?.into(),
            "--since" => since = Some(value("--since")?),
            "--limit" => {
                limit = value("--limit")?
                    .parse()
                    .map_err(|_| "--limit takes a number".to_owned())?;
            }
            "--decisions" => decider = Decider::parse(&value("--decisions")?)?,
            other => {
                return Err(format!(
                    "unknown option {other}; coder solver watch takes --repo PATH, --state DIR, --since REV, --limit N and --decisions DOOR"
                ));
            }
        }
    }
    if decider == Decider::Off {
        return Err("coder solver watch needs a decision door".into());
    }
    let mark = state.join("watch-mark");
    let head = output("git", &["rev-parse", "origin/main"], &repo)
        .await?
        .trim()
        .to_owned();
    let since = since
        .or_else(|| {
            std::fs::read_to_string(&mark)
                .ok()
                .map(|s| s.trim().to_owned())
        })
        .unwrap_or_else(|| "origin/main~20".to_owned());
    let range = format!("{since}..{head}");
    let log = output("git", &["log", "--format=%H %s", &range], &repo).await?;
    let issues_json = output(
        "gh",
        &[
            "issue",
            "list",
            "--state",
            "open",
            "--limit",
            &limit.to_string(),
            "--json",
            "number,title,body",
        ],
        &repo,
    )
    .await?;
    let issues: Vec<Value> = serde_json::from_str(&issues_json).map_err(|e| e.to_string())?;
    let mut out = String::new();
    for line in log.lines() {
        let Some((sha, subject)) = line.split_once(' ') else {
            continue;
        };
        let files = output("git", &["show", "--name-only", "--format=", sha], &repo)
            .await
            .unwrap_or_default();
        let files: Vec<&str> = files.lines().filter(|l| !l.is_empty()).collect();
        let named = issue_numbers(subject);
        for issue in candidates(&issues, &files)
            .into_iter()
            .filter(|issue| !named.contains(&issue["number"].as_u64().unwrap_or_default()))
            .take(10)
        {
            let stat = output("git", &["show", "--stat", "--format=%s%n%n%b", sha], &repo)
                .await
                .unwrap_or_default();
            let state = format!(
                "Issue #{}: {}\n\n{}\n\nA commit on main:\n{}",
                issue["number"],
                issue["title"].as_str().unwrap_or_default(),
                issue["body"]
                    .as_str()
                    .unwrap_or_default()
                    .chars()
                    .take(3000)
                    .collect::<String>(),
                tail(&stat, 60, 3000),
            );
            let request = jev::SystemOneRequest::new(
                state,
                jev::Questions::new().with(
                    "landed",
                    jev::Noul::new("Does this commit do the work this issue asks for?"),
                ),
            );
            let Ok((response, door)) = decide::ask(decider, request).await else {
                continue;
            };
            let Ok(answer) = response.noul("landed") else {
                continue;
            };
            out.push_str(
                &json!({
                    "commit": &sha[..sha.len().min(10)],
                    "subject": subject,
                    "issue": issue["number"],
                    "title": issue["title"],
                    "landed": answer.noul,
                    "verdict": if answer.noul >= queue::LANDED_AT { "likely_landed" } else { "open" },
                    "door": door,
                })
                .to_string(),
            );
            out.push('\n');
        }
    }
    let _ = std::fs::create_dir_all(&state);
    let _ = std::fs::write(&mark, &head);
    Ok(out)
}

/// The issue numbers a commit subject names (`#123`); those are already
/// linked and need no question.
#[must_use]
pub fn issue_numbers(subject: &str) -> Vec<u64> {
    subject
        .split('#')
        .skip(1)
        .filter_map(|rest| {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            digits.parse().ok()
        })
        .collect()
}

/// The open issues whose text names one of `files` or its crate, most
/// mentions first.
#[must_use]
pub fn candidates<'a>(issues: &'a [Value], files: &[&str]) -> Vec<&'a Value> {
    let needles: Vec<String> = files
        .iter()
        .flat_map(|file| {
            let mut words = vec![(*file).to_owned()];
            if let Some(name) = Path::new(file).file_name().and_then(|n| n.to_str()) {
                if name.len() > 6 && name != "mod.rs" && name != "lib.rs" && name != "Cargo.toml" {
                    words.push(name.to_owned());
                }
            }
            if let Some(krate) = file
                .strip_prefix("crates/")
                .and_then(|r| r.split('/').next())
            {
                words.push(format!("crates/{krate}"));
            }
            words
        })
        .collect();
    let mut scored: Vec<(usize, &Value)> = issues
        .iter()
        .filter_map(|issue| {
            let text = format!(
                "{} {}",
                issue["title"].as_str().unwrap_or_default(),
                issue["body"].as_str().unwrap_or_default()
            );
            let hits = needles.iter().filter(|n| text.contains(n.as_str())).count();
            (hits > 0).then_some((hits, issue))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.into_iter().map(|(_, issue)| issue).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(judged: Option<Value>) -> queue::Entry {
        queue::Entry {
            issue: 7,
            title: "t".into(),
            verdict: "ready",
            why: String::new(),
            judged,
        }
    }

    #[test]
    fn the_gate_stops_only_on_clear_answers() {
        let at = |landed: f64, current: f64, actionable: f64| {
            gate(&entry(Some(json!({
                "door": "Clef", "model": "clef-flash",
                "landed": landed, "current": current, "actionable": actionable,
            }))))
            .map(|o| o.status)
        };
        assert_eq!(at(0.85, 0.9, 0.9), Some(Status::AlreadyDone));
        assert_eq!(at(0.1, 0.1, 0.9), Some(Status::Gated));
        assert_eq!(at(0.1, 0.9, 0.15), Some(Status::Gated));
        // The queue's midpoint is not enough to stop a run.
        assert_eq!(at(0.6, 0.4, 0.4), None);
        assert_eq!(gate(&entry(None)), None);
        let rows = gate_rows(&entry(Some(json!({
            "door": "Clef", "model": "m", "landed": 0.9, "current": 0.9, "actionable": 0.9
        }))));
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0]["decision"], "stop");
        assert_eq!(rows[1]["decision"], "go");
    }

    fn t(check: &str, cause: &str, p: f64, outside: f64) -> Triaged {
        Triaged {
            check: check.into(),
            cause: cause.into(),
            p,
            outside,
        }
    }

    #[test]
    fn triage_routes_only_when_every_failure_is_sure() {
        assert!(matches!(
            route(&[t("test:a", "change", 0.95, 0.1)], 1),
            Some(Route::Coder(_))
        ));
        assert!(matches!(
            route(&[t("test:a", "change", 0.95, 0.95)], 1),
            Some(Route::Briefer(_))
        ));
        assert!(matches!(
            route(
                &[
                    t("a", "change", 0.95, 0.1),
                    t("b", "environment", 0.97, 0.0)
                ],
                2
            ),
            Some(Route::Environment(_))
        ));
        // One unsure or one unanswered check asks the Tester.
        assert_eq!(
            route(
                &[t("a", "change", 0.95, 0.1), t("b", "change", 0.6, 0.1)],
                2
            ),
            None
        );
        assert_eq!(route(&[t("a", "change", 0.95, 0.1)], 2), None);
        assert_eq!(route(&[t("a", "flaky", 0.95, 0.1)], 1), None);
        assert_eq!(route(&[], 0), None);
    }

    const DIFF: &str = "diff --git a/src/a.rs b/src/a.rs\nindex 1..2 100644\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1,3 +1,4 @@ fn main()\n fn main() {\n+    dbg!(x);\n }\n@@ -10,2 +11,2 @@\n-old\n+new\ndiff --git a/README.md b/README.md\n--- a/README.md\n+++ b/README.md\n@@ -1 +1 @@\n-a\n+b\n";

    #[test]
    fn hunks_split_a_diff_by_file_and_header() {
        let hunks = hunks(DIFF);
        assert_eq!(hunks.len(), 3);
        assert_eq!(hunks[0].file, "src/a.rs");
        assert_eq!(hunks[0].header, "@@ -1,3 +1,4 @@");
        assert!(hunks[0].text.contains("dbg!"));
        assert_eq!(hunks[2].name(), "README.md @@ -1 +1 @@");
    }

    #[test]
    fn a_review_hands_back_only_clear_strays() {
        let hunk = hunks(DIFF).remove(0);
        let r = |serves, unrequested, stray| Reviewed {
            hunk: hunk.clone(),
            serves,
            unrequested,
            stray,
        };
        assert!(r(0.9, 0.1, 0.95).flagged());
        assert!(r(0.05, 0.95, 0.1).flagged());
        assert!(!r(0.5, 0.95, 0.5).flagged());
        assert_eq!(review_notes(&[r(0.9, 0.1, 0.1)]), None);
        let notes = review_notes(&[r(0.9, 0.1, 0.95)]).unwrap();
        assert!(notes.contains("src/a.rs @@ -1,3 +1,4 @@"), "{notes}");
    }

    #[test]
    fn rows_learn_whether_the_change_touched_their_file() {
        let rows = vec![
            json!({"question_set": "file_relevance", "subject": "src/a.rs"}),
            json!({"question_set": "file_relevance", "subject": "src/b.rs"}),
            json!({"question_set": "hunk_stray", "subject": "src/a.rs @@ -1 +1 @@"}),
            json!({"question_set": "issue_kind", "subject": null}),
        ];
        let out = labeled(&rows, &json!("passed"), &json!(["src/a.rs"]));
        assert_eq!(out[0]["in_change"], true);
        assert_eq!(out[1]["in_change"], false);
        assert_eq!(out[2]["in_change"], true);
        assert!(out[3].get("in_change").is_none());
        assert!(out.iter().all(|row| row["outcome"] == "passed"));
    }

    #[test]
    fn watch_candidates_are_issues_naming_a_touched_file_or_crate() {
        let issues = vec![
            json!({"number": 1, "title": "solver gate", "body": "see crates/coder-new/src/solver/queue.rs"}),
            json!({"number": 2, "title": "web", "body": "apps/web only"}),
            json!({"number": 3, "title": "coder-new", "body": "something in crates/coder-new"}),
        ];
        let found = candidates(&issues, &["crates/coder-new/src/solver/queue.rs"]);
        let numbers: Vec<u64> = found
            .iter()
            .map(|i| i["number"].as_u64().unwrap())
            .collect();
        assert_eq!(numbers, vec![1, 3]);
        assert_eq!(issue_numbers("Fix it (#11484, #12)"), vec![11484, 12]);
    }
}
