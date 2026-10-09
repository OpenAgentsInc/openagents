//! Credential-free task chat state over admitted task and history clients.
use crate::coder_tab::{Choice, Mode};
use crate::conversation::{self, Row};
use coder_connect::protocol::{Observation, Query};
use coder_history::{
    CatalogCursor, CatalogRequest, RecordChunk, TranscriptCursor, TranscriptRequest,
};
use coder_host::access::protocol::{CommandAction, Operation, Outcome, QueueEdit, TaskQueue};
use nostr::activity_summary::{ActivitySummary, Attention, Phase};
use openagents_chat::basic_chats::Spawned;
use rust_native::style::Style;
use rust_native::{Element, Node, TextRole};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    Activity {
        task: String,
    },
    History {
        query: Query,
    },
    Operation {
        request: String,
        operation: Operation,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    Activity(ActivitySummary),
    History(Observation),
    Operation(Outcome),
    /// The computer does not know the operation, as an older host asked
    /// for `task.review`.
    Unsupported,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Stop,
    Send,
    Queue,
    Steer,
    Approve,
    Deny,
    /// A decision panel control ([`crate::decision`]).
    Decide(crate::decision::Control),
    Retry,
    Earlier,
    Latest,
    EditQueue,
    DoneQueue,
    EditQueued(String),
    RemoveQueued(String),
    SendQueuedNow(String),
    MoveQueuedUp(String),
    NextQueue,
    PreviousQueue,
}

impl Action {
    /// Whether a click on this control may end on the revision after the
    /// one it began on ([`rust_native::Press`]): the controls that must
    /// work while the task streams. A queued message's controls name it.
    #[must_use]
    pub fn late(&self) -> bool {
        matches!(
            self,
            Self::Stop
                | Self::Approve
                | Self::Deny
                | Self::Decide(_)
                | Self::SendQueuedNow(_)
                | Self::RemoveQueued(_)
        )
    }
}

/// What a row button does and what it does it to: two equal targets do the
/// same thing, so a click begun on one revision may end on the next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub action: Action,
    pub task: String,
    /// The question or approval answered, as its summary says it.
    pub about: Option<String>,
}

pub struct Session {
    pub binding: Spawned,
    pub summary: Option<ActivitySummary>,
    pub queue: Option<TaskQueue>,
    pub error: Option<String>,
    pub revision: u64,
    pub actions: BTreeMap<String, Action>,
    pub editing: Option<String>,
    rows: Vec<Row>,
    sources: Vec<String>,
    chunks: Vec<RecordChunk>,
    cursor: Option<TranscriptCursor>,
    previous: Option<u64>,
    catalog: Option<CatalogCursor>,
    poll: Instant,
    next_catalog: Instant,
    stage: u8,
    reading: bool,
    queue_page: usize,
    leasing: bool,
    pending: BTreeMap<u64, Request>,
    failed: Option<Request>,
    next_ticket: u64,
    /// The finished task's change for the "What changed" card.
    reviewer: crate::changes::Reviewer,
    /// The completed summary's sequence the reviewer follows.
    reviewed: Option<u64>,
    /// The question or approval the task waits on, as its summary's
    /// attention and headline say it, and the decision panel's state. A
    /// summary that only bumps its sequence keeps the panel's page.
    decision: Option<((Attention, String), crate::decision::Flow)>,
}
impl Session {
    pub fn new(binding: Spawned, now: Instant) -> Self {
        Self {
            binding,
            summary: None,
            queue: None,
            error: None,
            revision: 0,
            actions: BTreeMap::new(),
            editing: None,
            rows: vec![],
            sources: vec![],
            chunks: vec![],
            cursor: None,
            previous: None,
            catalog: None,
            poll: now,
            next_catalog: now,
            stage: 0,
            reading: false,
            queue_page: 0,
            leasing: false,
            pending: BTreeMap::new(),
            failed: None,
            next_ticket: 1,
            reviewer: crate::changes::Reviewer::new(),
            reviewed: None,
            decision: None,
        }
    }
    pub fn busy(&self) -> bool {
        self.pending.values().any(mutation)
    }
    pub fn active(&self) -> bool {
        self.summary
            .as_ref()
            .is_some_and(|s| matches!(s.phase, Phase::Running | Phase::Waiting | Phase::Queued))
    }
    /// Whether the task's summary says the work finished.
    #[must_use]
    pub fn finished(&self) -> bool {
        self.summary
            .as_ref()
            .is_some_and(|summary| summary.phase == Phase::Completed)
    }
    pub fn next_wake(&self, now: Instant) -> Instant {
        self.poll.max(now + Duration::from_millis(100))
    }
    pub fn mode(&self) -> Mode {
        Mode::of(
            self.summary.as_ref().map(|s| s.phase),
            self.summary.as_ref().map(|s| s.attention),
        )
    }
    pub fn placeholder(&self) -> &'static str {
        if self.editing.is_some() {
            return "Edit your queued message";
        }
        match self.mode() {
            Mode::Send => "Message Coder…",
            Mode::Queue => "Queue a message for Coder's next turn…",
            Mode::Answer => "Answer Coder…",
        }
    }
    /// The question or approval the task waits on: its summary's
    /// attention and headline.
    fn asking(&self) -> Option<(Attention, &str)> {
        let summary = self.summary.as_ref()?;
        (summary.phase == Phase::Waiting
            && matches!(summary.attention, Attention::Input | Attention::Approval))
        .then_some((summary.attention, summary.headline.as_str()))
    }
    /// Follow the summary with a decision panel: a new one for a new
    /// question or approval, none once the task stops waiting.
    fn sync_decision(&mut self) {
        use crate::decision::Flow;
        let Some((attention, headline)) = self.asking() else {
            self.decision = None;
            return;
        };
        if self
            .decision
            .as_ref()
            .is_some_and(|((at, text), _)| *at == attention && text == headline)
        {
            return;
        }
        let flow = if attention == Attention::Approval {
            Flow::approval(headline)
        } else {
            Flow::question(headline)
        };
        self.decision = Some(((attention, headline.to_owned()), flow));
    }
    /// The decision panel's state while the task waits on a question or
    /// an approval.
    #[must_use]
    pub fn decision(&self) -> Option<&crate::decision::Flow> {
        let (attention, headline) = self.asking()?;
        self.decision
            .as_ref()
            .filter(|((at, text), _)| *at == attention && text == headline)
            .map(|(_, flow)| flow)
    }
    /// Carry a decision step out: send the answer once every page is
    /// answered, or redraw the moved panel.
    fn decided(&mut self, step: crate::decision::Step, now: u64) -> Option<(u64, Request)> {
        match step {
            crate::decision::Step::Stay => None,
            crate::decision::Step::Moved => {
                self.revision += 1;
                None
            }
            crate::decision::Step::Done(text) => {
                self.submit(&text, Some(CommandAction::Answer), now)
            }
        }
    }
    pub fn steer_choice(&self) -> Option<Choice> {
        Choice::offered(self.summary.as_ref().map(|summary| summary.phase))
            .into_iter()
            .find(|choice| *choice != Choice::Queue)
    }
    fn request(&mut self, request: Request) -> Option<(u64, Request)> {
        if mutation(&request)
            && self
                .failed
                .as_ref()
                .is_some_and(|failed| mutation(failed) && failed != &request)
        {
            return None;
        }
        if if mutation(&request) {
            self.busy()
        } else {
            self.pending.values().any(|pending| !mutation(pending))
        } {
            return None;
        }
        if mutation(&request) {
            self.revision += 1;
        }
        let ticket = self.next_ticket;
        self.next_ticket += 1;
        self.pending.insert(ticket, request.clone());
        Some((ticket, request))
    }
    pub fn tick(&mut self, now: Instant) -> Option<(u64, Request)> {
        if self.busy() || self.pending.values().any(|pending| !mutation(pending)) {
            return None;
        }
        // A finished task's change: read once, again now and then to notice
        // a moved worktree, and published when the person asks.
        let completed = self
            .summary
            .as_ref()
            .filter(|summary| summary.phase == Phase::Completed)
            .map(|summary| summary.sequence);
        if completed.is_some() && completed != self.reviewed {
            self.reviewed = completed;
            self.reviewer.reset(Some(&self.binding.task));
            self.revision += 1;
        }
        if completed.is_some() && self.reviewer.unsupported() {
            let diff = self.unified_diff().map(str::to_owned);
            let before = self.reviewer.revision();
            self.reviewer.set_legacy(diff.as_deref());
            if self.reviewer.revision() != before {
                self.revision += 1;
            }
        }
        if let Some(need) = self.reviewer.tick(now, completed.is_some(), true) {
            let operation = match need {
                crate::changes::Need::Read { task } => Operation::ReviewTask { task },
                crate::changes::Need::Publish {
                    task,
                    base,
                    head_commit,
                    head,
                } => Operation::PublishTask {
                    task,
                    base,
                    head_commit,
                    head,
                },
            };
            self.revision += 1;
            return self.request(Request::Operation {
                request: coder_host::access::protocol::random_id(),
                operation,
            });
        }
        if now < self.poll {
            return None;
        }
        self.poll = now
            + Duration::from_millis(
                if self
                    .summary
                    .as_ref()
                    .is_some_and(|s| matches!(s.phase, Phase::Running | Phase::Waiting))
                {
                    200
                } else {
                    1000
                },
            );
        let stage = self.stage;
        self.stage = (self.stage + 1) % 4;
        let request = match stage {
            0 => Request::Activity {
                task: self.binding.task.clone(),
            },
            1 if now >= self.next_catalog || self.catalog.is_some() => {
                self.next_catalog = now + Duration::from_secs(2);
                Request::History {
                    query: Query::Catalog(CatalogRequest {
                        cursor: self.catalog.clone(),
                        limit: 32,
                    }),
                }
            }
            2 => self.queue_request(
                if self.leasing
                    && self
                        .queue
                        .as_ref()
                        .and_then(|queue| queue.lease.as_ref())
                        .is_some_and(|lease| lease.expires_at <= unix_now().saturating_add(15))
                {
                    QueueEdit::Lease {}
                } else {
                    QueueEdit::List {}
                },
            ),
            _ if !self.reading && !self.sources.is_empty() => Request::History {
                query: Query::Page(TranscriptRequest {
                    source_id: self.sources.last()?.clone(),
                    cursor: self.cursor.clone(),
                    max_bytes: 32 * 1024,
                    end: self.cursor.is_none().then_some(u64::MAX),
                }),
            },
            _ => Request::Activity {
                task: self.binding.task.clone(),
            },
        };
        self.request(request)
    }
    fn queue_request(&self, edit: QueueEdit) -> Request {
        Request::Operation {
            request: coder_host::access::protocol::random_id(),
            operation: Operation::QueueTask {
                task: self.binding.task.clone(),
                edit,
            },
        }
    }
    pub fn retry(&mut self) -> Option<(u64, Request)> {
        self.request(self.failed.clone()?)
    }
    pub fn pending_request(&self, ticket: u64) -> Option<&Request> {
        self.pending.get(&ticket)
    }
    /// A verified refusal ends this attempt; the unsent draft remains editable.
    pub fn refused(&mut self, ticket: u64, reason: String) {
        let Some(request) = self.pending.remove(&ticket) else {
            return;
        };
        if mutation(&request) || self.failed.as_ref().is_none_or(|failed| !mutation(failed)) {
            self.error = Some(reason);
            self.failed = None;
            self.revision += 1;
        }
        if let Request::Operation {
            operation:
                Operation::QueueTask {
                    edit: QueueEdit::Lease {} | QueueEdit::Release {},
                    ..
                },
            ..
        } = request
        {
            self.leasing = false;
            self.editing = None;
        }
    }
    pub fn source_changed(&mut self, ticket: u64) {
        if !matches!(self.pending.get(&ticket), Some(Request::History { .. })) {
            return;
        }
        self.pending.retain(|_, request| mutation(request));
        self.sources.clear();
        self.chunks.clear();
        self.rows.clear();
        self.cursor = None;
        self.previous = None;
        self.catalog = None;
        self.reading = false;
        self.stage = 1;
        self.next_catalog = Instant::now();
        self.poll = Instant::now();
        if self
            .failed
            .as_ref()
            .is_none_or(|request| !mutation(request))
        {
            self.failed = None;
            self.error = Some("This task changed. Showing the latest steps.".into());
        }
        self.revision += 1;
    }
    pub fn submit(
        &mut self,
        text: &str,
        choice: Option<CommandAction>,
        now: u64,
    ) -> Option<(u64, Request)> {
        if self.busy() || text.trim().is_empty() || text.len() > 16 * 1024 {
            return None;
        }
        if let Some(command) = &self.editing
            && choice.is_none()
        {
            return self.request(self.queue_request(QueueEdit::Edit {
                command: command.clone(),
                text: text.into(),
            }));
        }
        let summary = self.summary.as_ref()?;
        let action = choice.unwrap_or(match self.mode() {
            Mode::Send => CommandAction::Send,
            Mode::Queue => CommandAction::Queue,
            Mode::Answer => CommandAction::Answer,
        });
        if matches!(action, CommandAction::Steer | CommandAction::Interrupt)
            && !matches!(
                summary.phase,
                Phase::Running | Phase::Waiting | Phase::Queued
            )
        {
            return None;
        }
        if action == CommandAction::Answer && self.mode() != Mode::Answer {
            return None;
        }
        let emulate = if action == CommandAction::Steer {
            self.steer_choice()?.command().1
        } else {
            false
        };
        let command = crate::outbox::Draft {
            task: &self.binding.task,
            action,
            based_on: summary.sequence,
            text,
            emulate,
        }
        .command(now);
        let request = command.command.clone();
        self.request(Request::Operation {
            request,
            operation: Operation::CommandTask { command },
        })
    }
    pub fn action(&mut self, action: Action, text: &str, now: u64) -> Option<(u64, Request)> {
        self.sync_decision();
        match action {
            Action::Retry => self.retry(),
            // A question's panel takes the typed answer for its page; an
            // approval's answer in the person's own words goes as it is.
            Action::Send
                if self.editing.is_none()
                    && self
                        .decision()
                        .is_some_and(|flow| flow.kind() == crate::decision::Kind::Question) =>
            {
                if self.busy() {
                    return None;
                }
                let step = self.decision.as_mut()?.1.answer_typed(text);
                self.decided(step, now)
            }
            Action::Decide(control) => {
                if self.busy() {
                    return None;
                }
                let step = self.decision.as_mut()?.1.control(control);
                self.decided(step, now)
            }
            Action::Send => self.submit(text, None, now),
            Action::Queue => self.submit(text, Some(CommandAction::Queue), now),
            Action::Steer => self.submit(text, Some(CommandAction::Steer), now),
            Action::Stop => self.submit(
                "Stopped from the OpenAgents chat.",
                Some(CommandAction::Interrupt),
                now,
            ),
            Action::Approve | Action::Deny
                if self
                    .summary
                    .as_ref()
                    .is_some_and(|s| s.attention == Attention::Approval) =>
            {
                self.submit(
                    if action == Action::Approve {
                        "Approved."
                    } else {
                        "Denied."
                    },
                    Some(CommandAction::Answer),
                    now,
                )
            }
            Action::EditQueue => self.request(self.queue_request(QueueEdit::Lease {})),
            Action::DoneQueue => self.request(self.queue_request(QueueEdit::Release {})),
            Action::RemoveQueued(command) => {
                self.request(self.queue_request(QueueEdit::Remove { command }))
            }
            Action::SendQueuedNow(command) => {
                self.request(self.queue_request(QueueEdit::SendNow { command }))
            }
            Action::MoveQueuedUp(command) => {
                let mut order: Vec<_> = self
                    .queue
                    .as_ref()?
                    .items
                    .iter()
                    .filter(|item| !item.priority)
                    .map(|item| item.command.clone())
                    .collect();
                let index = order.iter().position(|id| *id == command)?;
                if index == 0 {
                    return None;
                }
                order.swap(index - 1, index);
                self.request(self.queue_request(QueueEdit::Reorder { commands: order }))
            }
            Action::EditQueued(command) => {
                if !self.leasing
                    || !self
                        .queue
                        .as_ref()?
                        .items
                        .iter()
                        .any(|item| item.command == command && item.text.is_some())
                {
                    return None;
                }
                self.editing = Some(command);
                self.revision += 1;
                None
            }
            Action::NextQueue => {
                self.queue_page += 1;
                self.revision += 1;
                None
            }
            Action::PreviousQueue => {
                self.queue_page = self.queue_page.saturating_sub(1);
                self.revision += 1;
                None
            }
            Action::Earlier => {
                let end = self.previous?;
                self.pending.retain(|_, request| mutation(request));
                self.reading = true;
                self.request(Request::History {
                    query: Query::Page(TranscriptRequest {
                        source_id: self.sources.last()?.clone(),
                        cursor: None,
                        max_bytes: 32 * 1024,
                        end: Some(end),
                    }),
                })
            }
            Action::Latest => {
                self.pending.retain(|_, request| mutation(request));
                self.reading = false;
                self.cursor = None;
                self.rows.clear();
                self.chunks.clear();
                self.poll = Instant::now();
                self.stage = 3;
                self.revision += 1;
                None
            }
            _ => None,
        }
    }
    /// Returns whether a submitted composer draft was durably accepted.
    pub fn outcome(&mut self, ticket: u64, result: Result<Answer, String>) -> bool {
        let Some(request) = self.pending.remove(&ticket) else {
            return false;
        };
        if mutation(&request) {
            self.revision += 1;
        }
        // The change's reads and publications answer the reviewer, which
        // says what failed on the card.
        if let Request::Operation { operation, .. } = &request {
            match (operation, &result) {
                (
                    Operation::ReviewTask { .. },
                    Ok(Answer::Operation(Outcome::Review { review })),
                ) if Outcome::Review {
                    review: review.clone(),
                }
                .answers(operation) =>
                {
                    self.reviewer.read(Ok((**review).clone()), Instant::now());
                    self.revision += 1;
                    return false;
                }
                (Operation::ReviewTask { .. }, Ok(Answer::Unsupported)) => {
                    self.reviewer.read(
                        Err(crate::changes::ReadFailure::Unsupported),
                        Instant::now(),
                    );
                    self.revision += 1;
                    return false;
                }
                (Operation::ReviewTask { .. }, _) => {
                    let why = match &result {
                        Err(error) => error.clone(),
                        _ => "Couldn't load the changes. Try again.".into(),
                    };
                    self.reviewer.read(
                        Err(crate::changes::ReadFailure::Failed(why)),
                        Instant::now(),
                    );
                    self.revision += 1;
                    return false;
                }
                (
                    Operation::PublishTask { .. },
                    Ok(Answer::Operation(Outcome::Published { publication })),
                ) if Outcome::Published {
                    publication: publication.clone(),
                }
                .answers(operation) =>
                {
                    self.reviewer.published(Ok((**publication).clone()));
                    self.revision += 1;
                    return false;
                }
                (Operation::PublishTask { .. }, _) => {
                    self.reviewer.published(Err(match &result {
                        Err(error) => error.clone(),
                        _ => "Couldn't publish the change. Try again.".into(),
                    }));
                    self.revision += 1;
                    return false;
                }
                _ => {}
            }
        }
        let answer = match result {
            Ok(answer) => answer,
            Err(error) => {
                self.fail(request, error);
                return false;
            }
        };
        let mut accepted = false;
        match (&request, answer) {
            (Request::Activity { task }, Answer::Activity(summary))
                if summary.subject == *task
                    && summary.host == self.binding.host
                    && summary.subject_kind == nostr::activity_summary::SubjectKind::Task =>
            {
                if self
                    .summary
                    .as_ref()
                    .is_none_or(|old| old.sequence <= summary.sequence)
                {
                    if self.summary.as_ref().is_none_or(|old| {
                        old.sequence != summary.sequence
                            || old.phase != summary.phase
                            || old.attention != summary.attention
                            || old.headline != summary.headline
                    }) {
                        self.revision += 1
                    }
                    self.summary = Some(summary);
                }
            }
            (
                Request::History {
                    query: Query::Catalog(_),
                },
                Answer::History(Observation::Catalog(page)),
            ) => {
                self.catalog = page.next;
                if let Some(chat) = page.entries.iter().find(|chat| {
                    !chat.archived && chat.native_id.as_deref() == Some(&self.binding.task)
                }) && let Some(source) = &chat.source_id
                {
                    self.catalog = None;
                    if self.sources.last() != Some(source) {
                        if self.sources.len() == 200 {
                            self.sources.clear();
                            self.rows.clear();
                        }
                        self.sources.push(source.clone());
                        self.chunks.clear();
                        self.cursor = None;
                        self.previous = None;
                        self.stage = 3;
                    }
                }
            }
            (
                Request::History {
                    query: Query::Page(query),
                },
                Answer::History(Observation::Page(page)),
            ) if query.source_id == page.source_id => {
                if self.sources.last() != Some(&page.source_id) {
                    return false;
                }
                let Some(segment) = self
                    .sources
                    .iter()
                    .position(|source| *source == page.source_id)
                else {
                    return false;
                };
                for chunk in &page.chunks {
                    self.chunks
                        .retain(|old| old.offset < chunk.offset || old.offset >= chunk.end_offset);
                    if let Some(old) = self
                        .chunks
                        .iter_mut()
                        .find(|old| old.offset == chunk.offset)
                    {
                        *old = chunk.clone();
                    } else {
                        self.chunks.push(chunk.clone());
                    }
                }
                self.chunks.sort_by_key(|chunk| chunk.offset);
                while self.chunks.len() > 512
                    || self
                        .chunks
                        .iter()
                        .map(|chunk| chunk.raw_base64.len())
                        .sum::<usize>()
                        > 384 * 1024
                {
                    if self.reading {
                        self.chunks.pop();
                    } else {
                        self.chunks.remove(0);
                    }
                }
                let mut found = conversation::rows(&self.chunks);
                for row in &mut found {
                    row.segment = segment as u8;
                }
                let previous_rows = self.rows.clone();
                for row in found.into_iter().filter(|row| segment == 0 || !row.carried) {
                    if let Some(old) = self.rows.iter_mut().find(|old| {
                        (old.segment, old.offset, old.part) == (row.segment, row.offset, row.part)
                    }) {
                        *old = row;
                    } else {
                        self.rows.push(row);
                    }
                }
                self.rows
                    .sort_by_key(|row| (row.segment, row.offset, row.part));
                while self.rows.len() > 240
                    || self.rows.iter().map(row_bytes).sum::<usize>() > 160 * 1024
                {
                    if self.reading {
                        self.rows.pop();
                    } else {
                        self.rows.remove(0);
                    }
                }
                if self.rows != previous_rows {
                    self.revision += 1
                }
                if query.end.is_some() {
                    self.previous = page.previous;
                }
                if !self.reading {
                    self.cursor = Some(page.next);
                }
            }
            (Request::Operation { operation, .. }, Answer::Operation(outcome))
                if outcome.answers(operation)
                    && outcome.validate().is_ok()
                    && !matches!(&outcome, Outcome::Dispatched { receipt } if receipt.reference != self.binding.task) =>
            {
                if let Outcome::Queue { queue } = outcome {
                    if self
                        .queue
                        .as_ref()
                        .is_some_and(|old| old.revision > queue.revision)
                    {
                        return false;
                    }
                    if queue
                        .lease
                        .as_ref()
                        .is_none_or(|lease| lease.expires_at <= unix_now())
                    {
                        self.leasing = false;
                        self.editing = None;
                    }
                    if self.queue.as_ref() != Some(&queue) {
                        self.revision += 1
                    }
                    self.queue = Some(queue);
                    if let Operation::QueueTask { edit, .. } = operation {
                        match edit {
                            QueueEdit::Lease {} => self.leasing = true,
                            QueueEdit::Release {} => {
                                self.leasing = false;
                                self.editing = None;
                            }
                            QueueEdit::Edit { .. } => {
                                self.editing = None;
                                accepted = true;
                            }
                            _ => {}
                        }
                    }
                } else {
                    accepted = matches!(operation, Operation::CommandTask { command } if command.action != CommandAction::Interrupt);
                }
                self.stage = 0;
                self.poll = Instant::now();
                if mutation(&request) || self.failed.as_ref() == Some(&request) {
                    self.error = None;
                    self.failed = None;
                    self.revision += 1;
                }
            }
            _ => {
                self.fail(request, "Something went wrong. Try again.".into());
            }
        }
        accepted
    }
    fn fail(&mut self, request: Request, error: String) {
        if self.failed.as_ref().is_none_or(|failed| !mutation(failed)) || mutation(&request) {
            self.error = Some(error);
            self.failed = Some(request);
            self.revision += 1;
        }
    }
    /// The finished task's change, for the "What changed" card. A computer
    /// that reviews no change gets the last diff its transcript recorded,
    /// with no revisions.
    #[must_use]
    pub fn reviewer(&self) -> Option<&crate::changes::Reviewer> {
        self.finished().then_some(&self.reviewer)
    }

    /// The same, to fill syntax spans, refresh, or ask to publish.
    pub fn reviewer_mut(&mut self) -> Option<&mut crate::changes::Reviewer> {
        if !self.finished() {
            return None;
        }
        Some(&mut self.reviewer)
    }

    /// The last unified diff in this task's loaded rows, when one was recorded.
    #[must_use]
    pub fn unified_diff(&self) -> Option<&str> {
        self.rows.iter().rev().find_map(|row| {
            let text = match &row.entry {
                conversation::Entry::Message { text, .. } => text.as_str(),
                conversation::Entry::Tool { body, .. } => body.as_str(),
                conversation::Entry::Delegate { .. } => return None,
            };
            crate::changes::extract(text)
        })
    }
    pub fn editing_text(&self) -> Option<&str> {
        let id = self.editing.as_ref()?;
        self.queue
            .as_ref()?
            .items
            .iter()
            .find(|item| &item.command == id)?
            .text
            .as_deref()
    }
    /// What the row button `key` does now, and to what.
    #[must_use]
    pub fn target(&self, key: &str) -> Option<Target> {
        let action = self.actions.get(key)?.clone();
        let about = match action {
            Action::Approve | Action::Deny => self
                .summary
                .as_ref()
                .filter(|s| s.attention == Attention::Approval)
                .map(|s| s.headline.clone()),
            // The question and the page a decision control was drawn for.
            Action::Decide(_) => self
                .decision()
                .zip(self.asking())
                .map(|(flow, (_, headline))| format!("{}:{headline}", flow.page())),
            _ => None,
        };
        Some(Target {
            action,
            task: self.binding.task.clone(),
            about,
        })
    }
    pub fn rows(&mut self) -> Vec<Node<()>> {
        self.actions.clear();
        self.actions.insert("task-steer".into(), Action::Steer);
        let mut rows = conversation::project_rows(&self.rows);
        // The question or approval the task waits on is the decision panel,
        // in place of its state line.
        self.sync_decision();
        if let Some((_, flow)) = &self.decision {
            let panel = flow.view("task", !self.busy());
            for (key, control) in panel.controls {
                let action = match control {
                    crate::decision::Control::Pick(0)
                        if flow.kind() == crate::decision::Kind::Approval =>
                    {
                        Action::Approve
                    }
                    crate::decision::Control::Pick(_)
                        if flow.kind() == crate::decision::Kind::Approval =>
                    {
                        Action::Deny
                    }
                    control => Action::Decide(control),
                };
                self.actions.insert(key, action);
            }
            rows.push(panel.node);
        } else {
            rows.push(status(
                "task-state",
                self.summary
                    .as_ref()
                    .map_or("Loading Coder task…", |s| &s.headline),
            ));
        }
        if let Some(error) = &self.error {
            rows.push(status("task-error", error));
        }
        if self.failed.is_some() {
            rows.push(self.button("task-retry", "Retry", Action::Retry));
        }
        if self.previous.is_some() {
            rows.push(self.button("task-earlier", "Load earlier steps", Action::Earlier));
        }
        if self.reading {
            rows.push(self.button("task-latest", "Latest steps", Action::Latest));
        }
        if self
            .summary
            .as_ref()
            .is_some_and(|s| matches!(s.phase, Phase::Running | Phase::Waiting | Phase::Queued))
        {
            rows.push(self.button("task-stop", "Stop Coder", Action::Stop));
        }
        if self
            .queue
            .as_ref()
            .is_some_and(|queue| !queue.items.is_empty())
        {
            rows.push(self.button(
                "task-edit-queue",
                if self.leasing {
                    "Done editing queue"
                } else {
                    "Edit queue"
                },
                if self.leasing {
                    Action::DoneQueue
                } else {
                    Action::EditQueue
                },
            ));
            let queue = self.queue.as_ref().unwrap().clone();
            self.queue_page = self.queue_page.min(queue.items.len().saturating_sub(1) / 8);
            for (index, item) in queue
                .items
                .iter()
                .enumerate()
                .skip(self.queue_page * 8)
                .take(8)
            {
                rows.push(status(
                    &format!("task-queue-{index}"),
                    item.text
                        .as_deref()
                        .unwrap_or("Message queued on another device"),
                ));
                if self.leasing && item.text.is_some() {
                    for (suffix, label, action) in [
                        ("edit", "Edit", Action::EditQueued(item.command.clone())),
                        (
                            "remove",
                            "Remove",
                            Action::RemoveQueued(item.command.clone()),
                        ),
                        (
                            "now",
                            "Send now",
                            Action::SendQueuedNow(item.command.clone()),
                        ),
                        ("up", "Move up", Action::MoveQueuedUp(item.command.clone())),
                    ] {
                        rows.push(self.button(
                            &format!("task-queue-{index}-{suffix}"),
                            label,
                            action,
                        ));
                    }
                }
            }
            if self.queue_page > 0 {
                rows.push(self.button(
                    "task-queue-previous",
                    "Previous queued messages",
                    Action::PreviousQueue,
                ));
            }
            if (self.queue_page + 1) * 8 < queue.items.len() {
                rows.push(self.button(
                    "task-queue-next",
                    "More queued messages",
                    Action::NextQueue,
                ));
            }
        }
        rows
    }
    /// A task's control (Stop Coder, Approve, Deny, Retry, the steps'
    /// paging): as wide as its words in the transcript, as the phone draws
    /// its Coder controls (#10075, #10091).
    fn button(&mut self, key: &str, label: &str, action: Action) -> Node<()> {
        self.actions.insert(key.into(), action);
        Node {
            key: key.into(),
            style: Style {
                intrinsic_width: Some(true),
                ..Style::default()
            },
            element: Element::Button {
                shortcut: None,
                label: label.into(),
                enabled: !self.busy(),
                icon: None,
                intent: (),
            },
        }
    }
}
fn status(key: &str, text: &str) -> Node<()> {
    Node {
        key: key.into(),
        style: Style::default(),
        element: Element::Text {
            value: text.into(),
            role: TextRole::Status,
        },
    }
}
fn row_bytes(row: &Row) -> usize {
    match &row.entry {
        conversation::Entry::Message { text, .. } => text.len(),
        conversation::Entry::Tool { name, detail, body } => name.len() + detail.len() + body.len(),
        conversation::Entry::Delegate {
            agent,
            session,
            error,
        } => agent.len() + session.len() + error.as_ref().map_or(0, String::len),
    }
}

fn mutation(request: &Request) -> bool {
    // A review and a publication belong to the change card, which keeps
    // its own state; they never hold the composer.
    matches!(request, Request::Operation { operation, .. } if !matches!(
        operation,
        Operation::QueueTask { edit: QueueEdit::List {}, .. }
            | Operation::ReviewTask { .. }
            | Operation::PublishTask { .. }
    ))
}
pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use nostr::activity_summary::SubjectKind;

    fn session() -> Session {
        let mut session = Session::new(
            Spawned {
                host: "a".repeat(64),
                task: "b".repeat(64),
                project: Some("scratch".into()),
                at: None,
            },
            Instant::now(),
        );
        session.summary = Some(summary(4, Phase::Running, Attention::None));
        session
    }
    fn summary(sequence: u64, phase: Phase, attention: Attention) -> ActivitySummary {
        ActivitySummary {
            host: "a".repeat(64),
            subject_kind: SubjectKind::Task,
            subject: "b".repeat(64),
            sequence,
            phase,
            attention,
            headline: "Scratch task".into(),
            updated_at: unix_now(),
        }
    }
    fn command(request: &Request) -> &coder_host::TaskCommand {
        let Request::Operation {
            operation: Operation::CommandTask { command },
            ..
        } = request
        else {
            panic!("command")
        };
        command
    }
    fn dispatched(request: &Request) -> Answer {
        let Request::Operation { operation, .. } = request else {
            panic!("operation")
        };
        let operation = match operation {
            Operation::CommandTask { .. } => "task.command",
            _ => panic!("command"),
        };
        Answer::Operation(Outcome::Dispatched {
            receipt: coder_host::access::protocol::Receipt {
                operation: operation.into(),
                reference: "b".repeat(64),
            },
        })
    }
    #[test]
    fn a_background_read_does_not_block_queueing_and_the_acknowledged_bytes_retry_exactly() {
        let mut session = session();
        let (read, _) = session
            .request(Request::Activity {
                task: session.binding.task.clone(),
            })
            .unwrap();
        let (ticket, request) = session
            .submit("keep these spaces  ", None, unix_now())
            .unwrap();
        assert_eq!(command(&request).action, CommandAction::Queue);
        assert_eq!(command(&request).based_on, 4);
        assert_eq!(command(&request).text, "keep these spaces  ");
        session.outcome(ticket, Err("unreached".into()));
        session.outcome(read, Err("background read failed".into()));
        assert_eq!(session.error.as_deref(), Some("unreached"));
        assert!(
            session
                .submit("a second command", None, unix_now())
                .is_none()
        );
        let (retry, same) = session.retry().unwrap();
        assert_eq!(same, request);
        assert!(session.outcome(retry, Ok(dispatched(&same))));
        assert!(session.error.is_none());
        assert!(session.failed.is_none());
    }
    #[test]
    fn activity_rejects_another_task_host_and_an_older_revision() {
        let mut session = session();
        for (host, task, seq) in [
            ("c".repeat(64), "b".repeat(64), 5),
            ("a".repeat(64), "c".repeat(64), 5),
            ("a".repeat(64), "b".repeat(64), 3),
        ] {
            let (ticket, _) = session
                .request(Request::Activity {
                    task: session.binding.task.clone(),
                })
                .unwrap();
            let mut answer = summary(seq, Phase::Completed, Attention::Completed);
            answer.host = host;
            answer.subject = task;
            session.outcome(ticket, Ok(Answer::Activity(answer)));
            assert_eq!(session.summary.as_ref().unwrap().sequence, 4);
        }
    }
    #[test]
    fn questions_and_approvals_use_the_same_answer_commands_as_the_phone() {
        let mut session = session();
        session.summary = Some(summary(8, Phase::Waiting, Attention::Input));
        assert_eq!(session.mode(), Mode::Answer);
        let (ticket, request) = session
            .action(Action::Send, "Use the second option.", unix_now())
            .unwrap();
        assert_eq!(command(&request).action, CommandAction::Answer);
        session.refused(ticket, "This question was already answered.".into());
        assert!(!session.busy());
        assert!(session.failed.is_none());
        session.summary = Some(summary(10, Phase::Waiting, Attention::Approval));
        let (ticket, approved) = session.action(Action::Approve, "", unix_now()).unwrap();
        assert_eq!(command(&approved).action, CommandAction::Answer);
        assert_eq!(command(&approved).text, "Approved.");
        assert!(session.outcome(ticket, Ok(dispatched(&approved))));
        let (_, denied) = session.action(Action::Deny, "", unix_now()).unwrap();
        assert_eq!(command(&denied).text, "Denied.");
    }
    /// A waiting task's question or approval is the decision panel in
    /// place of its state line, and its controls answer it (#10469).
    #[test]
    fn the_decision_panel_answers_a_waiting_task() {
        use crate::decision::{Control, Kind};
        let mut session = session();
        let mut waiting = summary(8, Phase::Waiting, Attention::Approval);
        waiting.headline = "May I delete slugs.py?".into();
        session.summary = Some(waiting.clone());
        let rows = session.rows();
        assert!(rows.iter().any(|row| row.key == "task-decision"));
        assert!(rows.iter().all(|row| row.key != "task-state"));
        assert_eq!(
            session.decision().map(|flow| flow.kind()),
            Some(Kind::Approval)
        );
        assert_eq!(session.actions.get("task-approve"), Some(&Action::Approve));
        assert_eq!(session.actions.get("task-deny"), Some(&Action::Deny));
        // A number key's pick: 2 is Deny, and nothing wider than once.
        assert!(!session.decision().unwrap().takes_number(3));
        let (ticket, denied) = session
            .action(Action::Decide(Control::Pick(1)), "", unix_now())
            .unwrap();
        assert_eq!(command(&denied).action, CommandAction::Answer);
        assert_eq!(command(&denied).text, "Denied.");
        assert!(session.outcome(ticket, Ok(dispatched(&denied))));
        // A summary that only bumps its sequence keeps the panel and the
        // controls' target.
        waiting.sequence = 9;
        session.summary = Some(waiting);
        session.rows();
        let target = session.target("task-approve").unwrap();
        assert_eq!(target.about.as_deref(), Some("May I delete slugs.py?"));
        // Once the task runs again the state line returns.
        session.summary = Some(summary(10, Phase::Running, Attention::None));
        let rows = session.rows();
        assert!(rows.iter().any(|row| row.key == "task-state"));
        assert!(session.decision().is_none());
    }
    #[test]
    fn stop_and_steer_keep_the_observed_revision_and_emulated_steering_choice() {
        let mut session = session();
        let (ticket, stop) = session.action(Action::Stop, "", unix_now()).unwrap();
        assert_eq!(command(&stop).action, CommandAction::Interrupt);
        assert_eq!(command(&stop).based_on, 4);
        assert!(
            !session.outcome(ticket, Ok(dispatched(&stop))),
            "stop never clears the composer"
        );
        let (_, steer) = session
            .action(Action::Steer, "Take this direction.", unix_now())
            .unwrap();
        assert_eq!(command(&steer).action, CommandAction::Steer);
        assert!(command(&steer).emulate);
    }
    fn page(session: &mut Session, chunks: Vec<RecordChunk>, end: Option<u64>) {
        let source = session.sources.last().unwrap().clone();
        let next = chunks.last().map_or(0, |chunk| chunk.end_offset);
        let (ticket, _) = session
            .request(Request::History {
                query: Query::Page(TranscriptRequest {
                    source_id: source.clone(),
                    cursor: None,
                    max_bytes: 32 * 1024,
                    end,
                }),
            })
            .unwrap();
        session.outcome(
            ticket,
            Ok(Answer::History(Observation::Page(
                coder_history::TranscriptPage {
                    source_id: source.clone(),
                    incarnation: "scratch".into(),
                    snapshot_bytes: next,
                    chunks,
                    next: TranscriptCursor {
                        source_id: source,
                        incarnation: "scratch".into(),
                        offset: next,
                        record_offset: next,
                        record_index: 1,
                        prefix_sha256: "c".repeat(64),
                    },
                    has_more: false,
                    pending_line: false,
                    notices: vec![],
                    previous: None,
                },
            ))),
        );
    }
    fn chunk(bytes: &[u8], offset: u64, complete: bool) -> RecordChunk {
        RecordChunk {
            id: format!("record-{offset}"),
            index: 0,
            record_offset: 0,
            offset,
            end_offset: offset + bytes.len() as u64,
            raw_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
            complete,
            oversized: false,
            readable: None,
        }
    }
    #[test]
    fn split_atif_records_merge_into_the_phone_projection_without_duplicate_rows() {
        let mut session = session();
        session.sources.push("scratch-source".into());
        let record = b"{\"record\":\"step\",\"step\":{\"source\":\"Agent\",\"message\":\"Which option should I use?\"}}\n";
        page(&mut session, vec![chunk(&record[..35], 0, false)], None);
        assert!(session.rows.is_empty());
        page(&mut session, vec![chunk(&record[35..], 35, true)], None);
        assert_eq!(session.rows.len(), 1);
        assert!(
            matches!(&session.rows[0].entry, conversation::Entry::Message { text, .. } if text == "Which option should I use?")
        );
        page(&mut session, vec![chunk(record, 0, true)], None);
        assert_eq!(session.rows.len(), 1);
    }
    #[test]
    fn latest_discards_an_outstanding_earlier_read_and_replaced_sources_refresh() {
        let mut session = session();
        session.sources.push("scratch-source".into());
        session.previous = Some(20);
        let (earlier, _) = session.action(Action::Earlier, "", unix_now()).unwrap();
        session.action(Action::Latest, "", unix_now());
        assert!(session.pending_request(earlier).is_none());
        assert!(!session.reading);
        let (ticket, _) = session
            .request(Request::History {
                query: Query::Catalog(CatalogRequest::default()),
            })
            .unwrap();
        session.source_changed(ticket);
        assert!(session.sources.is_empty());
        assert!(session.tick(Instant::now()).is_some());
    }
    #[test]
    fn foreign_queued_text_cannot_be_edited_and_an_expired_lease_stops_editing() {
        let mut session = session();
        session.leasing = true;
        session.queue = Some(TaskQueue {
            task: session.binding.task.clone(),
            revision: 1,
            lease: None,
            items: vec![coder_host::access::protocol::QueueItem {
                command: "d".repeat(64),
                device: "e".repeat(64),
                text: None,
                priority: false,
            }],
        });
        session.action(Action::EditQueued("d".repeat(64)), "", unix_now());
        assert!(session.editing.is_none());
        let (ticket, _) = session
            .request(session.queue_request(QueueEdit::List {}))
            .unwrap();
        session.outcome(
            ticket,
            Ok(Answer::Operation(Outcome::Queue {
                queue: session.queue.clone().unwrap(),
            })),
        );
        assert!(!session.leasing);
        assert!(session.editing.is_none());
    }
}
