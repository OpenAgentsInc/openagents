//! The Agent Studio from a live host: a studio [`Source`] over the host's
//! same-user control socket, the one the desktop app pairs and runs tasks
//! through (`openagents_connect::control`). On the host's own computer the
//! socket's peer is the host's owner, so it holds every right; the host
//! still checks each operation's right, and a view offers only what
//! [`Source::rights`] allows.
//!
//! [`Live`] observes only between [`Source::start`] and [`Source::stop`],
//! which Everglade calls on entry and exit, as the Gym hall's boards load
//! only while the player is inside. While it observes, a worker thread asks
//! the host for the studio every [`POLL`]: `studio.snapshot` first, then
//! sequenced `studio.update`s, through the client [`Mirror`]. A missed
//! update or a host that started again refuses as `stale`, and the worker
//! reads a fresh snapshot at once. The same thread sends each intent a
//! panel asks for, with a fresh request identity that a retry after an
//! uncertain answer keeps, and reads a task's review when a panel asks for
//! one. The frame never waits on the socket.

use super::{Answer, Source};
use coder_access::review::TaskReview;
use coder_access::studio::{Mirror, Snapshot, TaskStatus};
use coder_access::{Code, Error, Operation, Outcome, Right};
use openagents_connect::control::{MAX_MESSAGE_BYTES, Op, Reply, Request, Response, VERSION};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

/// How often the worker asks the host for the studio.
pub const POLL: Duration = Duration::from_secs(1);
/// How long one exchange with the host may take.
pub const TIMEOUT: Duration = Duration::from_secs(10);

/// One NIP-HOST operation to the host and its answer.
pub trait Transport: Send {
    /// Sends `operation` under `request`, a 64-hex identity that an exact
    /// retry keeps, and returns the host's answer.
    ///
    /// # Errors
    /// The host's refusal, with its code, or `unavailable` when nothing
    /// answers.
    fn call(&mut self, request: &str, operation: &Operation) -> coder_access::Result<Outcome>;
}

/// The host's control socket as a [`Transport`]: one connection per
/// operation, carrying `openagents.control.v1` length-prefixed JSON.
#[derive(Debug)]
pub struct ControlSocket {
    path: PathBuf,
    next: u64,
}

impl ControlSocket {
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self { path, next: 1 }
    }

    /// One request on the socket and the host's reply.
    #[cfg(unix)]
    fn exchange(&mut self, op: Op) -> coder_access::Result<Reply> {
        use std::io::{Read, Write};
        let unreachable = || Error::new(Code::Unavailable, "the host does not answer its socket");
        let malformed = || Error::new(Code::Malformed, "the host's answer is malformed");
        let id = self.next;
        self.next += 1;
        let body = serde_json::to_vec(&Request::new(id, op)).map_err(|_| malformed())?;
        if body.len() > MAX_MESSAGE_BYTES {
            return Err(Error::new(Code::Bounds, "the request exceeds one message"));
        }
        let mut stream =
            std::os::unix::net::UnixStream::connect(&self.path).map_err(|_| unreachable())?;
        stream
            .set_read_timeout(Some(TIMEOUT))
            .and_then(|()| stream.set_write_timeout(Some(TIMEOUT)))
            .map_err(|_| unreachable())?;
        let length = u32::try_from(body.len()).map_err(|_| malformed())?;
        let mut frame = Vec::with_capacity(4 + body.len());
        frame.extend(length.to_be_bytes());
        frame.extend(body);
        stream
            .write_all(&frame)
            .and_then(|()| stream.flush())
            .map_err(|_| unreachable())?;
        let mut length = [0u8; 4];
        stream.read_exact(&mut length).map_err(|_| unreachable())?;
        let length = u32::from_be_bytes(length) as usize;
        if length > MAX_MESSAGE_BYTES {
            return Err(malformed());
        }
        let mut body = vec![0u8; length];
        stream.read_exact(&mut body).map_err(|_| unreachable())?;
        let response: Response = serde_json::from_slice(&body).map_err(|_| malformed())?;
        if response.v != VERSION || response.id != id {
            return Err(malformed());
        }
        Ok(response.result)
    }

    /// The control socket is a Unix socket here; elsewhere there is none
    /// to reach.
    #[cfg(not(unix))]
    fn exchange(&mut self, _op: Op) -> coder_access::Result<Reply> {
        Err(Error::new(
            Code::Unavailable,
            "the studio reaches a host only over its Unix control socket",
        ))
    }
}

impl Transport for ControlSocket {
    fn call(&mut self, request: &str, operation: &Operation) -> coder_access::Result<Outcome> {
        operation.validate()?;
        let reply = self.exchange(Op::Task {
            request: request.into(),
            operation: operation.clone(),
        })?;
        match reply {
            Reply::Task { outcome } if outcome.answers(operation) => {
                outcome.validate()?;
                Ok(outcome)
            }
            Reply::Refused { code, message } => Err(Error::new(refusal(&code), message)),
            _ => Err(Error::new(
                Code::Malformed,
                "the host answered another operation",
            )),
        }
    }
}

/// The refusal code a reply names, or `unavailable` for one this client
/// does not know.
fn refusal(code: &str) -> Code {
    serde_json::from_value(serde_json::Value::String(code.into())).unwrap_or(Code::Unavailable)
}

/// What the frame asks the worker to do.
enum Job {
    Send { ticket: u64, operation: Operation },
    Review(String),
}

/// What the worker tells the frame.
enum Event {
    Studio(Snapshot),
    Answer(Answer),
    Review(String, Box<TaskReview>),
    /// A read the host refused or did not answer.
    Failed(&'static str, Error),
    /// A read succeeded after a failure.
    Recovered,
}

/// A running worker: the frame's ends of its two channels. Dropping it
/// stops the worker after its current exchange.
struct Worker {
    jobs: Sender<Job>,
    events: Receiver<Event>,
}

/// Makes a transport for each start.
pub type Connect = Box<dyn Fn() -> Box<dyn Transport> + Send>;

/// The Agent Studio from a live host.
pub struct Live {
    connect: Connect,
    rights: Vec<Right>,
    local_runs: bool,
    worker: Option<Worker>,
    /// The newest studio the worker read, not yet handed to the view.
    pending: Option<Snapshot>,
    /// Each task's status in the newest snapshot, so the review of a task
    /// whose status moved is read again.
    statuses: BTreeMap<String, TaskStatus>,
    reviews: BTreeMap<String, TaskReview>,
    /// Reviews asked for and not yet answered.
    asked: BTreeSet<String>,
    answers: Vec<Answer>,
    /// The last read failure reported, so one outage is one answer.
    failing: Option<Error>,
    next_ticket: u64,
}

impl Live {
    /// A source whose worker reaches the host through a fresh transport
    /// from `connect` each time it starts, holding `rights`.
    #[must_use]
    pub fn new(connect: Connect, rights: Vec<Right>) -> Self {
        Self {
            connect,
            rights,
            local_runs: false,
            worker: None,
            pending: None,
            statuses: BTreeMap::new(),
            reviews: BTreeMap::new(),
            asked: BTreeSet::new(),
            answers: Vec::new(),
            failing: None,
            next_ticket: 1,
        }
    }

    /// The host on this computer, through its control socket at `path`.
    /// The socket's peer is the host's owner, which holds every right.
    #[must_use]
    pub fn control(path: PathBuf) -> Self {
        let mut source = Self::new(
            Box::new(move || Box::new(ControlSocket::new(path.clone())) as Box<dyn Transport>),
            Right::ALL.to_vec(),
        );
        source.local_runs = true;
        source
    }

    /// Whether the worker runs.
    #[must_use]
    pub fn observing(&self) -> bool {
        self.worker.is_some()
    }

    fn take(&mut self, event: Event) {
        match event {
            Event::Studio(snapshot) => {
                let statuses: BTreeMap<String, TaskStatus> = snapshot
                    .view
                    .tasks
                    .iter()
                    .map(|task| (task.task.clone(), task.status))
                    .collect();
                // A task whose status moved may have changed its worktree;
                // its review is read afresh.
                let before = std::mem::replace(&mut self.statuses, statuses);
                self.reviews
                    .retain(|task, _| self.statuses.get(task) == before.get(task));
                // A review the host refused is asked for again once its
                // task moves.
                self.asked
                    .retain(|task| self.statuses.get(task) == before.get(task));
                self.pending = Some(snapshot);
            }
            Event::Answer(answer) => {
                if answer.operation == "studio.merge.decide" {
                    // A decision, landed or refused as stale, moves the
                    // worktree or names an old review: read them again.
                    self.reviews.clear();
                    self.asked.clear();
                }
                self.answers.push(answer);
            }
            Event::Review(task, review) => {
                self.asked.remove(&task);
                self.reviews.insert(task, *review);
            }
            Event::Failed(operation, error) => {
                if self.failing.as_ref() != Some(&error) {
                    self.failing = Some(error.clone());
                    self.answers.push(Answer {
                        ticket: 0,
                        operation,
                        result: Err(error),
                    });
                }
            }
            Event::Recovered => self.failing = None,
        }
    }
}

impl Source for Live {
    fn available(&self) -> bool {
        self.worker.is_some() && self.failing.is_none()
    }
    fn local_runs(&self) -> bool {
        self.local_runs
    }
    fn start(&mut self) {
        if self.worker.is_some() {
            return;
        }
        let (jobs, job_queue) = mpsc::channel();
        let (event_queue, events) = mpsc::channel();
        let transport = (self.connect)();
        let spawned = std::thread::Builder::new()
            .name("verse-studio-host".into())
            .spawn(move || run(transport, &job_queue, &event_queue));
        match spawned {
            Ok(_) => self.worker = Some(Worker { jobs, events }),
            Err(error) => self.answers.push(Answer {
                ticket: 0,
                operation: "studio.snapshot",
                result: Err(Error::new(
                    Code::Unavailable,
                    format!("the studio's connection did not start: {error}"),
                )),
            }),
        }
    }

    fn stop(&mut self) {
        self.worker = None;
        self.pending = None;
        self.statuses.clear();
        self.reviews.clear();
        self.asked.clear();
        self.answers.clear();
        self.failing = None;
    }

    fn poll(&mut self, _dt: f32) -> Option<Snapshot> {
        let mut events = Vec::new();
        if let Some(worker) = &self.worker {
            while let Ok(event) = worker.events.try_recv() {
                events.push(event);
            }
        }
        for event in events {
            self.take(event);
        }
        self.pending.take()
    }

    fn review(&mut self, task: &str) -> Option<TaskReview> {
        if let Some(review) = self.reviews.get(task) {
            return Some(review.clone());
        }
        if !self.asked.contains(task)
            && let Some(worker) = &self.worker
            && worker.jobs.send(Job::Review(task.to_owned())).is_ok()
        {
            self.asked.insert(task.to_owned());
        }
        None
    }

    fn rights(&self) -> &[Right] {
        &self.rights
    }

    fn send(&mut self, operation: Operation) -> Result<u64, Error> {
        operation.validate()?;
        let Some(worker) = &self.worker else {
            return Err(Error::new(
                Code::Unavailable,
                "the studio is not connected; it connects while you are in Everglade",
            ));
        };
        let ticket = self.next_ticket;
        if worker.jobs.send(Job::Send { ticket, operation }).is_err() {
            return Err(Error::new(
                Code::Unavailable,
                "the studio's connection stopped",
            ));
        }
        self.next_ticket += 1;
        Ok(ticket)
    }

    fn answers(&mut self) -> Vec<Answer> {
        std::mem::take(&mut self.answers)
    }
}

/// The worker: read the studio, then run the jobs that arrive until the
/// next read is due, until the frame's end goes away.
fn run(mut transport: Box<dyn Transport>, jobs: &Receiver<Job>, events: &Sender<Event>) {
    let mut mirror = Mirror::default();
    loop {
        // A stale refusal forgets the copy, and the snapshot is read at
        // once.
        let mut tries = 0;
        loop {
            tries += 1;
            let operation = mirror.next();
            let name = operation.name();
            let result = transport
                .call(&super::intents::mint(), &operation)
                .and_then(|outcome| mirror.accept(&outcome));
            match result {
                Ok(changed) => {
                    if events.send(Event::Recovered).is_err() {
                        return;
                    }
                    if changed
                        && let Some(snapshot) = mirror.snapshot()
                        && events.send(Event::Studio(snapshot.clone())).is_err()
                    {
                        return;
                    }
                }
                Err(error) => {
                    mirror.refused(&error);
                    if error.code == Code::Stale && tries < 2 {
                        continue;
                    }
                    if events.send(Event::Failed(name, error)).is_err() {
                        return;
                    }
                }
            }
            break;
        }
        let due = Instant::now() + POLL;
        loop {
            let left = due.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            let job = match jobs.recv_timeout(left) {
                Ok(job) => job,
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return,
            };
            let event = match job {
                Job::Send { ticket, operation } => Event::Answer(Answer {
                    ticket,
                    operation: operation.name(),
                    result: deliver(transport.as_mut(), &operation),
                }),
                Job::Review(task) => {
                    match transport.call(
                        &super::intents::mint(),
                        &Operation::OpenReview { task: task.clone() },
                    ) {
                        Ok(Outcome::Review { review }) => Event::Review(task, review),
                        Ok(_) => Event::Failed(
                            "studio.review.open",
                            Error::new(Code::Malformed, "the host answered another operation"),
                        ),
                        Err(error) => Event::Answer(Answer {
                            ticket: 0,
                            operation: "studio.review.open",
                            result: Err(error),
                        }),
                    }
                }
            };
            let intent = matches!(event, Event::Answer(_));
            if events.send(event).is_err() {
                return;
            }
            if intent {
                // What the intent changed shows at once.
                break;
            }
        }
    }
}

/// Sends an intent, and once more under the same request identity when
/// the first answer was lost: the host answers an exact retry with the
/// same outcome and repeats no effect.
fn deliver(transport: &mut dyn Transport, operation: &Operation) -> Result<Outcome, Error> {
    let request = super::intents::mint();
    match transport.call(&request, operation) {
        Err(error) if matches!(error.code, Code::Unavailable | Code::Transport) => {
            transport.call(&request, operation)
        }
        result => result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_access::studio::{
        Activity, Goal, GoalStatus, Repository, Role, Seat, Station, Stream, View,
    };
    use std::sync::{Arc, Mutex};

    /// A host for the source's worker: a studio stream over a view a test
    /// changes, and every intent it was sent.
    #[derive(Clone, Default)]
    struct Host {
        view: Arc<Mutex<View>>,
        stream: Arc<Mutex<Option<Stream>>>,
        sent: Arc<Mutex<Vec<(String, Operation)>>>,
        /// Refuse the next intent's first attempt as if its answer was
        /// lost.
        lose: Arc<Mutex<bool>>,
    }

    impl Host {
        /// Starts the host's process again: another stream.
        fn restart(&self) {
            *self.stream.lock().unwrap() = Some(Stream::new("b1"));
        }
    }

    impl Transport for Host {
        fn call(&mut self, request: &str, operation: &Operation) -> coder_access::Result<Outcome> {
            operation.validate()?;
            let view = self.view.lock().unwrap().clone();
            let mut stream = self.stream.lock().unwrap();
            let stream = stream.get_or_insert_with(|| Stream::new("a0"));
            match operation {
                Operation::StudioSnapshot {} => Ok(Outcome::Studio {
                    snapshot: Box::new(stream.snapshot(view)),
                }),
                Operation::StudioUpdate { stream: id, since } => stream
                    .update(view, id, *since)
                    .map(|update| Outcome::StudioUpdate {
                        update: Box::new(update),
                    })
                    .map_err(|code| Error::new(code, "the studio update was refused")),
                Operation::PauseSeat { seat } => {
                    let mut lose = self.lose.lock().unwrap();
                    self.sent
                        .lock()
                        .unwrap()
                        .push((request.to_owned(), operation.clone()));
                    if std::mem::take(&mut *lose) {
                        return Err(Error::new(Code::Unavailable, "the answer was lost"));
                    }
                    Ok(Outcome::Dispatched {
                        receipt: coder_access::protocol::Receipt {
                            operation: "studio.seat.pause".into(),
                            reference: seat.clone(),
                        },
                    })
                }
                _ => Err(Error::new(Code::Unsupported, "not in this host")),
            }
        }
    }

    fn view() -> View {
        View {
            goals: vec![Goal {
                goal: "g1-0011aabb".into(),
                text: "Add a dark mode".into(),
                workspace: "app".into(),
                lead: "lead".into(),
                status: GoalStatus::Planning,
                final_tasks: 0,
                total_tasks: 0,
                submitted_at: 1_790_000_000,
                spend: Default::default(),
            }],
            seats: vec![Seat {
                seat: "lead".into(),
                role: Role::Lead,
                route: "codex:gpt-6".into(),
                look: "default".into(),
                desk: 0,
                activity: Activity::Idle,
                station: Station::Desk,
                task: None,
                paused: false,
                spend: Default::default(),
            }],
            repositories: vec![Repository {
                workspace: "app".into(),
                goals: 1,
                open_tasks: 0,
            }],
            ..View::default()
        }
    }

    /// Polls `live` until `test` holds of what it returns, or panics.
    fn until<T>(live: &mut Live, mut test: impl FnMut(&mut Live) -> Option<T>) -> T {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(found) = test(live) {
                return found;
            }
            assert!(Instant::now() < deadline, "the live source never caught up");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn source(host: &Host) -> Live {
        let host = host.clone();
        Live::new(
            Box::new(move || Box::new(host.clone()) as Box<dyn Transport>),
            vec![Right::Observe, Right::Operate],
        )
    }

    #[test]
    fn outage_disables_facts_and_only_control_socket_sources_offer_local_runs() {
        let host = Host::default();
        let mut live = source(&host);
        assert!(!live.available() && !live.local_runs());
        live.start();
        assert!(live.available());
        live.take(Event::Failed(
            "studio.snapshot",
            Error::new(Code::Unavailable, "offline"),
        ));
        assert!(!live.available());
        live.take(Event::Recovered);
        assert!(live.available());
        live.stop();
        assert!(!live.available());
        // Construction does not connect or create a host.
        assert!(Live::control(PathBuf::from("/missing/scratch.sock")).local_runs());
    }

    #[test]
    fn the_source_observes_only_between_start_and_stop() {
        let host = Host::default();
        *host.view.lock().unwrap() = view();
        let mut live = source(&host);
        assert!(live.poll(0.0).is_none());
        assert_eq!(
            live.send(Operation::PauseSeat {
                seat: "lead".into()
            })
            .unwrap_err()
            .code,
            Code::Unavailable
        );
        live.start();
        let first = until(&mut live, |live| live.poll(0.0));
        assert_eq!(first.view, view());
        // A change reaches the view through an update.
        host.view.lock().unwrap().goals[0].status = GoalStatus::Running;
        let second = until(&mut live, |live| live.poll(0.0));
        assert_eq!(second.view.goals[0].status, GoalStatus::Running);
        assert_eq!(second.stream, first.stream);
        live.stop();
        assert!(!live.observing());
    }

    #[test]
    fn a_host_that_started_again_is_read_from_a_fresh_snapshot() {
        let host = Host::default();
        *host.view.lock().unwrap() = view();
        let mut live = source(&host);
        live.start();
        let first = until(&mut live, |live| live.poll(0.0));
        {
            // At once, as the worker reads them: it locks the view first.
            let mut view = host.view.lock().unwrap();
            view.seats[0].paused = true;
            host.restart();
        }
        let fresh = until(&mut live, |live| live.poll(0.0));
        assert!(fresh.view.seats[0].paused);
        assert_ne!(fresh.stream, first.stream);
    }

    #[test]
    fn an_intent_whose_answer_was_lost_is_sent_again_under_its_identity() {
        let host = Host::default();
        *host.view.lock().unwrap() = view();
        *host.lose.lock().unwrap() = true;
        let mut live = source(&host);
        live.start();
        until(&mut live, |live| live.poll(0.0));
        let ticket = live
            .send(Operation::PauseSeat {
                seat: "lead".into(),
            })
            .unwrap();
        let answer = until(&mut live, |live| {
            live.poll(0.0);
            live.answers().into_iter().find(|a| a.ticket == ticket)
        });
        assert_eq!(answer.operation, "studio.seat.pause");
        assert!(matches!(answer.result, Ok(Outcome::Dispatched { .. })));
        let sent = host.sent.lock().unwrap().clone();
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[0].0, sent[1].0, "the retry keeps its request identity");
        // An intent the host does not take is refused with its code.
        let ticket = live
            .send(Operation::ResumeSeat {
                seat: "lead".into(),
            })
            .unwrap();
        let refused = until(&mut live, |live| {
            live.poll(0.0);
            live.answers().into_iter().find(|a| a.ticket == ticket)
        });
        assert_eq!(refused.result.unwrap_err().code, Code::Unsupported);
    }
}
