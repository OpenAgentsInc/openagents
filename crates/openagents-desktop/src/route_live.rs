//! The route map with today's real traffic: the deck scene
//! `scene: routes-live`, beside `routes-future` (#10198).
//!
//! It draws through the same view as the simulation ([`MapPage`] over the
//! committed map, laid out by `Layout::of`, the layout the pay host's
//! `/flow/snapshot` topology carries, so the web page at
//! `openagents.com/live` and this scene agree), but its dots come from the
//! public flow stream (`openagents.flow-event.v1`, section 7 of
//! `docs/payments/2026-10-02-central-receive-and-splits.md`), never from a
//! seeded clock:
//!
//! | event | dot |
//! | --- | --- |
//! | `call` | white, from the router out to the node |
//! | `payment` | gold, from the node back to the router |
//! | `share` | gold, from the router out past the node to its author |
//! | `payout` | gold, from the router straight to the author's wallet |
//! | `bonus` | gold with a ring, from the router out to the author |
//! | `run` | white, from the router out to Coder |
//!
//! An event's `node` names a map node (`plugin:outline` finds the plugin
//! `crates/plugin-outline`); one the map doesn't have runs to
//! its nearest known place (Coder for plugins and hosted resources, the
//! router otherwise). Events on one node wait for the one before to land,
//! so a call's white dot goes out before its payment comes back.
//!
//! When the stream can't be reached the scene says so and draws no dots.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::time::{Duration, Instant};

use openagents_chat_app::route_map::layout::{Camera, Layout, Point};
use openagents_chat_app::route_map::{Kind, Map};
use openagents_chat_app::visual;
use rust_native::style::{Color, TextAlign};
use rust_native_desktop::text::{Fonts, font};
use rust_native_desktop::{Frame, PxRect};
use serde::Deserialize;

use crate::route_future::{payment, request};
use crate::route_map::{MapPage, Pulse};

/// The public flow stream openagents.com proxies to the pay host.
pub const FLOW_URL: &str = "https://openagents.com/api/flow";
/// Overrides [`FLOW_URL`]: an `https://…/flow` base, or `file:PATH` to
/// replay a JSON-lines fixture.
pub const FLOW_URL_ENV: &str = "OPENAGENTS_FLOW_URL";

/// How long a dot takes from the router out to a node, or back.
pub const TRIP: f32 = 1.6;
/// How long a share takes out past the node to its author.
pub const SHARE: f32 = 2.0;
/// How long a payout takes to the author's wallet.
pub const PAYOUT: f32 = 1.4;
/// The longest an event waits for the one before it on its node.
pub const MAX_WAIT: f32 = 3.0;
/// How far past its node an author sits, and its wallet, in world units.
pub const AUTHOR: f32 = 34.0;
pub const WALLET: f32 = 64.0;
/// How far apart a fixture replays its events, in seconds.
pub const FIXTURE_PACE: f32 = 0.8;

/// Below 2^41 sats an f64's spacing is under half a millisatoshi, so a
/// fractional amount still reads back exactly to three places.
const MAX_FRACTIONAL_SATS: f64 = 2_199_023_255_552.0;

/// Exact sats on the wire, stored internally as integer millisatoshis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Sats(u64);

impl Sats {
    pub const fn from_msat(msat: u64) -> Self {
        Self(msat)
    }

    pub const fn msat(self) -> u64 {
        self.0
    }

    fn checked_add(self, other: Self) -> Result<Self, &'static str> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or("Flow amount overflow")
    }
}

impl<'de> Deserialize<'de> for Sats {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let number = serde_json::Number::deserialize(deserializer)?;
        // A fractional amount arrives as an f64; refuse one too large to
        // read back exactly rather than show a wrong number.
        if number.is_f64()
            && number
                .as_f64()
                .is_none_or(|sats| sats >= MAX_FRACTIONAL_SATS)
        {
            return Err(serde::de::Error::custom(
                "Fractional amount too large to be exact",
            ));
        }
        exact_msat(&number.to_string())
            .map(Self)
            .ok_or_else(|| serde::de::Error::custom("Expected nonnegative exact millisatoshis"))
    }
}

fn exact_msat(text: &str) -> Option<u64> {
    if text.starts_with('-') {
        return None;
    }
    let (mantissa, exponent) = match text.split_once(['e', 'E']) {
        Some((mantissa, exponent)) => (mantissa, exponent.parse::<i64>().ok()?),
        None => (text, 0),
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits = format!("{whole}{fraction}");
    let digits = digits.trim_start_matches('0');
    if digits.is_empty() {
        return Some(0);
    }
    let scale = exponent
        .checked_add(3)?
        .checked_sub(fraction.len() as i64)?;
    if scale >= 0 {
        let scale = u32::try_from(scale).ok()?;
        if digits.len().checked_add(scale as usize)? > 20 {
            return None;
        }
        digits
            .parse::<u64>()
            .ok()?
            .checked_mul(10u64.checked_pow(scale)?)
    } else {
        let places = usize::try_from(scale.checked_neg()?).ok()?;
        let keep = digits.len().checked_sub(places)?;
        if !digits[keep..].bytes().all(|b| b == b'0') {
            return None;
        }
        digits[..keep].parse().ok()
    }
}

impl std::fmt::Display for Sats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let whole = grouped(self.0 / 1000);
        let fraction = self.0 % 1000;
        if fraction == 0 {
            f.write_str(&whole)
        } else {
            write!(
                f,
                "{whole}.{}",
                format!("{fraction:03}").trim_end_matches('0')
            )
        }
    }
}

/// One public flow event (`openagents.flow-event.v1`). Every field the
/// scene doesn't draw is optional, so a newer producer still parses.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct FlowEvent {
    #[serde(default)]
    pub v: u32,
    #[serde(default)]
    pub seq: u64,
    /// Milliseconds since the epoch.
    #[serde(default)]
    pub at: i64,
    #[serde(rename = "type")]
    pub kind: EventKind,
    #[serde(default)]
    pub resource: Option<String>,
    #[serde(default)]
    pub plugin: Option<String>,
    #[serde(default)]
    pub node: String,
    #[serde(default)]
    pub amount_sats: Option<Sats>,
    #[serde(default)]
    pub split: BTreeMap<String, Sats>,
    #[serde(default)]
    pub author: Option<String>,
}

/// What an event says happened.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Call,
    Payment,
    Share,
    Payout,
    Bonus,
    Run,
    /// A type a newer producer added; drawn as nothing.
    #[serde(other)]
    Other,
}

/// The running totals under the map.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct Totals {
    #[serde(default)]
    pub received_sats: Sats,
    #[serde(default)]
    pub paid_out_sats: Sats,
    #[serde(default)]
    pub calls: u64,
}

impl Totals {
    /// Counts one more event.
    pub fn count(&mut self, event: &FlowEvent) -> Result<(), &'static str> {
        let amount = event.amount_sats.unwrap_or_default();
        match event.kind {
            EventKind::Call => {
                self.calls = self
                    .calls
                    .checked_add(1)
                    .ok_or("Flow call count overflow")?
            }
            EventKind::Payment => self.received_sats = self.received_sats.checked_add(amount)?,
            EventKind::Payout => self.paid_out_sats = self.paid_out_sats.checked_add(amount)?,
            _ => {}
        }
        Ok(())
    }
}

/// `GET /flow/snapshot`: the last events and the totals. The topology it
/// carries is the committed map's layout, which this scene computes
/// itself.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Snapshot {
    #[serde(default)]
    pub events: Vec<FlowEvent>,
    #[serde(default)]
    pub totals: Totals,
}

/// A stop on a dot's way: a node, or a point `distance` past a node,
/// away from its parent (where its author and their wallet sit).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stop {
    Node(usize),
    Past(usize, f32),
}

/// One dot an event sends: its way, its color, whether it wears a ring,
/// and how long it takes.
#[derive(Clone, Debug, PartialEq)]
pub struct Leg {
    pub stops: Vec<Stop>,
    pub color: Color,
    pub ring: bool,
    pub seconds: f32,
}

/// The node an event's dot runs to.
pub fn target(map: &Map, event: &FlowEvent) -> usize {
    if let Some(index) = map.find(&event.node) {
        return index;
    }
    let name = event
        .node
        .strip_prefix("plugin:")
        .or(event.plugin.as_deref())
        .filter(|name| !name.is_empty());
    if let Some(name) = name {
        let found = map.nodes.iter().position(|node| {
            node.kind == Kind::Plugin
                && node.id.strip_prefix("plugin:").is_some_and(|dir| {
                    let last = dir.rsplit('/').next().unwrap_or(dir);
                    last == name || last.strip_prefix("plugin-") == Some(name)
                })
        });
        if let Some(index) = found {
            return index;
        }
    }
    let near = match event.resource.as_deref() {
        Some("plugin" | "hosted_resource" | "coder") => map.find("coder"),
        _ if event.kind == EventKind::Run => map.find("coder"),
        _ => None,
    };
    near.or_else(|| map.find("front")).unwrap_or(0)
}

/// The nodes from the map's root down to `leaf`.
pub fn path_to(map: &Map, leaf: usize) -> Vec<usize> {
    let mut path = vec![leaf];
    let mut at = map.nodes[leaf].parent;
    while let Some(parent) = at {
        path.push(parent);
        at = map.nodes[parent].parent;
    }
    path.reverse();
    path
}

/// The dots one event sends, in order.
pub fn legs(map: &Map, event: &FlowEvent) -> Vec<Leg> {
    let leaf = target(map, event);
    let out: Vec<Stop> = path_to(map, leaf).into_iter().map(Stop::Node).collect();
    let root = out.first().copied().unwrap_or(Stop::Node(leaf));
    let back: Vec<Stop> = out.iter().rev().copied().collect();
    let mut to_author = out.clone();
    to_author.push(Stop::Past(leaf, AUTHOR));
    let leg = |stops: Vec<Stop>, color, ring, seconds| Leg {
        stops,
        color,
        ring,
        seconds,
    };
    match event.kind {
        EventKind::Call | EventKind::Run => vec![leg(out, request(), false, TRIP)],
        EventKind::Payment => vec![leg(back, payment(), false, TRIP)],
        EventKind::Share => vec![leg(to_author, payment(), false, SHARE)],
        EventKind::Bonus => vec![leg(to_author, payment(), true, SHARE)],
        EventKind::Payout => vec![leg(
            vec![root, Stop::Past(leaf, WALLET)],
            payment(),
            false,
            PAYOUT,
        )],
        EventKind::Other => Vec::new(),
    }
}

/// Where a stop sits in `layout`.
pub fn place(map: &Map, layout: &Layout, stop: Stop) -> Point {
    match stop {
        Stop::Node(index) => layout.positions[index],
        Stop::Past(index, distance) => {
            let at = layout.positions[index];
            let from = map.nodes[index]
                .parent
                .map_or(Point::default(), |parent| layout.positions[parent]);
            let (dx, dy) = (at.x - from.x, at.y - from.y);
            let length = (dx * dx + dy * dy).sqrt();
            if length < 0.001 {
                return Point::new(at.x + distance, at.y);
            }
            Point::new(at.x + dx / length * distance, at.y + dy / length * distance)
        }
    }
}

/// The point `t` of the way along `stops`, each hop taking the same time.
pub fn along(map: &Map, layout: &Layout, stops: &[Stop], t: f32) -> Point {
    match stops {
        [] => Point::default(),
        [only] => place(map, layout, *only),
        _ => {
            let hops = (stops.len() - 1) as f32;
            let at = (t.clamp(0.0, 1.0) * hops).min(hops - 0.0001);
            let hop = at.floor() as usize;
            let (a, b) = (
                place(map, layout, stops[hop]),
                place(map, layout, stops[hop + 1]),
            );
            let f = at - hop as f32;
            Point::new(a.x + (b.x - a.x) * f, a.y + (b.y - a.y) * f)
        }
    }
}

/// A dot under way: its leg and when it set off, in the scene's seconds.
#[derive(Clone, Debug, PartialEq)]
pub struct Flight {
    pub seq: u64,
    pub leg: Leg,
    pub start: f32,
}

/// Schedules events into flights: each leg sets off when it arrives, or
/// once the dot before it on the same node has landed, waiting at most
/// [`MAX_WAIT`].
#[derive(Clone, Debug, Default)]
pub struct Schedule {
    ready: HashMap<usize, f32>,
    pub flights: Vec<Flight>,
}

impl Schedule {
    /// Adds `event`, arrived at `now`.
    pub fn push(&mut self, map: &Map, event: &FlowEvent, now: f32) {
        let leaf = target(map, event);
        for leg in legs(map, event) {
            let ready = self.ready.get(&leaf).copied().unwrap_or(now);
            let start = ready.clamp(now, now + MAX_WAIT);
            self.ready.insert(leaf, start + leg.seconds);
            self.flights.push(Flight {
                seq: event.seq,
                leg,
                start,
            });
        }
    }

    /// Drops the flights that have landed by `now`.
    pub fn land(&mut self, now: f32) {
        self.flights.retain(|f| f.start + f.leg.seconds > now);
    }

    /// The dots at `now`. With `still`, each sits where it lands.
    pub fn pulses(&self, map: &Map, layout: &Layout, now: f32, still: bool) -> Vec<Pulse> {
        let mut out = Vec::new();
        for flight in &self.flights {
            let t = (now - flight.start) / flight.leg.seconds.max(0.01);
            if !(0.0..1.0).contains(&t) {
                continue;
            }
            let t = if still { 1.0 } else { t };
            let gold = crate::route_future::is_payment(flight.leg.color);
            out.push(Pulse {
                at: along(map, layout, &flight.leg.stops, t),
                color: flight.leg.color,
                radius: if gold { 3.2 } else { 3.0 },
                ring: flight.leg.ring,
            });
        }
        out
    }
}

/// Parses a server-sent event stream a line at a time.
#[derive(Clone, Debug, Default)]
pub struct SseParser {
    id: Option<String>,
    data: String,
}

impl SseParser {
    /// Takes one line (without its line ending); at a blank line, gives
    /// the event it ended: its id and its data.
    pub fn line(&mut self, line: &str) -> Option<(Option<String>, String)> {
        if line.is_empty() {
            let data = std::mem::take(&mut self.data);
            let id = self.id.take();
            return (!data.is_empty()).then_some((id, data));
        }
        if line.starts_with(':') {
            return None;
        }
        let (field, value) = line.split_once(':').unwrap_or((line, ""));
        let value = value.strip_prefix(' ').unwrap_or(value);
        match field {
            "data" => {
                if !self.data.is_empty() {
                    self.data.push('\n');
                }
                self.data.push_str(value);
            }
            "id" => self.id = Some(value.to_string()),
            _ => {}
        }
        None
    }
}

/// Where the scene's events come from.
#[derive(Clone, Debug, PartialEq)]
pub enum FlowSource {
    /// The pay host's flow endpoints under this base (`…/flow`):
    /// `snapshot`, then `stream` with `Last-Event-ID` resume.
    Url(String),
    /// A fixture's events, replayed [`FIXTURE_PACE`] apart on the frame
    /// clock, for captures and tests.
    Fixture(Vec<FlowEvent>),
}

impl FlowSource {
    /// [`FLOW_URL_ENV`] if set (`file:PATH` replays a fixture), otherwise
    /// [`FLOW_URL`].
    pub fn from_env() -> Self {
        match std::env::var(FLOW_URL_ENV) {
            Ok(value) if value.starts_with("file:") => {
                let path = value
                    .trim_start_matches("file://")
                    .trim_start_matches("file:");
                FlowSource::Fixture(
                    std::fs::read_to_string(path)
                        .map(|text| fixture(&text))
                        .unwrap_or_default(),
                )
            }
            Ok(value) if !value.trim().is_empty() => FlowSource::Url(value.trim().to_string()),
            _ => FlowSource::Url(FLOW_URL.to_string()),
        }
    }
}

/// A JSON-lines fixture's events; lines that don't parse are skipped.
pub fn fixture(text: &str) -> Vec<FlowEvent> {
    text.lines()
        .filter_map(|line| serde_json::from_str(line.trim()).ok())
        .collect()
}

/// What the feed tells the scene.
#[derive(Clone, Debug)]
pub enum Feed {
    Snapshot(Snapshot),
    Event(FlowEvent),
    Connected,
    Dropped,
    Invalid,
}

/// Where the stream stands, as the scene says it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Connecting,
    Live,
    Unreachable,
    Invalid,
}

/// The live scene.
pub struct RouteLive {
    page: MapPage,
    layout: Layout,
    schedule: Schedule,
    totals: Totals,
    /// When the last event happened, in milliseconds since the epoch.
    last_at: Option<i64>,
    last_seq: u64,
    status: Status,
    started: Option<Instant>,
    seconds: f32,
    reduce_motion: bool,
    /// A fixture's events still to come, and when each comes.
    replay: VecDeque<(f32, FlowEvent)>,
    feed: Option<Receiver<Feed>>,
    stop: Arc<AtomicBool>,
    fonts: Fonts,
}

impl Drop for RouteLive {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl RouteLive {
    /// The scene over the committed map, fed from `source`.
    pub fn new(source: FlowSource, reduce_motion: bool) -> Self {
        let mut map = Map::committed();
        // The gaps are the Map page's to show; this scene shows traffic.
        map.gaps.clear();
        for node in &mut map.nodes {
            node.gaps.clear();
        }
        let layout = Layout::of(&map);
        let mut live = RouteLive {
            page: MapPage::presenting(map, reduce_motion),
            layout,
            schedule: Schedule::default(),
            totals: Totals::default(),
            last_at: None,
            last_seq: 0,
            status: Status::Connecting,
            started: None,
            seconds: 0.0,
            reduce_motion,
            replay: VecDeque::new(),
            feed: None,
            stop: Arc::new(AtomicBool::new(false)),
            fonts: Fonts::new(),
        };
        match source {
            FlowSource::Fixture(events) => {
                live.status = Status::Live;
                live.replay = events
                    .into_iter()
                    .enumerate()
                    .map(|(k, event)| (k as f32 * FIXTURE_PACE, event))
                    .collect();
            }
            FlowSource::Url(base) => {
                let (send, receive) = std::sync::mpsc::channel();
                let stop = live.stop.clone();
                std::thread::Builder::new()
                    .name("flow-stream".into())
                    .spawn(move || follow(&base, &send, &stop))
                    .ok();
                live.feed = Some(receive);
            }
        }
        live
    }

    /// The scene fed by `feed`, as a host thread or a test sends it.
    pub fn fed(feed: Receiver<Feed>, reduce_motion: bool) -> Self {
        let mut live = RouteLive::new(FlowSource::Fixture(Vec::new()), reduce_motion);
        live.status = Status::Connecting;
        live.feed = Some(feed);
        live
    }

    pub fn map(&self) -> &Map {
        self.page.map()
    }

    pub fn status(&self) -> Status {
        self.status
    }

    pub fn totals(&self) -> Totals {
        self.totals
    }

    /// The flights in the air or waiting to set off.
    pub fn flights(&self) -> &[Flight] {
        &self.schedule.flights
    }

    pub fn seconds(&self) -> f32 {
        self.seconds
    }

    pub fn version(&self) -> u64 {
        self.page.version()
    }

    /// Moves the clock to `now`, takes what the feed sent, and releases
    /// the fixture's events that are due.
    pub fn advance(&mut self, now: Instant) {
        let started = *self.started.get_or_insert(now);
        // The clock only goes forward, whoever's frame asks.
        self.seconds = self
            .seconds
            .max(now.saturating_duration_since(started).as_secs_f32());
        self.take();
    }

    /// Takes what is due at the current time.
    fn take(&mut self) {
        while self
            .replay
            .front()
            .is_some_and(|(at, _)| *at <= self.seconds)
        {
            if let Some((due, event)) = self.replay.pop_front() {
                // On its own time, even when a frame comes late.
                self.arrive(Feed::Event(event), due);
            }
        }
        let mut messages = Vec::new();
        if let Some(feed) = &self.feed {
            loop {
                match feed.try_recv() {
                    Ok(message) => messages.push(message),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        messages.push(Feed::Dropped);
                        break;
                    }
                }
            }
        }
        for message in messages {
            self.receive(message);
        }
        self.schedule.land(self.seconds);
    }

    /// Takes one message from the feed at the current time.
    pub fn receive(&mut self, message: Feed) {
        self.arrive(message, self.seconds);
    }

    /// Takes one message that arrived `at` seconds into the scene.
    fn arrive(&mut self, message: Feed, at: f32) {
        match message {
            Feed::Snapshot(snapshot) => {
                self.status = Status::Live;
                self.totals = snapshot.totals;
                if let Some(last) = snapshot.events.iter().max_by_key(|e| e.seq) {
                    self.last_at = Some(last.at);
                    self.last_seq = self.last_seq.max(last.seq);
                }
            }
            Feed::Event(event) => {
                if self.status == Status::Invalid {
                    return;
                }
                if event.seq != 0 && event.seq <= self.last_seq {
                    return;
                }
                if self.totals.count(&event).is_err() {
                    self.status = Status::Invalid;
                    self.schedule.flights.clear();
                    return;
                }
                self.status = Status::Live;
                self.last_seq = self.last_seq.max(event.seq);
                self.last_at = Some(event.at);
                let map = self.page.map();
                self.schedule.push(map, &event, at);
            }
            Feed::Connected if self.status != Status::Invalid => self.status = Status::Live,
            Feed::Connected => {}
            Feed::Invalid => {
                self.status = Status::Invalid;
                self.schedule.flights.clear();
            }
            Feed::Dropped => {
                if self.status != Status::Invalid {
                    self.status = Status::Unreachable;
                }
                // No dots while the stream is down.
                self.schedule.flights.clear();
            }
        }
    }

    /// The dots at the current time.
    pub fn pulses(&self) -> Vec<Pulse> {
        if matches!(self.status, Status::Unreachable | Status::Invalid) {
            return Vec::new();
        }
        self.schedule.pulses(
            self.page.map(),
            &self.layout,
            self.seconds,
            self.reduce_motion,
        )
    }

    /// The line under the map: where the stream stands and the last event.
    pub fn status_line(&self) -> String {
        let last = self
            .last_at
            .map(|at| format!("last event {}", utc(at)))
            .unwrap_or_else(|| "no events yet".into());
        match self.status {
            Status::Connecting => "Connecting to the flow stream".into(),
            Status::Live => format!("Live \u{b7} {last}"),
            Status::Unreachable => format!("The flow stream is unreachable \u{b7} {last}"),
            Status::Invalid => format!("The flow stream has invalid data \u{b7} {last}"),
        }
    }

    /// The totals line.
    pub fn totals_line(&self) -> String {
        format!(
            "{} sats received \u{b7} {} sats paid out \u{b7} {} calls",
            self.totals.received_sats,
            self.totals.paid_out_sats,
            grouped(self.totals.calls)
        )
    }

    /// Paints the scene into `rect`, in pixels, with `unit` pixels a point.
    pub fn paint(&mut self, frame: &mut Frame, rect: PxRect, unit: f32) {
        self.take();
        let mut layout = self.layout.clone();
        let n = layout.positions.len();
        let shown = vec![1.0_f32; n];
        let size = (rect.w / unit.max(0.1), rect.h / unit.max(0.1));
        let camera = fit(&layout, (size.0, size.1));
        let boost = (0.55 / camera.zoom.max(0.01)).sqrt().clamp(1.0, 2.2);
        for r in &mut layout.radii {
            *r *= boost;
        }
        let traffic = self.pulses();
        self.page.set_unit(unit);
        self.page.set_frame(layout, shown, traffic, camera);
        self.page.paint(frame, rect);
        let lines = [
            (self.status_line(), 24.0, 230),
            (self.totals_line(), 16.0, 170),
        ];
        let mut y = rect.y + rect.h - 18.0 * unit;
        for (text, size, alpha) in lines.iter().rev() {
            let size = size * unit;
            let paragraph = self.fonts.paragraph(
                text,
                font(size, rust_native::layout::display::Weight::Semibold, false),
                None,
            );
            let line = size * rust_native_desktop::text::LINE_EM;
            y -= line;
            self.fonts.draw(
                frame,
                &paragraph,
                rect.x + 22.0 * unit,
                y,
                paragraph.width + 4.0,
                TextAlign::Start,
                1.0,
                Color {
                    alpha: *alpha,
                    ..visual::current().text
                },
            );
            y -= 4.0 * unit;
        }
    }
}

/// The camera fitting the whole map, with room under it for the lines.
fn fit(layout: &Layout, size: (f32, f32)) -> Camera {
    let mut min = Point::new(f32::MAX, f32::MAX);
    let mut max = Point::new(f32::MIN, f32::MIN);
    for (index, p) in layout.positions.iter().enumerate() {
        let r = layout.radii[index] + WALLET;
        min = Point::new(min.x.min(p.x - r), min.y.min(p.y - r));
        max = Point::new(max.x.max(p.x + r), max.y.max(p.y + r));
    }
    if min.x > max.x {
        return Camera::default();
    }
    Camera::fit((min, max), size.0.max(1.0), (size.1 - 60.0).max(1.0), 24.0)
}

/// `1204` as `1,204`.
pub fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (k, c) in digits.chars().enumerate() {
        if k > 0 && (digits.len() - k).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Milliseconds since the epoch as `2026-10-02 14:03:12 UTC`.
pub fn utc(ms: i64) -> String {
    let seconds = ms.div_euclid(1000);
    let days = seconds.div_euclid(86_400);
    let of_day = seconds.rem_euclid(86_400);
    // Days to a civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC",
        of_day / 3600,
        of_day % 3600 / 60,
        of_day % 60
    )
}

/// Follows the pay host's flow endpoints until `stop`: the snapshot, then
/// the stream, resuming from the last id after a drop, backing off up to
/// 30 seconds between tries.
fn follow(base: &str, send: &Sender<Feed>, stop: &AtomicBool) {
    use std::io::BufRead;
    let base = base.trim_end_matches('/');
    let client = match reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(None)
        .build()
    {
        Ok(client) => client,
        Err(_) => {
            let _ = send.send(Feed::Dropped);
            return;
        }
    };
    let mut last: Option<String> = None;
    let mut wait = Duration::from_secs(2);
    while !stop.load(Ordering::Relaxed) {
        if last.is_none()
            && let Ok(response) = client
                .get(format!("{base}/snapshot"))
                .timeout(Duration::from_secs(15))
                .send()
            && response.status().is_success()
        {
            let snapshot = match response.json::<Snapshot>() {
                Ok(snapshot) => snapshot,
                Err(_) => {
                    let _ = send.send(Feed::Invalid);
                    return;
                }
            };
            last = snapshot
                .events
                .iter()
                .map(|e| e.seq)
                .max()
                .map(|s| s.to_string());
            if send.send(Feed::Snapshot(snapshot)).is_err() {
                return;
            }
        }
        let mut request = client
            .get(format!("{base}/stream"))
            .header("Accept", "text/event-stream");
        if let Some(id) = &last {
            request = request.header("Last-Event-ID", id.as_str());
        }
        match request.send() {
            Ok(response) if response.status().is_success() => {
                wait = Duration::from_secs(2);
                if send.send(Feed::Connected).is_err() {
                    return;
                }
                let mut parser = SseParser::default();
                for line in std::io::BufReader::new(response).lines() {
                    if stop.load(Ordering::Relaxed) {
                        return;
                    }
                    let Ok(line) = line else { break };
                    let Some((id, data)) = parser.line(line.trim_end_matches('\r')) else {
                        continue;
                    };
                    let event = match serde_json::from_str::<FlowEvent>(&data) {
                        Ok(event) => event,
                        Err(_) => {
                            let _ = send.send(Feed::Invalid);
                            return;
                        }
                    };
                    last = id.or_else(|| Some(event.seq.to_string()));
                    if send.send(Feed::Event(event)).is_err() {
                        return;
                    }
                }
            }
            _ => {}
        }
        if send.send(Feed::Dropped).is_err() {
            return;
        }
        let until = Instant::now() + wait;
        while Instant::now() < until {
            if stop.load(Ordering::Relaxed) {
                return;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        wait = (wait * 2).min(Duration::from_secs(30));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../../docs/payments/fixtures/flow-stream.jsonl");

    fn event(line: &str) -> FlowEvent {
        serde_json::from_str(line).expect("the event parses")
    }

    #[test]
    fn the_fixture_parses_every_type() {
        let events = fixture(FIXTURE);
        assert_eq!(events.len(), 12);
        for kind in [
            EventKind::Call,
            EventKind::Payment,
            EventKind::Share,
            EventKind::Payout,
            EventKind::Bonus,
            EventKind::Run,
        ] {
            assert!(events.iter().any(|e| e.kind == kind), "{kind:?}");
        }
        let unknown = event(r#"{"v":1,"seq":9,"at":0,"type":"refund","node":"front"}"#);
        assert_eq!(unknown.kind, EventKind::Other);
        assert!(legs(&Map::committed(), &unknown).is_empty());
    }

    #[test]
    fn an_events_node_finds_its_place_on_the_map() {
        let map = Map::committed();
        let id = |i: usize| map.nodes[i].id.clone();
        let short = event(r#"{"type":"call","resource":"plugin","node":"plugin:outline"}"#);
        assert_eq!(id(target(&map, &short)), "plugin:crates/plugin-outline");
        // The hosted runner's sample plugins are not on the map: Coder.
        let sample = event(r#"{"type":"call","resource":"plugin","node":"plugin:explain-error"}"#);
        assert_eq!(id(target(&map, &sample)), "coder");
        let exact = event(r#"{"type":"call","node":"engine:codex"}"#);
        assert_eq!(id(target(&map, &exact)), "engine:codex");
        let new =
            event(r#"{"type":"call","resource":"plugin","node":"plugin:not-yet-on-the-map"}"#);
        assert_eq!(id(target(&map, &new)), "coder");
        let run = event(r#"{"type":"run","resource":"coder","node":"coder"}"#);
        assert_eq!(id(target(&map, &run)), "coder");
        let elsewhere = event(r#"{"type":"call","resource":"route","node":"route:nowhere"}"#);
        assert_eq!(id(target(&map, &elsewhere)), "front");
    }

    #[test]
    fn each_event_type_sends_its_dot() {
        let map = Map::committed();
        let front = map.find("front").unwrap();
        let plugin = map.find("plugin:crates/plugin-outline").unwrap();
        let at = |kind: &str| {
            let e = event(&format!(
                r#"{{"type":"{kind}","resource":"plugin","node":"plugin:outline","amount_sats":5}}"#
            ));
            let legs = legs(&map, &e);
            assert_eq!(legs.len(), 1, "{kind}");
            legs.into_iter().next().unwrap()
        };
        // A call: white, from the router out to the plugin.
        let call = at("call");
        assert_eq!(call.color, request());
        assert_eq!(call.stops.first(), Some(&Stop::Node(front)));
        assert_eq!(call.stops.last(), Some(&Stop::Node(plugin)));
        assert!(call.stops.len() >= 4);
        // A payment: gold, the same way back.
        let paid = at("payment");
        assert_eq!(paid.color, payment());
        let back: Vec<Stop> = call.stops.iter().rev().copied().collect();
        assert_eq!(paid.stops, back);
        // A share: gold, out past the plugin to its author.
        let share = at("share");
        assert_eq!(share.color, payment());
        assert!(!share.ring);
        assert_eq!(share.stops.last(), Some(&Stop::Past(plugin, AUTHOR)));
        // A bonus: the same, with a ring.
        let bonus = at("bonus");
        assert!(bonus.ring);
        assert_eq!(bonus.stops, share.stops);
        // A payout: gold, from the router straight to the wallet.
        let payout = at("payout");
        assert_eq!(payout.color, payment());
        assert_eq!(
            payout.stops,
            vec![Stop::Node(front), Stop::Past(plugin, WALLET)]
        );
        // A run: white, out to Coder.
        let run = legs(
            &map,
            &event(r#"{"type":"run","resource":"coder","node":"coder"}"#),
        );
        assert_eq!(run[0].color, request());
        assert_eq!(
            run[0].stops.last(),
            Some(&Stop::Node(map.find("coder").unwrap()))
        );
    }

    #[test]
    fn the_author_and_the_wallet_sit_past_the_node_away_from_its_parent() {
        let map = Map::committed();
        let layout = Layout::of(&map);
        let plugin = map.find("plugin:crates/plugin-outline").unwrap();
        let parent = map.nodes[plugin].parent.unwrap();
        let (p, q) = (layout.positions[parent], layout.positions[plugin]);
        let author = place(&map, &layout, Stop::Past(plugin, AUTHOR));
        let wallet = place(&map, &layout, Stop::Past(plugin, WALLET));
        let from_parent = |x: Point| ((x.x - p.x).powi(2) + (x.y - p.y).powi(2)).sqrt();
        assert!(from_parent(author) > from_parent(q));
        assert!(from_parent(wallet) > from_parent(author));
        let gap = ((author.x - q.x).powi(2) + (author.y - q.y).powi(2)).sqrt();
        assert!((gap - AUTHOR).abs() < 0.01, "{gap}");
    }

    #[test]
    fn a_payment_waits_for_its_call_to_land() {
        let map = Map::committed();
        // The recorded stream names sample plugins, which are not on the
        // map; two real plugins stand in so each keeps its own node.
        let events = fixture(
            &FIXTURE
                .replace("plugin:explain-error", "plugin:outline")
                .replace("plugin:code-search", "plugin:action-items"),
        );
        let mut schedule = Schedule::default();
        // The call, its payment, and its share arrive together.
        for e in &events[..3] {
            schedule.push(&map, e, 10.0);
        }
        let starts: Vec<f32> = schedule.flights.iter().map(|f| f.start).collect();
        assert_eq!(starts[0], 10.0);
        assert_eq!(starts[1], 10.0 + TRIP);
        // The share waits too, but never longer than MAX_WAIT.
        assert_eq!(starts[2], 10.0 + MAX_WAIT);
        // Another node's call sets off at once.
        schedule.push(&map, &events[3], 10.0);
        assert_eq!(schedule.flights[3].start, 10.0);
        // Mid flight, the call is a white dot between router and plugin.
        let layout = Layout::of(&map);
        let pulses = schedule.pulses(&map, &layout, 10.8, false);
        assert_eq!(pulses.len(), 2);
        assert!(pulses.iter().all(|p| p.color == request()));
        // Then the gold one comes back.
        let later = schedule.pulses(&map, &layout, 10.0 + TRIP + 0.5, false);
        assert!(later.iter().any(|p| p.color == payment()));
        // Landed flights go.
        schedule.land(100.0);
        assert!(schedule.flights.is_empty());
        assert!(schedule.pulses(&map, &layout, 100.0, false).is_empty());
    }

    #[test]
    fn the_stream_is_read_as_server_sent_events() {
        let mut parser = SseParser::default();
        assert_eq!(parser.line(": keep-alive"), None);
        assert_eq!(parser.line(""), None);
        assert_eq!(parser.line("id: 7"), None);
        assert_eq!(parser.line(r#"data: {"type":"call","#), None);
        assert_eq!(parser.line(r#"data: "seq":7}"#), None);
        assert_eq!(
            parser.line(""),
            Some((Some("7".into()), "{\"type\":\"call\",\n\"seq\":7}".into()))
        );
        let mut parser = SseParser::default();
        parser.line(r#"data:{"type":"run","seq":8,"node":"coder"}"#);
        let (_, data) = parser.line("").unwrap();
        assert_eq!(event(&data).seq, 8);
    }

    #[test]
    fn a_fixture_replays_on_the_frame_clock_and_counts_its_totals() {
        let mut live = RouteLive::new(FlowSource::Fixture(fixture(FIXTURE)), false);
        let start = Instant::now();
        live.advance(start);
        assert_eq!(live.status(), Status::Live);
        assert_eq!(live.flights().len(), 1);
        live.advance(start + Duration::from_secs_f32(FIXTURE_PACE * 11.0 + 0.01));
        assert_eq!(
            live.totals(),
            Totals {
                received_sats: Sats::from_msat(64_000),
                paid_out_sats: Sats::from_msat(40_000),
                calls: 4,
            }
        );
        assert!(!live.pulses().is_empty());
        assert!(
            live.status_line()
                .starts_with("Live \u{b7} last event 2026-")
        );
        assert_eq!(
            live.totals_line(),
            "64 sats received \u{b7} 40 sats paid out \u{b7} 4 calls"
        );
        // Long after, everything has landed and nothing is made up.
        live.advance(start + Duration::from_secs(120));
        assert!(live.pulses().is_empty());
    }

    #[test]
    fn an_unreachable_stream_says_so_and_draws_nothing() {
        let (send, receive) = std::sync::mpsc::channel();
        let mut live = RouteLive::fed(receive, false);
        let start = Instant::now();
        live.advance(start);
        assert_eq!(live.status(), Status::Connecting);
        assert!(live.pulses().is_empty());
        send.send(Feed::Snapshot(Snapshot {
            events: fixture(FIXTURE)[..2].to_vec(),
            totals: Totals {
                received_sats: Sats::from_msat(31_000),
                paid_out_sats: Sats::default(),
                calls: 1,
            },
        }))
        .unwrap();
        live.advance(start + Duration::from_millis(10));
        // The snapshot's events are history: counted, not drawn.
        assert_eq!(live.status(), Status::Live);
        assert!(live.pulses().is_empty());
        assert_eq!(live.totals().received_sats.msat(), 31_000);
        // A replayed event the snapshot already had is skipped.
        send.send(Feed::Event(fixture(FIXTURE)[1].clone())).unwrap();
        send.send(Feed::Event(fixture(FIXTURE)[2].clone())).unwrap();
        live.advance(start + Duration::from_millis(500));
        assert_eq!(live.flights().len(), 1);
        assert_eq!(live.totals().received_sats.msat(), 31_000);
        send.send(Feed::Dropped).unwrap();
        live.advance(start + Duration::from_millis(600));
        assert_eq!(live.status(), Status::Unreachable);
        assert!(live.pulses().is_empty());
        assert!(
            live.status_line()
                .starts_with("The flow stream is unreachable \u{b7} last event ")
        );
        drop(send);
        live.advance(start + Duration::from_millis(700));
        assert_eq!(live.status(), Status::Unreachable);
    }

    #[test]
    fn numbers_and_times_read_plainly() {
        assert_eq!(grouped(0), "0");
        assert_eq!(grouped(1204), "1,204");
        assert_eq!(grouped(1_234_567), "1,234,567");
        assert_eq!(utc(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(utc(1_791_043_200_123), "2026-10-03 16:00:00 UTC");
    }
}

#[cfg(test)]
#[path = "route_live_tests.rs"]
mod fractional_tests;
