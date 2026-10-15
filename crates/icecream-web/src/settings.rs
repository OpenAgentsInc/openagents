//! The player's settings and best time, kept in the browser's storage,
//! and the small rules around them: the speedrun clock's format, when a
//! time is a new best, and which hot-bar tool comes next.

use icecream_rules::Tool;
use icecream_rules::tuning::Difficulty;

/// The browser storage key for [`Settings`].
pub const SETTINGS_KEY: &str = "icecream.settings.v1";

/// The browser storage key for the best escape time, in seconds, on
/// Easy: the game before the difficulty pass (#11667) played as Easy
/// does, so the times kept before it stay Easy's. Normal and Hard keep
/// theirs under this key and a suffix ([`best_key`]).
pub const BEST_KEY: &str = "icecream.best.v1";

/// The storage key for a difficulty's best time.
#[must_use]
pub fn best_key(difficulty: Difficulty) -> String {
    match difficulty {
        Difficulty::Easy => BEST_KEY.to_owned(),
        other => format!("{BEST_KEY}.{}", other.id()),
    }
}

/// What the player chose in the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// The run timer at the top of the screen.
    pub timer: bool,
    /// The scary ending when he scoops you; off, a gentle one.
    pub jumpscares: bool,
    /// The touch Sprint button: tap to turn sprint on and off, instead of
    /// holding it.
    pub sprint_tap: bool,
    /// How hard the next house is.
    pub difficulty: Difficulty,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            timer: true,
            jumpscares: true,
            sprint_tap: false,
            difficulty: Difficulty::Normal,
        }
    }
}

impl Settings {
    /// Settings from their stored text,
    /// `timer=1;jumpscares=0;sprint=tap;difficulty=hard`.
    /// Anything missing or unreadable keeps its default.
    #[must_use]
    pub fn parse(stored: Option<&str>) -> Self {
        let mut settings = Self::default();
        for pair in stored.unwrap_or_default().split(';') {
            let Some((key, value)) = pair.split_once('=') else {
                continue;
            };
            match (key.trim(), value.trim()) {
                ("timer", "1") => settings.timer = true,
                ("timer", "0") => settings.timer = false,
                ("jumpscares", "1") => settings.jumpscares = true,
                ("jumpscares", "0") => settings.jumpscares = false,
                ("sprint", "tap") => settings.sprint_tap = true,
                ("sprint", "hold") => settings.sprint_tap = false,
                ("difficulty", id) => {
                    if let Some(d) = Difficulty::from_id(id) {
                        settings.difficulty = d;
                    }
                }
                _ => {}
            }
        }
        settings
    }

    /// The text [`Settings::parse`] reads back.
    #[must_use]
    pub fn stored(self) -> String {
        format!(
            "timer={};jumpscares={};sprint={};difficulty={}",
            u8::from(self.timer),
            u8::from(self.jumpscares),
            if self.sprint_tap { "tap" } else { "hold" },
            self.difficulty.id()
        )
    }
}

/// A run's time for the speedrun clock: `0:26.4`, or `12:03.0`.
#[must_use]
pub fn clock(seconds: f32) -> String {
    let tenths = (seconds.max(0.0) * 10.0).floor() as u32;
    let minutes = tenths / 600;
    let rest = tenths % 600;
    format!("{minutes}:{:02}.{}", rest / 10, rest % 10)
}

/// The stored best time, when it reads as a positive number of seconds.
#[must_use]
pub fn best(stored: Option<&str>) -> Option<f32> {
    stored
        .and_then(|text| text.trim().parse::<f32>().ok())
        .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
}

/// Whether `seconds` beats `best` (any time beats none).
#[must_use]
pub fn is_new_best(best: Option<f32>, seconds: f32) -> bool {
    seconds > 0.0 && best.is_none_or(|best| seconds < best)
}

/// The tool after `current` in the hot bar (`owned`, in order), going
/// round; `back` goes the other way. `None` with one tool or none.
#[must_use]
pub fn cycle(owned: &[Tool], current: Tool, back: bool) -> Option<Tool> {
    if owned.len() < 2 {
        return None;
    }
    let n = owned.len();
    let at = owned.iter().position(|t| *t == current).unwrap_or(0);
    let next = if back { (at + n - 1) % n } else { (at + 1) % n };
    Some(owned[next])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_default_on() {
        let default = Settings::parse(None);
        assert!(default.timer && default.jumpscares && !default.sprint_tap);
        assert_eq!(default.difficulty, Difficulty::Normal);
        let changed = Settings {
            timer: false,
            jumpscares: false,
            sprint_tap: true,
            difficulty: Difficulty::Hard,
        };
        assert_eq!(Settings::parse(Some(&changed.stored())), changed);
        assert_eq!(Settings::parse(Some("junk;timer=maybe")), default);
        assert_eq!(Settings::parse(Some("difficulty=impossible")), default);
        // Stored before difficulties: Normal.
        assert_eq!(
            Settings::parse(Some("timer=1;jumpscares=1;sprint=hold")),
            default
        );
        assert_eq!(
            Settings::parse(Some("difficulty=easy")).difficulty,
            Difficulty::Easy
        );
        assert_eq!(
            Settings::parse(Some("jumpscares=0")),
            Settings {
                jumpscares: false,
                ..default
            }
        );
    }

    #[test]
    fn the_clock_shows_tenths() {
        assert_eq!(clock(26.47), "0:26.4");
        assert_eq!(clock(0.0), "0:00.0");
        assert_eq!(clock(61.0), "1:01.0");
        assert_eq!(clock(723.05), "12:03.0");
        assert_eq!(clock(-3.0), "0:00.0");
    }

    #[test]
    fn a_faster_time_is_a_new_best() {
        assert_eq!(best(Some("41.2")), Some(41.2));
        assert_eq!(best(Some("nope")), None);
        assert_eq!(best(Some("0")), None);
        assert!(is_new_best(None, 51.0));
        assert!(is_new_best(Some(41.0), 26.0));
        assert!(!is_new_best(Some(26.0), 46.0));
        assert!(!is_new_best(Some(26.0), 26.0));
    }

    #[test]
    fn each_difficulty_keeps_its_own_best() {
        // Times kept before difficulties were Easy's game (version 4).
        assert_eq!(best_key(Difficulty::Easy), "icecream.best.v1");
        assert_eq!(best_key(Difficulty::Normal), "icecream.best.v1.normal");
        assert_eq!(best_key(Difficulty::Hard), "icecream.best.v1.hard");
    }

    #[test]
    fn the_next_item_goes_round_the_hot_bar() {
        let owned = [Tool::Fudge, Tool::Fart, Tool::Keys];
        assert_eq!(cycle(&owned, Tool::Fudge, false), Some(Tool::Fart));
        assert_eq!(cycle(&owned, Tool::Keys, false), Some(Tool::Fudge));
        assert_eq!(cycle(&owned, Tool::Fudge, true), Some(Tool::Keys));
        assert_eq!(cycle(&[Tool::Fudge], Tool::Fudge, false), None);
    }
}
