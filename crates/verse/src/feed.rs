//! Real conversations from the wider Nostr network, inside Verse.
//!
//! The NOSTR tab of the world chat window shows live public notes (kind
//! `1`) from popular relays, `relay.damus.io` and `relay.primal.net`. The
//! firehose moves faster than anyone reads, so notes queue and drip into
//! the window about one a second. Each poster also appears on the plaza as
//! a dim stand-in avatar around the pylon, with their note in a speech
//! bubble, so the world feels populated by the people actually talking.
//!
//! Notes are filtered for readable human conversation: notes behind a
//! content warning, JSON payloads, and long hex or base64 tokens are
//! skipped, text the amber font cannot draw is dropped (and a note that is
//! mostly undrawable is skipped), links are shortened, long notes are cut,
//! and each poster shows at most once a minute. Replies stay, marked as
//! such. Nothing is ever published to these relays; the feed only reads.

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

use glam::Vec3;
use serde_json::json;

use crate::chat::Line;
use crate::net::{In, Link, Out};

/// The public relays the NOSTR tab reads.
pub const RELAYS: [&str; 2] = ["wss://relay.damus.io", "wss://relay.primal.net"];
/// Lines kept in the NOSTR tab.
const HISTORY: usize = 150;
/// Notes waiting to be shown.
const QUEUE: usize = 40;
/// Time between shown notes.
const DRIP: Duration = Duration::from_millis(1100);
/// Longest note text shown, in characters.
const MAX_TEXT: usize = 220;
/// Words that keep a note off a shared screen. A blunt first filter; the
/// relays' own moderation and `content-warning` tags do the rest.
const ADULT: [&str; 10] = [
    "sex", "porn", "nsfw", "nude", "naked", "xxx", "onlyfans", "erotic", "horny", "fetish",
];

/// Shortest time between two shown notes from one poster.
const PER_POSTER: Duration = Duration::from_secs(60);
/// Stand-in avatars on the plaza.
const VISITORS: usize = 16;
/// How long a stand-in's bubble stays up.
const BUBBLE: Duration = Duration::from_secs(9);
/// Where the stand-ins gather, and how far out.
const RING_CENTER: Vec3 = Vec3::new(0.0, 0.0, 14.0);
const RING: (f32, f32) = (13.0, 24.0);

/// A Nostr poster standing on the plaza.
#[derive(Clone, Debug, PartialEq)]
pub struct Visitor {
    /// Their pubkey.
    pub pubkey: String,
    /// Where they stand.
    pub pos: Vec3,
    /// Which way they face.
    pub yaw: f32,
    /// Their latest note, while it is fresh.
    pub bubble: Option<(String, Instant)>,
    /// When they last posted.
    pub last: Instant,
}

#[derive(Clone, Debug)]
struct Note {
    pubkey: String,
    text: String,
    relay: &'static str,
    reply: bool,
}

/// The live feed.
pub struct Feed {
    links: Vec<(&'static str, Link, bool)>,
    seen: HashSet<String>,
    seen_order: VecDeque<String>,
    queue: VecDeque<Note>,
    last_drip: Option<Instant>,
    /// Lines for the NOSTR tab, oldest first.
    pub lines: VecDeque<Line>,
    names: HashMap<String, String>,
    asked: HashSet<String>,
    want: Vec<(String, usize)>,
    last_ask: Option<Instant>,
    /// Stand-ins on the plaza.
    pub visitors: Vec<Visitor>,
    /// Notes shown so far.
    pub shown: u64,
}

impl Feed {
    /// Connects to [`RELAYS`] and asks for recent and new public notes.
    #[must_use]
    pub fn start() -> Self {
        let links = RELAYS
            .iter()
            .map(|url| {
                let link = Link::start(url);
                link.send(Out::Subscribe {
                    id: "verse-notes".into(),
                    filters: vec![json!({"kinds": [1], "limit": 15})],
                    live: true,
                });
                (*url, link, false)
            })
            .collect();
        Self {
            links,
            seen: HashSet::new(),
            seen_order: VecDeque::new(),
            queue: VecDeque::new(),
            last_drip: None,
            lines: VecDeque::new(),
            names: HashMap::new(),
            asked: HashSet::new(),
            want: Vec::new(),
            last_ask: None,
            visitors: Vec::new(),
            shown: 0,
        }
    }

    /// Which relays are connected, as a short heading.
    #[must_use]
    pub fn title(&self) -> String {
        let parts: Vec<String> = self
            .links
            .iter()
            .map(|(url, _, up)| {
                let host = short_relay(url);
                if *up {
                    host.to_owned()
                } else {
                    format!("{host} (connecting)")
                }
            })
            .collect();
        format!("live public notes · {}", parts.join(" · "))
    }

    /// Drains the relays, shows a queued note when due, and asks for names.
    pub fn tick(&mut self, now: Instant) {
        let mut inbox = Vec::new();
        for (i, (_, link, up)) in self.links.iter_mut().enumerate() {
            for message in link.drain() {
                match message {
                    In::Connected => *up = true,
                    In::Disconnected(_) => *up = false,
                    other => inbox.push((i, other)),
                }
            }
        }
        for (i, message) in inbox {
            let relay = self.links[i].0;
            if let In::Event { event, .. } = message {
                match event.kind {
                    0 => {
                        if event.validate_crypto().is_ok()
                            && let Some(name) = profile_name(&event.content)
                        {
                            self.names.insert(event.pubkey.clone(), name);
                        }
                    }
                    1 => self.take(&event, relay, i),
                    _ => {}
                }
            }
        }
        self.drip(now);
        self.ask_names(now);
        for v in &mut self.visitors {
            if v.bubble.as_ref().is_some_and(|(_, until)| *until <= now) {
                v.bubble = None;
            }
        }
    }

    fn take(&mut self, event: &nostr::domain::Event, relay: &'static str, link: usize) {
        if !self.remember(&event.id) {
            return;
        }
        if event.tags.iter().any(|t| {
            t.name() == Some("content-warning")
                || (t.name() == Some("t")
                    && t.value().is_some_and(|v| v.eq_ignore_ascii_case("nsfw")))
        }) || event.validate_id().is_err()
            || event.validate_crypto().is_err()
        {
            return;
        }
        let Some(text) = tidy(&event.content) else {
            return;
        };
        if !self.names.contains_key(&event.pubkey) && self.asked.insert(event.pubkey.clone()) {
            self.want.push((event.pubkey.clone(), link));
        }
        if self.queue.iter().any(|n| n.pubkey == event.pubkey)
            || self
                .visitors
                .iter()
                .any(|v| v.pubkey == event.pubkey && v.last.elapsed() < PER_POSTER)
        {
            return;
        }
        self.queue.push_back(Note {
            pubkey: event.pubkey.clone(),
            text,
            relay,
            reply: event.tag_values("e").next().is_some(),
        });
        while self.queue.len() > QUEUE {
            self.queue.pop_front();
        }
    }

    fn remember(&mut self, id: &str) -> bool {
        if !self.seen.insert(id.to_owned()) {
            return false;
        }
        self.seen_order.push_back(id.to_owned());
        while self.seen_order.len() > 4_000 {
            if let Some(old) = self.seen_order.pop_front() {
                self.seen.remove(&old);
            }
        }
        true
    }

    fn drip(&mut self, now: Instant) {
        if self
            .last_drip
            .is_some_and(|t| now.saturating_duration_since(t) < DRIP)
        {
            return;
        }
        let Some(note) = self.queue.pop_front() else {
            return;
        };
        self.last_drip = Some(now);
        self.shown += 1;
        let name = self.name_of(&note.pubkey);
        self.lines.push_back(Line {
            channel: None,
            from: name,
            to: None,
            text: note.text.clone(),
            note: Some(format!(
                "· {}{}",
                short_relay(note.relay),
                if note.reply { " · reply" } else { "" }
            )),
        });
        while self.lines.len() > HISTORY {
            self.lines.pop_front();
        }
        self.visit(&note.pubkey, &note.text, now);
    }

    /// Puts the poster on the plaza, or refreshes them, with a bubble.
    fn visit(&mut self, pubkey: &str, text: &str, now: Instant) {
        let bubble = Some((text.to_owned(), now + BUBBLE));
        if let Some(v) = self.visitors.iter_mut().find(|v| v.pubkey == pubkey) {
            v.bubble = bubble;
            v.last = now;
            return;
        }
        if self.visitors.len() >= VISITORS
            && let Some(oldest) = self
                .visitors
                .iter()
                .enumerate()
                .min_by_key(|(_, v)| v.last)
                .map(|(i, _)| i)
        {
            self.visitors.remove(oldest);
        }
        let pos = spot(pubkey, &self.visitors);
        let to_center = RING_CENTER - pos;
        self.visitors.push(Visitor {
            pubkey: pubkey.to_owned(),
            pos,
            yaw: to_center.x.atan2(to_center.z),
            bubble,
            last: now,
        });
    }

    fn ask_names(&mut self, now: Instant) {
        if self.want.is_empty()
            || self
                .last_ask
                .is_some_and(|t| now.saturating_duration_since(t) < Duration::from_secs(2))
        {
            return;
        }
        self.last_ask = Some(now);
        let batch: Vec<(String, usize)> = self.want.drain(..).collect();
        for (i, (_, link, _)) in self.links.iter().enumerate() {
            let authors: Vec<&String> = batch
                .iter()
                .filter(|(_, l)| *l == i)
                .map(|(p, _)| p)
                .take(100)
                .collect();
            if !authors.is_empty() {
                link.send(Out::Subscribe {
                    id: format!("verse-names-{}", self.asked.len()),
                    filters: vec![json!({"kinds": [0], "authors": authors})],
                    live: false,
                });
            }
        }
    }

    /// A poster's display name, or a short key while unknown.
    #[must_use]
    pub fn name_of(&self, pubkey: &str) -> String {
        self.names
            .get(pubkey)
            .cloned()
            .unwrap_or_else(|| format!("{}…", &pubkey[..pubkey.len().min(8)]))
    }

    /// The last few notes, as plain sentences for the agent.
    #[must_use]
    pub fn recent(&self, n: usize) -> Vec<String> {
        self.lines
            .iter()
            .rev()
            .take(n)
            .map(|l| format!("{} said on Nostr: {}", l.from, l.text))
            .collect()
    }
}

/// A free spot on the ring around the pylon, chosen from the pubkey so a
/// poster tends to stand in the same place.
fn spot(pubkey: &str, taken: &[Visitor]) -> Vec3 {
    let seed = pubkey.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    });
    for k in 0..24u64 {
        let h = seed.wrapping_add(k.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        let a = (h % 3600) as f32 / 3600.0 * std::f32::consts::TAU;
        let r = RING.0 + ((h >> 16) % 1000) as f32 / 1000.0 * (RING.1 - RING.0);
        let p = RING_CENTER + Vec3::new(a.cos() * r, 0.0, a.sin() * r);
        if taken.iter().all(|v| v.pos.distance(p) > 2.5) {
            return p;
        }
    }
    RING_CENTER + Vec3::new(RING.1, 0.0, 0.0)
}

fn short_relay(url: &str) -> &str {
    let host = url
        .trim_start_matches("wss://")
        .trim_start_matches("ws://")
        .trim_end_matches('/');
    host.trim_start_matches("relay.")
        .split('.')
        .next()
        .unwrap_or(host)
}

/// Cleans a note for a chat line: one line, links shortened, cut to
/// length. `None` when nothing readable is left.
#[must_use]
pub fn tidy(content: &str) -> Option<String> {
    let trimmed = content.trim_start();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return None;
    }
    let mut total = 0usize;
    let mut kept = 0usize;
    let mut words: Vec<String> = Vec::new();
    for w in content.split_whitespace() {
        if w.starts_with("http://") || w.starts_with("https://") {
            words.push("[link]".to_owned());
            continue;
        }
        if w.starts_with("nostr:") {
            words.push("[mention]".to_owned());
            continue;
        }
        let long_token = w.chars().count() > 40
            && w.chars()
                .all(|c| c.is_ascii_alphanumeric() || "+/=_-".contains(c));
        if long_token {
            return None;
        }
        total += w.chars().count();
        let word: String = w.chars().filter(|c| crate::ui::drawable(*c)).collect();
        kept += word.chars().count();
        if !word.is_empty() {
            words.push(word);
        }
    }
    if total == 0 || kept * 100 < total * 85 {
        return None;
    }
    let links = words.iter().filter(|w| *w == "[link]").count();
    let tags = words.iter().filter(|w| w.starts_with('#')).count();
    let prose = words
        .iter()
        .filter(|w| !w.starts_with('#') && !w.starts_with('['))
        .count();
    if links > 1 || tags > 3 || prose < 3 {
        return None;
    }
    let text = words.join(" ");
    let lower = text.to_lowercase();
    if ADULT.iter().any(|w| lower.contains(w)) {
        return None;
    }
    if text.chars().count() > MAX_TEXT {
        let cut: String = text.chars().take(MAX_TEXT).collect();
        return Some(format!("{}…", cut.trim_end()));
    }
    Some(text)
}

fn profile_name(content: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(content).ok()?;
    let name = value
        .get("display_name")
        .and_then(serde_json::Value::as_str)
        .filter(|n| !n.trim().is_empty())
        .or_else(|| value.get("name").and_then(serde_json::Value::as_str))?;
    let name: String = name
        .chars()
        .filter(|c| crate::ui::drawable(*c))
        .take(24)
        .collect();
    let name = name.trim().to_owned();
    (!name.is_empty()).then_some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_are_tidied_for_a_chat_line() {
        assert_eq!(
            tidy("gm fam, look at this\n\nhttps://example.com/x.png  nostr:npub1abc").as_deref(),
            Some("gm fam, look at this [link] [mention]")
        );
        assert!(tidy("news https://a.example https://b.example today").is_none());
        assert!(tidy("#a #b #c #d hello there friends").is_none());
        assert!(tidy("https://example.com").is_none());
        assert!(tidy(r#"{"id":"p1","n":"x"}"#).is_none());
        assert!(tidy(&format!("channel:__roster {}", "ab".repeat(40))).is_none());
        assert!(tidy("ちょまどさん、プログラミング").is_none());
        assert_eq!(tidy("gm 🌍 my frens").as_deref(), Some("gm my frens"));
        assert_eq!(tidy("café au lait").as_deref(), Some("café au lait"));
        let long = "word ".repeat(100);
        assert!(tidy("an erotic game tonight").is_none());
        let cut = tidy(&long).expect("text");
        assert!(cut.chars().count() <= MAX_TEXT + 1 && cut.ends_with('…'));
    }

    #[test]
    fn relay_names_shorten() {
        assert_eq!(short_relay("wss://relay.damus.io"), "damus");
        assert_eq!(short_relay("wss://relay.primal.net"), "primal");
    }

    #[test]
    fn visitors_stand_apart_on_the_ring() {
        let mut taken = Vec::new();
        for i in 0..VISITORS {
            let p = spot(&format!("{i:064x}"), &taken);
            let d = p.distance(RING_CENTER);
            assert!(d >= RING.0 - 1e-3 && d <= RING.1 + 1e-3);
            taken.push(Visitor {
                pubkey: String::new(),
                pos: p,
                yaw: 0.0,
                bubble: None,
                last: Instant::now(),
            });
        }
    }
}
