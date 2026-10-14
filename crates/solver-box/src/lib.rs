//! The solver box runner (#11560, spec `docs/cloud/solver-on-gcp.md`).
//!
//! One run takes one GitHub issue to a pull request on a Google Cloud box of
//! its own: [`run`] asks our Box service (oa-boat, through the `boat` SDK)
//! for a `large` box, writes the run's settings and credentials to it
//! through the files API (on tmpfs, mode 600, never in the box's env),
//! starts the solver container there (`deploy/solver-box`), follows its
//! output as [`Progress`] lines, rewrites the Vertex AI token every 20
//! minutes while it runs, reads the run's result, and deletes the box
//! however the run ends.
//!
//! The website side (claiming solver work runs and reporting their
//! progress and result) is the dispatcher's, #11562: it calls [`run`] per
//! claimed run and posts each [`Progress`] and the result [`Value`], which
//! is already in the work-run result shape (`diff`, `landed.url`, `checks`,
//! `cost_usd`, `secs`, `error`).

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use boat::models::{
    CommandParams, CommandRequest, CommandResponseBody, CreateParams, CreateSandboxRequest,
    DeleteSandboxParams, UsageParams,
};
use boat::{CommandFrame, Nullable, WaitOptions, shell_quote};
use serde_json::{Value, json};

/// The solver container, built by `deploy/solver-box/cloudbuild.yaml`.
pub const IMAGE: &str =
    "us-central1-docker.pkg.dev/openagentsgemini/openagents/oa-solver-box:latest";
/// Every hand's model unless the run names another.
pub const DEFAULT_MODEL: &str = "google/gemini-4-argon";
/// The box size: n2d-standard-8, 32 GB (`crates/oa-boat/src/sizes.rs`).
pub const SIZE: &str = "large";
/// The box's TTL: a work run is cancelled after 6 hours anyway.
pub const TTL_SECONDS: i64 = 6 * 3600;
/// How often the Vertex AI token on the box is rewritten (a token lives an
/// hour).
pub const VERTEX_REFRESH: Duration = Duration::from_secs(20 * 60);
/// Where the container reads its env file and the Vertex AI token.
pub const RUN_DIR: &str = "/run/oa-solver";
/// Where the box's own script reads the registry token.
pub const HOST_DIR: &str = "/run/oa-solver-host";
const SCRIPT_PATH: &str = "/tmp/oa-solver-box.sh";
const BOX_SCRIPT: &str = include_str!("box-run.sh");

/// One run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
    /// `OWNER/NAME`.
    pub repo: String,
    pub issue: u64,
    pub model: String,
    pub image: String,
    pub git_name: Option<String>,
    pub git_email: Option<String>,
}

impl Run {
    /// A run of `repo`'s issue with the default model and image.
    #[must_use]
    pub fn new(repo: &str, issue: u64) -> Self {
        Self {
            repo: repo.to_owned(),
            issue,
            model: DEFAULT_MODEL.to_owned(),
            image: IMAGE.to_owned(),
            git_name: None,
            git_email: None,
        }
    }
}

/// The run's credentials. Never printed: `Debug` names only which are set.
#[derive(Clone, Default)]
pub struct Credentials {
    /// Clones the repository, reads the issue, pushes, opens the pull
    /// request.
    pub github_token: String,
    /// An OpenAgents sign-in key: the Coder loop reaches Argon through the
    /// OpenAgents gateway with it until its Vertex door (#11562) reads the
    /// Vertex AI token file instead.
    pub openagents_token: Option<String>,
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("github_token", &!self.github_token.is_empty())
            .field("openagents_token", &self.openagents_token.is_some())
            .finish()
    }
}

/// `OWNER/NAME#N`.
#[must_use]
pub fn parse_issue(text: &str) -> Option<(String, u64)> {
    let (repo, number) = text.trim().split_once('#')?;
    let (owner, name) = repo.split_once('/')?;
    let fine = |part: &str| {
        !part.is_empty()
            && part
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    };
    if !fine(owner) || !fine(name) {
        return None;
    }
    let number = number.parse::<u64>().ok().filter(|n| *n > 0)?;
    Some((repo.to_owned(), number))
}

/// The env file the container sources and deletes (`solver-box-run`):
/// `KEY='value'` lines, every value shell-quoted.
#[must_use]
pub fn env_file(run: &Run, credentials: &Credentials) -> String {
    let mut vars: BTreeMap<&str, String> = BTreeMap::new();
    vars.insert("OA_REPO", run.repo.clone());
    vars.insert("OA_ISSUE", run.issue.to_string());
    vars.insert("OA_SOLVER_MODEL", run.model.clone());
    vars.insert("GH_TOKEN", credentials.github_token.clone());
    vars.insert("CODER_VERTEX_TOKEN_FILE", format!("{RUN_DIR}/vertex-token"));
    vars.insert("CODER_VERTEX_PROJECT", "openagentsgemini".into());
    vars.insert("CODER_VERTEX_LOCATION", "global".into());
    if let Some(name) = &run.git_name {
        vars.insert("OA_GIT_NAME", name.clone());
    }
    if let Some(email) = &run.git_email {
        vars.insert("OA_GIT_EMAIL", email.clone());
    }
    if let Some(token) = &credentials.openagents_token {
        vars.insert("OPENAGENTS_TOKEN", token.clone());
    }
    vars.iter()
        .map(|(key, value)| format!("{key}={}\n", shell_quote(value)))
        .collect()
}

/// Where Google access tokens come from: the registry pull and Vertex AI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GoogleToken {
    /// `gcloud auth print-access-token` (honours `CLOUDSDK_CONFIG`).
    Gcloud,
    /// The GCE / Cloud Run metadata server.
    Metadata,
}

impl GoogleToken {
    /// A fresh access token.
    ///
    /// # Errors
    /// The token could not be had; the text never holds a token.
    pub async fn get(&self) -> Result<String, String> {
        match self {
            Self::Gcloud => {
                let out = tokio::process::Command::new("gcloud")
                    .args(["auth", "print-access-token"])
                    .output()
                    .await
                    .map_err(|e| format!("gcloud did not run: {e}"))?;
                let token = String::from_utf8_lossy(&out.stdout).trim().to_owned();
                if out.status.success() && !token.is_empty() {
                    Ok(token)
                } else {
                    Err("gcloud gave no access token".into())
                }
            }
            Self::Metadata => {
                let answer: Value = reqwest::Client::new()
                    .get("http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token")
                    .header("Metadata-Flavor", "Google")
                    .send()
                    .await
                    .map_err(|e| format!("the metadata server did not answer: {e}"))?
                    .json()
                    .await
                    .map_err(|e| format!("the metadata server's answer was not read: {e}"))?;
                answer["access_token"]
                    .as_str()
                    .filter(|t| !t.is_empty())
                    .map(str::to_owned)
                    .ok_or_else(|| "the metadata server gave no access token".into())
            }
        }
    }
}

/// One progress line for the chat: `kind` is one of `box` (the computer
/// and the container), `stage` (the run's and each hand's task), `briefing`
/// (the solver's steps: the issue, the briefing, the checks), `thinking`,
/// `tool`, `checks`, `pr`, `cost`, `notice` (a step's error, the run's end).
#[derive(Clone, Debug, PartialEq)]
pub struct Progress {
    pub kind: &'static str,
    pub text: String,
    /// The line's own fields (a tool's input, a check's state).
    pub data: Value,
}

impl Progress {
    fn new(kind: &'static str, text: impl Into<String>) -> Self {
        Self {
            kind,
            text: text.into(),
            data: Value::Null,
        }
    }

    /// The line as JSON.
    #[must_use]
    pub fn json(&self) -> Value {
        json!({"kind": self.kind, "text": self.text, "data": self.data})
    }
}

/// Text cut to `max` characters, on one line.
fn short(text: &str, max: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= max {
        flat
    } else {
        let mut cut: String = flat.chars().take(max.saturating_sub(1)).collect();
        cut.push('…');
        cut
    }
}

/// Turns the container's output lines into progress lines. The solver
/// rewrites a transcript entry as it grows (`{"set": i, "entry": ...}`),
/// so a thought is reported once its entry is followed by another, and a
/// tool once, when it starts.
#[derive(Default)]
pub struct Reader {
    thought: Option<(u64, String)>,
    tools: std::collections::BTreeSet<u64>,
    failed: std::collections::BTreeSet<u64>,
    /// The container's last line, `{"type": "solver_box_summary", ...}`.
    pub summary: Option<Value>,
    /// Lines that were not JSON (the container's stderr, say), last 40.
    pub other: Vec<String>,
}

impl Reader {
    /// The progress lines `line` makes.
    pub fn line(&mut self, line: &str) -> Vec<Progress> {
        let line = line.trim();
        if line.is_empty() {
            return Vec::new();
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            self.other.push(short(line, 400));
            if self.other.len() > 40 {
                self.other.remove(0);
            }
            return Vec::new();
        };
        let mut out = Vec::new();
        match value["type"].as_str() {
            Some("solver_box_step") => {
                let text = value["text"].as_str().unwrap_or_default();
                out.push(Progress {
                    data: json!({"step": value["step"], "state": value["state"]}),
                    ..Progress::new("box", text)
                });
            }
            Some("solver_box_summary") => {
                self.flush(&mut out);
                out.extend(summary_lines(&value));
                self.summary = Some(value);
            }
            _ => {
                if let Some(index) = value["set"].as_u64() {
                    let entry = &value["entry"];
                    if let Some(text) = entry["assistant"].as_str() {
                        if self.thought.as_ref().is_some_and(|(i, _)| *i != index) {
                            self.flush(&mut out);
                        }
                        self.thought = Some((index, text.to_owned()));
                    } else {
                        self.flush(&mut out);
                        if let Some(text) = entry["user"].as_str() {
                            out.push(Progress::new("stage", short(text, 300)));
                        } else if let Some(name) = entry["tool"].as_str() {
                            if self.tools.insert(index) {
                                out.push(tool_line(name, &entry["input"]));
                            }
                            if entry["running"] != true
                                && let Some(why) = entry["output"]["error"].as_str()
                                && self.failed.insert(index)
                            {
                                out.push(Progress::new("notice", short(why, 600)));
                            }
                        }
                    }
                } else if let Some(text) = value["notice"].as_str() {
                    self.flush(&mut out);
                    out.push(Progress::new("notice", short(text, 300)));
                }
            }
        }
        out
    }

    /// The thought still held, at the end of the output.
    pub fn finish(&mut self) -> Vec<Progress> {
        let mut out = Vec::new();
        self.flush(&mut out);
        out
    }

    fn flush(&mut self, out: &mut Vec<Progress>) {
        if let Some((_, text)) = self.thought.take()
            && !text.trim().is_empty()
        {
            out.push(Progress::new("thinking", short(&text, 600)));
        }
    }
}

/// A tool entry's line. The solver's own steps are cards (`Decision`,
/// with a `title` and a `detail`): the issue, the briefing, the checks.
fn tool_line(name: &str, input: &Value) -> Progress {
    if name == "Decision" {
        let title = input["title"].as_str().unwrap_or("A step");
        let text = match input["detail"].as_str() {
            Some(detail) if !detail.is_empty() => format!("{title}: {}", short(detail, 200)),
            _ => title.to_owned(),
        };
        return Progress {
            data: json!({"card": input["kind"]}),
            ..Progress::new("briefing", text)
        };
    }
    let what = input
        .as_object()
        .and_then(|fields| {
            ["command", "path", "pattern", "query", "file_path", "url"]
                .iter()
                .find_map(|key| fields.get(*key).and_then(Value::as_str))
        })
        .map(|arg| format!("{name} {}", short(arg, 200)))
        .unwrap_or_else(|| name.to_owned());
    Progress {
        data: json!({"tool": name}),
        ..Progress::new("tool", what)
    }
}

/// The checks, pull request and cost lines of the container's summary.
fn summary_lines(summary: &Value) -> Vec<Progress> {
    let mut out = Vec::new();
    let checks = checks(summary);
    if !checks.is_empty() {
        let passed = checks.iter().filter(|c| c["ok"] == true).count();
        out.push(Progress {
            data: json!(checks),
            ..Progress::new(
                "checks",
                format!("{passed} of {} checks passed", checks.len()),
            )
        });
    }
    if let Some(url) = summary["pr"].as_str() {
        out.push(Progress::new("pr", url));
    }
    if let Some(usd) = summary["cost_usd"].as_f64() {
        out.push(Progress::new("cost", format!("${usd:.2} in model calls")));
    }
    out
}

/// The summary's checks as `{name, ok}`.
fn checks(summary: &Value) -> Vec<Value> {
    summary["checks"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|check| {
                    let name = check["id"]
                        .as_str()
                        .or_else(|| check["name"].as_str())
                        .unwrap_or("check");
                    json!({"name": name, "ok": check["ok"].as_bool().unwrap_or(false)})
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The box's cost and time, from oa-boat's usage.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoxUsage {
    pub sandbox: String,
    pub usd: Option<f64>,
    pub secs: Option<f64>,
}

/// The run's result in the work-run shape (`crates/openagents-web/src/work_runs.rs`
/// `outcome`): `ok`, `engine`, `cost_usd`, `secs`, `checks`, `diff`,
/// `commit`, `landed.url`, `error`, plus the box and the patch.
#[must_use]
pub fn result(
    summary: Option<&Value>,
    error: Option<String>,
    usage: &BoxUsage,
    secs: f64,
) -> Value {
    let empty = Value::Null;
    let summary = summary.unwrap_or(&empty);
    let model_usd = summary["cost_usd"].as_f64();
    let cost = match (model_usd, usage.usd) {
        (Some(model), Some(machine)) => Some(model + machine),
        // Unknown stays unknown, never zero.
        _ => None,
    };
    let error = error.or_else(|| summary["error"].as_str().map(str::to_owned));
    let pr = summary["pr"].as_str();
    json!({
        "engine": "solver",
        "ok": pr.is_some() && summary["ok"] == true,
        "outcome": summary["outcome"],
        "cost_usd": cost,
        "model_usd": model_usd,
        "secs": secs,
        "checks": checks(summary),
        "diff": {
            "files": summary["files"].as_array().cloned().unwrap_or_default(),
            "added": summary["added"].as_u64().unwrap_or(0),
            "removed": summary["removed"].as_u64().unwrap_or(0),
        },
        "commit": summary["commit"],
        "landed": {"url": pr},
        "reply": summary["reply"],
        "error": error,
        "box": {"sandbox": usage.sandbox, "usd": usage.usd, "secs": usage.secs},
        "patch": summary["patch"],
        "patch_truncated": summary["patch_truncated"],
    })
}

fn why(error: &boat::Error) -> String {
    match error {
        boat::Error::Api(api) => match api.code() {
            Some(code) => format!("{error} ({code})"),
            None => error.to_string(),
        },
        other => other.to_string(),
    }
}

/// Runs `command` on the box and waits (up to 5 minutes).
async fn sh(client: &boat::Client, id: &str, command: &str) -> Result<(i64, String), String> {
    let answer = client
        .command(&CommandParams {
            sandbox_id: id.into(),
            body: CommandRequest {
                command: command.into(),
                timeout_seconds: Some(300),
                ..Default::default()
            },
            ..Default::default()
        })
        .await
        .map_err(|e| why(&e))?;
    match answer {
        CommandResponseBody::Finished(done) => Ok((
            done.exit_code.unwrap_or(-1),
            format!("{}{}", done.stdout, done.stderr),
        )),
        CommandResponseBody::Started(_) => Err("the command did not finish".into()),
    }
}

/// Waits until the box runs commands (it can report ready a few seconds
/// before), up to ten minutes.
async fn reachable(client: &boat::Client, id: &str) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(600);
    loop {
        match sh(client, id, "true").await {
            Ok(_) => return Ok(()),
            Err(e) if Instant::now() > deadline => return Err(e),
            Err(_) => tokio::time::sleep(Duration::from_secs(3)).await,
        }
    }
}

/// Takes one issue to a pull request on a box of its own and returns the
/// result ([`result`]'s shape). `on` gets each progress line as it comes.
/// The box is deleted however the run ends.
pub async fn run(
    client: &boat::Client,
    run: &Run,
    credentials: &Credentials,
    google: &GoogleToken,
    mut on: impl FnMut(&Progress),
) -> Value {
    let started = Instant::now();
    on(&Progress::new("box", "Asking for a computer"));
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let mut extra = BTreeMap::new();
    extra.insert("idleStopSeconds".to_owned(), json!(1800));
    let created = client
        .create(&CreateParams {
            idempotency_key: Some(format!("oa-solver-box-{}-{nonce}", run.issue)),
            body: Some(CreateSandboxRequest {
                type_: Some(SIZE.into()),
                ttl_seconds: Nullable::Value(TTL_SECONDS),
                no_env: Some(true),
                extra,
                ..Default::default()
            }),
            ..Default::default()
        })
        .await;
    let id = match created {
        Ok(created) => created.sandbox.id,
        Err(e) => {
            let usage = BoxUsage::default();
            return result(
                None,
                Some(format!("No computer was free: {}", why(&e))),
                &usage,
                started.elapsed().as_secs_f64(),
            );
        }
    };
    on(&Progress {
        data: json!({"sandbox": id}),
        ..Progress::new("box", format!("Got computer {id}"))
    });
    let (summary, error) = match on_box(client, &id, run, credentials, google, &mut on).await {
        Ok(summary) => (summary, None),
        Err(error) => (None, Some(error)),
    };
    let usage = usage(client, &id).await;
    let deleted = client
        .delete_sandbox(&DeleteSandboxParams {
            sandbox_id: id.clone(),
            x_ascii_confirm_delete: id.clone(),
            ..Default::default()
        })
        .await;
    match deleted {
        Ok(_) => on(&Progress::new("box", format!("Deleted computer {id}"))),
        Err(e) => on(&Progress::new(
            "box",
            format!("Could not delete computer {id}: {}", why(&e)),
        )),
    }
    result(
        summary.as_ref(),
        error,
        &usage,
        started.elapsed().as_secs_f64(),
    )
}

async fn usage(client: &boat::Client, id: &str) -> BoxUsage {
    let read = client
        .usage(&UsageParams {
            sandbox_id: id.into(),
            ..Default::default()
        })
        .await
        .ok();
    #[allow(clippy::cast_precision_loss)]
    BoxUsage {
        sandbox: id.to_owned(),
        usd: read.as_ref().map(|u| u.dollars),
        secs: read.as_ref().map(|u| u.seconds as f64),
    }
}

/// Everything between getting the box and deleting it.
async fn on_box(
    client: &boat::Client,
    id: &str,
    run: &Run,
    credentials: &Credentials,
    google: &GoogleToken,
    on: &mut impl FnMut(&Progress),
) -> Result<Option<Value>, String> {
    let wait = WaitOptions {
        timeout: Duration::from_secs(900),
        interval: Duration::from_secs(3),
        ..Default::default()
    };
    client
        .wait_until_ready(id, &wait)
        .await
        .map_err(|e| format!("The computer did not start: {}", why(&e)))?;
    reachable(client, id)
        .await
        .map_err(|e| format!("The computer did not start: {e}"))?;
    on(&Progress::new("box", "The computer is up"));
    let dirs =
        format!("sudo -n install -d -m 0700 -o \"$(id -u)\" -g \"$(id -g)\" {RUN_DIR} {HOST_DIR}");
    match sh(client, id, &dirs).await {
        Ok((0, _)) => {}
        Ok((code, text)) => {
            return Err(format!(
                "The computer could not be set up (exit {code}): {}",
                short(&text, 300)
            ));
        }
        Err(e) => return Err(format!("The computer could not be set up: {e}")),
    }
    let token = google
        .get()
        .await
        .map_err(|e| format!("The solver could not start: {e}"))?;
    let write = |path: String, text: String| async move {
        client
            .write_text(id, &path, &text)
            .await
            .map_err(|e| format!("Writing {path} on the computer: {}", why(&e)))
    };
    write(format!("{HOST_DIR}/registry-token"), token.clone()).await?;
    write(format!("{RUN_DIR}/vertex-token"), token).await?;
    write(format!("{RUN_DIR}/env"), env_file(run, credentials)).await?;
    write(SCRIPT_PATH.into(), BOX_SCRIPT.into()).await?;
    let lock = format!("chmod 600 {RUN_DIR}/env {RUN_DIR}/vertex-token {HOST_DIR}/registry-token");
    sh(client, id, &lock)
        .await
        .map_err(|e| format!("The computer could not be set up: {e}"))?;
    let process = client
        .exec_detached(
            id,
            CommandRequest {
                command: format!("bash {SCRIPT_PATH} {}", shell_quote(&run.image)),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| format!("The solver could not start: {}", why(&e)))?;
    let mut follower = client
        .follow_command(
            id,
            process.process_id,
            WaitOptions {
                timeout: Duration::from_secs(u64::try_from(TTL_SECONDS).unwrap_or(21_600)),
                interval: Duration::from_secs(3),
                ..Default::default()
            },
        )
        .map_err(|e| e.to_string())?;
    // The Vertex AI token is rewritten every 20 minutes while the run
    // lives; a failed rewrite is retried at the next tick.
    let refresher = {
        let client = client.clone();
        let id = id.to_owned();
        let google = google.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(VERTEX_REFRESH);
            tick.tick().await;
            loop {
                tick.tick().await;
                if let Ok(token) = google.get().await {
                    let _ = client
                        .write_text(&id, &format!("{RUN_DIR}/vertex-token"), &token)
                        .await;
                }
            }
        })
    };
    let followed = follow(&mut follower, on).await;
    refresher.abort();
    followed
}

/// Reads the container's output to its end; returns its summary line.
async fn follow(
    follower: &mut boat::CommandFollower,
    on: &mut impl FnMut(&Progress),
) -> Result<Option<Value>, String> {
    let mut reader = Reader::default();
    let mut partial = String::new();
    let mut exit = None;
    loop {
        let frame = match follower.next().await {
            Ok(Some(frame)) => frame,
            Ok(None) => break,
            Err(e) => return Err(format!("Reading the run on the computer: {}", why(&e))),
        };
        match frame {
            CommandFrame::Stdout(text) => {
                partial.push_str(&text);
                while let Some(end) = partial.find('\n') {
                    let line: String = partial.drain(..=end).collect();
                    for progress in reader.line(&line) {
                        on(&progress);
                    }
                }
            }
            CommandFrame::Stderr(text) => {
                for line in text.lines() {
                    reader.line(line);
                }
            }
            CommandFrame::Exit { exit_code, .. } => exit = exit_code,
            _ => {}
        }
    }
    for progress in reader.line(&partial) {
        on(&progress);
    }
    for progress in reader.finish() {
        on(&progress);
    }
    match reader.summary {
        Some(summary) => Ok(Some(summary)),
        None => Err(format!(
            "The solver ended (exit {}) without a result: {}",
            exit.map_or_else(|| "unknown".into(), |c| c.to_string()),
            reader.other.last().cloned().unwrap_or_default()
        )),
    }
}

#[cfg(test)]
mod tests;
