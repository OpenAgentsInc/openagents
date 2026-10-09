//! A bounded store of attempt records: the newest `max` records no older
//! than `retain_ms`, in memory, and optionally one JSON line per attempt
//! in day files (`DIR/YYYY-MM-DD.jsonl`, UTC) pruned after `keep_days`.

use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::PathBuf;

use super::Attempt;

/// One day in milliseconds.
pub const DAY_MS: u64 = 86_400_000;

/// The in-memory window: the newest records within the retention.
#[derive(Debug)]
pub struct Store {
    records: VecDeque<Attempt>,
    max: usize,
    retain_ms: u64,
    /// Records dropped because the store was full or they aged out.
    pub dropped: u64,
}

impl Store {
    /// At most `max` records, none older than `retain_ms`.
    pub fn new(max: usize, retain_ms: u64) -> Self {
        Self {
            records: VecDeque::new(),
            max: max.max(1),
            retain_ms,
            dropped: 0,
        }
    }

    /// Keep `attempt`, dropping the oldest when full.
    pub fn push(&mut self, attempt: Attempt) {
        while self.records.len() >= self.max {
            self.records.pop_front();
            self.dropped += 1;
        }
        self.records.push_back(attempt);
    }

    /// Drop records that ended before `now - retain`.
    pub fn prune(&mut self, now_ms: u64) {
        let floor = now_ms.saturating_sub(self.retain_ms);
        while self
            .records
            .front()
            .is_some_and(|attempt| attempt.end_ms() < floor)
        {
            self.records.pop_front();
            self.dropped += 1;
        }
    }

    /// Records that ended at or after `since_ms`.
    pub fn since(&self, since_ms: u64) -> impl Iterator<Item = &Attempt> {
        self.records
            .iter()
            .filter(move |attempt| attempt.end_ms() >= since_ms)
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn max(&self) -> usize {
        self.max
    }

    pub fn retain_ms(&self) -> u64 {
        self.retain_ms
    }
}

/// Day files on disk.
#[derive(Debug)]
pub struct Journal {
    dir: PathBuf,
    keep_days: u32,
    last_day: Option<u64>,
}

impl Journal {
    pub fn new(dir: impl Into<PathBuf>, keep_days: u32) -> Self {
        Self {
            dir: dir.into(),
            keep_days: keep_days.max(1),
            last_day: None,
        }
    }

    /// Append one line to the attempt's day file; prune when a new day
    /// starts.
    pub fn append(&mut self, attempt: &Attempt) -> Result<(), String> {
        let day = attempt.at_ms / DAY_MS;
        if self.last_day != Some(day) {
            self.last_day = Some(day);
            self.prune(attempt.at_ms)?;
        }
        fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let path = self.dir.join(format!("{}.jsonl", date(day)));
        let mut line = serde_json::to_string(attempt).map_err(|e| e.to_string())?;
        line.push('\n');
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .and_then(|mut file| file.write_all(line.as_bytes()))
            .map_err(|e| e.to_string())
    }

    /// Delete day files older than `keep_days`; returns how many.
    pub fn prune(&self, now_ms: u64) -> Result<usize, String> {
        let oldest = date((now_ms / DAY_MS).saturating_sub(u64::from(self.keep_days) - 1));
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(e) => return Err(e.to_string()),
        };
        let mut removed = 0;
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(stem) = name.strip_suffix(".jsonl") else {
                continue;
            };
            if stem.len() == 10 && stem.as_bytes()[4] == b'-' && stem < oldest.as_str() {
                fs::remove_file(entry.path()).map_err(|e| e.to_string())?;
                removed += 1;
            }
        }
        Ok(removed)
    }
}

/// `YYYY-MM-DD` for a day number since the Unix epoch.
pub fn date(day: u64) -> String {
    // Howard Hinnant's civil-from-days.
    let z = day as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(ms: u64) -> Attempt {
        Attempt::new("r", 1, "zai", "m", ms)
    }

    #[test]
    fn keeps_the_newest_within_bounds() {
        let mut store = Store::new(2, 1_000);
        store.push(at(0));
        store.push(at(500));
        store.push(at(900));
        assert_eq!(store.len(), 2);
        store.prune(1_600);
        assert_eq!(store.len(), 1);
        assert_eq!(store.dropped, 2);
        assert_eq!(store.since(800).count(), 1);
    }

    #[test]
    fn dates() {
        assert_eq!(date(0), "1970-01-01");
        assert_eq!(date(20_735), "2026-10-09");
    }

    #[test]
    fn journal_rotates_and_prunes() {
        let dir = std::env::temp_dir().join(format!(
            "inference-journal-{}-{}",
            std::process::id(),
            line!()
        ));
        let _ = fs::remove_dir_all(&dir);
        let mut journal = Journal::new(&dir, 2);
        journal.append(&at(0)).unwrap();
        journal.append(&at(DAY_MS)).unwrap();
        journal.append(&at(2 * DAY_MS)).unwrap();
        let mut names: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(names, ["1970-01-02.jsonl", "1970-01-03.jsonl"]);
        let line = fs::read_to_string(dir.join("1970-01-03.jsonl")).unwrap();
        let back: Attempt = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(back.at_ms, 2 * DAY_MS);
        fs::remove_dir_all(&dir).unwrap();
    }
}
