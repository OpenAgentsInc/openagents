//! The terminal session on hosts without the harness's process supervisor.
//!
//! The shared native harness supervises an agent's process group only on
//! Unix, so a delegated CLI session refuses to start here instead of running
//! without its limits.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::delegate::{Briefing, Cli, Report, Status, Summary, Wrap};
use crate::session::{Capabilities, Session, StopAck};
use crate::stream::Event;

const UNSUPPORTED: &str = "delegating to a coding agent is not supported on this system yet";

/// A terminal session that reports it cannot run on this system.
pub struct CliSession<'a> {
    pub cli: &'a Cli,
    pub deadline: Duration,
    pub steerable: bool,
    pub resume: Option<String>,
    pub transport_bound: Duration,
    clock: Instant,
}

impl<'a> CliSession<'a> {
    #[must_use]
    pub fn new(cli: &'a Cli, _binary: PathBuf, _wrap: &'a Wrap<'a>, _name: &str) -> Self {
        Self {
            cli,
            deadline: cli.deadline,
            steerable: false,
            resume: None,
            transport_bound: crate::transport::BOUND,
            clock: Instant::now(),
        }
    }

    pub async fn shutdown(&mut self) {}

    #[must_use]
    pub fn summaries(&self) -> Vec<Summary> {
        Vec::new()
    }
}

impl Session for CliSession<'_> {
    fn adapter(&self) -> &str {
        self.cli.agent.word()
    }
    fn capabilities(&self) -> Capabilities {
        crate::adapter::capabilities(self.cli.agent).0
    }
    fn session_id(&self) -> Option<String> {
        None
    }
    async fn start(&mut self, _briefing: &Briefing) -> Result<(), String> {
        self.clock = Instant::now();
        Err(UNSUPPORTED.into())
    }
    async fn advance(&mut self, _now_ms: u64) -> bool {
        false
    }
    fn observe(&mut self) -> Vec<Event> {
        Vec::new()
    }
    async fn stop(&mut self, now_ms: u64, reason: &str) -> Result<StopAck, String> {
        Ok(StopAck {
            at_ms: now_ms,
            reason: reason.into(),
            pending: 0,
            cleanup: "nothing was running".into(),
        })
    }
    async fn resume(&mut self, _now_ms: u64, _message: &str) -> Result<(), String> {
        Err(UNSUPPORTED.into())
    }
    async fn steer(&mut self, _now_ms: u64, _message: &str) -> Result<(), String> {
        Err(UNSUPPORTED.into())
    }
    fn report(&mut self) -> Report {
        Report {
            status: Status::Harness(UNSUPPORTED.into()),
            summary: Summary::default(),
            milliseconds: self
                .clock
                .elapsed()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
            stderr: String::new(),
            stream: None,
        }
    }
}
