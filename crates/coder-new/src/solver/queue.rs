//! The queue pass in code: every open issue of a repository with a verdict
//! and the reason, so picking the next issue needs no chat turn. Verdicts
//! come from state the solver already reads: `main`'s log, the issue's
//! assignees, the landed-work ledger, and the issue's last update.
//!
//! With `--decisions gateway|jev|clef`, every issue the facts leave `ready` or
//! `stale` is also asked, in one batched request at that door, whether it is
//! already on main, still about code that exists, in conflict with a
//! declared intent, and actionable now; the 21-day window is then only the
//! fallback when the door is off or does not answer. `--next` with a door
//! asks a Choice over the ready set (#11388).

use std::path::Path;

use serde_json::{Value, json};

use super::hands::landed_commit;
use super::ledger;
use crate::comms::{Channel, Intent};
use crate::issue_run::{Decider, decide};

/// Days since an issue's last update after which it is `stale`.
pub const RECENT_DAYS: i64 = 21;
/// A door's probability that the work already landed at or above which the
/// issue is `already_done`. Higher than the midpoint: skipping real work
/// costs more than one more look.
pub const LANDED_AT: f64 = 0.8;
/// A door's probability at or above which the other questions read as yes.
pub const YES_AT: f64 = 0.5;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub issue: u64,
    pub title: String,
    pub verdict: &'static str,
    pub why: String,
    /// The door's answers when a door judged the issue: the door, the model,
    /// and each question's probability.
    pub judged: Option<Value>,
}

impl Entry {
    #[must_use]
    pub fn json(&self) -> Value {
        let mut value = json!({
            "issue": self.issue,
            "title": self.title,
            "verdict": self.verdict,
            "why": self.why,
        });
        if let Some(judged) = &self.judged {
            value["judged"] = judged.clone();
        }
        value
    }
}

/// The verdict for one open issue from facts already in hand. `landed` is
/// the `main` commit that closes it, `assignees` who holds it,
/// `in_flight` the unlanded run ids that already work it, `age_days` the
/// days since its last update.
#[must_use]
pub fn judge(
    issue: u64,
    title: &str,
    landed: Option<&str>,
    assignees: &[String],
    in_flight: &[String],
    age_days: i64,
) -> Entry {
    let (verdict, why) = if let Some(commit) = landed {
        ("already_done", format!("main already has {commit}"))
    } else if !assignees.is_empty() {
        ("claimed", format!("held by {}", assignees.join(", ")))
    } else if !in_flight.is_empty() {
        (
            "in_flight",
            format!("unlanded run {}", in_flight.join(", ")),
        )
    } else if age_days > RECENT_DAYS {
        ("stale", format!("last updated {age_days} days ago"))
    } else {
        ("ready", String::new())
    };
    Entry {
        issue,
        title: title.to_owned(),
        verdict,
        why,
        judged: None,
    }
}

/// A door's answers on one issue: the probability that the work already
/// landed on main, that the issue is still about code that exists and not
/// superseded, that a coding agent can act on it now, and, when intents are
/// declared, that it conflicts with one.
#[derive(Clone, Debug, PartialEq)]
pub struct Judged {
    pub landed: f64,
    pub current: f64,
    pub actionable: f64,
    pub conflicts: Option<f64>,
    pub door: String,
    pub model: String,
}

impl Judged {
    fn json(&self) -> Value {
        json!({
            "door": self.door,
            "model": self.model,
            "landed": self.landed,
            "current": self.current,
            "actionable": self.actionable,
            "conflicts": self.conflicts,
        })
    }
}

/// The verdict once a door has answered. Only `ready` and `stale` (the
/// date's guess) are weighed; a fact from main, an assignee or the ledger
/// stands. A door saying the issue is still current overrides the 21-day
/// window.
#[must_use]
pub fn weigh(mut entry: Entry, judged: &Judged) -> Entry {
    if entry.verdict != "ready" && entry.verdict != "stale" {
        return entry;
    }
    let date_stale = entry.verdict == "stale";
    let (verdict, why) = if judged.landed >= LANDED_AT {
        (
            "already_done",
            format!("judged already on main (p={:.2})", judged.landed),
        )
    } else if judged.current < YES_AT {
        (
            "stale",
            format!(
                "judged replaced by later work or about code that no longer exists (p current={:.2})",
                judged.current
            ),
        )
    } else if judged.conflicts.is_some_and(|p| p >= YES_AT) {
        (
            "conflicting",
            format!(
                "judged to conflict with a declared intent (p={:.2})",
                judged.conflicts.unwrap_or_default()
            ),
        )
    } else if judged.actionable < YES_AT {
        (
            "blocked",
            format!(
                "judged not actionable now (p actionable={:.2})",
                judged.actionable
            ),
        )
    } else if date_stale {
        (
            "ready",
            format!(
                "{}; judged still current (p={:.2})",
                entry.why, judged.current
            ),
        )
    } else {
        ("ready", String::new())
    };
    entry.verdict = verdict;
    entry.why = why;
    entry.judged = Some(judged.json());
    entry
}

/// What a door reads about one issue: its text, its newest comments, the
/// newest subjects on main, and the declared intents.
#[must_use]
pub fn issue_state(item: &Value, main_log: &str, intents: &[Intent]) -> String {
    let body: String = item["body"]
        .as_str()
        .unwrap_or_default()
        .chars()
        .take(3000)
        .collect();
    let mut state = format!(
        "Issue #{}: {}\n\n{body}",
        item["number"].as_u64().unwrap_or_default(),
        item["title"].as_str().unwrap_or_default()
    );
    let comments: Vec<&Value> = item["comments"]
        .as_array()
        .map(|list| list.iter().rev().take(3).collect())
        .unwrap_or_default();
    if !comments.is_empty() {
        state.push_str("\n\nNewest comments, newest first:");
        for comment in comments {
            let text: String = comment["body"]
                .as_str()
                .unwrap_or_default()
                .chars()
                .take(800)
                .collect();
            state.push_str(&format!("\n- {text}"));
        }
    }
    if !main_log.trim().is_empty() {
        state.push_str("\n\nNewest commits on main:\n");
        state.push_str(main_log.trim());
    }
    if !intents.is_empty() {
        state.push_str("\n\nIntents other agents declared (member: files, crates, resources):");
        for intent in intents {
            state.push_str(&format!(
                "\n- {}: {} | {} | {}",
                intent.member,
                intent.files.join(", "),
                intent.crates.join(", "),
                intent.resources.join(", ")
            ));
        }
    }
    state
}

/// The batched questions for one issue.
fn questions(with_intents: bool) -> jev::Questions {
    let mut questions = jev::Questions::new()
        .with(
            "landed",
            jev::Noul::new(
                "Has the work this issue asks for already landed on main, per its comments and main's commits?",
            ),
        )
        .with(
            "current",
            jev::Noul::new(
                "Is this issue still about code that exists and not already replaced by later work?",
            ),
        )
        .with(
            "actionable",
            jev::Noul::new(
                "Can a coding agent act on this issue now, without waiting on an owner decision or another open issue?",
            ),
        );
    if with_intents {
        questions = questions.with(
            "conflicts",
            jev::Noul::new(
                "Would working this issue now collide with one of the declared intents (same files or same work)?",
            ),
        );
    }
    questions
}

/// Asks the door about one issue; `None` when it does not answer.
async fn ask(decider: Decider, state: String, with_intents: bool) -> Option<Judged> {
    let request = jev::SystemOneRequest::new(state, questions(with_intents));
    let (response, door) = decide::ask(decider, request).await.ok()?;
    let p = |id: &str| response.noul(id).ok().map(|answer| answer.noul);
    Some(Judged {
        landed: p("landed")?,
        current: p("current")?,
        actionable: p("actionable")?,
        conflicts: if with_intents { p("conflicts") } else { None },
        door,
        model: response.model.clone(),
    })
}

/// Which ready issue to take next, as a Choice at the door; the deterministic
/// pick when there is one ready issue or the door does not answer.
async fn pick(decider: Decider, entries: &[Entry]) -> Option<Value> {
    let fallback = next(entries)?;
    let ready: Vec<&Entry> = entries
        .iter()
        .filter(|entry| entry.verdict == "ready")
        .take(12)
        .collect();
    if decider == Decider::Off || ready.len() < 2 {
        return Some(fallback);
    }
    let mut choice = jev::Choice::default();
    for entry in &ready {
        choice = choice.option(entry.issue.to_string(), entry.title.clone());
    }
    choice.instructions = Some(
        "Which open issue should a coding agent take next: the most valuable work that is ready now?"
            .into(),
    );
    let state = ready
        .iter()
        .map(|entry| format!("#{}: {}", entry.issue, entry.title))
        .collect::<Vec<_>>()
        .join("\n");
    let request = jev::SystemOneRequest::new(state, jev::Questions::new().with("next", choice));
    let Ok((response, door)) = decide::ask(decider, request).await else {
        return Some(fallback);
    };
    let Ok(answer) = response.choice("next") else {
        return Some(fallback);
    };
    let Ok(issue) = answer.choice.parse::<u64>() else {
        return Some(fallback);
    };
    let title = ready
        .iter()
        .find(|entry| entry.issue == issue)
        .map(|entry| entry.title.clone())?;
    Some(json!({
        "next": issue,
        "title": title,
        "why": format!(
            "picked by {door} (p={:.2})",
            answer.probabilities.get(&answer.choice).copied().unwrap_or(0.0)
        ),
        "ready": fallback["ready"],
    }))
}

fn age_days(updated_at: &str) -> i64 {
    let Some(date) = updated_at.get(..10) else {
        return 0;
    };
    let mut parts = date.split('-').filter_map(|part| part.parse::<i64>().ok());
    let (Some(y), Some(m), Some(d)) = (parts.next(), parts.next(), parts.next()) else {
        return 0;
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
        / 86_400;
    now - days_from_civil(y, m, d)
}

// Howard Hinnant's days-from-civil, valid for the proleptic Gregorian calendar.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Which issue to take next from a queue when no door answers: the
/// longest-waiting `ready` issue (the lowest number), with the rest of the
/// ready set so a caller can see what was passed over. `None` when nothing
/// is ready.
#[must_use]
pub fn next(entries: &[Entry]) -> Option<Value> {
    let mut ready: Vec<&Entry> = entries
        .iter()
        .filter(|entry| entry.verdict == "ready")
        .collect();
    ready.sort_by_key(|entry| entry.issue);
    let first = ready.first()?;
    Some(json!({
        "next": first.issue,
        "title": first.title,
        "why": "longest-waiting ready issue",
        "ready": ready.iter().map(|entry| entry.issue).collect::<Vec<_>>(),
    }))
}

/// The channel members whose declared intent names `issue` as a
/// resource (`issue:N`), each marked as a channel claim.
#[must_use]
pub fn intended(intents: &[Intent], issue: u64) -> Vec<String> {
    let resource = format!("issue:{issue}");
    intents
        .iter()
        .filter(|intent| intent.resources.iter().any(|r| r == &resource))
        .map(|intent| format!("{} (agent channel)", intent.member))
        .collect()
}

fn channel_intents(channel: Option<&Path>) -> Result<Vec<Intent>, String> {
    match channel {
        Some(dir) => Channel::at(dir)?.intents(),
        None => Ok(Vec::new()),
    }
}

pub async fn read(
    github: &str,
    repo: &Path,
    state: &Path,
    channel: Option<&Path>,
    decider: Decider,
) -> Result<Vec<Entry>, String> {
    let intents = channel_intents(channel)?;
    let fields = if decider == Decider::Off {
        "number,title,assignees,updatedAt"
    } else {
        "number,title,assignees,updatedAt,body,comments"
    };
    let args = [
        "issue", "list", "-R", github, "--state", "open", "--limit", "200", "--json", fields,
    ];
    let text = crate::issue_run::setup::output("gh", &args, repo).await?;
    let main_log = if decider == Decider::Off {
        String::new()
    } else {
        main_log(repo).await
    };
    let issues: Vec<Value> = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    let rows = ledger::read(state, repo).await;
    let mut entries = Vec::new();
    for item in issues {
        let Some(issue) = item["number"].as_u64() else {
            continue;
        };
        let mut assignees: Vec<String> = item["assignees"]
            .as_array()
            .map(|list| {
                list.iter()
                    .filter_map(|a| a["login"].as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        assignees.extend(intended(&intents, issue));
        let in_flight: Vec<String> = rows
            .iter()
            .filter(|row| row.issue == issue && row.merged.is_none())
            .map(|row| row.run_id.clone())
            .collect();
        let landed = landed_commit(repo, issue).await;
        let mut entry = judge(
            issue,
            item["title"].as_str().unwrap_or_default(),
            landed.as_deref(),
            &assignees,
            &in_flight,
            age_days(item["updatedAt"].as_str().unwrap_or_default()),
        );
        if decider != Decider::Off && (entry.verdict == "ready" || entry.verdict == "stale") {
            let state = issue_state(&item, &main_log, &intents);
            if let Some(judged) = ask(decider, state, !intents.is_empty()).await {
                entry = weigh(entry, &judged);
            }
        }
        entries.push(entry);
    }
    entries.sort_by_key(|entry| (entry.verdict != "ready", entry.issue));
    Ok(entries)
}

/// The newest subjects on `origin/main`, for a door to read.
async fn main_log(repo: &Path) -> String {
    crate::issue_run::setup::output(
        "git",
        &["log", "--oneline", "-n", "40", "origin/main"],
        repo,
    )
    .await
    .unwrap_or_default()
}

/// The queue verdict for one issue, as `read` would judge it; `except`
/// is a run id to leave out of the in-flight set (the run asking). With a
/// door other than off, a `ready` or `stale` verdict is weighed by that
/// door's answers, as `coder solver queue --decisions` does, so the run's
/// summary.json carries the same judgment the queue would.
pub async fn verdict(
    github: &str,
    repo: &Path,
    state: &Path,
    issue: u64,
    except: &str,
    decider: Decider,
) -> Option<Entry> {
    let number = issue.to_string();
    let fields = if decider == Decider::Off {
        "number,title,assignees,updatedAt"
    } else {
        "number,title,assignees,updatedAt,body,comments"
    };
    let args = ["issue", "view", &number, "-R", github, "--json", fields];
    let text = crate::issue_run::setup::output("gh", &args, repo)
        .await
        .ok()?;
    let item: Value = serde_json::from_str(&text).ok()?;
    let assignees: Vec<String> = item["assignees"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|a| a["login"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    let in_flight: Vec<String> = ledger::read(state, repo)
        .await
        .into_iter()
        .filter(|row| row.issue == issue && row.merged.is_none() && row.run_id != except)
        .map(|row| row.run_id)
        .collect();
    let landed = landed_commit(repo, issue).await;
    let entry = judge(
        issue,
        item["title"].as_str().unwrap_or_default(),
        landed.as_deref(),
        &assignees,
        &in_flight,
        age_days(item["updatedAt"].as_str().unwrap_or_default()),
    );
    if decider == Decider::Off || (entry.verdict != "ready" && entry.verdict != "stale") {
        return Some(entry);
    }
    let state = issue_state(&item, &main_log(repo).await, &[]);
    Some(match ask(decider, state, false).await {
        Some(judged) => weigh(entry, &judged),
        None => entry,
    })
}

pub async fn command(args: &[String]) -> Result<String, String> {
    let mut state = super::default_state();
    let mut repo = std::env::current_dir().map_err(|error| error.to_string())?;
    let mut github = None;
    let mut pick = false;
    let mut channel = None;
    let mut decider = Decider::Off;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--next" => pick = true,
            "--channel" => {
                channel = Some(std::path::PathBuf::from(
                    args.next().ok_or("--channel needs a directory")?,
                ));
            }
            "--decisions" => {
                decider = match args.next().map(String::as_str) {
                    Some("gateway") => Decider::Gateway,
                    Some("jev") => Decider::Jev,
                    Some("clef") => Decider::Clef,
                    Some("off") => Decider::Off,
                    _ => return Err("--decisions takes gateway, jev, clef or off".into()),
                };
            }
            "--state" => state = args.next().ok_or("--state needs a directory")?.into(),
            "--repo" => repo = args.next().ok_or("--repo needs a path")?.into(),
            "--github" => github = Some(args.next().ok_or("--github needs owner/repo")?.clone()),
            other => {
                return Err(format!(
                    "unknown option {other}; coder solver queue takes --github OWNER/REPO, --state DIR, --repo PATH, --channel DIR, --decisions gateway|jev|clef|off, and --next"
                ));
            }
        }
    }
    let github = match github {
        Some(github) => github,
        None => crate::issue_run::setup::output(
            "gh",
            &[
                "repo",
                "view",
                "--json",
                "nameWithOwner",
                "-q",
                ".nameWithOwner",
            ],
            &repo,
        )
        .await?
        .trim()
        .to_owned(),
    };
    let entries = read(&github, &repo, &state, channel.as_deref(), decider).await?;
    if pick {
        return Ok(self::pick(decider, &entries)
            .await
            .map(|choice| format!("{choice}\n"))
            .unwrap_or_else(|| "{\"next\":null,\"why\":\"nothing is ready\"}\n".to_owned()));
    }
    let mut out = String::new();
    for entry in entries {
        out.push_str(&entry.json().to_string());
        out.push('\n');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_takes_the_lowest_ready_issue_and_lists_the_rest() {
        let entries = [
            judge(30, "c", None, &[], &[], 1),
            judge(10, "a", None, &["someone".into()], &[], 1),
            judge(20, "b", None, &[], &[], 1),
            judge(5, "stale", None, &[], &[], 40),
        ];
        let choice = next(&entries).expect("a ready issue");
        assert_eq!(choice["next"], json!(20));
        assert_eq!(choice["ready"], json!([20, 30]));
        assert!(next(&entries[1..2]).is_none());
    }

    #[test]
    fn a_declared_intent_on_an_issue_counts_as_a_claim() {
        let intents = [
            Intent::new("alice", vec![], vec!["issue:7".into()]),
            Intent::new("bob", vec!["src/a.rs".into()], vec![]),
        ];
        let held = intended(&intents, 7);
        assert_eq!(held, ["alice (agent channel)"]);
        assert_eq!(judge(7, "x", None, &held, &[], 1).verdict, "claimed");
        assert!(intended(&intents, 8).is_empty());
    }

    fn judged(landed: f64, current: f64, actionable: f64, conflicts: Option<f64>) -> Judged {
        Judged {
            landed,
            current,
            actionable,
            conflicts,
            door: "test".into(),
            model: "m".into(),
        }
    }

    #[test]
    fn a_door_turns_ready_and_stale_into_judged_verdicts() {
        let ready = || judge(1, "x", None, &[], &[], 1);
        assert_eq!(
            weigh(ready(), &judged(0.9, 0.9, 0.9, None)).verdict,
            "already_done"
        );
        assert_eq!(
            weigh(ready(), &judged(0.1, 0.2, 0.9, None)).verdict,
            "stale"
        );
        assert_eq!(
            weigh(ready(), &judged(0.1, 0.9, 0.9, Some(0.7))).verdict,
            "conflicting"
        );
        assert_eq!(
            weigh(ready(), &judged(0.1, 0.9, 0.2, None)).verdict,
            "blocked"
        );
        let kept = weigh(ready(), &judged(0.1, 0.9, 0.9, Some(0.1)));
        assert_eq!(kept.verdict, "ready");
        assert_eq!(kept.json()["judged"]["door"], json!("test"));
        // A door saying the issue is still current overrides the date window.
        let old = weigh(
            judge(2, "y", None, &[], &[], 40),
            &judged(0.1, 0.8, 0.9, None),
        );
        assert_eq!(old.verdict, "ready");
        assert!(old.why.contains("judged still current"));
        // Facts stand whatever the door says.
        let held = judge(3, "z", None, &["someone".into()], &[], 1);
        assert_eq!(weigh(held, &judged(0.9, 0.1, 0.1, None)).verdict, "claimed");
    }

    #[test]
    fn the_door_reads_the_issue_its_comments_main_and_the_intents() {
        let item = json!({
            "number": 9,
            "title": "Fix it",
            "body": "the body",
            "comments": [{"body": "old"}, {"body": "Landed in #12"}],
        });
        let intents = [Intent::new("alice", vec!["src/a.rs".into()], vec![])];
        let state = issue_state(&item, "abc solver: thing (#9)\n", &intents);
        assert!(state.starts_with("Issue #9: Fix it"));
        assert!(state.find("Landed in #12") < state.find("old"));
        assert!(state.contains("abc solver: thing (#9)"));
        assert!(state.contains("alice: src/a.rs"));
    }
}
