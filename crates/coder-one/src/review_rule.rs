//! `control.review`: start the review session only when something
//! disagrees (issue #9637, change 6 of `docs/coder/design/microluna-v18.md`).
//!
//! The v13 self-check ran after every lean loop. On the three
//! `embedding-drift-monitor` trials it took about a fifth of agent time and
//! changed one docstring, and on the two fast held-out failures it changed
//! nothing, because the frozen score was full on wrong figures. This rule
//! starts the review when any of four triggers fires and otherwise lets the
//! episode finish:
//!
//! 1. [`Trigger::Score`]: the frozen score isn't full on the lines that
//!    count. With `check-grades.json` (`accept.grade`, #9635), advisory
//!    lines don't count; without it, every line counts, as keep-best counts
//!    them.
//! 2. [`Trigger::Regressed`]: an executed check regressed on the reviewed
//!    candidate or on a later session's, which `verify.executed` (#9636)
//!    then rejected: a record in `executed-commands.jsonl` with `stage`
//!    `after_session` and `verdict` `regressed`.
//! 3. [`Trigger::Hardcoded`]: the lean loop's hard-coding question flagged
//!    the candidate.
//! 4. [`Trigger::Uncovered`]: a `deliverable` or `check` requirement in the
//!    requirement map names a path or command that no executed check on
//!    the candidate touches.
//!
//! Each trigger reads [`Reading::Fired`], [`Reading::Clear`], or
//! [`Reading::Unknown`]. A trigger is `unknown` when the record it needs
//! doesn't exist, as for triggers 2 and 4 until `verify.executed` writes
//! its record. [`Params::unknown_fires`] decides what `unknown` does; by
//! default it starts the review, so missing evidence never skips it.
//!
//! Both record shapes are the ones `docs/gym/run-card.md` documents. The
//! review's output is a list of concerns, each tied to a requirement ID
//! and, where possible, a command that demonstrates it
//! ([`parse_concerns`]); its edit power is the v13 self-check's.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::record::Implementation;
use crate::requirements::{Kind, RequirementMap};

/// The component ID.
pub const COMPONENT: &str = "control.review";

/// The schema of the rule's record.
pub const RULE_SCHEMA: &str = "openagents.coder-one.review-rule.v1";

/// The rule's revision. A change to what a trigger reads is a new one.
pub const RULE_VERSION: &str = "review-rule-v1";

/// The host-executed command records' file, in `artifacts/` or a lean
/// group's directory.
pub const EXECUTED_FILE: &str = crate::baseline::EXECUTED_FILE;

/// The host-executed command records' schema.
pub const EXECUTED_SCHEMA: &str = crate::baseline::EXECUTED_SCHEMA;

/// The check-line grades' file, in `artifacts/` or a lean group's
/// directory.
pub const GRADES_FILE: &str = "check-grades.json";

/// The check-line grades' schema.
pub const GRADES_SCHEMA: &str = "openagents.coder-one.check-grades.v1";

/// What starts a review.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    /// The frozen score isn't full on non-advisory lines.
    Score,
    /// An executed check on the candidate regressed.
    Regressed,
    /// The hard-coding question flagged the candidate.
    Hardcoded,
    /// A deliverable or check requirement has no executed check that
    /// touches its path or command.
    Uncovered,
}

impl Trigger {
    /// Every trigger, in the issue's order.
    pub const ALL: [Trigger; 4] = [
        Trigger::Score,
        Trigger::Regressed,
        Trigger::Hardcoded,
        Trigger::Uncovered,
    ];

    /// The trigger as the record spells it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Trigger::Score => "score",
            Trigger::Regressed => "regressed",
            Trigger::Hardcoded => "hardcoded",
            Trigger::Uncovered => "uncovered",
        }
    }
}

/// What a trigger reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reading {
    Fired,
    Clear,
    /// The record the trigger needs doesn't exist or doesn't say.
    Unknown,
}

/// The rule's one parameter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    /// Whether a trigger that reads `unknown` starts the review.
    #[serde(default = "yes")]
    pub unknown_fires: bool,
}

fn yes() -> bool {
    true
}

impl Default for Params {
    fn default() -> Self {
        Self {
            unknown_fires: true,
        }
    }
}

/// One line of `check-grades.json`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GradedLine {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub line: Option<u64>,
    /// `follows`, `advisory`, or `unknown`.
    #[serde(default)]
    pub grade: String,
    /// Whether the line passed on each session's candidate, when recorded.
    #[serde(default)]
    pub results: Vec<LineResult>,
}

/// Whether a check line passed on one session's candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineResult {
    pub session: u32,
    pub passed: bool,
}

/// `check-grades.json`, the part the rule reads.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Grades {
    #[serde(default)]
    pub lines: Vec<GradedLine>,
}

/// One line of `executed-commands.jsonl`, the part the rule reads.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Executed {
    /// `baseline`, `after_session`, or `probe`.
    #[serde(default)]
    pub stage: String,
    #[serde(default)]
    pub session: Option<u32>,
    /// `named`, `module`, `make`, `script`, `compile`, or `score`.
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub exit: Option<i64>,
    #[serde(default)]
    pub requirements: Vec<String>,
    /// `ok`, `regressed`, `not_a_regression`, `unknown`, or `null`.
    #[serde(default)]
    pub verdict: Option<String>,
}

/// A requirement trigger 4 reads: its ID, kind, and what it names.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Target {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub commands: Vec<String>,
}

impl Target {
    /// Whether trigger 4 reads it: a deliverable or a check that names a
    /// path or a command.
    #[must_use]
    pub fn in_scope(&self) -> bool {
        matches!(self.kind.as_str(), "deliverable" | "check")
            && !(self.paths.is_empty() && self.commands.is_empty())
    }
}

/// The requirement map's targets, every requirement with its kind.
#[must_use]
pub fn targets(map: &RequirementMap) -> Vec<Target> {
    map.requirements
        .iter()
        .map(|r| Target {
            id: r.id.clone(),
            kind: r.kind.word().to_string(),
            paths: r.extracted.paths.clone(),
            commands: r.extracted.commands.clone(),
        })
        .collect()
}

/// Whether `kind` is one trigger 4 reads.
#[must_use]
pub fn checked_kind(kind: Kind) -> bool {
    matches!(kind, Kind::Deliverable | Kind::Check)
}

/// Everything the rule reads about the candidate under review.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    /// The session whose candidate the review would read.
    pub session: u32,
    /// The host's frozen score on that candidate.
    #[serde(default)]
    pub score: Option<(u64, u64)>,
    /// The grades record, or `None` when not recorded.
    #[serde(default)]
    pub grades: Option<Grades>,
    /// The host-executed command records, or `None` when not recorded.
    #[serde(default)]
    pub executed: Option<Vec<Executed>>,
    /// Whether the hard-coding question flagged the candidate, or `None`
    /// when it wasn't asked or didn't answer.
    #[serde(default)]
    pub hardcoded: Option<bool>,
    /// The requirement map's requirements.
    #[serde(default)]
    pub targets: Vec<Target>,
}

/// One trigger's reading and why.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Reason {
    pub trigger: Trigger,
    pub reading: Reading,
    pub detail: String,
    /// The requirement IDs it bears on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requirements: Vec<String>,
    /// The commands it bears on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<String>,
}

impl Reason {
    fn new(trigger: Trigger, reading: Reading, detail: impl Into<String>) -> Self {
        Self {
            trigger,
            reading,
            detail: detail.into(),
            requirements: Vec::new(),
            commands: Vec::new(),
        }
    }
}

/// The rule's decision.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Decision {
    pub schema: String,
    pub version: String,
    pub session: u32,
    /// Whether the review starts.
    pub review: bool,
    /// The triggers that fired, in order.
    pub fired: Vec<Trigger>,
    /// The triggers that read `unknown`.
    pub unknown: Vec<Trigger>,
    pub unknown_fires: bool,
    /// Every trigger's reading.
    pub triggers: Vec<Reason>,
    /// The decision in words.
    pub reason: String,
}

impl Decision {
    /// The decision as the card shows it: the triggers that fired, `none`,
    /// or `unknown: …` when only an unknown started the review.
    #[must_use]
    pub fn words(&self) -> String {
        words(&self.fired, &self.unknown, self.review)
    }
}

/// The decision in a few words, from its parts.
#[must_use]
pub fn words(fired: &[Trigger], unknown: &[Trigger], review: bool) -> String {
    let join = |t: &[Trigger]| t.iter().map(|t| t.word()).collect::<Vec<_>>().join(", ");
    if !fired.is_empty() {
        join(fired)
    } else if review {
        format!("unknown: {}", join(unknown))
    } else {
        "none".to_string()
    }
}

/// Trigger 1: the frozen score on the lines that count.
#[must_use]
pub fn score_reading(evidence: &Evidence) -> Reason {
    let t = Trigger::Score;
    if let Some(grades) = &evidence.grades {
        let counted: Vec<&GradedLine> = grades
            .lines
            .iter()
            .filter(|line| line.grade != "advisory")
            .collect();
        let last: Vec<Option<bool>> = counted
            .iter()
            .map(|line| {
                line.results
                    .iter()
                    .rev()
                    .find(|r| r.session == evidence.session)
                    .map(|r| r.passed)
            })
            .collect();
        let failed: Vec<String> = counted
            .iter()
            .zip(&last)
            .filter(|(_, passed)| **passed == Some(false))
            .map(|(line, _)| line.id.clone())
            .collect();
        if !failed.is_empty() {
            return Reason::new(
                t,
                Reading::Fired,
                format!(
                    "{} non-advisory check line{} failed on session {}'s candidate: {}",
                    failed.len(),
                    if failed.len() == 1 { "" } else { "s" },
                    evidence.session,
                    failed.join(", ")
                ),
            );
        }
        if last.iter().all(Option::is_some) {
            return Reason::new(
                t,
                Reading::Clear,
                format!(
                    "all {} non-advisory check lines passed on session {}'s candidate",
                    counted.len(),
                    evidence.session
                ),
            );
        }
    }
    match evidence.score {
        Some((passed, total)) if passed >= total => Reason::new(
            t,
            Reading::Clear,
            format!("the frozen score was full, {passed} of {total}"),
        ),
        Some((passed, total)) if evidence.grades.is_some() => Reason::new(
            t,
            Reading::Unknown,
            format!(
                "the frozen score was {passed} of {total}, and the grades don't say which lines \
                 failed"
            ),
        ),
        Some((passed, total)) => Reason::new(
            t,
            Reading::Fired,
            format!(
                "the frozen score was {passed} of {total}; no line is graded, so every line \
                 counts"
            ),
        ),
        None => Reason::new(t, Reading::Unknown, "no frozen score for the candidate"),
    }
}

/// The executed checks on session `session`'s candidate.
fn on_candidate(records: &[Executed], session: u32) -> Vec<&Executed> {
    records
        .iter()
        .filter(|r| r.stage == "after_session" && r.session == Some(session))
        .collect()
}

/// Trigger 2: an executed check regressed on the reviewed candidate, or
/// on a later session's candidate, which `verify.executed` then rejected,
/// so the review reads the kept one.
#[must_use]
pub fn regressed_reading(evidence: &Evidence) -> Reason {
    let t = Trigger::Regressed;
    let Some(records) = &evidence.executed else {
        return Reason::new(
            t,
            Reading::Unknown,
            "no host-executed command record (verify.executed, #9636)",
        );
    };
    let regressed: Vec<&Executed> = records
        .iter()
        .filter(|r| {
            r.stage == "after_session"
                && r.session.is_some_and(|n| n >= evidence.session)
                && r.verdict.as_deref() == Some("regressed")
        })
        .collect();
    if !regressed.is_empty() {
        let mut sessions: Vec<u32> = regressed.iter().filter_map(|r| r.session).collect();
        sessions.sort_unstable();
        sessions.dedup();
        let mut reason = Reason::new(
            t,
            Reading::Fired,
            format!(
                "{} executed check{} regressed on the candidate of session {}",
                regressed.len(),
                if regressed.len() == 1 { "" } else { "s" },
                sessions
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        );
        reason.commands = regressed.iter().map(|r| r.command.clone()).collect();
        let mut ids: Vec<String> = regressed
            .iter()
            .flat_map(|r| r.requirements.iter().cloned())
            .collect();
        ids.sort();
        ids.dedup();
        reason.requirements = ids;
        return reason;
    }
    let checks = on_candidate(records, evidence.session);
    if checks.is_empty() {
        return Reason::new(
            t,
            Reading::Unknown,
            format!(
                "no executed check ran on session {}'s candidate",
                evidence.session
            ),
        );
    }
    let undecided = checks
        .iter()
        .filter(|r| r.verdict.as_deref() == Some("unknown"))
        .count();
    if undecided > 0 {
        return Reason::new(
            t,
            Reading::Unknown,
            format!(
                "none of {} executed checks regressed, but {undecided} had an unknown verdict",
                checks.len()
            ),
        );
    }
    Reason::new(
        t,
        Reading::Clear,
        format!(
            "none of {} executed checks on session {}'s candidate regressed",
            checks.len(),
            evidence.session
        ),
    )
}

/// Trigger 3: the hard-coding question.
#[must_use]
pub fn hardcoded_reading(evidence: &Evidence) -> Reason {
    let t = Trigger::Hardcoded;
    match evidence.hardcoded {
        Some(true) => Reason::new(
            t,
            Reading::Fired,
            format!(
                "the host flagged session {}'s candidate as hard-coded",
                evidence.session
            ),
        ),
        Some(false) => Reason::new(t, Reading::Clear, "the hard-coding question didn't fire"),
        None => Reason::new(
            t,
            Reading::Unknown,
            "the hard-coding question wasn't asked or didn't answer",
        ),
    }
}

/// `path` with a trailing slash, `./`, and a leading `/app/` or `cwd/`
/// taken off, in every form a command might spell it.
fn spellings(path: &str, cwd: Option<&str>) -> Vec<String> {
    let path = path.trim().trim_end_matches('/');
    let mut out = vec![path.to_string()];
    let mut strip = |prefix: &str| {
        if let Some(rest) = path.strip_prefix(prefix)
            && rest.len() >= 3
        {
            out.push(rest.to_string());
        }
    };
    strip("./");
    strip("/app/");
    if let Some(cwd) = cwd {
        strip(&format!("{}/", cwd.trim_end_matches('/')));
    }
    if let Some(name) = path.rsplit('/').next()
        && name.contains('.')
        && name.len() >= 4
        && name != path
    {
        out.push(name.to_string());
    }
    out.retain(|s| !s.is_empty());
    out.dedup();
    out
}

/// Spaces collapsed, for comparing commands.
fn squeeze(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Whether an executed check touches a requirement: it lists the
/// requirement's ID, its command names one of the requirement's paths, or
/// it contains one of the requirement's commands.
#[must_use]
pub fn touches(check: &Executed, target: &Target) -> bool {
    if check.requirements.iter().any(|id| id == &target.id) {
        return true;
    }
    let command = squeeze(&check.command);
    target.paths.iter().any(|path| {
        spellings(path, check.cwd.as_deref())
            .iter()
            .any(|spelling| command.contains(spelling.as_str()))
    }) || target.commands.iter().any(|named| {
        let named = squeeze(named);
        !named.is_empty() && command.contains(&named)
    })
}

/// Trigger 4 over `checks`, the executed checks on the candidate, or `None`
/// when no record says which ran.
#[must_use]
pub fn uncovered_reading(checks: Option<&[Executed]>, targets: &[Target], session: u32) -> Reason {
    let t = Trigger::Uncovered;
    let Some(records) = checks else {
        return Reason::new(
            t,
            Reading::Unknown,
            "no host-executed command record (verify.executed, #9636)",
        );
    };
    let on = on_candidate(records, session);
    if on.is_empty() {
        return Reason::new(
            t,
            Reading::Unknown,
            format!("no executed check ran on session {session}'s candidate"),
        );
    }
    let scope: Vec<&Target> = targets.iter().filter(|t| t.in_scope()).collect();
    if scope.is_empty() {
        return Reason::new(
            t,
            Reading::Clear,
            "no deliverable or check requirement names a path or command",
        );
    }
    let uncovered: Vec<String> = scope
        .iter()
        .filter(|target| !on.iter().any(|check| touches(check, target)))
        .map(|target| target.id.clone())
        .collect();
    if uncovered.is_empty() {
        return Reason::new(
            t,
            Reading::Clear,
            format!(
                "an executed check touches each of the {} deliverable and check requirements \
                 that name a path or command",
                scope.len()
            ),
        );
    }
    let mut reason = Reason::new(
        t,
        Reading::Fired,
        format!(
            "{} of {} deliverable and check requirements have no executed check that touches \
             their path or command: {}",
            uncovered.len(),
            scope.len(),
            uncovered.join(", ")
        ),
    );
    reason.requirements = uncovered;
    reason
}

/// Assembles a decision from the four readings.
#[must_use]
pub fn decision(session: u32, triggers: Vec<Reason>, params: Params) -> Decision {
    let fired: Vec<Trigger> = triggers
        .iter()
        .filter(|r| r.reading == Reading::Fired)
        .map(|r| r.trigger)
        .collect();
    let unknown: Vec<Trigger> = triggers
        .iter()
        .filter(|r| r.reading == Reading::Unknown)
        .map(|r| r.trigger)
        .collect();
    let review = !fired.is_empty() || (params.unknown_fires && !unknown.is_empty());
    let names = |t: &[Trigger]| t.iter().map(|t| t.word()).collect::<Vec<_>>().join(", ");
    let reason = if !fired.is_empty() {
        let details: Vec<&str> = triggers
            .iter()
            .filter(|r| r.reading == Reading::Fired)
            .map(|r| r.detail.as_str())
            .collect();
        format!("the review runs: {}", details.join("; "))
    } else if review {
        format!(
            "the review runs: no trigger fired, but {} read unknown, and unknown fires",
            names(&unknown)
        )
    } else if unknown.is_empty() {
        "no review: no trigger fired".to_string()
    } else {
        format!(
            "no review: no trigger fired; {} read unknown, and unknown doesn't fire",
            names(&unknown)
        )
    };
    Decision {
        schema: RULE_SCHEMA.to_string(),
        version: RULE_VERSION.to_string(),
        session,
        review,
        fired,
        unknown,
        unknown_fires: params.unknown_fires,
        triggers,
        reason,
    }
}

/// The rule: each trigger's reading, and whether the review starts.
#[must_use]
pub fn decide(evidence: &Evidence, params: Params) -> Decision {
    decision(
        evidence.session,
        vec![
            score_reading(evidence),
            regressed_reading(evidence),
            hardcoded_reading(evidence),
            uncovered_reading(
                evidence.executed.as_deref(),
                &evidence.targets,
                evidence.session,
            ),
        ],
        params,
    )
}

/// The host-executed command records under `dirs`, the first file found,
/// or `None` when no directory has one. Lines with another schema are
/// skipped.
#[must_use]
pub fn read_executed(dirs: &[&Path]) -> Option<Vec<Executed>> {
    let text = dirs
        .iter()
        .find_map(|dir| std::fs::read_to_string(dir.join(EXECUTED_FILE)).ok())?;
    Some(
        text.lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .filter(|value| value["schema"] == EXECUTED_SCHEMA)
            .filter_map(|value| serde_json::from_value(value).ok())
            .collect(),
    )
}

/// The check-line grades under `dirs`, the first file found with the
/// grades schema, or `None`.
#[must_use]
pub fn read_grades(dirs: &[&Path]) -> Option<Grades> {
    dirs.iter().find_map(|dir| {
        let text = std::fs::read_to_string(dir.join(GRADES_FILE)).ok()?;
        let value: Value = serde_json::from_str(&text).ok()?;
        (value["schema"] == GRADES_SCHEMA)
            .then(|| serde_json::from_value(value).ok())
            .flatten()
    })
}

/// Whether a lean loop record's `hardcoded` field says the candidate was
/// flagged: `Some(true)` when flagged, `Some(false)` when Jev answered and
/// no literal was found, `None` when the question wasn't asked or Jev
/// didn't answer.
#[must_use]
pub fn hardcoded_of(record: &Value) -> Option<bool> {
    if record.is_null() {
        return None;
    }
    if record["flagged"] == true {
        return Some(true);
    }
    record["p"].as_f64().map(|_| false)
}

/// What the review session is told when the rule started it.
#[must_use]
pub fn guidance(decision: &Decision) -> String {
    // The fired triggers, or, when only unknown ones started the review,
    // those.
    let shown = if decision.fired.is_empty() {
        Reading::Unknown
    } else {
        Reading::Fired
    };
    let why: Vec<String> = decision
        .triggers
        .iter()
        .filter(|r| r.reading == shown)
        .map(|r| format!("- {}", r.detail))
        .collect();
    format!(
        "The host started this review because:\n{}\n\n{CONCERNS_GUIDANCE}",
        why.join("\n")
    )
}

/// How the review reports what it found.
pub const CONCERNS_GUIDANCE: &str = "End your finish summary with one line per concern, in this \
form: `CONCERN R3: what is wrong, and the task's words that show it | command: a command that \
demonstrates it`. Use the requirement's ID from the list in the state, and leave out `| command:` \
when no command demonstrates the concern. When you have no concern, end with the line `CONCERN \
none`.";

/// The requirement list the review's state carries, one line each.
#[must_use]
pub fn requirement_lines(map: &RequirementMap) -> Vec<String> {
    map.requirements
        .iter()
        .map(|r| {
            format!(
                "{} ({}): {}",
                r.id,
                r.kind.word(),
                crate::judge::clip(&squeeze(&r.text), 200)
            )
        })
        .collect()
}

/// One concern from the review.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Concern {
    /// The requirement ID it names, or `None` when it names none.
    pub requirement: Option<String>,
    pub concern: String,
    /// A command that demonstrates it, when given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

/// The concerns in a review's finish summary: every line that starts with
/// `CONCERN`. `CONCERN none` gives none.
#[must_use]
pub fn parse_concerns(summary: &str) -> Vec<Concern> {
    let mut out = Vec::new();
    for line in summary.lines() {
        let line = line
            .trim()
            .trim_start_matches(['-', '*', '`'])
            .trim()
            .trim_end_matches('`');
        let Some(rest) = line.strip_prefix("CONCERN") else {
            continue;
        };
        let rest = rest.trim();
        if rest.eq_ignore_ascii_case("none") || rest.is_empty() {
            continue;
        }
        let (head, body) = match rest.split_once(':') {
            Some((head, body)) if is_id(head.trim()) => (Some(head.trim().to_string()), body),
            _ => (None, rest),
        };
        let (concern, command) = match body.split_once("| command:") {
            Some((concern, command)) => (
                concern.trim(),
                Some(command.trim().trim_matches('`').trim().to_string()).filter(|c| !c.is_empty()),
            ),
            None => (body.trim(), None),
        };
        out.push(Concern {
            requirement: head,
            concern: concern.to_string(),
            command,
        });
    }
    out
}

/// Whether `word` is a requirement ID: `R` and digits.
fn is_id(word: &str) -> bool {
    word.len() > 1 && word.starts_with('R') && word[1..].chars().all(|c| c.is_ascii_digit())
}

/// The concerns record for the loop.
#[must_use]
pub fn concerns_record(session: u32, summary: &str, known: &[Target]) -> Value {
    let concerns = parse_concerns(summary);
    let unknown_ids: Vec<&String> = concerns
        .iter()
        .filter_map(|c| c.requirement.as_ref())
        .filter(|id| !known.iter().any(|t| &t.id == *id))
        .collect();
    json!({
        "kind": "lean.review_concerns",
        "session": session,
        "concerns": concerns,
        "with_requirement": concerns.iter().filter(|c| c.requirement.is_some()).count(),
        "with_command": concerns.iter().filter(|c| c.command.is_some()).count(),
        "unknown_requirements": unknown_ids,
    })
}

/// The implementation this build runs.
#[must_use]
pub fn implementation(params: Params) -> Implementation {
    Implementation::new(
        COMPONENT,
        RULE_VERSION,
        &json!({
            "unknown_fires": params.unknown_fires,
            "executed_schema": EXECUTED_SCHEMA,
            "grades_schema": GRADES_SCHEMA,
        }),
    )
}

#[cfg(test)]
mod tests;
