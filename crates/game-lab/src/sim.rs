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
    /// Runs won by melting him with the earned weapons instead.
    pub melted: u32,
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
    /// Runs won: escaped, or he melted.
    #[must_use]
    pub fn wins(&self) -> u32 {
        self.escaped + self.melted
    }

    /// Wins, in percent.
    #[must_use]
    pub fn win_rate(&self) -> u32 {
        (self.wins() * 100 + self.runs / 2) / self.runs.max(1)
    }

    #[must_use]
    pub fn json(&self) -> Value {
        json!({
            "runs": self.runs,
            "escaped": self.escaped,
            "melted": self.melted,
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
        let melted = if self.melted > 0 {
            format!(" and melted him in {} more", self.melted)
        } else {
            String::new()
        };
        format!(
            "The bot escaped {} of {} runs{melted} ({}% won){escape}, took {:.1} hits and was spotted {:.1} times a run; the house has {} hiding spots.",
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
    melted: bool,
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
        melted: game.status == Status::Melted,
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
    let melted = runs.iter().filter(|r| r.melted).count() as u32;
    let mut escapes: Vec<f32> = runs
        .iter()
        .filter(|r| r.escaped)
        .map(|r| r.seconds)
        .collect();
    escapes.sort_by(f32::total_cmp);
    Stats {
        runs: runs.len() as u32,
        escaped: escapes.len() as u32,
        melted,
        median_escape: (!escapes.is_empty()).then(|| escapes[escapes.len() / 2]),
        hits: runs.iter().map(|r| r.hits as f32).sum::<f32>() / n,
        spotted: runs.iter().map(|r| r.spotted as f32).sum::<f32>() / n,
        hiding_spots: House::of(seed, &tuning).closets.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use icecream_rules::tuning::Difficulty;

    #[test]
    fn a_slower_ice_cream_man_is_easier_to_escape() {
        let base = measure(Tuning::original(), 7);
        let mut slow = Tuning::original();
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
            measure(Tuning::original(), 7),
            base,
            "the same runs every time"
        );
        assert!(base.line().contains("of 24 runs"));
    }

    /// The designer's revision (#11626): the earned weapons, minions and
    /// free roam, in the Backrooms.
    pub(crate) fn revision() -> Tuning {
        let mut t = Tuning::original();
        for (name, value) in [
            ("earned_weapons", 1.0),
            ("minion_count", 3.0),
            ("free_roam", 1.0),
            ("backrooms", 1.0),
            ("loops", 10.0),
            ("yellow", 1.0),
            ("fog_near", 4.0),
            ("fog_far", 24.0),
            ("flicker", 0.12),
            ("buzz", 0.6),
        ] {
            t.set(name, value).unwrap();
        }
        t
    }

    #[test]
    fn the_original_numbers_measure_as_before() {
        // Version 1 as Game Lab measured it before the revision's code.
        let base = measure(Tuning::original(), 7);
        eprintln!("original: {}", base.line());
        assert_eq!((base.escaped, base.melted), (24, 0), "{base:?}");
        assert!(
            base.median_escape.is_some_and(|t| (t - 25.6).abs() < 0.05),
            "{base:?}"
        );
        assert!(
            (base.hits - 1.7).abs() < 0.05 && (base.spotted - 2.0).abs() < 0.05,
            "{base:?}"
        );
    }

    #[test]
    fn easy_plays_version_4() {
        // Version 4 of the-ice-cream-man, the designer's revision with the
        // Backrooms, as it measured before the difficulty pass (#11667).
        let easy = measure(Difficulty::Easy.tuning(), 7);
        eprintln!("easy (v4): {}", easy.line());
        assert_eq!(
            (easy.escaped + easy.melted, easy.runs),
            (24, 24),
            "{easy:?}"
        );
        assert!(
            easy.median_escape.is_some_and(|t| (t - 42.2).abs() < 0.05),
            "{easy:?}"
        );
        assert!((easy.hits - 1.9).abs() < 0.05, "{easy:?}");
        assert_eq!(easy.hiding_spots, 7, "{easy:?}");
    }

    #[test]
    fn the_game_plays_version_5_normal_by_default() {
        // Version 5 (#11667): a bigger house, keys at its far ends and one
        // behind the button puzzle, him faster and smarter, a long unlock.
        let today = measure(Tuning::default(), 7);
        eprintln!("normal (v5): {}", today.line());
        assert!(today.wins() >= 18, "{today:?}");
        assert!(today.wins() < 24 || today.melted > 0, "{today:?}");
        // A typical run takes two minutes or more, not 42 seconds.
        assert!(today.median_escape.is_some_and(|t| t >= 110.0), "{today:?}");
        let hard = measure(Difficulty::Hard.tuning(), 7);
        eprintln!("hard: {}", hard.line());
        assert!(hard.wins() >= 12, "still winnable: {hard:?}");
        assert!(
            hard.median_escape > today.median_escape,
            "{hard:?} {today:?}"
        );
    }

    #[test]
    fn the_designers_revision_is_winnable() {
        let mut items = Tuning::original();
        for (name, value) in [
            ("earned_weapons", 1.0),
            ("minion_count", 3.0),
            ("free_roam", 1.0),
        ] {
            items.set(name, value).unwrap();
        }
        let with_items = measure(items, 7);
        eprintln!("items and minions: {}", with_items.line());
        let revised = measure(revision(), 7);
        eprintln!("the Backrooms too: {}", revised.line());
        assert!(with_items.wins() >= 18, "{with_items:?}");
        assert!(revised.wins() >= 18, "{revised:?}");
    }

    #[test]
    fn a_door_that_takes_ten_minutes_to_open_is_never_escaped() {
        let mut t = Tuning::default();
        t.set("unlock", 600.0).unwrap();
        assert_eq!(measure(t, 7).escaped, 0);
    }
}
