//! The hosted runner's usage log: one JSON line per job, so usage stats
//! are one command away (#10121).
//!
//! The owner decided on 2026-10-01 that the app shows no usage limit
//! anywhere and that every service records everything instead (#10120).
//! Every request the runner answers, admitted or refused, is appended to
//! `DIR/YYYY-MM-DD.jsonl` (the UTC day it arrived) as one [`Record`]: when,
//! which trainer key, what was asked (a run, a check, a validation, a
//! publish), which tool and test set, how many tests, runs, and agent
//! turns, how it ended, its verdict and pass counts, and how long it took.
//! A record holds ids, words from fixed vocabularies, counts, and times,
//! never a test's text or a key's secret.
//!
//! [`read`] and [`stats`] are what `eval-runner usage` runs; the dates are
//! the chat worker's (`coder::relay::usage`).

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use coder::relay::usage::{is_date, rfc3339};
use serde::{Deserialize, Serialize};

/// One job, as the usage log keeps it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Record {
    /// When the request arrived, RFC 3339 UTC with milliseconds.
    pub time: String,
    /// The trainer's public key, hex: the request's verified signer.
    pub key: String,
    /// The request event's id.
    pub request: String,
    /// `run`, `check`, `validation`, `publish`, or `unread` for a request
    /// whose input did not parse.
    pub action: String,
    /// The tested tool's DefinitionRef id, for a run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    /// The test set: its release's event id, or `draft` for a chat draft.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suite: Option<String>,
    /// Tests in the set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tests: Option<u64>,
    /// Runs per arm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runs: Option<u32>,
    /// Agent turns planned (tests x runs x arms).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turns: Option<u64>,
    /// `completed`, `failed`, `cancelled`, or `refused`.
    pub outcome: String,
    /// The typed code of a refusal or failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// The run's verdict: `pass`, `fail`, or `inconclusive`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verdict: Option<String>,
    /// Tests passed with the tool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_passed: Option<u64>,
    /// Tests passed without it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_passed: Option<u64>,
    /// The published result's event id, for a publish.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    /// Milliseconds from arrival to the end.
    pub total_ms: u64,
    /// The request's ciphertext length.
    pub bytes_in: usize,
}

impl Record {
    /// A record of a request from `key` with event id `request`, `bytes_in`
    /// long, arriving at `unix_ms`, ending as `outcome`.
    #[must_use]
    pub fn new(key: &str, request: &str, bytes_in: usize, unix_ms: u64, outcome: &str) -> Self {
        Self {
            time: rfc3339(unix_ms),
            key: key.to_string(),
            request: request.to_string(),
            action: "unread".to_string(),
            outcome: outcome.to_string(),
            bytes_in,
            ..Self::default()
        }
    }
}

/// Unix milliseconds now.
#[must_use]
pub fn unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |span| span.as_millis() as u64)
}

/// The usage log: a directory of day files.
#[derive(Clone, Debug)]
pub struct Log {
    dir: PathBuf,
}

impl Log {
    /// A log in `dir`, made on the first append if it is missing.
    #[must_use]
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// The directory the log writes to.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Appends `record` to its day's file as one line.
    ///
    /// # Errors
    ///
    /// The directory or the file could not be written.
    pub fn append(&self, record: &Record) -> Result<(), String> {
        fs::create_dir_all(&self.dir).map_err(|e| format!("{}: {e}", self.dir.display()))?;
        let day = record.time.get(..10).unwrap_or("unknown");
        let path = self.dir.join(format!("{day}.jsonl"));
        let mut line = serde_json::to_string(record).map_err(|e| e.to_string())?;
        line.push('\n');
        // One write of one line to a file opened for appending: lines from
        // concurrent jobs do not interleave.
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .and_then(|mut file| file.write_all(line.as_bytes()))
            .map_err(|e| format!("{}: {e}", path.display()))
    }
}

/// What [`read`] found.
#[derive(Debug, Default)]
pub struct Read {
    pub records: Vec<Record>,
    /// Lines that did not parse as a record.
    pub unreadable: usize,
}

/// Every record in `dir` from day files on or after `since`
/// (`YYYY-MM-DD`), oldest day first.
///
/// # Errors
///
/// `dir` could not be listed, or `since` is not a date.
pub fn read(dir: &Path, since: Option<&str>) -> Result<Read, String> {
    if let Some(since) = since
        && !is_date(since)
    {
        return Err(format!("{since} is not a YYYY-MM-DD date"));
    }
    let mut days: Vec<(String, PathBuf)> = fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let day = name.strip_suffix(".jsonl")?.to_string();
            is_date(&day).then(|| (day, entry.path()))
        })
        .filter(|(day, _)| since.is_none_or(|since| day.as_str() >= since))
        .collect();
    days.sort();
    let mut read = Read::default();
    for (_, path) in days {
        let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            match serde_json::from_str::<Record>(line) {
                Ok(record) => read.records.push(record),
                Err(_) => read.unreadable += 1,
            }
        }
    }
    Ok(read)
}

/// What [`stats`] groups by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum By {
    Key,
    Action,
    Subject,
    Day,
    Outcome,
}

impl By {
    /// Reads `key`, `action`, `subject`, `day`, or `outcome`.
    ///
    /// # Errors
    ///
    /// Any other word.
    pub fn parse(text: &str) -> Result<Self, String> {
        Ok(match text {
            "key" => By::Key,
            "action" => By::Action,
            "subject" => By::Subject,
            "day" => By::Day,
            "outcome" => By::Outcome,
            other => {
                return Err(format!(
                    "--by is key, action, subject, day, or outcome, not {other}"
                ));
            }
        })
    }

    /// The column heading.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            By::Key => "key",
            By::Action => "action",
            By::Subject => "subject",
            By::Day => "day",
            By::Outcome => "outcome",
        }
    }

    fn group(self, record: &Record) -> String {
        match self {
            By::Key => record.key.clone(),
            By::Action => record.action.clone(),
            By::Subject => record.subject.clone().unwrap_or_else(|| "-".to_string()),
            By::Day => record.time.get(..10).unwrap_or("-").to_string(),
            By::Outcome => match &record.code {
                Some(code) => format!("{} {code}", record.outcome),
                None => record.outcome.clone(),
            },
        }
    }
}

/// One group's totals.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Row {
    pub group: String,
    pub jobs: u64,
    pub completed: u64,
    pub failed: u64,
    pub refused: u64,
    /// Distinct trainer keys.
    pub keys: u64,
    /// Agent turns planned by the jobs that ran.
    pub turns: u64,
    /// The median total time over completed jobs.
    pub total_p50_ms: Option<u64>,
}

/// `records` grouped `by`, busiest group first (days in order), with a
/// final `all` row.
#[must_use]
pub fn stats(records: &[Record], by: By) -> Vec<Row> {
    let mut groups: BTreeMap<String, Vec<&Record>> = BTreeMap::new();
    for record in records {
        groups.entry(by.group(record)).or_default().push(record);
    }
    let mut rows: Vec<Row> = groups
        .into_iter()
        .map(|(group, records)| row(group, &records))
        .collect();
    if by == By::Day {
        rows.sort_by(|a, b| a.group.cmp(&b.group));
    } else {
        rows.sort_by(|a, b| b.jobs.cmp(&a.jobs).then_with(|| a.group.cmp(&b.group)));
    }
    let all: Vec<&Record> = records.iter().collect();
    rows.push(row("all".to_string(), &all));
    rows
}

fn row(group: String, records: &[&Record]) -> Row {
    let count = |outcome: &str| records.iter().filter(|r| r.outcome == outcome).count() as u64;
    let mut keys: Vec<&str> = records.iter().map(|r| r.key.as_str()).collect();
    keys.sort_unstable();
    keys.dedup();
    let mut totals: Vec<u64> = records
        .iter()
        .filter(|r| r.outcome == "completed")
        .map(|r| r.total_ms)
        .collect();
    totals.sort_unstable();
    Row {
        group,
        jobs: records.len() as u64,
        completed: count("completed"),
        failed: count("failed"),
        refused: count("refused"),
        keys: keys.len() as u64,
        turns: records
            .iter()
            .filter(|r| r.outcome != "refused")
            .filter_map(|r| r.turns)
            .sum(),
        total_p50_ms: totals.get(totals.len() / 2).copied(),
    }
}

/// `rows` as an aligned text table headed by `by`.
#[must_use]
pub fn table(rows: &[Row], by: By) -> String {
    let mut lines = vec![[
        by.word().to_string(),
        "jobs".into(),
        "completed".into(),
        "failed".into(),
        "refused".into(),
        "keys".into(),
        "turns".into(),
        "total_p50_ms".into(),
    ]];
    for row in rows {
        lines.push([
            row.group.clone(),
            row.jobs.to_string(),
            row.completed.to_string(),
            row.failed.to_string(),
            row.refused.to_string(),
            row.keys.to_string(),
            row.turns.to_string(),
            row.total_p50_ms
                .map_or_else(|| "-".to_string(), |v| v.to_string()),
        ]);
    }
    let widths: Vec<usize> = (0..lines[0].len())
        .map(|column| lines.iter().map(|l| l[column].len()).max().unwrap_or(0))
        .collect();
    let mut out = String::new();
    for line in &lines {
        let cells: Vec<String> = line
            .iter()
            .enumerate()
            .map(|(column, cell)| {
                if column == 0 {
                    format!("{cell:<width$}", width = widths[column])
                } else {
                    format!("{cell:>width$}", width = widths[column])
                }
            })
            .collect();
        out.push_str(cells.join("  ").trim_end());
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Records land in their day's file and read back; `--since` skips
    /// earlier days, and the stats group, count, and take medians.
    #[test]
    fn the_log_rotates_by_day_and_the_stats_read_it_back() {
        let dir = tempfile::tempdir().unwrap();
        let log = Log::new(dir.path().join("usage"));
        let day_one = 1_790_812_800_000; // 2026-10-01T00:00:00Z
        let job = |key: &str, at: u64, action: &str, outcome: &str, ms: u64| Record {
            action: action.into(),
            turns: Some(12),
            total_ms: ms,
            ..Record::new(key, "e1", 900, at, outcome)
        };
        log.append(&job("a", day_one, "run", "completed", 100))
            .unwrap();
        log.append(&job("a", day_one + 1, "run", "completed", 300))
            .unwrap();
        log.append(&job("b", day_one + 86_400_000, "check", "refused", 2))
            .unwrap();
        fs::write(log.dir().join("notes.txt"), "not a day").unwrap();

        let all = read(log.dir(), None).unwrap();
        assert_eq!((all.records.len(), all.unreadable), (3, 0));
        let rows = stats(&all.records, By::Action);
        assert_eq!(rows[0].group, "run");
        assert_eq!((rows[0].jobs, rows[0].completed, rows[0].keys), (2, 2, 1));
        assert_eq!((rows[0].turns, rows[0].total_p50_ms), (24, Some(300)));
        assert_eq!((rows[1].group.as_str(), rows[1].refused), ("check", 1));
        let total = rows.last().unwrap();
        assert_eq!(
            (total.group.as_str(), total.jobs, total.keys),
            ("all", 3, 2)
        );
        assert_eq!(stats(&all.records, By::Day)[0].group, "2026-10-01");
        assert!(table(&rows, By::Action).starts_with("action  jobs"));
        assert_eq!(
            read(log.dir(), Some("2026-10-02")).unwrap().records.len(),
            1
        );
        assert!(read(log.dir(), Some("yesterday")).is_err());
        assert!(By::parse("color").is_err());
    }
}
