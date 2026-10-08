//! The client half: one request at a time over an agent's streams, with the
//! agent's own traffic handled on the way past.
//!
//! The client owns no process ([`crate::process`] starts one). Every request
//! pumps the agent's output until its reply arrives: `session/update`
//! notifications become typed [`Update`]s for the handler,
//! `session/request_permission` is answered by the handler's policy, and any
//! other reverse request is answered by the handler or refused with
//! `method not found`, so the agent never waits on an answer that is not
//! coming.

use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader, Lines};

use crate::wire::{
    self, Incoming, Initialized, Listed, Opened, PermissionAnswer, PermissionRequest, Prompted,
    RpcError, Update, method,
};

/// How often a request checks whether the caller wants to stop.
const CANCEL_POLL: Duration = Duration::from_millis(100);

/// The most bytes one protocol line may hold. A longer line ends the
/// request as a protocol failure instead of growing without bound.
pub const MAX_LINE: usize = 16 * 1024 * 1024;

/// What a client does with what the agent sends while a request is open.
pub trait Handler {
    /// A `session/update` notification, typed.
    fn update(&mut self, update: Update);

    /// Answer a `session/request_permission`. The default refuses with the
    /// agent's own `reject` option, so an unattended agent never gets a
    /// permission nobody granted.
    fn permission(&mut self, request: &PermissionRequest) -> PermissionAnswer {
        match request.reject() {
            Some(option) => PermissionAnswer::Selected(option.to_owned()),
            None => PermissionAnswer::Cancelled,
        }
    }

    /// Any notification other than `session/update`, such as an agent's
    /// own vendor-namespaced ones. The default ignores it.
    fn notification(&mut self, _method: &str, _params: &Value) {}

    /// Answer any other request the agent makes. The default says the
    /// method does not exist.
    fn reverse(&mut self, method: &str, _params: &Value) -> Result<Value, RpcError> {
        Err(RpcError::method_not_found(method))
    }
}

/// A handler that keeps nothing, for handshakes.
pub struct Ignore;

impl Handler for Ignore {
    fn update(&mut self, _update: Update) {}
}

/// Why a request ended without a usable reply.
#[derive(Clone, Debug, PartialEq)]
pub enum ClientError {
    /// The agent closed its output before answering.
    Closed,
    /// The streams failed.
    Io(String),
    /// The agent answered with an error.
    Refused { method: String, error: RpcError },
    /// The agent answered with a reply of the wrong shape.
    Malformed { method: String },
    /// The agent wrote no protocol line for the whole limit.
    Silent { method: String, seconds: u64 },
    /// The caller stopped the request.
    Cancelled,
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::Closed => write!(f, "the agent exited before it answered"),
            ClientError::Io(why) => write!(f, "the agent's streams failed: {why}"),
            ClientError::Refused { method, error } => {
                let mut message = error.message.clone();
                if message.len() > 300 {
                    let mut end = 300;
                    while !message.is_char_boundary(end) {
                        end -= 1;
                    }
                    message.truncate(end);
                }
                write!(
                    f,
                    "the agent refused `{method}`: {message} ({})",
                    error.code
                )
            }
            ClientError::Malformed { method } => {
                write!(f, "the agent's reply to `{method}` has the wrong shape")
            }
            ClientError::Silent { method, seconds } => {
                write!(f, "the agent wrote nothing on `{method}` for {seconds} s")
            }
            ClientError::Cancelled => write!(f, "stopped before the agent answered"),
        }
    }
}

impl std::error::Error for ClientError {}

/// What a request does when the caller asks it to stop.
#[derive(Clone, Copy, Debug)]
pub enum OnCancel<'a> {
    /// Return [`ClientError::Cancelled`] at once and leave the agent to the
    /// caller, which stops its process.
    Abandon,
    /// Send `session/cancel` for this session, then keep reading for up to
    /// `grace` so the agent can answer the prompt with `cancelled`.
    Notify { session: &'a str, grace: Duration },
}

/// How long one request may wait, and how it stops early.
#[derive(Clone, Copy)]
pub struct Wait<'a> {
    /// How long the agent may go without writing a protocol line. Any
    /// inbound line resets it, so an agent that streams for an hour is not
    /// treated as hung.
    pub silence: Duration,
    /// Polled while the request is open; `true` asks the request to stop.
    pub cancel: &'a dyn Fn() -> bool,
    pub on_cancel: OnCancel<'a>,
}

impl<'a> Wait<'a> {
    /// A wait that never stops early.
    #[must_use]
    pub fn plain(silence: Duration) -> Self {
        Wait {
            silence,
            cancel: &|| false,
            on_cancel: OnCancel::Abandon,
        }
    }
}

/// An ACP client over one agent's output (`reader`) and input (`writer`).
pub struct Client<R, W> {
    lines: Lines<BufReader<R>>,
    out: W,
    next: u64,
}

fn write_failed(error: &std::io::Error) -> ClientError {
    if error.kind() == std::io::ErrorKind::BrokenPipe {
        ClientError::Closed
    } else {
        ClientError::Io(format!("the agent stopped reading: {error}"))
    }
}

impl<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> Client<R, W> {
    pub fn new(reader: R, writer: W) -> Self {
        Client {
            lines: BufReader::new(reader).lines(),
            out: writer,
            next: 0,
        }
    }

    async fn write(&mut self, value: &Value) -> Result<(), ClientError> {
        let mut line = value.to_string();
        line.push('\n');
        self.out
            .write_all(line.as_bytes())
            .await
            .map_err(|error| write_failed(&error))?;
        self.out.flush().await.map_err(|error| write_failed(&error))
    }

    /// Send a notification.
    ///
    /// # Errors
    /// The agent's input is closed or failed.
    pub async fn notify(&mut self, method: &str, params: &Value) -> Result<(), ClientError> {
        self.write(&wire::notification(method, params)).await
    }

    /// Send one request and pump the agent's output until its reply.
    ///
    /// # Errors
    /// See [`ClientError`].
    pub async fn request(
        &mut self,
        method: &str,
        params: &Value,
        wait: Wait<'_>,
        handler: &mut dyn Handler,
    ) -> Result<Value, ClientError> {
        self.next += 1;
        let id = self.next;
        self.write(&wire::request(id, method, params)).await?;
        let mut silence = tokio::time::Instant::now() + wait.silence;
        let mut grace: Option<tokio::time::Instant> = None;
        loop {
            let deadline = grace.map_or(silence, |grace| silence.min(grace));
            let read = tokio::select! {
                biased;
                read = self.lines.next_line() => Some(read),
                () = tokio::time::sleep_until(deadline) => {
                    if grace.is_some_and(|grace| grace <= tokio::time::Instant::now()) {
                        return Err(ClientError::Cancelled);
                    }
                    return Err(ClientError::Silent {
                        method: method.into(),
                        seconds: wait.silence.as_secs(),
                    });
                }
                () = tokio::time::sleep(CANCEL_POLL) => None,
            };
            let Some(read) = read else {
                if grace.is_none() && (wait.cancel)() {
                    match wait.on_cancel {
                        OnCancel::Abandon => return Err(ClientError::Cancelled),
                        OnCancel::Notify {
                            session,
                            grace: allowed,
                        } => {
                            self.notify(method::SESSION_CANCEL, &json!({"sessionId": session}))
                                .await?;
                            grace = Some(tokio::time::Instant::now() + allowed);
                        }
                    }
                }
                continue;
            };
            let line = match read {
                Ok(Some(line)) => line,
                Ok(None) => return Err(ClientError::Closed),
                Err(error) => {
                    return Err(ClientError::Io(format!(
                        "the agent's output could not be read: {error}"
                    )));
                }
            };
            if line.len() > MAX_LINE {
                return Err(ClientError::Io("the agent wrote an oversized line".into()));
            }
            let Some(incoming) = wire::classify(&line) else {
                continue;
            };
            silence = tokio::time::Instant::now() + wait.silence;
            match incoming {
                Incoming::Reply { id: got, result } => {
                    if got.as_u64() != Some(id) {
                        continue;
                    }
                    return result.map_err(|error| ClientError::Refused {
                        method: method.into(),
                        error,
                    });
                }
                Incoming::Request {
                    id: asked,
                    method: name,
                    params,
                } => {
                    let result = if name == method::REQUEST_PERMISSION {
                        serde_json::from_value::<PermissionRequest>(params)
                            .map(|request| handler.permission(&request).to_value())
                            .map_err(|_| RpcError::new(-32602, "invalid permission request"))
                    } else {
                        handler.reverse(&name, &params)
                    };
                    self.write(&wire::reply(&asked, result)).await?;
                }
                Incoming::Notification {
                    method: name,
                    params,
                } => {
                    if name == method::SESSION_UPDATE {
                        if let Some((_, update)) = wire::parse_update(&params) {
                            handler.update(update);
                        }
                    } else {
                        handler.notification(&name, &params);
                    }
                }
            }
        }
    }

    async fn typed<T: DeserializeOwned>(
        &mut self,
        method: &str,
        params: &Value,
        wait: Wait<'_>,
        handler: &mut dyn Handler,
    ) -> Result<T, ClientError> {
        let value = self.request(method, params, wait, handler).await?;
        serde_json::from_value(value).map_err(|_| ClientError::Malformed {
            method: method.into(),
        })
    }

    /// `initialize`, declaring no file system and no terminal, so the agent
    /// runs its own tools.
    ///
    /// # Errors
    /// See [`ClientError`].
    pub async fn initialize(
        &mut self,
        wait: Wait<'_>,
        handler: &mut dyn Handler,
    ) -> Result<Initialized, ClientError> {
        self.typed(
            method::INITIALIZE,
            &json!({
                "protocolVersion": wire::PROTOCOL_VERSION,
                "clientCapabilities": {"fs": {"readTextFile": false, "writeTextFile": false}, "terminal": false},
            }),
            wait,
            handler,
        )
        .await
    }

    /// `authenticate` with the agent's advertised method `method_id`.
    ///
    /// # Errors
    /// See [`ClientError`]; an agent that is not signed in refuses.
    pub async fn authenticate(
        &mut self,
        method_id: &str,
        wait: Wait<'_>,
        handler: &mut dyn Handler,
    ) -> Result<(), ClientError> {
        self.request(
            method::AUTHENTICATE,
            &json!({"methodId": method_id}),
            wait,
            handler,
        )
        .await
        .map(|_| ())
    }

    /// `session/new` in `cwd`, with no MCP servers and `meta` as the
    /// session's `_meta`.
    ///
    /// # Errors
    /// See [`ClientError`]; a reply that names no session is malformed.
    pub async fn new_session(
        &mut self,
        cwd: &str,
        meta: Option<&Value>,
        wait: Wait<'_>,
        handler: &mut dyn Handler,
    ) -> Result<Opened, ClientError> {
        let mut params = json!({"cwd": cwd, "mcpServers": []});
        if let Some(meta) = meta {
            params["_meta"] = meta.clone();
        }
        let opened: Opened = self
            .typed(method::SESSION_NEW, &params, wait, handler)
            .await?;
        if opened.session_id.is_empty() {
            return Err(ClientError::Malformed {
                method: method::SESSION_NEW.into(),
            });
        }
        Ok(opened)
    }

    /// `session/load`: reattach to `session` in `cwd`. The agent replays the
    /// session's history as updates before it answers.
    ///
    /// # Errors
    /// See [`ClientError`].
    pub async fn load_session(
        &mut self,
        session: &str,
        cwd: &str,
        wait: Wait<'_>,
        handler: &mut dyn Handler,
    ) -> Result<Opened, ClientError> {
        let mut opened: Opened = self
            .typed(
                method::SESSION_LOAD,
                &json!({"sessionId": session, "cwd": cwd, "mcpServers": []}),
                wait,
                handler,
            )
            .await?;
        session.clone_into(&mut opened.session_id);
        Ok(opened)
    }

    /// `session/list`, optionally only the sessions in `cwd`.
    ///
    /// # Errors
    /// See [`ClientError`].
    pub async fn list_sessions(
        &mut self,
        cwd: Option<&str>,
        wait: Wait<'_>,
        handler: &mut dyn Handler,
    ) -> Result<Listed, ClientError> {
        let params = match cwd {
            Some(cwd) => json!({"cwd": cwd}),
            None => json!({}),
        };
        self.typed(method::SESSION_LIST, &params, wait, handler)
            .await
    }

    /// `session/set_mode`.
    ///
    /// # Errors
    /// See [`ClientError`].
    pub async fn set_mode(
        &mut self,
        session: &str,
        mode: &str,
        wait: Wait<'_>,
        handler: &mut dyn Handler,
    ) -> Result<(), ClientError> {
        self.request(
            method::SESSION_SET_MODE,
            &json!({"sessionId": session, "modeId": mode}),
            wait,
            handler,
        )
        .await
        .map(|_| ())
    }

    /// `session/prompt` with one text block, until the turn ends.
    ///
    /// # Errors
    /// See [`ClientError`].
    pub async fn prompt(
        &mut self,
        session: &str,
        text: &str,
        wait: Wait<'_>,
        handler: &mut dyn Handler,
    ) -> Result<Prompted, ClientError> {
        self.typed(
            method::SESSION_PROMPT,
            &json!({"sessionId": session, "prompt": [{"type": "text", "text": text}]}),
            wait,
            handler,
        )
        .await
    }

    /// `session/cancel`, a notification.
    ///
    /// # Errors
    /// The agent's input is closed or failed.
    pub async fn cancel(&mut self, session: &str) -> Result<(), ClientError> {
        self.notify(method::SESSION_CANCEL, &json!({"sessionId": session}))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    #[derive(Default)]
    struct Seen {
        updates: Vec<Update>,
        asked: usize,
    }

    impl Handler for Seen {
        fn update(&mut self, update: Update) {
            self.updates.push(update);
        }
        fn permission(&mut self, request: &PermissionRequest) -> PermissionAnswer {
            self.asked += 1;
            PermissionAnswer::Selected(request.allow().unwrap_or_default().into())
        }
    }

    #[tokio::test]
    async fn a_prompt_pumps_updates_answers_permissions_and_types_the_reply() {
        let (client_in, mut agent_out) = tokio::io::duplex(8192);
        let (agent_in, client_out) = tokio::io::duplex(8192);
        let agent = tokio::spawn(async move {
            let mut lines = BufReader::new(agent_in).lines();
            let request: Value =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            let id = request["id"].clone();
            let mut send = async |value: Value| {
                agent_out
                    .write_all(format!("{value}\n").as_bytes())
                    .await
                    .unwrap();
            };
            send(json!("not protocol")).await;
            send(wire::notification(method::SESSION_UPDATE, &json!({"sessionId":"s","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"hi"}}}))).await;
            send(wire::request(7, method::REQUEST_PERMISSION, &json!({"sessionId":"s","toolCall":{"kind":"execute"},"options":[{"optionId":"y","kind":"allow_once"},{"optionId":"n","kind":"reject_once"}]}))).await;
            let answer: Value =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            send(wire::request(8, "x/other", &json!({}))).await;
            let refused: Value =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            send(wire::reply(
                &id,
                Ok(json!({"stopReason":"end_turn","usage":{"inputTokens":3,"outputTokens":1}})),
            ))
            .await;
            (request, answer, refused)
        });
        let mut client = Client::new(client_in, client_out);
        let mut seen = Seen::default();
        let reply = client
            .prompt("s", "do it", Wait::plain(Duration::from_secs(5)), &mut seen)
            .await
            .unwrap();
        assert_eq!(reply.stop_reason, wire::StopReason::EndTurn);
        assert_eq!(seen.updates, vec![Update::AgentText("hi".into())]);
        assert_eq!(seen.asked, 1);
        let (request, answer, refused) = agent.await.unwrap();
        assert_eq!(request["params"]["prompt"][0]["text"], "do it");
        assert_eq!(answer["result"]["outcome"]["optionId"], "y");
        assert_eq!(refused["error"]["code"], wire::METHOD_NOT_FOUND);
    }

    #[tokio::test]
    async fn silence_and_closure_are_named() {
        let (client_in, _agent_out) = tokio::io::duplex(64);
        let (_agent_in, client_out) = tokio::io::duplex(64);
        let mut client = Client::new(client_in, client_out);
        let error = client
            .request(
                "x",
                &json!({}),
                Wait::plain(Duration::from_millis(50)),
                &mut Ignore,
            )
            .await
            .unwrap_err();
        assert!(matches!(error, ClientError::Silent { .. }));

        let (client_in, agent_out) = tokio::io::duplex(64);
        let (_agent_in, client_out) = tokio::io::duplex(64);
        drop(agent_out);
        let mut client = Client::new(client_in, client_out);
        let error = client
            .request(
                "x",
                &json!({}),
                Wait::plain(Duration::from_secs(1)),
                &mut Ignore,
            )
            .await
            .unwrap_err();
        assert_eq!(error, ClientError::Closed);
    }

    #[tokio::test]
    async fn a_notify_cancel_sends_session_cancel_then_gives_up_after_the_grace() {
        let (client_in, _agent_out) = tokio::io::duplex(4096);
        let (agent_in, client_out) = tokio::io::duplex(4096);
        let mut client = Client::new(client_in, client_out);
        let stop = || true;
        let wait = Wait {
            silence: Duration::from_secs(5),
            cancel: &stop,
            on_cancel: OnCancel::Notify {
                session: "s",
                grace: Duration::from_millis(100),
            },
        };
        let error = client
            .request(method::SESSION_PROMPT, &json!({}), wait, &mut Ignore)
            .await
            .unwrap_err();
        assert_eq!(error, ClientError::Cancelled);
        let mut lines = BufReader::new(agent_in).lines();
        let _prompt = lines.next_line().await.unwrap().unwrap();
        let cancel: Value =
            serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert_eq!(cancel["method"], method::SESSION_CANCEL);
        assert_eq!(cancel["params"]["sessionId"], "s");
        assert!(cancel.get("id").is_none());
    }

    #[tokio::test]
    async fn a_reply_of_the_wrong_shape_is_malformed() {
        let (client_in, mut agent_out) = tokio::io::duplex(4096);
        let (agent_in, client_out) = tokio::io::duplex(4096);
        tokio::spawn(async move {
            let mut lines = BufReader::new(agent_in).lines();
            let request: Value =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            let reply = wire::reply(&request["id"], Ok(json!({"sessionId": ""})));
            agent_out
                .write_all(format!("{reply}\n").as_bytes())
                .await
                .unwrap();
        });
        let mut client = Client::new(client_in, client_out);
        let error = client
            .new_session("/w", None, Wait::plain(Duration::from_secs(5)), &mut Ignore)
            .await
            .unwrap_err();
        assert!(matches!(error, ClientError::Malformed { .. }));
    }
}
