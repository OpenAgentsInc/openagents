//! What the Grid homepage's visit records of Verse (#12439): joining,
//! first person, and zoom. Zone trips reach the visit through the page's
//! own `verse-zone-load` events (#12438).
//!
//! [`Journey`] turns what the frame loop sees into events, each once when
//! it should be once, as plain data so it is tested natively. The page
//! sends them with [`send`] through `globalThis.oaTrack`, which only the
//! Grid homepage defines, and only when the browser didn't ask not to be
//! tracked; elsewhere nothing is sent. Every value is a fixed word or a
//! duration: never a key, a name, or a position.

use serde_json::{Value, json};

/// One event: its name and details.
pub type Event = (&'static str, Value);

/// How the visitor joined the Grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoinBy {
    Click,
    Keys,
    Touch,
}

impl JoinBy {
    fn word(self) -> &'static str {
        match self {
            Self::Click => "click",
            Self::Keys => "keys",
            Self::Touch => "touch",
        }
    }
}

/// The visit's journey through Verse so far.
#[derive(Debug, Default)]
pub struct Journey {
    watching_since: Option<f64>,
    join_by: Option<JoinBy>,
    joined: bool,
    first_person: bool,
    zoomed: bool,
}

impl Journey {
    /// The overview is on screen at `now` (ms).
    pub fn watching(&mut self, now: f64) {
        self.watching_since.get_or_insert(now);
    }

    /// The visitor asked to join this way; the first way asked wins.
    pub fn join_by(&mut self, by: JoinBy) {
        if !self.joined && self.join_by.is_none() {
            self.join_by = Some(by);
        }
    }

    /// The visitor joined at `now` (ms): how long they watched the
    /// overview, and how they joined. Once.
    pub fn joined(&mut self, now: f64) -> Vec<Event> {
        if self.joined {
            return Vec::new();
        }
        self.joined = true;
        let watched = (now - self.watching_since.unwrap_or(now)).max(0.0).round();
        vec![
            ("overview_watched", json!({ "time": watched })),
            (
                "joined",
                json!({ "by": self.join_by.unwrap_or(JoinBy::Keys).word() }),
            ),
        ]
    }

    /// Whether the camera is in first person now; the first time it is.
    pub fn first_person(&mut self, on: bool) -> Option<Event> {
        let first = on && !self.first_person;
        self.first_person |= on;
        first.then(|| ("first_person", json!({})))
    }

    /// Zoom was used, by `by` (`wheel`, `keys`, or `pinch`); the first
    /// time.
    pub fn zoomed(&mut self, by: &'static str) -> Option<Event> {
        (!std::mem::replace(&mut self.zoomed, true)).then(|| ("zoom_used", json!({ "by": by })))
    }
}

/// Sends `events` to the page's visit, when the page keeps one.
#[cfg(target_arch = "wasm32")]
pub fn send(events: impl IntoIterator<Item = Event>) {
    use wasm_bindgen::{JsCast, JsValue};
    let Ok(track) = js_sys::Reflect::get(&js_sys::global(), &"oaTrack".into()) else {
        return;
    };
    let Some(track) = track.dyn_ref::<js_sys::Function>() else {
        return;
    };
    for (name, detail) in events {
        let _ = track.call2(
            &JsValue::UNDEFINED,
            &name.into(),
            &detail.to_string().into(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joining_records_the_watch_and_the_first_way_asked_once() {
        let mut journey = Journey::default();
        journey.watching(1_000.0);
        journey.watching(5_000.0);
        journey.join_by(JoinBy::Click);
        journey.join_by(JoinBy::Keys);
        let events = journey.joined(13_400.0);
        assert_eq!(events[0], ("overview_watched", json!({ "time": 12_400.0 })));
        assert_eq!(events[1], ("joined", json!({ "by": "click" })));
        assert!(journey.joined(20_000.0).is_empty());
    }

    #[test]
    fn first_person_and_zoom_are_recorded_the_first_time_only() {
        let mut journey = Journey::default();
        assert_eq!(journey.first_person(false), None);
        assert!(journey.first_person(true).is_some());
        assert_eq!(journey.first_person(false), None);
        assert_eq!(journey.first_person(true), None);
        assert_eq!(
            journey.zoomed("wheel"),
            Some(("zoom_used", json!({ "by": "wheel" })))
        );
        assert_eq!(journey.zoomed("keys"), None);
    }
}
