//! `openagents chat work --on gce`: each issue's flow runs on a host of the
//! GCE pool this computer granted with `openagents cloud up` (issue
//! #10225; `crate::cloud`, runbook `docs/cloud/gce-pool.md`).
//!
//! This computer only orchestrates, as with `--on boat`:
//!
//! 1. the grant is checked: no pool record, or a revoked one, refuses; the
//!    run never moves to another computer;
//! 2. when the pool has fewer slots than `--parallel` asks, it grows within
//!    the grant's `--max-hosts` (two runs per host);
//! 3. each worker takes one slot of a host and takes issues from a
//!    queue; the run's credentials go into the script sent over ssh's
//!    standard input (never a command line, never the image), which the host
//!    saves as a private file and deletes as it starts;
//! 4. on the host the run takes a slot lock, builds `origin/main`'s
//!    `openagents` and `microcoder` on the warm target when the commit moved,
//!    and runs `openagents chat work --local --json --issues N`: the same
//!    issue flow as on this computer (claim, worktree, engine, checks, the
//!    multi-machine landing of #10226, comment, close). It runs in a session
//!    of its own, so a dropped connection is followed again from where it
//!    left off;
//! 5. events stream back and print as a local run's do, marked with the
//!    issue; at the end the run's wall time and its share of the host's
//!    list price go into an issue comment and a route record
//!    (`~/.openagents/cloud/runs.jsonl`: placement computer `gce`, the
//!    pool's operator grant).
//!
//! Lost hosts are replaced within the grant. Each preemption is recorded before
//! retrying, and replacement runs recover pushed work for the lost task.
//! Hosts are not deleted here: each deletes itself after its idle minutes,
//! and `openagents cloud down` deletes them all.

use std::collections::{BTreeMap, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use coder::task::issue_run::Land;
use openagents_chat::tool_groups::Stream;
use serde_json::{Value, json};

use super::boat::{Credentials, EngineLogins, Inner, inner};
use super::{Failure, event, failed};
use crate::cloud::{self, Host, Pool};
use crate::out::Output;

/// The most runs one queue holds at once (16 hosts).
pub(super) const MAX_PARALLEL: u64 = 32;

/// What `chat work --on gce` was asked to do.
pub(super) struct Request {
    pub repository: String,
    pub numbers: Vec<u64>,
    pub parallel: u64,
    pub land: Option<Land>,
    pub engine_fallback: bool,
}

fn nonce() -> String {
    format!(
        "{:x}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    )
}

/// The script one run's host runs: credentials as private variables (the
/// file holding them is deleted first thing), a slot lock, the CLI build,
/// and the issue flow with NDJSON events. Exit 75 means every slot of the
/// host was taken.
fn run_script(credentials: &Credentials, issue: u64, land: Option<Land>, slots: u64) -> String {
    let land = match land {
        Some(Land::Main) => " --land main",
        Some(Land::PullRequest) => " --land pr",
        Some(Land::Queue) => " --land queue",
        None => "",
    };
    let exports: String = credentials_lines(credentials);
    let last = slots.saturating_sub(1);
    format!(
        r#"set -uo pipefail
rm -f "$0"
d=$(dirname "$0")
{exports}{build}main() {{
  touch "$HOME/.oa-pool/busy"
  local slot=""
  for i in $(seq 0 {last}); do
    exec 9>"$HOME/.oa-pool/slot-$i.lock"
    if flock -n 9; then slot=$i; break; fi
    exec 9>&-
  done
  [ -n "$slot" ] || {{ echo "pool: every slot on $(hostname) is taken" >&2; return 75; }}
  echo "pool: slot $slot on $(hostname)" >&2
  [ -n "${{OA_GIT_NAME:-}}" ] && git config --global user.name "$OA_GIT_NAME"
  [ -n "${{OA_GIT_EMAIL:-}}" ] && git config --global user.email "$OA_GIT_EMAIL"
  unset OA_GIT_NAME OA_GIT_EMAIL
  gh auth setup-git >/dev/null 2>&1 || true
  # Coder gives Grok Build the XAI_API_KEY of the account's login shell,
  # not of this process: keep it in a private file the login shell reads.
  # The file goes with the host's disk when the host deletes itself.
  # An OpenAI key logs Codex in (API-key login, run as a lean codex exec
  # session, #10275); the login goes with the host's disk.
  if [ -n "${{OA_CODEX_API_KEY:-}}" ]; then
    printenv OA_CODEX_API_KEY | codex login --with-api-key >/dev/null 2>&1 \
      || echo "pool: codex login with the API key failed" >&2
  fi
  unset OA_CODEX_API_KEY
  # A ChatGPT login that cannot refresh (no refresh token): Codex runs on
  # it until its access token expires. Each run writes the newest copy; it
  # goes with the host's disk.
  if [ -n "${{OA_CODEX_AUTH:-}}" ]; then
    mkdir -p "$HOME/.codex" \
      && (umask 077; printf '%s' "$OA_CODEX_AUTH" | base64 -d >"$HOME/.codex/auth.json.$$" \
        && mv -f "$HOME/.codex/auth.json.$$" "$HOME/.codex/auth.json") \
      || echo "pool: the Codex login could not be written" >&2
  fi
  unset OA_CODEX_AUTH
  if [ -n "${{XAI_API_KEY:-}}" ]; then
    (umask 077; printf 'export XAI_API_KEY=%q\n' "$XAI_API_KEY" >"$HOME/.oa-pool/engine.env")
    for f in "$HOME/.profile" "$HOME/.bashrc"; do
      grep -q oa-pool/engine.env "$f" 2>/dev/null \
        || echo '[ -f "$HOME/.oa-pool/engine.env" ] && . "$HOME/.oa-pool/engine.env"' >>"$f"
    done
  fi
  oa_build || return $?
  cd "$HOME/openagents" || return 2
  OPENAGENTS_CODER_CONTROLLER=$HOME/.oa-pool/bin/microcoder OPENAGENTS_CODER_PLACEMENT=gce \
    "$HOME/.oa-pool/bin/openagents" chat work --local --json --issues {issue} --parallel 1{land}
}}
main; rc=$?
echo "$rc" >"$d/exit"; touch "$HOME/.oa-pool/busy"; exit "$rc"
"#,
        build = cloud::BUILD,
    )
}

/// `export NAME='value'` lines for the run's credentials.
fn credentials_lines(credentials: &Credentials) -> String {
    credentials
        .file()
        .lines()
        .map(|line| format!("export {line}\n"))
        .collect()
}

/// Cloud operations, injectable so tests can lose a host during an SSH stream.
trait Backend: Send + Sync {
    fn ssh(&self, project: &str, host: &Host, remote: &str) -> std::process::Command;
    fn hosts(&self, pool: &Pool) -> Result<Vec<Host>, String>;
    fn start(&self, pool: &Pool, output: Output) -> Result<Host, String>;
    fn granted(&self, pool: &Pool) -> Result<(), String>;
    fn record(&self, record: &Value) {
        append_record(record);
    }
    fn comment(&self, repository: &str, issue: u64, body: &str) {
        super::boat::comment(repository, issue, body);
    }
}

struct Live;
impl Backend for Live {
    fn ssh(&self, project: &str, host: &Host, remote: &str) -> std::process::Command {
        cloud::ssh(project, host, remote)
    }
    fn hosts(&self, pool: &Pool) -> Result<Vec<Host>, String> {
        cloud::list_hosts(&pool.project, Some(&pool.pool))
    }
    fn start(&self, pool: &Pool, output: Output) -> Result<Host, String> {
        let started = cloud::start_hosts(pool, 1, output)?
            .into_iter()
            .next()
            .ok_or("The replacement host did not start.")?;
        if let Some(error) = started.error {
            return Err(error);
        }
        started
            .host
            .ok_or_else(|| "The replacement host was not described.".into())
    }
    fn granted(&self, pool: &Pool) -> Result<(), String> {
        let current = Pool::granted()?;
        if current.grant != pool.grant || current.epoch != pool.epoch {
            return Err("The pool grant changed; no replacement run starts.".into());
        }
        Ok(())
    }
}

/// Serializes replacement provisioning in this queue so concurrent losses do
/// not each grow from the same host count.
fn replacement(
    backend: &dyn Backend,
    pool: &Pool,
    lost: &[String],
    output: Output,
    running: &Mutex<BTreeMap<String, Host>>,
    allocation: &Mutex<()>,
    stopping: &AtomicBool,
) -> Result<Host, String> {
    let _allocation = allocation.lock().unwrap_or_else(|e| e.into_inner());
    if stopping.load(Ordering::SeqCst) {
        return Err("Stopped before recovery.".into());
    }
    backend.granted(pool)?;
    let hosts = backend.hosts(pool)?;
    let active = running.lock().unwrap_or_else(|e| e.into_inner());
    let available = hosts
        .iter()
        .filter(|h| h.status == "RUNNING" && !lost.contains(&h.name))
        .filter(|h| {
            active.values().filter(|r| r.name == h.name).count()
                < pool.slots_per_host.max(1) as usize
        })
        .min_by_key(|h| active.values().filter(|r| r.name == h.name).count())
        .cloned();
    drop(active);
    if let Some(host) = available {
        return Ok(host);
    }
    if hosts
        .iter()
        .filter(|h| h.status != "TERMINATED" && !lost.contains(&h.name))
        .count() as u64
        >= pool.max_hosts
    {
        return Err("No replacement slot is free within the pool's --max-hosts grant.".into());
    }
    // start_hosts tries every spot zone, then on-demand capacity.
    backend.start(pool, output)
}

#[derive(Debug)]
enum FollowError {
    Lost {
        message: String,
        task: String,
        thread: String,
    },
    Failed(String),
}
impl From<String> for FollowError {
    fn from(message: String) -> Self {
        Self::Failed(message)
    }
}

/// What a finished (or failed) run reports.
struct Ended {
    outcome: String,
    message: String,
    commits: Vec<String>,
    task: String,
    thread: String,
}

/// Follows run `run` on `host` until it ends, reattaching when the
/// connection drops. `script` is sent on the first connection only.
#[allow(clippy::too_many_arguments)]
fn follow(
    backend: &dyn Backend,
    pool: &Pool,
    host: &Host,
    run: &str,
    script: &str,
    issue: u64,
    output: Output,
    stopping: &AtomicBool,
    running: &Mutex<BTreeMap<String, Host>>,
) -> Result<Ended, FollowError> {
    let project = &pool.project;
    let mut seen: u64 = 0;
    let mut tools = Stream::default();
    let mut errors: VecDeque<String> = VecDeque::new();
    let (mut task, mut thread) = (String::new(), String::new());
    let mut ended: Option<(String, String, Vec<String>)> = None;
    let mut exit: Option<String> = None;
    let mut attempts = 0;
    running
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(run.to_owned(), host.clone());
    while exit.is_none() {
        attempts += 1;
        let remote = if attempts == 1 {
            cloud::start_remote(run)
        } else {
            cloud::follow_remote(run, seen + 1)
        };
        let mut child = backend
            .ssh(project, host, &remote)
            .stdin(if attempts == 1 {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("ssh did not start: {e}"))?;
        if attempts == 1 {
            if let Some(mut stdin) = child.stdin.take() {
                // A host can disappear while stdin is being sent. Drain and
                // reap SSH, then check GCE rather than abandoning the child.
                let _ = stdin.write_all(script.as_bytes());
            }
        }
        let stderr = child.stderr.take();
        let ssh_errors = std::thread::spawn(move || {
            stderr.map_or_else(String::new, |stderr| {
                BufReader::new(stderr)
                    .lines()
                    .map_while(Result::ok)
                    .last()
                    .unwrap_or_default()
            })
        });
        if let Some(stdout) = child.stdout.take() {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(code) = line.strip_prefix("OA_POOL_END ") {
                    exit = Some(code.trim().to_owned());
                    continue;
                }
                if let Some(error) = line.strip_prefix("OA_POOL_ERR ") {
                    if !output.json() && error.starts_with("pool:") {
                        eprintln!("#{issue} {error}");
                    }
                    errors.push_back(error.to_owned());
                    if errors.len() > 30 {
                        errors.pop_front();
                    }
                    continue;
                }
                seen += 1;
                match inner(line.trim()) {
                    Inner::Started {
                        task: t,
                        thread: th,
                    } => {
                        super::boat::say(
                            output,
                            json!({"event": "coder", "issue": issue, "thread": th,
                                "accepted": true, "placement": cloud::COMPUTER,
                                "host": host.name,
                                "task": {"host": cloud::COMPUTER, "task": t, "issue": issue}}),
                            &format!(
                                "#{issue}: Coder took the issue on {} as task {t}.",
                                host.name
                            ),
                        );
                        (task, thread) = (t, th);
                    }
                    Inner::Done {
                        outcome,
                        message,
                        commits,
                    } => ended = Some((outcome, message, commits)),
                    Inner::Event(line) => super::work::show(&output, issue, &mut tools, &line),
                    Inner::Other => {}
                }
            }
        }
        let status = child.wait();
        let last_ssh = ssh_errors.join().unwrap_or_default();
        if exit.is_some() {
            break;
        }
        if stopping.load(Ordering::SeqCst) {
            return Err(FollowError::Failed(
                "Stopped: the flow on the host was killed.".into(),
            ));
        }
        // A failed listing is uncertainty: reconnect, never launch a duplicate.
        let alive = backend
            .hosts(pool)
            .map(|hosts| {
                hosts
                    .iter()
                    .any(|h| h.name == host.name && h.status == "RUNNING")
            })
            .unwrap_or(true);
        if !alive {
            return Err(FollowError::Lost {
                message: format!(
                    "{} is gone (preempted or deleted) while the run was going",
                    host.name
                ),
                task,
                thread,
            });
        }
        if attempts >= 6 {
            return Err(format!(
                "lost {} ({status:?}): {last_ssh}; the run may still be going there",
                host.name
            )
            .into());
        }
        std::thread::sleep(Duration::from_secs(5));
    }
    running
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(run);
    match ended {
        Some((outcome, message, commits)) => Ok(Ended {
            outcome,
            message,
            commits,
            task,
            thread,
        }),
        None => {
            let code = exit.unwrap_or_else(|| "unknown".into());
            if code == "75" {
                return Err(FollowError::Failed("busy".into()));
            }
            let tail = errors.into_iter().collect::<Vec<_>>().join("\n");
            Err(format!(
                "The flow on {} ended without an outcome (exit {code}).\n{tail}",
                host.name
            )
            .into())
        }
    }
}

/// The issue comment that records where the run ran and its estimated cost.
fn cost_comment(host: &Host, slots: u64, outcome: &str, wall: Duration, dollars: f64) -> String {
    format!(
        "Ran on the GCE pool (`openagents chat work --on gce`): host `{}` ({}, {}, {}), one of \
         {slots} slots.\n\n\
         - outcome: {outcome}\n\
         - wall time (start to end, from the orchestrator): {} s\n\
         - cost estimate at list price (the slot's share of the host): ${dollars:.4}\n",
        host.name,
        host.machine,
        if host.spot { "spot" } else { "on demand" },
        host.zone,
        wall.as_secs(),
    )
}

/// The route record for one run on the pool.
fn route_record(
    pool: &Pool,
    repository: &str,
    issue: u64,
    host: &Host,
    ended: &Ended,
    wall: Duration,
    dollars: f64,
) -> Value {
    let placement = super::placement::granted(
        super::placement::Target::Gce.computer(),
        pool.grant.clone(),
        pool.epoch,
        repository,
    );
    let run = super::placement::outcome(
        &ended.task,
        &ended.outcome,
        Some(super::placement::microusd(dollars)),
        u64::try_from(wall.as_millis()).ok(),
    );
    json!({
        "schema": "openagents.cloud.run.v1",
        "issue": issue,
        "pool": pool.pool,
        "host": host.name,
        "zone": host.zone,
        "spot": host.spot,
        "outcome": ended.outcome,
        "wall_seconds": wall.as_secs(),
        "placement": placement,
        "run": run,
    })
}

fn append_record(record: &Value) {
    let dir = std::env::var_os("OPENAGENTS_CLOUD_HOME").map_or_else(
        || {
            std::env::var_os("HOME")
                .map_or_else(|| std::path::PathBuf::from("."), std::path::PathBuf::from)
                .join(".openagents/cloud")
        },
        std::path::PathBuf::from,
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

/// One issue on one host, start to end. Returns the `issue` record, or
/// `None` when the host's slots were all taken (the issue goes back).
#[allow(clippy::too_many_arguments)]
fn run_issue(
    backend: &dyn Backend,
    allocation: &Mutex<()>,
    pool: &Pool,
    request: &Request,
    credentials: &Credentials,
    host: &mut Host,
    issue: u64,
    output: Output,
    stopping: &AtomicBool,
    running: &Mutex<BTreeMap<String, Host>>,
) -> Option<Value> {
    let began = Instant::now();
    let mut dollars = 0.0;
    let mut preemptions = Vec::new();
    let mut lost = Vec::new();
    let mut recover_task: Option<String> = None;
    let slots = pool.slots_per_host.max(1);
    let failure = |message: String| Ended {
        outcome: "failed".into(),
        message,
        commits: vec![],
        task: String::new(),
        thread: String::new(),
    };
    let ended = loop {
        if stopping.load(Ordering::SeqCst) {
            break Ended {
                outcome: "not_started".into(),
                ..failure("Stopped before it started.".into())
            };
        }
        if let Err(message) = backend.granted(pool) {
            break failure(message);
        }
        let run = format!("issue-{issue}-{}", nonce());
        super::boat::say(
            output,
            json!({"event": "gce_run", "issue": issue, "host": host.name, "zone": host.zone, "run": run}),
            &format!("#{issue}: on {} ({})", host.name, host.zone),
        );
        let mut script = run_script(credentials, issue, request.land, pool.slots_per_host);
        if let Some(task) = &recover_task {
            script = format!(
                "export OPENAGENTS_CODER_RECOVER_TASK={}\n{script}",
                boat::shell_quote(task.as_str())
            );
        }
        let attempt_began = Instant::now();
        let followed = follow(
            backend, pool, host, &run, &script, issue, output, stopping, running,
        );
        // Remove cancelled, failed, and lost runs too, not only clean endings.
        running
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&run);
        let wall = attempt_began.elapsed();
        let cost = cloud::hourly_usd(&host.machine, host.spot) * wall.as_secs_f64()
            / 3600.0
            / slots as f64;
        dollars += cost;
        match followed {
            Ok(ended) => break ended,
            Err(FollowError::Failed(message)) if message == "busy" && preemptions.is_empty() => {
                return None;
            }
            Err(FollowError::Failed(message)) => break failure(message),
            Err(FollowError::Lost {
                message,
                task,
                thread,
            }) => {
                let ended = Ended {
                    outcome: "preempted".into(),
                    message: message.clone(),
                    task: task.clone(),
                    thread,
                    commits: vec![],
                };
                let mut record =
                    route_record(pool, &request.repository, issue, host, &ended, wall, cost);
                record["run_id"] = json!(run);
                record["message"] = json!(message);
                record["lost_at"] = json!(
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map_or(0, |d| d.as_secs())
                );
                backend.record(&record);
                super::boat::say(
                    output,
                    json!({"event": "gce_preemption", "issue": issue, "record": record}),
                    &format!(
                        "#{issue}: {} was lost; resuming on another pool host.",
                        host.name
                    ),
                );
                preemptions.push(record);
                lost.push(host.name.clone());
                // Never override a claim unless its task came from this run's stream.
                if !task.is_empty() {
                    recover_task = Some(task);
                }
                if lost.len() >= 3 {
                    break failure("Three hosts were lost; recovery stopped.".into());
                }
                match replacement(backend, pool, &lost, output, running, allocation, stopping) {
                    Ok(next) => *host = next,
                    Err(message) => {
                        break failure(format!("The host was lost and recovery failed: {message}"));
                    }
                }
            }
        }
    };
    let wall = began.elapsed();
    let mut record = route_record(
        pool,
        &request.repository,
        issue,
        host,
        &ended,
        wall,
        dollars,
    );
    record["preemptions"] = json!(preemptions);
    backend.record(&record);
    event(&output, json!({"event": "route_record", "record": record}));
    if !matches!(ended.outcome.as_str(), "skipped" | "closed" | "not_started") {
        let mut body = cost_comment(host, slots, &ended.outcome, wall, dollars);
        if !preemptions.is_empty() {
            body.push_str(
                "\nWall time includes recovery; cost sums the run's share on each host.\n",
            );
            for loss in &preemptions {
                body.push_str(&format!(
                    "- Preempted or deleted host `{}` after {} s; task `{}`.\n",
                    loss["host"].as_str().unwrap_or_default(),
                    loss["wall_seconds"],
                    loss["run"]["task"].as_str().unwrap_or_default()
                ));
            }
        }
        backend.comment(&request.repository, issue, &body);
    }
    Some(
        json!({"event": "issue", "issue": issue, "outcome": ended.outcome,
        "message": ended.message, "placement": cloud::COMPUTER, "host": host.name,
        "zone": host.zone, "spot": host.spot, "wall_seconds": wall.as_secs(),
        "cost_usd": dollars, "commits": ended.commits, "preemptions": preemptions,
        "task": (!ended.task.is_empty()).then_some(ended.task),
        "thread": (!ended.thread.is_empty()).then_some(ended.thread)}),
    )
}

/// Which host each of `parallel` workers is pinned to: round-robin, so no
/// host gets more workers than it has slots.
fn assign(hosts: &[Host], parallel: u64, slots: u64) -> Vec<Host> {
    let mut seats = Vec::new();
    for _ in 0..slots {
        seats.extend(hosts.iter().cloned());
    }
    seats.truncate(usize::try_from(parallel).unwrap_or(usize::MAX));
    seats
}

/// `chat work --on gce`.
pub(super) async fn work(output: &Output, request: Request) -> Result<u8, Failure> {
    let output = *output;
    let pool = Pool::granted().map_err(failed)?;
    let engine_fallback = request.engine_fallback;
    let credentials = Arc::new(
        tokio::task::spawn_blocking(move || {
            super::boat::credentials(EngineLogins::ApiKeys, engine_fallback)
        })
        .await
        .map_err(|_| failed("the run credentials could not be read"))?
        .map_err(failed)?,
    );
    let project = pool.project.clone();
    let listed = {
        let project = project.clone();
        let name = pool.pool.clone();
        tokio::task::spawn_blocking(move || cloud::list_hosts(&project, Some(&name)))
            .await
            .map_err(|_| failed("the pool's hosts could not be listed"))?
            .map_err(failed)?
    };
    let mut hosts: Vec<Host> = listed
        .into_iter()
        .filter(|h| h.status == "RUNNING")
        .collect();
    let slots = pool.slots_per_host.max(1);
    let wanted = request.parallel.div_ceil(slots);
    if (hosts.len() as u64) < wanted {
        let grow = wanted
            .min(pool.max_hosts)
            .saturating_sub(hosts.len() as u64);
        if grow > 0 {
            if !output.json() {
                eprintln!(
                    "cloud: the pool has {} host(s); starting {grow} more for --parallel {} \
                     (the grant allows {})",
                    hosts.len(),
                    request.parallel,
                    pool.max_hosts
                );
            }
            let grown = {
                let pool = pool.clone();
                tokio::task::spawn_blocking(move || cloud::start_hosts(&pool, grow, output))
                    .await
                    .map_err(|_| failed("the pool could not grow"))?
                    .map_err(failed)?
            };
            hosts.extend(
                grown
                    .into_iter()
                    .filter(|s| s.error.is_none())
                    .filter_map(|s| s.host),
            );
        }
    }
    if hosts.is_empty() {
        return Err(failed(format!(
            "the GCE pool {} has no running host; `openagents cloud up --hosts N` starts some",
            pool.pool
        )));
    }
    let seats = assign(&hosts, request.parallel, slots);
    event(
        &output,
        json!({"event": "queue", "repository": request.repository, "issues": request.numbers,
            "parallel": seats.len(), "placement": cloud::COMPUTER, "pool": pool.pool,
            "hosts": hosts.iter().map(|h| &h.name).collect::<Vec<_>>()}),
    );
    if !output.json() {
        eprintln!(
            "Coder works {} issue(s) of {} on the GCE pool {} ({} host(s)), {} at a time: {}",
            request.numbers.len(),
            request.repository,
            pool.pool,
            hosts.len(),
            seats.len(),
            request
                .numbers
                .iter()
                .map(|n| format!("#{n}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    let queue = Arc::new(Mutex::new(
        request.numbers.iter().copied().collect::<VecDeque<u64>>(),
    ));
    let stopping = Arc::new(AtomicBool::new(false));
    let running: Arc<Mutex<BTreeMap<String, Host>>> = Arc::default();
    let request = Arc::new(request);
    let pool = Arc::new(pool);
    let mut workers = Vec::new();
    let allocation = Arc::new(Mutex::new(()));
    for mut host in seats {
        let allocation = Arc::clone(&allocation);
        let (queue, stopping, running, request, pool, credentials) = (
            Arc::clone(&queue),
            Arc::clone(&stopping),
            Arc::clone(&running),
            Arc::clone(&request),
            Arc::clone(&pool),
            Arc::clone(&credentials),
        );
        workers.push(tokio::task::spawn_blocking(move || {
            let mut records = Vec::new();
            loop {
                let next = queue.lock().ok().and_then(|mut q| q.pop_front());
                let Some(issue) = next else { break };
                match run_issue(
                    &Live,
                    &allocation,
                    &pool,
                    &request,
                    &credentials,
                    &mut host,
                    issue,
                    output,
                    &stopping,
                    &running,
                ) {
                    Some(record) => {
                        print_record(output, &record);
                        records.push(record);
                    }
                    None => {
                        // Another queue holds this host's slots: give the
                        // issue back and leave the host to the others.
                        if let Ok(mut q) = queue.lock() {
                            q.push_front(issue);
                        }
                        break;
                    }
                }
            }
            records
        }));
    }
    let all = futures_join(workers);
    tokio::pin!(all);
    let interrupt = tokio::signal::ctrl_c();
    tokio::pin!(interrupt);
    let mut interrupted = false;
    let mut results: Vec<Value> = loop {
        tokio::select! {
            records = &mut all => break records,
            _ = &mut interrupt, if !interrupted => {
                interrupted = true;
                stopping.store(true, Ordering::SeqCst);
                eprintln!("Stopping: no more runs start, and each running flow is killed on its host.");
                let runs: Vec<(String, Host)> = running
                    .lock()
                    .map(|r| r.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                    .unwrap_or_default();
                let project = project.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    for (run, host) in runs {
                        let _ = cloud::ssh_output(&project, &host, &cloud::kill_remote(&run));
                    }
                })
                .await;
            }
        }
    };
    // Issues no worker could start (every slot taken by another queue).
    let left: Vec<u64> = queue
        .lock()
        .map(|q| q.iter().copied().collect())
        .unwrap_or_default();
    for issue in left {
        let record = json!({"event": "issue", "issue": issue, "outcome": "not_started",
            "message": "Every slot of the pool was taken by another queue.",
            "placement": cloud::COMPUTER});
        print_record(output, &record);
        results.push(record);
    }
    let landed = results
        .iter()
        .filter(|r| matches!(r["outcome"].as_str(), Some("landed" | "pull_request")))
        .count();
    let total: f64 = results.iter().filter_map(|r| r["cost_usd"].as_f64()).sum();
    event(
        &output,
        json!({"event": "queue_done", "issues": results.len(), "landed": landed,
            "placement": cloud::COMPUTER, "cost_usd": total}),
    );
    if !output.json() {
        eprintln!(
            "Coder landed {landed} of {} issue(s) on the GCE pool; about ${total:.4} of host time. \
             Hosts delete themselves when idle; `openagents cloud down` deletes them now.",
            results.len()
        );
    }
    let good = results.iter().all(|r| {
        matches!(
            r["outcome"].as_str(),
            Some("landed" | "pull_request" | "queued" | "skipped" | "closed")
        )
    });
    Ok(if good { 0 } else { crate::EXIT_FAILURE })
}

/// Waits for every worker and gathers their records.
async fn futures_join(workers: Vec<tokio::task::JoinHandle<Vec<Value>>>) -> Vec<Value> {
    let mut all = Vec::new();
    for worker in workers {
        if let Ok(records) = worker.await {
            all.extend(records);
        }
    }
    all
}

fn print_record(output: Output, record: &Value) {
    event(&output, record.clone());
    if !output.json() {
        let cost = record["cost_usd"]
            .as_f64()
            .map_or_else(String::new, |d| format!(" About ${d:.4}."));
        println!(
            "#{}: {}. {}{cost} ({} s on {})",
            record["issue"],
            record["outcome"].as_str().unwrap_or("failed"),
            record["message"].as_str().unwrap_or_default(),
            record["wall_seconds"],
            record["host"].as_str().unwrap_or("no host"),
        );
        let _ = std::io::stdout().flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(name: &str) -> Host {
        Host {
            name: name.into(),
            zone: "us-central1-a".into(),
            status: "RUNNING".into(),
            machine: "c3-standard-8".into(),
            spot: true,
            created: String::new(),
            address: None,
        }
    }

    fn pool() -> Pool {
        Pool {
            schema: "openagents.cloud.pool.v1".into(),
            computer: "gce".into(),
            pool: "p1".into(),
            project: "test".into(),
            grant: "gce:p1".into(),
            epoch: 1,
            revoked_at: None,
            granted_at: 0,
            machine: "c3-standard-8".into(),
            spot: true,
            max_hosts: 1,
            idle_minutes: 10,
            slots_per_host: 2,
        }
    }

    struct FakePool {
        dir: tempfile::TempDir,
        existing: bool,
        started: AtomicBool,
        fail_start: bool,
        revoked: AtomicBool,
        records: Mutex<Vec<Value>>,
    }
    impl FakePool {
        fn new(existing: bool) -> Self {
            Self {
                dir: tempfile::tempdir().unwrap(),
                existing,
                started: AtomicBool::new(false),
                fail_start: false,
                revoked: AtomicBool::new(false),
                records: Mutex::new(vec![]),
            }
        }
    }
    impl Backend for FakePool {
        fn ssh(&self, _: &str, host: &Host, _: &str) -> std::process::Command {
            let path = self.dir.path().join(format!("{}.sh", host.name));
            let text = if host.name == "spot" {
                // The fake dies after publishing a task, without an end marker.
                r#"cat >/dev/null
printf '%s\n' '{"event":"coder","thread":"thread1","task":{"task":"losttask123"}}'
touch "$(dirname "$0")/dead"
exit 255
"#
            } else {
                r#"cat >"$(dirname "$0")/received"
printf '%s\n' '{"event":"coder","thread":"thread2","task":{"task":"newtask123"}}' '{"event":"issue","outcome":"landed","message":"Recovered","commits":["abc"]}' 'OA_POOL_END 0'
"#
            };
            std::fs::write(&path, text).unwrap();
            let mut command = std::process::Command::new("bash");
            command.arg(path);
            command
        }
        fn hosts(&self, _: &Pool) -> Result<Vec<Host>, String> {
            let mut hosts = vec![];
            if !self.dir.path().join("dead").exists() {
                hosts.push(host("spot"));
            }
            if self.existing || self.started.load(Ordering::SeqCst) {
                let mut next = host("replacement");
                next.spot = self.existing;
                hosts.push(next);
            }
            Ok(hosts)
        }
        fn start(&self, _: &Pool, _: Output) -> Result<Host, String> {
            assert!(self.dir.path().join("dead").exists());
            assert_eq!(
                self.records.lock().unwrap()[0]["outcome"],
                "preempted",
                "the loss is durable before provisioning"
            );
            if self.fail_start {
                return Err("No capacity".into());
            }
            self.started.store(true, Ordering::SeqCst);
            let mut next = host("replacement");
            next.spot = false;
            Ok(next)
        }
        fn granted(&self, _: &Pool) -> Result<(), String> {
            if self.revoked.load(Ordering::SeqCst) {
                Err("Grant revoked".into())
            } else {
                Ok(())
            }
        }
        fn record(&self, record: &Value) {
            self.records.lock().unwrap().push(record.clone());
        }
        fn comment(&self, _: &str, _: u64, _: &str) {}
    }

    #[test]
    fn a_fake_pool_kills_a_host_mid_run_and_resumes_on_another_host() {
        for existing in [true, false] {
            let fake = FakePool::new(existing);
            let mut current = host("spot");
            let running = Mutex::new(BTreeMap::new());
            let request = Request {
                repository: "test/repo".into(),
                numbers: vec![42],
                parallel: 1,
                land: Some(Land::Main),
                engine_fallback: false,
            };
            let result = run_issue(
                &fake,
                &Mutex::new(()),
                &pool(),
                &request,
                &Credentials::default(),
                &mut current,
                42,
                Output::new(true),
                &AtomicBool::new(false),
                &running,
            )
            .unwrap();
            assert_eq!(result["outcome"], "landed");
            assert_eq!(result["host"], "replacement");
            assert_eq!(result["spot"], existing);
            assert_eq!(fake.started.load(Ordering::SeqCst), !existing);
            assert!(running.lock().unwrap().is_empty());
            let received = std::fs::read_to_string(fake.dir.path().join("received")).unwrap();
            assert!(received.contains("export OPENAGENTS_CODER_RECOVER_TASK='losttask123'"));
            assert!(received.contains("--issues 42 --parallel 1 --land main"));
            let records = fake.records.lock().unwrap();
            assert_eq!(records.len(), 2);
            assert_eq!(records[0]["outcome"], "preempted");
            assert_eq!(records[0]["host"], "spot");
            assert_eq!(records[0]["run"]["task"], "losttask123");
            assert!(records[0]["lost_at"].as_u64().unwrap() > 0);
            assert_eq!(records[1]["preemptions"][0], records[0]);
            assert_eq!(records[1]["placement"]["grant"]["id"], "gce:p1");
        }
    }

    struct Reconnecting {
        calls: Mutex<Vec<String>>,
        listing_fails: bool,
    }
    impl Backend for Reconnecting {
        fn ssh(&self, _: &str, _: &Host, remote: &str) -> std::process::Command {
            let mut calls = self.calls.lock().unwrap();
            calls.push(remote.into());
            let mut command = std::process::Command::new("bash");
            command.args(["-c", if calls.len() == 1 {
                "cat >/dev/null; printf '%s\\n' '{\"event\":\"coder\",\"task\":{\"task\":\"original\"}}'; exit 255"
            } else {
                "printf '%s\\n' '{\"event\":\"issue\",\"outcome\":\"landed\"}' 'OA_POOL_END 0'"
            }]);
            command
        }
        fn hosts(&self, _: &Pool) -> Result<Vec<Host>, String> {
            if self.listing_fails {
                Err("GCE is unavailable".into())
            } else {
                Ok(vec![host("spot")])
            }
        }
        fn start(&self, _: &Pool, _: Output) -> Result<Host, String> {
            panic!("A disconnect alone must not provision a host")
        }
        fn granted(&self, _: &Pool) -> Result<(), String> {
            Ok(())
        }
        fn record(&self, _: &Value) {}
        fn comment(&self, _: &str, _: u64, _: &str) {}
    }

    #[test]
    fn a_disconnect_or_listing_failure_reattaches_without_restarting() {
        for listing_fails in [false, true] {
            let backend = Reconnecting {
                calls: Mutex::new(vec![]),
                listing_fails,
            };
            let result = follow(
                &backend,
                &pool(),
                &host("spot"),
                "r1",
                "script",
                42,
                Output::new(true),
                &AtomicBool::new(false),
                &Mutex::new(BTreeMap::new()),
            )
            .unwrap();
            assert_eq!(result.outcome, "landed");
            assert_eq!(result.task, "original");
            let calls = backend.calls.lock().unwrap();
            assert_eq!(calls.len(), 2);
            assert_eq!(calls[0], cloud::start_remote("r1"));
            assert_eq!(calls[1], cloud::follow_remote("r1", 2));
        }
    }

    #[test]
    fn a_failed_replacement_keeps_the_preemption_record() {
        let mut fake = FakePool::new(false);
        fake.fail_start = true;
        let request = Request {
            repository: "test/repo".into(),
            numbers: vec![42],
            parallel: 1,
            land: None,
            engine_fallback: false,
        };
        let result = run_issue(
            &fake,
            &Mutex::new(()),
            &pool(),
            &request,
            &Credentials::default(),
            &mut host("spot"),
            42,
            Output::new(true),
            &AtomicBool::new(false),
            &Mutex::new(BTreeMap::new()),
        )
        .unwrap();
        assert_eq!(result["outcome"], "failed");
        assert!(
            result["message"]
                .as_str()
                .unwrap()
                .contains("recovery failed")
        );
        assert_eq!(fake.records.lock().unwrap()[0]["outcome"], "preempted");
    }

    #[test]
    fn replacement_respects_stop_revocation_and_max_hosts() {
        let fake = FakePool::new(true);
        let running = Mutex::new(BTreeMap::from([
            ("r1".into(), host("replacement")),
            ("r2".into(), host("replacement")),
        ]));
        let choose = |stop| {
            replacement(
                &fake,
                &pool(),
                &["spot".into()],
                Output::new(true),
                &running,
                &Mutex::new(()),
                &AtomicBool::new(stop),
            )
        };
        assert!(choose(true).unwrap_err().contains("Stopped"));
        fake.revoked.store(true, Ordering::SeqCst);
        assert!(choose(false).unwrap_err().contains("revoked"));
        fake.revoked.store(false, Ordering::SeqCst);
        assert!(choose(false).unwrap_err().contains("--max-hosts"));
        assert!(!fake.started.load(Ordering::SeqCst));
    }

    #[test]
    fn workers_spread_over_hosts_and_never_exceed_their_slots() {
        let hosts = [host("a"), host("b")];
        let seats: Vec<String> = assign(&hosts, 3, 2).into_iter().map(|h| h.name).collect();
        assert_eq!(seats, ["a", "b", "a"]);
        assert_eq!(assign(&hosts, 9, 2).len(), 4);
        assert_eq!(assign(&hosts[..1], 1, 2).len(), 1);
    }

    #[test]
    fn the_run_script_deletes_itself_before_reading_on_and_locks_a_slot() {
        let mut credentials = Credentials::default();
        credentials
            .variables
            .insert("GH_TOKEN".into(), "gho_secret'$(x)".into());
        let script = run_script(&credentials, 10225, Some(Land::Main), 2);
        let removed = script.find("rm -f \"$0\"").unwrap();
        let exported = script
            .find("export GH_TOKEN='gho_secret'\\''$(x)'")
            .unwrap();
        assert!(removed < exported);
        assert!(script.contains("flock -n 9"));
        assert!(script.contains("$(seq 0 1)"));
        assert!(script.contains("--issues 10225 --parallel 1 --land main"));
        assert!(
            run_script(&credentials, 10225, Some(Land::Queue), 2)
                .contains("--issues 10225 --parallel 1 --land queue")
        );
        assert!(script.contains("OPENAGENTS_CODER_CONTROLLER"));
        assert!(script.contains("OPENAGENTS_CODER_PLACEMENT=gce"));
        assert!(script.contains("return 75"));
        // The remote command line carries no credential: only the script
        // sent on standard input does.
        assert!(!cloud::start_remote("r").contains("gho_"));
    }

    #[test]
    fn the_route_record_names_the_pool_grant() {
        let pool = Pool {
            schema: "openagents.cloud.pool.v1".into(),
            computer: "gce".into(),
            pool: "pabc123".into(),
            project: "p".into(),
            grant: "gce:pabc123".into(),
            epoch: 2,
            revoked_at: None,
            granted_at: 0,
            machine: "c3-standard-8".into(),
            spot: true,
            max_hosts: 8,
            idle_minutes: 10,
            slots_per_host: 2,
        };
        let ended = Ended {
            outcome: "landed".into(),
            message: String::new(),
            commits: vec![],
            task: "k1".into(),
            thread: String::new(),
        };
        let record = route_record(
            &pool,
            "o/r",
            7,
            &host("a"),
            &ended,
            Duration::from_secs(600),
            0.0123,
        );
        assert_eq!(record["placement"]["computer"], "gce");
        assert_eq!(record["placement"]["grant"]["id"], "gce:pabc123");
        assert_eq!(record["placement"]["grant"]["epoch"], 2);
        assert_eq!(record["placement"]["grant"]["source"], "operator");
        assert_eq!(record["run"]["cost_microusd"], 12_300);
        assert_eq!(record["run"]["projection"]["state"], "completed");
        let comment = cost_comment(&host("a"), 2, "landed", Duration::from_secs(600), 0.0123);
        assert!(comment.contains("600 s") && comment.contains("$0.0123"));
    }
}
