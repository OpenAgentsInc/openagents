//! The opt-in playtest session log.
//!
//! While the tester has turned **Playtest session** on, the app appends one
//! [`Event`] per structural change: the tab, the screen, an event code, and
//! the time. Every field is a closed enumeration or a number, so the log
//! can't hold message text, prompts, transcripts, keys, recovery words,
//! invoices, addresses, amounts, balances, or other players' keys: there is
//! no field to put them in. The log stays on the device and leaves it only
//! inside a report whose preview showed it in full; [`Log::digest`] is how
//! the app checks that the log it attaches is the one the tester saw.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The most events a log keeps; older ones drop first.
pub const MAX_EVENTS: usize = 200;

/// The app's tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tab {
    Coder,
    Verse,
    Wallet,
    Account,
}

/// A screen within a tab. The set is closed; a screen the list doesn't
/// name is reported as its tab's `home`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Route {
    /// The tab's first screen.
    Home,
    /// Coder: an open chat.
    Chat,
    /// Coder: a computer's terminal.
    Terminal,
    /// Verse: the Gym building.
    Gym,
    /// Verse: the Gym's RESULTS board.
    Results,
    /// Verse: a trace replay.
    Trace,
    /// Verse: the Lagrange 1 zone.
    Lagrange,
    /// Wallet: sending.
    Send,
    /// Wallet: receiving.
    Receive,
    /// Wallet: history.
    History,
    /// Wallet: recovery words.
    Recovery,
    /// Account: the trainer card, which can reveal the world key's nsec.
    Trainer,
    Computers,
    Tailnet,
    /// Account: Identity keys, which can reveal the device's nsec.
    Identity,
    /// Account: About this device.
    Device,
    Changelog,
    /// Account: the playtest card.
    Playtest,
    /// Account: My reports.
    Reports,
}

impl Route {
    /// Whether this screen can show a secret key, recovery words, or money,
    /// so the app never captures it.
    #[must_use]
    pub fn sensitive(self, tab: Tab) -> bool {
        tab == Tab::Wallet
            || matches!(
                self,
                Self::Identity | Self::Trainer | Self::Send | Self::Receive | Self::Recovery
            )
    }
}

/// What happened. Codes, never text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Code {
    /// The session started.
    Started,
    /// The tester moved to this tab and screen.
    Screen,
    /// The app came to the foreground.
    Foreground,
    /// The app went to the background.
    Background,
    /// The app showed an app-level problem notice.
    Notice,
    /// The Wallet showed an error.
    WalletError,
    /// A Coder chat showed an error.
    CoderError,
    /// The tester opened Report a problem.
    ReportOpened,
    /// A report was sent.
    ReportSent,
    /// A report couldn't be sent.
    ReportFailed,
}

/// One structural event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    /// Unix seconds.
    pub at: u64,
    pub tab: Tab,
    pub route: Route,
    pub code: Code,
}

/// The session log: on or off, and the events since the session started.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Log {
    pub on: bool,
    /// When the current session started, if one has.
    pub started_at: Option<u64>,
    pub events: VecDeque<Event>,
}

impl Log {
    /// Turn the session on or off. Turning it on starts a new log and
    /// drops the last one; turning it off stops recording and keeps the
    /// events on the device until the tester clears them or starts again.
    pub fn set(&mut self, on: bool, at: u64, tab: Tab, route: Route) {
        if on == self.on {
            return;
        }
        self.on = on;
        if on {
            self.events.clear();
            self.started_at = Some(at);
            self.record(Event {
                at,
                tab,
                route,
                code: Code::Started,
            });
        }
    }

    /// Delete every event.
    pub fn clear(&mut self) {
        self.events.clear();
        self.started_at = None;
    }

    /// Append an event while the session is on; otherwise do nothing.
    /// A screen event that repeats the last one is dropped.
    pub fn record(&mut self, event: Event) {
        if !self.on {
            return;
        }
        if event.code == Code::Screen
            && self
                .events
                .back()
                .is_some_and(|last| last.tab == event.tab && last.route == event.route)
        {
            return;
        }
        self.events.push_back(event);
        while self.events.len() > MAX_EVENTS {
            self.events.pop_front();
        }
    }

    /// Where the tester is now, from the last event.
    #[must_use]
    pub fn position(&self) -> Option<(Tab, Route)> {
        self.events.back().map(|event| (event.tab, event.route))
    }

    /// The lowercase hex SHA-256 of the events' JSON: the preview shows it,
    /// and a report attaches the log only when the digest still matches.
    #[must_use]
    pub fn digest(&self) -> String {
        digest(&self.events)
    }

    /// One line per event, for the preview.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        self.events.iter().map(line).collect()
    }
}

/// The digest of a list of events.
#[must_use]
pub fn digest<'a>(events: impl IntoIterator<Item = &'a Event>) -> String {
    let events: Vec<&Event> = events.into_iter().collect();
    let bytes = serde_json::to_vec(&events).unwrap_or_default();
    Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// `1790000012 verse/gym screen`: the Unix time, the place, and the code.
#[must_use]
pub fn line(event: &Event) -> String {
    format!(
        "{} {}/{} {}",
        event.at,
        name(&event.tab),
        name(&event.route),
        name(&event.code)
    )
}

/// The serialized name of a unit enum value, such as `felt-good`.
#[must_use]
pub fn name<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(at: u64, tab: Tab, route: Route, code: Code) -> Event {
        Event {
            at,
            tab,
            route,
            code,
        }
    }

    #[test]
    fn nothing_is_recorded_while_the_session_is_off() {
        let mut log = Log::default();
        log.record(event(1, Tab::Verse, Route::Gym, Code::Screen));
        assert!(log.events.is_empty());
        log.set(true, 2, Tab::Verse, Route::Home);
        log.record(event(3, Tab::Verse, Route::Gym, Code::Screen));
        log.set(false, 4, Tab::Verse, Route::Gym);
        log.record(event(5, Tab::Coder, Route::Chat, Code::Screen));
        assert_eq!(log.events.len(), 2);
        assert_eq!(log.events[0].code, Code::Started);
        // Starting again begins a new log.
        log.set(true, 6, Tab::Account, Route::Home);
        assert_eq!(log.events.len(), 1);
        assert_eq!(log.started_at, Some(6));
    }

    #[test]
    fn the_session_log_holds_only_closed_structural_values() {
        // Every serialized event is exactly {at, tab, route, code} with a
        // number and three names from closed sets: no field can carry text.
        let mut log = Log::default();
        log.set(true, 10, Tab::Coder, Route::Chat);
        log.record(event(11, Tab::Wallet, Route::Send, Code::WalletError));
        let json = serde_json::to_value(&log.events).unwrap();
        for item in json.as_array().unwrap() {
            let object = item.as_object().unwrap();
            let keys: Vec<&str> = object.keys().map(String::as_str).collect();
            assert_eq!(keys, ["at", "code", "route", "tab"]);
            assert!(object["at"].is_u64());
        }
        // A host can't smuggle text in: unknown fields and unknown names
        // are refused.
        for bad in [
            r#"{"at":1,"tab":"coder","route":"chat","code":"screen","text":"hello"}"#,
            r#"{"at":1,"tab":"coder","route":"my secret words","code":"screen"}"#,
            r#"{"at":1,"tab":"nsec1abc","route":"home","code":"screen"}"#,
            r#"{"at":"1","tab":"coder","route":"home","code":"screen"}"#,
        ] {
            assert!(serde_json::from_str::<Event>(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_log_is_bounded_and_drops_repeated_screens() {
        let mut log = Log::default();
        log.set(true, 0, Tab::Verse, Route::Home);
        log.record(event(1, Tab::Verse, Route::Home, Code::Screen));
        assert_eq!(log.events.len(), 1);
        for at in 0..(MAX_EVENTS as u64 * 2) {
            log.record(event(at, Tab::Verse, Route::Home, Code::Notice));
        }
        assert_eq!(log.events.len(), MAX_EVENTS);
    }

    #[test]
    fn the_digest_changes_with_the_events_and_lines_name_them() {
        let mut log = Log::default();
        log.set(true, 100, Tab::Verse, Route::Gym);
        let first = log.digest();
        assert_eq!(first.len(), 64);
        log.record(event(101, Tab::Verse, Route::Results, Code::Screen));
        assert_ne!(log.digest(), first);
        assert_eq!(log.lines()[1], "101 verse/results screen");
    }

    #[test]
    fn wallet_and_key_screens_are_sensitive() {
        assert!(Route::Home.sensitive(Tab::Wallet));
        assert!(Route::Identity.sensitive(Tab::Account));
        assert!(Route::Trainer.sensitive(Tab::Account));
        assert!(!Route::Gym.sensitive(Tab::Verse));
        assert!(!Route::Changelog.sensitive(Tab::Account));
    }
}
