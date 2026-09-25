//! `evidence.data_profile`: code profiles every data file the task ships
//! (issue #9654).
//!
//! In each Fable 5.1 pass on `embedding-drift-monitor`, steps 3 and 4
//! profiled the shipped data files before any edit: shape, dtype, means
//! and deviations, vector norms, zero rows, and missing values. The zero
//! rows in one file were the input that exposes a normalization defect
//! (`docs/coder/design/pattern-components.md`). That is a general pattern
//! and needs no model: this module does it by code.
//!
//! [`profile`] finds the data files by extension and by content
//! ([`classify`]) and profiles each by type, all in Rust, with no new
//! dependency and without running anything:
//!
//! - **NumPy** `.npy`, and the stored members of an `.npz`: dtype and
//!   shape; count, NaN, infinite values, minimum, maximum, mean, and
//!   standard deviation; for a matrix, row L2 norms, all-zero rows,
//!   duplicate rows, constant columns, and per-column statistics
//!   ([`npy`]).
//! - **CSV and TSV**: rows and columns, a header when the first row reads
//!   as one, per-column type, empty fields, and statistics or distinct
//!   values, rows whose field count differs, and duplicate rows; a table
//!   whose columns are all numeric is profiled as a matrix too.
//! - **JSON and JSON Lines**: the top-level structure, records' keys with
//!   their types, missing keys, nulls, empty strings, and statistics, lines
//!   that don't parse, and duplicate records.
//! - **Plain text logs** (`.log`, `.txt`): lines, empty and duplicate
//!   lines, level words such as `ERROR`, and lines that start with a date.
//! - **Parquet**: named with its size; this build has no Parquet reader.
//!
//! Everything is bounded: at most [`Params::max_files`] files,
//! [`Params::read_bytes`] of each, [`Params::total_bytes`] in all, and
//! [`Params::wall_ms`] of time. Each file's profile becomes one evidence
//! item labeled [`LABEL`] and the file's path, which the probe stage puts
//! in the survey, so the coverage packer ranks and trims it with the rest
//! (`crate::pack`, [`crate::pack::Source::Profile`]).

pub mod npy;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;
use std::path::Path;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::record::Implementation;

/// The component's ID.
pub const COMPONENT: &str = "evidence.data_profile";

/// What each evidence item's label starts with, before the file's path.
pub const LABEL: &str = "Data profile of";

/// The note each item's text starts with.
pub const NOTE: &str = "The host profiled this data file by code before the session; nothing \
ran and nothing changed.";

/// Extensions of files that may be data, and what each reads as.
const EXTENSIONS: &[(&str, Kind)] = &[
    ("npy", Kind::Npy),
    ("npz", Kind::Npz),
    ("csv", Kind::Csv),
    ("tsv", Kind::Tsv),
    ("tab", Kind::Tsv),
    ("json", Kind::Json),
    ("jsonl", Kind::Jsonl),
    ("ndjson", Kind::Jsonl),
    ("parquet", Kind::Parquet),
    ("log", Kind::Text),
    ("txt", Kind::Text),
];

/// Extensions whose content is sniffed: a file that may hold any of the
/// kinds above.
const SNIFFED: &[&str] = &["dat", "data", "out", "tbl", ""];

/// Extensions of data formats this build can't read, named rather than
/// skipped in silence.
const UNREAD: &[&str] = &[
    "xlsx", "xls", "h5", "hdf5", "pkl", "pickle", "feather", "arrow", "sqlite", "db", "mat",
    "avro", "orc",
];

/// Rows or records read from one file, at most.
const MAX_ROWS: usize = 200_000;

/// Distinct values one text column tracks, at most.
const MAX_DISTINCT: usize = 10_000;

/// Characters of one shown value or head line, at most.
const VALUE_CHARS: usize = 40;
const LINE_CHARS: usize = 200;

/// The component's tunable bounds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Params {
    /// Data files profiled, at most.
    pub max_files: usize,
    /// Bytes read from one file, at most.
    pub read_bytes: u64,
    /// Bytes read from all files, at most.
    pub total_bytes: u64,
    /// Characters of one file's profile in the briefing, at most.
    pub chars: usize,
    /// Head rows shown per file.
    pub head_rows: usize,
    /// Columns described one by one, at most; the rest are summarized.
    pub columns: usize,
    /// The relevance each item enters the pack with, as a Jev relevance
    /// would.
    pub relevance: f64,
    /// Milliseconds the whole profile may take; files after that are
    /// named, not read.
    pub wall_ms: u64,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            max_files: 24,
            read_bytes: 32 * 1024 * 1024,
            total_bytes: 256 * 1024 * 1024,
            chars: 1_600,
            head_rows: 3,
            columns: 8,
            relevance: 0.8,
            wall_ms: 10_000,
        }
    }
}

impl Params {
    /// Checks the bounds.
    ///
    /// # Errors
    ///
    /// Returns the first problem.
    pub fn validate(&self) -> Result<(), String> {
        if self.max_files == 0 || self.read_bytes == 0 || self.total_bytes == 0 {
            return Err("evidence.data_profile bounds must be at least 1".to_string());
        }
        if self.chars < 200 {
            return Err("evidence.data_profile.chars must be at least 200".to_string());
        }
        if !(0.0..=1.0).contains(&self.relevance) {
            return Err("evidence.data_profile.relevance must be from 0 to 1".to_string());
        }
        Ok(())
    }
}

/// What a data file reads as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Npy,
    Npz,
    Csv,
    Tsv,
    Json,
    Jsonl,
    Parquet,
    Text,
}

impl Kind {
    /// The kind as the briefing names it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Kind::Npy => "NumPy array",
            Kind::Npz => "NumPy archive",
            Kind::Csv => "CSV table",
            Kind::Tsv => "TSV table",
            Kind::Json => "JSON document",
            Kind::Jsonl => "JSON Lines file",
            Kind::Parquet => "Parquet file",
            Kind::Text => "text file",
        }
    }
}

/// One file's profile.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FileProfile {
    /// The path relative to the workspace.
    pub path: String,
    pub kind: Kind,
    pub bytes: u64,
    /// Whether the whole file was read.
    pub whole: bool,
    /// The conditions worth a look: zero rows, NaN, infinite values,
    /// duplicates, empty fields, and the like. Empty when none.
    pub findings: Vec<String>,
    /// The profile as the briefing shows it, before the character bound.
    pub text: String,
    pub ms: u64,
}

/// Every data file's profile.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub files: Vec<FileProfile>,
    /// Data files named and not profiled, with why.
    pub skipped: Vec<(String, String)>,
    pub ms: u64,
}

impl Profile {
    /// The evidence items: one label and text per file, the text within
    /// `params.chars`.
    #[must_use]
    pub fn items(&self, params: &Params) -> Vec<(String, String)> {
        self.files
            .iter()
            .map(|file| {
                let text = if file.text.chars().count() > params.chars {
                    let cut: String = file.text.chars().take(params.chars).collect();
                    let cut = cut.rsplit_once('\n').map_or(cut.as_str(), |(head, _)| head);
                    format!(
                        "{cut}\n[the rest of this profile is cut at {} characters]",
                        params.chars
                    )
                } else {
                    file.text.clone()
                };
                (format!("{LABEL} {}", file.path), text)
            })
            .collect()
    }

    /// The record the probe stage and the component runner keep.
    #[must_use]
    pub fn summary(&self) -> Value {
        json!({
            "files": self.files.iter().map(|f| json!({
                "path": f.path,
                "kind": f.kind,
                "bytes": f.bytes,
                "whole": f.whole,
                "findings": f.findings,
                "ms": f.ms,
            })).collect::<Vec<_>>(),
            "skipped": self.skipped,
            "ms": self.ms,
        })
    }
}

/// The implementation record.
#[must_use]
pub fn implementation(params: &Params) -> Implementation {
    Implementation::new(
        COMPONENT,
        "data files by extension and content, profiled by type in Rust",
        &json!({
            "params": params,
            "extensions": EXTENSIONS.iter().map(|(e, _)| *e).collect::<Vec<_>>(),
            "sniffed": SNIFFED,
            "max_rows": MAX_ROWS,
            "npz": "stored members only",
            "parquet": "named, not read",
        }),
    )
}

fn extension(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.rsplit_once('.')
        .filter(|(stem, _)| !stem.is_empty())
        .map(|(_, e)| e.to_lowercase())
        .unwrap_or_default()
}

/// Whether `head` reads as delimited rows: at least two lines, each with
/// the same positive count of `delimiter` outside quotes.
fn delimited(head: &str, delimiter: u8) -> bool {
    let lines: Vec<&str> = head
        .lines()
        .filter(|l| !l.trim().is_empty())
        .take(6)
        .collect();
    // The last line may be cut short.
    let lines = if lines.len() > 2 {
        &lines[..lines.len() - 1]
    } else {
        &lines[..]
    };
    if lines.len() < 2 {
        return false;
    }
    let counts: Vec<usize> = lines
        .iter()
        .map(|l| split_row(l, delimiter).len() - 1)
        .collect();
    counts[0] > 0 && counts.iter().all(|c| *c == counts[0])
}

/// What a file reads as, by its extension or, for a file that could hold
/// anything, its first bytes. `None` when it isn't a data file.
#[must_use]
pub fn classify(path: &str, head: &[u8]) -> Option<Kind> {
    let name = path.rsplit('/').next().unwrap_or(path);
    if !crate::checks::contract::entry::wide::is_input_name(name) {
        return None;
    }
    let ext = extension(path);
    if head.starts_with(npy::MAGIC) {
        return Some(Kind::Npy);
    }
    if head.starts_with(b"PAR1") {
        return Some(Kind::Parquet);
    }
    if let Some((_, kind)) = EXTENSIONS.iter().find(|(e, _)| *e == ext) {
        return Some(*kind);
    }
    if !SNIFFED.contains(&ext.as_str()) {
        return None;
    }
    let text = std::str::from_utf8(head).ok().or_else(|| {
        // A multibyte character cut at the end of the head.
        std::str::from_utf8(&head[..head.len().saturating_sub(3)]).ok()
    })?;
    let trimmed = text.trim_start();
    if (trimmed.starts_with('{') || trimmed.starts_with('['))
        && serde_json::from_str::<Value>(text).is_ok()
    {
        return Some(Kind::Json);
    }
    let lines: Vec<&str> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .take(4)
        .collect();
    if lines.len() >= 2
        && lines[..lines.len() - 1]
            .iter()
            .all(|l| serde_json::from_str::<Value>(l).is_ok_and(|v| v.is_object()))
    {
        return Some(Kind::Jsonl);
    }
    if delimited(text, b'\t') {
        return Some(Kind::Tsv);
    }
    if delimited(text, b',') {
        return Some(Kind::Csv);
    }
    (ext == "out" || ext == "dat").then_some(Kind::Text)
}

/// A number with about four significant digits.
#[must_use]
pub fn num(x: f64) -> String {
    if x.is_nan() {
        return "NaN".to_string();
    }
    if x.is_infinite() {
        return if x > 0.0 { "inf" } else { "-inf" }.to_string();
    }
    if x == 0.0 {
        return "0".to_string();
    }
    let abs = x.abs();
    if !(1e-3..1e6).contains(&abs) {
        return format!("{x:.3e}");
    }
    if x.fract() == 0.0 && abs < 1e6 {
        return format!("{x:.0}");
    }
    let digits = (3 - abs.log10().floor() as i32).clamp(0, 6) as usize;
    let text = format!("{x:.digits$}");
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        text
    }
}

/// A count with thousands separators.
fn count(n: usize) -> String {
    crate::say::count(n as u64)
}

fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let cut: String = text.chars().take(max).collect();
        format!("{cut}…")
    }
}

/// Running statistics over numbers.
#[derive(Clone, Debug)]
struct Stats {
    finite: usize,
    nan: usize,
    inf: usize,
    zeros: usize,
    min: f64,
    max: f64,
    mean: f64,
    m2: f64,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            finite: 0,
            nan: 0,
            inf: 0,
            zeros: 0,
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
            mean: 0.0,
            m2: 0.0,
        }
    }
}

impl Stats {
    fn add(&mut self, x: f64) {
        if x.is_nan() {
            self.nan += 1;
            return;
        }
        if x.is_infinite() {
            self.inf += 1;
            return;
        }
        if x == 0.0 {
            self.zeros += 1;
        }
        self.finite += 1;
        self.min = self.min.min(x);
        self.max = self.max.max(x);
        let delta = x - self.mean;
        self.mean += delta / self.finite as f64;
        self.m2 += delta * (x - self.mean);
    }

    fn std(&self) -> f64 {
        if self.finite == 0 {
            f64::NAN
        } else {
            (self.m2 / self.finite as f64).sqrt()
        }
    }

    fn describe(&self) -> String {
        if self.finite == 0 {
            return "no finite value".to_string();
        }
        format!(
            "min {}, max {}, mean {}, standard deviation {}",
            num(self.min),
            num(self.max),
            num(self.mean),
            num(self.std())
        )
    }
}

fn indices(list: &[usize], total: usize) -> String {
    let shown: Vec<String> = list.iter().take(5).map(ToString::to_string).collect();
    if total > shown.len() {
        format!("{}, …", shown.join(", "))
    } else {
        shown.join(", ")
    }
}

/// A row-major matrix's profile: the lines it adds and its findings.
fn matrix(
    values: &[f64],
    rows: usize,
    cols: usize,
    params: &Params,
    names: Option<&[String]>,
) -> (Vec<String>, Vec<String>) {
    let mut lines = Vec::new();
    let mut findings = Vec::new();
    let mut overall = Stats::default();
    for v in values {
        overall.add(*v);
    }
    lines.push(format!(
        "Values: {} ({} rows × {} columns); NaN {}; infinite {}; {}",
        count(values.len()),
        count(rows),
        count(cols),
        count(overall.nan),
        count(overall.inf),
        overall.describe()
    ));
    if overall.nan > 0 {
        findings.push(format!("{} NaN values", count(overall.nan)));
    }
    if overall.inf > 0 {
        findings.push(format!("{} infinite values", count(overall.inf)));
    }
    if cols == 0 || rows == 0 {
        return (lines, findings);
    }
    let mut zero_rows = Vec::new();
    let mut nan_rows = 0usize;
    let mut inf_rows = 0usize;
    let mut norms: Vec<f64> = Vec::with_capacity(rows);
    let mut seen: HashSet<Vec<u64>> = HashSet::new();
    let mut duplicates = Vec::new();
    for r in 0..rows {
        let row = &values[r * cols..(r + 1) * cols];
        if row.iter().all(|v| *v == 0.0) {
            zero_rows.push(r);
        }
        if row.iter().any(|v| v.is_nan()) {
            nan_rows += 1;
        } else if row.iter().any(|v| v.is_infinite()) {
            inf_rows += 1;
        } else {
            norms.push(row.iter().map(|v| v * v).sum::<f64>().sqrt());
        }
        if cols > 1 && !seen.insert(row.iter().map(|v| v.to_bits()).collect()) {
            duplicates.push(r);
        }
    }
    if cols > 1 {
        lines.push(format!(
            "Rows: {} all zero{}; {} with NaN; {} with an infinite value; {} duplicate an \
             earlier row{}",
            count(zero_rows.len()),
            if zero_rows.is_empty() {
                String::new()
            } else {
                format!(" (rows {})", indices(&zero_rows, zero_rows.len()))
            },
            count(nan_rows),
            count(inf_rows),
            count(duplicates.len()),
            if duplicates.is_empty() {
                String::new()
            } else {
                format!(" (rows {})", indices(&duplicates, duplicates.len()))
            }
        ));
        if !zero_rows.is_empty() {
            findings.push(format!(
                "{} all-zero rows (rows {})",
                count(zero_rows.len()),
                indices(&zero_rows, zero_rows.len())
            ));
        }
        if !duplicates.is_empty() {
            findings.push(format!("{} duplicate rows", count(duplicates.len())));
        }
        if !norms.is_empty() {
            norms.sort_by(f64::total_cmp);
            let mean = norms.iter().sum::<f64>() / norms.len() as f64;
            lines.push(format!(
                "Row L2 norms: min {}, median {}, mean {}, max {}",
                num(norms[0]),
                num(norms[norms.len() / 2]),
                num(mean),
                num(norms[norms.len() - 1])
            ));
        }
    } else if overall.zeros > 0 {
        lines.push(format!("Zeros: {}", count(overall.zeros)));
    }
    let mut columns: Vec<Stats> = vec![Stats::default(); cols];
    for r in 0..rows {
        for (c, stats) in columns.iter_mut().enumerate() {
            stats.add(values[r * cols + c]);
        }
    }
    if cols > 1 {
        let constant = columns
            .iter()
            .filter(|s| s.finite > 1 && s.min == s.max)
            .count();
        let means: Vec<f64> = columns
            .iter()
            .filter(|s| s.finite > 0)
            .map(|s| s.mean)
            .collect();
        let stds: Vec<f64> = columns
            .iter()
            .filter(|s| s.finite > 0)
            .map(Stats::std)
            .collect();
        let range = |v: &[f64]| {
            let lo = v.iter().copied().fold(f64::INFINITY, f64::min);
            let hi = v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            format!("{} to {}", num(lo), num(hi))
        };
        if !means.is_empty() {
            lines.push(format!(
                "Columns: means from {}; standard deviations from {}; {} constant",
                range(&means),
                range(&stds),
                count(constant)
            ));
        }
        if constant > 0 && rows > 1 {
            findings.push(format!("{} constant columns", count(constant)));
        }
        for (c, stats) in columns.iter().enumerate().take(params.columns) {
            let name = names
                .and_then(|n| n.get(c))
                .map_or_else(|| format!("Column {c}"), |n| format!("Column \"{n}\""));
            lines.push(format!("{name}: {}", stats.describe()));
        }
        if cols > params.columns {
            lines.push(format!(
                "({} more columns, summarized above)",
                count(cols - params.columns)
            ));
        }
    }
    let shown_cols = cols.min(8);
    let head: Vec<String> = (0..rows.min(params.head_rows))
        .map(|r| {
            let row: Vec<String> = values[r * cols..r * cols + shown_cols]
                .iter()
                .map(|v| num(*v))
                .collect();
            format!(
                "  [{}{}]",
                row.join(", "),
                if cols > shown_cols { ", …" } else { "" }
            )
        })
        .collect();
    if !head.is_empty() {
        lines.push(format!(
            "Head (first {} rows{}):\n{}",
            head.len(),
            if cols > shown_cols {
                format!(", first {shown_cols} of {cols} values")
            } else {
                String::new()
            },
            head.join("\n")
        ));
    }
    (lines, findings)
}

/// Profiles an `.npy` file's bytes: header line, lines, and findings.
fn profile_npy(bytes: &[u8], params: &Params, prefix: &str) -> (String, Vec<String>, Vec<String>) {
    let header = match npy::header(bytes) {
        Ok(header) => header,
        Err(error) => return (format!("{prefix}not read: {error}"), Vec::new(), Vec::new()),
    };
    let dtype = npy::dtype(&header.descr).map_or(header.descr.clone(), |(d, _)| d.name().into());
    let head = format!(
        "{prefix}dtype {dtype}, shape {}{}",
        header.shape_text(),
        if header.fortran {
            ", Fortran order"
        } else {
            ""
        }
    );
    match npy::values(bytes, &header) {
        Ok((values, read_rows)) => {
            let rows_total = header.shape.first().copied().unwrap_or(1);
            let cols = if header.shape.len() <= 1 {
                1
            } else {
                header.shape[1..].iter().product()
            };
            let (mut lines, findings) = matrix(&values, read_rows, cols, params, None);
            if header.shape.len() > 2 {
                lines.insert(
                    0,
                    format!(
                        "Each row is the {} values after the first axis, flattened.",
                        count(cols)
                    ),
                );
            }
            if read_rows < rows_total {
                lines.insert(
                    0,
                    format!(
                        "Read the first {} of {} rows (the read bound).",
                        count(read_rows),
                        count(rows_total)
                    ),
                );
            }
            (head, lines, findings)
        }
        Err(error) => (head, vec![format!("Values not read: {error}.")], Vec::new()),
    }
}

/// Splits one delimited row, honoring double quotes.
fn split_row(line: &str, delimiter: u8) -> Vec<String> {
    let mut out = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                field.push(c);
            }
        } else if c == '"' && field.is_empty() {
            quoted = true;
        } else if c as u32 == u32::from(delimiter) {
            out.push(std::mem::take(&mut field));
        } else {
            field.push(c);
        }
    }
    out.push(field);
    out
}

/// A table column or a record key.
#[derive(Clone, Debug, Default)]
struct Column {
    name: String,
    present: usize,
    empty: usize,
    nulls: usize,
    missing: usize,
    numbers: Stats,
    numeric: usize,
    text: usize,
    other: usize,
    distinct: HashMap<String, usize>,
    overflow: bool,
}

impl Column {
    fn named(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Self::default()
        }
    }

    fn add_text(&mut self, value: &str) {
        self.present += 1;
        let trimmed = value.trim();
        if trimmed.is_empty() {
            self.empty += 1;
            return;
        }
        match trimmed.parse::<f64>() {
            Ok(x) => {
                self.numeric += 1;
                self.numbers.add(x);
            }
            Err(_) => {
                self.text += 1;
                self.track(trimmed);
            }
        }
    }

    fn add_json(&mut self, value: &Value) {
        self.present += 1;
        match value {
            Value::Null => self.nulls += 1,
            Value::Number(n) => {
                self.numeric += 1;
                self.numbers.add(n.as_f64().unwrap_or(f64::NAN));
            }
            Value::String(s) if s.trim().is_empty() => self.empty += 1,
            Value::String(s) => {
                self.text += 1;
                self.track(s);
            }
            Value::Bool(b) => {
                self.text += 1;
                self.track(if *b { "true" } else { "false" });
            }
            other => {
                self.other += 1;
                self.track(&clip(&other.to_string(), VALUE_CHARS));
            }
        }
    }

    fn track(&mut self, value: &str) {
        if let Some(n) = self.distinct.get_mut(value) {
            *n += 1;
        } else if self.distinct.len() < MAX_DISTINCT {
            self.distinct.insert(value.to_string(), 1);
        } else {
            self.overflow = true;
        }
    }

    fn is_numeric(&self) -> bool {
        let filled = self.numeric + self.text + self.other;
        filled > 0 && self.numeric * 10 >= filled * 9
    }

    fn describe(&self) -> String {
        let mut out = format!("\"{}\": ", clip(&self.name, VALUE_CHARS));
        let mut notes = Vec::new();
        if self.missing > 0 {
            notes.push(format!("missing in {}", count(self.missing)));
        }
        if self.nulls > 0 {
            notes.push(format!("{} null", count(self.nulls)));
        }
        if self.empty > 0 {
            notes.push(format!("{} empty", count(self.empty)));
        }
        if self.is_numeric() {
            out.push_str(&format!(
                "numeric, {}",
                if self.numbers.nan + self.numbers.inf > 0 {
                    format!(
                        "{}; NaN {}; infinite {}",
                        self.numbers.describe(),
                        self.numbers.nan,
                        self.numbers.inf
                    )
                } else {
                    self.numbers.describe()
                }
            ));
            if self.text + self.other > 0 {
                notes.push(format!("{} not numbers", count(self.text + self.other)));
            }
        } else {
            let mut top: Vec<(&String, &usize)> = self.distinct.iter().collect();
            top.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
            let shown: Vec<String> = top
                .iter()
                .take(3)
                .map(|(v, n)| format!("\"{}\" ×{n}", clip(v, VALUE_CHARS)))
                .collect();
            out.push_str(&format!(
                "{}{} distinct (most common {})",
                if self.numeric > 0 {
                    format!("mixed, {} numbers and ", count(self.numeric))
                } else {
                    "text, ".to_string()
                },
                if self.overflow {
                    format!("over {}", count(MAX_DISTINCT))
                } else {
                    count(self.distinct.len())
                },
                shown.join(", ")
            ));
        }
        if !notes.is_empty() {
            out.push_str(&format!(" ({})", notes.join(", ")));
        }
        out
    }
}

/// Lines and findings for a set of columns over `rows` rows.
fn columns_report(columns: &[Column], params: &Params) -> (Vec<String>, Vec<String>) {
    let mut lines = Vec::new();
    let mut findings = Vec::new();
    let empty: usize = columns.iter().map(|c| c.empty).sum();
    let empty_cols = columns.iter().filter(|c| c.empty > 0).count();
    if empty > 0 {
        findings.push(format!(
            "{} empty fields in {} columns",
            count(empty),
            count(empty_cols)
        ));
    }
    let nulls: usize = columns.iter().map(|c| c.nulls).sum();
    if nulls > 0 {
        findings.push(format!("{} null values", count(nulls)));
    }
    let missing: usize = columns.iter().map(|c| c.missing).sum();
    if missing > 0 {
        findings.push(format!("{} missing keys", count(missing)));
    }
    let nan: usize = columns.iter().map(|c| c.numbers.nan).sum();
    if nan > 0 {
        findings.push(format!("{} NaN values", count(nan)));
    }
    let inf: usize = columns.iter().map(|c| c.numbers.inf).sum();
    if inf > 0 {
        findings.push(format!("{} infinite values", count(inf)));
    }
    for column in columns {
        let filled = column.numeric + column.text + column.other;
        if column.numeric > 0 && !column.is_numeric() && column.numeric * 10 >= filled {
            findings.push(format!(
                "column \"{}\" mixes {} numbers with {} other values",
                clip(&column.name, VALUE_CHARS),
                count(column.numeric),
                count(column.text + column.other)
            ));
        }
    }
    for column in columns.iter().take(params.columns) {
        lines.push(format!("Column {}", column.describe()));
    }
    if columns.len() > params.columns {
        let numeric = columns[params.columns..]
            .iter()
            .filter(|c| c.is_numeric())
            .count();
        lines.push(format!(
            "({} more columns: {} numeric)",
            count(columns.len() - params.columns),
            count(numeric)
        ));
    }
    (lines, findings)
}

fn head_lines(lines: &[&str], params: &Params) -> String {
    lines
        .iter()
        .take(params.head_rows)
        .map(|l| format!("  {}", clip(l, LINE_CHARS)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Profiles a delimited table.
fn profile_table(text: &str, delimiter: u8, params: &Params) -> (String, Vec<String>, Vec<String>) {
    let raw: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let blank = text.lines().count() - raw.len();
    let rows: Vec<Vec<String>> = raw
        .iter()
        .take(MAX_ROWS + 1)
        .map(|l| split_row(l.trim_end_matches('\r'), delimiter))
        .collect();
    if rows.is_empty() {
        return ("empty".to_string(), Vec::new(), Vec::new());
    }
    let numeric = |f: &String| f.trim().parse::<f64>().is_ok();
    let first_numeric = rows[0].iter().any(numeric);
    let later_numeric = rows.iter().skip(1).take(20).any(|r| r.iter().any(numeric));
    let unique: HashSet<&String> = rows[0].iter().collect();
    let header = rows.len() > 1
        && !first_numeric
        && rows[0].iter().all(|f| !f.trim().is_empty())
        && (later_numeric || unique.len() == rows[0].len());
    let width = rows[0].len();
    let names: Vec<String> = if header {
        rows[0].iter().map(|f| f.trim().to_string()).collect()
    } else {
        (1..=width).map(|i| format!("column {i}")).collect()
    };
    let body = &rows[usize::from(header)..];
    let mut columns: Vec<Column> = names.iter().map(|n| Column::named(n)).collect();
    let mut ragged = Vec::new();
    let mut seen: HashSet<&Vec<String>> = HashSet::new();
    let mut duplicates = 0usize;
    for (i, row) in body.iter().enumerate() {
        if row.len() != width {
            ragged.push(i + 1 + usize::from(header));
        }
        if !seen.insert(row) {
            duplicates += 1;
        }
        for (c, column) in columns.iter_mut().enumerate() {
            match row.get(c) {
                Some(value) => column.add_text(value),
                None => column.missing += 1,
            }
        }
    }
    let head = format!(
        "{} rows × {} columns{}",
        count(body.len()),
        count(width),
        if header {
            ", with a header row"
        } else {
            ", no header row"
        }
    );
    let mut lines = Vec::new();
    let mut findings = Vec::new();
    if raw.len() > MAX_ROWS + 1 {
        lines.push(format!(
            "Read the first {} rows (the row bound).",
            count(MAX_ROWS)
        ));
    }
    if !ragged.is_empty() {
        findings.push(format!(
            "{} rows with a field count other than {} (lines {})",
            count(ragged.len()),
            width,
            indices(&ragged, ragged.len())
        ));
    }
    if duplicates > 0 {
        findings.push(format!("{} duplicate rows", count(duplicates)));
    }
    if blank > 0 {
        lines.push(format!("Blank lines: {}", count(blank)));
    }
    let (column_lines, column_findings) = columns_report(&columns, params);
    findings.extend(column_findings);
    // A table of numbers is a matrix too: rows' norms and zero rows.
    if width > 1 && columns.iter().all(Column::is_numeric) && ragged.is_empty() {
        let values: Vec<f64> = body
            .iter()
            .flat_map(|r| {
                r.iter()
                    .map(|f| f.trim().parse::<f64>().unwrap_or(f64::NAN))
            })
            .collect();
        let (matrix_lines, matrix_findings) =
            matrix(&values, body.len(), width, params, Some(&names));
        // The matrix's value and column lines repeat the columns'; keep its
        // rows and norms.
        lines.extend(
            matrix_lines
                .into_iter()
                .filter(|l| l.starts_with("Rows:") || l.starts_with("Row L2")),
        );
        findings.extend(
            matrix_findings
                .into_iter()
                .filter(|f| f.contains("all-zero") || f.contains("constant")),
        );
    }
    lines.extend(column_lines);
    lines.push(format!("Head:\n{}", head_lines(&raw, params)));
    (head, lines, findings)
}

/// Profiles records: JSON objects, by key.
fn profile_records(
    records: &[&serde_json::Map<String, Value>],
    params: &Params,
) -> (Vec<String>, Vec<String>) {
    let mut order: Vec<String> = Vec::new();
    let mut columns: BTreeMap<String, Column> = BTreeMap::new();
    for record in records {
        for key in record.keys() {
            if !columns.contains_key(key) {
                order.push(key.clone());
                columns.insert(key.clone(), Column::named(key));
            }
        }
    }
    let mut seen: HashSet<String> = HashSet::new();
    let mut duplicates = 0usize;
    for record in records {
        for (key, column) in &mut columns {
            match record.get(key) {
                Some(value) => column.add_json(value),
                None => column.missing += 1,
            }
        }
        if !seen.insert(Value::Object((*record).clone()).to_string()) {
            duplicates += 1;
        }
    }
    let ordered: Vec<Column> = order.iter().filter_map(|k| columns.remove(k)).collect();
    let (mut lines, mut findings) = columns_report(&ordered, params);
    lines.insert(0, format!("Keys: {}", count(ordered.len())));
    if duplicates > 0 {
        findings.push(format!("{} duplicate records", count(duplicates)));
    }
    (lines, findings)
}

fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Nulls and empty strings anywhere in a document, visiting at most a
/// million values.
fn blanks(value: &Value) -> (usize, usize) {
    let mut stack = vec![value];
    let (mut nulls, mut empty, mut visited) = (0, 0, 0usize);
    while let Some(v) = stack.pop() {
        visited += 1;
        if visited > 1_000_000 {
            break;
        }
        match v {
            Value::Null => nulls += 1,
            Value::String(s) if s.trim().is_empty() => empty += 1,
            Value::Array(items) => stack.extend(items.iter()),
            Value::Object(map) => stack.extend(map.values()),
            _ => {}
        }
    }
    (nulls, empty)
}

/// Profiles an array's items: records, a matrix, a vector, or a mix.
fn profile_array(items: &[Value], params: &Params) -> (Vec<String>, Vec<String>) {
    let items = &items[..items.len().min(MAX_ROWS)];
    if !items.is_empty() && items.iter().all(Value::is_object) {
        let records: Vec<&serde_json::Map<String, Value>> =
            items.iter().filter_map(Value::as_object).collect();
        return profile_records(&records, params);
    }
    if !items.is_empty() && items.iter().all(Value::is_number) {
        let values: Vec<f64> = items.iter().filter_map(Value::as_f64).collect();
        return matrix(&values, values.len(), 1, params, None);
    }
    let width = items.first().and_then(Value::as_array).map(Vec::len);
    if let Some(width) = width
        && width > 0
        && items.iter().all(|i| {
            i.as_array()
                .is_some_and(|r| r.len() == width && r.iter().all(Value::is_number))
        })
    {
        let values: Vec<f64> = items
            .iter()
            .flat_map(|r| r.as_array().into_iter().flatten().filter_map(Value::as_f64))
            .collect();
        return matrix(&values, items.len(), width, params, None);
    }
    let mut kinds: BTreeMap<&str, usize> = BTreeMap::new();
    for item in items {
        *kinds.entry(type_name(item)).or_default() += 1;
    }
    (
        vec![format!(
            "Items by type: {}",
            kinds
                .iter()
                .map(|(k, n)| format!("{k} {n}"))
                .collect::<Vec<_>>()
                .join(", ")
        )],
        Vec::new(),
    )
}

/// Profiles a JSON document.
fn profile_json(text: &str, params: &Params) -> (String, Vec<String>, Vec<String>) {
    let value: Value = match serde_json::from_str(text) {
        Ok(value) => value,
        Err(error) => {
            return (
                "doesn't parse as JSON".to_string(),
                Vec::new(),
                vec![format!("invalid JSON: {error}")],
            );
        }
    };
    let (nulls, empty) = blanks(&value);
    let (head, mut lines, mut findings) = match &value {
        Value::Array(items) => {
            let (lines, findings) = profile_array(items, params);
            let shown: Vec<String> = items
                .iter()
                .take(params.head_rows)
                .map(|i| format!("  {}", clip(&i.to_string(), LINE_CHARS)))
                .collect();
            let mut lines = lines;
            if !shown.is_empty() {
                lines.push(format!("Head:\n{}", shown.join("\n")));
            }
            (
                format!("an array of {} items", count(items.len())),
                lines,
                findings,
            )
        }
        Value::Object(map) => {
            let mut lines = vec![format!(
                "Top-level keys: {}",
                map.iter()
                    .take(20)
                    .map(|(k, v)| match v {
                        Value::Array(a) =>
                            format!("{} (array of {})", clip(k, VALUE_CHARS), a.len()),
                        Value::Object(o) =>
                            format!("{} (object of {} keys)", clip(k, VALUE_CHARS), o.len()),
                        other => format!("{} ({})", clip(k, VALUE_CHARS), type_name(other)),
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            )];
            if map.len() > 20 {
                lines.push(format!("({} more keys)", count(map.len() - 20)));
            }
            let mut findings = Vec::new();
            // The largest array under a top-level key, profiled as the
            // document's data.
            if let Some((key, items)) = map
                .iter()
                .filter_map(|(k, v)| v.as_array().map(|a| (k, a)))
                .filter(|(_, a)| a.len() > 1)
                .max_by_key(|(_, a)| a.len())
            {
                let (inner_lines, inner_findings) = profile_array(items, params);
                lines.push(format!(
                    "Under \"{}\" ({} items):",
                    clip(key, VALUE_CHARS),
                    count(items.len())
                ));
                lines.extend(inner_lines);
                findings.extend(
                    inner_findings
                        .into_iter()
                        .map(|f| format!("{f} under \"{}\"", clip(key, VALUE_CHARS))),
                );
            }
            (
                format!("an object with {} keys", count(map.len())),
                lines,
                findings,
            )
        }
        other => (
            format!("a single {}", type_name(other)),
            Vec::new(),
            Vec::new(),
        ),
    };
    if !findings.iter().any(|f| f.contains("null")) && nulls > 0 {
        findings.push(format!("{} null values", count(nulls)));
    }
    if empty > 0 && !findings.iter().any(|f| f.contains("empty")) {
        findings.push(format!("{} empty strings", count(empty)));
    }
    lines.retain(|l| !l.is_empty());
    (
        head,
        std::mem::take(&mut lines),
        std::mem::take(&mut findings),
    )
}

/// Profiles a JSON Lines file.
fn profile_jsonl(text: &str, params: &Params) -> (String, Vec<String>, Vec<String>) {
    let mut values = Vec::new();
    let mut invalid = Vec::new();
    let mut blank = 0usize;
    let raw: Vec<&str> = text.lines().collect();
    for (i, line) in raw.iter().enumerate().take(MAX_ROWS) {
        if line.trim().is_empty() {
            blank += 1;
            continue;
        }
        match serde_json::from_str::<Value>(line) {
            Ok(value) => values.push(value),
            Err(_) => invalid.push(i + 1),
        }
    }
    let mut findings = Vec::new();
    if !invalid.is_empty() {
        findings.push(format!(
            "{} lines that don't parse as JSON (lines {})",
            count(invalid.len()),
            indices(&invalid, invalid.len())
        ));
    }
    let (mut lines, inner) = profile_array(&values, params);
    findings.extend(inner);
    if blank > 0 {
        lines.push(format!("Blank lines: {}", count(blank)));
    }
    let shown: Vec<&str> = raw
        .iter()
        .filter(|l| !l.trim().is_empty())
        .copied()
        .collect();
    lines.push(format!("Head:\n{}", head_lines(&shown, params)));
    (format!("{} records", count(values.len())), lines, findings)
}

/// Profiles a plain text log.
fn profile_text(text: &str, params: &Params) -> (String, Vec<String>, Vec<String>) {
    let lines_all: Vec<&str> = text.lines().collect();
    let empty = lines_all.iter().filter(|l| l.trim().is_empty()).count();
    let mut seen: HashSet<&str> = HashSet::new();
    let duplicates = lines_all
        .iter()
        .filter(|l| !l.trim().is_empty() && !seen.insert(l))
        .count();
    let longest = lines_all
        .iter()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(0);
    let level =
        regex::Regex::new(r"\b(TRACE|DEBUG|INFO|NOTICE|WARN|WARNING|ERROR|CRITICAL|FATAL)\b")
            .expect("a valid pattern");
    let mut levels: BTreeMap<String, usize> = BTreeMap::new();
    for line in &lines_all {
        if let Some(c) = level.captures(line) {
            *levels.entry(c[1].to_string()).or_default() += 1;
        }
    }
    let dated = regex::Regex::new(r"^\s*\[?\d{4}-\d{2}-\d{2}").expect("a valid pattern");
    let dated_lines = lines_all.iter().filter(|l| dated.is_match(l)).count();
    let mut out = vec![format!(
        "Empty lines: {}; duplicate lines: {}; longest line: {} characters",
        count(empty),
        count(duplicates),
        count(longest)
    )];
    if !levels.is_empty() {
        out.push(format!(
            "Level words: {}",
            levels
                .iter()
                .map(|(k, n)| format!("{k} {n}"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if dated_lines > 0 {
        out.push(format!(
            "Lines that start with a date: {}",
            count(dated_lines)
        ));
    }
    let shown: Vec<&str> = lines_all
        .iter()
        .filter(|l| !l.trim().is_empty())
        .copied()
        .collect();
    out.push(format!("Head:\n{}", head_lines(&shown, params)));
    (format!("{} lines", count(lines_all.len())), out, Vec::new())
}

/// Profiles one file already read.
fn profile_bytes(kind: Kind, bytes: &[u8], params: &Params) -> (String, Vec<String>, Vec<String>) {
    let text = || String::from_utf8_lossy(bytes).into_owned();
    match kind {
        Kind::Npy => profile_npy(bytes, params, ""),
        Kind::Npz => match npy::members(bytes) {
            Err(error) => (format!("not read: {error}"), Vec::new(), Vec::new()),
            Ok(members) => {
                let mut lines = Vec::new();
                let mut findings = Vec::new();
                for member in &members {
                    match &member.stored {
                        Some(range) => {
                            let (head, inner, inner_findings) =
                                profile_npy(&bytes[range.clone()], params, "");
                            lines.push(format!("Member {}: {head}", member.name));
                            lines.extend(inner);
                            findings.extend(
                                inner_findings
                                    .into_iter()
                                    .map(|f| format!("{f} in {}", member.name)),
                            );
                        }
                        None => lines.push(format!(
                            "Member {}: {} bytes, compressed, not read (no decompressor in this \
                             build)",
                            member.name,
                            count(usize::try_from(member.size).unwrap_or(usize::MAX))
                        )),
                    }
                }
                (format!("{} members", members.len()), lines, findings)
            }
        },
        Kind::Csv => profile_table(&text(), b',', params),
        Kind::Tsv => profile_table(&text(), b'\t', params),
        Kind::Json => profile_json(&text(), params),
        Kind::Jsonl => profile_jsonl(&text(), params),
        Kind::Parquet => (
            "not read: this build has no Parquet reader".to_string(),
            Vec::new(),
            Vec::new(),
        ),
        Kind::Text => profile_text(&text(), params),
    }
}

fn read_head(path: &Path, max: u64) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let mut out = Vec::new();
    std::fs::File::open(path)?.take(max).read_to_end(&mut out)?;
    Ok(out)
}

/// Profiles one file of the workspace at `root`. `None` when it isn't a
/// data file.
fn profile_file(
    root: &Path,
    path: &str,
    params: &Params,
    budget: &mut u64,
) -> Option<Result<FileProfile, String>> {
    let full = root.join(path);
    let bytes = std::fs::metadata(&full).ok()?.len();
    let sniff = read_head(&full, 4096).ok()?;
    let kind = classify(path, &sniff)?;
    let started = Instant::now();
    let limit = params.read_bytes.min(*budget);
    if kind == Kind::Npz && bytes > limit {
        return Some(Err(format!(
            "{} bytes, over the {}-byte read bound, and an archive must be read whole",
            count(usize::try_from(bytes).unwrap_or(usize::MAX)),
            count(usize::try_from(limit).unwrap_or(usize::MAX))
        )));
    }
    if limit == 0 {
        return Some(Err("the total read bound is spent".to_string()));
    }
    let data = match read_head(&full, limit) {
        Ok(data) => data,
        Err(error) => return Some(Err(error.to_string())),
    };
    *budget = budget.saturating_sub(data.len() as u64);
    let whole = data.len() as u64 >= bytes;
    // A text file cut at the bound ends at its last whole line.
    let data: &[u8] = if whole || matches!(kind, Kind::Npy | Kind::Npz | Kind::Parquet) {
        &data
    } else {
        let end = data
            .iter()
            .rposition(|b| *b == b'\n')
            .map_or(data.len(), |i| i + 1);
        &data[..end]
    };
    let (head, lines, findings) = profile_bytes(kind, data, params);
    let mut text = format!(
        "{NOTE}\n{path}: {}, {} bytes{}; {head}",
        kind.name(),
        count(usize::try_from(bytes).unwrap_or(usize::MAX)),
        if whole {
            String::new()
        } else {
            format!(
                ", first {} bytes read",
                count(usize::try_from(limit).unwrap_or(usize::MAX))
            )
        }
    );
    if findings.is_empty() {
        let _ = write!(
            text,
            "\nNotable: none of all-zero rows, NaN or infinite values, duplicates, or empty \
             fields"
        );
    } else {
        let _ = write!(text, "\nNotable: {}", findings.join("; "));
    }
    for line in &lines {
        text.push('\n');
        text.push_str(line);
    }
    Some(Ok(FileProfile {
        path: path.to_string(),
        kind,
        bytes,
        whole,
        findings,
        text,
        ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    }))
}

/// Profiles every data file in the workspace at `root`, in path order,
/// within `params`' bounds.
#[must_use]
pub fn profile(root: &Path, params: &Params) -> Profile {
    let started = Instant::now();
    let deadline = Duration::from_millis(params.wall_ms);
    let mut out = Profile::default();
    let mut budget = params.total_bytes;
    for path in crate::micro::parallel::workspace_files(root) {
        if path.split('/').any(|part| part.starts_with('.')) {
            continue;
        }
        let ext = extension(&path);
        if UNREAD.contains(&ext.as_str()) {
            out.skipped
                .push((path, format!("no reader for .{ext} in this build")));
            continue;
        }
        if out.files.len() >= params.max_files || started.elapsed() >= deadline {
            let name = path.rsplit('/').next().unwrap_or(&path);
            let known = EXTENSIONS.iter().any(|(e, _)| *e == ext);
            if known && crate::checks::contract::entry::wide::is_input_name(name) {
                out.skipped.push((
                    path,
                    if out.files.len() >= params.max_files {
                        format!("over the {}-file bound", params.max_files)
                    } else {
                        format!("over the {} ms bound", params.wall_ms)
                    },
                ));
            }
            continue;
        }
        match profile_file(root, &path, params, &mut budget) {
            None => {}
            Some(Ok(file)) => out.files.push(file),
            Some(Err(why)) => out.skipped.push((path, why)),
        }
    }
    out.ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    out
}
