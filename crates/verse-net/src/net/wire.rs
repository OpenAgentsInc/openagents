//! The relay client's protocol state, apart from any socket: the
//! subscriptions to restore after a reconnect or an accepted NIP-42
//! answer, the durable events held while offline, and the frames to write
//! when a socket opens, a command comes from the game, or a message
//! arrives. The browser link drives it from WebSocket callbacks; the native
//! worker shares its offline queue and filters.
use super::{In, MAX_WIRE, Out, QUEUE, parse};
use nostr::domain::Event;
use serde_json::{Value, json};
use std::collections::{BTreeMap, VecDeque};

/// Open subscriptions by id: their filters and whether they stay live.
pub(super) type Subscriptions = BTreeMap<String, (Vec<Value>, bool)>;

/// The notice a refused offline command leaves for the game.
pub(super) const QUEUE_FULL: &str = "Relay queue is full; an unsent operation was refused.";

/// One relay connection's protocol state across reconnects.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
#[derive(Debug, Default)]
pub(super) struct Wire {
    subscriptions: Subscriptions,
    backlog: VecDeque<Event>,
    auth_id: Option<String>,
    open: bool,
}

/// What one step asks of the socket and the game.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
#[derive(Debug, Default)]
pub(super) struct Step {
    /// Text frames to write, in order.
    pub writes: Vec<String>,
    /// A message for the game.
    pub message: Option<In>,
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
impl Wire {
    /// A socket opened: restore every subscription, then send what waited.
    pub fn opened(&mut self) -> Step {
        self.open = true;
        self.auth_id = None;
        let mut writes: Vec<String> = self
            .subscriptions
            .iter()
            .map(|(id, (filters, _))| req(id, filters).to_string())
            .collect();
        writes.extend(
            self.backlog
                .drain(..)
                .map(|event| json!(["EVENT", event]).to_string()),
        );
        Step {
            writes,
            message: Some(In::Connected),
        }
    }

    /// The socket closed; commands queue offline until the next one opens.
    pub fn closed(&mut self) {
        self.open = false;
        self.auth_id = None;
    }

    /// A command from the game.
    pub fn command(&mut self, out: Out) -> Step {
        if !self.open {
            let refused = !retain_offline(out, &mut self.subscriptions, &mut self.backlog);
            return Step {
                writes: vec![],
                message: refused.then(|| In::Notice(QUEUE_FULL.into())),
            };
        }
        let write = |value: Value| Step {
            writes: vec![value.to_string()],
            message: None,
        };
        match out {
            Out::Publish(event) => write(json!(["EVENT", event])),
            Out::Auth(event) => {
                self.auth_id = Some(event.id.clone());
                write(json!(["AUTH", event]))
            }
            Out::Subscribe { id, filters, live } => {
                if self.subscriptions.len() >= QUEUE && !self.subscriptions.contains_key(&id) {
                    return Step {
                        writes: vec![],
                        message: Some(In::Closed(id, "restricted: subscription bound".into())),
                    };
                }
                let frame = req(&id, &filters);
                self.subscriptions.insert(id, (filters, live));
                write(frame)
            }
            Out::Close(id) => {
                self.subscriptions.remove(&id);
                write(json!(["CLOSE", id]))
            }
        }
    }

    /// A text frame from the relay. `None` is a frame that breaks the
    /// protocol; the caller closes the socket.
    pub fn received(&mut self, text: &str) -> Option<Step> {
        if text.len() > MAX_WIRE {
            return None;
        }
        let message = parse(text)?;
        let mut writes = Vec::new();
        if matches!(&message, In::Ok { id, accepted: true, .. } if self.auth_id.as_ref() == Some(id))
        {
            self.auth_id = None;
            writes.extend(
                self.subscriptions
                    .iter()
                    .map(|(id, (filters, _))| req(id, filters).to_string()),
            );
        }
        if let In::Eose(id) = &message
            && self.subscriptions.get(id).is_some_and(|(_, live)| !live)
        {
            self.subscriptions.remove(id);
            writes.push(json!(["CLOSE", id]).to_string());
        }
        Some(Step {
            writes,
            message: Some(message),
        })
    }
}

/// Keeps `command` for the next connection: subscriptions and durable
/// events, but never motion or an authentication answer, which must not
/// replay. False means the bounded queue is full.
pub(super) fn retain_offline(
    command: Out,
    subscriptions: &mut Subscriptions,
    backlog: &mut VecDeque<Event>,
) -> bool {
    match command {
        Out::Subscribe { id, filters, live } => {
            if subscriptions.len() >= QUEUE && !subscriptions.contains_key(&id) {
                return false;
            }
            subscriptions.insert(id, (filters, live));
        }
        Out::Close(id) => {
            subscriptions.remove(&id);
        }
        Out::Publish(event) if !(20_000..30_000).contains(&event.kind) => {
            backlog.retain(|old| old.id != event.id && !same_address(old, &event));
            if backlog.len() >= QUEUE {
                return false;
            }
            backlog.push_back(event);
        }
        // Motion and authentication from a previous connection must not replay.
        _ => {}
    }
    true
}

fn same_address(a: &Event, b: &Event) -> bool {
    if a.kind != b.kind || a.pubkey != b.pubkey {
        return false;
    }
    match a.kind {
        0 | 3 | 10_000..20_000 => true,
        30_000..40_000 => a.tag_values("d").next() == b.tag_values("d").next(),
        _ => false,
    }
}

/// A NIP-01 `REQ` for `id` with every filter.
pub(super) fn req(id: &str, filters: &[Value]) -> Value {
    let mut message = vec![json!("REQ"), json!(id)];
    message.extend(filters.iter().cloned());
    Value::Array(message)
}

/// Whether `commands` fit one batch: at most a queue's worth, each within
/// the wire bound.
pub(super) fn batch_fits(commands: &[Out]) -> bool {
    !commands.is_empty()
        && commands.len() <= QUEUE
        && commands.iter().all(|out| {
            let size = match out {
                Out::Publish(e) | Out::Auth(e) => {
                    serde_json::to_vec(e).map_or(usize::MAX, |b| b.len())
                }
                Out::Subscribe { id, filters, .. } => {
                    if id.len() > 128 || filters.len() > 16 {
                        usize::MAX
                    } else {
                        req(id, filters).to_string().len()
                    }
                }
                Out::Close(id) => id.len(),
            };
            size <= MAX_WIRE
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signer() -> nostr::domain::RelaySigner {
        nostr::domain::RelaySigner::from_secret_hex(&"01".repeat(32)).unwrap()
    }

    fn kind(frame: &str) -> String {
        let value: Value = serde_json::from_str(frame).unwrap();
        value[0].as_str().unwrap().to_owned()
    }

    #[test]
    fn an_offline_wire_keeps_subscriptions_and_states_and_drops_motion() {
        let mut wire = Wire::default();
        let signer = signer();
        for out in [
            Out::Subscribe {
                id: "live".into(),
                filters: vec![json!({"kinds":[23300]})],
                live: true,
            },
            Out::Publish(signer.sign(1, 33301, vec![], "state".into())),
            Out::Publish(signer.sign(1, 23300, vec![], "pose".into())),
            Out::Auth(signer.sign(1, 22242, vec![], String::new())),
        ] {
            let step = wire.command(out);
            assert!(step.writes.is_empty() && step.message.is_none());
        }
        let opened = wire.opened();
        assert!(matches!(opened.message, Some(In::Connected)));
        let kinds: Vec<_> = opened.writes.iter().map(|w| kind(w)).collect();
        assert_eq!(kinds, ["REQ", "EVENT"]);
        assert!(opened.writes[1].contains("\"state\""));
    }

    #[test]
    fn an_open_wire_writes_at_once_and_restores_after_authentication() {
        let mut wire = Wire::default();
        wire.opened();
        let step = wire.command(Out::Subscribe {
            id: "world".into(),
            filters: vec![json!({"kinds":[33301]})],
            live: true,
        });
        assert_eq!(step.writes.len(), 1);
        let auth = signer().sign(1, 22242, vec![], String::new());
        let step = wire.command(Out::Auth(auth.clone()));
        assert_eq!(kind(&step.writes[0]), "AUTH");
        let ok = json!(["OK", auth.id, true, ""]).to_string();
        let step = wire.received(&ok).unwrap();
        assert!(matches!(step.message, Some(In::Ok { accepted: true, .. })));
        assert_eq!(step.writes.len(), 1);
        assert!(step.writes[0].contains("\"world\""));
        // A second acceptance of the same answer restores nothing.
        assert!(wire.received(&ok).unwrap().writes.is_empty());
    }

    #[test]
    fn a_one_shot_subscription_closes_after_its_stored_events() {
        let mut wire = Wire::default();
        wire.opened();
        wire.command(Out::Subscribe {
            id: "me".into(),
            filters: vec![],
            live: false,
        });
        let step = wire.received(r#"["EOSE","me"]"#).unwrap();
        assert_eq!(step.writes, [r#"["CLOSE","me"]"#]);
        wire.closed();
        // It is not restored on the next connection.
        assert!(wire.opened().writes.is_empty());
    }

    #[test]
    fn a_broken_frame_ends_the_connection() {
        let mut wire = Wire::default();
        wire.opened();
        assert!(wire.received("not json").is_none());
        assert!(wire.received(&"x".repeat(MAX_WIRE + 1)).is_none());
    }

    #[test]
    fn batches_are_bounded() {
        assert!(!batch_fits(&[]));
        let close = || Out::Close("a".into());
        assert!(batch_fits(&[close()]));
        assert!(!batch_fits(&vec![close(); QUEUE + 1]));
        assert!(!batch_fits(&[Out::Subscribe {
            id: "x".repeat(129),
            filters: vec![],
            live: true
        }]));
    }
}
