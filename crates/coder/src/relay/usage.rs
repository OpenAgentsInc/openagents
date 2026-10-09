//! The chat worker's usage log: one JSON line per job, so usage stats are
//! one command away (#10120).
//!
//! The owner decided on 2026-10-01 that the app shows no usage limit
//! anywhere and that the worker records everything instead. Every job the
//! worker reads is appended to `DIR/YYYY-MM-DD.jsonl` (the UTC day the job
//! arrived, so the files rotate by day) as one [`Record`]: when, which
//! caller key, which client and surface, how it was routed and answered,
//! which model and door wrote it, which Jev door judged it, how long it
//! took, its tokens, and how it ended. No message text is ever written:
//! the record holds ids, words from fixed vocabularies, counts, and times.
//!
//! Day files are deleted once they are older than the log's window
//! ([`Log::keeping`]; the chat worker keeps [`DEFAULT_KEEP_DAYS`] unless
//! `CODER_WORKER_USAGE_DAYS` says otherwise, #11042): the log prunes when
//! the first job of a new day is appended, and the worker prunes once at
//! start.
//!
//! [`read`] and [`stats`] are what `coder-worker usage` runs; read
//! `docs/deployment/chat-worker-usage.md` for the queries.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// How a job ended.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// A result was published.
    Answered,
    /// The worker's own door or judge failed (`internal`).
    Failed,
    /// The worker declined with a typed code (`busy`, `stale`, ...).
    Refused,
    /// The worker published neither a result nor a refusal: the serving
    /// loop went away mid-job.
    #[default]
    Unfinished,
}

impl Outcome {
    fn word(self) -> &'static str {
        match self {
            Outcome::Answered => "answered",
            Outcome::Failed => "failed",
            Outcome::Refused => "refused",
            Outcome::Unfinished => "unfinished",
        }
    }
}

/// One job, as the usage log keeps it. Every field but `time`, `key`,
/// `kind`, `outcome`, and the sizes is optional: a refused job has no
/// model, a prepared answer no tokens.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Record {
    /// When the job arrived, RFC 3339 UTC with milliseconds.
    pub time: String,
    /// The caller's public key, hex: the request's verified signer.
    pub key: String,
    /// `turn`, `rank`, `probe`, `delegation`, or `unread` for a request
    /// that did not decrypt or parse.
    pub kind: String,
    /// The client's own word (`openagents-mobile`, `openagents-desktop`,
    /// `openagents-terminal`, `openagents-web`, ...), when it sent one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<String>,
    /// The surface its typed context named (`phone`, `desktop`,
    /// `terminal`, `web`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface: Option<String>,
    /// The chat router's route, for a routed turn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    /// The tier the turn was served at, for a routed turn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
    /// `id@version` of the bank entry that supplied text, when one did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer: Option<String>,
    /// The model the result names as its writer (`bank:...` when no model
    /// wrote it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// The door that answered: the host of the model's gateway, or the
    /// door's name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub door: Option<String>,
    /// The Jev door that judged the turn, when one did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jev_door: Option<String>,
    /// The model that Jev door served.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jev_model: Option<String>,
    /// Milliseconds from arrival to the first words sent, when any were.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_token_ms: Option<u64>,
    /// Milliseconds from arrival to the job's end.
    pub total_ms: u64,
    /// Input tokens, when the door reported them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_in: Option<u64>,
    /// Output tokens, when the door reported them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_out: Option<u64>,
    /// Dollars, when the door reported a cost.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
    /// How the job ended.
    pub outcome: Outcome,
    /// The typed code of a refusal or failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// The request's ciphertext length.
    pub bytes_in: usize,
    /// The published reply text's length, in bytes.
    pub bytes_out: usize,
    /// Who paid for the job's model calls: `theirs` when the caller sent
    /// its own provider keys (BYOK, a `payer.keys` job); absent is ours.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payer: Option<String>,
    /// The provider of the caller's first key: `openrouter`, `vercel`, or
    /// `typesafe`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payer_provider: Option<String>,
    /// That key's fingerprint (`model_access::fingerprint`), never the key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payer_fingerprint: Option<String>,
}

/// What a job published, watched as it is published, folded into a
/// [`Record`] at the end.
#[derive(Debug, Default)]
pub struct Observed {
    pub record: Record,
}

impl Observed {
    /// A job from `key` whose request is `bytes_in` long, arriving at
    /// `unix_ms`.
    #[must_use]
    pub fn new(key: &str, bytes_in: usize, unix_ms: u64) -> Self {
        Self {
            record: Record {
                time: rfc3339(unix_ms),
                key: key.to_string(),
                kind: "unread".to_string(),
                bytes_in,
                ..Record::default()
            },
        }
    }

    /// Reads what the request's payload says about its caller: the
    /// client's word, the surface, and the job's kind. Never its text.
    pub fn request(&mut self, payload: &Value) {
        let record = &mut self.record;
        record.client = word(&payload["client"]);
        record.surface = word(&payload["context"]["surface"]);
        record.kind = if payload["delegation"].is_object() {
            "delegation".to_string()
        } else {
            match payload["type"].as_str() {
                Some("probe") => "probe".to_string(),
                Some("rank") => "rank".to_string(),
                _ => "turn".to_string(),
            }
        };
    }

    /// Notes that the caller paid with its own keys: the first key's
    /// provider and fingerprint, never a key.
    pub fn paid_by(&mut self, payer: &model_access::Payer) {
        if let model_access::Payer::Theirs {
            provider,
            fingerprint,
        } = payer
        {
            self.record.payer = Some("theirs".to_string());
            self.record.payer_provider = Some(provider.word().to_string());
            self.record.payer_fingerprint = Some(fingerprint.clone());
        }
    }

    /// Notes one published body, `elapsed_ms` after the job arrived.
    pub fn saw(&mut self, body: &Value, elapsed_ms: u64) {
        let record = &mut self.record;
        match body["type"].as_str() {
            Some("partial") => {
                if record.first_token_ms.is_none() {
                    record.first_token_ms = Some(elapsed_ms);
                }
            }
            Some("judgment") => {
                record.jev_door = word(&body["door"]);
                record.jev_model = word(&body["model"]);
            }
            Some("result") => {
                record.outcome = Outcome::Answered;
                record.code = None;
                record.model = word(&body["model"]);
                record.route = word(&body["route"]);
                record.tier = word(&body["tier"]);
                record.answer = word(&body["answer"]);
                record.tokens_in = body["usage"]["input"].as_u64();
                record.tokens_out = body["usage"]["output"].as_u64();
                record.cost_usd = body["usage"]["cost_usd"].as_f64();
                record.bytes_out = body["text"].as_str().map_or(0, str::len);
                if record.first_token_ms.is_none() && record.bytes_out > 0 {
                    record.first_token_ms = Some(elapsed_ms);
                }
            }
            Some("status") if body["status"].as_str() == Some("error") => {
                let code = word(&body["code"]);
                record.outcome = if code.as_deref() == Some("internal") {
                    Outcome::Failed
                } else {
                    Outcome::Refused
                };
                record.code = code;
            }
            _ => {}
        }
    }

    /// The record, ending `total_ms` after arrival, with the door that
    /// answered.
    #[must_use]
    pub fn finish(mut self, total_ms: u64, door: Option<String>) -> Record {
        self.record.total_ms = total_ms;
        if self.record.outcome == Outcome::Answered {
            self.record.door = door;
        }
        self.record
    }
}

/// A short word from a fixed vocabulary, never free text: at most 128
/// bytes of printable ASCII, else nothing.
fn word(value: &Value) -> Option<String> {
    value
        .as_str()
        .filter(|text| {
            !text.is_empty()
                && text.len() <= 128
                && text
                    .bytes()
                    .all(|byte| byte.is_ascii_graphic() || byte == b' ')
        })
        .map(str::to_string)
}

/// How many days of usage the chat worker keeps unless configured
/// otherwise: today and the 29 days before it.
pub const DEFAULT_KEEP_DAYS: u32 = 30;

/// The usage log: a directory of day files.
#[derive(Clone, Debug)]
pub struct Log {
    dir: PathBuf,
    /// Days of files kept, today included; `None` keeps every file.
    keep: Option<u32>,
    /// The day the log last pruned for, so it prunes once a day.
    pruned: std::sync::Arc<std::sync::Mutex<String>>,
}

impl Log {
    /// A log in `dir`, made on the first append if it is missing, that
    /// keeps every day file until [`Log::keeping`] bounds it.
    #[must_use]
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            keep: None,
            pruned: std::sync::Arc::default(),
        }
    }

    /// The same log keeping `days` days of files, today included (`None`
    /// keeps them all; zero is read as one).
    #[must_use]
    pub fn keeping(mut self, days: Option<u32>) -> Self {
        self.keep = days.map(|days| days.max(1));
        self
    }

    /// The days of files the log keeps, or `None` for all of them.
    #[must_use]
    pub fn keeps(&self) -> Option<u32> {
        self.keep
    }

    /// Deletes the day files older than the window, as of `now_secs` (Unix
    /// seconds), and returns how many it deleted. Files that are not day
    /// files are left alone; a log that keeps everything deletes nothing.
    ///
    /// # Errors
    ///
    /// The directory could not be listed or a file could not be deleted.
    /// A missing directory is nothing to prune.
    pub fn prune(&self, now_secs: u64) -> Result<usize, String> {
        let Some(keep) = self.keep else {
            return Ok(0);
        };
        let oldest = date(now_secs.saturating_sub(u64::from(keep - 1) * 86_400));
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(e) => return Err(format!("{}: {e}", self.dir.display())),
        };
        let mut deleted = 0;
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(day) = name.strip_suffix(".jsonl") else {
                continue;
            };
            if is_date(day) && day < oldest.as_str() {
                fs::remove_file(entry.path())
                    .map_err(|e| format!("{}: {e}", entry.path().display()))?;
                deleted += 1;
            }
        }
        Ok(deleted)
    }

    /// Prunes once per new day seen in `day`, as of the clock now. A
    /// failure is reported on stderr and costs nothing else: the job's line
    /// is already written.
    fn prune_for(&self, day: &str) {
        if self.keep.is_none() {
            return;
        }
        {
            let Ok(mut pruned) = self.pruned.lock() else {
                return;
            };
            if pruned.as_str() == day {
                return;
            }
            *pruned = day.to_string();
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_secs());
        if let Err(why) = self.prune(now) {
            eprintln!("usage   could not delete old usage files: {why}");
        }
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
            .map_err(|e| format!("{}: {e}", path.display()))?;
        self.prune_for(day);
        Ok(())
    }
}

/// `YYYY-MM-DD` for a Unix time in seconds, UTC.
#[must_use]
pub fn date(unix_secs: u64) -> String {
    let (year, month, day) = civil(unix_secs / 86_400);
    format!("{year:04}-{month:02}-{day:02}")
}

/// `YYYY-MM-DDTHH:MM:SS.mmmZ` for a Unix time in milliseconds.
#[must_use]
pub fn rfc3339(unix_ms: u64) -> String {
    let secs = unix_ms / 1_000;
    let of_day = secs % 86_400;
    format!(
        "{}T{:02}:{:02}:{:02}.{:03}Z",
        date(secs),
        of_day / 3_600,
        of_day % 3_600 / 60,
        of_day % 60,
        unix_ms % 1_000
    )
}

/// The civil date of a day count since 1970-01-01 (Howard Hinnant's
/// `civil_from_days`).
fn civil(days: u64) -> (u64, u64, u64) {
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + u64::from(month <= 2);
    (year, month, day)
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
    Surface,
    Route,
    Model,
    Day,
    Kind,
    Outcome,
    /// `ours`, or `theirs PROVIDER` for a job the caller paid for.
    Payer,
}

impl By {
    /// Reads `key`, `surface`, `route`, `model`, `day`, `kind`, or
    /// `outcome`.
    ///
    /// # Errors
    ///
    /// Any other word.
    pub fn parse(text: &str) -> Result<Self, String> {
        Ok(match text {
            "key" => By::Key,
            "surface" => By::Surface,
            "route" => By::Route,
            "model" => By::Model,
            "day" => By::Day,
            "kind" => By::Kind,
            "outcome" => By::Outcome,
            "payer" => By::Payer,
            other => {
                return Err(format!(
                    "--by is key, surface, route, model, day, kind, outcome, or payer, not {other}"
                ));
            }
        })
    }

    /// The column heading.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            By::Key => "key",
            By::Surface => "surface",
            By::Route => "route",
            By::Model => "model",
            By::Day => "day",
            By::Kind => "kind",
            By::Outcome => "outcome",
            By::Payer => "payer",
        }
    }

    fn group(self, record: &Record) -> String {
        let or_none = |value: &Option<String>| value.clone().unwrap_or_else(|| "-".to_string());
        match self {
            By::Key => record.key.clone(),
            // A client that names no surface is grouped by its word.
            By::Surface => record
                .surface
                .clone()
                .or_else(|| record.client.clone())
                .unwrap_or_else(|| "-".to_string()),
            By::Route => or_none(&record.route),
            By::Model => or_none(&record.model),
            By::Day => record.time.get(..10).unwrap_or("-").to_string(),
            By::Kind => record.kind.clone(),
            By::Outcome => match &record.code {
                Some(code) => format!("{} {code}", record.outcome.word()),
                None => record.outcome.word().to_string(),
            },
            By::Payer => match (&record.payer, &record.payer_provider) {
                (Some(payer), Some(provider)) => format!("{payer} {provider}"),
                (Some(payer), None) => payer.clone(),
                _ => "ours".to_string(),
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
    pub failed: u64,
    pub refused: u64,
    /// Distinct caller keys.
    pub keys: u64,
    pub tokens_in: u64,
    pub tokens_out: u64,
    /// Summed over the jobs that reported a cost.
    pub cost_usd: f64,
    /// The median first-token time over jobs that sent words.
    pub first_token_p50_ms: Option<u64>,
    /// The median total time over answered jobs.
    pub total_p50_ms: Option<u64>,
    pub bytes_in: u64,
    pub bytes_out: u64,
}

/// `records` grouped `by`, busiest group first, with a final `all` row.
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
    rows.sort_by(|a, b| b.jobs.cmp(&a.jobs).then_with(|| a.group.cmp(&b.group)));
    if by == By::Day {
        rows.sort_by(|a, b| a.group.cmp(&b.group));
    }
    let all: Vec<&Record> = records.iter().collect();
    rows.push(row("all".to_string(), &all));
    rows
}

fn row(group: String, records: &[&Record]) -> Row {
    let count = |outcome| records.iter().filter(|r| r.outcome == outcome).count() as u64;
    let mut keys: Vec<&str> = records.iter().map(|r| r.key.as_str()).collect();
    keys.sort_unstable();
    keys.dedup();
    let firsts: Vec<u64> = records.iter().filter_map(|r| r.first_token_ms).collect();
    let totals: Vec<u64> = records
        .iter()
        .filter(|r| r.outcome == Outcome::Answered)
        .map(|r| r.total_ms)
        .collect();
    Row {
        group,
        jobs: records.len() as u64,
        answered: count(Outcome::Answered),
        failed: count(Outcome::Failed),
        refused: count(Outcome::Refused),
        keys: keys.len() as u64,
        tokens_in: records.iter().filter_map(|r| r.tokens_in).sum(),
        tokens_out: records.iter().filter_map(|r| r.tokens_out).sum(),
        cost_usd: records.iter().filter_map(|r| r.cost_usd).sum(),
        first_token_p50_ms: median(firsts),
        total_p50_ms: median(totals),
        bytes_in: records.iter().map(|r| r.bytes_in as u64).sum(),
        bytes_out: records.iter().map(|r| r.bytes_out as u64).sum(),
    }
}

fn median(mut values: Vec<u64>) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    Some(values[values.len() / 2])
}

/// `rows` as an aligned text table headed by `by`.
#[must_use]
pub fn table(rows: &[Row], by: By) -> String {
    let ms = |value: Option<u64>| value.map_or_else(|| "-".to_string(), |v| v.to_string());
    let mut lines = vec![[
        by.word().to_string(),
        "jobs".into(),
        "answered".into(),
        "failed".into(),
        "refused".into(),
        "keys".into(),
        "tokens_in".into(),
        "tokens_out".into(),
        "first_p50_ms".into(),
        "total_p50_ms".into(),
    ]];
    for row in rows {
        lines.push([
            row.group.clone(),
            row.jobs.to_string(),
            row.answered.to_string(),
            row.failed.to_string(),
            row.refused.to_string(),
            row.keys.to_string(),
            row.tokens_in.to_string(),
            row.tokens_out.to_string(),
            ms(row.first_token_p50_ms),
            ms(row.total_p50_ms),
        ]);
    }
    let widths: Vec<usize> = (0..lines[0].len())
        .map(|column| {
            lines
                .iter()
                .map(|line| line[column].len())
                .max()
                .unwrap_or(0)
        })
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
    fn dates_are_utc_civil_dates() {
        assert_eq!(date(0), "1970-01-01");
        assert_eq!(date(951_782_400), "2000-02-29");
        assert_eq!(rfc3339(1_790_890_635_123), "2026-10-01T21:37:15.123Z");
        assert!(is_date("2026-10-01"));
        assert!(!is_date("2026-1-01"));
        assert!(!is_date("../../etc"));
    }

    /// A routed turn's published bodies become one record: the caller,
    /// the route, the model, the Jev door, the first token, the tokens,
    /// and never the text.
    #[test]
    fn a_turn_is_recorded_from_what_it_published_without_its_text() {
        let mut observed = Observed::new("ab", 812, 1_790_890_635_000);
        observed.request(&json!({
            "v": 2, "task": "secret question", "client": "openagents-mobile",
            "context": {"surface": "phone"}, "router": "chat-router-v4",
        }));
        observed.saw(&json!({"type": "status", "status": "processing"}), 5);
        observed.saw(
            &json!({"type": "judgment", "door": "https://api.typesafe.ai", "model": "jev-1.13"}),
            300,
        );
        observed.saw(&json!({"type": "partial", "delta": "secret answer"}), 900);
        observed.saw(&json!({"type": "partial", "delta": " more"}), 950);
        observed.saw(
            &json!({"type": "result", "text": "secret answer more", "model": "example/primary-model",
                "route": "general", "tier": "model", "usage": {"input": 120, "output": 40}}),
            1_400,
        );
        let record = observed.finish(1_410, Some("openrouter.ai".into()));
        assert_eq!(record.kind, "turn");
        assert_eq!(record.client.as_deref(), Some("openagents-mobile"));
        assert_eq!(record.surface.as_deref(), Some("phone"));
        assert_eq!(record.route.as_deref(), Some("general"));
        assert_eq!(record.tier.as_deref(), Some("model"));
        assert_eq!(record.jev_model.as_deref(), Some("jev-1.13"));
        assert_eq!(record.door.as_deref(), Some("openrouter.ai"));
        assert_eq!(record.first_token_ms, Some(900));
        assert_eq!(record.total_ms, 1_410);
        assert_eq!((record.tokens_in, record.tokens_out), (Some(120), Some(40)));
        assert_eq!(record.outcome, Outcome::Answered);
        assert_eq!(record.bytes_out, "secret answer more".len());
        let line = serde_json::to_string(&record).unwrap();
        assert!(!line.contains("secret"), "{line}");
    }

    /// A job the caller paid for with its own key names the payer, the
    /// provider, and the fingerprint, never the key, and groups by payer.
    #[test]
    fn a_job_on_the_callers_key_records_the_payer_and_never_the_key() {
        let key = "sk-or-v1-callers-own-key";
        let mut observed = Observed::new("ab", 10, 0);
        observed.paid_by(&model_access::Payer::Theirs {
            provider: model_access::Provider::OpenRouter,
            fingerprint: model_access::fingerprint(key),
        });
        observed.saw(&json!({"type": "result", "text": "hi", "model": "m"}), 5);
        let record = observed.finish(6, None);
        let line = serde_json::to_string(&record).unwrap();
        assert!(!line.contains(key), "{line}");
        assert!(line.contains(r#""payer":"theirs""#), "{line}");
        assert!(line.contains(&model_access::fingerprint(key)), "{line}");
        let ours = Observed::new("cd", 10, 0).finish(6, None);
        let rows = stats(&[record, ours], By::parse("payer").unwrap());
        let groups: Vec<&str> = rows.iter().map(|row| row.group.as_str()).collect();
        assert!(groups.contains(&"theirs openrouter") && groups.contains(&"ours"));
    }

    #[test]
    fn a_refusal_and_a_failure_are_recorded_with_their_codes() {
        let mut busy = Observed::new("cd", 10, 0);
        busy.saw(
            &json!({"type": "status", "status": "error", "code": "busy"}),
            1,
        );
        let busy = busy.finish(2, Some("door".into()));
        assert_eq!(busy.outcome, Outcome::Refused);
        assert_eq!(busy.code.as_deref(), Some("busy"));
        assert_eq!(busy.door, None);
        let mut failed = Observed::new("cd", 10, 0);
        failed.request(&json!({"type": "rank"}));
        failed.saw(
            &json!({"type": "status", "status": "error", "code": "internal",
            "message": "the door said 500"}),
            1,
        );
        let failed = failed.finish(2, None);
        assert_eq!(
            (failed.kind.as_str(), failed.outcome),
            ("rank", Outcome::Failed)
        );
    }

    /// Records land in their day's file and read back; `--since` skips
    /// earlier days, and the stats group, count, and take medians.
    #[test]
    fn the_log_rotates_by_day_and_the_stats_read_it_back() {
        let dir = tempfile::tempdir().unwrap();
        let log = Log::new(dir.path().join("usage"));
        let day_one = 1_790_812_800_000; // 2026-10-01T00:00:00Z
        let day_two = day_one + 86_400_000;
        let job = |key: &str, at: u64, surface: &str, outcome: Outcome, first: u64| Record {
            time: rfc3339(at),
            key: key.into(),
            kind: "turn".into(),
            surface: Some(surface.into()),
            model: Some("m".into()),
            first_token_ms: Some(first),
            total_ms: first * 2,
            tokens_in: Some(10),
            tokens_out: Some(5),
            outcome,
            bytes_in: 100,
            ..Record::default()
        };
        log.append(&job("a", day_one, "phone", Outcome::Answered, 100))
            .unwrap();
        log.append(&job("a", day_one + 1, "phone", Outcome::Answered, 300))
            .unwrap();
        log.append(&job("b", day_two, "web", Outcome::Refused, 200))
            .unwrap();
        fs::write(log.dir().join("notes.txt"), "not a day").unwrap();
        let mut files: Vec<String> = fs::read_dir(log.dir())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        files.sort();
        assert_eq!(files, ["2026-10-01.jsonl", "2026-10-02.jsonl", "notes.txt"]);

        let all = read(log.dir(), None).unwrap();
        assert_eq!((all.records.len(), all.unreadable), (3, 0));
        let rows = stats(&all.records, By::Surface);
        assert_eq!(rows[0].group, "phone");
        assert_eq!((rows[0].jobs, rows[0].answered, rows[0].keys), (2, 2, 1));
        assert_eq!(rows[0].tokens_in, 20);
        assert_eq!(rows[0].first_token_p50_ms, Some(300));
        assert_eq!(rows[1].group, "web");
        assert_eq!(rows[1].refused, 1);
        let total = rows.last().unwrap();
        assert_eq!(
            (total.group.as_str(), total.jobs, total.keys),
            ("all", 3, 2)
        );
        let days = stats(&all.records, By::Day);
        assert_eq!(days[0].group, "2026-10-01");
        assert!(table(&rows, By::Surface).starts_with("surface  jobs"));

        let since = read(log.dir(), Some("2026-10-02")).unwrap();
        assert_eq!(since.records.len(), 1);
        // A log with no window keeps every day.
        assert_eq!(log.prune(day_two / 1_000 + 400 * 86_400).unwrap(), 0);
        assert!(read(log.dir(), Some("yesterday")).is_err());
        assert!(By::parse("color").is_err());
    }

    /// #11042: a bounded log deletes day files older than its window and
    /// keeps the rest, including files that are not day files.
    #[test]
    fn a_bounded_log_deletes_days_past_its_window() {
        let dir = tempfile::tempdir().unwrap();
        let log = Log::new(dir.path().join("usage")).keeping(Some(30));
        assert_eq!(log.keeps(), Some(30));
        // Nothing written yet: nothing to prune.
        assert_eq!(log.prune(1_790_812_800).unwrap(), 0);
        fs::create_dir_all(log.dir()).unwrap();
        for day in [
            "2026-08-31",
            "2026-09-01",
            "2026-09-02",
            "2026-09-30",
            "2026-10-01",
        ] {
            fs::write(log.dir().join(format!("{day}.jsonl")), "{}\n").unwrap();
        }
        fs::write(log.dir().join("notes.txt"), "kept").unwrap();
        // 2026-10-01: thirty days back to 2026-09-02 stay.
        let now = 1_790_812_800 + 3_600;
        assert_eq!(date(now), "2026-10-01");
        assert_eq!(log.prune(now).unwrap(), 2);
        let mut left: Vec<String> = fs::read_dir(log.dir())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(
            left,
            [
                "2026-09-02.jsonl",
                "2026-09-30.jsonl",
                "2026-10-01.jsonl",
                "notes.txt"
            ]
        );
        // A one-day window keeps only today.
        let today = Log::new(log.dir()).keeping(Some(0));
        assert_eq!(today.keeps(), Some(1));
        assert_eq!(today.prune(now).unwrap(), 2);
    }
}
