//! `openagents chat work --on archil`: each issue's flow runs in an Archil
//! sandbox of its own (#12136; `coder_cloud::archil_backend`), the same
//! briefed engine as `--on gce` (finder, briefed agent, verify, replayed
//! checks, Claude Code fallback).
//!
//! This computer orchestrates, and does the steps the sandbox's network
//! cannot reach (Archil's managed allowlist has github.com but not GitHub's
//! API, openagents.com or the landing queue's bucket):
//!
//! 1. typing `--on archil` is the grant; the cloud budget's daily cap and
//!    per-run cap hold as on the pool, at Archil's list price;
//! 2. the image is imported once and a template sandbox (origin/main, the
//!    briefed agent, a warm target) prepared once a day; each issue forks
//!    the template;
//! 3. this computer reads the issue and writes it, the engine's scripts as
//!    this binary carries them, and the run's credentials (as a private
//!    file the run deletes first thing) into the fork;
//! 4. the fork runs `scripts/work/work_issue.py --land push`: the engine's
//!    change, judged by its replayed checks, is pushed to a `land/archil-*`
//!    branch on github.com;
//! 5. this computer submits that branch to the landing queue
//!    (`openagents land submit --branch`), or opens the pull request, then
//!    stops and deletes the sandbox, comments the run's cost on the issue,
//!    and appends a route record (placement computer `archil`) to
//!    `~/.openagents/cloud/runs.jsonl`.

use std::collections::VecDeque;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use coder::task::issue_run::Land;
use coder_cloud::archil_backend as archil;
use serde_json::{Value, json};

use super::boat::{Credentials, EngineLogins};
use super::{Failure, event, failed};
use crate::out::Output;

/// The most sandboxes one queue runs at once.
pub(super) const MAX_PARALLEL: u64 = 16;

/// What `chat work --on archil` was asked to do.
pub(super) struct Request {
    pub repository: String,
    pub numbers: Vec<u64>,
    pub parallel: u64,
    pub land: Option<Land>,
    pub engine_fallback: bool,
    /// A briefed engine (`briefed:codex` by default).
    pub engine: String,
    /// This computer's checkout, where `land submit` runs.
    pub top: PathBuf,
}

fn nonce() -> String {
    format!(
        "{:x}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
            & 0xffff_ffff_ffff
    )
}

/// The issue as the driver reads it, through `gh` here.
fn issue_json(repository: &str, issue: u64) -> Result<String, String> {
    let output = std::process::Command::new("gh")
        .args([
            "issue",
            "view",
            &issue.to_string(),
            "--repo",
            repository,
            "--json",
            "number,title,body,state,url",
        ])
        .output()
        .map_err(|e| format!("gh did not run: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "gh could not read #{issue}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Lands the pushed `branch` the way `land` asks, from this computer.
/// Returns the driver's `landed` object for the result.
fn land_branch(
    land: Option<Land>,
    repository: &str,
    top: &std::path::Path,
    issue: u64,
    branch: &str,
    summary: &str,
    title: &str,
) -> Result<Value, String> {
    match land {
        Some(Land::PullRequest) => {
            let output = std::process::Command::new("gh")
                .args([
                    "pr",
                    "create",
                    "--repo",
                    repository,
                    "--head",
                    branch,
                    "--base",
                    "main",
                    "--title",
                    &format!("{title} (#{issue})"),
                    "--body",
                    &format!(
                        "Fixes #{issue}.\n\nWorked in an Archil sandbox ({summary}).\n\n\
                         🤖 Generated with [Claude Code](https://claude.com/claude-code)"
                    ),
                ])
                .output()
                .map_err(|e| format!("gh did not run: {e}"))?;
            if !output.status.success() {
                return Err(format!(
                    "the pull request could not be opened: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
            let url = String::from_utf8_lossy(&output.stdout)
                .lines()
                .last()
                .unwrap_or_default()
                .to_owned();
            Ok(json!({"how": "pr", "branch": branch, "url": url}))
        }
        _ => {
            let cli = std::env::current_exe().map_err(|e| format!("this CLI's path: {e}"))?;
            let output = std::process::Command::new(cli)
                .current_dir(top)
                .args([
                    "--json",
                    "land",
                    "submit",
                    "--branch",
                    branch,
                    "--issue",
                    &issue.to_string(),
                    "--summary",
                    summary,
                ])
                .output()
                .map_err(|e| format!("land submit did not run: {e}"))?;
            if !output.status.success() {
                return Err(format!(
                    "the landing queue refused it: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
            let entry = serde_json::from_slice::<Value>(&output.stdout)
                .ok()
                .map(|v| v["entry"].clone())
                .unwrap_or(Value::Null);
            Ok(json!({"how": "queue", "branch": branch, "entry": entry}))
        }
    }
}

/// The driver's last `{"type": "result"}` line, or a failed result saying
/// why there was none.
fn result_or(result: Option<Value>, code: i64, tail: &VecDeque<String>) -> Value {
    result.unwrap_or_else(|| {
        let tail = tail.iter().cloned().collect::<Vec<_>>().join("\n");
        json!({"type": "result", "ok": false, "landed": null, "checks": [],
            "error": format!("the run ended without a result (exit {code}).\n{tail}")})
    })
}

/// The issue comment that records where the run ran and its cost.
fn cost_comment(sandbox: &str, outcome: &str, wall: Duration, dollars: f64) -> String {
    format!(
        "Ran on Archil (`openagents chat work --on archil`): sandbox `{sandbox}` ({} vCPU, {} GiB), \
         forked from the warm template.\n\n\
         - outcome: {outcome}\n\
         - wall time (fork to delete, from the orchestrator): {} s\n\
         - cost estimate at Archil list price: ${dollars:.4}\n",
        archil::VCPUS,
        archil::MEM_MIB / 1024,
        wall.as_secs(),
    )
}

/// The route record for one run on Archil.
fn route_record(
    repository: &str,
    issue: u64,
    sandbox: &str,
    outcome: &str,
    task: &str,
    wall: Duration,
    dollars: f64,
) -> Value {
    let placement = super::placement::granted(
        super::placement::Target::Archil.computer(),
        format!("{}:operator", archil::COMPUTER),
        0,
        repository,
    );
    let run = super::placement::outcome(
        task,
        match outcome {
            "queued" => "landed",
            other => other,
        },
        Some(super::placement::microusd(dollars)),
        u64::try_from(wall.as_millis()).ok(),
    );
    json!({
        "schema": "openagents.cloud.run.v1",
        "issue": issue,
        "sandbox": sandbox,
        "machine": archil::machine(),
        "outcome": outcome,
        "wall_seconds": wall.as_secs(),
        "placement": placement,
        "run": run,
    })
}

fn append_record(record: &Value) {
    let dir = std::env::var_os("OPENAGENTS_CLOUD_HOME").map_or_else(
        || {
            user_home::home_os()
                .map_or_else(|| PathBuf::from("."), PathBuf::from)
                .join(".openagents/cloud")
        },
        PathBuf::from,
    );
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("runs.jsonl"))
    {
        let _ = writeln!(file, "{record}");
    }
}

/// The engine's scripts as this binary carries them, under the run dir.
async fn put_engine(sandbox: &archil::Sandbox) -> Result<(), String> {
    for (path, text) in super::work_briefed::SCRIPTS {
        let mode = if path.ends_with("work_issue.py") || path.ends_with("local-exec") {
            0o755
        } else {
            0o644
        };
        archil::put(
            sandbox,
            &format!("{}/engine/{path}", archil::RUN_DIR),
            text.as_bytes().to_vec(),
            mode,
        )
        .await?;
    }
    Ok(())
}

/// Shared by every worker of one queue.
struct Shared {
    client: archil::Archil,
    template: tokio::sync::Mutex<archil::Sandbox>,
    request: Request,
    credentials: Credentials,
    output: Output,
    queue: std::sync::Mutex<VecDeque<u64>>,
    /// Sandboxes alive now, by name, so Ctrl-C deletes them.
    live: std::sync::Mutex<Vec<String>>,
}

/// One issue in one sandbox, fork to delete. Returns the `issue` record.
async fn run_issue(shared: &Shared, issue: u64) -> Value {
    let output = shared.output;
    let request = &shared.request;
    let began = Instant::now();
    let name = format!("oa-issue-{issue}-{}", nonce());
    let failure = |message: String| json!({"type": "result", "ok": false, "landed": null, "checks": [], "error": message});
    if let Err(message) = coder_cloud::budget::admit_run() {
        let record = json!({"event": "issue", "issue": issue, "outcome": "not_started",
            "message": message, "placement": archil::COMPUTER});
        event(&output, record.clone());
        if !output.json() {
            println!("#{issue}: not_started. {message}");
        }
        return record;
    }
    let issue_text = match tokio::task::spawn_blocking({
        let repository = request.repository.clone();
        move || issue_json(&repository, issue)
    })
    .await
    .unwrap_or_else(|_| Err("gh did not run".into()))
    {
        Ok(text) => text,
        Err(message) => return super::work_briefed::finished(&output, issue, &failure(message)),
    };
    let title = serde_json::from_str::<Value>(&issue_text)
        .ok()
        .and_then(|v| v["title"].as_str().map(str::to_owned))
        .unwrap_or_default();
    let mut sandbox = {
        let mut template = shared.template.lock().await;
        match archil::fork(&mut template, &name).await {
            Ok(sandbox) => sandbox,
            Err(message) => {
                return super::work_briefed::finished(&output, issue, &failure(message));
            }
        }
    };
    if let Ok(mut live) = shared.live.lock() {
        live.push(sandbox.id.clone());
    }
    coder_cloud::budget::sandbox_started(
        &sandbox.id,
        &archil::machine(),
        coder_cloud::budget::agent().as_deref(),
    );
    super::boat::say(
        output,
        json!({"event": "archil_run", "issue": issue, "sandbox": sandbox.id, "name": name}),
        &format!("#{issue}: in Archil sandbox {name}"),
    );
    let run_id = nonce();
    let branch = format!("land/archil-{issue}-{run_id}");
    let script = archil::run_script(
        &request.repository,
        issue,
        &request.engine,
        &branch,
        &run_id,
    );
    let limit = coder_cloud::budget::Budget::load().run_limit(
        &coder_cloud::pool::Host {
            name: sandbox.id.clone(),
            zone: String::new(),
            status: "RUNNING".into(),
            machine: archil::machine(),
            spot: false,
            created: String::new(),
            address: None,
        },
        1,
    );
    let staged = async {
        archil::put(
            &sandbox,
            &format!("{}/env", archil::RUN_DIR),
            shared.credentials.file().into_bytes(),
            0o600,
        )
        .await?;
        archil::put(
            &sandbox,
            &format!("{}/issue.json", archil::RUN_DIR),
            issue_text.into_bytes(),
            0o644,
        )
        .await?;
        put_engine(&sandbox).await?;
        archil::put(
            &sandbox,
            &format!("{}/run.sh", archil::RUN_DIR),
            script.into_bytes(),
            0o644,
        )
        .await
    }
    .await;
    let mut result = None;
    let mut tail: VecDeque<String> = VecDeque::new();
    let code = match staged {
        Err(message) => {
            tail.push_back(message);
            -1
        }
        Ok(()) => {
            let path = format!("{}/run.sh", archil::RUN_DIR);
            let run = archil::run_lines(&sandbox, &path, limit, |stdout, line| {
                if !stdout {
                    if line.starts_with("archil:") && !output.json() {
                        eprintln!("#{issue} {line}");
                    }
                    tail.push_back(line.to_owned());
                    if tail.len() > 30 {
                        tail.pop_front();
                    }
                    return;
                }
                let Ok(row) = serde_json::from_str::<Value>(line) else {
                    return;
                };
                if row["type"] == "result" {
                    result = Some(row);
                } else {
                    super::work_briefed::shown(&output, issue, &row);
                }
            });
            match tokio::time::timeout(limit + Duration::from_secs(120), run).await {
                Ok(Ok(code)) => code,
                Ok(Err(message)) => {
                    tail.push_back(message);
                    -1
                }
                Err(_) => {
                    tail.push_back(format!(
                        "Stopped at the per-run cap: about ${:.2} of sandbox time after {} min \
                         (`openagents cloud budget --per-run USD` raises it).",
                        coder_cloud::budget::Budget::load().per_run_usd,
                        limit.as_secs() / 60
                    ));
                    -1
                }
            }
        }
    };
    archil::discard(&mut sandbox).await;
    coder_cloud::budget::sandbox_gone(&sandbox.id);
    if let Ok(mut live) = shared.live.lock() {
        live.retain(|id| *id != sandbox.id);
    }
    let mut result = result_or(result, code, &tail);
    // The sandbox pushed the change; this computer lands it.
    if result["ok"] == true
        && result["landed"]["how"] == "push"
        && let Some(pushed) = result["landed"]["branch"].as_str().map(str::to_owned)
    {
        let summary = format!(
            "{} engine in Archil, run {run_id}",
            result["engine"].as_str().unwrap_or("briefed")
        );
        let landed = tokio::task::spawn_blocking({
            let (land, repository, top) = (
                request.land,
                request.repository.clone(),
                request.top.clone(),
            );
            let (pushed, title) = (pushed.clone(), title.clone());
            move || land_branch(land, &repository, &top, issue, &pushed, &summary, &title)
        })
        .await
        .unwrap_or_else(|_| Err("land submit did not run".into()));
        match landed {
            Ok(landed) => result["landed"] = landed,
            Err(message) => {
                result["ok"] = json!(false);
                result["error"] = json!(format!("pushed {pushed}, but {message}"));
            }
        }
    }
    let wall = began.elapsed();
    let dollars =
        archil::hourly_usd(&archil::machine()).unwrap_or(0.0) * wall.as_secs_f64() / 3600.0;
    let mut record = super::work_briefed::finished(&output, issue, &result);
    let outcome = record["outcome"].as_str().unwrap_or("failed").to_owned();
    record["placement"] = json!(archil::COMPUTER);
    record["sandbox"] = json!(sandbox.id);
    record["wall_seconds"] = json!(wall.as_secs());
    record["cost_usd"] = json!(dollars);
    let route = route_record(
        &request.repository,
        issue,
        &sandbox.id,
        &outcome,
        result["run_id"].as_str().unwrap_or(&run_id),
        wall,
        dollars,
    );
    append_record(&route);
    event(&output, json!({"event": "route_record", "record": route}));
    let body = cost_comment(&name, &outcome, wall, dollars);
    let repository = request.repository.clone();
    let _ =
        tokio::task::spawn_blocking(move || super::boat::comment(&repository, issue, &body)).await;
    record
}

/// `chat work --on archil`.
pub(super) async fn work(output: &Output, request: Request) -> Result<u8, Failure> {
    let output = *output;
    let engine_fallback = request.engine_fallback;
    let mut credentials = tokio::task::spawn_blocking(move || {
        super::boat::credentials(EngineLogins::ApiKeys, engine_fallback)
    })
    .await
    .map_err(|_| failed("the run credentials could not be read"))?
    .map_err(failed)?;
    // Codex on a ChatGPT login cannot reach chatgpt.com through Archil's
    // managed allowlist; api.openai.com is on it, so --engine-fallback
    // runs Codex on the OpenAI API key instead.
    if engine_fallback
        && request.engine == "briefed:codex"
        && let Some(key) = tokio::task::spawn_blocking(super::boat::openai_api_key)
            .await
            .ok()
            .flatten()
    {
        credentials.variables.remove("OA_CODEX_AUTH");
        credentials.variables.insert("OA_CODEX_API_KEY".into(), key);
    }
    if let Some(key) = archil::openrouter_key() {
        credentials
            .variables
            .insert("OPENROUTER_API_KEY".into(), key);
    }
    // Claude Code in the sandbox: the fallback the pool's hosts lack.
    if let Some(token) = archil::claude_token() {
        credentials
            .variables
            .insert("CLAUDE_CODE_OAUTH_TOKEN".into(), token);
    } else if !output.json() {
        eprintln!(
            "archil: no Claude Code token (CLAUDE_CODE_OAUTH_TOKEN or OA_CLAUDE_TOKEN_FILE); \
             a failed briefed run has no Claude Code fallback"
        );
    }
    let client = archil::client().map_err(failed)?;
    let say = |text: &str| {
        if !output.json() {
            eprintln!("archil: {}", text.trim_start_matches("archil: "));
        }
    };
    let image = archil::image(&client).await.map_err(failed)?;
    let template = archil::template(&client, &image, &request.repository, &say)
        .await
        .map_err(failed)?;
    let parallel = request.parallel.min(request.numbers.len() as u64).max(1);
    event(
        &output,
        json!({"event": "queue", "repository": request.repository, "issues": request.numbers,
            "parallel": parallel, "placement": archil::COMPUTER, "engine": request.engine,
            "template": template.id}),
    );
    if !output.json() {
        eprintln!(
            "The briefed agent works {} issue(s) of {} in Archil sandboxes, {} at a time: {}",
            request.numbers.len(),
            request.repository,
            parallel,
            request
                .numbers
                .iter()
                .map(|n| format!("#{n}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    let shared = Arc::new(Shared {
        client,
        template: tokio::sync::Mutex::new(template),
        queue: std::sync::Mutex::new(request.numbers.iter().copied().collect()),
        request,
        credentials,
        output,
        live: std::sync::Mutex::new(Vec::new()),
    });
    let mut workers = tokio::task::JoinSet::new();
    for _ in 0..parallel {
        let shared = Arc::clone(&shared);
        workers.spawn(async move {
            let mut records = Vec::new();
            loop {
                let next = shared.queue.lock().ok().and_then(|mut q| q.pop_front());
                let Some(issue) = next else { break };
                records.push(run_issue(&shared, issue).await);
            }
            records
        });
    }
    let mut results: Vec<Value> = Vec::new();
    let interrupt = tokio::signal::ctrl_c();
    tokio::pin!(interrupt);
    loop {
        tokio::select! {
            joined = workers.join_next() => match joined {
                Some(Ok(records)) => results.extend(records),
                Some(Err(_)) => {}
                None => break,
            },
            _ = &mut interrupt => {
                eprintln!("Stopping: each running sandbox is stopped and deleted.");
                if let Ok(mut queue) = shared.queue.lock() {
                    queue.clear();
                }
                workers.abort_all();
                let live: Vec<String> = shared.live.lock().map(|l| l.clone()).unwrap_or_default();
                for id in live {
                    if let Ok(mut sandbox) = shared.client.sandboxes().get(&id).await {
                        archil::discard(&mut sandbox).await;
                    }
                    coder_cloud::budget::sandbox_gone(&id);
                }
                return Ok(crate::EXIT_FAILURE);
            }
        }
    }
    let landed = results
        .iter()
        .filter(|r| matches!(r["outcome"].as_str(), Some("queued" | "pull_request")))
        .count();
    let total: f64 = results.iter().filter_map(|r| r["cost_usd"].as_f64()).sum();
    event(
        &output,
        json!({"event": "queue_done", "issues": results.len(), "landed": landed,
            "placement": archil::COMPUTER, "cost_usd": total}),
    );
    if !output.json() {
        eprintln!(
            "Archil sandboxes handed {landed} of {} issue(s) to landing; about ${total:.4} of sandbox time.",
            results.len()
        );
    }
    let good = results
        .iter()
        .all(|r| matches!(r["outcome"].as_str(), Some("queued" | "pull_request")));
    Ok(if good { 0 } else { crate::EXIT_FAILURE })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_without_a_result_fails_with_its_last_lines() {
        let tail = VecDeque::from(["archil: git fetch failed".to_owned()]);
        let result = result_or(None, 2, &tail);
        assert_eq!(result["ok"], false);
        let error = result["error"].as_str().unwrap();
        assert!(error.contains("exit 2") && error.contains("git fetch failed"));
        let given = json!({"type": "result", "ok": true});
        assert_eq!(result_or(Some(given.clone()), 0, &tail), given);
    }

    #[test]
    fn the_route_record_places_the_run_on_archil() {
        let record = route_record(
            "o/r",
            7,
            "sb1",
            "queued",
            "run1",
            Duration::from_secs(600),
            0.1267,
        );
        assert_eq!(record["placement"]["computer"], "archil");
        assert_eq!(record["placement"]["grant"]["id"], "archil:operator");
        assert_eq!(record["run"]["projection"]["state"], "completed");
        assert_eq!(record["run"]["cost_microusd"], 126_700);
        assert_eq!(record["machine"], archil::machine());
        let comment = cost_comment("oa-issue-7-x", "queued", Duration::from_secs(600), 0.1267);
        assert!(comment.contains("600 s") && comment.contains("$0.1267"));
    }
}
