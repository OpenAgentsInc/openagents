//! What a batch from the capture script (`/static/r.js`) may hold, and how
//! it becomes stored [`Event`]s.
//!
//! Every text is bounded and scrubbed here, whatever the script sent: no
//! control characters, an email address becomes `[email]`, and a run of
//! six or more digits becomes `#`. A batch never carries an input's value;
//! the script reads none, and the fixed kinds and property keys leave no
//! field for one.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::Event;

/// The batch format's version.
pub const VERSION: u32 = 1;
/// The largest batch body, in bytes.
pub const MOST_BYTES: usize = 32 * 1024;
/// The most events one batch carries.
pub const MOST_EVENTS: usize = 60;
/// The most properties one event carries.
pub const MOST_PROPS: usize = 12;
/// The longest path kept.
pub const MOST_PATH: usize = 128;
/// The longest label kept.
pub const MOST_LABEL: usize = 80;
/// The longest property value kept.
pub const MOST_VALUE: usize = 120;
/// How far back a client clock may place an event: ten minutes.
const MOST_AGE_MS: u64 = 10 * 60 * 1000;

/// The kinds of event the script sends.
pub const KINDS: [&str; 11] = [
    "page_view",
    "click",
    "route",
    "visibility",
    "session",
    "scroll",
    "submit",
    "error",
    "engine",
    // A named milestone the page reports (`window.oaResearch.track`).
    "milestone",
    // A playtest step the server records (`playtest::events`).
    "step",
];

#[derive(Deserialize)]
struct Wire {
    v: u32,
    /// The session id; absent in aggregate mode.
    #[serde(default)]
    s: Option<String>,
    /// The client's clock when it sent the batch, in Unix milliseconds.
    #[serde(default)]
    n: Option<u64>,
    /// `1`: aggregate mode (opted out, or the browser asks not to be
    /// tracked).
    #[serde(default)]
    a: Option<u8>,
    /// `1`: this session just opted out.
    #[serde(default)]
    o: Option<u8>,
    /// Campaign tags from the landing address.
    #[serde(default)]
    u: Option<Utm>,
    #[serde(default)]
    e: Vec<WireEvent>,
}

#[derive(Default, Deserialize)]
struct Utm {
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    campaign: Option<String>,
}

#[derive(Deserialize)]
struct WireEvent {
    k: String,
    #[serde(default)]
    t: Option<u64>,
    #[serde(default)]
    p: Option<String>,
    #[serde(default)]
    l: Option<String>,
    #[serde(default)]
    x: BTreeMap<String, serde_json::Value>,
}

/// A checked batch.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Batch {
    /// `None` in aggregate mode.
    pub session: Option<String>,
    /// The session asked to stop being recorded.
    pub opt_out: bool,
    pub source: String,
    pub campaign: String,
    /// Kept events (`session`, `account` and `device` are filled by the
    /// caller).
    pub events: Vec<Event>,
    /// Events refused: an unknown kind or a malformed path.
    pub refused: usize,
}

/// Why a batch was refused whole.
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    TooLarge,
    Malformed,
}

/// A session id: 8 to 64 of `A-Z a-z 0-9 _ -`.
pub fn valid_session(id: &str) -> bool {
    (8..=64).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// A step or property name: 1 to 48 of `a-z 0-9 _`, starting with a letter.
pub fn valid_name(name: &str) -> bool {
    (1..=48).contains(&name.len())
        && name.as_bytes()[0].is_ascii_lowercase()
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// A campaign tag: lowercase, at most 48 of `a-z 0-9 _ - .`; anything else
/// is dropped.
pub fn tag(text: Option<&str>) -> String {
    let text = text.unwrap_or_default().trim().to_ascii_lowercase();
    if text.len() <= 48
        && text.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-' | b'.')
        })
    {
        text
    } else {
        String::new()
    }
}

/// A page path: begins with `/`, no query or fragment, at most
/// [`MOST_PATH`] printable characters.
pub fn path(text: &str) -> Option<String> {
    let text = text.split(['?', '#']).next().unwrap_or_default();
    (text.starts_with('/') && text.len() <= MOST_PATH && text.chars().all(|c| c.is_ascii_graphic()))
        .then(|| text.to_owned())
}

/// Bounded, scrubbed free text: control characters and runs of whitespace
/// become one space, email addresses `[email]`, digit runs of six or more
/// `#`, and the result is cut to `most` characters.
pub fn scrub(text: &str, most: usize) -> String {
    let mut words = Vec::new();
    for word in text.split_whitespace() {
        let word: String = word.chars().filter(|c| !c.is_control()).collect();
        if word.contains('@') && word.split('@').nth(1).is_some_and(|d| d.contains('.')) {
            words.push("[email]".to_owned());
            continue;
        }
        let mut out = String::new();
        let mut digits = String::new();
        for c in word.chars() {
            if c.is_ascii_digit() {
                digits.push(c);
                continue;
            }
            flush_digits(&mut out, &mut digits);
            out.push(c);
        }
        flush_digits(&mut out, &mut digits);
        words.push(out);
    }
    words.join(" ").chars().take(most).collect()
}

fn flush_digits(out: &mut String, digits: &mut String) {
    if digits.len() >= 6 {
        out.push('#');
    } else {
        out.push_str(digits);
    }
    digits.clear();
}

/// Checks a batch. `now_ms` is the server's clock.
pub fn parse(body: &[u8], now_ms: u64) -> Result<Batch, Refusal> {
    if body.len() > MOST_BYTES {
        return Err(Refusal::TooLarge);
    }
    let wire: Wire = serde_json::from_slice(body).map_err(|_| Refusal::Malformed)?;
    if wire.v != VERSION || wire.e.len() > MOST_EVENTS {
        return Err(Refusal::Malformed);
    }
    let aggregate = wire.a == Some(1);
    let session = match (&wire.s, aggregate) {
        (_, true) => None,
        (Some(id), false) if valid_session(id) => Some(id.clone()),
        _ => return Err(Refusal::Malformed),
    };
    let utm = wire.u.unwrap_or_default();
    let sent = wire.n.unwrap_or(now_ms);
    let mut batch = Batch {
        session,
        opt_out: wire.o == Some(1),
        source: tag(utm.source.as_deref()),
        campaign: tag(utm.campaign.as_deref()),
        ..Batch::default()
    };
    for event in wire.e {
        let Some(kind) = KINDS.iter().find(|k| **k == event.k && **k != "step") else {
            batch.refused += 1;
            continue;
        };
        let Some(path) = event.p.as_deref().map_or(Some(String::new()), path) else {
            batch.refused += 1;
            continue;
        };
        let age = sent
            .saturating_sub(event.t.unwrap_or(sent))
            .min(MOST_AGE_MS);
        let mut props = BTreeMap::new();
        if !aggregate {
            for (key, value) in event.x.into_iter().take(MOST_PROPS) {
                if !valid_name(&key) {
                    continue;
                }
                let value = match value {
                    serde_json::Value::String(text) => scrub(&text, MOST_VALUE),
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    _ => continue,
                };
                props.insert(key, value);
            }
        }
        batch.events.push(Event {
            at: now_ms.saturating_sub(age),
            session: String::new(),
            account: None,
            kind: (*kind).to_owned(),
            path,
            label: if aggregate {
                String::new()
            } else {
                event
                    .l
                    .as_deref()
                    .map(|l| scrub(l, MOST_LABEL))
                    .unwrap_or_default()
            },
            props,
            device: String::new(),
            source: String::new(),
            campaign: String::new(),
        });
    }
    Ok(batch)
}

/// The browser family of a user agent; the user agent itself is never
/// kept.
pub fn browser(agent: &str) -> &'static str {
    if agent.contains("Edg/") {
        "edge"
    } else if agent.contains("Firefox/") || agent.contains("FxiOS") {
        "firefox"
    } else if agent.contains("Chrome/") || agent.contains("CriOS") {
        "chrome"
    } else if agent.contains("Safari/") {
        "safari"
    } else {
        "other"
    }
}

/// The operating system family of a user agent.
pub fn system(agent: &str) -> &'static str {
    if agent.contains("iPhone") || agent.contains("iPad") || agent.contains("iPod") {
        "ios"
    } else if agent.contains("Android") {
        "android"
    } else if agent.contains("Mac OS X") || agent.contains("Macintosh") {
        "mac"
    } else if agent.contains("Windows") {
        "windows"
    } else if agent.contains("Linux") || agent.contains("CrOS") {
        "linux"
    } else {
        "other"
    }
}
