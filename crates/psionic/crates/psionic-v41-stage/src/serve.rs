//! The stage's side of Pylon's stage-group control protocol.
//!
//! One thread reads the front's lines and answers pings and checks at once,
//! so a stage stays healthy while it computes. Everything else, the steps,
//! cancellations, fences, and stage state, runs in order on the thread that
//! owns the [`StageWorker`].
//!
//! Stage state travels one of two ways, as the front's transport says:
//!
//! - **Front** (W10): the front sends every stage its step and the next
//!   stage's address. Listener threads read length-prefixed frames from the
//!   stage before, and every stage acknowledges every step.
//! - **Direct** (W12, `--data-listen`): the front sends each step only to
//!   stage 0. A `link` message gives every stage the generation's key and
//!   the next stage's link address; each stage sends its state to the next
//!   over an authenticated [`stage_link`] connection with the step's
//!   settings as the frame's meta, reports `forwarded` with the bytes, and
//!   only the last stage acknowledges, with its token. A lost link is
//!   reported as `link_failed`, a refused frame as `refused`, and stage 0
//!   starts a cancel frame down the chain.
//!
//! Readers hand stage state to the worker's thread through one queue bounded
//! by [`InboxBounds`], in frames and in bytes. A reader that finds the queue
//! full waits, so it stops reading its connection and the stage before it
//! feels the pressure; after [`InboxBounds::wait`] it gives up. The front's
//! messages, link cancellations, and link reports skip the bound and go
//! before queued state, so a cancellation or a fence is served next.
//!
//! The stage announces its [`Budget`] in its `hello`. Stage 0 checks each
//! request against it before any stage runs it, and a request it can't
//! serve, or state a stage can't send, ends with a `rejected` message that
//! fails only that request: the stage stays healthy.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt::Display,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

use stage_link::{Frame, Hop, Key, Kind, Limits, Listener, Refusal};

use psionic_models::{
    deepseek_v41_stage::V41GroupManifest,
    stage_state::{
        CodecLimits, SectionKind, StageExecutor, StageMessage, StageRefusal, StageWorker, StepKind,
        WorkerError, decode, encode,
    },
};
use serde_json::{Value, json};
use tokenizers::Tokenizer;

use crate::{
    AnswerParser, Budget, PROTOCOL, PromptMode, Refused, RequestLimit, Sampling, StageBackend,
    TextStream, Vocabulary, budget, sample,
};

/// Decoded frames from earlier stages kept while their step hasn't come; their
/// bytes are bounded by [`InboxBounds::bytes`] too.
const BUFFERED_FRAMES: usize = 32;

/// How much stage state may wait between a stage's readers and its worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InboxBounds {
    /// Frames queued for the worker.
    pub frames: usize,
    /// Their bytes. One frame larger than this still passes when nothing
    /// else waits, so a full prefill step always fits.
    pub bytes: usize,
    /// How long a reader waits for room before it drops its connection.
    pub wait: Duration,
}

impl Default for InboxBounds {
    /// Four frames and 512 MiB: two of the largest messages the codec
    /// allows. The front runs one step at a time, so a healthy group keeps
    /// at most one frame here.
    fn default() -> Self {
        Self {
            frames: 4,
            bytes: 2 * CodecLimits::default().max_message_bytes,
            wait: Duration::from_secs(30),
        }
    }
}

/// What the stage tells the front and the stages after it.
pub struct Stage<'a> {
    /// The name the front gave it.
    pub name: String,
    /// The secret the front gave it.
    pub token: String,
    /// The weights digest it announces.
    pub weights: String,
    /// The group manifest.
    pub manifest: V41GroupManifest,
    /// This stage's index in the manifest.
    pub index: u16,
    /// Turns the prompt into tokens (stage 0) and tokens into text (last).
    pub vocabulary: &'a Vocabulary,
    /// The token that ends an answer.
    pub eos: u32,
    /// The prompt when the front sends none.
    pub default_token: u32,
    /// The front's control address.
    pub front: String,
    /// Where to listen for state from the front transport.
    pub listen: String,
    /// Where to listen for the direct transport's link; `None` serves only
    /// the front transport.
    pub data_listen: Option<String>,
    /// The link address the stage announces, when it differs from the bound
    /// one (a machine's internal name).
    pub data_advertise: Option<String>,
    /// The runtime it announces, with its backend.
    pub runtime: &'static str,
    /// The numerical profile it runs ([`crate::profile_name`]).
    pub profile: String,
    /// Where to record what the stage does.
    pub events: Option<PathBuf>,
    /// Qualification (#12405): where each step's logits (last stage) and
    /// observations are written; the ack names the files.
    pub record: Option<PathBuf>,
    /// Positions one request may use, as the executor was built with.
    pub max_context: usize,
    /// The operator's cap on prompt tokens.
    pub max_input: Option<usize>,
    /// How much stage state may wait for the worker.
    pub inbox: InboxBounds,
}

enum Event {
    Front(Value),
    Frame(Vec<u8>),
    /// A frame from the previous stage's direct link at a generation.
    Link(u64, Frame),
    /// A link of a generation broke; `peer` is `next` or `prev`.
    LinkLost {
        generation: u64,
        peer: &'static str,
        reason: String,
    },
    /// The link to the next stage opened, or couldn't be opened.
    Connected(u64, Result<stage_link::Sender, String>),
    Closed,
}

/// Why a reader couldn't queue stage state.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Blocked {
    /// The worker stopped taking events.
    Closed,
    /// The queue stayed full for the reader's whole wait.
    Full(String),
}

#[derive(Default)]
struct Queue {
    control: VecDeque<Event>,
    data: VecDeque<(Event, usize)>,
    bytes: usize,
    open: bool,
    /// The most frames and bytes that were ever queued, for tests.
    peak: (usize, usize),
}

/// The worker's events: control events, unbounded and served first, and
/// stage state, bounded by [`InboxBounds`].
#[derive(Clone)]
struct Events {
    shared: Arc<(Mutex<Queue>, Condvar)>,
    bounds: InboxBounds,
}

impl Events {
    fn new(bounds: InboxBounds) -> Self {
        let queue = Queue {
            open: true,
            ..Queue::default()
        };
        Self {
            shared: Arc::new((Mutex::new(queue), Condvar::new())),
            bounds,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Queue> {
        self.shared
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Queues a control event; false once the worker has stopped.
    fn control(&self, event: Event) -> bool {
        let mut queue = self.lock();
        if !queue.open {
            return false;
        }
        queue.control.push_back(event);
        drop(queue);
        self.shared.1.notify_all();
        true
    }

    /// Queues `bytes` of stage state, waiting for room up to the bound's
    /// wait.
    fn data(&self, event: Event, bytes: usize) -> Result<(), Blocked> {
        let bounds = self.bounds;
        let full = |q: &mut Queue| {
            q.open
                && !q.data.is_empty()
                && (q.data.len() >= bounds.frames || q.bytes.saturating_add(bytes) > bounds.bytes)
        };
        let queue = self.lock();
        let (mut queue, timeout) = self
            .shared
            .1
            .wait_timeout_while(queue, bounds.wait, full)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !queue.open {
            return Err(Blocked::Closed);
        }
        if timeout.timed_out() && full(&mut queue) {
            return Err(Blocked::Full(format!(
                "the stage's inbox held {} frames and {} bytes for {} ms",
                queue.data.len(),
                queue.bytes,
                bounds.wait.as_millis()
            )));
        }
        queue.bytes += bytes;
        queue.data.push_back((event, bytes));
        let depth = (queue.data.len(), queue.bytes);
        queue.peak = (queue.peak.0.max(depth.0), queue.peak.1.max(depth.1));
        drop(queue);
        self.shared.1.notify_all();
        Ok(())
    }

    /// The next event, control first.
    fn recv(&self) -> Option<Event> {
        let queue = self.lock();
        let mut queue = self
            .shared
            .1
            .wait_while(queue, |q| {
                q.open && q.control.is_empty() && q.data.is_empty()
            })
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let event = if let Some(event) = queue.control.pop_front() {
            Some(event)
        } else if let Some((event, bytes)) = queue.data.pop_front() {
            queue.bytes -= bytes;
            Some(event)
        } else {
            None
        };
        drop(queue);
        self.shared.1.notify_all();
        event
    }

    /// Stops taking events and wakes every waiting reader.
    fn close(&self) {
        let mut queue = self.lock();
        queue.open = false;
        queue.control.clear();
        queue.data.clear();
        queue.bytes = 0;
        drop(queue);
        self.shared.1.notify_all();
    }

    /// The most frames and bytes ever queued.
    fn peak(&self) -> (usize, usize) {
        self.lock().peak
    }
}

#[derive(Clone)]
struct Writer(Arc<Mutex<TcpStream>>);

impl Writer {
    fn send(&self, message: &Value) {
        let line = format!("{message}\n");
        if let Ok(mut stream) = self.0.lock() {
            let _ = stream.write_all(line.as_bytes());
        }
    }
}

struct Log {
    stage: String,
    path: Option<PathBuf>,
}

impl Log {
    fn record(&self, event: &str, generation: u64, data: Value) {
        let mut line = json!({
            "event": event,
            "stage": self.stage,
            "pid": std::process::id(),
            "generation": generation,
        });
        if let (Some(map), Value::Object(extra)) = (line.as_object_mut(), data) {
            map.extend(extra);
        }
        if event == "refused" {
            let _ = writeln!(
                std::io::stderr(),
                "psionic-v41-stage {}: {line}",
                self.stage
            );
        }
        if let Some(path) = &self.path
            && let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
        {
            // One write, so lines from several stages never interleave.
            let _ = file.write_all(format!("{line}\n").as_bytes());
        }
    }
}

/// Writes one length-prefixed frame.
///
/// # Errors
///
/// Any write error.
pub fn write_frame(stream: &mut TcpStream, bytes: &[u8]) -> std::io::Result<()> {
    let len = u32::try_from(bytes.len())
        .map_err(|_| std::io::Error::other("a stage message over 4 GiB"))?;
    stream.write_all(&len.to_le_bytes())?;
    stream.write_all(bytes)?;
    stream.flush()
}

fn read_frames(mut stream: TcpStream, events: &Events) {
    let limit = CodecLimits::default().max_message_bytes;
    loop {
        let mut len = [0u8; 4];
        if stream.read_exact(&mut len).is_err() {
            return;
        }
        let len = u32::from_le_bytes(len) as usize;
        if len > limit {
            return;
        }
        let mut bytes = vec![0; len];
        if stream.read_exact(&mut bytes).is_err() {
            return;
        }
        // A full inbox drops the connection: the stage before sees its
        // write fail and reports it.
        if events.data(Event::Frame(bytes), len).is_err() {
            return;
        }
    }
}

/// A step the front asked for.
#[derive(Clone)]
struct Step {
    generation: u64,
    request: String,
    attempt: u32,
    stage: u16,
    step: u64,
    prompt: Option<Value>,
    /// Qualification: the prompt's token IDs, instead of messages.
    prompt_ids: Option<Vec<u32>>,
    token: Option<u32>,
    sampling: Option<Sampling>,
    next: Option<String>,
    /// The output limit ends the answer after this step.
    last: bool,
    /// How stage 0 renders the prompt and the last stage parses the answer.
    mode: PromptMode,
    /// The most answer tokens the front may ask for (step 0 only).
    max_tokens: Option<u64>,
}

impl Step {
    fn parse(message: &Value) -> Option<Self> {
        Some(Self {
            generation: message["generation"].as_u64()?,
            request: message["request_id"].as_str()?.to_owned(),
            attempt: u32::try_from(message["attempt"].as_u64()?).ok()?,
            stage: u16::try_from(message["stage"].as_u64()?).ok()?,
            step: message["step"].as_u64()?,
            prompt: message.get("prompt").cloned(),
            prompt_ids: match message.get("prompt_ids") {
                None => None,
                Some(ids) => Some(
                    ids.as_array()?
                        .iter()
                        .map(|t| t.as_u64().and_then(|t| u32::try_from(t).ok()))
                        .collect::<Option<Vec<u32>>>()?,
                ),
            },
            token: message["token"]
                .as_u64()
                .and_then(|t| u32::try_from(t).ok()),
            sampling: message
                .get("sampling")
                .and_then(|s| serde_json::from_value(s.clone()).ok()),
            next: message["next"].as_str().map(str::to_owned),
            last: message["final"].as_bool().unwrap_or(false),
            mode: PromptMode::of(message).ok()?,
            max_tokens: message["max_tokens"].as_u64(),
        })
    }

    /// The settings a direct link carries to the next stage with the state.
    fn meta(&self) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "sampling": self.sampling,
            "final": self.last,
            "mode": self.mode.as_str(),
        }))
        .unwrap_or_default()
    }

    fn matches(&self, message: &StageMessage) -> bool {
        message.request_id == self.request
            && message.attempt == self.attempt
            && message.step == self.step
    }
}

/// Why stage 0 won't start a step.
enum Rejected {
    /// The request is larger than the group serves.
    Limit(RequestLimit),
    /// The request can't be served as sent.
    Invalid(Refused),
}

impl From<Refused> for Rejected {
    fn from(refused: Refused) -> Self {
        Self::Invalid(refused)
    }
}

/// One generation's link settings, from the front's `link` message.
#[derive(Clone)]
struct LinkConfig {
    generation: u64,
    key: Key,
    group: String,
    stage: u16,
    next: Option<String>,
    limits: Limits,
}

impl LinkConfig {
    fn parse(message: &Value) -> Option<Self> {
        let queue = message["queue"].as_u64().unwrap_or(4);
        let send_timeout_ms = message["send_timeout_ms"].as_u64().unwrap_or(10_000);
        Some(Self {
            generation: message["generation"].as_u64()?,
            key: Key::from_hex(message["key"].as_str()?).ok()?,
            group: message["group"].as_str()?.to_owned(),
            stage: u16::try_from(message["stage"].as_u64()?).ok()?,
            next: message["next"].as_str().map(str::to_owned),
            limits: Limits {
                // A prefill step's state may be as large as W9's codec allows.
                max_state: u32::try_from(CodecLimits::default().max_message_bytes)
                    .unwrap_or(u32::MAX),
                queue: usize::try_from(queue).unwrap_or(4).max(1),
                send_timeout: Duration::from_millis(send_timeout_ms),
                ..Limits::default()
            },
        })
    }
}

/// The direct transport's links (W12): an inbound listener, the current
/// inbound reader, and the link to the next stage.
struct Direct {
    runtime: tokio::runtime::Runtime,
    advertise: String,
    config: Arc<Mutex<Option<LinkConfig>>>,
    inbound: Arc<Mutex<Option<tokio::task::AbortHandle>>>,
    out: Option<(u64, stage_link::Sender)>,
    /// Requests whose cancellation this stage has passed down the chain.
    cancels: BTreeSet<(String, u32)>,
}

impl Direct {
    fn start(listen: &str, advertise: Option<String>, events: Events) -> Result<Self, Refused> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .map_err(|e| Refused(format!("can't start the link runtime: {e}")))?;
        let listener = runtime
            .block_on(Listener::bind(listen))
            .map_err(|e| Refused(format!("can't listen for links on {listen}: {e}")))?;
        let bound = listener
            .local_addr()
            .map_err(|e| Refused(e.to_string()))?
            .to_string();
        let config = Arc::new(Mutex::new(None));
        let inbound = Arc::new(Mutex::new(None));
        runtime.spawn(accept_links(
            listener,
            Arc::clone(&config),
            Arc::clone(&inbound),
            events,
        ));
        Ok(Self {
            runtime,
            advertise: advertise.unwrap_or(bound),
            config,
            inbound,
            out: None,
            cancels: BTreeSet::new(),
        })
    }

    /// Drops every link, as a fence or a new generation's `link` requires.
    fn unlink(&mut self) {
        if let Ok(mut config) = self.config.lock() {
            *config = None;
        }
        if let Some(reader) = self.inbound.lock().ok().and_then(|mut r| r.take()) {
            reader.abort();
        }
        self.out = None;
        self.cancels.clear();
    }
}

/// Accepts the previous stage's link for the current generation, and reads
/// its frames into the stage's events. A new link replaces the old reader.
async fn accept_links(
    listener: Listener,
    config: Arc<Mutex<Option<LinkConfig>>>,
    inbound: Arc<Mutex<Option<tokio::task::AbortHandle>>>,
    events: Events,
) {
    let handshake = Limits::default();
    loop {
        let Ok(offer) = listener.accept(&handshake).await else {
            continue;
        };
        let current = config.lock().ok().and_then(|c| c.clone());
        let Some(current) = current.filter(|c| c.stage > 0) else {
            offer.refuse(Refusal::Generation).await;
            continue;
        };
        let expected = Hop {
            group: current.group.clone(),
            generation: current.generation,
            from: current.stage - 1,
            to: current.stage,
        };
        let Ok(mut receiver) = offer.accept(&current.key, &expected, &current.limits).await else {
            continue;
        };
        let events = events.clone();
        let generation = current.generation;
        let reader = tokio::spawn(async move {
            loop {
                match receiver.recv().await {
                    Ok(frame) if frame.kind == Kind::Cancel => {
                        // A cancellation goes before queued state.
                        if !events.control(Event::Link(generation, frame)) {
                            return;
                        }
                    }
                    Ok(frame) => {
                        // Waiting for room stops this reader, so the link's
                        // sender fills its bounded queue and then fails.
                        let bytes = frame.state.len() + frame.meta.len();
                        let queued = tokio::task::block_in_place(|| {
                            events.data(Event::Link(generation, frame), bytes)
                        });
                        match queued {
                            Ok(()) => {}
                            Err(Blocked::Closed) => return,
                            Err(Blocked::Full(reason)) => {
                                events.control(Event::LinkLost {
                                    generation,
                                    peer: "self",
                                    reason,
                                });
                                return;
                            }
                        }
                    }
                    Err(error) => {
                        let reason = match error {
                            stage_link::LinkError::Closed => {
                                "the previous stage closed the link".to_owned()
                            }
                            other => other.to_string(),
                        };
                        events.control(Event::LinkLost {
                            generation,
                            peer: "prev",
                            reason,
                        });
                        return;
                    }
                }
            }
        });
        if let Ok(mut slot) = inbound.lock()
            && let Some(old) = slot.replace(reader.abort_handle())
        {
            old.abort();
        }
    }
}

struct Server<'a, E: StageExecutor> {
    stage: Stage<'a>,
    tokenizer: Arc<Tokenizer>,
    worker: StageWorker<E>,
    generation: u64,
    writer: Writer,
    log: Log,
    listen: Option<String>,
    next: BTreeMap<String, TcpStream>,
    pending: Option<Step>,
    frames: VecDeque<StageMessage>,
    text: Option<((String, u32), TextStream)>,
    answer: Option<((String, u32), AnswerParser)>,
    direct: Option<Direct>,
    /// Where link tasks report back (direct transport).
    events: Option<Events>,
    /// The request sizes the group serves.
    budget: Budget,
    /// Bounds on decoded frames kept in `frames`.
    bounds: InboxBounds,
}

/// Connects to the front and serves steps until the front closes the
/// connection.
///
/// # Errors
///
/// Refuses when the front or the listener can't be reached.
pub fn serve<E>(stage: Stage<'_>, worker: StageWorker<E>, generation: u64) -> Result<(), Refused>
where
    E: StageBackend,
    E::Error: Display,
{
    let bounds = stage.inbox;
    let events = Events::new(bounds);
    let budget = budget(
        &stage.manifest,
        &worker.executor().geometry(),
        stage.max_context,
        stage.max_input,
    );
    let first = stage.index == 0;
    let direct = match &stage.data_listen {
        Some(listen) => Some(Direct::start(
            listen,
            stage.data_advertise.clone(),
            events.clone(),
        )?),
        None => None,
    };
    let listen = if first || direct.is_some() {
        None
    } else {
        let listener = TcpListener::bind(&stage.listen)
            .map_err(|e| Refused(format!("can't listen on {}: {e}", stage.listen)))?;
        let address = listener
            .local_addr()
            .map_err(|e| Refused(e.to_string()))?
            .to_string();
        let events = events.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let events = events.clone();
                std::thread::spawn(move || read_frames(stream, &events));
            }
        });
        Some(address)
    };
    let stream = TcpStream::connect(&stage.front)
        .map_err(|e| Refused(format!("can't reach the front at {}: {e}", stage.front)))?;
    let _ = stream.set_nodelay(true);
    let reader = stream.try_clone().map_err(|e| Refused(e.to_string()))?;
    let writer = Arc::new(Mutex::new(stream));
    let reader_writer = Writer(Arc::clone(&writer));
    let link_events = direct.as_ref().map(|_| events.clone());
    let front_events = events.clone();
    std::thread::spawn(move || read_front(reader, &reader_writer, &front_events));
    let log = Log {
        stage: stage.name.clone(),
        path: stage.events.clone(),
    };
    let mut server = Server {
        tokenizer: stage.vocabulary_tokenizer(),
        stage,
        worker,
        generation,
        writer: Writer(writer),
        log,
        listen,
        next: BTreeMap::new(),
        pending: None,
        frames: VecDeque::new(),
        text: None,
        answer: None,
        events: link_events,
        direct,
        budget,
        bounds,
    };
    server.log.record("spawn", generation, json!({}));
    server.hello();
    server.run(&events);
    let (frames, bytes) = events.peak();
    events.close();
    server.log.record(
        "front-closed",
        server.generation,
        json!({"peak_frames": frames, "peak_bytes": bytes}),
    );
    Ok(())
}

impl Stage<'_> {
    fn vocabulary_tokenizer(&self) -> Arc<Tokenizer> {
        self.vocabulary.tokenizer()
    }
}

fn read_front(stream: TcpStream, writer: &Writer, events: &Events) {
    for line in BufReader::new(stream).lines() {
        let Ok(line) = line else { break };
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        match message["kind"].as_str() {
            Some("ping") => writer.send(&json!({"kind": "pong", "nonce": message["nonce"]})),
            Some("check") => {
                // The front's known-answer check: the stage is responsive
                // and speaks this protocol.
                let input = message["input"].as_u64().unwrap_or_default();
                writer.send(&json!({
                    "kind": "checked",
                    "nonce": message["nonce"],
                    "output": input.wrapping_mul(2).wrapping_add(1),
                }));
            }
            _ => {
                if !events.control(Event::Front(message)) {
                    return;
                }
            }
        }
    }
    events.control(Event::Closed);
}

impl<E> Server<'_, E>
where
    E: StageBackend,
    E::Error: Display,
{
    fn last(&self) -> bool {
        usize::from(self.stage.index) + 1 == self.stage.manifest.stages.len()
    }

    fn hello(&self) {
        let mut hello = json!({
            "kind": "hello",
            "stage": self.stage.name,
            "token": self.stage.token,
            "generation": self.generation,
            "weights": self.stage.weights,
            "runtime": self.stage.runtime,
            "protocol": PROTOCOL,
            "build": crate::build_identity(),
            "profile": self.stage.profile,
            "limits": {
                "max_context_tokens": self.budget.max_context_tokens,
                "max_input_tokens": self.budget.max_input_tokens,
                "max_prefill_tokens": self.budget.max_prefill_tokens,
                "modes": PromptMode::ALL.map(PromptMode::as_str),
            },
        });
        if let Some(listen) = &self.listen {
            hello["listen"] = json!(listen);
        }
        if let Some(direct) = &self.direct {
            hello["data"] = json!(direct.advertise);
        }
        self.writer.send(&hello);
    }

    fn refused(&self, reason: impl Display, data: Value) {
        let mut data = data;
        data["reason"] = json!(reason.to_string());
        self.log.record("refused", self.generation, data);
    }

    fn run(&mut self, inbox: &Events) {
        while let Some(event) = inbox.recv() {
            match event {
                Event::Closed => return,
                Event::Frame(bytes) => self.frame(&bytes),
                Event::Link(generation, frame) => self.link_frame(generation, frame),
                Event::LinkLost {
                    generation,
                    peer,
                    reason,
                } => {
                    if generation == self.generation {
                        self.link_failed(generation, peer, &reason);
                    }
                }
                Event::Connected(generation, result) => self.connected(generation, result),
                Event::Front(message) => match message["kind"].as_str() {
                    Some("step") => match Step::parse(&message) {
                        Some(step) => self.step(step),
                        None => self.refused("a malformed step", json!({})),
                    },
                    Some("cancel") => {
                        let request = message["request_id"].as_str().unwrap_or_default();
                        let attempt = message["attempt"].as_u64().unwrap_or_default() as u32;
                        self.cancel(request, attempt);
                    }
                    Some("fence") => {
                        let generation = message["generation"].as_u64().unwrap_or_default();
                        self.fence(generation);
                    }
                    Some("link") => self.link(&message),
                    _ => {}
                },
            }
        }
    }

    fn cancel(&mut self, request: &str, attempt: u32) {
        if self
            .pending
            .as_ref()
            .is_some_and(|p| p.request == request && p.attempt == attempt)
        {
            self.pending = None;
        }
        self.frames.retain(|m| m.request_id != request);
        self.worker.reset(request, attempt);
        if self.stage.index == 0 {
            // The first stage starts the cancellation down the chain.
            self.pass_cancel(request, attempt);
        }
        self.log
            .record("stopped", self.generation, json!({"request": request}));
        self.writer.send(&json!({
            "kind": "stopped",
            "request_id": request,
            "attempt": attempt,
        }));
    }

    fn fence(&mut self, generation: u64) {
        if generation > self.generation {
            if let Err(refusal) = self.worker.advance_generation(generation) {
                self.refused(refusal, json!({}));
            }
            self.generation = generation;
        }
        self.pending = None;
        self.frames.clear();
        self.text = None;
        self.answer = None;
        if let Some(direct) = &mut self.direct {
            direct.unlink();
        }
        self.log.record("fenced", self.generation, json!({}));
        self.hello();
    }

    fn frame(&mut self, bytes: &[u8]) {
        let message = match decode(bytes, &CodecLimits::default()) {
            Ok(message) => message,
            Err(refusal) => return self.refused(refusal, json!({})),
        };
        if message.generation < self.generation {
            return self.refused(
                format!(
                    "state from generation {} is older than {}",
                    message.generation, self.generation
                ),
                json!({"request": message.request_id, "step": message.step}),
            );
        }
        if let Some(pending) = self.pending.clone()
            && pending.matches(&message)
        {
            return self.run_step(&pending, message);
        }
        if let Some((request, attempt, next_step, _)) = self.worker.inbox().summary().active
            && request == message.request_id
            && attempt == message.attempt
            && message.step < next_step
        {
            // A step this stage already took.
            return self.stale(&message);
        }
        let bytes = message.wire_bytes();
        while !self.frames.is_empty()
            && (self.frames.len() >= BUFFERED_FRAMES
                || self
                    .frames
                    .iter()
                    .map(StageMessage::wire_bytes)
                    .sum::<usize>()
                    + bytes
                    > self.bounds.bytes)
        {
            if let Some(old) = self.frames.pop_front() {
                self.refused(
                    "the stage dropped buffered state to stay within its bounds",
                    json!({"request": old.request_id, "step": old.step}),
                );
            }
        }
        self.frames.push_back(message);
    }

    fn step(&mut self, step: Step) {
        if step.generation != self.generation {
            return self.refused(
                format!(
                    "a step for generation {} at generation {}",
                    step.generation, self.generation
                ),
                json!({"request": step.request, "step": step.step}),
            );
        }
        if step.stage != self.stage.index {
            return self.refused(
                format!(
                    "a step for stage {} sent to stage {}",
                    step.stage, self.stage.index
                ),
                json!({"request": step.request, "step": step.step}),
            );
        }
        self.log.record(
            "step",
            self.generation,
            json!({
                "request": step.request,
                "attempt": step.attempt,
                "step": step.step,
                "mode": step.mode.as_str(),
            }),
        );
        if let Some((request, attempt, _, _)) = self.worker.inbox().summary().active
            && (request != step.request || attempt != step.attempt)
            && step.step == 0
        {
            // The front runs one request at a time, so a new request's first
            // step ends the one before.
            self.worker.reset(&request, attempt);
        }
        if self.stage.index == 0 {
            match self.first_message(&step) {
                Ok(message) => self.run_step(&step, message),
                Err(Rejected::Limit(limit)) => self.reject(
                    &step,
                    "request_limit",
                    &limit.reason,
                    Some((limit.limit, limit.actual)),
                ),
                Err(Rejected::Invalid(refusal)) => {
                    self.reject(&step, "invalid_request", &refusal.0, None);
                }
            }
            return;
        }
        // State from the stage before usually arrives first.
        let mut stale = Vec::new();
        let mut found = None;
        for (i, message) in self.frames.iter().enumerate() {
            if step.matches(message) {
                found = Some(i);
                break;
            }
            if message.request_id != step.request
                || message.attempt != step.attempt
                || message.step < step.step
            {
                stale.push(i);
            }
        }
        if let Some(i) = found
            && let Some(message) = self.frames.remove(i)
        {
            stale.retain(|s| *s < i);
            for i in stale.into_iter().rev() {
                if let Some(old) = self.frames.remove(i) {
                    self.stale(&old);
                }
            }
            return self.run_step(&step, message);
        }
        for i in stale.into_iter().rev() {
            if let Some(old) = self.frames.remove(i) {
                self.stale(&old);
            }
        }
        self.pending = Some(step);
    }

    fn stale(&self, message: &StageMessage) {
        let reason = match self.worker.inbox().check(message) {
            Err(refusal) => refusal.to_string(),
            Ok(()) => "state for a step the front didn't ask for".into(),
        };
        self.refused(
            reason,
            json!({"request": message.request_id, "step": message.step}),
        );
    }

    fn first_message(&self, step: &Step) -> Result<StageMessage, Rejected> {
        let (tokens, position) = if step.step == 0 {
            let tokens = match &step.prompt_ids {
                Some(_) if self.stage.record.is_none() => {
                    return Err(Rejected::Invalid(Refused(
                        "prompt token IDs are taken only by a recording stage".into(),
                    )));
                }
                Some(ids) if ids.is_empty() => {
                    return Err(Rejected::Invalid(Refused(
                        "the prompt has no token IDs".into(),
                    )));
                }
                Some(ids) => ids.clone(),
                None => self.stage.vocabulary.encode(
                    step.prompt.as_ref(),
                    self.stage.default_token,
                    step.mode,
                )?,
            };
            // Checked before any stage runs the request.
            self.budget
                .check(tokens.len() as u64, step.max_tokens.unwrap_or(1))
                .map_err(Rejected::Limit)?;
            (tokens, 0)
        } else {
            let Some(token) = step.token else {
                return Err(Rejected::Invalid(Refused(format!(
                    "step {} came without the previous token",
                    step.step
                ))));
            };
            let position = match self.worker.inbox().summary().active {
                Some((request, attempt, _, position))
                    if request == step.request && attempt == step.attempt =>
                {
                    position
                }
                _ => {
                    return Err(Rejected::Invalid(Refused(format!(
                        "step {} of request {} came before its first step",
                        step.step, step.request
                    ))));
                }
            };
            if position >= self.budget.max_context_tokens {
                return Err(Rejected::Limit(RequestLimit {
                    reason: format!(
                        "the answer reached the group's {} positions",
                        self.budget.max_context_tokens
                    ),
                    limit: self.budget.max_context_tokens,
                    actual: position + 1,
                }));
            }
            (vec![token], position)
        };
        let entry = &self.stage.manifest.stages[0];
        Ok(StageMessage {
            manifest_digest: self.stage.manifest.digest(),
            generation: self.generation,
            request_id: step.request.clone(),
            attempt: step.attempt,
            stage: 0,
            layers: entry.layers(),
            step: step.step,
            kind: if tokens.len() > 1 {
                StepKind::Prefill
            } else {
                StepKind::Decode
            },
            position,
            tokens,
            sections: Vec::new(),
        })
    }

    fn run_step(&mut self, step: &Step, message: StageMessage) {
        self.pending = None;
        // The step's own time on this stage, which the front subtracts from
        // a token's time to find what the links between stages cost.
        let started = Instant::now();
        let request = (step.request.clone(), step.attempt);
        if self.last() {
            self.worker
                .executor()
                .prepare(step.sampling.unwrap_or(Sampling::GREEDY), step.step);
        }
        let sections = match self.worker.step(message.clone()) {
            Ok(sections) => sections,
            Err(WorkerError::Refused(refusal)) if self.direct.is_some() => {
                // Direct state arrives only once: report it to the front.
                return self.refuse_frame(
                    refusal,
                    json!({"request": message.request_id, "step": message.step}),
                );
            }
            Err(WorkerError::Refused(refusal)) => {
                // Not this step's state after all; keep waiting for it.
                self.refused(
                    refusal,
                    json!({"request": message.request_id, "step": message.step}),
                );
                self.pending = Some(step.clone());
                return;
            }
            Err(WorkerError::Executor(error)) => {
                // The attempt can't continue here; the front's step deadline
                // fails the request and replaces this stage.
                self.refused(
                    format!("the executor failed: {error}"),
                    json!({"request": step.request, "step": step.step}),
                );
                return;
            }
        };
        let mut ack = json!({
            "kind": "ack",
            "stage": self.stage.index,
            "generation": self.generation,
            "request_id": step.request,
            "attempt": step.attempt,
            "step": step.step,
        });
        if self.stage.record.is_some() {
            match self.record(step, &message, &sections) {
                Ok(files) => ack["records"] = json!(files),
                Err(error) => {
                    return self.refused(
                        format!("can't record the step: {error}"),
                        json!({"request": step.request, "step": step.step}),
                    );
                }
            }
        }
        if self.last() {
            let token = if let Some(token) = self.worker.executor().sampled() {
                // The backend sampled on its device with the step's settings.
                token
            } else {
                let Some(logits) = sections
                    .iter()
                    .find(|s| s.kind == SectionKind::Logits)
                    .and_then(|s| s.to_f32().ok())
                else {
                    return self.refused("the last stage made no logits", json!({}));
                };
                match sample(
                    &logits,
                    step.sampling.unwrap_or(Sampling::GREEDY),
                    step.step,
                ) {
                    Ok(token) => token,
                    Err(refusal) => return self.refused(refusal, json!({})),
                }
            };
            if self.text.as_ref().is_none_or(|(r, _)| *r != request) {
                self.text = Some((request.clone(), TextStream::default()));
            }
            let piece = self
                .text
                .as_mut()
                .map(|(_, text)| text.push(&self.tokenizer, token))
                .unwrap_or_default();
            ack["token"] = json!(token);
            ack["text"] = json!(piece);
            if token == self.stage.eos {
                ack["end"] = json!(true);
            }
            if matches!(self.stage.vocabulary, Vocabulary::V41(_)) {
                if self.answer.as_ref().is_none_or(|(r, _)| *r != request) {
                    let parser =
                        AnswerParser::new(Arc::clone(&self.tokenizer), self.stage.eos, step.mode);
                    self.answer = Some((request.clone(), parser));
                }
                let parsed = match self.answer.as_mut() {
                    Some((_, parser)) => parser.push(token, step.last),
                    None => Err(Refused("no answer parser".into())),
                };
                match parsed {
                    Ok(piece) => {
                        ack["reasoning"] = json!(piece.reasoning);
                        ack["content"] = json!(piece.content);
                        if let Some(finish) = piece.finish {
                            ack["finish"] = json!(finish);
                        }
                    }
                    Err(refusal) => return self.refused(refusal, json!({})),
                }
            }
            ack["compute_us"] = json!(micros(started));
        } else {
            let next = usize::from(self.stage.index) + 1;
            let layers = self.stage.manifest.stages[next].layers();
            let hop = message.next_hop(next as u16, layers, sections);
            if self.direct.is_some() {
                return self.forward_link(step, &hop, micros(started));
            }
            let Some(address) = step.next.clone() else {
                return self.refused("the front sent no address for the next stage", json!({}));
            };
            let bytes = match encode(&hop, &CodecLimits::default()) {
                Ok(bytes) => bytes,
                Err(refusal) => return self.unsendable(step, &refusal),
            };
            if let Err(error) = self.forward(&address, &bytes) {
                return self.refused(
                    format!("can't reach the next stage at {address}: {error}"),
                    json!({}),
                );
            }
        }
        self.writer.send(&ack);
    }

    /// Qualification (#12405): writes the last stage's logits and every
    /// stage's observations of this step as little-endian files in the
    /// record folder, and returns their names.
    fn record(
        &self,
        step: &Step,
        message: &StageMessage,
        sections: &[psionic_models::stage_state::Section],
    ) -> std::io::Result<Vec<String>> {
        let Some(dir) = &self.stage.record else {
            return Ok(Vec::new());
        };
        let stem = format!("{}-{}-{}", step.request, step.attempt, step.step);
        let mut files = Vec::new();
        let mut write = |name: String, bytes: Vec<u8>| -> std::io::Result<()> {
            std::fs::write(dir.join(&name), bytes)?;
            files.push(name);
            Ok(())
        };
        if self.last() {
            let logits = self.worker.executor().logits().or_else(|| {
                sections
                    .iter()
                    .find(|s| s.kind == SectionKind::Logits)
                    .and_then(|s| s.to_f32().ok())
            });
            let Some(logits) = logits else {
                return Err(std::io::Error::other("the last stage kept no logits"));
            };
            write(
                format!("{stem}.logits.f32"),
                logits.iter().flat_map(|x| x.to_le_bytes()).collect(),
            )?;
        }
        if let Some(observed) = self.worker.executor().observations(message.tokens.len()) {
            for (site, words) in observed {
                write(
                    format!("{stem}.stage{}.{site}.u32", self.stage.index),
                    words.iter().flat_map(|w| w.to_le_bytes()).collect(),
                )?;
            }
        }
        Ok(files)
    }

    // -----------------------------------------------------------------------
    // The direct transport (W12)
    // -----------------------------------------------------------------------

    /// Ends one request without blaming the stage: the front fails the
    /// request with `code` and keeps the group.
    fn reject(&self, step: &Step, code: &str, reason: &str, limit: Option<(u64, u64)>) {
        let mut message = json!({
            "kind": "rejected",
            "code": code,
            "generation": self.generation,
            "request_id": step.request,
            "attempt": step.attempt,
            "step": step.step,
            "reason": reason,
        });
        if let Some((limit, actual)) = limit {
            message["limit"] = json!(limit);
            message["actual"] = json!(actual);
        }
        self.log.record(
            "rejected",
            self.generation,
            json!({"request": step.request, "step": step.step, "code": code, "reason": reason}),
        );
        self.writer.send(&message);
    }

    /// State this stage made but can't send: over the codec's limits, the
    /// request is too large for the group; anything else is this request's
    /// fault too, since the stage only encodes what it computed.
    fn unsendable(&self, step: &Step, refusal: &StageRefusal) {
        let reason = format!("the stage state can't be sent: {refusal}");
        match refusal {
            StageRefusal::Oversize { limit, actual, .. } => self.reject(
                step,
                "request_limit",
                &reason,
                Some((*limit as u64, *actual as u64)),
            ),
            _ => self.reject(step, "invalid_state", &reason, None),
        }
    }

    fn refuse_frame(&self, reason: impl Display, data: Value) {
        let reason = reason.to_string();
        self.refused(&reason, data);
        self.writer
            .send(&json!({"kind": "refused", "reason": reason}));
    }

    fn link_failed(&self, generation: u64, peer: &str, reason: &str) {
        self.log.record(
            "link-failed",
            generation,
            json!({"peer": peer, "reason": reason}),
        );
        self.writer.send(&json!({
            "kind": "link_failed",
            "generation": generation,
            "peer": peer,
            "reason": reason,
        }));
    }

    /// The front's `link`: this generation's key and the next stage's link
    /// address. The last stage links at once; the others once the next
    /// stage accepts.
    fn link(&mut self, message: &Value) {
        let Some(config) = LinkConfig::parse(message) else {
            return self.refused("a malformed link message", json!({}));
        };
        if config.generation != self.generation || config.stage != self.stage.index {
            return self.refused(
                format!(
                    "a link for stage {} at generation {}",
                    config.stage, config.generation
                ),
                json!({}),
            );
        }
        let Some(direct) = &mut self.direct else {
            return self.refused("a link message, but no --data-listen", json!({}));
        };
        direct.unlink();
        if let Ok(mut current) = direct.config.lock() {
            *current = Some(config.clone());
        }
        let generation = config.generation;
        let Some(next) = config.next.clone() else {
            self.log.record("linked", generation, json!({}));
            self.writer
                .send(&json!({"kind": "linked", "generation": generation}));
            return;
        };
        let (Some(events), hop) = (
            self.events.clone(),
            Hop {
                group: config.group.clone(),
                generation,
                from: config.stage,
                to: config.stage + 1,
            },
        ) else {
            return;
        };
        direct.runtime.spawn(async move {
            // The next stage may not have its own link message yet.
            let deadline = Instant::now() + Duration::from_secs(5);
            let result = loop {
                match stage_link::connect(next.as_str(), &config.key, &hop, &config.limits).await {
                    Ok(sender) => break Ok(sender),
                    Err(error) if Instant::now() >= deadline => {
                        break Err(format!("can't reach the next stage at {next}: {error}"));
                    }
                    Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
                }
            };
            events.control(Event::Connected(generation, result));
        });
    }

    fn connected(&mut self, generation: u64, result: Result<stage_link::Sender, String>) {
        if generation != self.generation {
            return;
        }
        let Some(direct) = &mut self.direct else {
            return;
        };
        match result {
            Ok(sender) => {
                direct.out = Some((generation, sender));
                self.log.record("linked", generation, json!({}));
                self.writer
                    .send(&json!({"kind": "linked", "generation": generation}));
            }
            Err(reason) => self.link_failed(generation, "next", &reason),
        }
    }

    /// Sends a step's state to the next stage and tells the front.
    fn forward_link(&mut self, step: &Step, hop: &StageMessage, compute_us: u64) {
        let state = match encode(hop, &CodecLimits::default()) {
            Ok(bytes) => bytes,
            Err(refusal) => return self.unsendable(step, &refusal),
        };
        let generation = self.generation;
        let frame = Frame {
            kind: Kind::State,
            generation,
            request_id: step.request.clone(),
            attempt: step.attempt,
            step: step.step,
            meta: step.meta(),
            state,
        };
        let Some(direct) = &mut self.direct else {
            return;
        };
        let sent = match &mut direct.out {
            Some((g, sender)) if *g == generation => direct
                .runtime
                .block_on(sender.send(&frame))
                .map_err(|e| e.to_string()),
            _ => Err("no link to the next stage".to_owned()),
        };
        match sent {
            Ok(bytes) => {
                self.log.record(
                    "forwarded",
                    generation,
                    json!({"request": step.request, "step": step.step, "bytes": bytes}),
                );
                self.writer.send(&json!({
                    "kind": "forwarded",
                    "generation": generation,
                    "request_id": step.request,
                    "attempt": step.attempt,
                    "step": step.step,
                    "bytes": bytes,
                    "compute_us": compute_us,
                }));
            }
            Err(reason) => self.link_failed(generation, "next", &reason),
        }
    }

    /// Passes a request's cancellation to the next stage, once.
    fn pass_cancel(&mut self, request: &str, attempt: u32) {
        let generation = self.generation;
        let Some(direct) = &mut self.direct else {
            return;
        };
        if !direct.cancels.insert((request.to_owned(), attempt)) {
            return;
        }
        if let Some((g, sender)) = &mut direct.out
            && *g == generation
        {
            let frame = Frame {
                kind: Kind::Cancel,
                generation,
                request_id: request.to_owned(),
                attempt,
                step: 0,
                meta: Vec::new(),
                state: Vec::new(),
            };
            let _ = direct.runtime.block_on(sender.send(&frame));
            self.log
                .record("cancel-passed", generation, json!({"request": request}));
        }
    }

    /// A frame from the previous stage's link.
    fn link_frame(&mut self, generation: u64, frame: Frame) {
        if generation != self.generation || frame.generation != self.generation {
            return self.refused(
                format!(
                    "a link frame from generation {} at generation {}",
                    frame.generation, self.generation
                ),
                json!({"request": frame.request_id, "step": frame.step}),
            );
        }
        if frame.kind == Kind::Cancel {
            self.pending = None;
            self.frames.retain(|m| m.request_id != frame.request_id);
            self.worker.reset(&frame.request_id, frame.attempt);
            self.log.record(
                "cancel-frame",
                self.generation,
                json!({"request": frame.request_id}),
            );
            return self.pass_cancel(&frame.request_id, frame.attempt);
        }
        let message = match decode(&frame.state, &CodecLimits::default()) {
            Ok(message) => message,
            Err(refusal) => {
                return self.refuse_frame(
                    refusal,
                    json!({"request": frame.request_id, "step": frame.step}),
                );
            }
        };
        if message.request_id != frame.request_id
            || message.attempt != frame.attempt
            || message.step != frame.step
        {
            return self.refuse_frame(
                "a link frame whose stage state names another step",
                json!({"request": frame.request_id, "step": frame.step}),
            );
        }
        let meta: Value = serde_json::from_slice(&frame.meta).unwrap_or(Value::Null);
        let mode = match PromptMode::of(&meta) {
            Ok(mode) => mode,
            Err(refusal) => {
                return self.refuse_frame(
                    refusal,
                    json!({"request": frame.request_id, "step": frame.step}),
                );
            }
        };
        let step = Step {
            generation,
            request: frame.request_id,
            attempt: frame.attempt,
            stage: self.stage.index,
            step: frame.step,
            prompt: None,
            prompt_ids: None,
            token: None,
            sampling: meta
                .get("sampling")
                .and_then(|s| serde_json::from_value(s.clone()).ok()),
            next: None,
            last: meta["final"].as_bool().unwrap_or(false),
            mode,
            max_tokens: None,
        };
        self.log.record(
            "step",
            self.generation,
            json!({
                "request": step.request,
                "attempt": step.attempt,
                "step": step.step,
                "mode": step.mode.as_str(),
            }),
        );
        if let Some((request, attempt, _, _)) = self.worker.inbox().summary().active
            && (request != step.request || attempt != step.attempt)
            && step.step == 0
        {
            self.worker.reset(&request, attempt);
        }
        self.run_step(&step, message);
    }

    fn forward(&mut self, address: &str, bytes: &[u8]) -> std::io::Result<()> {
        if let Some(stream) = self.next.get_mut(address)
            && write_frame(stream, bytes).is_ok()
        {
            return Ok(());
        }
        self.next.remove(address);
        let mut stream = TcpStream::connect(address)?;
        let _ = stream.set_nodelay(true);
        let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));
        write_frame(&mut stream, bytes)?;
        self.next.insert(address.to_owned(), stream);
        Ok(())
    }
}

/// Microseconds since `started`.
fn micros(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX)
}
