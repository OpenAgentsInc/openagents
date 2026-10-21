//! The stage's side of Pylon's stage-group control protocol.
//!
//! One thread reads the front's lines and answers pings and checks at once,
//! so a stage stays healthy while it computes. Listener threads read
//! length-prefixed stage-state frames from the stage before. Everything
//! else, the steps, cancellations, and fences, runs in order on the thread
//! that owns the [`StageWorker`].

use std::{
    collections::{BTreeMap, VecDeque},
    fmt::Display,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender},
    },
    time::Duration,
};

use psionic_models::{
    deepseek_v41_stage::V41GroupManifest,
    stage_state::{
        CodecLimits, SectionKind, StageExecutor, StageMessage, StageWorker, StepKind, WorkerError,
        decode, encode,
    },
};
use serde_json::{Value, json};
use tokenizers::Tokenizer;

use crate::{AnswerParser, PROTOCOL, RUNTIME, Refused, Sampling, TextStream, Vocabulary, sample};

/// Encoded frames from earlier stages kept while their step hasn't come.
const BUFFERED_FRAMES: usize = 32;

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
    /// Where to listen for state.
    pub listen: String,
    /// Where to record what the stage does.
    pub events: Option<PathBuf>,
}

enum Event {
    Front(Value),
    Frame(Vec<u8>),
    Closed,
}

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

fn read_frames(mut stream: TcpStream, events: Sender<Event>) {
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
        if stream.read_exact(&mut bytes).is_err() || events.send(Event::Frame(bytes)).is_err() {
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
    token: Option<u32>,
    sampling: Option<Sampling>,
    next: Option<String>,
    /// The output limit ends the answer after this step.
    last: bool,
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
            token: message["token"]
                .as_u64()
                .and_then(|t| u32::try_from(t).ok()),
            sampling: message
                .get("sampling")
                .and_then(|s| serde_json::from_value(s.clone()).ok()),
            next: message["next"].as_str().map(str::to_owned),
            last: message["final"].as_bool().unwrap_or(false),
        })
    }

    fn matches(&self, message: &StageMessage) -> bool {
        message.request_id == self.request
            && message.attempt == self.attempt
            && message.step == self.step
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
}

/// Connects to the front and serves steps until the front closes the
/// connection.
///
/// # Errors
///
/// Refuses when the front or the listener can't be reached.
pub fn serve<E>(stage: Stage<'_>, worker: StageWorker<E>, generation: u64) -> Result<(), Refused>
where
    E: StageExecutor,
    E::Error: Display,
{
    let (events, inbox) = mpsc::channel();
    let first = stage.index == 0;
    let listen = if first {
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
                std::thread::spawn(move || read_frames(stream, events));
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
    std::thread::spawn(move || read_front(reader, &reader_writer, &events));
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
    };
    server.log.record("spawn", generation, json!({}));
    server.hello();
    server.run(&inbox);
    server
        .log
        .record("front-closed", server.generation, json!({}));
    Ok(())
}

impl Stage<'_> {
    fn vocabulary_tokenizer(&self) -> Arc<Tokenizer> {
        self.vocabulary.tokenizer()
    }
}

fn read_front(stream: TcpStream, writer: &Writer, events: &Sender<Event>) {
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
                if events.send(Event::Front(message)).is_err() {
                    return;
                }
            }
        }
    }
    let _ = events.send(Event::Closed);
}

impl<E> Server<'_, E>
where
    E: StageExecutor,
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
            "runtime": RUNTIME,
            "protocol": PROTOCOL,
        });
        if let Some(listen) = &self.listen {
            hello["listen"] = json!(listen);
        }
        self.writer.send(&hello);
    }

    fn refused(&self, reason: impl Display, data: Value) {
        let mut data = data;
        data["reason"] = json!(reason.to_string());
        self.log.record("refused", self.generation, data);
    }

    fn run(&mut self, inbox: &Receiver<Event>) {
        while let Ok(event) = inbox.recv() {
            match event {
                Event::Closed => return,
                Event::Frame(bytes) => self.frame(&bytes),
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
        if self.frames.len() == BUFFERED_FRAMES {
            self.frames.pop_front();
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
            json!({"request": step.request, "attempt": step.attempt, "step": step.step}),
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
                Err(refusal) => {
                    self.refused(refusal, json!({"request": step.request, "step": step.step}));
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

    fn first_message(&self, step: &Step) -> Result<StageMessage, Refused> {
        let (tokens, position) = if step.step == 0 {
            let tokens = self
                .stage
                .vocabulary
                .encode(step.prompt.as_ref(), self.stage.default_token)?;
            (tokens, 0)
        } else {
            let Some(token) = step.token else {
                return Err(Refused(format!(
                    "step {} came without the previous token",
                    step.step
                )));
            };
            let position = match self.worker.inbox().summary().active {
                Some((request, attempt, _, position))
                    if request == step.request && attempt == step.attempt =>
                {
                    position
                }
                _ => {
                    return Err(Refused(format!(
                        "step {} of request {} came before its first step",
                        step.step, step.request
                    )));
                }
            };
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
        let request = (step.request.clone(), step.attempt);
        let sections = match self.worker.step(message.clone()) {
            Ok(sections) => sections,
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
        if self.last() {
            let Some(logits) = sections
                .iter()
                .find(|s| s.kind == SectionKind::Logits)
                .and_then(|s| s.to_f32().ok())
            else {
                return self.refused("the last stage made no logits", json!({}));
            };
            let token = match sample(
                &logits,
                step.sampling.unwrap_or(Sampling::GREEDY),
                step.step,
            ) {
                Ok(token) => token,
                Err(refusal) => return self.refused(refusal, json!({})),
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
                    let parser = AnswerParser::new(Arc::clone(&self.tokenizer), self.stage.eos);
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
        } else {
            let next = usize::from(self.stage.index) + 1;
            let layers = self.stage.manifest.stages[next].layers();
            let hop = message.next_hop(next as u16, layers, sections);
            let Some(address) = step.next.clone() else {
                return self.refused("the front sent no address for the next stage", json!({}));
            };
            let bytes = match encode(&hop, &CodecLimits::default()) {
                Ok(bytes) => bytes,
                Err(refusal) => return self.refused(refusal, json!({})),
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
