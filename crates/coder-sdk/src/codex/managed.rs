//! Explicit app-server I/O for a separately admitted managed-session host.
//!
//! Reads never write permission replies, steering, or another turn. The caller
//! saves its delivery intent and holds its admission through each write. This
//! port owns no process, credentials, environment, budget, or execution rights.
use super::{Reader, Writer};
use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::atomic::{AtomicU64, Ordering},
};
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite},
    time::{Instant, timeout_at},
};

const FRAME: usize = 1_048_576;
const CAPTURE_BYTES: usize = 4 * FRAME;
const CAPTURES: usize = 256;
const PENDING: usize = 16;
static INSTANCES: AtomicU64 = AtomicU64::new(1);

struct Unique(Value);
impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct Json;
        impl<'de> Visitor<'de> for Json {
            type Value = Unique;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("JSON without duplicate keys")
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> std::result::Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> std::result::Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> std::result::Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> std::result::Result<Unique, E> {
                serde_json::Number::from_f64(v)
                    .map(|v| Unique(Value::Number(v)))
                    .ok_or_else(|| E::custom("invalid number"))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_string<E: de::Error>(self, v: String) -> std::result::Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_unit<E: de::Error>(self) -> std::result::Result<Unique, E> {
                Ok(Unique(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<Unique, A::Error> {
                let mut out = Vec::new();
                while let Some(Unique(v)) = a.next_element()? {
                    out.push(v);
                }
                Ok(Unique(out.into()))
            }
            fn visit_map<A: MapAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<Unique, A::Error> {
                let mut out = serde_json::Map::new();
                while let Some(k) = a.next_key::<String>()? {
                    if out.contains_key(&k) {
                        return Err(de::Error::custom("duplicate key"));
                    }
                    out.insert(k, a.next_value::<Unique>()?.0);
                }
                Ok(Unique(out.into()))
            }
        }
        d.deserialize_any(Json)
    }
}

#[cfg(test)]
#[path = "managed_tests.rs"]
mod tests;
fn strict(bytes: &[u8]) -> std::result::Result<Value, serde_json::Error> {
    let mut d = serde_json::Deserializer::from_slice(bytes);
    let value = Unique::deserialize(&mut d)?.0;
    d.end()?;
    Ok(value)
}
fn native_id(value: &Value) -> bool {
    value
        .as_str()
        .is_some_and(|s| !s.is_empty() && s.len() <= 1024)
        || value
            .as_i64()
            .is_some_and(|n| n.unsigned_abs() <= 9_007_199_254_740_991)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    State,
    Limit,
    Timeout,
    Disconnected,
    UnknownDelivery,
    Malformed,
    ChangedSource,
}
pub type Result<T> = std::result::Result<T, Error>;
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::State => "The engine cannot take that request now.",
            Self::Limit => "The engine message exceeds this connection's limit.",
            Self::Timeout => "The engine did not answer in time.",
            Self::Disconnected => "The engine disconnected.",
            Self::UnknownDelivery => "We could not confirm that the engine received the request.",
            Self::Malformed => "The engine sent an invalid message.",
            Self::ChangedSource => "The engine message does not match the current request.",
        })
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    New,
    Initializing,
    Initialized,
    Ready,
    Opening,
    Idle,
    Starting,
    Running,
    Stopping,
    Faulted,
    Unknown,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Initialize,
    Open,
    Start,
    Steer,
    Interrupt,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Sent,
    Received,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Capture {
    pub direction: Direction,
    pub bytes: Vec<u8>,
    pub complete: bool,
}
impl fmt::Debug for Capture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Capture")
            .field("direction", &self.direction)
            .field("bytes", &self.bytes.len())
            .field("complete", &self.complete)
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Written {
    pub id: i64,
    pub operation: Operation,
    pub thread: Option<String>,
    pub turn: Option<String>,
}
struct PendingWrite {
    request: Written,
    bytes: usize,
    expected: usize,
}

/// The original reverse request. Its private source token prevents a caller
/// from changing parameters or replying through another connection.
#[derive(Clone, PartialEq)]
pub struct ReverseRequest {
    instance: u64,
    serial: u64,
    id: Value,
    method: String,
    params: Value,
    source: Vec<u8>,
}
impl ReverseRequest {
    pub fn method(&self) -> &str {
        &self.method
    }
    pub fn params(&self) -> &Value {
        &self.params
    }
    pub fn id(&self) -> &Value {
        &self.id
    }
    pub fn serial(&self) -> u64 {
        self.serial
    }
    pub fn source(&self) -> &[u8] {
        &self.source
    }
}
impl fmt::Debug for ReverseRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReverseRequest")
            .field("serial", &self.serial)
            .field("method", &self.method)
            .finish_non_exhaustive()
    }
}

pub enum Event {
    Waiting,
    Answer {
        request: Written,
        result: std::result::Result<Value, Value>,
    },
    Reverse(ReverseRequest),
    Notification {
        method: String,
        params: Value,
    },
    Terminal {
        thread: String,
        turn: String,
        status: String,
        body: Value,
    },
    Fault {
        method: Option<String>,
    },
}
impl fmt::Debug for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Waiting => "Waiting",
            Self::Answer { .. } => "Answer",
            Self::Reverse(_) => "Reverse",
            Self::Notification { .. } => "Notification",
            Self::Terminal { .. } => "Terminal",
            Self::Fault { .. } => "Fault",
        })
    }
}

/// One connection, with exact outstanding request and native turn identity.
pub struct Managed {
    reader: Reader,
    writer: Writer,
    phase: Phase,
    faulted: bool,
    stop_issued: bool,
    instance: u64,
    serial: u64,
    next_id: i64,
    thread: Option<String>,
    turn: Option<String>,
    announced_thread: Option<String>,
    announced_turn: Option<String>,
    pending: BTreeMap<i64, PendingWrite>,
    reverse: BTreeMap<String, ReverseRequest>,
    replied: BTreeSet<String>,
    input: Vec<u8>,
    captures: Vec<Capture>,
    capture_bytes: usize,
}
impl Managed {
    pub fn over(
        reader: impl AsyncBufRead + Unpin + Send + 'static,
        writer: impl AsyncWrite + Unpin + Send + 'static,
    ) -> Self {
        Self {
            reader: Box::new(reader),
            writer: Box::new(writer),
            phase: Phase::New,
            faulted: false,
            stop_issued: false,
            instance: INSTANCES.fetch_add(1, Ordering::Relaxed),
            serial: 0,
            next_id: 0,
            thread: None,
            turn: None,
            announced_thread: None,
            announced_turn: None,
            pending: BTreeMap::new(),
            reverse: BTreeMap::new(),
            replied: BTreeSet::new(),
            input: Vec::new(),
            captures: Vec::new(),
            capture_bytes: 0,
        }
    }
    pub fn phase(&self) -> Phase {
        self.phase
    }
    pub fn thread(&self) -> Option<&str> {
        self.thread.as_deref()
    }
    pub fn turn(&self) -> Option<&str> {
        self.turn.as_deref()
    }
    pub fn take_captures(&mut self) -> Vec<Capture> {
        self.capture_bytes = 0;
        std::mem::take(&mut self.captures)
    }
    /// Inspect a retained frame prefix without consuming it or changing how
    /// the next read completes that frame.
    pub fn partial_capture(&self) -> Option<Capture> {
        (!self.input.is_empty()).then(|| Capture {
            direction: Direction::Received,
            bytes: self.input.clone(),
            complete: false,
        })
    }
    fn require(&self, phase: Phase) -> Result<()> {
        if self.phase == phase {
            Ok(())
        } else {
            Err(Error::State)
        }
    }
    fn write_ready(&self, value: &Value, deadline: Instant) -> Result<Vec<u8>> {
        if Instant::now() >= deadline {
            return Err(Error::Timeout);
        }
        let mut bytes = serde_json::to_vec(value).map_err(|_| Error::Malformed)?;
        bytes.push(b'\n');
        if bytes.len() > FRAME
            || self.captures.len() >= CAPTURES
            || self.capture_bytes + self.input.len() + bytes.len() > CAPTURE_BYTES
        {
            return Err(Error::Limit);
        }
        Ok(bytes)
    }
    async fn write(&mut self, value: &Value, deadline: Instant) -> Result<()> {
        let bytes = self.write_ready(value, deadline)?;
        let next_phase = self.phase;
        // Cancellation at any await keeps delivery unknown. Never retry a
        // partially written frame on this connection.
        self.phase = Phase::Unknown;
        let index = self.captures.len();
        self.captures.push(Capture {
            direction: Direction::Sent,
            bytes: Vec::new(),
            complete: false,
        });
        let mut sent = 0;
        while sent < bytes.len() {
            let n = timeout_at(
                deadline,
                std::future::poll_fn(|cx| {
                    if Instant::now() >= deadline {
                        return std::task::Poll::Ready(Err(std::io::Error::from(
                            std::io::ErrorKind::TimedOut,
                        )));
                    }
                    std::pin::Pin::new(&mut *self.writer).poll_write(cx, &bytes[sent..])
                }),
            )
            .await
            .map_err(|_| Error::UnknownDelivery)?
            .map_err(|_| Error::UnknownDelivery)?;
            if n == 0 {
                return Err(Error::UnknownDelivery);
            }
            self.captures[index]
                .bytes
                .extend_from_slice(&bytes[sent..sent + n]);
            self.capture_bytes += n;
            sent += n;
            if value.get("method").is_some() {
                if let Some(pending) = value["id"]
                    .as_i64()
                    .and_then(|id| self.pending.get_mut(&id))
                {
                    pending.bytes += n;
                }
            }
        }
        timeout_at(
            deadline,
            std::future::poll_fn(|cx| {
                if Instant::now() >= deadline {
                    return std::task::Poll::Ready(Err(std::io::Error::from(
                        std::io::ErrorKind::TimedOut,
                    )));
                }
                std::pin::Pin::new(&mut *self.writer).poll_flush(cx)
            }),
        )
        .await
        .map_err(|_| Error::UnknownDelivery)?
        .map_err(|_| Error::UnknownDelivery)?;
        self.captures[index].complete = true;
        self.phase = next_phase;
        Ok(())
    }
    async fn send(
        &mut self,
        operation: Operation,
        method: &str,
        params: Value,
        deadline: Instant,
    ) -> Result<Written> {
        if self.pending.len()
            >= if operation == Operation::Interrupt {
                PENDING
            } else {
                PENDING - 1
            }
        {
            return Err(Error::Limit);
        }
        self.next_id = self
            .next_id
            .checked_add(1)
            .filter(|v| *v <= 9_007_199_254_740_991)
            .ok_or(Error::Limit)?;
        let request = Written {
            id: self.next_id,
            operation,
            thread: self.thread.clone(),
            turn: self.turn.clone(),
        };
        let body = json!({"jsonrpc":"2.0","id":request.id,"method":method,"params":params});
        let expected = serde_json::to_vec(&body)
            .map_err(|_| Error::Malformed)?
            .len()
            + 1;
        self.pending.insert(
            request.id,
            PendingWrite {
                request: request.clone(),
                bytes: 0,
                expected,
            },
        );
        if let Err(error) = self.write(&body, deadline).await {
            if error != Error::UnknownDelivery {
                self.pending.remove(&request.id);
            }
            return Err(error);
        }
        Ok(request)
    }
    pub async fn initialize(
        &mut self,
        name: &str,
        version: &str,
        deadline: Instant,
    ) -> Result<Written> {
        self.require(Phase::New)?;
        let request = self.send(Operation::Initialize,"initialize",json!({"clientInfo":{"name":name,"version":version},"capabilities":{"experimentalApi":true}}),deadline).await?;
        self.phase = Phase::Initializing;
        Ok(request)
    }
    pub async fn initialized(&mut self, deadline: Instant) -> Result<()> {
        self.require(Phase::Initialized)?;
        self.write(&json!({"jsonrpc":"2.0","method":"initialized"}), deadline)
            .await?;
        self.phase = Phase::Ready;
        Ok(())
    }
    /// The caller independently admits every native option. This port cannot
    /// turn an app-server option into evidence that a bound is enforced.
    pub async fn open(&mut self, params: Value, deadline: Instant) -> Result<Written> {
        self.require(Phase::Ready)?;
        if !params.is_object() {
            return Err(Error::Malformed);
        }
        let request = self
            .send(Operation::Open, "thread/start", params, deadline)
            .await?;
        self.phase = Phase::Opening;
        Ok(request)
    }
    pub async fn start(&mut self, input: Vec<Value>, deadline: Instant) -> Result<Written> {
        self.require(Phase::Idle)?;
        if !self.pending.is_empty() {
            return Err(Error::State);
        }
        let request = self
            .send(
                Operation::Start,
                "turn/start",
                json!({"threadId":self.thread,"input":input}),
                deadline,
            )
            .await?;
        self.phase = Phase::Starting;
        Ok(request)
    }
    fn current_turn(&self, expected: &str) -> Result<()> {
        if self.phase != Phase::Running
            || self.turn.as_deref() != Some(expected)
            || !self.reverse.is_empty()
        {
            return Err(Error::ChangedSource);
        }
        Ok(())
    }
    pub async fn steer(
        &mut self,
        expected: &str,
        input: Vec<Value>,
        deadline: Instant,
    ) -> Result<Written> {
        self.current_turn(expected)?;
        self.send(
            Operation::Steer,
            "turn/steer",
            json!({"threadId":self.thread,"expectedTurnId":expected,"input":input}),
            deadline,
        )
        .await
    }
    pub async fn interrupt(&mut self, expected: &str, deadline: Instant) -> Result<Written> {
        if self.stop_issued
            || self.turn.as_deref() != Some(expected)
            || !matches!(self.phase, Phase::Running | Phase::Faulted | Phase::Unknown)
        {
            return Err(Error::ChangedSource);
        }
        self.stop_issued = true;
        let result = self
            .send(
                Operation::Interrupt,
                "turn/interrupt",
                json!({"threadId":self.thread,"turnId":expected}),
                deadline,
            )
            .await;
        match &result {
            Ok(_) => {
                self.phase = if self.faulted {
                    Phase::Faulted
                } else {
                    Phase::Stopping
                }
            }
            Err(error) if *error != Error::UnknownDelivery => self.stop_issued = false,
            Err(_) => {}
        }
        result
    }
    fn original_reverse(&self, request: &ReverseRequest) -> Result<String> {
        let key = request.id.to_string();
        if request.instance != self.instance || self.reverse.get(&key) != Some(request) {
            return Err(Error::ChangedSource);
        }
        Ok(key)
    }
    pub async fn permission(
        &mut self,
        request: &ReverseRequest,
        allow_once: bool,
        deadline: Instant,
    ) -> Result<()> {
        if allow_once {
            self.require(Phase::Running)?;
            if self.stop_issued {
                return Err(Error::State);
            }
        } else if !matches!(
            self.phase,
            Phase::Running | Phase::Stopping | Phase::Faulted | Phase::Unknown
        ) {
            return Err(Error::State);
        }
        let key = self.original_reverse(request)?;
        if !matches!(
            request.method.as_str(),
            "item/commandExecution/requestApproval" | "item/fileChange/requestApproval"
        ) {
            return Err(Error::ChangedSource);
        }
        // Consume before any write. A lost reply cannot become another grant.
        let reply = json!({"jsonrpc":"2.0","id":request.id,"result":{"decision":if allow_once {"accept"}else{"decline"}}});
        self.write_ready(&reply, deadline)?;
        self.reverse.remove(&key);
        self.replied.insert(key);
        if let Err(error) = self.write(&reply, deadline).await {
            self.phase = Phase::Unknown;
            return Err(error);
        }
        Ok(())
    }
    pub async fn deny_unsupported(
        &mut self,
        request: &ReverseRequest,
        deadline: Instant,
    ) -> Result<()> {
        let key = self.original_reverse(request)?;
        let reply = json!({"jsonrpc":"2.0","id":request.id,"error":{"code":-32601,"message":"This session does not support that method."}});
        self.write_ready(&reply, deadline)?;
        self.reverse.remove(&key);
        self.replied.insert(key);
        self.faulted = true;
        if let Err(error) = self.write(&reply, deadline).await {
            self.phase = Phase::Unknown;
            return Err(error);
        }
        self.phase = Phase::Faulted;
        Ok(())
    }

    /// Read at most one bounded frame. Partial bytes survive cancellation or
    /// timeout; receiving a message never sends input back to the engine.
    pub async fn next(&mut self, deadline: Instant) -> Result<Event> {
        if Instant::now() >= deadline {
            return if self.pending.is_empty() {
                Ok(Event::Waiting)
            } else {
                self.phase = Phase::Unknown;
                Err(Error::UnknownDelivery)
            };
        }
        loop {
            let available = match timeout_at(deadline, self.reader.fill_buf()).await {
                Err(_) if self.pending.is_empty() => return Ok(Event::Waiting),
                Err(_) => {
                    self.phase = Phase::Unknown;
                    return Err(Error::UnknownDelivery);
                }
                Ok(Err(_)) => {
                    self.phase = Phase::Unknown;
                    self.capture_partial();
                    return Err(Error::Disconnected);
                }
                Ok(Ok(bytes)) if bytes.is_empty() => {
                    self.phase = Phase::Unknown;
                    self.capture_partial();
                    return Err(Error::Disconnected);
                }
                Ok(Ok(bytes)) => bytes,
            };
            let count = available
                .iter()
                .position(|b| *b == b'\n')
                .map_or(available.len(), |n| n + 1);
            if self.input.len() + count > FRAME {
                self.phase = Phase::Faulted;
                self.faulted = true;
                return Err(Error::Limit);
            }
            if self.captures.len() >= CAPTURES
                || self.capture_bytes + self.input.len() + count > CAPTURE_BYTES
            {
                return Err(Error::Limit);
            }
            self.input.extend_from_slice(&available[..count]);
            self.reader.consume(count);
            if self.input.last() != Some(&b'\n') {
                continue;
            }
            let bytes = std::mem::take(&mut self.input);
            let body = strict(&bytes);
            self.capture_bytes += bytes.len();
            self.captures.push(Capture {
                direction: Direction::Received,
                bytes: bytes.clone(),
                complete: true,
            });
            let body = body.map_err(|_| {
                self.phase = Phase::Faulted;
                self.faulted = true;
                Error::Malformed
            })?;
            return self.receive(body, bytes);
        }
    }
    fn fault(&mut self, method: Option<String>) -> Result<Event> {
        self.phase = Phase::Faulted;
        self.faulted = true;
        Ok(Event::Fault { method })
    }
    fn capture_partial(&mut self) {
        if !self.input.is_empty()
            && self.captures.len() < CAPTURES
            && self.capture_bytes + self.input.len() <= CAPTURE_BYTES
        {
            let bytes = std::mem::take(&mut self.input);
            self.capture_bytes += bytes.len();
            self.captures.push(Capture {
                direction: Direction::Received,
                bytes,
                complete: false,
            });
        }
    }
    fn receive(&mut self, body: Value, source: Vec<u8>) -> Result<Event> {
        if !body.is_object()
            || body.get("jsonrpc").is_some_and(|v| v != "2.0")
            || body.get("method").is_some_and(|m| !m.is_string())
            || body.get("method").is_some()
                && (body.get("result").is_some() || body.get("error").is_some())
            || body.get("method").is_none() && body.get("params").is_some()
        {
            return self.fault(None);
        }
        let Some(method) = body["method"].as_str() else {
            let Some(id) = body["id"].as_i64() else {
                return self.fault(None);
            };
            let Some(pending) = self.pending.remove(&id) else {
                return self.fault(None);
            };
            if pending.bytes != pending.expected {
                return self.fault(None);
            }
            let request = pending.request;
            if body.get("result").is_some() == body.get("error").is_some() {
                return self.fault(None);
            }
            if let Some(error) = body.get("error") {
                self.phase = Phase::Unknown;
                return Ok(Event::Answer {
                    request,
                    result: Err(error.clone()),
                });
            }
            let result = body["result"].clone();
            match request.operation {
                Operation::Initialize => {
                    if !result["userAgent"].is_string() {
                        return self.fault(None);
                    }
                    self.phase = Phase::Initialized;
                }
                Operation::Open => {
                    let Some(id) = result["thread"]["id"]
                        .as_str()
                        .filter(|s| !s.is_empty() && s.len() <= 1024)
                    else {
                        return self.fault(None);
                    };
                    if self
                        .announced_thread
                        .as_ref()
                        .is_some_and(|seen| seen != id)
                    {
                        return self.fault(None);
                    }
                    self.announced_thread = None;
                    self.thread = Some(id.into());
                    self.phase = Phase::Idle;
                }
                Operation::Start => {
                    let Some(id) = result["turn"]["id"]
                        .as_str()
                        .filter(|s| !s.is_empty() && s.len() <= 1024)
                    else {
                        return self.fault(None);
                    };
                    if self.announced_turn.as_ref().is_some_and(|seen| seen != id) {
                        return self.fault(None);
                    }
                    self.announced_turn = None;
                    self.turn = Some(id.into());
                    self.phase = Phase::Running;
                }
                Operation::Steer | Operation::Interrupt => {}
            }
            if self.faulted {
                self.phase = Phase::Faulted;
            }
            return Ok(Event::Answer {
                request,
                result: Ok(result),
            });
        };
        let method = method.to_string();
        let params = body.get("params").cloned().unwrap_or(Value::Null);
        if !params.is_object() {
            return self.fault(Some(method));
        }
        if let Some(id) = body.get("id") {
            if !native_id(id)
                || self.phase != Phase::Running
                || self.reverse.len() >= PENDING
                || self.replied.len() >= CAPTURES
            {
                return self.fault(Some(method));
            }
            if matches!(
                method.as_str(),
                "item/commandExecution/requestApproval" | "item/fileChange/requestApproval"
            ) && (params["threadId"].as_str() != self.thread()
                || params["turnId"].as_str() != self.turn()
                || !params["itemId"].is_string())
            {
                return self.fault(Some(method));
            }
            let key = id.to_string();
            if self.replied.contains(&key) {
                return self.fault(Some(method));
            }
            if let Some(prior) = self.reverse.get(&key) {
                if prior.method == method && prior.params == params && prior.source == source {
                    return Ok(Event::Reverse(prior.clone()));
                }
                return self.fault(Some(method));
            }
            self.serial = self.serial.checked_add(1).ok_or(Error::Limit)?;
            let request = ReverseRequest {
                instance: self.instance,
                serial: self.serial,
                id: id.clone(),
                method,
                params,
                source,
            };
            self.reverse.insert(key, request.clone());
            if !matches!(
                request.method.as_str(),
                "item/commandExecution/requestApproval" | "item/fileChange/requestApproval"
            ) {
                self.phase = Phase::Faulted;
                self.faulted = true;
            }
            return Ok(Event::Reverse(request));
        }
        for (key, field, event, current) in [
            ("threadId", "thread", "thread/started", self.thread.clone()),
            ("turnId", "turn", "turn/started", self.turn.clone()),
        ] {
            if let Some(value) = params.get(key) {
                let Some(id) = value.as_str().filter(|s| !s.is_empty() && s.len() <= 1024) else {
                    return self.fault(Some(method));
                };
                if Some(id) != current.as_deref()
                    && !(current.is_none() && method == event && params[field]["id"] == id)
                {
                    return self.fault(Some(method));
                }
            }
        }
        for (event, field, operation, current) in [
            (
                "thread/started",
                "thread",
                Operation::Open,
                self.thread.clone(),
            ),
            ("turn/started", "turn", Operation::Start, self.turn.clone()),
        ] {
            if method != event {
                continue;
            }
            let Some(id) = params[field]["id"]
                .as_str()
                .filter(|s| !s.is_empty() && s.len() <= 1024)
            else {
                return self.fault(Some(method));
            };
            if let Some(current) = current {
                if current != id {
                    return self.fault(Some(method));
                }
            } else {
                if !self
                    .pending
                    .values()
                    .any(|p| p.request.operation == operation && p.bytes == p.expected)
                {
                    return self.fault(Some(method));
                }
                let announced = if operation == Operation::Open {
                    &mut self.announced_thread
                } else {
                    &mut self.announced_turn
                };
                if announced.as_ref().is_some_and(|seen| seen != id) {
                    return self.fault(Some(method));
                }
                *announced = Some(id.into());
            }
        }
        if method == "turn/completed" {
            let Some(turn) = params["turn"]["id"].as_str() else {
                return self.fault(Some(method));
            };
            let Some(status) = params["turn"]["status"].as_str() else {
                return self.fault(Some(method));
            };
            if params["threadId"].as_str() != self.thread()
                || Some(turn) != self.turn()
                || !matches!(status, "completed" | "failed" | "interrupted")
            {
                return self.fault(Some(method));
            }
            let event = Event::Terminal {
                thread: self.thread.clone().ok_or(Error::State)?,
                turn: turn.into(),
                status: status.into(),
                body: params,
            };
            self.turn = None;
            self.stop_issued = false;
            self.phase = if self.faulted {
                Phase::Faulted
            } else {
                Phase::Idle
            };
            self.reverse.clear();
            self.replied.clear();
            return Ok(event);
        }
        if !matches!(
            method.as_str(),
            "thread/started"
                | "turn/started"
                | "item/started"
                | "item/completed"
                | "item/agentMessage/delta"
                | "item/reasoning/summaryTextDelta"
                | "item/reasoning/textDelta"
                | "thread/tokenUsage/updated"
                | "account/rateLimits/updated"
                | "error"
        ) {
            return self.fault(Some(method));
        }
        Ok(Event::Notification { method, params })
    }
}
