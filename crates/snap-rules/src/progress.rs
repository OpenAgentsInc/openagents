//! The player's record, `snap.progress.v1`: the most stars and the fewest
//! pieces for each job, and settings. It is a small JSON object, written
//! and read here without a JSON library so the rules stay light:
//!
//! ```text
//! {"format":"snap.progress.v1","sound":true,"contrast":false,
//!  "jobs":[{"job":1,"stars":3,"pieces":3}]}
//! ```

pub const FORMAT: &str = "snap.progress.v1";

/// A job's best.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Best {
    pub job: u8,
    /// The most stars won, 1 to 3.
    pub stars: u8,
    /// The fewest pieces a win used.
    pub pieces: u32,
}

/// The player's record and settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Progress {
    pub sound: bool,
    pub high_contrast: bool,
    pub jobs: Vec<Best>,
}

impl Default for Progress {
    fn default() -> Self {
        Self {
            sound: true,
            high_contrast: false,
            jobs: Vec::new(),
        }
    }
}

impl Progress {
    /// Job `job`'s best, if it has been done.
    #[must_use]
    pub fn best(&self, job: u8) -> Option<Best> {
        self.jobs.iter().copied().find(|b| b.job == job)
    }

    /// Stars across every job.
    #[must_use]
    pub fn stars(&self) -> u32 {
        self.jobs.iter().map(|b| u32::from(b.stars)).sum()
    }

    /// Whether job `job` is open: the first is, and each after a job that
    /// has been done. Every job stays open once it has been.
    #[must_use]
    pub fn open(&self, job: u8) -> bool {
        job <= 1 || self.best(job - 1).is_some() || self.best(job).is_some()
    }

    /// Records a win. Returns whether it beat the job's best.
    pub fn record(&mut self, job: u8, stars: u8, pieces: u32) -> bool {
        match self.jobs.iter_mut().find(|b| b.job == job) {
            Some(best) => {
                let better = stars > best.stars || pieces < best.pieces;
                best.stars = best.stars.max(stars);
                best.pieces = best.pieces.min(pieces);
                better
            }
            None => {
                self.jobs.push(Best { job, stars, pieces });
                self.jobs.sort_by_key(|b| b.job);
                true
            }
        }
    }

    /// Two saves of the same player: each job keeps its best.
    #[must_use]
    pub fn merge(&self, other: &Self) -> Self {
        let mut merged = self.clone();
        for best in &other.jobs {
            merged.record(best.job, best.stars, best.pieces);
        }
        merged
    }

    #[must_use]
    pub fn encode(&self) -> String {
        let jobs: Vec<String> = self
            .jobs
            .iter()
            .map(|b| {
                format!(
                    "{{\"job\":{},\"stars\":{},\"pieces\":{}}}",
                    b.job, b.stars, b.pieces
                )
            })
            .collect();
        format!(
            "{{\"format\":\"{FORMAT}\",\"sound\":{},\"contrast\":{},\"jobs\":[{}]}}",
            self.sound,
            self.high_contrast,
            jobs.join(",")
        )
    }

    /// Reads a save, tolerantly: a missing or broken field reads as its
    /// default.
    #[must_use]
    pub fn decode(text: &str) -> Self {
        let number = |text: &str, key: &str| -> Option<u64> {
            let at = text.find(&format!("\"{key}\""))?;
            let rest = text[at + key.len() + 2..].trim_start().strip_prefix(':')?;
            let digits: String = rest
                .trim_start()
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            digits.parse().ok()
        };
        let flag = |key: &str, default: bool| {
            text.find(&format!("\"{key}\""))
                .map(|at| text[at + key.len() + 2..].trim_start())
                .and_then(|rest| rest.strip_prefix(':'))
                .map_or(default, |rest| rest.trim_start().starts_with("true"))
        };
        let mut progress = Self {
            sound: flag("sound", true),
            high_contrast: flag("contrast", false),
            jobs: Vec::new(),
        };
        if let Some(list) = text.find("\"jobs\"").map(|at| &text[at..]) {
            for item in list.split('{').skip(1) {
                let (Some(job), Some(stars)) = (number(item, "job"), number(item, "stars")) else {
                    continue;
                };
                let pieces = number(item, "pieces").unwrap_or(u64::from(u32::MAX));
                if (1..=99).contains(&job) && (1..=3).contains(&stars) {
                    progress.record(
                        job as u8,
                        stars as u8,
                        u32::try_from(pieces).unwrap_or(u32::MAX),
                    );
                }
            }
        }
        progress
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_save_round_trips_and_keeps_each_jobs_best() {
        let mut progress = Progress::default();
        assert!(progress.open(1) && !progress.open(2));
        assert!(progress.record(1, 2, 7));
        assert!(progress.open(2) && !progress.open(3));
        assert!(!progress.record(1, 1, 9), "worse");
        assert!(progress.record(1, 3, 5));
        progress.high_contrast = true;
        let text = progress.encode();
        assert_eq!(Progress::decode(&text), progress);
        assert_eq!(progress.stars(), 3);
        assert_eq!(progress.best(1).unwrap().pieces, 5);
    }

    #[test]
    fn broken_saves_read_as_defaults_and_merges_keep_the_best() {
        assert_eq!(Progress::decode("nonsense"), Progress::default());
        let odd = Progress::decode("{\"jobs\":[{\"job\":0,\"stars\":9},{\"job\":2,\"stars\":1}]}");
        assert_eq!(odd.jobs.len(), 1);
        let mut phone = Progress::default();
        phone.record(1, 3, 4);
        let mut desktop = Progress::default();
        desktop.record(1, 2, 3);
        desktop.record(2, 1, 9);
        let merged = phone.merge(&desktop);
        assert_eq!(
            merged.best(1),
            Some(Best {
                job: 1,
                stars: 3,
                pieces: 3
            })
        );
        assert_eq!(merged.best(2).unwrap().stars, 1);
    }
}
