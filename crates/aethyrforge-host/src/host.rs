use crate::*;
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone, Copy, Debug, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Connecting,
    Ready,
    Opening,
    Open,
    Closing,
    Closed,
    Revoked,
}
#[derive(Clone, Debug)]
struct Pending {
    action: Action,
    deadline: u64,
}
#[derive(Clone, Debug, serde::Serialize)]
pub struct Receipt {
    pub request_id: String,
    pub outcome: Outcome,
}

/// Transport validation and bounded lifecycle state, not an authorization service.
pub struct Host {
    pub origin: String,
    pub session: String,
    pub phase: Phase,
    pub status: Option<Status>,
    pub capabilities: Vec<Capability>,
    expected_release: String,
    expected_abi: String,
    expected_format: u16,
    project: Project,
    outgoing: u64,
    incoming: u64,
    pending: BTreeMap<String, Pending>,
    results: VecDeque<Receipt>,
    uncertain: BTreeMap<String, Action>,
}
impl Host {
    pub fn new(
        engine_url: &str,
        session: String,
        release: String,
        abi: String,
        format: u16,
        project: Project,
    ) -> Result<Self, Error> {
        let u = endpoint(engine_url)?;
        if !crate::protocol::id(&session)
            || !crate::protocol::id(&release)
            || !crate::protocol::id(&abi)
            || format == 0
        {
            return Err(Error::Invalid("identity"));
        }
        project.validate()?;
        Ok(Self {
            origin: u.origin().ascii_serialization(),
            session,
            phase: Phase::Connecting,
            status: None,
            capabilities: vec![],
            expected_release: release,
            expected_abi: abi,
            expected_format: format,
            project,
            outgoing: 0,
            incoming: 0,
            pending: BTreeMap::new(),
            results: VecDeque::new(),
            uncertain: BTreeMap::new(),
        })
    }
    fn envelope(&mut self, message: Message) -> Result<Envelope, Error> {
        let e = Envelope {
            protocol: PROTOCOL.into(),
            version: VERSION,
            session: self.session.clone(),
            sequence: self.outgoing + 1,
            message,
        };
        e.encode()?;
        self.outgoing += 1;
        Ok(e)
    }
    pub fn hello(&mut self) -> Result<Envelope, Error> {
        if self.phase != Phase::Connecting || self.outgoing != 0 {
            return Err(Error::Phase);
        }
        self.envelope(Message::Hello {
            versions: vec![VERSION],
        })
    }
    /// Window messages are accepted only during bootstrap, from the configured frame.
    pub fn check_peer(&self, origin: &str, expected_window: bool) -> Result<(), Error> {
        if !expected_window || origin != self.origin {
            Err(Error::Peer)
        } else {
            Ok(())
        }
    }
    pub fn request(&mut self, action: Action, now: u64) -> Result<Envelope, Error> {
        action.validate()?;
        match &action {
            Action::Open { .. } if self.phase == Phase::Ready => (),
            Action::Close if matches!(self.phase, Phase::Ready | Phase::Opening | Phase::Open) => {
                ()
            }
            Action::Open { .. } | Action::Close => return Err(Error::Phase),
            _ if self.phase != Phase::Open => return Err(Error::Phase),
            _ => (),
        }
        if let Some(c) = action.capability() {
            if !self.capabilities.contains(&c) {
                return Err(Error::Unsupported);
            }
        }
        if self.pending.len() >= MAX_PENDING {
            return Err(Error::Busy);
        }
        let request_id = format!("request-{}", self.outgoing + 1);
        let e = self.envelope(Message::Request {
            request_id: request_id.clone(),
            project: self.project.clone(),
            action: action.clone(),
        })?;
        if matches!(action, Action::Open { .. }) {
            self.phase = Phase::Opening;
        }
        if matches!(action, Action::Close) {
            self.phase = Phase::Closing;
        }
        self.pending.insert(
            request_id,
            Pending {
                action: action.clone(),
                deadline: now.saturating_add(if matches!(action, Action::Open { .. }) {
                    OPEN_DEADLINE_MS
                } else {
                    DEADLINE_MS
                }),
            },
        );
        Ok(e)
    }
    pub fn receive(&mut self, text: &str) -> Result<Message, Error> {
        let e = Envelope::decode(text)?;
        if e.session != self.session {
            return Err(Error::Session);
        }
        if e.sequence <= self.incoming {
            return Err(Error::Replay);
        }
        if matches!(self.phase, Phase::Closed | Phase::Revoked) {
            return Err(Error::Phase);
        }
        match &e.message {
            Message::Ready {
                release,
                engine_abi,
                project_format,
                capabilities,
            } => {
                if self.phase != Phase::Connecting || self.outgoing != 1 {
                    return Err(Error::Phase);
                }
                if release != &self.expected_release
                    || engine_abi != &self.expected_abi
                    || *project_format != self.expected_format
                {
                    return Err(Error::Version);
                }
                self.capabilities = capabilities.clone();
                self.phase = Phase::Ready;
            }
            Message::Reply {
                request_id,
                outcome,
            } => {
                let action = self
                    .pending
                    .get(request_id)
                    .map(|p| &p.action)
                    .or_else(|| self.uncertain.get(request_id))
                    .ok_or(Error::Replay)?
                    .clone();
                action.validate_outcome(outcome)?;
                if let Some(s) = &outcome.status {
                    self.check_status(s)?;
                }
                if matches!(action, Action::Open { .. })
                    && outcome.state == ResultState::Applied
                    && outcome.status.is_none()
                {
                    return Err(Error::Invalid("open status"));
                }
                self.pending.remove(request_id);
                self.uncertain.remove(request_id);
                if outcome.state == ResultState::Applied {
                    if let Some(s) = &outcome.status {
                        self.project = s.project.clone();
                        self.status = Some(s.clone());
                    }
                    if matches!(action, Action::Open { .. }) {
                        self.phase = Phase::Open;
                    }
                    if matches!(action, Action::Close) {
                        self.phase = Phase::Closed;
                        self.abandon("The experience closed.");
                    }
                } else {
                    if matches!(action, Action::Open { .. }) {
                        self.phase = Phase::Ready;
                    }
                    if matches!(action, Action::Close) {
                        self.phase = Phase::Open;
                    }
                }
                self.record(request_id.clone(), outcome.clone());
            }
            Message::Status { status } => {
                if self.phase != Phase::Open {
                    return Err(Error::Phase);
                }
                self.check_status(status)?;
                self.status = Some(status.clone());
                self.project = status.project.clone();
            }
            Message::AuthRequired {
                request_id,
                message,
            } => {
                if self.pending.remove(request_id).is_none() {
                    return Err(Error::Replay);
                }
                self.record(
                    request_id.clone(),
                    Outcome {
                        state: ResultState::Rejected,
                        message: message.clone(),
                        status: None,
                        detail: None,
                    },
                );
                if self.phase == Phase::Opening {
                    self.phase = Phase::Ready;
                }
            }
            Message::Revoked { message } => {
                self.phase = Phase::Revoked;
                self.abandon(message);
                self.status = None;
                self.capabilities.clear();
            }
            _ => return Err(Error::Phase),
        }
        self.incoming = e.sequence;
        Ok(e.message)
    }
    fn check_status(&self, s: &Status) -> Result<(), Error> {
        if !s.project.same_branch(&self.project) || s.project.revision < self.project.revision {
            return Err(Error::Invalid("project status"));
        }
        Ok(())
    }
    fn record(&mut self, id: String, outcome: Outcome) {
        self.results.retain(|r| r.request_id != id);
        self.results.push_back(Receipt {
            request_id: id,
            outcome,
        });
        while self.results.len() > MAX_RESULTS {
            if let Some(r) = self.results.pop_front() {
                self.uncertain.remove(&r.request_id);
            }
        }
    }
    pub fn expire(&mut self, now: u64) {
        let ids: Vec<_> = self
            .pending
            .iter()
            .filter(|(_, p)| p.deadline <= now)
            .map(|(id, _)| id.clone())
            .collect();
        for id in ids {
            if let Some(p) = self.pending.remove(&id) {
                self.uncertain.insert(id.clone(), p.action);
                self.record(
                    id,
                    Outcome {
                        state: ResultState::Unknown,
                        message: "The result has not arrived. Check its status before retrying."
                            .into(),
                        status: None,
                        detail: None,
                    },
                );
            }
        }
    }
    fn abandon(&mut self, message: &str) {
        let ids: Vec<_> = self.pending.keys().cloned().collect();
        self.pending.clear();
        self.uncertain.clear();
        for id in ids {
            self.record(
                id,
                Outcome {
                    state: ResultState::Unknown,
                    message: message.into(),
                    status: None,
                    detail: None,
                },
            );
        }
    }
    /// End the local connection without claiming unresolved work was cancelled.
    pub fn shutdown(&mut self) {
        if self.phase != Phase::Revoked {
            self.phase = Phase::Closed;
        }
        self.abandon("The world closed before its result arrived.");
        self.status = None;
        self.capabilities.clear();
    }
    pub fn receipt(&self, id: &str) -> Option<&Receipt> {
        self.results.iter().find(|r| r.request_id == id)
    }
    pub fn pending(&self) -> usize {
        self.pending.len()
    }
    pub fn receipts(&self) -> &VecDeque<Receipt> {
        &self.results
    }
    pub fn project(&self) -> &Project {
        &self.project
    }
}
