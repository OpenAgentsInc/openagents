//! The trace: every conversation recorded as it happens, on local disk.
//!
//! `crates/coder` used to persist nothing. The transcript lived in
//! [`Agent`](crate::Agent) for the life of the process and went when the
//! terminal exited, so the one agent workload this repository owns was the
//! one it could not measure. A [`Recorder`] fixes that: it turns each
//! conversation into an [ATIF](atif) session log under the user's home
//! directory, and it writes each step the moment the step happens.
//!
//! # Where the traces go
//!
//! `~/.openagents/traces/<session>.atif.jsonl`, one file per session, the
//! directory and the files readable only by the user.
//!
//! - `CODER_TRACE_DIR` writes them somewhere else.
//! - `CODER_TRACE=off` turns recording off. `0`, `no`, and `false` also
//!   work, and so does an unset `HOME` — with nowhere to write, there is
//!   nothing to record to.
//!
//! # What a session is
//!
//! One terminal invocation, start to exit. `coder` does not resume a
//! conversation across processes: the transcript is the process's. Tying the
//! trace's identity to anything longer would mean claiming a continuity the
//! agent does not have.
//!
//! # What recording is allowed to cost
//!
//! Nothing that a conversation depends on. A trace is evidence *about* a
//! conversation, not part of one, so a write that fails stops the recording,
//! keeps the reason, and leaves the turn alone. The terminal shows the
//! reason; the turn continues.

use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use atif::{Call, Decision, Log, Session, Source, Step};
use indexmap::IndexMap;
use jev::Answer;
use serde_json::{Map, Value, json};

use crate::generate::Usage;
use crate::shell::{self, Outcome, Status};

/// The variable that moves the trace directory.
pub const DIR_ENV: &str = "CODER_TRACE_DIR";

/// The variable that turns recording off.
pub const SWITCH_ENV: &str = "CODER_TRACE";

/// The schema a recorded shell call carries in its `extra`.
pub const SHELL_CALL_SCHEMA: &str = "openagents.shell-call.v1";

/// The kind a system step carries when it holds the instructions a
/// generation was given.
pub const INSTRUCTIONS_KIND: &str = "instructions";

/// Where this machine records traces, or `None` when recording is off.
#[must_use]
pub fn directory() -> Option<PathBuf> {
    resolve(env::var(SWITCH_ENV).ok().as_deref(), env::var_os(DIR_ENV))
}

/// The directory those two settings name. Split out from [`directory`] so
/// it can be tested without a test writing to the process environment.
fn resolve(switch: Option<&str>, dir: Option<OsString>) -> Option<PathBuf> {
    if switch.is_some_and(|switch| {
        matches!(
            switch.trim().to_ascii_lowercase().as_str(),
            "0" | "off" | "no" | "false"
        )
    }) {
        return None;
    }
    match dir.filter(|dir| !dir.is_empty()) {
        Some(dir) => Some(PathBuf::from(dir)),
        None => atif::log::default_dir(),
    }
}

/// One session's recorder.
pub struct Recorder {
    log: Log,
    /// Numbers the calls, so an observation names the call it answered.
    calls: usize,
    /// The digest of the instructions last written, so the same
    /// instructions are recorded once rather than once a turn.
    instructions: Option<String>,
    /// The directory the shell runs commands in.
    workdir: String,
    /// Why recording stopped, once it has.
    failure: Option<String>,
}

impl Recorder {
    /// Opens a recorder for this session, or `None` when recording is off.
    ///
    /// # Errors
    ///
    /// Returns a sentence naming why the log could not be opened. A caller
    /// should show it and carry on unrecorded.
    pub fn start(model: &str, door: &str, repository: &str) -> Result<Option<Self>, String> {
        match directory() {
            Some(dir) => Self::open(&dir, model, door, repository).map(Some),
            None => Ok(None),
        }
    }

    /// Opens a recorder in a named directory, whatever the environment says.
    ///
    /// # Errors
    ///
    /// Returns a sentence naming why the log could not be opened.
    pub fn open(dir: &Path, model: &str, door: &str, repository: &str) -> Result<Self, String> {
        let session = Self::session(model, door, repository);
        let log = Log::create(dir, &session)
            .map_err(|error| format!("cannot record to {}: {error}", dir.display()))?;
        Ok(Self::around(log))
    }

    /// Opens a recorder at a named file, whatever the environment says.
    ///
    /// A caller that names the file is telling the session where to land —
    /// a script that has to read the trace back should not have to guess a
    /// session identifier or watch a directory. Naming a file is a request
    /// to record, so it outranks [`SWITCH_ENV`].
    ///
    /// # Errors
    ///
    /// Returns a sentence naming why the log could not be opened. An
    /// existing file is one of those reasons: a session never writes over
    /// another session's record.
    pub fn at(path: &Path, model: &str, door: &str, repository: &str) -> Result<Self, String> {
        let session = Self::session(model, door, repository);
        let log = Log::create_at(path, &session)
            .map_err(|error| format!("cannot record to {}: {error}", path.display()))?;
        Ok(Self::around(log))
    }

    /// The session header both openers write.
    fn session(model: &str, door: &str, repository: &str) -> Session {
        Session::opening(
            &atif::log::session_id(atif::now_ms()),
            model,
            door,
            repository,
            env!("CARGO_PKG_VERSION"),
        )
    }

    /// A recorder over an opened log.
    fn around(log: Log) -> Self {
        Recorder {
            log,
            calls: 0,
            instructions: None,
            workdir: env::current_dir()
                .map(|dir| dir.display().to_string())
                .unwrap_or_default(),
            failure: None,
        }
    }

    /// The file this session is recording to.
    #[must_use]
    pub fn path(&self) -> &Path {
        self.log.path()
    }

    /// Why recording stopped, when it has.
    #[must_use]
    pub fn failure(&self) -> Option<&str> {
        self.failure.as_deref()
    }

    /// The person's turn.
    pub fn user(&mut self, text: &str) {
        self.write(Step::said(Source::User, text));
    }

    /// Something the host did, or could not do — a missing key, a door that
    /// would not answer. A turn that generated unrouted should say why.
    pub fn note(&mut self, text: &str) {
        self.write(Step::said(Source::System, text));
    }

    /// The instructions one generation was given, recorded when they differ
    /// from the last ones written.
    ///
    /// The instructions are part of the measured surface: two sessions that
    /// answered differently may have been asked differently, and a trace
    /// that leaves them out cannot tell. Recording them once per change
    /// rather than once per turn keeps that without repeating a repository
    /// context block into every step.
    pub fn instructions(&mut self, text: &str) {
        let digest = atif::digest(&json!(text));
        if self.instructions.as_deref() == Some(digest.as_str()) {
            return;
        }
        self.instructions = Some(digest);
        self.write(Step::said(Source::System, text).noting("kind", json!(INSTRUCTIONS_KIND)));
    }

    /// What the model answered, and what the turn cost.
    pub fn answer(&mut self, text: &str, usage: Option<Usage>, milliseconds: u64) {
        let mut step = Step::said(Source::Agent, text).taking(milliseconds);
        if let Some(usage) = usage {
            step.spent(atif::Usage {
                prompt: usage.input_tokens,
                completion: usage.output_tokens,
            });
        }
        self.write(step);
    }

    /// One shell command and everything that came back from it.
    ///
    /// The output is recorded whole. What the agent was *shown* is smaller —
    /// [`shell::HEAD_MAX`] bytes reach the judge and the model's next turn —
    /// so the call records that bound rather than applying it. A trace that
    /// held only what the agent saw could not answer whether the agent
    /// needed more, which is a question worth being able to ask.
    pub fn command(&mut self, outcome: &Outcome) {
        let mut extra = Map::new();
        extra.insert("schema".to_string(), json!(SHELL_CALL_SCHEMA));
        extra.insert("status".to_string(), json!(outcome.status.to_string()));
        if let Status::Exit(code) = outcome.status {
            extra.insert("exit_code".to_string(), json!(code));
        }
        extra.insert("output_bytes".to_string(), json!(outcome.output.len()));
        extra.insert(
            "shown_bytes".to_string(),
            json!(outcome.head(shell::HEAD_MAX).len()),
        );
        extra.insert("capture_bytes_max".to_string(), json!(shell::OUTPUT_MAX));
        let call = Call {
            id: self.next_call_id(),
            name: "shell".to_string(),
            arguments: json!({
                "command": outcome.proposal.command,
                "workdir": self.workdir,
            }),
            output: outcome.output.clone(),
            outcome: match outcome.status {
                Status::Exit(0) => atif::Outcome::Completed,
                Status::Denied(_) => atif::Outcome::Cancelled,
                _ => atif::Outcome::Failed,
            },
            milliseconds: outcome.elapsed.as_millis() as u64,
            purpose: (!outcome.proposal.why.is_empty()).then(|| outcome.proposal.why.clone()),
            extra,
        };
        self.write(Step::called(call));
    }

    /// One question put to a decision model, and what it answered.
    pub fn decision(&mut self, mut decision: Decision) {
        decision.id = self.next_call_id();
        self.write(Step::called(decision.call()));
    }

    /// Closes the log, so a reader can tell a session that ended from one
    /// that was killed.
    pub fn finish(&mut self, state: &str) {
        if self.failure.is_some() {
            return;
        }
        if let Err(error) = self.log.finish(state) {
            self.failure = Some(format!("trace stopped: {error}"));
        }
    }

    fn next_call_id(&mut self) -> String {
        self.calls += 1;
        format!("call-{}", self.calls)
    }

    /// Appends one step. The first failure stops the recording and keeps
    /// its reason; nothing after it is written, and nothing about it
    /// reaches the conversation.
    fn write(&mut self, step: Step) {
        if self.failure.is_some() {
            return;
        }
        if let Err(error) = self.log.append(&step) {
            self.failure = Some(format!("trace stopped: {error}"));
        }
    }
}

impl Drop for Recorder {
    /// A session that reaches the end of its process says so. One that does
    /// not leaves a log with no closing record, which reads back as
    /// `interrupted` — the record of what it had.
    fn drop(&mut self) {
        self.finish(atif::log::ENDED);
    }
}

/// The typed answers a door returned, as the document records them.
///
/// The whole distribution is kept, not the argmax: a choice at 0.52 and the
/// same choice at 0.98 are different evidence, and the difference is the
/// part a threshold is tuned on.
#[must_use]
pub fn answers_value(answers: &IndexMap<String, Answer>) -> Value {
    let mut out = Map::new();
    for (id, answer) in answers {
        let value = match answer {
            Answer::Noul(noul) => json!({ "type": "noul", "noul": noul.noul }),
            Answer::Choice(choice) => json!({
                "type": "choice",
                "choice": choice.choice,
                "confidence": choice.confidence,
                "probabilities": choice.probabilities,
            }),
            Answer::Score(score) => json!({
                "type": "score",
                "score": score.score,
                "confidence": score.confidence,
                "probabilities": score.probabilities,
            }),
        };
        out.insert(id.clone(), value);
    }
    Value::Object(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::Proposal;
    use std::time::Duration;

    fn an_outcome(command: &str, status: Status, output: &str) -> Outcome {
        Outcome {
            proposal: Proposal {
                command: command.to_string(),
                why: "look around".to_string(),
            },
            status,
            output: output.to_string(),
            elapsed: Duration::from_millis(40),
        }
    }

    /// A recorder writes each step as it happens, and the log reads back as
    /// a document without the session having to end.
    #[test]
    fn a_recorder_writes_steps_as_they_happen() {
        let dir = tempfile::tempdir().unwrap();
        let mut recorder = Recorder::open(dir.path(), "a-model", "stub", "/tmp/repo").unwrap();
        let path = recorder.path().to_path_buf();

        recorder.user("what crates are here");
        recorder.instructions("you are Coder");
        // The same instructions twice are recorded once.
        recorder.instructions("you are Coder");
        recorder.command(&an_outcome("ls crates", Status::Exit(0), "atif\ncoder"));
        recorder.answer(
            "atif and coder",
            Some(Usage {
                input_tokens: 40,
                output_tokens: 9,
            }),
            120,
        );

        // Nothing has closed the log, and it already reads.
        let recording = atif::log::read(&path).unwrap();
        assert_eq!(recording.steps.len(), 4);
        assert!(!recording.ended());
        let document = recording.document();
        assert_eq!(document["final_metrics"]["total_prompt_tokens"], 40);
        assert_eq!(document["final_metrics"]["extra"]["tool_calls_total"], 1);
        assert_eq!(document["extra"]["directive"], "what crates are here");

        drop(recorder);
        let closed = atif::log::read(&path).unwrap();
        assert!(closed.ended());
    }

    /// A shell call keeps its command, its directory, and its whole output,
    /// and says how much of that output the agent was shown.
    #[test]
    fn a_shell_call_keeps_its_output_and_names_what_was_shown() {
        let dir = tempfile::tempdir().unwrap();
        let mut recorder = Recorder::open(dir.path(), "a-model", "stub", "/tmp/repo").unwrap();
        let path = recorder.path().to_path_buf();
        let long = "x".repeat(shell::HEAD_MAX * 2);
        recorder.command(&an_outcome("cargo test", Status::Exit(101), &long));
        recorder.command(&an_outcome("sudo rm -rf /", Status::Denied("no"), ""));
        drop(recorder);

        let recording = atif::log::read(&path).unwrap();
        let first = recording.steps[0].call.as_ref().unwrap();
        assert_eq!(first.output.len(), long.len());
        assert_eq!(first.extra["shown_bytes"], shell::HEAD_MAX);
        assert_eq!(first.extra["output_bytes"], long.len());
        assert_eq!(first.extra["exit_code"], 101);
        assert_eq!(first.outcome, atif::Outcome::Failed);
        assert_eq!(first.arguments["command"], "cargo test");
        assert!(first.arguments["workdir"].is_string());
        assert_eq!(first.purpose.as_deref(), Some("look around"));

        // A refused command never ran, which is not the same as failing.
        let second = recording.steps[1].call.as_ref().unwrap();
        assert_eq!(second.outcome, atif::Outcome::Cancelled);
    }

    /// The switch turns recording off, and the directory variable moves it.
    /// Recording is on unless someone says otherwise.
    #[test]
    fn the_switch_turns_recording_off_and_the_directory_moves_it() {
        let named = Some(OsString::from("/tmp/traces"));
        for off in ["off", "0", "no", "false", "OFF", " off "] {
            assert!(
                resolve(Some(off), named.clone()).is_none(),
                "{off} should turn recording off"
            );
        }
        assert_eq!(
            resolve(None, named.clone()),
            Some(PathBuf::from("/tmp/traces"))
        );
        assert_eq!(
            resolve(Some("on"), named),
            Some(PathBuf::from("/tmp/traces"))
        );
        assert_eq!(resolve(None, None), atif::log::default_dir());
        assert_eq!(
            resolve(None, Some(OsString::new())),
            atif::log::default_dir()
        );
    }

    /// A failed write stops the recording and keeps its reason, and the
    /// conversation is told nothing more than that.
    #[test]
    fn a_failed_write_stops_the_recording_rather_than_the_turn() {
        let dir = tempfile::tempdir().unwrap();
        let mut recorder = Recorder::open(dir.path(), "a-model", "stub", "/tmp/repo").unwrap();
        recorder.user("hello");
        recorder.failure = Some("the disk is full".to_string());
        recorder.answer("hi", None, 1);
        let path = recorder.path().to_path_buf();
        assert_eq!(recorder.failure(), Some("the disk is full"));
        drop(recorder);

        let recording = atif::log::read(&path).unwrap();
        assert_eq!(recording.steps.len(), 1);
        // A stopped recording does not claim the session ended cleanly.
        assert!(!recording.ended());
    }
}
