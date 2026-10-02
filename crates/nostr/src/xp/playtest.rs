//! NIP-XP's `playtest` rule and the two records it reads
//! (`nips/openagents/NIP-XP.md`, "`playtest`").
//!
//! A **playtest report** (`3197`) is a small, content-free event the tester
//! signs: the build, the platform, the report's kind, and the SHA-256 of
//! the private report's exact bytes, which travelled to the triage key
//! privately (NIP-17). It carries no text. A **session record** (`3196`) is
//! the moderator's signed note that a tester completed a moderated or group
//! session script on a build.
//!
//! A `playtest` quest names one contribution (`feedback`, `bug`, `design`,
//! `verified-fix`, `session`, or `diary`) and the season's build list. An
//! award names the tester's report, and for a moderated or group session
//! the session record, and cites the public issue that records the
//! triager's acceptance. Readers re-check the binding, the season, the
//! build list, the amount, and uniqueness from signed events alone; whether
//! a bug was real is the referee's judgment on the public issue, which is
//! why a separate playtest referee signs these awards and they never feed
//! the trainer level.
//!
//! Uniqueness is rule-derived: the award's key isn't the quest coordinate
//! but a key this rule computes ([`key`]), so one issue earns one
//! report-class award across the season's quests, and a script earns one
//! session award per tester. A quest version also states `max_awards`; a
//! reader that sees more live awards on it counts none of them.

use serde_json::{Map, Value, json};

use super::{
    AWARD_KIND, Award, Awardee, PLAYTEST, QUEST_KIND, Quest, coordinate, in_season, open,
    parse_quest,
};
use crate::contracts::{ContractError, RefusalCode};
use crate::domain::Event;
use crate::kb::{
    Pointer, Unsigned, is_hex, malformed, mismatch, number, one_tag, reject, require, tag, text,
};

/// The tester's content-free playtest report.
pub const REPORT_KIND: u16 = crate::kinds::XP_PLAYTEST_REPORT;
/// A moderator's record of a completed moderated or group session.
pub const SESSION_KIND: u16 = crate::kinds::XP_PLAYTEST_SESSION;
/// Every playtest uniqueness key starts with this.
pub const KEY_PREFIX: &str = "playtest:";
/// The contributions a `playtest` quest can name.
pub const CONTRIBUTIONS: &[&str] = &[
    "feedback",
    "bug",
    "design",
    "verified-fix",
    "session",
    "diary",
];
/// A report's kinds. The first five are what a tester files from the app
/// (`comment` is **Give feedback** on selected text).
pub const REPORT_KINDS: &[&str] = &[
    "bug",
    "confusing",
    "idea",
    "felt-good",
    "comment",
    "verified",
    "session",
    "diary",
];
/// Where the tester ran the build.
pub const PLATFORMS: &[&str] = &["ios", "android", "macos", "windows", "linux"];
/// How a session script was run.
pub const FORMATS: &[&str] = &["unmoderated", "moderated", "group"];
/// Triage severities, most severe first.
pub const SEVERITIES: &[&str] = &["p0", "p1", "p2", "p3"];
/// Builds a quest may list, at most.
pub const MAX_BUILDS: usize = 64;
/// Awards a quest version may state, at most.
pub const MAX_AWARDS: u64 = 10_000;

/// What a `playtest` quest accepts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaytestAcceptance {
    /// One of [`CONTRIBUTIONS`].
    pub contribution: String,
    /// The season's build list, such as `1.0.0 (15)`.
    pub builds: Vec<String>,
    /// `bug`: the severities this quest pays for.
    pub severities: Vec<String>,
    /// `session`: the script, such as `session-2`.
    pub script: Option<String>,
    /// `session`: `unmoderated`, `moderated`, or `group`.
    pub format: Option<String>,
    /// The most live awards this quest version may have.
    pub max_awards: u64,
}

impl PlaytestAcceptance {
    /// The report kinds a completion of this contribution may cite.
    #[must_use]
    pub fn report_kinds(&self) -> &'static [&'static str] {
        report_kinds(&self.contribution)
    }

    /// Whether the contribution needs a moderator's session record.
    #[must_use]
    pub fn needs_session_record(&self) -> bool {
        matches!(self.format.as_deref(), Some("moderated" | "group"))
    }

    /// Whether the contribution cites a public issue.
    #[must_use]
    pub fn needs_issue(&self) -> bool {
        matches!(
            self.contribution.as_str(),
            "feedback" | "bug" | "design" | "verified-fix"
        )
    }
}

/// The report kinds `contribution` may cite.
#[must_use]
pub fn report_kinds(contribution: &str) -> &'static [&'static str] {
    match contribution {
        "feedback" => &["bug", "confusing", "idea", "felt-good", "comment"],
        "bug" => &["bug"],
        "design" => &["bug", "confusing", "idea"],
        "verified-fix" => &["verified"],
        "session" => &["session"],
        "diary" => &["diary"],
        _ => &[],
    }
}

/// The award fields only `playtest` has.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlaytestAward {
    /// The public issue that records the acceptance, `owner/repo#n`.
    pub issue: Option<String>,
    /// `bug`: the triager's severity.
    pub severity: Option<String>,
    /// `design`: the commit that shipped the change, 40 lowercase hex.
    pub commit: Option<String>,
}

/// A verified `3197`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaytestReport {
    pub build: String,
    pub platform: String,
    /// One of [`REPORT_KINDS`].
    pub kind: String,
    /// Lowercase hex SHA-256 of the private report's exact bytes.
    pub digest: String,
    /// The session script the report is about, if any.
    pub script: Option<String>,
}

/// A verified `3196`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaytestSession {
    pub script: String,
    /// `moderated` or `group`.
    pub format: String,
    pub build: String,
    /// The tester's hex public key.
    pub tester: String,
    pub held_at: u64,
}

fn valid_build(value: &str) -> bool {
    !value.trim().is_empty()
        && value.chars().count() <= 32
        && value.chars().all(|c| !c.is_control())
        && value.trim() == value
}

fn valid_script(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Whether `value` is `owner/repo#number`.
#[must_use]
pub fn valid_issue(value: &str) -> bool {
    let Some((repo, number)) = value.split_once('#') else {
        return false;
    };
    let Some((owner, name)) = repo.split_once('/') else {
        return false;
    };
    let part = |s: &str| {
        !s.is_empty()
            && s.len() <= 100
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    };
    part(owner)
        && part(name)
        && !number.is_empty()
        && number.len() <= 10
        && !number.starts_with('0')
        && number.chars().all(|c| c.is_ascii_digit())
}

fn string_list(value: &Value, what: &str, max: usize) -> Result<Vec<String>, ContractError> {
    let items = value.as_array().ok_or_else(|| malformed(what))?;
    if items.is_empty() || items.len() > max {
        return Err(malformed(what));
    }
    let mut out = Vec::new();
    for item in items {
        let text = item.as_str().ok_or_else(|| malformed(what))?.to_string();
        if out.contains(&text) {
            return Err(malformed(format!("{what} repeats {text}")));
        }
        out.push(text);
    }
    Ok(out)
}

fn one_of(value: &str, allowed: &[&str], what: &str) -> Result<(), ContractError> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(ContractError::new(
            RefusalCode::UnsupportedFeature,
            format!("{what} {value}"),
        ))
    }
}

/// Parses a `playtest` quest's `acceptance` object.
pub(crate) fn acceptance(object: &Map<String, Value>) -> Result<PlaytestAcceptance, ContractError> {
    let contribution = text(object, "contribution")?;
    one_of(&contribution, CONTRIBUTIONS, "acceptance.contribution")?;
    let mut keys = vec!["rule", "contribution", "builds", "max_awards"];
    match contribution.as_str() {
        "bug" => keys.push("severities"),
        "session" => keys.extend(["script", "format"]),
        _ => {}
    }
    reject(object, &keys)?;
    let builds = string_list(require(object, "builds")?, "acceptance.builds", MAX_BUILDS)?;
    if builds.iter().any(|b| !valid_build(b)) {
        return Err(malformed("acceptance.builds"));
    }
    let max_awards = number(object, "max_awards")?;
    if !(1..=MAX_AWARDS).contains(&max_awards) {
        return Err(malformed("acceptance.max_awards"));
    }
    let severities = if contribution == "bug" {
        let list = string_list(
            require(object, "severities")?,
            "acceptance.severities",
            SEVERITIES.len(),
        )?;
        for severity in &list {
            one_of(severity, SEVERITIES, "acceptance.severities")?;
        }
        list
    } else {
        Vec::new()
    };
    let (script, format) = if contribution == "session" {
        let script = text(object, "script")?;
        if !valid_script(&script) {
            return Err(malformed("acceptance.script"));
        }
        let format = text(object, "format")?;
        one_of(&format, FORMATS, "acceptance.format")?;
        (Some(script), Some(format))
    } else {
        (None, None)
    };
    Ok(PlaytestAcceptance {
        contribution,
        builds,
        severities,
        script,
        format,
        max_awards,
    })
}

/// Reads the award fields only `playtest` has; [`bind_fields`] checks
/// them against the quest.
pub(crate) fn award_fields(object: &Map<String, Value>) -> Result<PlaytestAward, ContractError> {
    let optional = |key: &str| -> Result<Option<String>, ContractError> {
        match object.get(key) {
            None => Ok(None),
            Some(value) => value
                .as_str()
                .map(|s| Some(s.to_string()))
                .ok_or_else(|| malformed(key)),
        }
    };
    let fields = PlaytestAward {
        issue: optional("issue")?,
        severity: optional("severity")?,
        commit: optional("commit")?,
    };
    if fields.issue.as_deref().is_some_and(|i| !valid_issue(i)) {
        return Err(malformed("issue"));
    }
    if let Some(severity) = &fields.severity {
        one_of(severity, SEVERITIES, "severity")?;
    }
    if fields
        .commit
        .as_deref()
        .is_some_and(|c| c.len() != 40 || !c.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')))
    {
        return Err(malformed("commit"));
    }
    Ok(fields)
}

/// Checks that `key` has the shape of a playtest key; [`bind_fields`]
/// re-derives the exact key from the quest.
pub(crate) fn check_key_shape(key: &str) -> Result<(), ContractError> {
    if !key.starts_with(KEY_PREFIX)
        || key.len() > 320
        || key.chars().any(|c| c.is_whitespace() || c.is_control())
        || key.split(':').count() < 4
    {
        return Err(malformed("key"));
    }
    Ok(())
}

/// The award's uniqueness key under `playtest`:
///
/// - `feedback`, `bug`, `design`: `playtest:<season>:report:<issue>`, so
///   one issue earns one report-class award from a referee, whichever
///   quest pays it.
/// - `verified-fix`: `playtest:<season>:verified:<issue>`.
/// - `session`, `diary`: `playtest:<season>:<quest address>:<tester>`, so
///   each tester earns a script or a diary once per season.
///
/// # Errors
///
/// When a contribution that needs an issue has none.
pub fn key(quest: &Quest, tester: &str, issue: Option<&str>) -> Result<String, ContractError> {
    let accepted = quest
        .acceptance
        .playtest
        .as_ref()
        .ok_or_else(|| mismatch("the quest's rule isn't playtest"))?;
    let season = &quest.season.id;
    let issue = || issue.ok_or_else(|| malformed("issue"));
    Ok(match accepted.contribution.as_str() {
        "feedback" | "bug" | "design" => format!("{KEY_PREFIX}{season}:report:{}", issue()?),
        "verified-fix" => format!("{KEY_PREFIX}{season}:verified:{}", issue()?),
        _ => format!("{KEY_PREFIX}{season}:{}:{tester}", quest.address),
    })
}

/// The tester signed the report; the triager, for a session, signed the
/// session record; and the tester is neither the triager nor the referee.
pub(crate) fn check_awardees(
    referee: &str,
    tester: &Awardee,
    triager: &Awardee,
    evidence: &[Pointer],
) -> Result<(), ContractError> {
    if tester.pubkey != evidence[0].pubkey {
        return Err(mismatch("the tester awardee didn't sign the report"));
    }
    if let Some(record) = evidence.get(1)
        && triager.pubkey != record.pubkey
    {
        return Err(mismatch(
            "the triager awardee didn't sign the session record",
        ));
    }
    if tester.pubkey == triager.pubkey {
        return Err(ContractError::new(
            RefusalCode::NotAdmitted,
            "the tester is the triager: accepting your own contribution earns nothing",
        ));
    }
    if tester.pubkey == referee {
        return Err(ContractError::new(
            RefusalCode::NotAdmitted,
            "the tester is the referee: a referee never awards a key it controls",
        ));
    }
    Ok(())
}

/// Checks a parsed `playtest` award's own fields against its quest: the
/// issue, severity, and commit its contribution needs and nothing else,
/// the session record when the format needs one, and the key the rule
/// derives.
pub(crate) fn bind_fields(award: &Award, quest: &Quest) -> Result<(), ContractError> {
    let accepted = quest
        .acceptance
        .playtest
        .as_ref()
        .ok_or_else(|| mismatch("the quest's rule isn't playtest"))?;
    let fields = award.playtest.clone().unwrap_or_default();
    if accepted.needs_issue() != fields.issue.is_some() {
        return Err(malformed(if accepted.needs_issue() {
            "the contribution needs an issue"
        } else {
            "the contribution cites no issue"
        }));
    }
    let bug = accepted.contribution == "bug";
    match (&fields.severity, bug) {
        (Some(severity), true) if accepted.severities.contains(severity) => {}
        (Some(_), true) => {
            return Err(ContractError::new(
                RefusalCode::NotAdmitted,
                "the severity isn't one this quest pays for",
            ));
        }
        (None, true) => return Err(malformed("a bug award names its severity")),
        (Some(_), false) => return Err(malformed("only a bug award has a severity")),
        (None, false) => {}
    }
    if (accepted.contribution == "design") != fields.commit.is_some() {
        return Err(malformed("a design award, and only it, names its commit"));
    }
    let records = if accepted.needs_session_record() {
        2
    } else {
        1
    };
    if award.evidence.len() != records {
        return Err(mismatch(
            "a moderated or group session names its session record, and nothing else does",
        ));
    }
    let tester = award.role("tester").ok_or_else(|| malformed("awardees"))?;
    if award.key != key(quest, &tester.pubkey, fields.issue.as_deref())? {
        return Err(mismatch("key"));
    }
    Ok(())
}

/// The parts of a `3197` report for the private report whose exact bytes
/// hash to `digest`. It holds no text.
///
/// # Errors
///
/// When a field is outside its grammar.
pub fn playtest_report(
    build: &str,
    platform: &str,
    kind: &str,
    digest: &str,
    script: Option<&str>,
) -> Result<Unsigned, ContractError> {
    let content = json!({
        "v": 1, "requires": [], "type": "playtest-report",
        "build": build, "platform": platform, "kind": kind, "digest": digest,
        "script": script,
    });
    report_body(content.as_object().expect("an object"))?;
    Ok(Unsigned {
        kind: REPORT_KIND,
        tags: vec![tag(&["t", "oa:xp:playtest-report:v1"])],
        content: content.to_string(),
    })
}

fn report_body(object: &Map<String, Value>) -> Result<PlaytestReport, ContractError> {
    reject(
        object,
        &[
            "v", "requires", "type", "build", "platform", "kind", "digest", "script",
        ],
    )?;
    let report = PlaytestReport {
        build: text(object, "build")?,
        platform: text(object, "platform")?,
        kind: text(object, "kind")?,
        digest: text(object, "digest")?,
        script: match require(object, "script")? {
            Value::Null => None,
            value => Some(
                value
                    .as_str()
                    .ok_or_else(|| malformed("script"))?
                    .to_string(),
            ),
        },
    };
    if !valid_build(&report.build) {
        return Err(malformed("build"));
    }
    one_of(&report.platform, PLATFORMS, "platform")?;
    one_of(&report.kind, REPORT_KINDS, "kind")?;
    if !is_hex(&report.digest) {
        return Err(malformed("digest"));
    }
    if report.script.as_deref().is_some_and(|s| !valid_script(s)) {
        return Err(malformed("script"));
    }
    Ok(report)
}

/// Checks a signed `3197`.
///
/// # Errors
///
/// A bad signature, marker, or body.
pub fn parse_playtest_report(event: &Event) -> Result<PlaytestReport, ContractError> {
    let object = open(event, REPORT_KIND, "playtest-report")?;
    report_body(&object)
}

/// The parts of a `3196` session record the moderator signs.
///
/// # Errors
///
/// When a field is outside its grammar.
pub fn playtest_session(
    script: &str,
    format: &str,
    build: &str,
    tester: &str,
    held_at: u64,
) -> Result<Unsigned, ContractError> {
    let content = json!({
        "v": 1, "requires": [], "type": "playtest-session",
        "script": script, "format": format, "build": build, "tester": tester,
        "held_at": held_at,
    });
    session_body(content.as_object().expect("an object"))?;
    Ok(Unsigned {
        kind: SESSION_KIND,
        tags: vec![
            tag(&["t", "oa:xp:playtest-session:v1"]),
            tag(&["p", tester]),
        ],
        content: content.to_string(),
    })
}

fn session_body(object: &Map<String, Value>) -> Result<PlaytestSession, ContractError> {
    reject(
        object,
        &[
            "v", "requires", "type", "script", "format", "build", "tester", "held_at",
        ],
    )?;
    let session = PlaytestSession {
        script: text(object, "script")?,
        format: text(object, "format")?,
        build: text(object, "build")?,
        tester: text(object, "tester")?,
        held_at: number(object, "held_at")?,
    };
    if !valid_script(&session.script) {
        return Err(malformed("script"));
    }
    one_of(&session.format, &["moderated", "group"], "format")?;
    if !valid_build(&session.build) {
        return Err(malformed("build"));
    }
    if !is_hex(&session.tester) {
        return Err(malformed("tester"));
    }
    Ok(session)
}

/// Checks a signed `3196`.
///
/// # Errors
///
/// A bad signature, marker, body, or `p` tag.
pub fn parse_playtest_session(event: &Event) -> Result<PlaytestSession, ContractError> {
    let object = open(event, SESSION_KIND, "playtest-session")?;
    let session = session_body(&object)?;
    if one_tag(event, "p")? != session.tester {
        return Err(mismatch("p tag"));
    }
    if event.pubkey == session.tester {
        return Err(ContractError::new(
            RefusalCode::NotAdmitted,
            "the moderator is the tester",
        ));
    }
    Ok(session)
}

fn context(what: &str, error: ContractError) -> ContractError {
    ContractError::new(error.code, format!("{what}: {}", error.detail))
}

/// The `playtest` rule. A contribution completes the quest when all hold:
///
/// 1. `report` is a valid `3197` whose build is in the quest's build list,
///    whose kind the contribution accepts, and that was published inside
///    the season.
/// 2. For a session, the report names the quest's script; for a moderated
///    or group session, `session` is a valid `3196` by another key that
///    names the report's signer as tester, the same script and format, a
///    listed build, and a time inside the season. Any other contribution
///    names no session record.
///
/// # Errors
///
/// [`RefusalCode::NotAdmitted`] when the contribution fails the rule;
/// other codes when an event is invalid.
pub fn check_playtest(
    quest: &Quest,
    report: &Event,
    session: Option<&Event>,
) -> Result<(PlaytestReport, Option<PlaytestSession>), ContractError> {
    let accepted = quest
        .acceptance
        .playtest
        .as_ref()
        .ok_or_else(|| mismatch("the quest's rule isn't playtest"))?;
    let parsed = parse_playtest_report(report).map_err(|e| context("the report", e))?;
    let refuse = |why: String| Err(ContractError::new(RefusalCode::NotAdmitted, why));
    let season = &quest.season;
    if !accepted.builds.contains(&parsed.build) {
        return refuse(format!(
            "build {} isn't in season {}'s build list",
            parsed.build, season.id
        ));
    }
    if !accepted.report_kinds().contains(&parsed.kind.as_str()) {
        return refuse(format!(
            "a {} report isn't a {} contribution",
            parsed.kind, accepted.contribution
        ));
    }
    let inside = |t: u64| t >= season.opens_at && t <= season.closes_at;
    if !inside(report.created_at) {
        return refuse(format!("the report isn't inside season {}", season.id));
    }
    if accepted.script.is_some() && parsed.script != accepted.script {
        return refuse("the report isn't about the quest's script".into());
    }
    let record = match (accepted.needs_session_record(), session) {
        (false, None) => None,
        (false, Some(_)) => return Err(mismatch("this contribution names no session record")),
        (true, None) => {
            return Err(ContractError::new(
                RefusalCode::ContentUnavailable,
                "a moderated or group session needs its session record",
            ));
        }
        (true, Some(event)) => {
            let record =
                parse_playtest_session(event).map_err(|e| context("the session record", e))?;
            if record.tester != report.pubkey {
                return refuse("the session record names another tester".into());
            }
            if Some(&record.script) != accepted.script.as_ref()
                || Some(&record.format) != accepted.format.as_ref()
            {
                return refuse("the session record is another script or format".into());
            }
            if !accepted.builds.contains(&record.build) {
                return refuse(format!(
                    "the session's build {} isn't in the build list",
                    record.build
                ));
            }
            if !inside(record.held_at) || !inside(event.created_at) {
                return refuse(format!("the session isn't inside season {}", season.id));
            }
            Some(record)
        }
    };
    Ok((parsed, record))
}

/// The parts of a `3193` accepting the tester's `report` (and, for a
/// moderated or group session, the moderator's `session` record) for the
/// signed `quest`. `triager` is the key that accepted the report; for a
/// session with a record it must be the record's signer. `issue` is the
/// public issue that records the acceptance.
///
/// # Errors
///
/// When an event isn't valid, the contribution fails the rule, a field the
/// contribution needs is missing, or `accepted_at` is outside the season or
/// before the evidence.
#[allow(clippy::too_many_arguments)]
pub fn playtest_award(
    quest: &Event,
    report: &Event,
    session: Option<&Event>,
    triager: &str,
    fields: &PlaytestAward,
    accepted_at: u64,
) -> Result<Unsigned, ContractError> {
    let parsed = parse_quest(quest)?;
    if parsed.acceptance.rule != PLAYTEST {
        return Err(mismatch("the quest's rule isn't playtest"));
    }
    in_season(&parsed, accepted_at)?;
    check_playtest(&parsed, report, session)?;
    if report.created_at > accepted_at || session.is_some_and(|s| s.created_at > accepted_at) {
        return Err(mismatch("the evidence is newer than its acceptance"));
    }
    if !is_hex(triager) {
        return Err(malformed("triager"));
    }
    let coordinate = coordinate(&quest.pubkey, &parsed.address);
    let key = key(&parsed, &report.pubkey, fields.issue.as_deref())?;
    let mut evidence = vec![json!({"id": report.id, "pubkey": report.pubkey, "kind": REPORT_KIND})];
    let mut tags = vec![
        tag(&["t", "oa:xp:award:v1"]),
        tag(&["a", &coordinate]),
        tag(&["e", &quest.id]),
        tag(&["e", &report.id]),
    ];
    if let Some(record) = session {
        evidence.push(json!({"id": record.id, "pubkey": record.pubkey, "kind": SESSION_KIND}));
        tags.push(tag(&["e", &record.id]));
    }
    tags.push(tag(&["p", &report.pubkey]));
    tags.push(tag(&["p", triager]));
    let mut content = json!({
        "v": 1, "requires": [], "type": "award",
        "quest": {"id": quest.id, "pubkey": quest.pubkey, "kind": QUEST_KIND, "coordinate": coordinate},
        "key": key,
        "accepted_at": accepted_at,
        "evidence": evidence,
        "awardees": [
            {"role": "tester", "pubkey": report.pubkey, "xp": parsed.award["tester"]},
            {"role": "triager", "pubkey": triager, "xp": parsed.award["triager"]},
        ],
    });
    let object = content.as_object_mut().expect("an object");
    for (name, value) in [
        ("issue", &fields.issue),
        ("severity", &fields.severity),
        ("commit", &fields.commit),
    ] {
        if let Some(value) = value {
            object.insert(name.into(), json!(value));
        }
    }
    let unsigned = Unsigned {
        kind: AWARD_KIND,
        tags,
        content: content.to_string(),
    };
    // Check what a reader will check, before anyone signs it.
    let preview = Award {
        rule: PLAYTEST.into(),
        quest: Pointer {
            id: quest.id.clone(),
            pubkey: quest.pubkey.clone(),
        },
        coordinate: coordinate.clone(),
        key,
        accepted_at,
        entry: None,
        entry_version: None,
        evidence: std::iter::once(report)
            .chain(session)
            .map(|e| Pointer {
                id: e.id.clone(),
                pubkey: e.pubkey.clone(),
            })
            .collect(),
        awardees: vec![
            Awardee {
                role: "tester".into(),
                pubkey: report.pubkey.clone(),
                xp: parsed.award["tester"],
            },
            Awardee {
                role: "triager".into(),
                pubkey: triager.into(),
                xp: parsed.award["triager"],
            },
        ],
        playtest: Some(fields.clone()),
    };
    check_awardees(
        &quest.pubkey,
        &preview.awardees[0],
        &preview.awardees[1],
        &preview.evidence,
    )?;
    bind_fields(&preview, &parsed)?;
    Ok(unsigned)
}

/// Checks a parsed `playtest` award against the signed report and session
/// record it names, then the rule over them.
///
/// # Errors
///
/// As [`check_playtest`], and [`RefusalCode::IdentityMismatch`] when an
/// event isn't the one the award names.
pub fn bind_playtest(
    award: &Award,
    quest: &Quest,
    report: &Event,
    session: Option<&Event>,
) -> Result<(), ContractError> {
    if award.rule != PLAYTEST {
        return Err(mismatch("the award isn't a playtest award"));
    }
    let named =
        |pointer: &Pointer, event: &Event| pointer.id == event.id && pointer.pubkey == event.pubkey;
    if !named(&award.evidence[0], report) {
        return Err(mismatch("report"));
    }
    match (award.evidence.get(1), session) {
        (None, None) => {}
        (Some(pointer), Some(event)) if named(pointer, event) => {}
        _ => return Err(mismatch("session record")),
    }
    check_playtest(quest, report, session)?;
    if report.created_at > award.accepted_at
        || session.is_some_and(|s| s.created_at > award.accepted_at)
    {
        return Err(mismatch("the evidence is newer than its acceptance"));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
