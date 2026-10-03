//! One relay connection for a command: connect, answer a NIP-42 challenge
//! with the caller's signer, run subscriptions to their end-of-stored-events
//! marker, publish and wait for `OK`, or follow live events until a deadline.
//!
//! Built on [`verse::net::Link`], the bounded worker the Verse client uses,
//! so every command shares its wire limits and reconnect policy.

use std::time::{Duration, Instant};

use nostr::domain::{Event, RelaySigner, Tag};
use serde_json::{Value, json};
use verse::net::{In, Link, Out};

use crate::{Args, Output};
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};

/// How long a one-shot command waits for a relay to answer.
pub const DEFAULT_WAIT: Duration = Duration::from_secs(8);
/// How often the drain loop polls the worker.
const POLL: Duration = Duration::from_millis(15);
/// How long a connected relay may stay quiet before a one-shot query is
/// taken as complete when its end-of-stored-events marker never arrives.
const SETTLE: Duration = Duration::from_millis(1500);

pub(crate) const USAGE: &str = "usage: openagents relay COMMAND [OPTIONS]
  req FILTER_JSON [--relay URL] [--wait SECONDS] [--as PROFILE]
        Print stored events matching one NIP-01 filter, then stop.
  tail FILTER_JSON [--relay URL] [--wait SECONDS] [--as PROFILE]
        Print stored and then live events until SECONDS pass (default 30).
  publish EVENT_JSON|FILE|- [--relay URL] [--as PROFILE]
        Send a signed event (inline, from a file, or from stdin) and report
        the relay's OK.
  sign KIND CONTENT [--tag NAME=VALUE]... [--as PROFILE] [--relay URL]
        Sign an event with PROFILE's key and publish it.
--relay defaults to wss://relay.openagents.com. --as names the Verse
profile key used to answer a NIP-42 challenge (default: default).";

/// What each command above does and where the phone runs it, for the
/// chat router's command tree (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("req", Effect::ReadOnly),
    Declared::computer("tail", Effect::LongRunning),
    Declared::computer("publish", Effect::Publishes),
    Declared::computer("sign", Effect::Publishes),
];

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage("relay", "a command is required", USAGE);
    };
    if command == "--help" || command == "-h" || command == "help" {
        println!("{USAGE}");
        return 0;
    }
    let args = match Args::parse(rest, &[]) {
        Ok(args) => args,
        Err(message) => return output.usage("relay", &message, USAGE),
    };
    let signer = match signer_for(args.option("as")) {
        Ok(signer) => signer,
        Err(message) => return output.fail("relay", &message),
    };
    let url = relay_url(args.option("relay"));
    let wait = match args.number::<u64>("wait", 0) {
        Ok(seconds) => seconds,
        Err(message) => return output.usage("relay", &message, USAGE),
    };
    match command.as_str() {
        "req" | "tail" => {
            let Some(filter) = args.positional().first() else {
                return output.usage("relay", "a filter is required", USAGE);
            };
            let filter: Value = match serde_json::from_str(filter) {
                Ok(Value::Object(filter)) => Value::Object(filter),
                _ => return output.fail("relay", "the filter must be a JSON object"),
            };
            let live = command == "tail";
            let wait = Duration::from_secs(if wait == 0 {
                if live { 30 } else { DEFAULT_WAIT.as_secs() }
            } else {
                wait
            });
            let mut client = Client::connect(&url, signer);
            // A tail asks for stored events and then live ones; a relay can
            // send the same event in both, so each id prints once.
            let mut seen = std::collections::HashSet::new();
            let outcome = client.subscribe(vec![filter], live, wait, |event| {
                let value = serde_json::to_value(event).unwrap_or(Value::Null);
                if first_sighting(&mut seen, &value) {
                    output.line(&value, summary);
                }
            });
            match outcome {
                Ok(()) => 0,
                Err(message) => output.fail("relay", &message),
            }
        }
        "publish" => {
            let Some(source) = args.positional().first() else {
                return output.usage("relay", "an event is required", USAGE);
            };
            let text = if source == "-" {
                let mut text = String::new();
                if let Err(error) = std::io::Read::read_to_string(&mut std::io::stdin(), &mut text)
                {
                    return output.fail("relay", &format!("cannot read stdin: {error}"));
                }
                text
            } else if source.trim_start().starts_with('{') {
                source.clone()
            } else {
                match std::fs::read_to_string(source) {
                    Ok(text) => text,
                    Err(error) => {
                        return output.fail("relay", &format!("cannot read {source}: {error}"));
                    }
                }
            };
            let event: Event = match serde_json::from_str(&text) {
                Ok(event) => event,
                Err(error) => return output.fail("relay", &format!("invalid event: {error}")),
            };
            if let Err(error) = event.validate_crypto() {
                return output.fail("relay", &format!("event does not verify: {error}"));
            }
            let mut client = Client::connect(&url, signer);
            report_publish(output, client.publish(event, DEFAULT_WAIT))
        }
        "sign" => {
            let [kind, content, ..] = args.positional() else {
                return output.usage("relay", "KIND and CONTENT are required", USAGE);
            };
            let Ok(kind) = kind.parse::<u16>() else {
                return output.usage("relay", "KIND is a number", USAGE);
            };
            let mut tags = Vec::new();
            for tag in args.options("tag") {
                let Some((name, value)) = tag.split_once('=') else {
                    return output.usage("relay", "--tag takes NAME=VALUE", USAGE);
                };
                tags.push(Tag::new(vec![name.to_owned(), value.to_owned()]));
            }
            let event = signer.sign(unix_now(), kind, tags, content.clone());
            let mut client = Client::connect(&url, signer);
            report_publish(output, client.publish(event, DEFAULT_WAIT))
        }
        other => output.usage("relay", &format!("unknown command `{other}`"), USAGE),
    }
}

fn report_publish(output: &Output, result: Result<Published, String>) -> u8 {
    match result {
        Ok(published) => {
            output.emit(
                &json!({
                    "id": published.id,
                    "accepted": published.accepted,
                    "message": published.message,
                }),
                |value| {
                    format!(
                        "{} {} {}",
                        if value["accepted"].as_bool().unwrap_or(false) {
                            "accepted"
                        } else {
                            "refused"
                        },
                        value["id"].as_str().unwrap_or(""),
                        value["message"].as_str().unwrap_or("")
                    )
                    .trim_end()
                    .to_owned()
                },
            );
            if published.accepted {
                0
            } else {
                crate::EXIT_FAILURE
            }
        }
        Err(message) => output.fail("relay", &message),
    }
}

/// Whether `value`'s id is new to `seen`; events without an id always print.
fn first_sighting(seen: &mut std::collections::HashSet<String>, value: &Value) -> bool {
    match value["id"].as_str() {
        Some(id) => seen.insert(id.to_owned()),
        None => true,
    }
}

/// `2026-10-03 14:05 UTC` for Unix seconds; JSON output keeps the raw number.
pub fn when(at: u64) -> String {
    let minutes = (at % 86_400) / 60;
    format!(
        "{} {:02}:{:02} UTC",
        crate::wallet::date(at),
        minutes / 60,
        minutes % 60
    )
}

/// A short text line for an event.
pub fn summary(value: &Value) -> String {
    let content = value["content"].as_str().unwrap_or("");
    let short: String = content.chars().take(80).collect();
    format!(
        "{} kind={} {} {}",
        value["created_at"]
            .as_u64()
            .map_or_else(|| value["created_at"].to_string(), when),
        value["kind"],
        value["pubkey"]
            .as_str()
            .map(|key| &key[..key.len().min(12)])
            .unwrap_or(""),
        short.replace('\n', " ")
    )
}

/// The relay a command uses: `--relay`, `OPENAGENTS_RELAY`, or the public relay.
pub fn relay_url(flag: Option<&str>) -> String {
    flag.map(str::to_owned)
        .or_else(|| std::env::var("OPENAGENTS_RELAY").ok())
        .unwrap_or_else(|| verse::session::PUBLIC_RELAY.to_owned())
}

/// The signer for a Verse profile, creating its key on first use.
pub fn signer_for(profile: Option<&str>) -> Result<RelaySigner, String> {
    identity_for(profile).map(|identity| identity.signer)
}

/// The identity for a Verse profile, creating its key on first use.
pub fn identity_for(profile: Option<&str>) -> Result<verse::identity::Identity, String> {
    let profile = profile
        .map(str::to_owned)
        .or_else(|| std::env::var("OPENAGENTS_PROFILE").ok())
        .unwrap_or_else(|| "default".to_owned());
    verse::identity::load_or_create(&verse::identity::home(), &profile)
}

/// The identity a read-only command signs relay AUTH with: the profile's
/// key when one exists, else a temporary key that is never saved, so
/// reading creates no identity on disk (#10320).
pub fn reader_identity_for(profile: Option<&str>) -> Result<verse::identity::Identity, String> {
    let profile = profile
        .map(str::to_owned)
        .or_else(|| std::env::var("OPENAGENTS_PROFILE").ok())
        .unwrap_or_else(|| "default".to_owned());
    verse::identity::load_or_ephemeral(&verse::identity::home(), &profile)
}

pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// The relay's answer to a publish.
#[derive(Clone, Debug)]
pub struct Published {
    pub id: String,
    pub accepted: bool,
    pub message: String,
}

/// One authenticated relay connection.
pub struct Client {
    link: Link,
    signer: RelaySigner,
    auth_id: Option<String>,
    subscriptions: u32,
    connected: bool,
    authenticated: bool,
    /// Live subscriptions opened with [`Client::listen`], and the events
    /// they received while another call was waiting on the relay.
    listening: Vec<String>,
    inbox: Vec<Event>,
    pending: std::collections::VecDeque<In>,
}

impl Client {
    pub fn connect(url: &str, signer: RelaySigner) -> Self {
        Self {
            link: Link::start(url),
            signer,
            auth_id: None,
            subscriptions: 0,
            connected: false,
            authenticated: false,
            listening: Vec::new(),
            inbox: Vec::new(),
            pending: std::collections::VecDeque::new(),
        }
    }

    /// Open a live subscription that stays up across publishes. Read its
    /// events with [`Client::recv`].
    ///
    /// # Errors
    /// Reports a full relay queue.
    // Only Unix-only groups call it.
    #[cfg_attr(not(unix), allow(dead_code))]
    pub fn listen(&mut self, filters: Vec<Value>) -> Result<String, String> {
        self.subscriptions += 1;
        let id = format!("oa-{}", self.subscriptions);
        if !self.link.send(Out::Subscribe {
            id: id.clone(),
            filters,
            live: true,
        }) {
            return Err("relay queue is full".into());
        }
        self.listening.push(id.clone());
        Ok(id)
    }

    /// The next event on any [`Client::listen`] subscription, or `None`
    /// after `wait`.
    // Only Unix-only groups call it.
    #[cfg_attr(not(unix), allow(dead_code))]
    pub fn recv(&mut self, wait: Duration) -> Option<Event> {
        let deadline = Instant::now() + wait;
        loop {
            if !self.inbox.is_empty() {
                return Some(self.inbox.remove(0));
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return None;
            }
            match self.next(remaining) {
                Some(In::Notice(notice)) => eprintln!("relay: {notice}"),
                Some(In::Closed(sub, reason)) if self.listening.contains(&sub) => {
                    eprintln!("relay closed {sub}: {reason}");
                    self.listening.retain(|open| *open != sub);
                }
                Some(_) | None => {}
            }
        }
    }

    /// Handle a NIP-42 challenge; everything else is returned to the caller.
    fn intercept(&mut self, message: In) -> Option<In> {
        match message {
            In::Auth(challenge) => {
                let event = self.signer.sign(
                    unix_now(),
                    22_242,
                    vec![
                        Tag::new(vec!["relay".into(), self.link.url.clone()]),
                        Tag::new(vec!["challenge".into(), challenge]),
                    ],
                    String::new(),
                );
                self.auth_id = Some(event.id.clone());
                self.link.send(Out::Auth(event));
                None
            }
            In::Ok { id, accepted, .. } if self.auth_id.as_deref() == Some(id.as_str()) => {
                self.auth_id = None;
                self.authenticated = accepted;
                None
            }
            In::Event { sub, event } if self.listening.contains(&sub) => {
                self.inbox.push(*event);
                None
            }
            other => Some(other),
        }
    }

    /// Wait up to `wait` for the next message, answering challenges on the way.
    pub fn next(&mut self, wait: Duration) -> Option<In> {
        let deadline = Instant::now() + wait;
        loop {
            if self.pending.is_empty() {
                self.pending.extend(self.link.drain());
            }
            while let Some(message) = self.pending.pop_front() {
                match &message {
                    In::Connected => self.connected = true,
                    In::Disconnected(_) => self.connected = false,
                    _ => {}
                }
                if let Some(message) = self.intercept(message) {
                    return Some(message);
                }
            }
            if Instant::now() >= deadline {
                return None;
            }
            std::thread::sleep(POLL);
        }
    }

    /// Run `filters` as one subscription. Stored events are handed to
    /// `on_event` until the relay's end-of-stored-events marker, or until
    /// the relay has stayed quiet for `SETTLE` after its last event; with
    /// `live` the subscription stays open until `wait` passes.
    ///
    /// # Errors
    /// Reports a relay that closed the subscription or never answered.
    pub fn subscribe(
        &mut self,
        filters: Vec<Value>,
        live: bool,
        wait: Duration,
        mut on_event: impl FnMut(&Event),
    ) -> Result<(), String> {
        self.subscriptions += 1;
        let id = format!("oa-{}", self.subscriptions);
        if !self.link.send(Out::Subscribe {
            id: id.clone(),
            filters,
            live,
        }) {
            return Err("relay queue is full".into());
        }
        let deadline = Instant::now() + wait;
        let mut settled_by: Option<Instant> = None;
        loop {
            let now = Instant::now();
            if let Some(settle) = settled_by
                && !live
                && now >= settle
            {
                self.link.send(Out::Close(id));
                return Ok(());
            }
            let mut remaining = deadline.saturating_duration_since(now);
            if remaining.is_zero() {
                self.link.send(Out::Close(id));
                return if self.connected || live {
                    Ok(())
                } else {
                    Err(format!("{} did not answer within {wait:?}", self.link.url))
                };
            }
            if let Some(settle) = settled_by {
                remaining = remaining.min(settle.saturating_duration_since(now));
            }
            let message = self.next(remaining);
            if self.connected && !live && (message.is_some() || settled_by.is_none()) {
                settled_by = Some(Instant::now() + SETTLE);
            }
            match message {
                Some(In::Event { sub, event }) if sub == id => on_event(&event),
                Some(In::Eose(sub)) if sub == id && !live => {
                    self.link.send(Out::Close(id));
                    return Ok(());
                }
                Some(In::Closed(sub, reason)) if sub == id => {
                    return Err(format!("relay closed the subscription: {reason}"));
                }
                Some(In::Notice(notice)) => eprintln!("relay: {notice}"),
                Some(In::Disconnected(reason)) if !self.connected => {
                    eprintln!("relay: {reason}");
                }
                Some(_) | None => {}
            }
        }
    }

    /// Publish `event` and wait for the relay's `OK`. An `auth-required`
    /// refusal that arrives while the NIP-42 answer is still in flight is
    /// retried once after the relay accepts the answer.
    ///
    /// # Errors
    /// Reports a relay that never answered.
    pub fn publish(&mut self, event: Event, wait: Duration) -> Result<Published, String> {
        let deadline = Instant::now() + wait;
        let mut retried = false;
        loop {
            let published = self.publish_once(event.clone(), deadline)?;
            let needs_auth = !published.accepted && published.message.starts_with("auth-required");
            if !needs_auth || retried || !self.authenticate(deadline) {
                return Ok(published);
            }
            retried = true;
        }
    }

    /// Wait until the relay has accepted this client's NIP-42 answer, or
    /// until `deadline`. Returns whether the client is authenticated.
    pub fn authenticate(&mut self, deadline: Instant) -> bool {
        while !self.authenticated {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            if let Some(In::Notice(notice)) = self.next(remaining.min(Duration::from_millis(500))) {
                eprintln!("relay: {notice}");
            }
        }
        self.authenticated
    }

    fn publish_once(&mut self, event: Event, deadline: Instant) -> Result<Published, String> {
        let id = event.id.clone();
        if !self.link.send(Out::Publish(event)) {
            return Err("relay queue is full".into());
        }
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(format!("{} did not acknowledge in time", self.link.url));
            }
            match self.next(remaining) {
                Some(In::Ok {
                    id: acknowledged,
                    accepted,
                    message,
                }) if acknowledged == id => {
                    return Ok(Published {
                        id,
                        accepted,
                        message,
                    });
                }
                Some(In::Notice(notice)) => eprintln!("relay: {notice}"),
                Some(_) | None => {}
            }
        }
    }

    /// Publish without waiting for acknowledgment.
    pub fn send(&mut self, event: Event) -> bool {
        self.link.send(Out::Publish(event))
    }

    pub fn close(mut self) {
        self.link.shutdown(Duration::from_millis(100));
    }
}

#[cfg(test)]
mod shakeout_tests {
    use super::*;

    #[test]
    fn a_tail_prints_each_event_once() {
        let mut seen = std::collections::HashSet::new();
        let event = json!({"id": "abc", "created_at": 1, "kind": 1});
        assert!(first_sighting(&mut seen, &event));
        assert!(!first_sighting(&mut seen, &event));
        assert!(first_sighting(&mut seen, &json!({"kind": 1})));
    }

    #[test]
    fn event_lines_show_a_readable_time() {
        assert_eq!(when(1_790_996_212), "2026-10-03 02:56 UTC");
        let line = summary(&json!({
            "created_at": 1_790_996_212u64, "kind": 1,
            "pubkey": "3e7e662614f5aaaa", "content": "hi"
        }));
        assert!(
            line.starts_with("2026-10-03 02:56 UTC kind=1 3e7e662614f5 hi"),
            "{line}"
        );
    }
}
