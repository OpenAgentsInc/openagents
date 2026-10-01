//! The decision worker's usage log: one JSON line per job, so usage stats
//! are one command away (#10121).
//!
//! The owner decided on 2026-10-01 that the app shows no usage limit
//! anywhere and that every service records everything instead (#10120).
//! Every decision job the worker reads, answered or refused, is appended
//! to `DIR/YYYY-MM-DD.jsonl` (the UTC day it arrived; `DIR` is `usage/`
//! under `jobs_dir`) as one [`Record`]: when, which caller key and lane,
//! which model it asked and which model and door answered, the door's time
//! and the whole job's, tokens and cost when the door reported them, and
//! how it ended. A record holds ids, words from fixed vocabularies, counts,
//! and times, never the state or the questions.
//!
//! [`read`] and [`stats`] are what `decision-worker usage` runs. The chat
//! worker keeps the same log (`coder::relay::usage`); this crate does not
//! depend on `coder`, so the same design is kept here.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One job, as the usage log keeps it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Record {
    /// When the job arrived, RFC 3339 UTC with milliseconds.
    pub time: String,
    /// The caller's public key, hex: the request's verified signer.
    pub key: String,
    /// `open` (the server-held key), `principal` (an operator-provisioned
    /// key), `anonymous` (the shared door), or `-` for a request the
    /// worker could not admit.
    pub lane: String,
    /// The request id the caller chose.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<String>,
    /// The attempt, one-based.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt: Option<u32>,
    /// The model the job asked for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// The model that answered, as the door named it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub served_model: Option<String>,
    /// The door that answered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub door: Option<String>,
    /// `answered`, `refused`, `unavailable`, `unattempted`, or `unknown`.
    pub outcome: String,
    /// The typed code of a refusal or failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// The doors' milliseconds, when one was asked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub door_ms: Option<u64>,
    /// Milliseconds from arrival to the end.
    pub total_ms: u64,
    /// Input tokens, when the door reported them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_in: Option<u64>,
    /// Output tokens, when the door reported them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_out: Option<u64>,
    /// Dollars, when the door priced the answer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
    /// The request's ciphertext length.
    pub bytes_in: usize,
}

impl Record {
    /// A job from `key` whose request is `bytes_in` long, arriving at
    /// `unix_ms`, ending as `outcome`.
    #[must_use]
    pub fn new(key: &str, bytes_in: usize, unix_ms: u64, outcome: &str) -> Self {
        Self {
            time: rfc3339(unix_ms),
            key: key.to_string(),
            lane: "-".to_string(),
            outcome: outcome.to_string(),
            total_ms: unix_ms_now().saturating_sub(unix_ms),
            bytes_in,
            ..Self::default()
        }
    }

    /// Reads what an answer reports about itself: the door, and the tokens
    /// and cost when the door priced it (`usage.cost`, `jev::doors`).
    pub fn answer(&mut self, response: &Value) {
        self.door = response["service"]["door"].as_str().map(str::to_string);
        let usage = &response["usage"];
        self.tokens_in = usage["input_tokens"]
            .as_u64()
            .or_else(|| usage["prompt_tokens"].as_u64());
        self.tokens_out = usage["output_tokens"]
            .as_u64()
            .or_else(|| usage["completion_tokens"].as_u64());
        self.cost_usd = usage["cost"].as_f64();
    }
}

/// Unix milliseconds now.
#[must_use]
pub fn unix_ms_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |span| span.as_millis() as u64)
}

/// `YYYY-MM-DDTHH:MM:SS.mmmZ` for a Unix time in milliseconds.
#[must_use]
pub fn rfc3339(unix_ms: u64) -> String {
    let secs = unix_ms / 1_000;
    let (year, month, day) = crate::serve::civil_from_days((secs / 86_400) as i64);
    let of_day = secs % 86_400;
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        of_day / 3_600,
        of_day % 3_600 / 60,
        of_day % 60,
        unix_ms % 1_000
    )
}

/// Whether `text` is a `YYYY-MM-DD` date.
#[must_use]
pub fn is_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(at, byte)| at == 4 || at == 7 || byte.is_ascii_digit())
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
    Lane,
    Model,
    Door,
    Day,
    Outcome,
}

impl By {
    /// Reads `key`, `lane`, `model`, `door`, `day`, or `outcome`.
    ///
    /// # Errors
    ///
    /// Any other word.
    pub fn parse(text: &str) -> Result<Self, String> {
        Ok(match text {
            "key" => By::Key,
            "lane" => By::Lane,
            "model" => By::Model,
            "door" => By::Door,
            "day" => By::Day,
            "outcome" => By::Outcome,
            other => {
                return Err(format!(
                    "--by is key, lane, model, door, day, or outcome, not {other}"
                ));
            }
        })
    }

    /// The column heading.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            By::Key => "key",
            By::Lane => "lane",
            By::Model => "model",
            By::Door => "door",
            By::Day => "day",
            By::Outcome => "outcome",
        }
    }

    fn group(self, record: &Record) -> String {
        let or_none = |value: &Option<String>| value.clone().unwrap_or_else(|| "-".to_string());
        match self {
            By::Key => record.key.clone(),
            By::Lane => record.lane.clone(),
            By::Model => or_none(&record.served_model.clone().or(record.model.clone())),
            By::Door => or_none(&record.door),
            By::Day => record.time.get(..10).unwrap_or("-").to_string(),
            By::Outcome => match &record.code {
                Some(code) => format!("{} {code}", record.outcome),
                None => record.outcome.clone(),
            },
        }
    }
}

/// One group's totals.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Row {
    pub group: String,
    pub jobs: u64,
    pub answered: u64,
    pub refused: u64,
    /// Neither answered nor refused: no door could answer.
    pub failed: u64,
    /// Distinct caller keys.
    pub keys: u64,
    pub tokens_in: u64,
    pub tokens_out: u64,
    /// Summed over the jobs that reported a cost.
    pub cost_usd: f64,
    /// The median total time over answered jobs.
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
        .filter(|r| r.outcome == "answered")
        .map(|r| r.total_ms)
        .collect();
    totals.sort_unstable();
    let answered = count("answered");
    let refused = count("refused");
    Row {
        group,
        jobs: records.len() as u64,
        answered,
        refused,
        failed: records.len() as u64 - answered - refused,
        keys: keys.len() as u64,
        tokens_in: records.iter().filter_map(|r| r.tokens_in).sum(),
        tokens_out: records.iter().filter_map(|r| r.tokens_out).sum(),
        cost_usd: records.iter().filter_map(|r| r.cost_usd).sum(),
        total_p50_ms: totals.get(totals.len() / 2).copied(),
    }
}

/// `rows` as an aligned text table headed by `by`.
#[must_use]
pub fn table(rows: &[Row], by: By) -> String {
    let mut lines = vec![[
        by.word().to_string(),
        "jobs".into(),
        "answered".into(),
        "refused".into(),
        "failed".into(),
        "keys".into(),
        "tokens_in".into(),
        "cost_usd".into(),
        "total_p50_ms".into(),
    ]];
    for row in rows {
        lines.push([
            row.group.clone(),
            row.jobs.to_string(),
            row.answered.to_string(),
            row.refused.to_string(),
            row.failed.to_string(),
            row.keys.to_string(),
            row.tokens_in.to_string(),
            format!("{:.6}", row.cost_usd),
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
    use serde_json::json;

    #[test]
    fn times_are_utc() {
        assert_eq!(rfc3339(1_790_890_635_123), "2026-10-01T21:37:15.123Z");
        assert!(is_date("2026-10-01"));
        assert!(!is_date("../../etc/x"));
    }

    /// An answer's door, tokens, and cost reach the record; records land
    /// in their day's file and read back; `--since` skips earlier days;
    /// the stats group, count, and sum.
    #[test]
    fn the_log_rotates_by_day_and_the_stats_read_it_back() {
        let dir = tempfile::tempdir().unwrap();
        let log = Log::new(dir.path().join("usage"));
        let day_one = 1_790_812_800_000; // 2026-10-01T00:00:00Z
        let mut answered = Record::new("a", 900, day_one, "answered");
        answered.lane = "open".into();
        answered.answer(&json!({
            "model": "typesafe-ai/jev",
            "usage": {"input_tokens": 286, "cost": 0.00002},
            "service": {"door": "https://ai-gateway.vercel.sh", "version": "decision-worker@x"},
        }));
        assert_eq!(
            answered.door.as_deref(),
            Some("https://ai-gateway.vercel.sh")
        );
        assert_eq!(
            (answered.tokens_in, answered.cost_usd),
            (Some(286), Some(0.00002))
        );
        log.append(&answered).unwrap();
        log.append(&answered).unwrap();
        let mut refused = Record::new("b", 10, day_one + 86_400_000, "refused");
        refused.code = Some("limit_exceeded".into());
        log.append(&refused).unwrap();
        fs::write(log.dir().join("notes.txt"), "not a day").unwrap();

        let all = read(log.dir(), None).unwrap();
        assert_eq!((all.records.len(), all.unreadable), (3, 0));
        let rows = stats(&all.records, By::Door);
        assert_eq!(rows[0].group, "https://ai-gateway.vercel.sh");
        assert_eq!((rows[0].jobs, rows[0].answered, rows[0].keys), (2, 2, 1));
        assert_eq!(rows[0].tokens_in, 572);
        let total = rows.last().unwrap();
        assert_eq!((total.jobs, total.refused, total.keys), (3, 1, 2));
        assert_eq!(stats(&all.records, By::Day)[0].group, "2026-10-01");
        assert!(table(&rows, By::Door).starts_with("door"));
        assert_eq!(
            read(log.dir(), Some("2026-10-02")).unwrap().records.len(),
            1
        );
        assert!(read(log.dir(), Some("yesterday")).is_err());
        assert!(By::parse("color").is_err());
    }
}
