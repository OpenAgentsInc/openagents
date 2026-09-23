//! Field meaning in data processing: metamorphic checks on a program that
//! counts records by a field.
//!
//! The public instruction names the output (a CSV and its rows), the input
//! directory and its file-name pattern, the values of the field to count,
//! and a date rule. The observed input shows the record format. From
//! those, the host generates its own inputs:
//!
//! - **message-severity**: the same records twice, the second time with
//!   each message carrying another level's word. Every count must stay the
//!   same; the relation needs no expected numbers.
//! - **date-boundaries**: one file on each side of each period's boundary,
//!   each with its own number of records, so a count that is off names the
//!   date that caused it.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use serde_json::json;

use super::{
    Bounds, Context, Ineligible, Relation, Scenario, Verdict, base_name, days_from_civil, iso_day,
    parse_day,
};
use crate::minitask::process;

/// A period's date rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Window {
    Today,
    /// The last `n` days, today included.
    LastDays(i64),
    MonthToDate,
    Total,
}

impl Window {
    fn parse(period: &str) -> Option<Self> {
        match period {
            "today" => Some(Window::Today),
            "total" => Some(Window::Total),
            "month_to_date" => Some(Window::MonthToDate),
            other => other
                .strip_prefix("last_")
                .and_then(|rest| rest.strip_suffix("_days"))
                .and_then(|n| n.parse().ok())
                .map(Window::LastDays),
        }
    }

    fn holds(&self, day: i64, reference: i64) -> bool {
        match self {
            Window::Today => day == reference,
            Window::LastDays(n) => (0..*n).contains(&(reference - day)),
            Window::MonthToDate => {
                let (ry, rm, _) = super::civil_from_days(reference);
                let (y, m, _) = super::civil_from_days(day);
                y == ry && m == rm && day <= reference
            }
            Window::Total => true,
        }
    }
}

/// What the instruction says about the output and the input.
#[derive(Clone, Debug, PartialEq)]
pub struct Rule {
    pub output: String,
    pub input_dir: String,
    pub pattern: Option<String>,
    pub header: String,
    /// The template rows: period and value.
    pub rows: Vec<(String, String)>,
    pub levels: Vec<String>,
    pub reference: Option<i64>,
    pub inclusive_today: bool,
    /// The program the instruction names, with its argument placeholders.
    pub command: Option<Vec<String>>,
}

fn clean(token: &str) -> String {
    token
        .trim_matches(|c: char| "`'\"(),;:".contains(c))
        .trim_end_matches('.')
        .to_string()
}

/// Reads the rule from the instruction, or says what is missing.
///
/// # Errors
///
/// Returns what the instruction doesn't name.
pub fn rule(instruction: &str) -> Result<Rule, String> {
    let lines: Vec<&str> = instruction.lines().map(str::trim).collect();
    let row = |line: &str| {
        let parts: Vec<&str> = line.split(',').collect();
        (parts.len() == 3
            && parts[2] == "<count>"
            && !parts[0].is_empty()
            && parts[0]
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            && !parts[1].is_empty()
            && parts[1].chars().all(|c| c.is_ascii_uppercase()))
        .then(|| (parts[0].to_string(), parts[1].to_string()))
    };
    let first = lines
        .iter()
        .position(|line| row(line).is_some())
        .ok_or("the instruction shows no CSV rows to fill")?;
    let rows: Vec<(String, String)> = lines[first..].iter().map_while(|l| row(l)).collect();
    let header = if first > 0 && lines[first - 1].contains(',') {
        lines[first - 1].to_string()
    } else {
        return Err("the instruction shows no CSV header".to_string());
    };
    let mut levels: Vec<String> = Vec::new();
    for (_, level) in &rows {
        if !levels.contains(level) {
            levels.push(level.clone());
        }
    }
    let tokens: Vec<String> = instruction.split_whitespace().map(clean).collect();
    let output = tokens
        .iter()
        .find(|t| t.ends_with(".csv"))
        .cloned()
        .ok_or("the instruction names no CSV output")?;
    let pattern = tokens.iter().find(|t| t.contains("YYYY-MM-DD")).cloned();
    let input_dir = tokens
        .iter()
        .find(|t| {
            *t != &output
                && !t.contains("YYYY")
                && (t.ends_with('/') || (t.starts_with('/') && !base_name(t).contains('.')))
                && t.chars().filter(|c| c.is_alphabetic()).count() > 1
        })
        .map(|t| t.trim_end_matches('/').to_string())
        .ok_or("the instruction names no input directory")?;
    let reference = ["current date is", "reference date is", "today is"]
        .iter()
        .find_map(|phrase| {
            let at = instruction.find(phrase)?;
            instruction[at + phrase.len()..]
                .split_whitespace()
                .next()
                .and_then(|word| parse_day(&clean(word)))
        });
    let command = instruction.split('`').skip(1).step_by(2).find_map(|span| {
        let words: Vec<String> = span.split_whitespace().map(str::to_string).collect();
        (words.len() >= 4
            && words[0].starts_with("python")
            && words[1].ends_with(".py")
            && words[2..]
                .iter()
                .all(|w| w.chars().all(|c| c.is_ascii_uppercase() || c == '_')))
        .then_some(words)
    });
    Ok(Rule {
        output,
        input_dir,
        pattern,
        header,
        rows,
        levels,
        reference,
        inclusive_today: instruction.contains("including today"),
        command,
    })
}

/// The record format the samples show: text before the level, how the
/// level is marked, and whether a date leads the line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Format {
    /// The sample the format came from.
    pub sample: String,
    /// The part before the level, with a leading date replaced by
    /// `{DATE}`.
    pub prefix: String,
    /// `[` and `]` for a bracketed level; empty for a bare token.
    pub open: String,
    pub close: String,
    /// Levels the samples show that the output doesn't count.
    pub others: Vec<String>,
}

impl Format {
    fn line(&self, date: &str, level: &str, message: &str) -> String {
        format!(
            "{}{}{level}{} {message}",
            self.prefix.replace("{DATE}", date),
            self.open,
            self.close
        )
    }
}

/// The format of the first sample that marks a level as a field.
#[must_use]
pub fn format(samples: &[String], levels: &[String]) -> Option<Format> {
    let is_date = |word: &str| parse_day(word).is_some();
    let mut others: Vec<String> = Vec::new();
    for sample in samples {
        for word in sample.split_whitespace() {
            let inner = word.trim_start_matches('[').trim_end_matches(']');
            if word.starts_with('[')
                && word.ends_with(']')
                && inner.len() >= 3
                && inner.chars().all(|c| c.is_ascii_uppercase())
                && !levels.iter().any(|l| l == inner)
                && !others.iter().any(|o| o == inner)
            {
                others.push(inner.to_string());
            }
        }
    }
    for sample in samples {
        for level in levels {
            for (open, close) in [("[", "]"), ("", "")] {
                let marked = format!("{open}{level}{close}");
                let Some(at) = sample.find(&format!("{marked} ")) else {
                    continue;
                };
                if open.is_empty() && at > 0 && !sample[..at].ends_with(' ') {
                    continue;
                }
                let prefix = &sample[..at];
                let first = prefix.split_whitespace().next().unwrap_or_default();
                let prefix = if is_date(first) {
                    prefix.replacen(first, "{DATE}", 1)
                } else {
                    prefix.to_string()
                };
                return Some(Format {
                    sample: sample.clone(),
                    prefix,
                    open: open.to_string(),
                    close: close.to_string(),
                    others,
                });
            }
        }
    }
    None
}

/// How the host runs the candidate program.
#[derive(Clone, Debug, PartialEq)]
enum Program {
    /// The instruction's own command, with the input and output as
    /// arguments.
    Argv { file: String, source: String },
    /// A program with the task's paths written in, rebound to scratch
    /// paths by replacing them in its text.
    Rebound { origin: String, source: String },
}

fn program(context: &Context<'_>, rule: &Rule) -> Result<Program, String> {
    if let Some(command) = &rule.command
        && let Some((path, source)) = context.candidate.file_named(base_name(&command[1]))
    {
        return Ok(Program::Argv {
            file: path.clone(),
            source: source.clone(),
        });
    }
    let output = &rule.output;
    let mentions =
        |source: &str| source.contains(output.as_str()) && source.contains(&rule.input_dir);
    if let Some(inline) = context
        .candidate
        .programs
        .iter()
        .rev()
        .find(|p| p.interpreter.starts_with("python") && mentions(&p.source))
    {
        return Ok(Program::Rebound {
            origin: format!("inline `{}` program", inline.interpreter),
            source: inline.source.clone(),
        });
    }
    if let Some((path, source)) = context
        .candidate
        .files
        .iter()
        .find(|(path, source)| path.ends_with(".py") && mentions(source))
    {
        return Ok(Program::Rebound {
            origin: path.clone(),
            source: source.clone(),
        });
    }
    Err(format!(
        "the candidate has no program that writes {output} from {}",
        rule.input_dir
    ))
}

/// Builds the data scenarios that apply.
///
/// # Errors
///
/// Returns why none applies.
pub fn build(context: &Context<'_>) -> Result<Vec<Scenario>, Vec<Ineligible>> {
    let not = |why: String| {
        Err(vec![Ineligible {
            kind: "data".to_string(),
            why,
        }])
    };
    let rule = match rule(&context.task.instruction) {
        Ok(rule) => rule,
        Err(why) => return not(why),
    };
    if rule.levels.len() < 2 {
        return not("the output counts fewer than two field values".to_string());
    }
    let program = match program(context, &rule) {
        Ok(program) => program,
        Err(why) => return not(why),
    };
    let candidate = context.candidate.digest();
    let interface = match &program {
        Program::Argv { file, .. } => format!(
            "{} (the instruction's command, the program {file})",
            rule.command
                .as_ref()
                .map(|c| c.join(" "))
                .unwrap_or_default()
        ),
        Program::Rebound { origin, .. } => format!(
            "{origin}, with {} and {} rebound to scratch paths",
            rule.input_dir, rule.output
        ),
    };
    let mut scenarios = Vec::new();
    let mut ineligible = Vec::new();
    let field = context.requirements_saying(&[&["severity", "level"], &["severity"], &["level"]]);
    match format(&context.observed.samples, &rule.levels) {
        None => ineligible.push(Ineligible {
            kind: "data.message-severity".to_string(),
            why: "no observed record shows the field the output counts".to_string(),
        }),
        Some(_) if field.is_empty() => ineligible.push(Ineligible {
            kind: "data.message-severity".to_string(),
            why: "no requirement names the field".to_string(),
        }),
        Some(format) => {
            let files = message_inputs(&rule, &format);
            scenarios.push(Scenario {
                id: "data.message-severity".to_string(),
                kind: "data.message-severity".to_string(),
                requirements: field.iter().map(|r| r.id.clone()).collect(),
                spans: context.spans_of(&field),
                applies: vec![
                    format!("the instruction shows {} CSV rows over {} values", rule.rows.len(), rule.levels.len()),
                    format!("an observed record marks the value as a field: {:?}", format.sample),
                    format!("the candidate has a program: {interface}"),
                ],
                interface: interface.clone(),
                bounds: Bounds { seconds: 60, processes: 2 },
                effects: vec!["runs the candidate program twice in a scratch directory; writes only there".to_string()],
                candidate: candidate.clone(),
                input: atif::digest(&json!(files)),
                seed: None,
                expected: Relation {
                    statement: "Changing only records' messages, so each carries another value's word, leaves every count unchanged.".to_string(),
                    derivation: "The instruction counts each record's field value; the observed records mark that value as a field separate from the message, so the message's words are not the record's value.".to_string(),
                },
                params: json!({
                    "levels": rule.levels,
                    "other_levels": format.others,
                    "record_format": { "prefix": format.prefix, "open": format.open, "close": format.close, "from_sample": format.sample },
                    "files": files.0.len(),
                    "records_per_file": files.0.first().map_or(0, |(_, lines)| lines.len()),
                    "derived_from": "the instruction's rows and values, and the observed record format",
                }),
            });
        }
    }
    let dated = context.requirements_saying(&[
        &["days"],
        &["month"],
        &["today"],
        &["current date"],
        &["reference date"],
        &["date range"],
    ]);
    let windows: Vec<(String, Window)> = rule
        .rows
        .iter()
        .map(|(period, _)| period.clone())
        .fold(Vec::<String>::new(), |mut seen, p| {
            if !seen.contains(&p) {
                seen.push(p);
            }
            seen
        })
        .into_iter()
        .filter_map(|p| Window::parse(&p).map(|w| (p, w)))
        .collect();
    let format = format(&context.observed.samples, &rule.levels);
    match (rule.reference, &rule.pattern, format) {
        (None, _, _) => ineligible.push(Ineligible {
            kind: "data.date-boundaries".to_string(),
            why: "the instruction states no reference date".to_string(),
        }),
        (_, None, _) => ineligible.push(Ineligible {
            kind: "data.date-boundaries".to_string(),
            why: "the instruction names no dated file pattern".to_string(),
        }),
        (_, _, None) => ineligible.push(Ineligible {
            kind: "data.date-boundaries".to_string(),
            why: "no observed record shows the field the output counts".to_string(),
        }),
        _ if dated.is_empty() || !windows.iter().any(|(_, w)| *w != Window::Total) => {
            ineligible.push(Ineligible {
                kind: "data.date-boundaries".to_string(),
                why: "no requirement states a date rule".to_string(),
            });
        }
        (Some(reference), Some(pattern), Some(format)) => {
            let days = boundary_days(reference, &windows);
            let files = boundary_inputs(&rule, &format, pattern, &days);
            scenarios.push(Scenario {
                id: "data.date-boundaries".to_string(),
                kind: "data.date-boundaries".to_string(),
                requirements: dated.iter().map(|r| r.id.clone()).collect(),
                spans: context.spans_of(&dated),
                applies: vec![
                    format!("the instruction states the reference date {}", iso_day(reference)),
                    format!("the file pattern {pattern} dates each file"),
                    format!("periods: {}", windows.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>().join(", ")),
                ],
                interface,
                bounds: Bounds { seconds: 30, processes: 1 },
                effects: vec!["runs the candidate program once in a scratch directory; writes only there".to_string()],
                candidate,
                input: atif::digest(&json!(files)),
                seed: None,
                expected: Relation {
                    statement: "Each period counts exactly the records of the files its date rule includes, on both sides of each boundary.".to_string(),
                    derivation: format!(
                        "Each period's rule, from the instruction: {}. The host counts its own generated records by that rule.",
                        windows.iter().map(|(p, w)| format!("{p} = {}", describe(w, rule.inclusive_today))).collect::<Vec<_>>().join("; ")
                    ),
                },
                params: json!({
                    "reference": iso_day(reference),
                    "days": days.iter().map(|(d, why)| json!({ "date": iso_day(*d), "age": reference - d, "why": why })).collect::<Vec<_>>(),
                    "inclusive_today": rule.inclusive_today,
                    "derived_from": "the instruction's reference date, periods, and file pattern",
                }),
            });
        }
    }
    if scenarios.is_empty() {
        Err(ineligible)
    } else {
        Ok(scenarios)
    }
}

fn describe(window: &Window, inclusive: bool) -> String {
    match window {
        Window::Today => "the reference date".to_string(),
        Window::LastDays(n) => format!(
            "the {n} days ending on the reference date{}",
            if inclusive {
                ", which the instruction includes"
            } else {
                ""
            }
        ),
        Window::MonthToDate => {
            "the first of the reference month through the reference date".to_string()
        }
        Window::Total => "every file".to_string(),
    }
}

/// The days on both sides of each period's boundary, with why each is
/// there.
fn boundary_days(reference: i64, windows: &[(String, Window)]) -> Vec<(i64, String)> {
    let mut days: Vec<(i64, String)> = vec![(reference, "the reference date".to_string())];
    let mut add = |day: i64, why: String| {
        if !days.iter().any(|(d, _)| *d == day) {
            days.push((day, why));
        }
    };
    for (period, window) in windows {
        match window {
            Window::LastDays(n) => {
                add(reference - (n - 1), format!("the oldest day in {period}"));
                add(reference - n, format!("the first day before {period}"));
            }
            Window::MonthToDate => {
                let (y, m, _) = super::civil_from_days(reference);
                let first = days_from_civil(y, m, 1);
                add(first, format!("the first day in {period}"));
                add(first - 1, format!("the last day before {period}"));
            }
            Window::Today => add(reference - 1, "the day before today".to_string()),
            Window::Total => add(reference - 400, "a day only the total covers".to_string()),
        }
    }
    days.sort_by_key(|(d, _)| std::cmp::Reverse(*d));
    days
}

/// A generated input directory: file name and lines.
type Files = (Vec<(String, Vec<String>)>,);

fn neutral(i: usize) -> String {
    format!("request {i} completed in {} ms", 10 + i)
}

fn file_name(pattern: Option<&String>, date: &str, source: &str) -> String {
    pattern.map_or_else(
        || format!("{date}_{source}.log"),
        |p| {
            p.replace("YYYY-MM-DD", date)
                .replace("<source>", source)
                .rsplit('/')
                .next()
                .unwrap_or_default()
                .to_string()
        },
    )
}

/// The message-severity base input: two files, each with two records of
/// every level, counted or not.
fn message_inputs(rule: &Rule, format: &Format) -> Files {
    let reference = rule
        .reference
        .unwrap_or_else(|| days_from_civil(2025, 1, 15));
    let mut files = Vec::new();
    let mut i = 0;
    for (n, day) in [reference, reference - 1].iter().enumerate() {
        let date = iso_day(*day);
        let mut lines = Vec::new();
        for level in rule.levels.iter().chain(&format.others) {
            for _ in 0..2 {
                i += 1;
                lines.push(format.line(&date, level, &neutral(i)));
            }
        }
        files.push((
            file_name(
                rule.pattern.as_ref(),
                &date,
                if n == 0 { "app" } else { "db" },
            ),
            lines,
        ));
    }
    // The line format keeps the sample's time after the date; the date
    // placeholder carries both.
    (files,)
}

/// The same records with each message carrying another level's word.
fn variant(rule: &Rule, format: &Format, files: &Files) -> Files {
    let all: Vec<&String> = rule.levels.iter().chain(&format.others).collect();
    let counted = &rule.levels;
    let files = files
        .0
        .iter()
        .map(|(name, lines)| {
            let lines = lines
                .iter()
                .enumerate()
                .map(|(i, line)| {
                    // Which level this record is, from its field.
                    let level = all
                        .iter()
                        .find(|l| line.contains(&format!("{}{}{} ", format.open, l, format.close)))
                        .map_or(String::new(), |l| (*l).clone());
                    let other = counted
                        .iter()
                        .cycle()
                        .skip(i)
                        .find(|c| **c != level)
                        .cloned()
                        .unwrap_or_default();
                    let marked = format!("{}{level}{} ", format.open, format.close);
                    match line.find(&marked) {
                        Some(at) => format!(
                            "{}{marked}retry after {other} from upstream (request {i})",
                            &line[..at]
                        ),
                        None => line.clone(),
                    }
                })
                .collect();
            (name.clone(), lines)
        })
        .collect();
    (files,)
}

fn boundary_inputs(rule: &Rule, format: &Format, pattern: &str, days: &[(i64, String)]) -> Files {
    let pattern = pattern.to_string();
    let files = days
        .iter()
        .enumerate()
        .map(|(k, (day, _))| {
            let date = iso_day(*day);
            let weight = k + 1;
            let mut lines = Vec::new();
            let mut i = 0;
            for level in &rule.levels {
                for _ in 0..weight {
                    i += 1;
                    lines.push(format.line(&date, level, &neutral(i)));
                }
            }
            (file_name(Some(&pattern), &date, "app"), lines)
        })
        .collect();
    (files,)
}

/// Runs the candidate on `files` and reads its CSV.
async fn produce(
    rule: &Rule,
    program: &Program,
    files: &Files,
    scratch: &Path,
    label: &str,
) -> Result<BTreeMap<(String, String), i64>, String> {
    let Some(python) = process::python() else {
        return Err("python3 is not on PATH".to_string());
    };
    let dir = scratch.join(label);
    let input = dir.join("input");
    std::fs::create_dir_all(&input).map_err(|e| e.to_string())?;
    for (name, lines) in &files.0 {
        std::fs::write(input.join(name), format!("{}\n", lines.join("\n")))
            .map_err(|e| e.to_string())?;
    }
    let output = dir.join("output.csv");
    let mut command = Command::new(python);
    command
        .current_dir(&dir)
        .env("PYTHONDONTWRITEBYTECODE", "1");
    match program {
        Program::Argv { file, source } => {
            let path = dir.join(base_name(file));
            std::fs::write(&path, source).map_err(|e| e.to_string())?;
            command.arg(&path).arg(&input).arg(&output);
        }
        Program::Rebound { source, .. } => {
            let rebound = source
                .replace(&rule.output, &output.to_string_lossy())
                .replace(&rule.input_dir, &input.to_string_lossy());
            command.arg("-c").arg(rebound);
        }
    }
    let ran = process::run(command, Duration::from_secs(30)).await;
    let text = std::fs::read_to_string(&output).map_err(|_| {
        format!(
            "the program wrote no output (exit {:?}{}): {}",
            ran.code,
            if ran.killed {
                ", killed at its deadline"
            } else {
                ""
            },
            crate::judge::clip(ran.stderr.trim(), 400)
        )
    })?;
    let mut counts = BTreeMap::new();
    for line in text.lines().skip(1) {
        let parts: Vec<&str> = line.trim().split(',').collect();
        if let [period, level, count] = parts.as_slice()
            && let Ok(count) = count.trim().parse::<i64>()
        {
            counts.insert((period.trim().to_string(), level.trim().to_string()), count);
        }
    }
    if counts.is_empty() {
        return Err(format!(
            "the output has no rows: {:?}",
            crate::judge::clip(&text, 200)
        ));
    }
    Ok(counts)
}

/// Runs a data scenario.
pub async fn run(context: &Context<'_>, scenario: &Scenario, scratch: &Path) -> Verdict {
    let (Ok(rule), Some(format)) = (
        rule(&context.task.instruction),
        format(
            &context.observed.samples,
            &rule(&context.task.instruction)
                .map(|r| r.levels)
                .unwrap_or_default(),
        ),
    ) else {
        return Verdict::unavailable(
            &scenario.id,
            "the rule or the record format no longer reads",
        );
    };
    let program = match program(context, &rule) {
        Ok(program) => program,
        Err(why) => return Verdict::unavailable(&scenario.id, &why),
    };
    let mut verdict = Verdict::new(&scenario.id, "passed");
    if let Program::Rebound { origin, .. } = &program {
        verdict.coverage.push(format!(
            "The candidate's paths were rebound by replacing their text in {origin}; a program that builds the path another way reads the task's own directory instead."
        ));
    }
    if scenario.kind == "data.message-severity" {
        let base = message_inputs(&rule, &format);
        let changed = variant(&rule, &format, &base);
        let before = match produce(&rule, &program, &base, scratch, "base").await {
            Ok(counts) => counts,
            Err(why) => {
                return Verdict::unavailable(&scenario.id, &format!("on the base input, {why}"));
            }
        };
        let after = match produce(&rule, &program, &changed, scratch, "variant").await {
            Ok(counts) => counts,
            Err(why) => {
                let mut v = Verdict::new(&scenario.id, "failed");
                v.observations
                    .push(json!({ "base": "produced", "variant": why }));
                v.hypotheses.push(
                    "The program fails on a message holding another value's word.".to_string(),
                );
                return v;
            }
        };
        let mut differ = Vec::new();
        for (period, level) in &rule.rows {
            let key = (period.clone(), level.clone());
            let (a, b) = (before.get(&key), after.get(&key));
            if a != b {
                differ.push(json!({ "row": format!("{period},{level}"), "base": a, "variant": b }));
            }
        }
        verdict
            .observations
            .push(json!({ "rows_compared": rule.rows.len(), "rows_changed": differ.len() }));
        verdict.coverage.push(format!(
            "One record format, from the observed sample {:?}.",
            format.sample
        ));
        verdict.coverage.push("Each changed message carries one other value's word; other message shapes are untested.".to_string());
        if !differ.is_empty() {
            verdict.verdict = "failed".to_string();
            let rose = differ
                .iter()
                .any(|d| d["variant"].as_i64() > d["base"].as_i64());
            verdict.observations.extend(differ);
            verdict.hypotheses.push(if rose {
                "The count reads value words anywhere in the line, not the record's field."
                    .to_string()
            } else {
                "A record whose message holds another value's word is dropped or misread."
                    .to_string()
            });
            verdict
                .hypotheses
                .push("A record is counted once for every value word it contains.".to_string());
        }
        return verdict;
    }
    // Date boundaries.
    let (Some(reference), Some(pattern)) = (rule.reference, rule.pattern.clone()) else {
        return Verdict::unavailable(&scenario.id, "no reference date or file pattern");
    };
    let windows: Vec<(String, Window)> = rule
        .rows
        .iter()
        .filter_map(|(p, _)| Window::parse(p).map(|w| (p.clone(), w)))
        .collect();
    let days = boundary_days(reference, &windows);
    let files = boundary_inputs(&rule, &format, &pattern, &days);
    let got = match produce(&rule, &program, &files, scratch, "boundaries").await {
        Ok(counts) => counts,
        Err(why) => return Verdict::unavailable(&scenario.id, &why),
    };
    let weight = |k: usize| i64::try_from(k + 1).unwrap_or(i64::MAX);
    let mut differ = Vec::new();
    for (period, level) in &rule.rows {
        let Some(window) = Window::parse(period) else {
            continue;
        };
        let want: i64 = days
            .iter()
            .enumerate()
            .filter(|(_, (d, _))| window.holds(*d, reference))
            .map(|(k, _)| weight(k))
            .sum();
        let have = got.get(&(period.clone(), level.clone())).copied();
        if have != Some(want) {
            let delta = have.map(|h| h - want);
            let culprits: Vec<String> = delta
                .map(|delta| {
                    days.iter()
                        .enumerate()
                        .filter(|(k, _)| weight(*k) == delta.abs())
                        .map(|(_, (d, why))| {
                            format!(
                                "{period} {} {} ({}, {} days before the reference date)",
                                if delta > 0 { "includes" } else { "excludes" },
                                iso_day(*d),
                                why,
                                reference - d
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            differ.push((
                json!({ "row": format!("{period},{level}"), "want": want, "got": have }),
                culprits,
            ));
        }
    }
    verdict.observations.push(json!({ "rows_compared": rule.rows.len(), "rows_wrong": differ.len(), "files": days.len() }));
    if !rule.inclusive_today
        && windows
            .iter()
            .any(|(_, w)| matches!(w, Window::LastDays(_)))
    {
        verdict.coverage.push("The instruction doesn't say whether the last days include today; the scenario reads them as including it.".to_string());
    }
    verdict.coverage.push("Dates after the reference date are not generated: the instruction doesn't say which periods hold them.".to_string());
    if !differ.is_empty() {
        verdict.verdict = "failed".to_string();
        for (observation, culprits) in differ {
            verdict.observations.push(observation);
            for culprit in culprits {
                if !verdict.hypotheses.contains(&culprit) {
                    verdict.hypotheses.push(culprit);
                }
            }
        }
        if verdict.hypotheses.is_empty() {
            verdict.hypotheses.push(
                "The period's date rule differs from the instruction's at more than one boundary."
                    .to_string(),
            );
        }
    }
    verdict
}

#[cfg(test)]
mod tests {
    use super::*;

    const TB: &str = "You are given multiple log files stored in /app/logs. Each log file name follows the pattern YYYY-MM-DD_<source>.log (e.g., 2025-08-10_db.log), indicating the date of the logs and their source. Each log line contains an event with a severity level.\nLast 7 days (including today)\n\nWrite a CSV file /app/summary.csv with the following structure (including the header):\nperiod,severity,count\ntoday,ERROR,<count>\ntoday,INFO,<count>\nlast_7_days,ERROR,<count>\nlast_7_days,INFO,<count>\nmonth_to_date,ERROR,<count>\nmonth_to_date,INFO,<count>\n\nThe current date is 2025-08-12. Use this as the reference date for all calculations.";

    #[test]
    fn the_rule_reads_from_public_words_only() {
        let rule = rule(TB).unwrap();
        assert_eq!(rule.output, "/app/summary.csv");
        assert_eq!(rule.input_dir, "/app/logs");
        assert_eq!(rule.pattern.as_deref(), Some("YYYY-MM-DD_<source>.log"));
        assert_eq!(rule.levels, ["ERROR", "INFO"]);
        assert_eq!(rule.reference, parse_day("2025-08-12"));
        assert!(rule.inclusive_today);
        let windows = [
            ("last_7_days".to_string(), Window::LastDays(7)),
            ("month_to_date".to_string(), Window::MonthToDate),
        ];
        let days: Vec<String> = boundary_days(rule.reference.unwrap(), &windows)
            .iter()
            .map(|(d, _)| iso_day(*d))
            .collect();
        assert_eq!(
            days,
            [
                "2025-08-12",
                "2025-08-06",
                "2025-08-05",
                "2025-08-01",
                "2025-07-31"
            ]
        );
    }

    #[test]
    fn the_record_format_comes_from_a_sample() {
        let samples = vec![
            "2025-07-10 01:25:00 [DEBUG] Temporary file created at: report.pdf".to_string(),
            "2025-07-10 23:42:38 [WARNING] Configuration file deprecated: data.csv".to_string(),
        ];
        let levels = vec![
            "ERROR".to_string(),
            "WARNING".to_string(),
            "INFO".to_string(),
        ];
        let found = format(&samples, &levels).unwrap();
        assert_eq!(found.prefix, "{DATE} 23:42:38 ");
        assert_eq!(found.others, ["DEBUG"]);
        assert_eq!(
            found.line("2025-08-12", "INFO", "hello"),
            "2025-08-12 23:42:38 [INFO] hello"
        );
    }
}
