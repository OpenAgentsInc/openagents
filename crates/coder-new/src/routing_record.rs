//! Routing records: every harness choice, what it was made from, and how
//! the turn went (#11454, part of #11451).
//!
//! Each time a chat's turn is given to a harness, one `chose` line is
//! written: the chat and turn, the admitted candidates, why every other
//! harness was left out, what the selector looked at (typed task features
//! and any System One probabilities, each with its model and door), and the
//! choice with the rule or pin that made it. When the turn ends, one `ended`
//! line with the same id adds the outcome: cost, time, tokens, whether
//! checks passed when that is known, how it ended, and the harness's
//! version.
//!
//! This is the loop `docs/research/typesafe/2026-10-03-calibration.md` says
//! we lack: probabilities written next to the outcomes they predicted, so
//! the router's thresholds can be fitted and judged instead of set by hand.
//!
//! The lines live in `~/.openagents/coder/routing/<YYYY-MM-DD>.jsonl`
//! (UTC dates), in a 0700 folder with 0600 files, and never leave this
//! computer. Nothing raw goes in them: error text is cut to a short line
//! with paths and credential-shaped words replaced. [`summarize`] reads
//! them back grouped by harness for Gym.
//!
//! `CODER_ROUTING_DIR` moves the folder; tests write nowhere unless it is
//! set or they pass a folder themselves.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::harness_plugin::{Candidates, Exclusion, Harness, Payer};

/// The environment variable that moves the records' folder.
pub const DIR_ENV: &str = "CODER_ROUTING_DIR";

/// The longest diagnostic a record keeps, in characters.
pub const DIAGNOSTIC_MAX: usize = 240;

/// What a request is for, so a title or compaction call is never counted
/// as a harness's work on the person's turn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// The person's turn.
    #[default]
    Primary,
    /// Shortening a long chat.
    Compaction,
    /// Naming a chat.
    Title,
    /// Any other helper call Coder makes for itself.
    Generate,
}

/// When a harness was left out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Stage {
    /// The candidate check, before anything ranked them.
    Candidates,
    /// The harness was chosen and failed to start.
    Start,
}

/// One harness that was not a candidate, and why, from a fixed set of
/// reasons.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Excluded {
    pub harness: Harness,
    pub stage: Stage,
    pub category: Exclusion,
}

/// What the selector knew about the task, as typed facts.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Features {
    /// The chat runs on this computer.
    pub local: bool,
    /// Pictures attached to the message.
    pub images: u32,
    /// The message's length in characters.
    pub prompt_chars: u32,
    /// The person chose a model rather than `auto`.
    pub pinned_model: bool,
}

/// One System One answer the selector read.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Probability {
    /// The question asked, by its stable name.
    pub question: String,
    /// The answer this probability belongs to.
    pub answer: String,
    pub p: f64,
    /// The model that answered, by its pinned id.
    pub model: String,
    /// The door it was asked through.
    pub door: String,
}

/// Everything the selector looked at.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Inputs {
    pub features: Features,
    #[serde(default)]
    pub probabilities: Vec<Probability>,
}

/// What made the choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Rule {
    /// The person pinned this harness.
    Pin,
    /// The first admitted candidate in declaration order.
    FirstAdmitted,
    /// The chat's earlier turns ran on it, and follow-ups stay there.
    Continues,
    /// The chat's harness went away mid-chat (its plugin was turned off)
    /// and the fixed rule picked again.
    Reroute,
    /// A router read the inputs and picked it.
    Router,
    /// The chosen harness failed and this one took the turn instead.
    Fallback,
}

/// The selector's call when it gives a turn to a harness.
#[derive(Clone, Debug)]
pub struct Choice<'a> {
    /// The chat's saved session id, when it has one yet.
    pub chat: Option<String>,
    /// The turn's number in the chat, from one.
    pub turn: u32,
    pub kind: Kind,
    pub candidates: &'a Candidates,
    pub inputs: Inputs,
    pub harness: Harness,
    pub rule: Rule,
    pub payer: Payer,
}

/// How a turn ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Ended {
    /// A reply came back.
    Done,
    /// The harness or its provider failed the turn.
    Error,
    /// A usage or plan limit stopped it.
    Limit,
    /// The person stopped it, or started another turn over it.
    Stopped,
}

/// What the caller knows when the turn ends.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outcome {
    pub ended: Option<Ended>,
    pub cost_usd: Option<f64>,
    pub tokens: Option<u64>,
    /// Whether the turn's checks passed, when any ran.
    pub checks_passed: Option<bool>,
    /// Whether the person redid or re-asked the turn, when that is known.
    pub redone: Option<bool>,
    /// The model that answered, when one did.
    pub model: Option<String>,
    /// The error, unscrubbed; the record keeps only [`scrub`] of it.
    pub error: Option<String>,
}

/// One line of a records file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Line {
    Chose(ChoseLine),
    Ended(EndedLine),
}

/// A choice as written.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChoseLine {
    pub id: String,
    pub at_ms: u64,
    pub coder_version: String,
    pub chat: Option<String>,
    pub turn: u32,
    pub kind: Kind,
    pub admitted: Vec<Harness>,
    pub excluded: Vec<Excluded>,
    pub inputs: Inputs,
    pub harness: Harness,
    pub rule: Rule,
    pub payer: Payer,
}

/// An outcome as written.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EndedLine {
    pub id: String,
    pub at_ms: u64,
    pub harness: Harness,
    pub ended: Ended,
    pub elapsed_ms: u64,
    pub cost_usd: Option<f64>,
    pub tokens: Option<u64>,
    pub checks_passed: Option<bool>,
    pub redone: Option<bool>,
    pub model: Option<String>,
    /// The harness's runtime version, so drift after an upgrade shows.
    pub harness_version: Option<String>,
    /// A short scrubbed line from the error, when there was one.
    pub diagnostic: Option<String>,
}

/// A turn whose choice is written and whose outcome is not yet.
#[derive(Clone, Debug)]
pub struct Open {
    pub id: String,
    pub harness: Harness,
    started: std::time::Instant,
    /// The chat's totals when the turn started, so the turn's own share
    /// can be taken at the end.
    pub tokens_before: u64,
    pub cost_before: f64,
}

/// Where records go.
#[derive(Clone, Debug)]
pub struct Recorder {
    dir: PathBuf,
}

/// The records folder on this computer: `CODER_ROUTING_DIR`, or
/// `~/.openagents/coder/routing` outside tests.
#[must_use]
pub fn dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os(DIR_ENV).filter(|dir| !dir.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    #[cfg(test)]
    {
        TEST.with(|test| test.borrow().as_ref().map(|(dir, _)| dir.clone()))
    }
    #[cfg(not(test))]
    crate::home::data_path("routing")
}

/// The recorder for this computer, or `None` when records are off (tests
/// without a folder).
#[must_use]
pub fn recorder() -> Option<Recorder> {
    dir().map(Recorder::new)
}

#[cfg(test)]
thread_local! {
    /// A test's records folder and the facts its App reads, on its own
    /// thread only.
    static TEST: std::cell::RefCell<Option<(PathBuf, crate::harness_plugin::Facts)>> =
        const { std::cell::RefCell::new(None) };
}

/// The facts the candidate check reads for `app`.
fn facts(app: &crate::App) -> crate::harness_plugin::Facts {
    #[cfg(test)]
    if let Some(facts) = TEST.with(|test| test.borrow().as_ref().map(|(_, facts)| *facts)) {
        return facts;
    }
    crate::harness_plugin::Facts::read(app)
}

static VERSIONS: Mutex<BTreeMap<String, String>> = Mutex::new(BTreeMap::new());

/// Notes the version a harness's runtime reported, such as Claude Code's
/// from its start-up handshake.
pub fn note_version(harness: Harness, version: Option<&str>) {
    let Some(version) = version.map(str::trim).filter(|v| !v.is_empty()) else {
        return;
    };
    VERSIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(key(harness), version.chars().take(64).collect());
}

/// The version `harness` last reported: Coder's own for its loop.
#[must_use]
pub fn version(harness: Harness) -> Option<String> {
    if harness == Harness::CoderLoop {
        return Some(env!("CARGO_PKG_VERSION").to_owned());
    }
    VERSIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&key(harness))
        .cloned()
}

fn key(harness: Harness) -> String {
    serde_json::to_value(harness)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

impl Recorder {
    #[must_use]
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// Writes the choice and returns the open turn to end later. A record
    /// that can't be written is skipped; routing never waits on it.
    pub fn chose(&self, choice: &Choice<'_>, tokens_before: u64, cost_before: f64) -> Open {
        let at_ms = atif::now_ms();
        let id = new_id(at_ms);
        let line = Line::Chose(ChoseLine {
            id: id.clone(),
            at_ms,
            coder_version: env!("CARGO_PKG_VERSION").to_owned(),
            chat: choice.chat.clone(),
            turn: choice.turn,
            kind: choice.kind,
            admitted: choice.candidates.admitted.clone(),
            excluded: choice
                .candidates
                .excluded
                .iter()
                .map(|&(harness, category)| Excluded {
                    harness,
                    stage: Stage::Candidates,
                    category,
                })
                .collect(),
            inputs: choice.inputs.clone(),
            harness: choice.harness,
            rule: choice.rule,
            payer: choice.payer,
        });
        let _ = self.append(at_ms, &line);
        Open {
            id,
            harness: choice.harness,
            started: std::time::Instant::now(),
            tokens_before,
            cost_before,
        }
    }

    /// Writes the outcome of `open`.
    pub fn ended(&self, open: &Open, outcome: Outcome) {
        let at_ms = atif::now_ms();
        let line = Line::Ended(EndedLine {
            id: open.id.clone(),
            at_ms,
            harness: open.harness,
            ended: outcome.ended.unwrap_or(if outcome.error.is_some() {
                Ended::Error
            } else {
                Ended::Done
            }),
            elapsed_ms: u64::try_from(open.started.elapsed().as_millis()).unwrap_or(u64::MAX),
            cost_usd: outcome.cost_usd.filter(|cost| cost.is_finite()),
            tokens: outcome.tokens,
            checks_passed: outcome.checks_passed,
            redone: outcome.redone,
            model: outcome.model.map(|model| scrub(&model)),
            harness_version: version(open.harness),
            diagnostic: outcome.error.as_deref().map(scrub),
        });
        let _ = self.append(at_ms, &line);
    }

    fn append(&self, at_ms: u64, line: &Line) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.dir, std::fs::Permissions::from_mode(0o700));
        }
        let path = self.dir.join(format!("{}.jsonl", utc_date(at_ms / 1000)));
        let mut options = std::fs::OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(path)?;
        let mut text = serde_json::to_string(line).map_err(std::io::Error::other)?;
        text.push('\n');
        file.write_all(text.as_bytes())
    }
}

fn new_id(at_ms: u64) -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let mut salt = [0u8; 4];
    let _ = getrandom::fill(&mut salt);
    format!(
        "{at_ms}-{}-{:08x}",
        NEXT.fetch_add(1, Ordering::Relaxed),
        u32::from_le_bytes(salt)
    )
}

/// `YYYY-MM-DD` for a time in seconds since 1970, in UTC.
#[must_use]
pub fn utc_date(seconds: u64) -> String {
    // Howard Hinnant's civil_from_days.
    let days = i64::try_from(seconds / 86_400).unwrap_or(0) + 719_468;
    let era = days.div_euclid(146_097);
    let doe = days.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// A diagnostic safe to keep: the first line, words that look like paths
/// or credentials replaced, and at most [`DIAGNOSTIC_MAX`] characters.
#[must_use]
pub fn scrub(text: &str) -> String {
    let first = text
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("");
    let words: Vec<String> = first
        .split_whitespace()
        .map(|word| {
            let bare = word.trim_matches(|c: char| "\"'`()[]{}<>,;:".contains(c));
            if looks_like_path(bare) {
                "[path]".to_owned()
            } else if looks_like_secret(bare) {
                "[redacted]".to_owned()
            } else {
                word.to_owned()
            }
        })
        .collect();
    let joined = words.join(" ");
    if joined.chars().count() <= DIAGNOSTIC_MAX {
        joined
    } else {
        let mut cut: String = joined.chars().take(DIAGNOSTIC_MAX - 1).collect();
        cut.push('…');
        cut
    }
}

fn looks_like_path(word: &str) -> bool {
    word.starts_with('/')
        || word.starts_with("~/")
        || word.starts_with("./")
        || word.starts_with("../")
        || (word.contains('/') && !word.contains("://") && word.contains('.'))
}

fn looks_like_secret(word: &str) -> bool {
    let lower = word.to_ascii_lowercase();
    if ["sk-", "oa_", "ghp_", "gho_", "xox", "bearer"]
        .iter()
        .any(|prefix| lower.starts_with(prefix) && word.len() > 8)
    {
        return true;
    }
    if word.contains('=') {
        let (name, _) = word.split_once('=').unwrap_or_default();
        let name = name.to_ascii_lowercase();
        if ["key", "token", "secret", "password"]
            .iter()
            .any(|marker| name.contains(marker))
        {
            return true;
        }
    }
    // A long run of letters and digits with both is a key, not a word.
    word.len() >= 24
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && word.chars().any(|c| c.is_ascii_digit())
        && word.chars().any(|c| c.is_ascii_alphabetic())
}

/// Every record line in `dir`'s `.jsonl` files, oldest file first. Lines
/// that don't parse are skipped.
#[must_use]
pub fn read(dir: &Path) -> Vec<Line> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
        .collect();
    files.sort();
    files
        .iter()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .flat_map(|text| {
            text.lines()
                .filter_map(|line| serde_json::from_str::<Line>(line).ok())
                .collect::<Vec<_>>()
        })
        .collect()
}

/// One harness's turns, for Gym.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HarnessSummary {
    pub harness: Option<Harness>,
    /// Person turns given to it (`primary` requests only).
    pub chosen: u64,
    /// How those turns ended, by [`Ended`].
    pub ended: BTreeMap<String, u64>,
    /// Chosen turns with no outcome written (Coder quit or crashed).
    pub unfinished: u64,
    /// Which rule chose it, by [`Rule`].
    pub rules: BTreeMap<String, u64>,
    /// How often it was left out, by [`Exclusion`].
    pub excluded: BTreeMap<String, u64>,
    pub cost_usd: f64,
    pub tokens: u64,
    pub elapsed_ms_total: u64,
    /// Turns whose checks ran, and of those how many passed.
    pub checked: u64,
    pub checks_passed: u64,
    /// The versions seen in outcomes, each with its count.
    pub versions: BTreeMap<String, u64>,
}

impl HarnessSummary {
    /// The share of ended turns that came back with a reply.
    #[must_use]
    pub fn done_rate(&self) -> Option<f64> {
        let total: u64 = self.ended.values().sum();
        (total > 0).then(|| {
            #[allow(clippy::cast_precision_loss)]
            let rate = *self.ended.get("done").unwrap_or(&0) as f64 / total as f64;
            rate
        })
    }
}

fn name<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// Groups `lines` by harness: choices, outcomes, exclusions, cost, tokens,
/// time, checks and versions. Helper requests (titles, compaction) are left
/// out so they are never counted as a harness's work.
#[must_use]
pub fn summarize(lines: &[Line]) -> Vec<HarnessSummary> {
    let mut by: BTreeMap<String, HarnessSummary> = BTreeMap::new();
    let mut primary = std::collections::BTreeSet::new();
    let mut open = std::collections::BTreeSet::new();
    for line in lines {
        if let Line::Chose(chose) = line {
            if chose.kind != Kind::Primary {
                continue;
            }
            primary.insert(chose.id.clone());
            open.insert(chose.id.clone());
            let summary = of(&mut by, chose.harness);
            summary.chosen += 1;
            *summary.rules.entry(name(&chose.rule)).or_default() += 1;
            for excluded in &chose.excluded {
                *of(&mut by, excluded.harness)
                    .excluded
                    .entry(name(&excluded.category))
                    .or_default() += 1;
            }
        }
    }
    for line in lines {
        if let Line::Ended(ended) = line {
            if !primary.contains(&ended.id) {
                continue;
            }
            open.remove(&ended.id);
            let summary = of(&mut by, ended.harness);
            *summary.ended.entry(name(&ended.ended)).or_default() += 1;
            summary.cost_usd += ended.cost_usd.unwrap_or_default();
            summary.tokens = summary.tokens.saturating_add(ended.tokens.unwrap_or(0));
            summary.elapsed_ms_total = summary.elapsed_ms_total.saturating_add(ended.elapsed_ms);
            if let Some(passed) = ended.checks_passed {
                summary.checked += 1;
                summary.checks_passed += u64::from(passed);
            }
            if let Some(version) = &ended.harness_version {
                *summary.versions.entry(version.clone()).or_default() += 1;
            }
        }
    }
    for line in lines {
        if let Line::Chose(chose) = line
            && open.contains(&chose.id)
        {
            of(&mut by, chose.harness).unfinished += 1;
        }
    }
    by.into_values().collect()
}

fn of(by: &mut BTreeMap<String, HarnessSummary>, harness: Harness) -> &mut HarnessSummary {
    by.entry(key(harness)).or_insert_with(|| HarnessSummary {
        harness: Some(harness),
        ..HarnessSummary::default()
    })
}

impl crate::App {
    /// Records that this chat's next turn went to `harness`, under the rule
    /// its selector set ([`crate::App::route_turn`], a pin),
    /// ending any turn still open first (it was stopped). Does nothing when
    /// records are off.
    pub(crate) fn record_choice(&mut self, harness: Harness, images: usize, prompt_chars: usize) {
        let rule = self.routing_rule.take().unwrap_or(Rule::Continues);
        self.record_end(Some(Ended::Stopped), None, None);
        let Some(recorder) = recorder() else {
            return;
        };
        let facts = facts(self);
        let candidates = crate::harness_plugin::candidates(&facts);
        let choice = Choice {
            chat: self.session_id().map(str::to_owned),
            turn: u32::try_from(self.live.user_count()).unwrap_or(u32::MAX),
            kind: Kind::Primary,
            candidates: &candidates,
            inputs: Inputs {
                features: Features {
                    local: facts.local,
                    images: u32::try_from(images).unwrap_or(u32::MAX),
                    prompt_chars: u32::try_from(prompt_chars).unwrap_or(u32::MAX),
                    pinned_model: crate::models::pinned(&self.plugins.model),
                },
                probabilities: Vec::new(),
            },
            harness,
            rule,
            payer: crate::harness_plugin::payer(harness, &facts),
        };
        self.routing = Some(recorder.chose(&choice, self.live.tokens, self.live.cost_usd));
    }

    /// Records how the open turn ended, with the chat's tokens and cost
    /// since it started.
    pub(crate) fn record_end(
        &mut self,
        ended: Option<Ended>,
        model: Option<String>,
        error: Option<String>,
    ) {
        let Some(open) = self.routing.take() else {
            return;
        };
        let Some(recorder) = recorder() else {
            return;
        };
        let ended = ended.or_else(|| {
            error.as_deref().map(|error| {
                if crate::long_session::pause_seconds(error).is_some() {
                    Ended::Limit
                } else {
                    Ended::Error
                }
            })
        });
        recorder.ended(
            &open,
            Outcome {
                ended,
                cost_usd: Some((self.live.cost_usd - open.cost_before).max(0.0)),
                tokens: Some(self.live.tokens.saturating_sub(open.tokens_before)),
                model,
                error,
                ..Outcome::default()
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidates() -> Candidates {
        Candidates {
            admitted: vec![Harness::ClaudeCode, Harness::CoderLoop],
            excluded: vec![(Harness::Codex, Exclusion::NoChatTurn)],
        }
    }

    fn choice(candidates: &Candidates, harness: Harness, kind: Kind) -> Choice<'_> {
        Choice {
            chat: Some("chat-1".into()),
            turn: 1,
            kind,
            candidates,
            inputs: Inputs {
                features: Features {
                    local: true,
                    images: 1,
                    prompt_chars: 42,
                    pinned_model: false,
                },
                probabilities: vec![Probability {
                    question: "route".into(),
                    answer: "code".into(),
                    p: 0.83,
                    model: "jev-1.13".into(),
                    door: "typesafe".into(),
                }],
            },
            harness,
            rule: Rule::FirstAdmitted,
            payer: Payer::ClaudePlan,
        }
    }

    fn temp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "coder-routing-test-{}-{}",
            std::process::id(),
            new_id(atif::now_ms())
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_choice_and_its_outcome_are_written_and_read_back() {
        let dir = temp();
        let recorder = Recorder::new(&dir);
        let candidates = candidates();
        note_version(Harness::ClaudeCode, Some("2.1.296"));
        let open = recorder.chose(
            &choice(&candidates, Harness::ClaudeCode, Kind::Primary),
            10,
            0.5,
        );
        recorder.ended(
            &open,
            Outcome {
                cost_usd: Some(0.02),
                tokens: Some(1200),
                checks_passed: Some(true),
                model: Some("claude-opus-5-5".into()),
                ..Outcome::default()
            },
        );
        let lines = read(&dir);
        assert_eq!(lines.len(), 2);
        let Line::Chose(chose) = &lines[0] else {
            panic!("the choice comes first");
        };
        assert_eq!(chose.harness, Harness::ClaudeCode);
        assert_eq!(chose.admitted, [Harness::ClaudeCode, Harness::CoderLoop]);
        assert_eq!(
            chose.excluded,
            [Excluded {
                harness: Harness::Codex,
                stage: Stage::Candidates,
                category: Exclusion::NoChatTurn,
            }]
        );
        assert_eq!(chose.inputs.probabilities[0].model, "jev-1.13");
        let Line::Ended(ended) = &lines[1] else {
            panic!("the outcome comes second");
        };
        assert_eq!(ended.id, chose.id);
        assert_eq!(ended.ended, Ended::Done);
        assert_eq!(ended.harness_version.as_deref(), Some("2.1.296"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let file = std::fs::read_dir(&dir)
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            assert_eq!(
                std::fs::metadata(file).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_summary_groups_turns_by_harness_and_leaves_out_helper_calls() {
        let dir = temp();
        let recorder = Recorder::new(&dir);
        let candidates = candidates();
        let claude = recorder.chose(
            &choice(&candidates, Harness::ClaudeCode, Kind::Primary),
            0,
            0.0,
        );
        recorder.ended(
            &claude,
            Outcome {
                error: Some("Claude Code hit the plan limit".into()),
                ended: Some(Ended::Limit),
                ..Outcome::default()
            },
        );
        let coder = recorder.chose(
            &choice(&candidates, Harness::CoderLoop, Kind::Primary),
            0,
            0.0,
        );
        recorder.ended(
            &coder,
            Outcome {
                cost_usd: Some(0.25),
                tokens: Some(500),
                checks_passed: Some(false),
                ..Outcome::default()
            },
        );
        // Never ended.
        recorder.chose(
            &choice(&candidates, Harness::CoderLoop, Kind::Primary),
            0,
            0.0,
        );
        // A title call is not the harness's work.
        let title = recorder.chose(
            &choice(&candidates, Harness::CoderLoop, Kind::Title),
            0,
            0.0,
        );
        recorder.ended(
            &title,
            Outcome {
                cost_usd: Some(9.0),
                ..Outcome::default()
            },
        );

        let summary = summarize(&read(&dir));
        let get = |harness| {
            summary
                .iter()
                .find(|s| s.harness == Some(harness))
                .cloned()
                .unwrap_or_default()
        };
        let claude = get(Harness::ClaudeCode);
        assert_eq!(claude.chosen, 1);
        assert_eq!(claude.ended.get("limit"), Some(&1));
        assert_eq!(claude.done_rate(), Some(0.0));
        let coder = get(Harness::CoderLoop);
        assert_eq!(coder.chosen, 2);
        assert_eq!(coder.unfinished, 1);
        assert_eq!(coder.ended.get("done"), Some(&1));
        assert!((coder.cost_usd - 0.25).abs() < 1e-9);
        assert_eq!((coder.checked, coder.checks_passed), (1, 0));
        assert_eq!(coder.rules.get("first-admitted"), Some(&2));
        assert_eq!(
            coder.versions.get(env!("CARGO_PKG_VERSION")),
            Some(&1),
            "the loop's version is Coder's own"
        );
        let codex = get(Harness::Codex);
        assert_eq!(codex.chosen, 0);
        assert_eq!(codex.excluded.get("no-chat-turn"), Some(&3));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn diagnostics_are_one_short_line_without_paths_or_keys() {
        let raw = "Failed reading /Users/someone/.claude/settings.json with key sk-ant-abcdef123456 token=hunter2\nstack trace here";
        let scrubbed = scrub(raw);
        assert_eq!(
            scrubbed,
            "Failed reading [path] with key [redacted] [redacted]"
        );
        assert!(!scrubbed.contains("stack"));
        let long = "word ".repeat(200);
        assert_eq!(scrub(&long).chars().count(), DIAGNOSTIC_MAX);
        assert_eq!(
            scrub("HTTP 429 (resets in 30 s)"),
            "HTTP 429 (resets in 30 s)"
        );
    }

    #[test]
    fn dates_are_utc_days() {
        assert_eq!(utc_date(0), "1970-01-01");
        assert_eq!(utc_date(1_791_763_200), "2026-10-12");
        assert_eq!(utc_date(951_782_400), "2000-02-29");
    }

    /// Real turns on both harnesses, through the paths the composer uses,
    /// each leave a choice and an outcome.
    #[test]
    fn turns_on_both_harnesses_are_recorded() {
        use crate::harness_plugin::{Facts, Health};
        let dir = temp();
        let healthy = Health {
            signed_in: true,
            at_limit: false,
        };
        TEST.with(|test| {
            *test.borrow_mut() = Some((
                dir.clone(),
                Facts {
                    local: true,
                    claude_plugin_on: true,
                    claude: healthy,
                    codex_on: false,
                    codex: healthy,
                    openrouter_key: false,
                    leaning: crate::routing_preferences::Leaning::default(),
                },
            ));
        });
        let finish = |app: &mut crate::App, result: Result<openrouter::Streamed, String>| {
            let id = app.request_id;
            app.apply_update(crate::live::Update::Finished { id, result });
        };
        // A pinned chat on Coder's loop, and its follow-up.
        let mut app = crate::App::default();
        app.set_mode(crate::Mode::Live);
        app.draft.text = "hello".into();
        app.submit_on(Harness::CoderLoop).unwrap();
        let id = app.request_id;
        app.apply_update(crate::live::Update::Usage {
            id,
            usage: crate::bundled_runtime::RoundUsage {
                prompt_tokens: 30,
                completion_tokens: 12,
                cost: Some(0.01),
                ..Default::default()
            },
        });
        finish(
            &mut app,
            Ok(openrouter::Streamed {
                text: "hi".into(),
                model: "openagents/auto".into(),
                ..Default::default()
            }),
        );
        app.draft.text = "and again".into();
        app.submit_live();
        finish(
            &mut app,
            Err("The gateway refused the call (HTTP 429)".into()),
        );
        // A chat on Claude Code.
        let mut app = crate::App::default();
        app.set_mode(crate::Mode::Live);
        app.draft.text = "over to claude".into();
        app.submit_on(Harness::ClaudeCode).unwrap();
        finish(
            &mut app,
            Err("Claude Code stopped before the turn ended.".into()),
        );
        TEST.with(|test| test.borrow_mut().take());

        let lines = read(&dir);
        let chose: Vec<&ChoseLine> = lines
            .iter()
            .filter_map(|line| match line {
                Line::Chose(chose) => Some(chose),
                Line::Ended(_) => None,
            })
            .collect();
        let ended: Vec<&EndedLine> = lines
            .iter()
            .filter_map(|line| match line {
                Line::Ended(ended) => Some(ended),
                Line::Chose(_) => None,
            })
            .collect();
        assert_eq!(lines.len(), 6, "{lines:?}");
        assert_eq!(
            chose
                .iter()
                .map(|c| (c.harness, c.rule))
                .collect::<Vec<_>>(),
            [
                (Harness::CoderLoop, Rule::Pin),
                (Harness::CoderLoop, Rule::Continues),
                (Harness::ClaudeCode, Rule::Pin),
            ]
        );
        assert_eq!(chose[1].turn, 2);
        assert_eq!(ended[0].ended, Ended::Done);
        assert_eq!(ended[0].tokens, Some(42));
        assert_eq!(ended[1].ended, Ended::Limit);
        assert_eq!(ended[2].harness, Harness::ClaudeCode);
        assert_eq!(ended[2].ended, Ended::Error);
        let summary = summarize(&lines);
        assert_eq!(summary.iter().map(|s| s.chosen).sum::<u64>(), 3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tests_write_nowhere_by_default() {
        if std::env::var_os(DIR_ENV).is_none() {
            assert!(dir().is_none());
        }
    }
}
