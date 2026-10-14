//! Seeded bot runs: the measured side of a version. The rules crate's bot
//! (`icecream_rules::bot`) plays the same houses with each version's
//! numbers, so two versions compare on equal terms, and an edit nobody can
//! win is caught before it's saved.

use icecream_rules::bot::Bot;
use icecream_rules::layout::House;
use icecream_rules::{Event, Game, HZ, Options, Status, Tuning};
use serde_json::{Value, json};

/// The houses every version is measured on.
pub const RUNS: u64 = 24;
/// The longest a bot run lasts, in seconds.
pub const LIMIT: u32 = 600;

/// What the bot runs measured.
#[derive(Clone, Debug, PartialEq)]
pub struct Stats {
    pub runs: u32,
    pub escaped: u32,
    /// The middle escape time, seconds.
    pub median_escape: Option<f32>,
    /// Hits the player took, on average.
    pub hits: f32,
    /// How often he spotted the player, on average.
    pub spotted: f32,
    /// Hiding spots in the house the card plays.
    pub hiding_spots: usize,
}

impl Stats {
    /// Escapes, in percent.
    #[must_use]
    pub fn win_rate(&self) -> u32 {
        (self.escaped * 100 + self.runs / 2) / self.runs.max(1)
    }

    #[must_use]
    pub fn json(&self) -> Value {
        json!({
            "runs": self.runs,
            "escaped": self.escaped,
            "win_rate": self.win_rate(),
            "median_escape": self.median_escape.map(tenths),
            "hits": tenths(self.hits),
            "spotted": tenths(self.spotted),
            "hiding_spots": self.hiding_spots,
        })
    }

    /// One line in plain words.
    #[must_use]
    pub fn line(&self) -> String {
        let escape = self
            .median_escape
            .map_or(String::new(), |s| format!(", in {} on average", clock(s)));
        format!(
            "The bot escaped {} of {} runs ({}%){escape}, took {:.1} hits and was spotted {:.1} times a run; the house has {} hiding spots.",
            self.escaped,
            self.runs,
            self.win_rate(),
            self.hits,
            self.spotted,
            self.hiding_spots
        )
    }
}

/// To a tenth, as JSON shows it (no `1.3999999`).
fn tenths(value: f32) -> f64 {
    (f64::from(value) * 10.0).round() / 10.0
}

/// `m:ss`.
#[must_use]
pub fn clock(seconds: f32) -> String {
    let s = seconds.max(0.0).round() as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

struct Run {
    escaped: bool,
    seconds: f32,
    hits: u32,
    spotted: u32,
}

fn run(seed: u64, tuning: Tuning) -> Run {
    let mut game = Game::tuned(seed, Options::default(), tuning);
    let mut bot = Bot::default();
    let mut spotted = 0;
    for _ in 0..LIMIT * HZ {
        let input = bot.input(&game);
        game.step(input);
        spotted += game.events.iter().filter(|e| **e == Event::Spotted).count() as u32;
        if game.status != Status::Playing {
            break;
        }
    }
    Run {
        escaped: game.status == Status::Escaped,
        seconds: game.seconds(),
        hits: game.stats.hits_taken,
        spotted,
    }
}

/// Plays [`RUNS`] seeded houses with the bot, in parallel; `seed` is the
/// house the card plays, for its hiding spots.
#[must_use]
pub fn measure(tuning: Tuning, seed: u64) -> Stats {
    let runs: Vec<Run> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..RUNS)
            .map(|s| scope.spawn(move || run(s, tuning)))
            .collect();
        handles.into_iter().filter_map(|h| h.join().ok()).collect()
    });
    let n = runs.len().max(1) as f32;
    let mut escapes: Vec<f32> = runs
        .iter()
        .filter(|r| r.escaped)
        .map(|r| r.seconds)
        .collect();
    escapes.sort_by(f32::total_cmp);
    Stats {
        runs: runs.len() as u32,
        escaped: escapes.len() as u32,
        median_escape: (!escapes.is_empty()).then(|| escapes[escapes.len() / 2]),
        hits: runs.iter().map(|r| r.hits as f32).sum::<f32>() / n,
        spotted: runs.iter().map(|r| r.spotted as f32).sum::<f32>() / n,
        hiding_spots: House::tuned(seed, tuning.extra_closets as usize)
            .closets
            .len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slower_ice_cream_man_is_easier_to_escape() {
        let base = measure(Tuning::default(), 7);
        let mut slow = Tuning::default();
        for name in ["man_wander", "man_investigate", "man_search", "man_chase"] {
            let value = slow.get(name).unwrap() * 0.8;
            slow.set(name, value).unwrap();
        }
        slow.extra_closets = 2;
        let slower = measure(slow, 7);
        assert!(base.escaped > 0, "{base:?}");
        assert!(slower.escaped >= base.escaped, "{base:?} {slower:?}");
        assert_eq!(slower.hiding_spots, base.hiding_spots + 2);
        assert_eq!(
            measure(Tuning::default(), 7),
            base,
            "the same runs every time"
        );
        assert!(base.line().contains("of 24 runs"));
    }

    #[test]
    fn a_door_that_takes_ten_minutes_to_open_is_never_escaped() {
        let mut t = Tuning::default();
        t.set("unlock", 600.0).unwrap();
        assert_eq!(measure(t, 7).escaped, 0);
    }
}
