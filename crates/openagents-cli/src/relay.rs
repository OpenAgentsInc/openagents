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

/// How long a one-shot command waits for a relay to answer.
pub const DEFAULT_WAIT: Duration = Duration::from_secs(8);
/// How often the drain loop polls the worker.
const POLL: Duration = Duration::from_millis(15);

const USAGE: &str = "usage: openagents relay COMMAND [OPTIONS]
  req FILTER_JSON [--relay URL] [--wait SECONDS] [--as PROFILE]
        Print stored events matching one NIP-01 filter, then stop.
  tail FILTER_JSON [--relay URL] [--wait SECONDS] [--as PROFILE]
        Print stored and then live events until SECONDS pass (default 30).
  publish EVENT_JSON|- [--relay URL] [--as PROFILE]
        Send a signed event and report the relay's OK.
  sign KIND CONTENT [--tag NAME=VALUE]... [--as PROFILE] [--relay URL]
        Sign an event with PROFILE's key and publish it.
--relay defaults to wss://relay.openagents.com. --as names the Verse
profile key used to answer a NIP-42 challenge (default: default).";

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
            let outcome = client.subscribe(vec![filter], live, wait, |event| {
                output.line(&serde_json::to_value(event).unwrap_or(Value::Null), summary);
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
            } else {
                source.clone()
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

/// A short text line for an event.
pub fn summary(value: &Value) -> String {
    let content = value["content"].as_str().unwrap_or("");
    let short: String = content.chars().take(80).collect();
    format!(
        "{} kind={} {} {}",
        value["created_at"],
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
}

impl Client {
    pub fn connect(url: &str, signer: RelaySigner) -> Self {
        Self {
            link: Link::start(url),
            signer,
            auth_id: None,
            subscriptions: 0,
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
            In::Ok { id, .. } if self.auth_id.as_deref() == Some(id.as_str()) => {
                self.auth_id = None;
                None
            }
            other => Some(other),
        }
    }

    /// Wait up to `wait` for the next message, answering challenges on the way.
    pub fn next(&mut self, wait: Duration) -> Option<In> {
        let deadline = Instant::now() + wait;
        loop {
            for message in self.link.drain() {
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
    /// `on_event` until the relay's end-of-stored-events marker; with `live`
    /// the subscription stays open until `wait` passes.
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
        let mut connected = false;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                self.link.send(Out::Close(id));
                return if connected || live {
                    Ok(())
                } else {
                    Err(format!("{} did not answer within {wait:?}", self.link.url))
                };
            }
            match self.next(remaining) {
                Some(In::Connected) => connected = true,
                Some(In::Event { sub, event }) if sub == id => on_event(&event),
                Some(In::Eose(sub)) if sub == id && !live => {
                    self.link.send(Out::Close(id));
                    return Ok(());
                }
                Some(In::Closed(sub, reason)) if sub == id => {
                    return Err(format!("relay closed the subscription: {reason}"));
                }
                Some(In::Notice(notice)) => eprintln!("relay: {notice}"),
                Some(In::Disconnected(reason)) if !connected => {
                    eprintln!("relay: {reason}");
                }
                Some(_) | None => {}
            }
        }
    }

    /// Publish `event` and wait for the relay's `OK`.
    ///
    /// # Errors
    /// Reports a relay that never answered.
    pub fn publish(&mut self, event: Event, wait: Duration) -> Result<Published, String> {
        let id = event.id.clone();
        if !self.link.send(Out::Publish(event)) {
            return Err("relay queue is full".into());
        }
        let deadline = Instant::now() + wait;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(format!(
                    "{} did not acknowledge within {wait:?}",
                    self.link.url
                ));
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
