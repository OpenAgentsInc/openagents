//! Daily measurements for any saved agent, addressed by its definition ID.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::{Args, Output};
use serde_json::{Value, json};
#[path = "foreman_report.rs"]
mod report;

pub(crate) const USAGE: &str =
    "usage: openagents foreman report --agent ID --day YYYY-MM-DD [OPTIONS]
  report       Daily issue, stall, check-in, disk, probe, deploy, and owner wait measurements.
  --agent ID   Saved agent definition ID; never an agent name.
  --day DATE   Calendar day in America/Chicago. Times include CDT or CST.
  --repo OWNER/REPO       GitHub repository (default OpenAgentsInc/openagents).
  --issues N,N           Also include these issues, including claims from older days.
  --decision-log FILE    Read this agent's JSONL decisions and observations.
  --build-sweep-log FILE Read disk samples (default ~/.openagents/logs/build-sweep.log).
  --fixture FILE        Replay a recorded JSON input; reads no account or network.
  --json                Print the same report as one JSON object.
GitHub reads use gh, with a 20-second call timeout and a 120-second total budget.
Missing observations are reported as unknown; zero means zero recorded events.
Input and measurement definitions: docs/foreman/report.md.";

#[cfg(test)]
pub(crate) const EFFECTS: &[coder::cli_route::tree::Declared] =
    &[coder::cli_route::tree::Declared::computer(
        "report",
        coder::cli_route::tree::Effect::ReadOnly,
    )];

pub fn run(output: &Output, words: &[String]) -> u8 {
    if words
        .iter()
        .any(|w| matches!(w.as_str(), "--help" | "-h" | "help"))
    {
        println!("{USAGE}");
        return 0;
    }
    let args = match Args::parse(words.get(1..).unwrap_or_default(), &[]) {
        Ok(a) => a,
        Err(e) => return output.usage("foreman report", &e, USAGE),
    };
    if words.first().map(String::as_str) != Some("report") || !args.positional().is_empty() {
        return output.usage("foreman report", "expected report and its options", USAGE);
    }
    for key in args.option_names() {
        if ![
            "agent",
            "day",
            "repo",
            "issues",
            "decision-log",
            "build-sweep-log",
            "fixture",
        ]
        .contains(&key)
        {
            return output.usage("foreman report", &format!("unknown option --{key}"), USAGE);
        }
    }
    let (Some(agent), Some(day)) = (args.option("agent"), args.option("day")) else {
        return output.usage(
            "foreman report",
            "--agent ID and --day YYYY-MM-DD are required",
            USAGE,
        );
    };
    if agent.is_empty()
        || !agent
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
    {
        return output.usage(
            "foreman report",
            "agent ID must use letters, digits, underscores, or hyphens",
            USAGE,
        );
    }
    let window = match report::window(day) {
        Ok(w) => w,
        Err(e) => return output.usage("foreman report", &e, USAGE),
    };
    let result = (|| {
        let input = if let Some(path) = args.option("fixture") {
            serde_json::from_str::<report::Input>(&read(Path::new(path))?)
                .map_err(|e| format!("invalid fixture: {e}"))?
        } else {
            crate::runtime().block_on(collect(&args, agent, window.0, window.1))?
        };
        report::measure(&input, agent, day)
    })();
    match result {
        Ok(value) => {
            output.emit(&value, report::text);
            0
        }
        Err(e) => output.fail("foreman report", &e),
    }
}

fn read(path: &Path) -> Result<String, String> {
    use std::io::Read;
    let file =
        std::fs::File::open(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let mut text = String::new();
    file.take(16 * 1024 * 1024 + 1)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    if text.len() > 16 * 1024 * 1024 {
        return Err(format!("{} exceeds 16 MiB", path.display()));
    }
    Ok(text)
}

struct Github {
    repo: String,
    deadline: Instant,
}
impl Github {
    async fn get(&self, endpoint: &str) -> Result<Value, String> {
        let left = self.deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err("GitHub report exceeded its 120-second budget".into());
        }
        let job = supervise::Job::new("gh")
            .args(["api", endpoint])
            .bounded(
                supervise::Limits::within(left.min(Duration::from_secs(20)))
                    .keeping(8 * 1024 * 1024),
            )
            .run()
            .await;
        if !job.ending.success() || job.truncated() {
            return Err(format!(
                "GitHub read failed ({}) for {endpoint}: {}",
                job.ending, job.stderr.text
            ));
        }
        serde_json::from_str(&job.stdout.text).map_err(|e| format!("invalid GitHub response: {e}"))
    }
    async fn pages(&self, endpoint: &str) -> Result<Vec<Value>, String> {
        let mut all = Vec::new();
        for page in 1..=20 {
            let sep = if endpoint.contains('?') { '&' } else { '?' };
            let value = self
                .get(&format!("{endpoint}{sep}per_page=100&page={page}"))
                .await?;
            let rows = value.as_array().ok_or("expected a GitHub array")?;
            all.extend(rows.iter().cloned());
            if rows.len() < 100 {
                return Ok(all);
            }
        }
        Err(format!(
            "GitHub pagination limit reached for {endpoint}; narrow --issues"
        ))
    }
}

async fn collect(args: &Args, agent: &str, start: i64, end: i64) -> Result<report::Input, String> {
    let repo = args.option("repo").unwrap_or("OpenAgentsInc/openagents");
    let parts: Vec<_> = repo.split('/').collect();
    if parts.len() != 2
        || parts.iter().any(|p| {
            p.is_empty()
                || !p
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
        })
    {
        return Err("--repo must be OWNER/REPO".into());
    }
    let home = user_home::home_dir()?;
    let mut input = report::Input {
        captured_at: Some(jiff::Timestamp::now().to_string()),
        ..Default::default()
    };
    let default_log = home
        .join(".openagents/agents")
        .join(agent)
        .join("decisions.jsonl");
    let log = args
        .option("decision-log")
        .map(PathBuf::from)
        .unwrap_or(default_log);
    if log.exists() || args.option("decision-log").is_some() {
        for (line, row) in read(&log)?.lines().enumerate() {
            if row.trim().is_empty() {
                continue;
            }
            let mut event: Value = serde_json::from_str(row)
                .map_err(|e| format!("{}:{}: {e}", log.display(), line + 1))?;
            let object = event
                .as_object_mut()
                .ok_or_else(|| format!("{}:{}: expected a JSON object", log.display(), line + 1))?;
            object.entry("agent_id").or_insert_with(|| json!(agent));
            input.events.push(event);
        }
        input.sources.push(log.display().to_string());
    } else {
        input.notes.push("No decision log found; chat, check-in, probe, deploy, and owner wait coverage is unknown. Use --decision-log FILE.".into());
    }
    let sweep = args
        .option("build-sweep-log")
        .map(PathBuf::from)
        .unwrap_or(home.join(".openagents/logs/build-sweep.log"));
    if sweep.exists() || args.option("build-sweep-log").is_some() {
        input.build_sweep_log = read(&sweep)?;
        input.sources.push(sweep.display().to_string());
    } else {
        input
            .notes
            .push("No build-sweep log found; disk coverage is unknown.".into());
    }
    let gh = Github {
        repo: repo.into(),
        deadline: Instant::now() + Duration::from_secs(120),
    };
    let mut numbers = std::collections::BTreeSet::new();
    if let Some(ids) = args.option("issues") {
        for id in ids.split(',') {
            numbers.insert(
                id.parse::<u64>()
                    .map_err(|_| "--issues must be comma-separated issue numbers")?,
            );
        }
    }
    for event in &input.events {
        if event["agent_id"] == agent {
            if let Some(n) = event["issue"].as_u64() {
                numbers.insert(n);
            }
        }
    }
    // Updated issues include old issues closed today. Their claims decide attribution.
    let since = report::iso(start)?;
    if args.option("issues").is_none() {
        for issue in gh
            .pages(&format!(
                "repos/{}/issues?state=all&since={since}&sort=updated&direction=desc",
                gh.repo
            ))
            .await?
        {
            if issue.get("pull_request").is_none() {
                if let Some(n) = issue["number"].as_u64() {
                    numbers.insert(n);
                }
            }
        }
    }
    for n in numbers {
        let mut issue = gh.get(&format!("repos/{}/issues/{n}", gh.repo)).await?;
        issue["comments"] = json!(
            gh.pages(&format!("repos/{}/issues/{n}/comments", gh.repo))
                .await?
        );
        let explicit = args.option("issues").is_some()
            || input
                .events
                .iter()
                .any(|e| e["agent_id"] == agent && e["issue"] == n);
        if explicit || report::claimed_by(&issue, agent) {
            issue["agent_id"] = json!(agent);
            input.issues.push(issue);
        }
    }
    if !input.issues.is_empty() {
        // Committed dates preserve branch work when a merge kept the original commits.
        let oldest = input
            .issues
            .iter()
            .filter_map(|i| i["created_at"].as_str())
            .min()
            .unwrap_or(&since)
            .to_owned();
        input.commits = gh
            .pages(&format!(
                "repos/{}/commits?since={oldest}&until={}",
                gh.repo,
                report::iso(end)?
            ))
            .await?;
        input.notes.push("First commit uses the earliest referenced author date (committer date if absent), which survives a rebase. These are not push times. Code complete needs a ready observation or an issue branch; otherwise it is unknown.".into());
        let branches: std::collections::BTreeSet<String> = input
            .events
            .iter()
            .filter(|e| e["agent_id"] == agent)
            .filter_map(|e| e["branch"].as_str().map(str::to_owned))
            .collect();
        for branch in branches {
            // Percent-encode the ref as a query value, separate from the endpoint path.
            let encoded: String = branch.bytes().map(|b| format!("%{b:02X}")).collect();
            input.commits.extend(
                gh.pages(&format!(
                    "repos/{}/commits?sha={encoded}&since={oldest}&until={}",
                    gh.repo,
                    report::iso(end)?
                ))
                .await?,
            );
        }
    }
    input.sources.push(format!("GitHub: {repo}"));
    Ok(input)
}
