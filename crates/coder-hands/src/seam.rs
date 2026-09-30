//! The thread one Jev request runs on, so an ask never waits on a frame.
//!
//! [`crate::judge`] holds the window, the questions, the floors, and the
//! answers-to-action table. This module runs the request: the caller
//! hands a state to [`Seam::ask`], which returns at once, and reads what
//! came back from [`Seam::take`] on a later pass. The worker owns a
//! current-thread runtime and blocks on it, the way `jev::BlockingClient`
//! does, so a caller with no runtime of its own asks over a channel.
//!
//! One request runs at a time. An ask made while a request is in flight
//! is recorded as [`crate::judge::Skip::InFlight`] and sent nowhere,
//! which bounds what the seam costs to one request a round trip and
//! keeps a slow answer from queueing windows the desk has already moved
//! past. The trigger in [`crate::judge::Window::ask`] is what keeps that
//! bound from swallowing most of the asks: it holds an ambiguous window
//! back until the window has turned over, so the seam is asked about one
//! window of hand a second rather than three.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::judge::{Counts, DEADLINE, Report, Skip, ask, questions};

/// How many requests the seam keeps in flight.
pub const IN_FLIGHT: usize = 1;

/// What the transcript keeps about the window an ask carried. It comes
/// back beside the answer, because the desk has moved on by then and the
/// record names the window that was asked about.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Meta {
    /// Frames the asked window carried.
    pub window: usize,
    /// The label the rules gave the newest frame, in the gesture module's
    /// own words, which is what its window log carries.
    pub pose: String,
    /// The margin that label was decided on.
    pub margin: Option<f32>,
    /// What the rules did on that frame, by name, and nothing when they
    /// did nothing, which is the ambiguous window's usual answer.
    pub rules: Vec<String>,
    /// Where the hand aimed when the ask went out, in fractions of the
    /// focused screen, so an answer that acts acts where the window
    /// pointed rather than where the hand has since moved.
    pub at: Option<(f32, f32)>,
}

/// What came back for one ask.
#[derive(Debug)]
pub struct Answer {
    /// The window the request was asked over.
    pub meta: Meta,
    /// The answers, or the sentence that says why none arrived.
    pub report: Result<Report, String>,
    /// Whether the round trip finished inside [`DEADLINE`].
    pub met_deadline: bool,
    /// How long the round trip took.
    pub elapsed: Duration,
}

/// One request for the worker.
struct Job {
    meta: Meta,
    state: Value,
}

/// The worker thread, the channels to it, and the counters a caller
/// reports. Dropping it closes the job channel, which ends the thread
/// once the request in flight returns.
#[derive(Debug)]
pub struct Seam {
    jobs: Sender<Job>,
    answers: Receiver<Answer>,
    flight: Arc<AtomicUsize>,
    counts: Counts,
}

impl Seam {
    /// A seam whose worker asks `client` about `model`.
    ///
    /// # Errors
    ///
    /// Returns the error the runtime or the thread failed with.
    pub fn start(client: jev::Client, model: String) -> Result<Seam, std::io::Error> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let (jobs, inbox) = mpsc::channel();
        let (outbox, answers) = mpsc::channel();
        let flight = Arc::new(AtomicUsize::new(0));
        let worker = Worker {
            client,
            model,
            runtime,
            inbox,
            outbox,
            flight: Arc::clone(&flight),
        };
        thread::Builder::new()
            .name("hands-judge".into())
            .spawn(move || worker.run())?;
        Ok(Seam {
            jobs,
            answers,
            flight,
            counts: Counts::default(),
        })
    }

    /// Sends one ask, or names why it sent none. It returns at once
    /// either way, and it never blocks.
    ///
    /// # Errors
    ///
    /// Returns [`Skip::InFlight`] when a request is already running, and
    /// [`Skip::Closed`] when the worker thread is gone. The caller
    /// records either one, because a window the seam did not ask about
    /// belongs in the transcript.
    pub fn ask(&mut self, meta: Meta, state: Value) -> Result<(), Skip> {
        if self.flight.load(Ordering::Relaxed) >= IN_FLIGHT {
            self.counts.skip(Skip::InFlight);
            return Err(Skip::InFlight);
        }
        self.flight.fetch_add(1, Ordering::Relaxed);
        if self.jobs.send(Job { meta, state }).is_err() {
            self.flight.fetch_sub(1, Ordering::Relaxed);
            self.counts.skip(Skip::Closed);
            return Err(Skip::Closed);
        }
        self.counts.asked += 1;
        Ok(())
    }

    /// Every answer that arrived since the last pass, in the order they
    /// arrived. It never blocks.
    pub fn take(&mut self) -> Vec<Answer> {
        let mut answers = Vec::new();
        loop {
            match self.answers.try_recv() {
                Ok(answer) => answers.push(answer),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => return answers,
            }
        }
    }

    /// What the seam did with the windows it was handed: the asks that
    /// went out, and the ones it sent nowhere, by reason. The trigger's
    /// own skips are counted by the caller and added to these.
    #[must_use]
    pub fn counts(&self) -> Counts {
        self.counts
    }

    /// Whether a request is in flight.
    #[must_use]
    pub fn busy(&self) -> bool {
        self.flight.load(Ordering::Relaxed) >= IN_FLIGHT
    }
}

/// Where a person's Jev settings live, under the home directory. The file
/// holds `api_key` and, optionally, `model`, the way every Jev caller in
/// this repository reads it.
const SETTINGS: &str = ".openagents/jev.json";

/// A seam over the Jev this machine can reach, and the model's name for
/// the caller's log line.
///
/// The key is `TYPESAFE_API_KEY` when it is set, and otherwise the
/// `api_key` field of `~/.openagents/jev.json`. The model is
/// `TYPESAFE_DEFAULT_MODEL`, then the file's `model` field, then the `jev`
/// client's default. `TYPESAFE_BASE_URL` points a keyed client at another
/// API root. With no key, the seam asks the OpenAgents hosted decision
/// service (`jev_hosted::resolve`), which needs none here.
///
/// # Errors
///
/// Returns the sentence that says why there is no seam: no Jev at all, a
/// client that did not build, or the thread's error. None of them carries
/// a key.
pub fn configured() -> Result<(Seam, String), String> {
    let file = settings();
    let model = present(std::env::var(jev::env::DEFAULT_MODEL).ok())
        .or_else(|| file.as_ref().and_then(|file| present(file.model.clone())))
        .unwrap_or_else(|| jev::defaults::MODEL.to_string());
    // One resolver for every Jev caller: this machine's key (the variable,
    // then the file) talks to TypeSafe directly; with none, the hosted
    // decision service answers.
    let key = present(std::env::var(jev::env::API_KEY).ok())
        .or_else(|| file.and_then(|file| present(file.api_key)));
    let url = present(std::env::var(jev::env::BASE_URL).ok())
        .unwrap_or_else(|| jev_hosted::DOOR.to_string());
    let dir =
        jev_hosted::openagents_dir().unwrap_or_else(|| std::path::PathBuf::from("/nonexistent"));
    let env = |name: &str| {
        if name == jev::env::API_KEY {
            key.clone()
        } else {
            std::env::var(name).ok()
        }
    };
    let client = jev_hosted::resolve(
        &env,
        &dir,
        &jev_hosted::Door {
            url: &url,
            model: &model,
        },
        &|config| config,
    )
    .map_err(|why| format!("Jev is unavailable: {why}."))?
    .client;
    let seam = Seam::start(client, model.clone())
        .map_err(|error| format!("the seam's thread did not start: {error}."))?;
    Ok((seam, model))
}

/// The two fields of `~/.openagents/jev.json` the seam reads.
#[derive(serde::Deserialize)]
struct Settings {
    api_key: Option<String>,
    model: Option<String>,
}

/// The settings file, when the process has a home directory and the file
/// parses. A file that is absent or malformed reads as no file.
fn settings() -> Option<Settings> {
    let home = std::env::var_os("HOME")?;
    let text = std::fs::read_to_string(std::path::Path::new(&home).join(SETTINGS)).ok()?;
    serde_json::from_str(&text).ok()
}

/// A value that holds more than whitespace, trimmed.
fn present(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// The thread that asks.
struct Worker {
    client: jev::Client,
    model: String,
    runtime: tokio::runtime::Runtime,
    inbox: Receiver<Job>,
    outbox: Sender<Answer>,
    flight: Arc<AtomicUsize>,
}

impl Worker {
    /// One request at a time until the caller drops the seam.
    fn run(self) {
        while let Ok(job) = self.inbox.recv() {
            let started = Instant::now();
            let read =
                self.runtime
                    .block_on(ask(&self.client, &self.model, job.state, questions()));
            let elapsed = started.elapsed();
            self.flight.fetch_sub(1, Ordering::Relaxed);
            let answer = Answer {
                meta: job.meta,
                report: read.map_err(|error| error.to_string()),
                met_deadline: elapsed <= DEADLINE,
                elapsed,
            };
            if self.outbox.send(answer).is_err() {
                return;
            }
        }
    }
}

#[cfg(test)]
#[path = "seam_tests.rs"]
mod tests;
