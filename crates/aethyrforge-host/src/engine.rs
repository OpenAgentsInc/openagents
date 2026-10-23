use crate::*;
use std::collections::VecDeque;

/// Engine-side wire validation. The engine still decides authorization and execution.
pub struct Engine {
    session: String,
    capabilities: Vec<Capability>,
    incoming: u64,
    outgoing: u64,
    project: Option<Project>,
    pending: VecDeque<(String, Project, Action)>,
    results: VecDeque<(String, Project, Action, Outcome)>,
    closed: bool,
}
impl Engine {
    pub fn accept(
        hello: &str,
        release: String,
        abi: String,
        format: u16,
        capabilities: Vec<Capability>,
    ) -> Result<(Self, Envelope), Error> {
        let e = Envelope::decode(hello)?;
        if !matches!(e.message, Message::Hello { .. }) || e.sequence != 1 {
            return Err(Error::Phase);
        }
        let mut engine = Self {
            session: e.session,
            capabilities: capabilities.clone(),
            incoming: 1,
            outgoing: 0,
            project: None,
            pending: VecDeque::new(),
            results: VecDeque::new(),
            closed: false,
        };
        let ready = engine.send(Message::Ready {
            release,
            engine_abi: abi,
            project_format: format,
            capabilities,
        })?;
        Ok((engine, ready))
    }
    fn send(&mut self, message: Message) -> Result<Envelope, Error> {
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
    pub fn receive(&mut self, text: &str) -> Result<Message, Error> {
        if self.closed {
            return Err(Error::Phase);
        }
        let e = Envelope::decode(text)?;
        if e.session != self.session {
            return Err(Error::Session);
        }
        if e.sequence <= self.incoming {
            return Err(Error::Replay);
        }
        let Message::Request {
            request_id,
            project,
            action,
        } = &e.message
        else {
            return Err(Error::Phase);
        };
        if let Action::Authorize { grant } = action {
            if grant.session != self.session || &grant.project != project {
                return Err(Error::Invalid("grant binding"));
            }
        }
        // Bind request identity to its sequence, so evicting bounded results cannot
        // allow a prior request ID to execute again with a fresh sequence.
        if request_id != &format!("request-{}", e.sequence) {
            return Err(Error::Replay);
        }
        if matches!(action, Action::Open { .. })
            && self
                .pending
                .iter()
                .any(|(_, _, a)| matches!(a, Action::Open { .. }))
        {
            return Err(Error::Phase);
        }
        if self.pending.iter().any(|(id, _, _)| id == request_id)
            || self.results.iter().any(|(id, _, _, _)| id == request_id)
        {
            return Err(Error::Replay);
        }
        if action
            .capability()
            .is_some_and(|c| !self.capabilities.contains(&c))
        {
            return Err(Error::Unsupported);
        }
        if self.pending.len() >= MAX_PENDING {
            return Err(Error::Busy);
        }
        if let Some(current) = &self.project {
            if !current.same_branch(project) || current.revision != project.revision {
                return Err(Error::Invalid("stale project"));
            }
            if matches!(action, Action::Open { .. }) {
                return Err(Error::Phase);
            }
        } else if !matches!(action, Action::Open { .. } | Action::Close) {
            return Err(Error::Phase);
        }
        self.pending
            .push_back((request_id.clone(), project.clone(), action.clone()));
        self.incoming = e.sequence;
        Ok(e.message)
    }
    pub fn reply(&mut self, id: &str, outcome: Outcome) -> Result<Envelope, Error> {
        outcome.validate()?;
        let index = self
            .pending
            .iter()
            .position(|(i, _, _)| i == id)
            .ok_or(Error::Replay)?;
        let (_, project, action) = self.pending.get(index).ok_or(Error::Replay)?;
        action.validate_outcome(&outcome)?;
        if let Some(status) = &outcome.status {
            if !status.project.same_branch(project) || status.project.revision < project.revision {
                return Err(Error::Invalid("project result"));
            }
        }
        if outcome.state == ResultState::Applied
            && matches!(action, Action::Open { .. })
            && outcome.status.is_none()
        {
            return Err(Error::Invalid("open status"));
        }
        let result = self.send(Message::Reply {
            request_id: id.into(),
            outcome: outcome.clone(),
        })?;
        let (_, project, action) = self.pending.remove(index).ok_or(Error::Replay)?;
        if outcome.state == ResultState::Applied {
            if let Some(status) = &outcome.status {
                self.project = Some(status.project.clone());
            }
            if matches!(action, Action::Close) {
                self.closed = true;
                self.pending.clear();
            }
        }
        self.results
            .push_back((id.into(), project, action, outcome));
        while self.results.len() > MAX_RESULTS {
            self.results.pop_front();
        }
        Ok(result)
    }
    pub fn status(&mut self, status: Status) -> Result<Envelope, Error> {
        if self.closed {
            return Err(Error::Phase);
        }
        let current = self.project.as_ref().ok_or(Error::Phase)?;
        if !status.project.same_branch(current) || status.project.revision < current.revision {
            return Err(Error::Invalid("project status"));
        }
        let e = self.send(Message::Status {
            status: status.clone(),
        })?;
        self.project = Some(status.project);
        Ok(e)
    }
    pub fn lookup(&self, id: &str) -> Option<&Outcome> {
        self.results
            .iter()
            .find(|(i, _, _, _)| i == id)
            .map(|(_, _, _, o)| o)
    }
    pub fn auth_required(&mut self, id: &str, message: String) -> Result<Envelope, Error> {
        if self.closed {
            return Err(Error::Phase);
        }
        let index = self
            .pending
            .iter()
            .position(|(i, _, _)| i == id)
            .ok_or(Error::Replay)?;
        let e = self.send(Message::AuthRequired {
            request_id: id.into(),
            message: message.clone(),
        })?;
        let (_, project, action) = self.pending.remove(index).ok_or(Error::Replay)?;
        self.results.push_back((
            id.into(),
            project,
            action,
            Outcome {
                state: ResultState::Rejected,
                message,
                status: None,
                detail: None,
            },
        ));
        while self.results.len() > MAX_RESULTS {
            self.results.pop_front();
        }
        Ok(e)
    }
    pub fn revoke(&mut self, message: String) -> Result<Envelope, Error> {
        let e = self.send(Message::Revoked { message })?;
        self.closed = true;
        self.pending.clear();
        self.results.clear();
        Ok(e)
    }
}
