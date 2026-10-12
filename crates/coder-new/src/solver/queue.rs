//! The queue pass in code: every open issue of a repository with a verdict
//! and the reason, so picking the next issue needs no chat turn. Verdicts
//! come from state the solver already reads: `main`'s log, the issue's
//! assignees, the landed-work ledger, and the issue's last update.

use std::path::Path;

use serde_json::{Value, json};

use super::hands::landed_commit;
use super::ledger;
use crate::comms::{Channel, Intent};

/// Days since an issue's last update after which it is `stale`.
pub const RECENT_DAYS: i64 = 21;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub issue: u64,
    pub title: String,
    pub verdict: &'static str,
    pub why: String,
}

impl Entry {
    #[must_use]
    pub fn json(&self) -> Value {
        json!({
            "issue": self.issue,
            "title": self.title,
            "verdict": self.verdict,
            "why": self.why,
        })
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
    }
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
) -> Result<Vec<Entry>, String> {
    let intents = channel_intents(channel)?;
    let args = [
        "issue",
        "list",
        "-R",
        github,
        "--state",
        "open",
        "--limit",
        "200",
        "--json",
        "number,title,assignees,updatedAt",
    ];
    let text = crate::issue_run::setup::output("gh", &args, repo).await?;
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
        entries.push(judge(
            issue,
            item["title"].as_str().unwrap_or_default(),
            landed.as_deref(),
            &assignees,
            &in_flight,
            age_days(item["updatedAt"].as_str().unwrap_or_default()),
        ));
    }
    entries.sort_by_key(|entry| (entry.verdict != "ready", entry.issue));
    Ok(entries)
}

/// The queue verdict for one issue, as `read` would judge it; `except`
/// is a run id to leave out of the in-flight set (the run asking).
pub async fn verdict(
    github: &str,
    repo: &Path,
    state: &Path,
    issue: u64,
    except: &str,
) -> Option<Entry> {
    let number = issue.to_string();
    let args = [
        "issue",
        "view",
        &number,
        "-R",
        github,
        "--json",
        "number,title,assignees,updatedAt",
    ];
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
    Some(judge(
        issue,
        item["title"].as_str().unwrap_or_default(),
        landed.as_deref(),
        &assignees,
        &in_flight,
        age_days(item["updatedAt"].as_str().unwrap_or_default()),
    ))
}

pub async fn command(args: &[String]) -> Result<String, String> {
    let mut state = super::default_state();
    let mut repo = std::env::current_dir().map_err(|error| error.to_string())?;
    let mut github = None;
    let mut pick = false;
    let mut channel = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--next" => pick = true,
            "--channel" => {
                channel = Some(std::path::PathBuf::from(
                    args.next().ok_or("--channel needs a directory")?,
                ));
            }
            "--state" => state = args.next().ok_or("--state needs a directory")?.into(),
            "--repo" => repo = args.next().ok_or("--repo needs a path")?.into(),
            "--github" => github = Some(args.next().ok_or("--github needs owner/repo")?.clone()),
            other => {
                return Err(format!(
                    "unknown option {other}; coder solver queue takes --github OWNER/REPO, --state DIR, --repo PATH, --channel DIR, and --next"
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
    let entries = read(&github, &repo, &state, channel.as_deref()).await?;
    if pick {
        return Ok(next(&entries)
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
}
