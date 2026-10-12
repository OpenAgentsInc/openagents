//! The queue pass in code: every open issue of a repository with a verdict
//! and the reason, so picking the next issue needs no chat turn. Verdicts
//! come from state the solver already reads: `main`'s log, the issue's
//! assignees, the landed-work ledger, and the issue's last update.

use std::path::Path;

use serde_json::{Value, json};

use super::hands::landed_commit;
use super::ledger;

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

pub async fn read(github: &str, repo: &Path, state: &Path) -> Result<Vec<Entry>, String> {
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
        let assignees: Vec<String> = item["assignees"]
            .as_array()
            .map(|list| {
                list.iter()
                    .filter_map(|a| a["login"].as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
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

pub async fn command(args: &[String]) -> Result<String, String> {
    let mut state = super::default_state();
    let mut repo = std::env::current_dir().map_err(|error| error.to_string())?;
    let mut github = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--state" => state = args.next().ok_or("--state needs a directory")?.into(),
            "--repo" => repo = args.next().ok_or("--repo needs a path")?.into(),
            "--github" => github = Some(args.next().ok_or("--github needs owner/repo")?.clone()),
            other => {
                return Err(format!(
                    "unknown option {other}; coder solver queue takes --github OWNER/REPO, --state DIR, and --repo PATH"
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
    let mut out = String::new();
    for entry in read(&github, &repo, &state).await? {
        out.push_str(&entry.json().to_string());
        out.push('\n');
    }
    Ok(out)
}
