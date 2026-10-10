//! The player's record, `bunny.progress.v1`: wins, best times and scores
//! per garden, and settings. The bunny's shade is derived from the win
//! count, never stored (`docs/verse/games/grow-little-bunny.md`,
//! Persistence).
//!
//! It is a small JSON object, written and read here without a JSON library
//! so the rules stay dependency-light:
//!
//! ```text
//! {"format":"bunny.progress.v1","wins":3,"gentle":false,"contrast":false,
//!  "best":[{"garden":1,"ticks":5400,"score":9120}]}
//! ```

use crate::Status;

pub const FORMAT: &str = "bunny.progress.v1";

/// A garden's best run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Best {
    pub garden: u8,
    /// Fastest clear, in ticks.
    pub ticks: u32,
    pub score: u32,
}

/// The player's record and settings.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    pub wins: u32,
    pub gentle: bool,
    pub high_contrast: bool,
    pub best: Vec<Best>,
}

impl Progress {
    /// The bunny's shade, 0 (snow) to 20 (neon).
    #[must_use]
    pub fn shade(&self) -> u32 {
        crate::shade::shade_for_wins(self.wins)
    }

    /// Wins left to the next shade, or none at the last.
    #[must_use]
    pub fn to_next_shade(&self) -> Option<u32> {
        (self.shade() < crate::shade::MAX_SHADE).then_some(1)
    }

    /// The best run of garden `n`, if any.
    #[must_use]
    pub fn best(&self, garden: u8) -> Option<Best> {
        self.best.iter().copied().find(|b| b.garden == garden)
    }

    /// Records a finished run. A win counts one step on the ladder from any
    /// garden; a catch or leaving counts nothing. Returns whether the
    /// shade changed.
    pub fn record(&mut self, garden: u8, outcome: Status, ticks: u32, score: u32) -> bool {
        if outcome != Status::Won {
            return false;
        }
        let before = self.shade();
        self.wins = self.wins.saturating_add(1);
        match self.best.iter_mut().find(|b| b.garden == garden) {
            Some(best) => {
                best.ticks = best.ticks.min(ticks);
                best.score = best.score.max(score);
            }
            None => {
                self.best.push(Best {
                    garden,
                    ticks,
                    score,
                });
                self.best.sort_by_key(|b| b.garden);
            }
        }
        self.shade() != before
    }

    /// Two saves of the same player (a phone and a desktop): win counts only
    /// go up, so the higher count wins, and each garden keeps its best.
    #[must_use]
    pub fn merge(&self, other: &Self) -> Self {
        let mut merged = if other.wins > self.wins {
            other.clone()
        } else {
            self.clone()
        };
        for best in self.best.iter().chain(&other.best) {
            match merged.best.iter_mut().find(|b| b.garden == best.garden) {
                Some(mine) => {
                    mine.ticks = mine.ticks.min(best.ticks);
                    mine.score = mine.score.max(best.score);
                }
                None => merged.best.push(*best),
            }
        }
        merged.best.sort_by_key(|b| b.garden);
        merged
    }

    #[must_use]
    pub fn encode(&self) -> String {
        let best: Vec<String> = self
            .best
            .iter()
            .map(|b| {
                format!(
                    "{{\"garden\":{},\"ticks\":{},\"score\":{}}}",
                    b.garden, b.ticks, b.score
                )
            })
            .collect();
        format!(
            "{{\"format\":\"{FORMAT}\",\"wins\":{},\"gentle\":{},\"contrast\":{},\"best\":[{}]}}",
            self.wins,
            self.gentle,
            self.high_contrast,
            best.join(",")
        )
    }

    /// Reads a save, tolerantly: a missing or broken field reads as its
    /// default, and the first saves (`{"wins":N}`) read too.
    #[must_use]
    pub fn decode(text: &str) -> Self {
        let number = |text: &str, key: &str| -> Option<u64> {
            let at = text.find(&format!("\"{key}\""))?;
            let rest = &text[at + key.len() + 2..];
            let rest = rest.trim_start().strip_prefix(':')?.trim_start();
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            digits.parse().ok()
        };
        let flag = |key: &str| {
            text.find(&format!("\"{key}\""))
                .map(|at| text[at + key.len() + 2..].trim_start())
                .and_then(|rest| rest.strip_prefix(':'))
                .is_some_and(|rest| rest.trim_start().starts_with("true"))
        };
        let mut best = Vec::new();
        if let Some(at) = text.find("\"best\"") {
            for item in text[at..].split('{').skip(1) {
                let (Some(garden), Some(ticks), Some(score)) = (
                    number(item, "garden"),
                    number(item, "ticks"),
                    number(item, "score"),
                ) else {
                    continue;
                };
                if let (Ok(garden), Ok(ticks), Ok(score)) = (
                    u8::try_from(garden),
                    u32::try_from(ticks),
                    u32::try_from(score),
                ) && best.iter().all(|b: &Best| b.garden != garden)
                {
                    best.push(Best {
                        garden,
                        ticks,
                        score,
                    });
                }
            }
        }
        best.sort_by_key(|b| b.garden);
        let wins_text = text.split("\"best\"").next().unwrap_or(text);
        Self {
            wins: number(wins_text, "wins")
                .and_then(|w| u32::try_from(w).ok())
                .unwrap_or(0),
            gentle: flag("gentle"),
            high_contrast: flag("contrast"),
            best,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_win_steps_the_ladder_and_a_catch_or_leaving_counts_nothing() {
        let mut p = Progress::default();
        assert!(!p.record(1, Status::Caught, 100, 50));
        assert!(!p.record(1, Status::Left, 100, 50));
        assert_eq!(p.wins, 0);
        assert!(p.record(1, Status::Won, 5_000, 9_000));
        assert_eq!((p.wins, p.shade()), (1, 1));
        assert!(p.record(1, Status::Won, 4_000, 8_000));
        assert_eq!(
            p.best(1),
            Some(Best {
                garden: 1,
                ticks: 4_000,
                score: 9_000
            })
        );
        p.wins = 20;
        assert!(!p.record(2, Status::Won, 1, 1), "shade 20 stays 20");
        assert_eq!(p.shade(), 20);
        assert_eq!(p.wins, 21);
        assert_eq!(p.to_next_shade(), None);
    }

    #[test]
    fn the_save_round_trips_and_reads_the_first_format() {
        let mut p = Progress {
            wins: 7,
            gentle: true,
            high_contrast: false,
            best: Vec::new(),
        };
        p.record(3, Status::Won, 6_000, 12_000);
        p.record(1, Status::Won, 5_000, 9_000);
        let text = p.encode();
        assert!(text.contains(FORMAT));
        assert_eq!(Progress::decode(&text), p);
        assert_eq!(Progress::decode("{\"wins\":4}").wins, 4);
        assert_eq!(Progress::decode("not json"), Progress::default());
        assert_eq!(Progress::decode(""), Progress::default());
    }

    #[test]
    fn a_conflict_keeps_the_higher_win_count_and_each_best() {
        let mut phone = Progress::default();
        phone.record(1, Status::Won, 5_000, 1_000);
        let mut desktop = Progress::default();
        for _ in 0..3 {
            desktop.record(2, Status::Won, 7_000, 2_000);
        }
        desktop.record(1, Status::Won, 6_000, 3_000);
        let merged = phone.merge(&desktop);
        assert_eq!(merged.wins, 4);
        assert_eq!(merged, desktop.merge(&phone));
        assert_eq!(
            merged.best(1),
            Some(Best {
                garden: 1,
                ticks: 5_000,
                score: 3_000
            })
        );
        assert_eq!(merged.best(2).map(|b| b.ticks), Some(7_000));
    }

    #[test]
    fn the_shade_comes_from_the_win_count_and_matches_the_table() {
        let spec = [
            0xFFFFFF, 0xFEFBF8, 0xFFF6EE, 0xFFF1E4, 0xFFECDB, 0xFFE6D0, 0xFFE0C5, 0xFFD9BB,
            0xFED3B1, 0xFFCCA5, 0xFFC59A, 0xFEBE90, 0xFFB684, 0xFFAE79, 0xFFA56C, 0xFE9D61,
            0xFE9455, 0xFF8A45, 0xFE8137, 0xFF7623, 0xFE6B04,
        ];
        for (wins, colour) in spec.iter().enumerate() {
            let p = Progress {
                wins: wins as u32,
                ..Progress::default()
            };
            assert_eq!(crate::shade::SHADES[p.shade() as usize], *colour);
        }
    }
}
