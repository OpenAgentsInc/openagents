//! Public token totals, derived only from the meter's attempt records.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::store::{DAY_MS, date};
use super::{Attempt, Audience, Outcome, Payment};

pub const SCHEMA: &str = "openagents.inference.tokens-served.v1";
const FILE: &str = "tokens-served.json";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counts {
    pub answers: u64,
    pub input: u64,
    pub output: u64,
    pub total: u64,
}

impl Counts {
    fn add(&mut self, attempt: &Attempt) {
        self.answers = self.answers.saturating_add(1);
        self.input = self.input.saturating_add(attempt.tokens.input);
        self.output = self.output.saturating_add(attempt.tokens.output);
        self.total = self.total.saturating_add(attempt.tokens.total());
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Billing {
    pub free: Counts,
    /// Includes calls on the caller's own provider key.
    pub paid: Counts,
    /// A subset of `paid`, shown separately so BYOK is explicit.
    pub own_key: Counts,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Totals {
    pub all: Counts,
    pub internal: Billing,
    pub outside: Billing,
}

impl Totals {
    fn add(&mut self, attempt: &Attempt) {
        self.all.add(attempt);
        let billing = match attempt.traffic.audience {
            Audience::Internal => &mut self.internal,
            Audience::Outside => &mut self.outside,
            Audience::Unknown => return,
        };
        match attempt.traffic.payment {
            Payment::Free => billing.free.add(attempt),
            Payment::Paid => billing.paid.add(attempt),
            Payment::OwnKey => {
                billing.paid.add(attempt);
                billing.own_key.add(attempt);
            }
            Payment::Unknown => {}
        }
    }
}

/// Totals since counting began. No caller, request, model, or key ids.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub v: String,
    /// First included attempt's start, Unix milliseconds; null before traffic.
    pub since_ms: Option<u64>,
    /// False when the meter has no journal directory and counts last only
    /// for this process.
    pub persistent: bool,
    pub totals: Totals,
    /// UTC dates. Cached tokens are part of input; reasoning is part of output.
    pub days: BTreeMap<String, Totals>,
}

impl Default for Report {
    fn default() -> Self {
        Self {
            v: SCHEMA.into(),
            since_ms: None,
            persistent: false,
            totals: Totals::default(),
            days: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Saved {
    #[serde(flatten)]
    report: Report,
    /// Byte positions already included from the meter's day files.
    #[serde(default)]
    offsets: BTreeMap<String, u64>,
}

#[derive(Debug)]
pub(super) struct Counter {
    path: Option<PathBuf>,
    saved: Saved,
    healthy: bool,
}

impl Counter {
    pub fn open(journal: Option<&Path>) -> Self {
        let path = journal.map(|dir| dir.join(FILE));
        let mut healthy = true;
        let mut saved = match path.as_ref().map(std::fs::read) {
            Some(Ok(bytes)) => match serde_json::from_slice::<Saved>(&bytes) {
                Ok(saved) if saved.report.v == SCHEMA => saved,
                _ => {
                    healthy = false;
                    Saved::default()
                }
            },
            Some(Err(error)) if error.kind() != std::io::ErrorKind::NotFound => {
                healthy = false;
                Saved::default()
            }
            _ => Saved::default(),
        };
        saved.report.persistent = path.is_some();
        let mut counter = Self {
            path,
            saved,
            healthy,
        };
        if counter.healthy
            && let Some(dir) = journal
        {
            let replay = (|| -> std::io::Result<()> {
                let entries = match std::fs::read_dir(dir) {
                    Ok(entries) => entries,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                    Err(error) => return Err(error),
                };
                let mut files = entries
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .map(|entry| entry.path())
                    .filter(|path| {
                        path.extension()
                            .is_some_and(|extension| extension == "jsonl")
                    })
                    .collect::<Vec<_>>();
                files.sort();
                for file in files {
                    counter.replay(&file)?;
                }
                counter.save()
            })();
            if replay.is_err() {
                counter.healthy = false;
            }
        }
        counter
    }

    pub fn report(&self) -> Option<Report> {
        self.healthy.then(|| self.saved.report.clone())
    }

    pub fn unavailable(&mut self) {
        self.healthy = false;
    }

    pub fn record(&mut self, attempt: &Attempt) {
        if !self.healthy {
            return;
        }
        let result = if let Some(path) = &self.path {
            let day_file = path
                .parent()
                .unwrap()
                .join(format!("{}.jsonl", date(attempt.at_ms / DAY_MS)));
            self.replay(&day_file).and_then(|()| self.save())
        } else {
            self.include(attempt);
            Ok(())
        };
        if result.is_err() {
            self.healthy = false;
            eprintln!("inference: token totals could not be saved");
        }
    }

    fn include(&mut self, attempt: &Attempt) {
        if attempt.outcome != Outcome::Ok
            || !attempt.usage_reported
            || attempt.tokens_counted
            || attempt.traffic.synthetic
            || attempt.traffic.audience == Audience::Unknown
            || attempt.traffic.payment == Payment::Unknown
        {
            return;
        }
        let report = &mut self.saved.report;
        report.since_ms = Some(
            report
                .since_ms
                .map_or(attempt.at_ms, |since| since.min(attempt.at_ms)),
        );
        report.totals.add(attempt);
        report
            .days
            .entry(date(attempt.at_ms / DAY_MS))
            .or_default()
            .add(attempt);
    }

    fn replay(&mut self, path: &Path) -> std::io::Result<()> {
        use std::io::{BufRead, Seek, SeekFrom};
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let offset = self.saved.offsets.get(&name).copied().unwrap_or_default();
        let mut file = std::fs::File::open(path)?;
        if file.metadata()?.len() < offset {
            return Err(std::io::Error::other("the attempt file was shortened"));
        }
        file.seek(SeekFrom::Start(offset))?;
        let mut reader = std::io::BufReader::new(file);
        let mut line = String::new();
        let mut included = offset;
        loop {
            line.clear();
            let read = reader.read_line(&mut line)?;
            if read == 0 {
                break;
            }
            if !line.ends_with('\n') {
                return Err(std::io::Error::other("the attempt record is incomplete"));
            }
            let attempt: Attempt = serde_json::from_str(&line).map_err(std::io::Error::other)?;
            self.include(&attempt);
            included = included.saturating_add(read as u64);
        }
        self.saved.offsets.insert(name, included);
        Ok(())
    }

    fn save(&self) -> std::io::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        use std::io::Write;
        let parent = path
            .parent()
            .expect("the counter is inside the journal directory");
        std::fs::create_dir_all(parent)?;
        let temporary = path.with_extension("json.tmp");
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(&serde_json::to_vec(&self.saved)?)?;
        file.sync_all()?;
        std::fs::rename(temporary, path)?;
        std::fs::File::open(parent)?.sync_all()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meter::{Config, Meter, Recorder, Traffic};

    fn attempt(day: u64, audience: Audience, payment: Payment) -> Attempt {
        Attempt {
            at_ms: day * DAY_MS,
            usage_reported: true,
            traffic: Traffic {
                audience,
                payment,
                synthetic: false,
            },
            tokens: super::super::Tokens {
                input: 10,
                cached_input: 4,
                cache_write: 2,
                output: 5,
                reasoning: 3,
            },
            ..Attempt::default()
        }
    }

    #[test]
    fn counts_only_reported_answers_with_server_assigned_labels() {
        let mut counter = Counter::open(None);
        for audience in [Audience::Internal, Audience::Outside] {
            for payment in [Payment::Free, Payment::Paid, Payment::OwnKey] {
                counter.record(&attempt(20_735, audience, payment));
            }
        }
        for change in 0..6 {
            let mut excluded = attempt(20_735, Audience::Outside, Payment::Paid);
            match change {
                0 => excluded.outcome = Outcome::Fallback,
                1 => excluded.outcome = Outcome::Canceled,
                2 => excluded.usage_reported = false,
                3 => excluded.tokens_counted = true,
                4 => excluded.traffic.synthetic = true,
                _ => excluded.traffic.audience = Audience::Unknown,
            }
            counter.record(&excluded);
        }
        let report = counter.report().unwrap();
        assert_eq!(report.totals.all.total, 90);
        assert_eq!(report.totals.all.answers, 6);
        assert_eq!(report.totals.internal.free.total, 15);
        assert_eq!(report.totals.outside.paid.total, 30);
        assert_eq!(report.totals.outside.own_key.total, 15);
        assert_eq!(report.days["2026-10-09"], report.totals);
        let json = serde_json::to_string(&report).unwrap();
        assert!(
            !json.contains("tenant") && !json.contains("key_id") && !json.contains("request_id")
        );
    }

    #[test]
    fn totals_outlive_the_live_window_and_a_restart() {
        let dir = std::env::temp_dir().join(format!(
            "inference-served-{}",
            super::super::super::upstream::emit::fresh_id("test")
        ));
        let config = Config {
            journal: Some(dir.clone()),
            max_records: Some(1),
            keep_days: Some(1),
            ..Config::default()
        };
        let meter = Meter::new(&config);
        meter.record(attempt(20_734, Audience::Internal, Payment::Free));
        meter.record(attempt(20_735, Audience::Outside, Payment::Paid));
        assert_eq!(meter.status(20_735 * DAY_MS).unwrap().records.kept, 1);
        let before = meter.tokens_served().unwrap();
        assert_eq!(before.totals.all.total, 30);
        assert_eq!(before.days.len(), 2);
        drop(meter);
        let meter = Meter::new(&config);
        assert_eq!(meter.tokens_served().unwrap(), before);
        meter.record(attempt(20_736, Audience::Outside, Payment::Free));
        assert_eq!(meter.tokens_served().unwrap().totals.all.total, 45);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn replays_an_attempt_saved_before_the_totals_update_once() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(super::super::super::upstream::emit::fresh_id(
            "inference-replay",
        ));
        let config = Config {
            journal: Some(dir.clone()),
            ..Config::default()
        };
        let meter = Meter::new(&config);
        meter.record(attempt(20_735, Audience::Outside, Payment::Free));
        drop(meter);
        let mut journal = std::fs::OpenOptions::new()
            .append(true)
            .open(dir.join("2026-10-09.jsonl"))
            .unwrap();
        serde_json::to_writer(
            &mut journal,
            &attempt(20_735, Audience::Outside, Payment::Paid),
        )
        .unwrap();
        journal.write_all(b"\n").unwrap();
        drop(journal);
        for _ in 0..2 {
            let meter = Meter::new(&config);
            let report = meter.tokens_served().unwrap();
            assert_eq!(report.totals.all.total, 30);
            assert_eq!(report.totals.outside.paid.total, 15);
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn corrupt_totals_are_unavailable_instead_of_reset_to_zero() {
        let dir = std::env::temp_dir().join(super::super::super::upstream::emit::fresh_id(
            "inference-served",
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(FILE), "broken").unwrap();
        let mut counter = Counter::open(Some(&dir));
        counter.record(&attempt(20_735, Audience::Outside, Payment::Paid));
        assert!(counter.report().is_none());
        assert_eq!(std::fs::read_to_string(dir.join(FILE)).unwrap(), "broken");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
