//! The scripted executor: a session that plays a script instead of a
//! model.
//!
//! A script is a list of timed actions: a native event to emit, an
//! assistant claim, a command and its output, a file to write or remove,
//! a real command to run, and how the session ends. Steering and resuming
//! have their own action lists, played from the moment the host sends the
//! message. The session writes the same native stream a Codex or Claude
//! Code session would, so the host's parsers, monitors, and records run on
//! it unchanged, and it demonstrates every capability in the
//! [`crate::session`] matrix, so the host's handling of each can be tested
//! in milliseconds.
//!
//! [`Script::from_stream`] turns a retained native stream into a script
//! that replays it: every line in order, plus the files its here-documents
//! and `Write` calls wrote, rebound from the task's absolute paths into a
//! scratch directory.
//!
//! ```json
//! {"schema":"openagents.coder-one.executor-script.v1","name":"good",
//!  "events":[{"at_ms":0,"do":"claim","text":"Writing run.py."},
//!            {"at_ms":20,"do":"write","path":"run.py","content":"…"},
//!            {"at_ms":40,"do":"end"}]}
//! ```

use std::path::{Component as PathComponent, Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::delegate::{self, Briefing, Executor, Report, Status, Summary};
use crate::record::Recorder;
use crate::session::{self, Capabilities, Controls, Driven, Session, StopAck};
use crate::stream::{self, Event, Format};

/// The schema every script carries.
pub const SCRIPT_SCHEMA: &str = "openagents.coder-one.executor-script.v1";

/// How long a scripted `run` command may take.
const RUN_DEADLINE: Duration = Duration::from_secs(60);

fn codex_format() -> Format {
    Format::Codex
}

fn scripted_model() -> String {
    "scripted".to_string()
}

fn yes() -> bool {
    true
}

/// A script.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Script {
    pub schema: String,
    pub name: String,
    /// The native stream format the session writes.
    #[serde(default = "codex_format")]
    pub format: Format,
    #[serde(default = "scripted_model")]
    pub model: String,
    /// What the session lets the host do; every capability by default.
    #[serde(default)]
    pub capabilities: Capabilities,
    pub events: Vec<Timed>,
    /// Played from the moment the host steers.
    #[serde(default)]
    pub on_steer: Vec<Timed>,
    /// Played from the moment the host resumes.
    #[serde(default)]
    pub on_resume: Vec<Timed>,
    /// Absolute path prefixes, such as `/app`, rebound into the working
    /// directory. Any other absolute path is refused.
    #[serde(default)]
    pub rebind: Vec<String>,
    /// Whether the session writes its own opening events. A replay turns
    /// it off: the replayed stream opens itself.
    #[serde(default = "yes")]
    pub opening: bool,
    /// Plays other events when the briefing lacks some text, so a script
    /// can stand in for an executor that acts on what it's told: a repair
    /// that fixes the fault only when the brief carries the check's
    /// observations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub briefed: Option<Box<Briefed>>,
}

/// Events for a briefing that lacks `contains`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Briefed {
    pub contains: String,
    pub otherwise: Vec<Timed>,
}

/// One action at a time, in milliseconds from when its list started.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Timed {
    pub at_ms: u64,
    #[serde(flatten)]
    pub act: Act,
}

/// What a script does.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "do", rename_all = "snake_case")]
pub enum Act {
    /// Emits one native event, as JSON.
    Emit { event: Value },
    /// Emits one native line verbatim; replays use this.
    Raw { line: String },
    /// An assistant message.
    Claim { text: String },
    /// A command the executor reports having run, with its output. Nothing
    /// runs.
    Command {
        command: String,
        #[serde(default)]
        output: String,
        #[serde(default)]
        exit_code: i64,
    },
    /// Writes a file under the working directory. `announce` emits the
    /// file change event; a replay turns it off, because the replayed
    /// stream already holds the command that wrote it.
    Write {
        path: String,
        content: String,
        #[serde(default = "yes")]
        announce: bool,
    },
    /// Removes a file under the working directory.
    Remove { path: String },
    /// Runs a real shell command in the working directory, bounded, and
    /// reports it with its output.
    Run { command: String },
    /// Ends the turn: completed, or failed when `error` is set.
    End {
        #[serde(default)]
        error: bool,
    },
    /// The process exits with `code` and no result.
    Exit { code: i32 },
    /// The session stops making progress until steered, stopped, or its
    /// deadline passes.
    Hang,
}

impl Script {
    /// Reads a script file.
    ///
    /// # Errors
    ///
    /// Returns a message when it doesn't read or isn't a script.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        Self::parse(&text).map_err(|error| format!("{}: {error}", path.display()))
    }

    /// Parses a script.
    ///
    /// # Errors
    ///
    /// Returns a message when the text isn't a script.
    pub fn parse(text: &str) -> Result<Self, String> {
        let script: Script = serde_json::from_str(text).map_err(|error| error.to_string())?;
        if script.schema != SCRIPT_SCHEMA {
            return Err(format!("not a {SCRIPT_SCHEMA} script"));
        }
        Ok(script)
    }

    /// A script that replays a retained native stream: each line `gap_ms`
    /// after the last, and each file the stream wrote in full, right after
    /// the line that wrote it. `rebind` names the absolute prefixes, such
    /// as `/app`, that map into the working directory.
    #[must_use]
    pub fn from_stream(name: &str, stream: &str, gap_ms: u64, rebind: &[&str]) -> Self {
        let format = Format::detect(stream).unwrap_or(Format::Codex);
        let writes = stream::writes(format, stream);
        let mut events = Vec::new();
        for (i, line) in stream.lines().enumerate() {
            let at_ms = i as u64 * gap_ms;
            events.push(Timed {
                at_ms,
                act: Act::Raw {
                    line: line.to_string(),
                },
            });
            for write in writes.iter().filter(|w| w.line == i + 1) {
                events.push(Timed {
                    at_ms,
                    act: Act::Write {
                        path: write.path.clone(),
                        content: write.content.clone(),
                        announce: false,
                    },
                });
            }
        }
        Script {
            schema: SCRIPT_SCHEMA.to_string(),
            name: name.to_string(),
            format,
            model: scripted_model(),
            capabilities: Capabilities::all(),
            events,
            on_steer: Vec::new(),
            on_resume: Vec::new(),
            rebind: rebind.iter().map(|prefix| (*prefix).to_string()).collect(),
            opening: false,
            briefed: None,
        }
    }

    /// The script's digest, for the record.
    #[must_use]
    pub fn digest(&self) -> String {
        atif::digest(&serde_json::to_value(self).unwrap_or(Value::Null))
    }
}

/// Where a scripted session is.
#[derive(Clone, Debug, PartialEq)]
enum State {
    Idle,
    Running,
    /// The script ran out, ended its turn, or exited.
    Exited(i32),
    Stopped {
        reason: String,
        deadline: bool,
    },
    /// The executor's own deadline passed, as a supervisor would enforce.
    TimedOut,
}

/// The scripted executor, and its one session at a time.
pub struct Scripted {
    pub script: Script,
    pub workdir: PathBuf,
    /// Where the briefing and the stream are written, when anywhere.
    pub artifacts: Option<PathBuf>,
    /// How the record names that directory.
    pub artifacts_label: String,
    /// The executor's own deadline, which a stop can come before.
    pub deadline: Duration,
    /// Real milliseconds per scripted millisecond: 0 runs on virtual time.
    pub speed: f64,
    /// What the host does during [`Executor::execute`].
    pub controls: Controls,
    pub recorder: Recorder,
    /// Delegations run so far.
    pub runs: u32,
    /// The last session's host-loop record.
    pub last: Option<Value>,
    /// A `control.monitor` to watch each session, in shadow mode unless
    /// its caller makes it act.
    pub monitor: Option<crate::monitor::Setup>,
    queue: Vec<(u64, Act)>,
    hanging: bool,
    lines: Vec<String>,
    observed: usize,
    seq: u64,
    state: State,
    session_id: String,
    errors: Vec<String>,
    last_claim: Option<String>,
}

impl Scripted {
    /// A scripted executor for `script` in `workdir`.
    #[must_use]
    pub fn new(script: Script, workdir: PathBuf) -> Self {
        Scripted {
            script,
            workdir,
            artifacts: None,
            artifacts_label: "artifacts".to_string(),
            deadline: Duration::from_secs(600),
            speed: 0.0,
            controls: Controls::default(),
            recorder: Recorder::default(),
            runs: 0,
            last: None,
            monitor: None,
            queue: Vec::new(),
            hanging: false,
            lines: Vec::new(),
            observed: 0,
            seq: 0,
            state: State::Idle,
            session_id: String::new(),
            errors: Vec::new(),
            last_claim: None,
        }
    }

    /// The native stream so far.
    #[must_use]
    pub fn stream(&self) -> String {
        let mut text = self.lines.join("\n");
        if !text.is_empty() {
            text.push('\n');
        }
        text
    }

    /// What went wrong applying the script, such as a refused path.
    #[must_use]
    pub fn errors(&self) -> &[String] {
        &self.errors
    }

    fn schedule(&mut self, from: u64, list: &[Timed]) {
        for timed in list {
            self.queue.push((from + timed.at_ms, timed.act.clone()));
        }
        // Stable, so actions at one time keep their script order.
        self.queue.sort_by_key(|(at, _)| *at);
    }

    fn emit(&mut self, event: &Value) {
        self.lines.push(event.to_string());
    }

    fn resolve(&self, path: &str) -> Result<PathBuf, String> {
        let relative = if Path::new(path).is_absolute() {
            self.script
                .rebind
                .iter()
                .find_map(|prefix| {
                    let prefix = prefix.trim_end_matches('/');
                    path.strip_prefix(prefix)
                        .filter(|rest| rest.is_empty() || rest.starts_with('/'))
                        .map(|rest| rest.trim_start_matches('/').to_string())
                })
                .ok_or_else(|| format!("{path} is outside the working directory"))?
        } else {
            path.to_string()
        };
        if Path::new(&relative)
            .components()
            .any(|c| !matches!(c, PathComponent::Normal(_) | PathComponent::CurDir))
        {
            return Err(format!("{path} leaves the working directory"));
        }
        Ok(self.workdir.join(relative))
    }

    fn command_lines(&mut self, command: &str, output: &str, exit_code: i64) {
        let id = self.next_id();
        let lines = match self.script.format {
            Format::Codex => vec![
                json!({"type":"item.started","item":{"id":id,"type":"command_execution","command":command,"aggregated_output":"","exit_code":null,"status":"in_progress"}}),
                json!({"type":"item.completed","item":{"id":id,"type":"command_execution","command":command,"aggregated_output":output,"exit_code":exit_code,"status":"completed"}}),
            ],
            Format::Claude => vec![
                json!({"type":"assistant","message":{"id":format!("msg_{id}"),"content":[{"type":"tool_use","id":id,"name":"Bash","input":{"command":command}}]}}),
                json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":id,"content":output,"is_error":exit_code != 0}]}}),
            ],
        };
        self.emit_all(&lines);
    }

    fn next_id(&self) -> String {
        format!("item_{}", self.lines.len())
    }

    fn emit_all(&mut self, lines: &[Value]) {
        for line in lines {
            self.emit(line);
        }
    }

    async fn apply(&mut self, act: Act) {
        match act {
            Act::Emit { event } => self.emit(&event),
            Act::Raw { line } => {
                if let Ok(event) = serde_json::from_str::<Value>(&line)
                    && event.pointer("/item/type").and_then(Value::as_str) == Some("agent_message")
                {
                    self.last_claim = event
                        .pointer("/item/text")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                }
                self.lines.push(line);
            }
            Act::Claim { text } => {
                self.last_claim = Some(text.clone());
                let id = self.next_id();
                let lines = match self.script.format {
                    Format::Codex => vec![
                        json!({"type":"item.completed","item":{"id":id,"type":"agent_message","text":text}}),
                    ],
                    Format::Claude => vec![
                        json!({"type":"assistant","message":{"id":format!("msg_{id}"),"content":[{"type":"text","text":text}]}}),
                    ],
                };
                self.emit_all(&lines);
            }
            Act::Command {
                command,
                output,
                exit_code,
            } => self.command_lines(&command, &output, exit_code),
            Act::Write {
                path,
                content,
                announce,
            } => match self.resolve(&path) {
                Ok(target) => {
                    let existed = target.exists();
                    let written = target
                        .parent()
                        .map_or(Ok(()), std::fs::create_dir_all)
                        .and_then(|()| std::fs::write(&target, &content));
                    if let Err(error) = written {
                        self.errors.push(format!("cannot write {path}: {error}"));
                    } else if announce {
                        let kind = if existed { "update" } else { "add" };
                        let id = self.next_id();
                        let lines = match self.script.format {
                            Format::Codex => vec![
                                json!({"type":"item.completed","item":{"id":id,"type":"file_change","changes":[{"path":path,"kind":kind}],"status":"completed"}}),
                            ],
                            Format::Claude => vec![
                                json!({"type":"assistant","message":{"id":format!("msg_{id}"),"content":[{"type":"tool_use","id":id,"name":"Write","input":{"file_path":path,"content":content}}]}}),
                            ],
                        };
                        self.emit_all(&lines);
                    }
                }
                Err(error) => self.errors.push(error),
            },
            Act::Remove { path } => match self.resolve(&path) {
                Ok(target) => {
                    if let Err(error) = std::fs::remove_file(&target) {
                        self.errors.push(format!("cannot remove {path}: {error}"));
                    } else {
                        let id = self.next_id();
                        self.emit(&json!({"type":"item.completed","item":{"id":id,"type":"file_change","changes":[{"path":path,"kind":"delete"}],"status":"completed"}}));
                    }
                }
                Err(error) => self.errors.push(error),
            },
            Act::Run { command } => {
                let mut prepared = std::process::Command::new("bash");
                prepared
                    .arg("-c")
                    .arg(&command)
                    .current_dir(&self.workdir)
                    .env("GIT_TERMINAL_PROMPT", "0")
                    .env("GIT_EDITOR", "true")
                    .env("PAGER", "cat");
                let ended = supervise::Job::from_command(prepared)
                    .bounded(supervise::Limits::within(RUN_DEADLINE).keeping(64 * 1024))
                    .run()
                    .await;
                let mut output = ended.stdout.marked();
                output.push_str(&ended.stderr.marked());
                let code = match ended.ending {
                    supervise::Ending::Exited(Some(code)) => i64::from(code),
                    _ => -1,
                };
                self.command_lines(&command, &output, code);
            }
            Act::End { error } => {
                let line = match (self.script.format, error) {
                    (Format::Codex, false) => {
                        json!({"type":"turn.completed","usage":{"input_tokens":0,"cached_input_tokens":0,"output_tokens":0}})
                    }
                    (Format::Codex, true) => {
                        json!({"type":"turn.failed","error":{"message":"the scripted turn failed"}})
                    }
                    (Format::Claude, error) => {
                        json!({"type":"result","subtype": if error { "error_during_execution" } else { "success" },"is_error":error,"result":self.last_claim,"num_turns":self.lines.len(),"total_cost_usd":0.0,"session_id":self.session_id,"usage":{"input_tokens":0,"output_tokens":0}})
                    }
                };
                self.emit(&line);
                self.queue.clear();
                self.state = State::Exited(0);
            }
            Act::Exit { code } => {
                self.queue.clear();
                self.state = State::Exited(code);
            }
            Act::Hang => {
                // What was left of this list waits for a steer that never
                // plays it; a steer schedules its own list.
                self.queue.clear();
                self.hanging = true;
            }
        }
    }

    fn opening(&mut self) {
        let line = match self.script.format {
            Format::Codex => json!({"type":"thread.started","thread_id":self.session_id}),
            Format::Claude => {
                json!({"type":"system","subtype":"init","session_id":self.session_id,"model":self.script.model,"claude_code_version":"scripted"})
            }
        };
        self.emit(&line);
        if self.script.format == Format::Codex {
            self.emit(&json!({"type":"turn.started"}));
        }
    }

    /// The report as the delegation record reads it.
    fn build_report(&mut self, milliseconds: u64) -> Report {
        let text = self.stream();
        let summary = match self.script.format {
            Format::Codex => Summary::parse_codex(&text, &self.script.model),
            Format::Claude => Summary::parse(&text),
        };
        let stderr = self.errors.join("\n");
        let status = match &self.state {
            State::Idle => Status::Harness("the session never started".to_string()),
            State::Running => Status::Harness("the session was still running".to_string()),
            State::TimedOut | State::Stopped { deadline: true, .. } => Status::TimedOut,
            State::Stopped { reason, .. } => {
                Status::Harness(format!("stopped by the host: {reason}"))
            }
            State::Exited(code) => {
                delegate::classify(&supervise::Ending::Exited(Some(*code)), &summary, &stderr)
            }
        };
        let mut stream_record = None;
        if let Some(dir) = &self.artifacts {
            let name = format!("delegate-{}.stream.jsonl", self.runs);
            if std::fs::create_dir_all(dir).is_ok()
                && std::fs::write(dir.join(&name), &text).is_ok()
            {
                stream_record = Some(json!({
                    "path": format!("{}/{name}", self.artifacts_label),
                    "bytes": text.len(),
                    "kept_bytes": text.len(),
                    "truncated": false,
                    "sha256": hex(&Sha256::digest(text.as_bytes())),
                }));
            }
        }
        Report {
            status,
            summary,
            milliseconds,
            stderr,
            stream: stream_record,
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

impl Session for Scripted {
    fn adapter(&self) -> &str {
        "scripted"
    }

    fn capabilities(&self) -> Capabilities {
        self.script.capabilities
    }

    fn session_id(&self) -> Option<String> {
        (!self.session_id.is_empty()).then(|| self.session_id.clone())
    }

    async fn start(&mut self, briefing: &Briefing) -> Result<(), String> {
        if self.state == State::Running {
            return Err("a session is already running".to_string());
        }
        self.queue.clear();
        self.lines.clear();
        self.observed = 0;
        self.seq = 0;
        self.errors.clear();
        self.hanging = false;
        self.last_claim = None;
        self.session_id = format!(
            "scripted-{}-{}",
            &self.script.digest()[..12],
            &briefing.sha256()[..12]
        );
        if let Some(dir) = &self.artifacts {
            let _ = std::fs::create_dir_all(dir);
            let _ = std::fs::write(
                dir.join(format!("delegate-{}.briefing.md", self.runs.max(1))),
                &briefing.text,
            );
        }
        self.state = State::Running;
        if self.script.opening {
            self.opening();
        }
        let events = match &self.script.briefed {
            Some(briefed) if !briefing.text.contains(&briefed.contains) => {
                briefed.otherwise.clone()
            }
            _ => self.script.events.clone(),
        };
        self.schedule(0, &events);
        Ok(())
    }

    async fn advance(&mut self, now_ms: u64) -> bool {
        while self.state == State::Running {
            let Some(index) = self.queue.iter().position(|(at, _)| *at <= now_ms) else {
                break;
            };
            let (_, act) = self.queue.remove(index);
            self.apply(act).await;
        }
        if self.state == State::Running && self.queue.is_empty() && !self.hanging {
            // The script ran out without ending its turn.
            self.state = State::Exited(0);
        }
        if self.state == State::Running
            && now_ms >= u64::try_from(self.deadline.as_millis()).unwrap_or(u64::MAX)
        {
            self.queue.clear();
            self.state = State::TimedOut;
        }
        self.state == State::Running
    }

    fn observe(&mut self) -> Vec<Event> {
        let fresh: Vec<Event> = self.lines[self.observed..]
            .iter()
            .enumerate()
            .flat_map(|(i, line)| {
                stream::normalize_line(
                    self.script.format,
                    line,
                    self.observed + i + 1,
                    &mut self.seq,
                )
            })
            .collect();
        self.observed = self.lines.len();
        fresh
    }

    async fn stop(&mut self, now_ms: u64, reason: &str) -> Result<StopAck, String> {
        if self.state != State::Running {
            return Err("no session is running".to_string());
        }
        let pending = self.queue.len();
        self.queue.clear();
        self.hanging = false;
        self.state = State::Stopped {
            reason: reason.to_string(),
            deadline: reason.contains("deadline"),
        };
        Ok(StopAck {
            at_ms: now_ms,
            reason: reason.to_string(),
            pending,
            cleanup: "no child process was left running; scripted commands run to completion before the next action".to_string(),
        })
    }

    async fn resume(&mut self, now_ms: u64, _message: &str) -> Result<(), String> {
        if matches!(self.state, State::Running | State::Idle) {
            return Err("only a stopped or ended session resumes".to_string());
        }
        self.state = State::Running;
        self.hanging = false;
        let line = match self.script.format {
            Format::Codex => json!({"type":"turn.started"}),
            Format::Claude => {
                json!({"type":"system","subtype":"init","session_id":self.session_id,"model":self.script.model,"claude_code_version":"scripted"})
            }
        };
        self.emit(&line);
        let list = self.script.on_resume.clone();
        self.schedule(now_ms, &list);
        Ok(())
    }

    async fn steer(&mut self, now_ms: u64, _message: &str) -> Result<(), String> {
        if self.state != State::Running {
            return Err("no session is running".to_string());
        }
        self.hanging = false;
        let list = self.script.on_steer.clone();
        self.schedule(now_ms, &list);
        Ok(())
    }

    fn report(&mut self) -> Report {
        self.build_report(0)
    }
}

impl Executor for Scripted {
    fn agent(&self) -> &str {
        "scripted"
    }

    fn cost_provenance(&self) -> &'static str {
        "none"
    }

    fn model(&self) -> &str {
        &self.script.model
    }

    fn deadline(&self) -> Duration {
        self.deadline
    }

    fn describe(&self) -> Map<String, Value> {
        let mut extra = Map::new();
        extra.insert("script".to_string(), json!(self.script.name));
        extra.insert("script_digest".to_string(), json!(self.script.digest()));
        extra.insert("workdir".to_string(), json!(self.workdir.to_string_lossy()));
        extra.insert(
            "capabilities".to_string(),
            self.script
                .capabilities
                .record("scripted", "demonstrated by the scripted adapter's tests"),
        );
        extra.insert("speed".to_string(), json!(self.speed));
        extra
    }

    async fn execute(&mut self, briefing: &Briefing) -> Report {
        self.runs += 1;
        let started = std::time::Instant::now();
        let recorder = self.recorder.clone();
        let mut controls = self.controls.clone();
        controls.deadline_ms = controls
            .deadline_ms
            .min(u64::try_from(self.deadline.as_millis()).unwrap_or(u64::MAX));
        let speed = self.speed;
        let mut pace: Box<dyn FnMut(u64) -> session::Tick> = if speed > 0.0 {
            Box::new(session::real_time(speed))
        } else {
            Box::new(session::virtual_time())
        };
        let mut monitor = self
            .monitor
            .as_ref()
            .map(|setup| setup.start(&briefing.text));
        let driven: Driven = session::drive_watched(
            self,
            briefing,
            &controls,
            &recorder,
            &mut *pace,
            monitor.as_mut().map(|m| m as &mut dyn session::Watch),
        )
        .await;
        let mut record = driven.record();
        if let Some(monitor) = &monitor {
            record["monitor"] = crate::monitor::summary(&monitor.judgments);
        }
        self.last = Some(record);
        let milliseconds = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let mut report = self.build_report(milliseconds);
        report.milliseconds = milliseconds;
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::delegate::BriefingInputs;
    use crate::session::{Capability, Steer, Trigger};

    fn briefing() -> Briefing {
        Briefing::build(
            &BriefingInputs {
                instruction: "Write the file.".to_string(),
                requirements: Vec::new(),
                files: Vec::new(),
                spans: Vec::new(),
                commands: Vec::new(),
                last_output: None,
                conclusion: String::new(),
                directions: String::new(),
            },
            1_000,
        )
    }

    fn scratch(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "coder-one-scripted-{label}-{}-{}",
            std::process::id(),
            atif::now_ms()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn script(events: Value) -> Script {
        Script::parse(
            &json!({ "schema": SCRIPT_SCHEMA, "name": "test", "events": events }).to_string(),
        )
        .unwrap()
    }

    async fn drive_with(scripted: &mut Scripted, controls: &Controls) -> Driven {
        let recorder = scripted.recorder.clone();
        session::drive(
            scripted,
            &briefing(),
            controls,
            &recorder,
            &mut session::virtual_time(),
        )
        .await
    }

    #[tokio::test]
    async fn a_script_writes_its_files_at_their_times_and_answers() {
        let dir = scratch("write");
        let mut scripted = Scripted::new(
            script(json!([
                { "at_ms": 0, "do": "claim", "text": "Writing the file." },
                { "at_ms": 50, "do": "write", "path": "out/a.txt", "content": "hello\n" },
                { "at_ms": 60, "do": "run", "command": "cat out/a.txt" },
                { "at_ms": 100, "do": "claim", "text": "Done." },
                { "at_ms": 100, "do": "end" },
            ])),
            dir.clone(),
        );
        scripted.artifacts = Some(dir.join("artifacts"));
        let report = scripted.execute(&briefing()).await;
        assert_eq!(report.status, Status::Answered, "{}", report.stderr);
        assert_eq!(report.summary.result.as_deref(), Some("Done."));
        assert_eq!(
            std::fs::read_to_string(dir.join("out/a.txt")).unwrap(),
            "hello\n"
        );
        let events = stream::normalize(Format::Codex, &scripted.stream());
        let completed = events
            .iter()
            .find_map(|e| match &e.kind {
                stream::Kind::CommandCompleted {
                    output, exit_code, ..
                } => Some((output.clone(), *exit_code)),
                _ => None,
            })
            .unwrap();
        assert_eq!(completed, ("hello\n".to_string(), Some(0)));
        assert!(dir.join("artifacts/delegate-1.stream.jsonl").is_file());
        let last = scripted.last.clone().unwrap();
        assert_eq!(last["events"]["artifact_changed"], json!(1));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn a_write_outside_the_workdir_is_refused() {
        let dir = scratch("escape");
        let mut scripted = Scripted::new(
            script(json!([
                { "at_ms": 0, "do": "write", "path": "../escape.txt", "content": "x" },
                { "at_ms": 0, "do": "write", "path": "/etc/escape.txt", "content": "x" },
                { "at_ms": 0, "do": "end" },
            ])),
            dir.clone(),
        );
        let _ = scripted.execute(&briefing()).await;
        assert_eq!(scripted.errors().len(), 2);
        assert!(!dir.parent().unwrap().join("escape.txt").exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn the_host_observes_events_as_they_arrive() {
        let dir = scratch("observe");
        let mut scripted = Scripted::new(
            script(json!([
                { "at_ms": 0, "do": "claim", "text": "one" },
                { "at_ms": 500, "do": "claim", "text": "two" },
                { "at_ms": 500, "do": "end" },
            ])),
            dir.clone(),
        );
        scripted.start(&briefing()).await.unwrap();
        assert!(scripted.advance(100).await);
        let first = scripted.observe();
        assert!(
            first
                .iter()
                .any(|e| matches!(&e.kind, stream::Kind::AssistantClaim { text } if text == "one"))
        );
        assert!(
            !first
                .iter()
                .any(|e| matches!(&e.kind, stream::Kind::AssistantClaim { text } if text == "two"))
        );
        assert!(!scripted.advance(500).await);
        let second = scripted.observe();
        assert!(
            second
                .iter()
                .any(|e| matches!(&e.kind, stream::Kind::AssistantClaim { text } if text == "two"))
        );
        assert!(second.iter().all(|e| e.seq > first.last().unwrap().seq));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn the_host_stops_a_hung_session_at_its_deadline_with_an_acknowledgement() {
        let dir = scratch("stop");
        let mut scripted = Scripted::new(
            script(json!([
                { "at_ms": 0, "do": "claim", "text": "thinking" },
                { "at_ms": 10, "do": "hang" },
                { "at_ms": 20, "do": "claim", "text": "never" },
            ])),
            dir.clone(),
        );
        let driven = drive_with(
            &mut scripted,
            &Controls {
                deadline_ms: 200,
                ..Controls::default()
            },
        )
        .await;
        assert_eq!(driven.report.status, Status::TimedOut);
        assert_eq!(driven.stops.len(), 1);
        assert!(driven.stops[0].at_ms >= 200);
        let stop = driven
            .actions
            .iter()
            .find(|a| a.capability == Capability::Stop)
            .unwrap();
        assert_eq!(stop.outcome, "done");
        let records = crate::record::invocations(&scripted.recorder.steps());
        assert!(
            records
                .iter()
                .any(|i| i.component == "exec.control" && i.name.as_deref() == Some("stop"))
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn a_steer_unblocks_the_session_and_plays_its_list() {
        let dir = scratch("steer");
        let mut script = script(json!([
            { "at_ms": 0, "do": "command", "command": "pytest", "output": "1 failed", "exit_code": 1 },
            { "at_ms": 10, "do": "hang" },
        ]));
        script.on_steer = serde_json::from_value(json!([
            { "at_ms": 30, "do": "write", "path": "fixed.txt", "content": "ok\n" },
            { "at_ms": 40, "do": "claim", "text": "Fixed after the steer." },
            { "at_ms": 40, "do": "end" },
        ]))
        .unwrap();
        let mut scripted = Scripted::new(script, dir.clone());
        let driven = drive_with(
            &mut scripted,
            &Controls {
                deadline_ms: 5_000,
                steer: Some(Steer {
                    when: Trigger::CommandFailed,
                    message: "A test failed; fix it before finishing.".to_string(),
                }),
                ..Controls::default()
            },
        )
        .await;
        assert_eq!(driven.report.status, Status::Answered);
        assert!(dir.join("fixed.txt").is_file());
        let steer = driven
            .actions
            .iter()
            .find(|a| a.capability == Capability::Steer)
            .unwrap();
        assert_eq!(steer.outcome, "done");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn a_stopped_session_resumes_under_its_own_id() {
        let dir = scratch("resume");
        let mut script = script(json!([
            { "at_ms": 0, "do": "claim", "text": "Wrote a draft." },
            { "at_ms": 10, "do": "hang" },
        ]));
        script.on_resume = serde_json::from_value(json!([
            { "at_ms": 5, "do": "write", "path": "final.txt", "content": "done\n" },
            { "at_ms": 5, "do": "end" },
        ]))
        .unwrap();
        let mut scripted = Scripted::new(script, dir.clone());
        let driven = drive_with(
            &mut scripted,
            &Controls {
                deadline_ms: 5_000,
                stop_when: Some(Trigger::Claim {
                    contains: "draft".to_string(),
                }),
                resume: Some("Continue from the draft.".to_string()),
                ..Controls::default()
            },
        )
        .await;
        let words: Vec<(&str, &str)> = driven
            .actions
            .iter()
            .map(|a| (a.capability.word(), a.outcome.as_str()))
            .collect();
        assert_eq!(
            words,
            [("start", "done"), ("stop", "done"), ("resume", "done")]
        );
        assert_eq!(driven.report.status, Status::Answered);
        assert!(
            driven.actions[2]
                .detail
                .contains(driven.session_id.as_deref().unwrap())
        );
        assert!(dir.join("final.txt").is_file());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn a_capability_the_adapter_lacks_is_refused_not_attempted() {
        let dir = scratch("refuse");
        let mut script = script(json!([
            { "at_ms": 0, "do": "command", "command": "make", "output": "error", "exit_code": 2 },
            { "at_ms": 10, "do": "hang" },
        ]));
        script.capabilities = Capabilities::start_only();
        script.on_steer = serde_json::from_value(json!([{ "at_ms": 0, "do": "end" }])).unwrap();
        let mut scripted = Scripted::new(script, dir.clone());
        scripted.deadline = Duration::from_millis(300);
        let driven = drive_with(
            &mut scripted,
            &Controls {
                deadline_ms: 100,
                steer: Some(Steer {
                    when: Trigger::After { ms: 50 },
                    message: "hurry".to_string(),
                }),
                ..Controls::default()
            },
        )
        .await;
        let refused: Vec<&str> = driven
            .actions
            .iter()
            .filter(|a| a.outcome == "refused")
            .map(|a| a.capability.word())
            .collect();
        assert_eq!(refused, ["observe", "steer", "stop"]);
        // Only the executor's own deadline, a supervisor's, ended it.
        assert_eq!(driven.report.status, Status::TimedOut);
        assert!(driven.events.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn a_retained_luna_stream_replays_with_its_files() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../bench/terminal-bench/traces/extended--coder-one-jevprobe3-luna--cancel-async-tasks-2/cancel-async-tasks__8MSemsU.episode/artifacts/delegate-1.stream.jsonl",
        );
        let Ok(stream) = std::fs::read_to_string(path) else {
            return;
        };
        let dir = scratch("replay");
        let script = Script::from_stream("cancel-async-tasks-2", &stream, 5, &["/app"]);
        let mut scripted = Scripted::new(script, dir.clone());
        let report = scripted.execute(&briefing()).await;
        assert_eq!(report.status, Status::Answered);
        assert_eq!(scripted.stream(), stream);
        let run = std::fs::read_to_string(dir.join("run.py")).unwrap();
        assert!(run.contains("async def run_tasks("));
        let _ = std::fs::remove_dir_all(dir);
    }
}
