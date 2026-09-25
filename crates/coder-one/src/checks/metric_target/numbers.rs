//! The numbers an instruction states, found by code (issue #9657).
//!
//! [`numbers`] reads each sentence of an instruction and finds its numbers
//! in four forms, in this order, so that a longer form claims its text
//! before a shorter one can:
//!
//! 1. Powers of ten: `1.5 \times 10^{-3}`, `10^{-7}`, `10^-7`, and `10⁻⁷`.
//! 2. Ranges: `5–10 seconds`, `10-20%`, and `2 to 4 hours`. Each end is its
//!    own number, which names the range it ends.
//! 3. Digits: `2.6x`, `$1,200`, `1e-7`, `98 %`, `55-second`, and `3
//!    million`.
//! 4. Words: `zero` through `nineteen`, the tens (`twenty-five`), and `a
//!    hundred`, `two thousand`, or `a million`. A bare `one` is a number
//!    only after a bound such as `at most`, because it's more often a
//!    pronoun.
//!
//! A number that continues a word, an identifier, a path, or a version
//! isn't one; nor is an ordinal (`95th`) or anything in an HTML comment.
//!
//! The host's own time limit is a known fact, not the task's goal. A
//! harness that appends its deadline to every instruction states that
//! limit, and a sentence whose every number restates it
//! ([`restates_host_limit`]) is left out. The rule matches on the value
//! the host passes in, never on the sentence's wording.

use std::sync::OnceLock;
use std::time::Duration;

use regex::Regex;

use super::{Candidate, MAX_CANDIDATES};

/// How far a stated time may be from the host's limit, as a share of it,
/// and still restate it: the host keeps its own deadline a little inside
/// the harness's.
pub const HOST_LIMIT_SLACK: f64 = 0.05;

/// The least slack, in seconds, for a short limit.
pub const HOST_LIMIT_SLACK_SEC: f64 = 300.0;

/// The sentences of `text`: split at line breaks and at a `.`, `!`, or `?`
/// followed by white space.
#[must_use]
pub fn sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = text.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c == '\n' {
            if !current.trim().is_empty() {
                out.push(current.trim().to_string());
            }
            current.clear();
            continue;
        }
        current.push(c);
        if matches!(c, '.' | '!' | '?') && chars.get(i + 1).is_none_or(|n| n.is_whitespace()) {
            if !current.trim().is_empty() {
                out.push(current.trim().to_string());
            }
            current.clear();
        }
    }
    if !current.trim().is_empty() {
        out.push(current.trim().to_string());
    }
    out
}

const UNIT: &str = r"(?:\s*|-)(?P<unit>[xX×%]|[A-Za-zµμ]+(?:/[A-Za-z]+)?)";

const SMALL: &str = "(?:twenty|thirty|forty|fifty|sixty|seventy|eighty|ninety)\
(?:[-\\s](?:one|two|three|four|five|six|seven|eight|nine)\\b)?\
|zero|one|two|three|four|five|six|seven|eight|nine|ten|eleven|twelve|thirteen\
|fourteen|fifteen|sixteen|seventeen|eighteen|nineteen";

fn compile(pattern: &str) -> Regex {
    Regex::new(pattern).expect("a number pattern compiles")
}

fn digit_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        compile(&format!(
            r"(?P<cur>[$€£])?(?P<num>\d+(?:,\d{{3}})*(?:\.\d+)?(?:[eE][-+]?\d+)?)(?:{UNIT})?"
        ))
    })
}

fn power_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        let exponent = |a: &str, b: &str| {
            format!(r"10\s*\^\s*(?:\{{\s*(?P<{a}>[-+−]?\s*\d+)\s*\}}|(?P<{b}>[-+−]?\d+))")
        };
        compile(&format!(
            r"(?:(?P<mant>\d+(?:\.\d+)?)\s*(?:\\times|\\cdot|×|·|\*|x)\s*{}|(?:(?:\\times|\\cdot)\s*)?{}|10(?P<sup>[⁻⁺]?[⁰¹²³⁴⁵⁶⁷⁸⁹]+))(?:\s*(?P<unit>[xX×%]|[A-Za-zµμ]+))?",
            exponent("e1", "e2"),
            exponent("e3", "e4"),
        ))
    })
}

fn range_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        compile(&format!(
            r"(?P<cur>[$€£])?(?P<a>\d+(?:\.\d+)?)(?P<ua>\s?%|[xX×])?(?:\s*[-–—]\s*|\s+to\s+)[$€£]?(?P<b>\d+(?:\.\d+)?)(?:{UNIT})?"
        ))
    })
}

fn word_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        compile(&format!(
            r"(?i)\b(?:(?:(?P<lead>an?|{SMALL})[\s-]+)?(?P<scale>hundred|thousand|million|billion)(?:\s+(?P<scale2>thousand|million|billion))?|(?P<small>{SMALL}))\b(?:{UNIT})?"
        ))
    })
}

fn comment_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| compile(r"(?s)<!--.*?-->"))
}

/// A number word's value: `zero` through `nineteen`, the tens, and the
/// tens with a digit, such as `twenty-five`.
fn small_value(word: &str) -> Option<f64> {
    const ONES: [&str; 20] = [
        "zero",
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
    ];
    const TENS: [&str; 8] = [
        "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
    ];
    let lower = word.to_lowercase();
    let mut parts = lower.split(|c: char| c == '-' || c.is_whitespace());
    let first = parts.next()?;
    let second = parts.find(|p| !p.is_empty());
    if let Some(k) = TENS.iter().position(|t| *t == first) {
        let tens = 20.0 + 10.0 * k as f64;
        return match second {
            None => Some(tens),
            Some(one) => ONES[1..10]
                .iter()
                .position(|o| *o == one)
                .map(|u| tens + (u + 1) as f64),
        };
    }
    if second.is_some() {
        return None;
    }
    ONES.iter().position(|o| *o == first).map(|u| u as f64)
}

fn scale_value(word: &str) -> Option<f64> {
    match word.to_lowercase().as_str() {
        "hundred" => Some(1e2),
        "thousand" | "k" => Some(1e3),
        "million" => Some(1e6),
        "billion" => Some(1e9),
        _ => None,
    }
}

/// Words that follow a number without being its unit.
fn stop_word(word: &str) -> bool {
    matches!(
        word.to_lowercase().as_str(),
        "a" | "an"
            | "and"
            | "are"
            | "as"
            | "at"
            | "be"
            | "by"
            | "each"
            | "for"
            | "from"
            | "in"
            | "into"
            | "is"
            | "it"
            | "must"
            | "no"
            | "not"
            | "of"
            | "on"
            | "or"
            | "per"
            | "should"
            | "than"
            | "that"
            | "the"
            | "then"
            | "this"
            | "to"
            | "was"
            | "will"
            | "with"
            | "within"
    )
}

fn normalize_unit(word: &str) -> String {
    let lower = word.to_lowercase();
    // A time per call, such as `s/call`, is a time; a rate such as `MB/s`
    // stays whole.
    if let Some((top, bottom)) = lower.split_once('/') {
        let top = normalize_unit(top);
        let timed = |u: &str| matches!(u, "s" | "ms" | "us" | "min" | "h");
        return if timed(&top) && !timed(&normalize_unit(bottom)) {
            top
        } else {
            lower
        };
    }
    match lower.as_str() {
        "x" | "×" | "times" | "fold" => "x".to_string(),
        "%" | "percent" | "pct" => "%".to_string(),
        "s" | "sec" | "secs" | "second" | "seconds" => "s".to_string(),
        "ms" | "millisecond" | "milliseconds" => "ms".to_string(),
        "us" | "µs" | "μs" | "microsecond" | "microseconds" => "us".to_string(),
        "min" | "mins" | "minute" | "minutes" => "min".to_string(),
        "h" | "hr" | "hrs" | "hour" | "hours" => "h".to_string(),
        "usd" | "dollar" | "dollars" => "USD".to_string(),
        _ => lower,
    }
}

fn ordinal(unit: &str) -> bool {
    matches!(unit.to_lowercase().as_str(), "st" | "nd" | "rd" | "th")
}

/// A signed exponent as written: `-7`, `- 7`, `−7`, or `⁻⁷`.
fn exponent(text: &str) -> Option<i32> {
    let mut digits = String::new();
    for c in text.chars() {
        match c {
            '-' | '−' | '⁻' => digits.push('-'),
            '0'..='9' => digits.push(c),
            '⁰' => digits.push('0'),
            '¹' => digits.push('1'),
            '²' => digits.push('2'),
            '³' => digits.push('3'),
            '⁴'..='⁹' => digits.push(char::from(b'4' + (c as u32 - '⁴' as u32) as u8)),
            _ => {}
        }
    }
    digits.parse().ok()
}

/// A number and the unit that follows it, from one match.
struct Reading {
    value: f64,
    /// The unit, normalized; empty for none.
    unit: String,
    /// Where the unit word starts, when it was kept.
    unit_start: Option<usize>,
}

/// A unit word as a match found it: dropped when it's a stop word, and
/// folded into the value when it's a scale such as `million`.
fn with_unit(value: f64, unit: Option<regex::Match<'_>>) -> Reading {
    let Some(unit) = unit else {
        return Reading {
            value,
            unit: String::new(),
            unit_start: None,
        };
    };
    let word = unit.as_str();
    if stop_word(word) || word.chars().count() > 12 {
        return Reading {
            value,
            unit: String::new(),
            unit_start: None,
        };
    }
    if let Some(scale) = scale_value(word) {
        return Reading {
            value: value * scale,
            unit: String::new(),
            unit_start: Some(unit.start()),
        };
    }
    Reading {
        value,
        unit: normalize_unit(word),
        unit_start: Some(unit.start()),
    }
}

fn read_power(found: &regex::Captures<'_>) -> Option<Reading> {
    let exp = ["e1", "e2", "e3", "e4", "sup"]
        .iter()
        .find_map(|name| found.name(name))
        .and_then(|m| exponent(m.as_str()))?;
    let mantissa: f64 = found
        .name("mant")
        .map_or(Ok(1.0), |m| m.as_str().parse())
        .ok()?;
    Some(with_unit(mantissa * 10f64.powi(exp), found.name("unit")))
}

fn read_digits(found: &regex::Captures<'_>, after: &str) -> Option<Reading> {
    let value: f64 = found["num"].replace(',', "").parse().ok()?;
    if found.name("unit").is_some_and(|u| ordinal(u.as_str())) {
        return None;
    }
    // `$5$` is LaTeX math, not money.
    let money = found.name("cur").is_some() && !after.trim_start().starts_with('$');
    let mut reading = with_unit(value, found.name("unit"));
    if money {
        reading.unit = "USD".to_string();
    }
    Some(reading)
}

fn read_words(found: &regex::Captures<'_>) -> Option<Reading> {
    let value = if let Some(scale) = found.name("scale") {
        let lead = match found.name("lead").map(|m| m.as_str().to_lowercase()) {
            None => 1.0,
            Some(word) if word == "a" || word == "an" => 1.0,
            Some(word) => small_value(&word)?,
        };
        let second = found
            .name("scale2")
            .and_then(|m| scale_value(m.as_str()))
            .unwrap_or(1.0);
        lead * scale_value(scale.as_str())? * second
    } else {
        small_value(found.name("small")?.as_str())?
    };
    Some(with_unit(value, found.name("unit")))
}

/// A number token's value and its normalized unit: `2.6x` is `(2.6,
/// "x")`, `5 seconds` is `(5, "s")`, `$1,200` is `(1200, "USD")`,
/// `10^{-7}` is `(1e-7, "")`, and `zero` is `(0, "")`.
#[must_use]
pub fn parse_number(token: &str) -> Option<(f64, String)> {
    let token = token.trim();
    let whole = |m: regex::Match<'_>| m.start() == 0;
    if let Some(found) = power_pattern()
        .captures(token)
        .filter(|f| f.get(0).is_some_and(whole))
        && let Some(reading) = read_power(&found)
    {
        return Some((reading.value, reading.unit));
    }
    if let Some(found) = digit_pattern()
        .captures(token)
        .filter(|f| f.get(0).is_some_and(whole))
    {
        let after = &token[found.get(0)?.end()..];
        return read_digits(&found, after).map(|r| (r.value, r.unit));
    }
    let found = word_pattern()
        .captures(token)
        .filter(|f| f.get(0).is_some_and(whole))?;
    read_words(&found).map(|r| (r.value, r.unit))
}

/// Whether the text before a number ends inside a word, a path, or a
/// version, so the number continues it. A LaTeX command such as `\le`
/// ends in letters and doesn't.
fn glued(before: &str) -> bool {
    let Some(c) = before.chars().last() else {
        return false;
    };
    if !(c.is_alphanumeric() || "_./-:".contains(c)) {
        return false;
    }
    let letters = before
        .chars()
        .rev()
        .take_while(char::is_ascii_alphabetic)
        .count();
    let rest = &before[..before.len() - letters];
    !(letters > 0 && rest.ends_with('\\'))
}

/// Whether the text after a number continues it as an identifier, a
/// version, or a path: `26b5c`, `1.2.3`, or `3.11/bin`.
fn continued(after: &str) -> bool {
    let mut chars = after.chars();
    match chars.next() {
        Some(c) if c.is_alphanumeric() || c == '_' => true,
        Some(c) if "._/".contains(c) => chars.next().is_some_and(char::is_alphanumeric),
        _ => false,
    }
}

/// Whether the text before a bare `one` makes it a bound's number, as in
/// `at most one`.
fn bounded(before: &str) -> bool {
    let lower = before.trim_end().to_lowercase();
    if lower.ends_with(['≤', '≥', '<', '>', '=']) {
        return true;
    }
    let last = lower
        .rsplit(|c: char| !c.is_alphanumeric())
        .next()
        .unwrap_or_default();
    matches!(
        last,
        "most"
            | "least"
            | "than"
            | "under"
            | "below"
            | "above"
            | "over"
            | "within"
            | "exceed"
            | "exceeds"
            | "exceeding"
            | "max"
            | "maximum"
            | "min"
            | "minimum"
    )
}

/// One number found in a sentence, before it becomes a candidate.
struct Found {
    start: usize,
    number: String,
    value: f64,
    unit: String,
    range: Option<String>,
}

/// Where a match's kept text ends: before its unit when the unit was
/// dropped.
fn kept_end(whole: regex::Match<'_>, unit: Option<regex::Match<'_>>, reading: &Reading) -> usize {
    match (unit, reading.unit_start) {
        (Some(u), None) => u.start(),
        _ => whole.end(),
    }
}

/// The numbers found so far in a sentence, and the text they claim.
#[derive(Default)]
struct Claimed {
    found: Vec<Found>,
    taken: Vec<(usize, usize)>,
}

impl Claimed {
    fn free(&self, whole: regex::Match<'_>) -> bool {
        !self
            .taken
            .iter()
            .any(|&(s, e)| whole.start() < e && s < whole.end())
    }

    fn claim(&mut self, whole: regex::Match<'_>, item: Found) {
        self.taken.push((whole.start(), whole.end()));
        self.found.push(item);
    }

    /// Claims a single number: its text runs to `end`.
    fn one(&mut self, sentence: &str, whole: regex::Match<'_>, end: usize, reading: Reading) {
        self.claim(
            whole,
            Found {
                start: whole.start(),
                number: sentence[whole.start()..end].trim().to_string(),
                value: reading.value,
                unit: reading.unit,
                range: None,
            },
        );
    }
}

/// Whether a match stands on its own: free of claimed text, and neither
/// continuing the text before it nor continued by the text after it.
fn alone(sentence: &str, claimed: &Claimed, whole: regex::Match<'_>) -> bool {
    claimed.free(whole)
        && !glued(&sentence[..whole.start()])
        && !continued(&sentence[whole.end()..])
}

fn powers(sentence: &str, claimed: &mut Claimed) {
    for m in power_pattern().captures_iter(sentence) {
        let whole = m.get(0).expect("a match has a whole");
        if !alone(sentence, claimed, whole) {
            continue;
        }
        let Some(reading) = read_power(&m) else {
            continue;
        };
        let end = kept_end(whole, m.name("unit"), &reading);
        claimed.one(sentence, whole, end, reading);
    }
}

fn ranges(sentence: &str, claimed: &mut Claimed) {
    for m in range_pattern().captures_iter(sentence) {
        let whole = m.get(0).expect("a match has a whole");
        let (a, b) = (m.name("a").expect("a"), m.name("b").expect("b"));
        let unit = m.name("unit");
        if !alone(sentence, claimed, whole) || unit.is_some_and(|u| ordinal(u.as_str())) {
            continue;
        }
        let (Ok(low), Ok(high)) = (a.as_str().parse::<f64>(), b.as_str().parse::<f64>()) else {
            continue;
        };
        // A year and a month, or a part number, isn't a range.
        if low >= high {
            continue;
        }
        let upper = with_unit(high, unit);
        // A scale such as `million` applies to both ends.
        let scale = upper.value / high;
        let money = m.name("cur").is_some();
        let normalized = if money {
            "USD".to_string()
        } else if upper.unit.is_empty() {
            m.name("ua")
                .map(|u| normalize_unit(u.as_str().trim()))
                .unwrap_or_default()
        } else {
            upper.unit.clone()
        };
        let written = unit
            .filter(|_| upper.unit_start.is_some() && scale == 1.0)
            .or_else(|| m.name("ua"))
            .map(|u| u.as_str().trim().to_string())
            .unwrap_or_default();
        let joined = |n: &str| {
            let n = if money {
                format!("${n}")
            } else {
                n.to_string()
            };
            if written.is_empty() {
                n
            } else if matches!(written.as_str(), "%" | "x" | "X" | "×") {
                format!("{n}{written}")
            } else {
                format!("{n} {written}")
            }
        };
        let end = kept_end(whole, unit, &upper);
        let range = sentence[whole.start()..end].trim().to_string();
        for (number, value, which) in [
            (a.as_str(), low * scale, "lower"),
            (b.as_str(), upper.value, "upper"),
        ] {
            claimed.claim(
                whole,
                Found {
                    start: whole.start(),
                    number: joined(number),
                    value,
                    unit: normalized.clone(),
                    range: Some(format!("the {which} end of {range}")),
                },
            );
        }
    }
}

fn digits(sentence: &str, claimed: &mut Claimed) {
    for m in digit_pattern().captures_iter(sentence) {
        let whole = m.get(0).expect("a match has a whole");
        if !alone(sentence, claimed, whole) {
            continue;
        }
        let Some(reading) = read_digits(&m, &sentence[whole.end()..]) else {
            continue;
        };
        let end = kept_end(whole, m.name("unit"), &reading);
        claimed.one(sentence, whole, end, reading);
    }
}

fn words(sentence: &str, claimed: &mut Claimed) {
    for m in word_pattern().captures_iter(sentence) {
        let whole = m.get(0).expect("a match has a whole");
        if !alone(sentence, claimed, whole) {
            continue;
        }
        if m.name("small")
            .is_some_and(|s| s.as_str().eq_ignore_ascii_case("one"))
            && !bounded(&sentence[..whole.start()])
        {
            continue;
        }
        // `two-phase` or `zero-copy` names a kind, unless the word after
        // the hyphen is a unit of time or a ratio, as in `five-second`.
        let unit = m.name("unit");
        if let Some(u) = unit
            && sentence[..u.start()].ends_with('-')
            && !matches!(
                normalize_unit(u.as_str()).as_str(),
                "x" | "%" | "s" | "ms" | "us" | "min" | "h"
            )
        {
            continue;
        }
        let Some(reading) = read_words(&m) else {
            continue;
        };
        let end = kept_end(whole, unit, &reading);
        claimed.one(sentence, whole, end, reading);
    }
}

fn in_sentence(sentence: &str) -> Vec<Found> {
    let mut claimed = Claimed::default();
    powers(sentence, &mut claimed);
    ranges(sentence, &mut claimed);
    digits(sentence, &mut claimed);
    words(sentence, &mut claimed);
    let mut found = claimed.found;
    found.sort_by_key(|f| f.start);
    found
}

/// What [`numbers`] found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Numbers {
    /// The numbers, in the instruction's order, at most
    /// [`MAX_CANDIDATES`].
    pub candidates: Vec<Candidate>,
    /// The sentences left out because every number in them restates the
    /// host's own time limit.
    pub host_limit: Vec<String>,
    /// How many numbers were found before the bound.
    pub found: usize,
}

/// A time's value in seconds, for a unit of time.
#[must_use]
pub fn in_seconds(value: f64, unit: &str) -> Option<f64> {
    match unit {
        "s" => Some(value),
        "ms" => Some(value / 1e3),
        "us" => Some(value / 1e6),
        "min" => Some(value * 60.0),
        "h" => Some(value * 3_600.0),
        _ => None,
    }
}

/// Whether `candidate` is a time that restates `limit`, the host's own
/// time limit for the whole task, within [`HOST_LIMIT_SLACK`].
#[must_use]
pub fn restates_host_limit(candidate: &Candidate, limit: Duration) -> bool {
    let Some(seconds) = in_seconds(candidate.value, &candidate.unit) else {
        return false;
    };
    let limit = limit.as_secs_f64();
    limit > 0.0 && (seconds - limit).abs() <= (limit * HOST_LIMIT_SLACK).max(HOST_LIMIT_SLACK_SEC)
}

/// Every number in `instruction` with its sentence, in order, at most
/// [`MAX_CANDIDATES`]. With `host_limit`, the host's own time limit for
/// the task, a sentence whose every number restates that limit is left
/// out: it's the host's limit, not the task's goal.
#[must_use]
pub fn numbers(instruction: &str, host_limit: Option<Duration>) -> Numbers {
    let text = comment_pattern().replace_all(instruction, " ");
    let mut out = Numbers::default();
    for sentence in sentences(&text) {
        let clipped = crate::judge::clip(&sentence, 500);
        let mut here: Vec<Candidate> = Vec::new();
        for found in in_sentence(&sentence) {
            let candidate = Candidate {
                sentence: clipped.clone(),
                number: found.number,
                value: found.value,
                unit: found.unit,
                range: found.range,
            };
            if !here
                .iter()
                .chain(&out.candidates)
                .any(|c| c.sentence == candidate.sentence && c.number == candidate.number)
            {
                here.push(candidate);
            }
        }
        if let Some(limit) = host_limit
            && !here.is_empty()
            && here.iter().all(|c| restates_host_limit(c, limit))
        {
            out.host_limit.push(clipped);
            continue;
        }
        out.found += here.len();
        for candidate in here {
            if out.candidates.len() < MAX_CANDIDATES {
                out.candidates.push(candidate);
            }
        }
    }
    out
}

/// Every number in `instruction` with its sentence, in order, at most
/// [`MAX_CANDIDATES`], with no host limit known.
#[must_use]
pub fn candidates(instruction: &str) -> Vec<Candidate> {
    numbers(instruction, None).candidates
}
