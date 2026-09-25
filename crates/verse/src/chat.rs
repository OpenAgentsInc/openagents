//! Chat: channels, input parsing, limits, and the two chat windows.
//!
//! The structure follows Horse Isle 1's chat, retold in the amber ladder:
//!
//! - **Channels.** ALL reaches the whole world, ADS is for trades and
//!   announcements, ZONE reaches the named district you stand in, NEAR
//!   reaches about one screen around you, HERE reaches the spot you stand
//!   on, a ROOM reaches a NIP-29 group, and a PM reaches one player.
//! - **Methods.** There are no tabs. A method selector left of the input
//!   picks the channel (`Tab` cycles it), and a `/` shortcut overrides it
//!   for one line: `/a`, `/$`, `/z`, `/n`, `/h`, `/r room`, `/ai` for your
//!   own agent, or `/name text` for a private message.
//! - **Two windows.** The left window carries world-wide lines (ALL, ADS,
//!   ZONE, system notices); the right window carries personal ones (NEAR,
//!   HERE, rooms, PMs, and your agent's gestures).
//! - **Limits.** ALL and ADS lines are at most 150 characters, ADS once a
//!   minute, ALL from a budget that refills over time, and no shouting in
//!   capitals.
//!
//! Nothing here touches the network. `session` carries lines over Nostr.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use glam::Vec3;

/// Longest ALL or ADS line, in characters.
pub const MAX_BROADCAST: usize = 150;
/// Longest line on any other channel, in characters.
pub const MAX_LINE: usize = 500;
/// Time between ADS posts.
pub const ADS_INTERVAL: Duration = Duration::from_secs(60);
/// ALL posts a player may bank.
pub const ALL_BUDGET: u32 = 15;
/// Time to earn one ALL post.
pub const ALL_REFILL: Duration = Duration::from_secs(20);
/// NEAR reach in meters, about one screen.
pub const NEAR_RADIUS: f32 = 40.0;
/// HERE reach in meters: the same spot.
pub const HERE_RADIUS: f32 = 3.0;
/// Lines kept per window.
const HISTORY: usize = 200;

/// Where a line goes.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Channel {
    /// The whole world.
    All,
    /// Trades and announcements, the whole world.
    Ads,
    /// The named district.
    Zone,
    /// About one screen around the speaker.
    Near,
    /// The speaker's spot.
    Here,
    /// A NIP-29 group, by id.
    Room(String),
    /// A private message to a pubkey.
    Pm(String),
    /// A private conversation with your own agent. Never leaves this
    /// machine except to the model that answers for the agent.
    Agent,
}

impl Channel {
    /// The channel's tag value on the wire (`t` tag), for public channels.
    #[must_use]
    pub fn slug(&self) -> Option<&'static str> {
        match self {
            Channel::All => Some("all"),
            Channel::Ads => Some("ads"),
            Channel::Zone => Some("zone"),
            Channel::Near => Some("near"),
            Channel::Here => Some("here"),
            Channel::Room(_) | Channel::Pm(_) | Channel::Agent => None,
        }
    }

    /// The public channel for a wire tag value.
    #[must_use]
    pub fn from_slug(slug: &str) -> Option<Channel> {
        match slug {
            "all" => Some(Channel::All),
            "ads" => Some(Channel::Ads),
            "zone" => Some(Channel::Zone),
            "near" => Some(Channel::Near),
            "here" => Some(Channel::Here),
            _ => None,
        }
    }

    /// Which window shows it.
    #[must_use]
    pub fn window(&self) -> Window {
        match self {
            Channel::All | Channel::Ads | Channel::Zone => Window::World,
            _ => Window::Personal,
        }
    }

    /// Whether lines on this channel float over the speaker's head.
    #[must_use]
    pub fn overhead(&self) -> bool {
        matches!(
            self,
            Channel::All | Channel::Zone | Channel::Near | Channel::Here
        )
    }
}

/// The two chat windows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Window {
    /// Bottom left: world-wide chat and notices.
    World,
    /// Bottom right: nearby, rooms, and private chat.
    Personal,
}

/// One line in a chat window.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    /// The channel, or `None` for a system notice.
    pub channel: Option<Channel>,
    /// Speaker's display name.
    pub from: String,
    /// For a PM this player sent, the recipient's name.
    pub to: Option<String>,
    /// The text.
    pub text: String,
    /// A sender-only audience note, such as `[3 near]`.
    pub note: Option<String>,
}

impl Line {
    /// A system notice.
    #[must_use]
    pub fn system(text: impl Into<String>) -> Self {
        Self {
            channel: None,
            from: String::new(),
            to: None,
            text: text.into(),
            note: None,
        }
    }
}

/// Both windows' history.
#[derive(Clone, Debug, Default)]
pub struct Log {
    /// Bottom-left lines, oldest first.
    pub world: VecDeque<Line>,
    /// Bottom-right lines, oldest first.
    pub personal: VecDeque<Line>,
}

impl Log {
    /// Appends a line to the window its channel belongs to.
    pub fn push(&mut self, line: Line) {
        let window = line.channel.as_ref().map_or(Window::World, Channel::window);
        let lines = match window {
            Window::World => &mut self.world,
            Window::Personal => &mut self.personal,
        };
        lines.push_back(line);
        while lines.len() > HISTORY {
            lines.pop_front();
        }
    }
}

/// What a submitted input line asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Send `text` on `channel`.
    Send(Channel, String),
    /// Send a PM to the player whose name starts with `name`.
    Whisper(String, String),
    /// Point the method selector at the player whose name starts with
    /// `name`.
    Target(String),
    /// Stop showing a channel: `!mute all`.
    Mute(String),
    /// Show a channel again: `!unmute all`.
    Unmute(String),
    /// Nothing to do.
    Nothing,
}

/// Parses one submitted line, given the method currently selected.
#[must_use]
pub fn parse(input: &str, method: &Channel) -> Command {
    let input = input.trim();
    if input.is_empty() {
        return Command::Nothing;
    }
    if let Some(rest) = input.strip_prefix('!') {
        let mut words = rest.split_whitespace();
        let verb = words.next().unwrap_or_default().to_ascii_lowercase();
        let what = words.next().unwrap_or("all").to_ascii_lowercase();
        return match verb.as_str() {
            "mute" => Command::Mute(what),
            "unmute" | "hear" => Command::Unmute(what),
            _ => Command::Send(method.clone(), input.to_owned()),
        };
    }
    let Some(rest) = input.strip_prefix('/') else {
        return Command::Send(method.clone(), input.to_owned());
    };
    let (word, text) = rest.split_once(' ').unwrap_or((rest, ""));
    let text = text.trim().to_owned();
    let channel = match word.to_ascii_lowercase().as_str() {
        "a" | "all" => Some(Channel::All),
        "$" | "ads" => Some(Channel::Ads),
        "z" | "zone" | "i" | "isle" => Some(Channel::Zone),
        "n" | "near" => Some(Channel::Near),
        "h" | "here" => Some(Channel::Here),
        "ai" | "agent" => Some(Channel::Agent),
        "r" | "room" => {
            let (room, text) = text.split_once(' ').unwrap_or((text.as_str(), ""));
            return if room.is_empty() {
                Command::Nothing
            } else {
                Command::Send(Channel::Room(room.to_owned()), text.trim().to_owned())
            };
        }
        _ => None,
    };
    match channel {
        Some(_) if text.is_empty() => Command::Nothing,
        Some(channel) => Command::Send(channel, text),
        None if text.is_empty() => Command::Target(word.to_owned()),
        None => Command::Whisper(word.to_owned(), text),
    }
}

/// Why a line was not sent, worded after Horse Isle's own notices.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Too long for ALL or ADS.
    TooLong(&'static str),
    /// ADS posted less than a minute ago.
    AdsTooSoon,
    /// The ALL budget is spent.
    AllSpent,
    /// A word of five or more letters in capitals.
    Shouting,
    /// Empty after trimming.
    Empty,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::TooLong(which) => write!(
                f,
                "CHAT NOT SENT: {which} chats are limited to {MAX_BROADCAST} characters. Keep it brief."
            ),
            Refusal::AdsTooSoon => {
                write!(f, "CHAT NOT SENT: Ads may only be posted once per minute.")
            }
            Refusal::AllSpent => write!(
                f,
                "CHAT NOT SENT: ALL chats are limited (+1 earned per {} seconds). Use ZONE, NEAR, or HERE when you can.",
                ALL_REFILL.as_secs()
            ),
            Refusal::Shouting => write!(
                f,
                "Please do not use all CAPS, it looks as if you are yelling."
            ),
            Refusal::Empty => write!(f, "Nothing to send."),
        }
    }
}

/// The sending limits for one player.
#[derive(Clone, Debug)]
pub struct Limits {
    last_ad: Option<Instant>,
    budget: u32,
    refilled: Instant,
}

impl Limits {
    /// Full budget at `now`.
    #[must_use]
    pub fn new(now: Instant) -> Self {
        Self {
            last_ad: None,
            budget: ALL_BUDGET,
            refilled: now,
        }
    }

    /// Checks `text` for `channel` at `now`, spending budget if it passes.
    ///
    /// # Errors
    ///
    /// Returns why the line may not be sent.
    pub fn admit(&mut self, channel: &Channel, text: &str, now: Instant) -> Result<(), Refusal> {
        if text.trim().is_empty() {
            return Err(Refusal::Empty);
        }
        if shouting(text) {
            return Err(Refusal::Shouting);
        }
        let len = text.chars().count();
        match channel {
            Channel::All | Channel::Ads if len > MAX_BROADCAST => {
                return Err(Refusal::TooLong(if *channel == Channel::All {
                    "ALL"
                } else {
                    "ADS"
                }));
            }
            _ if len > MAX_LINE => return Err(Refusal::TooLong("These")),
            _ => {}
        }
        match channel {
            Channel::Ads => {
                if self
                    .last_ad
                    .is_some_and(|t| now.saturating_duration_since(t) < ADS_INTERVAL)
                {
                    return Err(Refusal::AdsTooSoon);
                }
                self.last_ad = Some(now);
            }
            Channel::All => {
                let earned = (now.saturating_duration_since(self.refilled).as_secs()
                    / ALL_REFILL.as_secs()) as u32;
                if earned > 0 {
                    self.budget = (self.budget + earned).min(ALL_BUDGET);
                    self.refilled += ALL_REFILL * earned;
                }
                if self.budget == 0 {
                    return Err(Refusal::AllSpent);
                }
                self.budget -= 1;
            }
            _ => {}
        }
        Ok(())
    }
}

/// True when a word of five or more letters is all capitals.
#[must_use]
pub fn shouting(text: &str) -> bool {
    text.split(|c: char| !c.is_alphabetic())
        .any(|word| word.chars().count() >= 5 && word.chars().all(char::is_uppercase))
}

/// A named district of the world, Horse Isle's "isle".
#[must_use]
pub fn zone_of(pos: Vec3) -> &'static str {
    if pos.x.abs() < 60.0 && pos.z.abs() < 60.0 {
        return "plaza";
    }
    if pos.z.abs() >= pos.x.abs() {
        if pos.z > 0.0 {
            "north-ward"
        } else {
            "south-ward"
        }
    } else if pos.x > 0.0 {
        "west-ward"
    } else {
        "east-ward"
    }
}

/// A zone's display name.
#[must_use]
pub fn zone_name(zone: &str) -> String {
    zone.split('-')
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Whether a line on `channel` spoken at `from` reaches a listener at
/// `to` standing in `listener_zone`.
#[must_use]
pub fn reaches(
    channel: &Channel,
    speaker_zone: &str,
    from: Vec3,
    listener_zone: &str,
    to: Vec3,
) -> bool {
    let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z);
    match channel {
        Channel::All | Channel::Ads | Channel::Room(_) | Channel::Pm(_) | Channel::Agent => true,
        Channel::Zone => speaker_zone == listener_zone,
        Channel::Near => flat(from).distance(flat(to)) <= NEAR_RADIUS,
        Channel::Here => flat(from).distance(flat(to)) <= HERE_RADIUS,
    }
}

/// The method selector's order, cycled with `Tab`.
#[must_use]
pub fn methods(rooms: &[String], pm: Option<&str>) -> Vec<Channel> {
    let mut out = vec![
        Channel::All,
        Channel::Ads,
        Channel::Zone,
        Channel::Near,
        Channel::Here,
    ];
    out.extend(rooms.iter().map(|r| Channel::Room(r.clone())));
    if let Some(pm) = pm {
        out.push(Channel::Pm(pm.to_owned()));
    }
    out.push(Channel::Agent);
    out
}

/// Which channels a `!mute` word names.
#[must_use]
pub fn mute_set(word: &str) -> Vec<&'static str> {
    match word {
        "all" => vec!["all", "zone", "near", "here", "rooms", "pm"],
        "global" => vec!["all"],
        "isle" | "island" => vec!["zone"],
        "room" => vec!["rooms"],
        other => [
            "all", "ads", "zone", "near", "here", "rooms", "pm", "logins", "gestures",
        ]
        .into_iter()
        .filter(|w| *w == other)
        .collect(),
    }
}

/// The mute key a channel falls under.
#[must_use]
pub fn mute_key(channel: &Channel) -> &'static str {
    match channel {
        Channel::All => "all",
        Channel::Ads => "ads",
        Channel::Zone => "zone",
        Channel::Near => "near",
        Channel::Here => "here",
        Channel::Room(_) => "rooms",
        Channel::Pm(_) => "pm",
        Channel::Agent => "agent",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts_pick_the_channel_for_one_line() {
        assert_eq!(
            parse("/$ selling a spade", &Channel::All),
            Command::Send(Channel::Ads, "selling a spade".into())
        );
        assert_eq!(
            parse("/n anyone around?", &Channel::All),
            Command::Send(Channel::Near, "anyone around?".into())
        );
        assert_eq!(
            parse("/r lounge hello", &Channel::All),
            Command::Send(Channel::Room("lounge".into()), "hello".into())
        );
        assert_eq!(
            parse("hi all", &Channel::Zone),
            Command::Send(Channel::Zone, "hi all".into())
        );
    }

    #[test]
    fn an_unknown_shortcut_is_a_private_message() {
        assert_eq!(
            parse("/kiki want to race?", &Channel::All),
            Command::Whisper("kiki".into(), "want to race?".into())
        );
        assert_eq!(
            parse("/kiki", &Channel::All),
            Command::Target("kiki".into())
        );
    }

    #[test]
    fn bang_commands_mute_and_unmute() {
        assert_eq!(
            parse("!mute ads", &Channel::All),
            Command::Mute("ads".into())
        );
        assert_eq!(
            parse("!hear ads", &Channel::All),
            Command::Unmute("ads".into())
        );
        assert!(mute_set("all").contains(&"near"));
        assert!(
            !mute_set("all").contains(&"ads"),
            "MUTE ALL spares ads, as in Horse Isle"
        );
    }

    #[test]
    fn ads_wait_a_minute_and_all_draws_on_a_budget() {
        let t0 = Instant::now();
        let mut limits = Limits::new(t0);
        assert!(limits.admit(&Channel::Ads, "trading spades", t0).is_ok());
        assert_eq!(
            limits.admit(&Channel::Ads, "again", t0 + Duration::from_secs(5)),
            Err(Refusal::AdsTooSoon)
        );
        assert!(
            limits
                .admit(&Channel::Ads, "again", t0 + ADS_INTERVAL)
                .is_ok()
        );
        for _ in 0..ALL_BUDGET {
            assert!(limits.admit(&Channel::All, "hi", t0).is_ok());
        }
        assert_eq!(
            limits.admit(&Channel::All, "hi", t0),
            Err(Refusal::AllSpent)
        );
        assert!(limits.admit(&Channel::All, "hi", t0 + ALL_REFILL).is_ok());
        assert!(limits.admit(&Channel::Near, "near is free", t0).is_ok());
    }

    #[test]
    fn long_broadcasts_and_shouting_are_refused() {
        let mut limits = Limits::new(Instant::now());
        let long = "x".repeat(MAX_BROADCAST + 1);
        assert!(matches!(
            limits.admit(&Channel::All, &long, Instant::now()),
            Err(Refusal::TooLong(_))
        ));
        assert!(limits.admit(&Channel::Near, &long, Instant::now()).is_ok());
        assert_eq!(
            limits.admit(&Channel::Near, "HELLO there", Instant::now()),
            Err(Refusal::Shouting)
        );
        assert!(!shouting("OK fine, GG"));
    }

    #[test]
    fn scope_follows_distance_and_zone() {
        let a = Vec3::ZERO;
        let far = Vec3::new(100.0, 0.0, 0.0);
        assert!(reaches(&Channel::All, "plaza", a, "west-ward", far));
        assert!(!reaches(&Channel::Near, "plaza", a, "plaza", far));
        assert!(reaches(
            &Channel::Near,
            "plaza",
            a,
            "plaza",
            Vec3::new(10.0, 0.0, 0.0)
        ));
        assert!(!reaches(
            &Channel::Here,
            "plaza",
            a,
            "plaza",
            Vec3::new(10.0, 0.0, 0.0)
        ));
        assert!(!reaches(&Channel::Zone, "plaza", a, "west-ward", far));
        assert_eq!(zone_of(Vec3::ZERO), "plaza");
        assert_eq!(zone_of(Vec3::new(0.0, 0.0, 200.0)), "north-ward");
        assert_eq!(zone_name("north-ward"), "North Ward");
    }

    #[test]
    fn lines_land_in_their_windows() {
        let mut log = Log::default();
        log.push(Line::system("welcome"));
        log.push(Line {
            channel: Some(Channel::Here),
            from: "a".into(),
            to: None,
            text: "hi".into(),
            note: None,
        });
        assert_eq!(log.world.len(), 1);
        assert_eq!(log.personal.len(), 1);
    }
}
