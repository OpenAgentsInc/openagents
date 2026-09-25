//! The relay link: one websocket on a background thread.
//!
//! The game thread never blocks on the network. It sends [`Out`] commands
//! and drains [`In`] messages once per frame. The link thread connects,
//! replays the live subscriptions after every reconnect, and reports
//! connection changes. Both `ws://` and `wss://` URLs work.

use std::collections::BTreeMap;
use std::net::TcpStream;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::time::{Duration, Instant};

use nostr::domain::Event;
use serde_json::{Value, json};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

/// A command from the game to the relay.
#[derive(Clone, Debug)]
pub enum Out {
    /// Publish a signed event.
    Publish(Event),
    /// Open or replace a subscription. `live` subscriptions are replayed
    /// after a reconnect; one-shot queries are not.
    Subscribe {
        /// Subscription id.
        id: String,
        /// NIP-01 filters.
        filters: Vec<Value>,
        /// Whether to keep and replay it.
        live: bool,
    },
    /// Close a subscription.
    Close(String),
    /// Answer a NIP-42 challenge with a signed kind `22242` event.
    Auth(Event),
}

/// A message from the relay to the game.
#[derive(Clone, Debug)]
pub enum In {
    /// The socket is open.
    Connected,
    /// The socket closed or could not open.
    Disconnected(String),
    /// An event for a subscription.
    Event {
        /// Subscription id.
        sub: String,
        /// The event, not yet validated.
        event: Box<Event>,
    },
    /// End of stored events for a subscription.
    Eose(String),
    /// The relay's verdict on a published event.
    Ok {
        /// Event id.
        id: String,
        /// Accepted or not.
        accepted: bool,
        /// The relay's reason.
        message: String,
    },
    /// The relay closed a subscription.
    Closed(String, String),
    /// A human-readable relay notice.
    Notice(String),
    /// A NIP-42 challenge.
    Auth(String),
}

/// The game's handle on the link thread.
pub struct Link {
    tx: Sender<Out>,
    rx: Receiver<In>,
    /// The relay URL.
    pub url: String,
}

impl Link {
    /// Starts the link thread for `url`.
    #[must_use]
    pub fn start(url: &str) -> Self {
        let (out_tx, out_rx) = mpsc::channel();
        let (in_tx, in_rx) = mpsc::channel();
        let thread_url = url.to_owned();
        std::thread::Builder::new()
            .name("verse-relay".into())
            .spawn(move || run(&thread_url, &out_rx, &in_tx))
            .expect("the relay thread starts");
        Self {
            tx: out_tx,
            rx: in_rx,
            url: url.to_owned(),
        }
    }

    /// Queues a command. Dropped silently when the thread is gone.
    pub fn send(&self, out: Out) {
        let _ = self.tx.send(out);
    }

    /// Everything the relay has said since the last call.
    pub fn drain(&self) -> Vec<In> {
        self.rx.try_iter().collect()
    }
}

type Socket = WebSocket<MaybeTlsStream<TcpStream>>;

fn run(url: &str, out: &Receiver<Out>, inbox: &Sender<In>) {
    let mut live: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut backoff = Duration::from_millis(500);
    // Commands queued while disconnected, except stale poses.
    let mut backlog: Vec<Out> = Vec::new();
    loop {
        let mut socket = match tungstenite::connect(url) {
            Ok((socket, _)) => socket,
            Err(e) => {
                if inbox.send(In::Disconnected(e.to_string())).is_err() {
                    return;
                }
                if !wait_offline(out, &mut live, &mut backlog, backoff) {
                    return;
                }
                backoff = (backoff * 2).min(Duration::from_secs(8));
                continue;
            }
        };
        backoff = Duration::from_millis(500);
        let tcp = match socket.get_mut() {
            MaybeTlsStream::Plain(stream) => Some(&*stream),
            MaybeTlsStream::Rustls(stream) => Some(&stream.sock),
            _ => None,
        };
        if let Some(tcp) = tcp {
            let _ = tcp.set_read_timeout(Some(Duration::from_millis(15)));
            let _ = tcp.set_nodelay(true);
        }
        if inbox.send(In::Connected).is_err() {
            return;
        }
        let mut ok = true;
        for (id, filters) in &live {
            ok &= write(&mut socket, &req(id, filters));
        }
        for command in backlog.drain(..) {
            ok &= apply(&mut socket, &mut live, command);
        }
        while ok {
            loop {
                match out.try_recv() {
                    Ok(command) => ok &= apply(&mut socket, &mut live, command),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        let _ = socket.close(None);
                        return;
                    }
                }
            }
            match socket.read() {
                Ok(Message::Text(text)) => {
                    if let Some(message) = parse(&text)
                        && inbox.send(message).is_err()
                    {
                        return;
                    }
                }
                Ok(Message::Ping(bytes)) => ok &= socket.send(Message::Pong(bytes)).is_ok(),
                Ok(Message::Close(_)) => ok = false,
                Ok(_) => {}
                Err(tungstenite::Error::Io(e))
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                Err(_) => ok = false,
            }
        }
        if inbox
            .send(In::Disconnected("the relay closed the connection".into()))
            .is_err()
        {
            return;
        }
    }
}

/// Sleeps out a backoff while keeping subscriptions and durable commands.
/// Returns false when the game has gone away.
fn wait_offline(
    out: &Receiver<Out>,
    live: &mut BTreeMap<String, Vec<Value>>,
    backlog: &mut Vec<Out>,
    backoff: Duration,
) -> bool {
    let until = Instant::now() + backoff;
    while Instant::now() < until {
        loop {
            match out.try_recv() {
                Ok(Out::Subscribe {
                    id,
                    filters,
                    live: true,
                }) => {
                    live.insert(id, filters);
                }
                Ok(Out::Close(id)) => {
                    live.remove(&id);
                }
                Ok(Out::Publish(event)) if event.kind < 20_000 || event.kind >= 30_000 => {
                    backlog.retain(|o| !matches!(o, Out::Publish(e) if same_address(e, &event)));
                    backlog.push(Out::Publish(event));
                }
                Ok(_) => {}
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return false,
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    true
}

fn same_address(a: &Event, b: &Event) -> bool {
    a.kind == b.kind && a.pubkey == b.pubkey && a.tag_values("d").next() == b.tag_values("d").next()
}

fn apply(socket: &mut Socket, live: &mut BTreeMap<String, Vec<Value>>, command: Out) -> bool {
    match command {
        Out::Publish(event) => write(socket, &json!(["EVENT", event])),
        Out::Subscribe {
            id,
            filters,
            live: keep,
        } => {
            let sent = write(socket, &req(&id, &filters));
            if keep {
                live.insert(id, filters);
            }
            sent
        }
        Out::Close(id) => {
            live.remove(&id);
            write(socket, &json!(["CLOSE", id]))
        }
        Out::Auth(event) => write(socket, &json!(["AUTH", event])),
    }
}

fn req(id: &str, filters: &[Value]) -> Value {
    let mut message = vec![json!("REQ"), json!(id)];
    message.extend(filters.iter().cloned());
    Value::Array(message)
}

fn write(socket: &mut Socket, value: &Value) -> bool {
    socket.send(Message::text(value.to_string())).is_ok()
}

/// Parses one relay message.
#[must_use]
pub fn parse(text: &str) -> Option<In> {
    let value: Value = serde_json::from_str(text).ok()?;
    let parts = value.as_array()?;
    let str_at = |i: usize| parts.get(i).and_then(Value::as_str).map(str::to_owned);
    match parts.first()?.as_str()? {
        "EVENT" => {
            let event: Event = serde_json::from_value(parts.get(2)?.clone()).ok()?;
            Some(In::Event {
                sub: str_at(1)?,
                event: Box::new(event),
            })
        }
        "EOSE" => Some(In::Eose(str_at(1)?)),
        "OK" => Some(In::Ok {
            id: str_at(1)?,
            accepted: parts.get(2)?.as_bool()?,
            message: str_at(3).unwrap_or_default(),
        }),
        "CLOSED" => Some(In::Closed(str_at(1)?, str_at(2).unwrap_or_default())),
        "NOTICE" => Some(In::Notice(str_at(1)?)),
        "AUTH" => Some(In::Auth(str_at(1)?)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relay_messages_parse() {
        assert!(matches!(parse(r#"["EOSE","s1"]"#), Some(In::Eose(s)) if s == "s1"));
        assert!(matches!(
            parse(r#"["OK","ab",false,"rate-limited: slow down"]"#),
            Some(In::Ok { accepted: false, message, .. }) if message.starts_with("rate-limited:")
        ));
        assert!(matches!(parse(r#"["NOTICE","hi"]"#), Some(In::Notice(_))));
        assert!(matches!(parse(r#"["AUTH","abc"]"#), Some(In::Auth(c)) if c == "abc"));
        assert!(parse("not json").is_none());
        assert!(parse(r#"["EVENT","s1",{"bad":1}]"#).is_none());
    }

    #[test]
    fn a_req_carries_every_filter() {
        let v = req("s", &[json!({"kinds":[1]}), json!({"kinds":[2]})]);
        assert_eq!(v.as_array().map(Vec::len), Some(4));
    }
}
