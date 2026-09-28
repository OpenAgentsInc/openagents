//! NIP-XP v1: quests, awards, and revocations (`nips/openagents/NIP-XP.md`).
//!
//! A `30193` quest is one frozen version of a quest, at its own address. A
//! `3193` award is a referee's acceptance of one completion: it binds the
//! exact quest event, the entry version, the evidence, and the awardees,
//! under the quest version's uniqueness key. A `3194` revocation ends one
//! award. A NIP-32 label (`1985`) may point at an award for display, and
//! never carries XP itself.
//!
//! This module builds the unsigned parts of each and checks signed ones,
//! and it checks the two acceptance rules: `kb-transfer` against the NIP-KB
//! entry and NIP-EVAL evidence an award names, and `reproduce` against the
//! run evidence of a published attempt and its reproduction
//! ([`reproduce`]). Reading an entry's document to find the tasks it was
//! written from, and deciding which referees to trust, belong to the
//! reader.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

use crate::contracts::{ContractError, RefusalCode, parse_strict};
use crate::domain::{Event, Tag};
use crate::kb::{
    self, Pointer, Unsigned, is_hex, malformed, mismatch, number, one_tag, reject, require,
    requires_empty, t_values, tag, text, unsupported,
};

/// One frozen quest version, at its own address.
pub const QUEST_KIND: u16 = crate::kinds::XP_QUEST;
/// A referee's acceptance of one completion.
pub const AWARD_KIND: u16 = crate::kinds::XP_AWARD;
/// Irreversible revocation of one award.
pub const REVOCATION_KIND: u16 = crate::kinds::XP_REVOCATION;
/// A NIP-32 label.
pub const LABEL_KIND: u16 = 1_985;
/// The NIP-32 namespace achievements use.
pub const LABEL_NAMESPACE: &str = "openagents.xp";

/// The acceptance rules this version implements.
pub const RULES: &[&str] = &[KB_TRANSFER, REPRODUCE, PLAYTEST];
/// The rule a knowledge entry that helped out of sample completes.
pub const KB_TRANSFER: &str = "kb-transfer";
/// The rule an independent reproduction of a published attempt completes.
pub const REPRODUCE: &str = "reproduce";
/// The rule an accepted playtest contribution completes
/// ([`playtest`]).
pub const PLAYTEST: &str = "playtest";
/// The uniqueness policies this version implements. Under `first`, the
/// first accepted completion per uniqueness key earns the award: the key is
/// the quest version's coordinate, except under `playtest`, whose rule
/// derives it ([`playtest::key`]). Under [`PER_AWARDEE`], each distinct
/// key in the rule's keyed role earns it once, up to the quest's
/// `max_awards`.
pub const COMPLETIONS: &[&str] = &[FIRST, PER_AWARDEE];
/// The uniqueness policy that pays a quest version once.
pub const FIRST: &str = "first";
/// The uniqueness policy that pays each distinct awardee once, up to a
/// stated number of awards: tutorials and dailies.
pub const PER_AWARDEE: &str = "per-awardee";
/// The most awards a `per-awardee` quest version may state.
pub const MAX_PER_AWARDEE: u64 = 10_000;
/// The awardee roles of `kb-transfer`, in the order an award lists them.
pub const ROLES: &[&str] = &["author", "runner"];
/// The awardee roles of `reproduce`, in the order an award lists them.
pub const REPRODUCE_ROLES: &[&str] = &["claimant", "reproducer"];
/// The awardee roles of `playtest`, in the order an award lists them. For
/// a moderated or group session, the triager is the session's moderator.
pub const PLAYTEST_ROLES: &[&str] = &["tester", "triager"];

/// The role whose key a `per-awardee` quest pays once, under `rule`: the
/// reproducer under `reproduce`. Only rules with a keyed role take the
/// policy; every other role of such a quest carries 0 XP, so no key is
/// credited twice through a role every completion shares.
#[must_use]
pub fn keyed_role(rule: &str) -> Option<&'static str> {
    match rule {
        REPRODUCE => Some("reproducer"),
        _ => None,
    }
}

/// The awardee roles of `rule`, in the order an award lists them. An
/// unknown rule has none.
#[must_use]
pub fn roles(rule: &str) -> &'static [&'static str] {
    match rule {
        KB_TRANSFER => ROLES,
        REPRODUCE => REPRODUCE_ROLES,
        PLAYTEST => PLAYTEST_ROLES,
        _ => &[],
    }
}

/// The most XP one award may carry, all roles together.
pub const MAX_AWARD: u64 = 1_000;
/// Characters a quest title may have, at most.
pub const MAX_TITLE_CHARS: usize = 200;
/// Characters a quest objective or reference label may have, at most.
pub const MAX_TEXT_CHARS: usize = 4_000;
/// Characters a revocation reason may have, at most.
pub const MAX_REASON_CHARS: usize = kb::MAX_REASON_CHARS;

const QUEST_KEYS: &[&str] = &[
    "v",
    "requires",
    "type",
    "id",
    "version",
    "season",
    "title",
    "objective",
    "acceptance",
    "reference",
    "award",
    "completions",
    "max_awards",
];

/// A season: a slug and the Unix-second window awards must fall in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Season {
    pub id: String,
    pub opens_at: u64,
    pub closes_at: u64,
}

/// What a completion must show.
#[derive(Debug, Clone, PartialEq)]
pub struct Acceptance {
    /// `kb-transfer` or `reproduce`.
    pub rule: String,
    /// The task the evidence must pair on, as its report names it, or the
    /// task the reproduced attempt ran.
    pub task: String,
    /// `kb-transfer`: the lowest pass rate the with-entry arm may have on
    /// the task. `reproduce`: 1, since the reproduction must pass.
    pub min_pass_rate: f64,
    /// `kb-transfer`: the with-entry arm's cost per run must be below this,
    /// in dollars, when set. `None` under `reproduce`.
    pub max_usd_per_run: Option<f64>,
    /// `reproduce`: the lowercase hex SHA-256 of the recipe's canonical
    /// JSON (RFC 8785), which pins the task, harness, and model.
    pub recipe: Option<String>,
    /// `reproduce`: the exact `3189` run evidence of the published attempt.
    pub claim: Option<Pointer>,
    /// `playtest`: which contribution counts, and on which builds.
    pub playtest: Option<playtest::PlaytestAcceptance>,
}

/// The run a quest is measured against, for display and provenance.
#[derive(Debug, Clone, PartialEq)]
pub struct Reference {
    pub label: String,
    pub usd: Option<f64>,
    pub seconds: Option<u64>,
    /// Where the reference run is recorded: a path, a URL, or a digest.
    pub source: Option<String>,
}

/// A verified `30193`.
#[derive(Debug, Clone, PartialEq)]
pub struct Quest {
    /// The quest ID, without the version.
    pub id: String,
    pub version: u64,
    pub season: Season,
    pub title: String,
    pub objective: String,
    pub acceptance: Acceptance,
    pub reference: Option<Reference>,
    /// XP per role. The award is the sum; roles split it, never multiply it.
    pub award: BTreeMap<String, u64>,
    /// `first`: the first accepted completion per uniqueness key earns
    /// it. `per-awardee`: each distinct key in the rule's keyed role earns
    /// it once, up to `max_awards`.
    pub completions: String,
    /// `per-awardee`: the most live awards the quest version pays.
    pub max_awards: Option<u64>,
    /// The `d` tag: `<id>@<version>`.
    pub address: String,
}

impl Quest {
    /// The fixed XP of one accepted completion.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.award.values().sum()
    }

    /// The most live awards the quest version pays, when it states one:
    /// `max_awards` under `per-awardee`, or the `playtest` acceptance's.
    #[must_use]
    pub fn award_limit(&self) -> Option<u64> {
        self.max_awards.or_else(|| {
            self.acceptance
                .playtest
                .as_ref()
                .map(|accepted| accepted.max_awards)
        })
    }

    /// Whether each distinct awardee earns the award once.
    #[must_use]
    pub fn per_awardee(&self) -> bool {
        self.completions == PER_AWARDEE
    }
}

/// One credited key in an award.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Awardee {
    /// `author` or `runner` under `kb-transfer`; `claimant` or
    /// `reproducer` under `reproduce`.
    pub role: String,
    pub pubkey: String,
    pub xp: u64,
}

/// The exact entry version an award names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryVersionRef {
    pub id: String,
    pub version: u64,
    /// Lowercase hex SHA-256 of the entry document.
    pub digest: String,
}

/// A verified `3193`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Award {
    /// The rule the award's shape follows: `kb-transfer` or `reproduce`.
    /// The award doesn't state it; its awardee roles do, and
    /// [`bind_quest`] checks it against the quest.
    pub rule: String,
    /// The exact `30193` event.
    pub quest: Pointer,
    /// `30193:<referee>:<address>`.
    pub coordinate: String,
    /// The uniqueness key: at most one live award per key per referee.
    pub key: String,
    pub accepted_at: u64,
    /// The exact `3190` event, under `kb-transfer`.
    pub entry: Option<Pointer>,
    /// The entry version, under `kb-transfer`.
    pub entry_version: Option<EntryVersionRef>,
    /// The `3189` events: the runner's evidence under `kb-transfer`; the
    /// claim, then the reproduction, under `reproduce`.
    pub evidence: Vec<Pointer>,
    pub awardees: Vec<Awardee>,
    /// `playtest`: the public issue, severity, and commit the award cites.
    pub playtest: Option<playtest::PlaytestAward>,
}

impl Award {
    /// The XP the award carries, all roles together.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.awardees.iter().map(|a| a.xp).sum()
    }

    /// The awardee with `role`.
    #[must_use]
    pub fn role(&self, role: &str) -> Option<&Awardee> {
        self.awardees.iter().find(|a| a.role == role)
    }
}

/// A verified `3194`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revocation {
    pub award: Pointer,
    pub key: String,
    pub reason: String,
}

/// A verified achievement label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Achievement {
    /// The `3193` event ID the label points at.
    pub award: String,
    /// The label value, such as `beat-reference`.
    pub value: String,
    /// The `p` tags.
    pub awardees: Vec<String>,
}

/// The `d` tag of a quest version.
#[must_use]
pub fn address(id: &str, version: u64) -> String {
    format!("{id}@{version}")
}

/// The NIP-01 coordinate of a quest version by `referee`.
#[must_use]
pub fn coordinate(referee: &str, address: &str) -> String {
    format!("{QUEST_KIND}:{referee}:{address}")
}

/// The parts of a `30193` for the quest `spec`: every body field except
/// `v`, `requires`, and `type`, which this adds. `completions` defaults to
/// `first`.
///
/// # Errors
///
/// When the spec isn't a valid quest body.
pub fn quest(spec: &Value) -> Result<Unsigned, ContractError> {
    let mut object = spec.as_object().ok_or_else(|| malformed("quest"))?.clone();
    for key in ["v", "requires", "type"] {
        if object.contains_key(key) {
            return Err(malformed(format!("{key} is set by the builder")));
        }
    }
    object.insert("v".into(), json!(1));
    object.insert("requires".into(), json!([]));
    object.insert("type".into(), json!("quest"));
    object
        .entry("completions")
        .or_insert_with(|| json!("first"));
    let parsed = quest_body(&object)?;
    Ok(Unsigned {
        kind: QUEST_KIND,
        tags: vec![
            tag(&["d", &parsed.address]),
            tag(&["t", "oa:xp:quest:v1"]),
            tag(&["t", &format!("oa:xp:season:{}", parsed.season.id)]),
            tag(&["t", &format!("oa:xp:rule:{}", parsed.acceptance.rule)]),
        ],
        content: Value::Object(object).to_string(),
    })
}

/// Checks a signed `30193` and returns the quest it holds.
///
/// # Errors
///
/// A bad signature, a body this version doesn't implement, or a tag that
/// disagrees with the body.
pub fn parse_quest(event: &Event) -> Result<Quest, ContractError> {
    let object = open(event, QUEST_KIND, "quest")?;
    let quest = quest_body(&object)?;
    if one_tag(event, "d")? != quest.address {
        return Err(mismatch("d tag"));
    }
    let seasons: Vec<&str> = t_values(event)
        .filter_map(|t| t.strip_prefix("oa:xp:season:"))
        .collect();
    if seasons != [quest.season.id.as_str()] {
        return Err(mismatch("season tag"));
    }
    let rules: Vec<&str> = t_values(event)
        .filter_map(|t| t.strip_prefix("oa:xp:rule:"))
        .collect();
    if rules != [quest.acceptance.rule.as_str()] {
        return Err(mismatch("rule tag"));
    }
    Ok(quest)
}

fn quest_body(object: &Map<String, Value>) -> Result<Quest, ContractError> {
    reject(object, QUEST_KEYS)?;
    if object.get("v").and_then(Value::as_u64) != Some(1) {
        return Err(ContractError::new(RefusalCode::UnsupportedVersion, "v"));
    }
    requires_empty(object)?;
    if object.get("type").and_then(Value::as_str) != Some("quest") {
        return Err(mismatch("type"));
    }
    let id = text(object, "id")?;
    if !kb::valid_entry_id(&id) {
        return Err(malformed("id"));
    }
    let version = number(object, "version")?;
    if version == 0 {
        return Err(malformed("version"));
    }
    let season = season(require(object, "season")?)?;
    let title = bounded(object, "title", MAX_TITLE_CHARS)?;
    let objective = bounded(object, "objective", MAX_TEXT_CHARS)?;
    let acceptance = acceptance(require(object, "acceptance")?)?;
    let reference = match require(object, "reference")? {
        Value::Null => None,
        value => Some(reference(value)?),
    };
    let award = award_table(require(object, "award")?, roles(&acceptance.rule))?;
    let completions = text(object, "completions")?;
    if !COMPLETIONS.contains(&completions.as_str()) {
        return Err(unsupported("completions"));
    }
    let max_awards = if completions == PER_AWARDEE {
        let Some(keyed) = keyed_role(&acceptance.rule) else {
            return Err(unsupported(format!(
                "completions: per-awardee doesn't apply to {}",
                acceptance.rule
            )));
        };
        let max_awards = number(object, "max_awards")?;
        if !(1..=MAX_PER_AWARDEE).contains(&max_awards) {
            return Err(malformed("max_awards"));
        }
        if award.iter().any(|(role, xp)| role != keyed && *xp > 0) {
            return Err(malformed(format!(
                "award: under per-awardee only the {keyed} earns XP"
            )));
        }
        Some(max_awards)
    } else {
        if object.contains_key("max_awards") {
            return Err(malformed("max_awards is for per-awardee quests"));
        }
        None
    };
    Ok(Quest {
        address: address(&id, version),
        id,
        version,
        season,
        title,
        objective,
        acceptance,
        reference,
        award,
        completions,
        max_awards,
    })
}

/// The uniqueness key of an award for `quest` by `referee`, where
/// `keyed` is the awardee in the rule's keyed role: the quest version's
/// coordinate under `first`, and `<coordinate>:<awardee pubkey>` under
/// `per-awardee`. A `playtest` key comes from [`playtest::key`] instead.
#[must_use]
pub fn uniqueness_key(referee: &str, quest: &Quest, keyed: &str) -> String {
    let coordinate = coordinate(referee, &quest.address);
    if quest.per_awardee() {
        format!("{coordinate}:{keyed}")
    } else {
        coordinate
    }
}

fn bounded(object: &Map<String, Value>, key: &str, max: usize) -> Result<String, ContractError> {
    let value = text(object, key)?;
    if value.trim().is_empty() {
        return Err(malformed(key));
    }
    if value.chars().count() > max {
        return Err(ContractError::new(RefusalCode::LimitExceeded, key));
    }
    Ok(value)
}

fn season(value: &Value) -> Result<Season, ContractError> {
    let object = value.as_object().ok_or_else(|| malformed("season"))?;
    reject(object, &["id", "opens_at", "closes_at"])?;
    let id = text(object, "id")?;
    if !valid_slug(&id) {
        return Err(malformed("season.id"));
    }
    let opens_at = number(object, "opens_at")?;
    let closes_at = number(object, "closes_at")?;
    if opens_at >= closes_at {
        return Err(malformed("season window"));
    }
    Ok(Season {
        id,
        opens_at,
        closes_at,
    })
}

fn acceptance(value: &Value) -> Result<Acceptance, ContractError> {
    let object = value.as_object().ok_or_else(|| malformed("acceptance"))?;
    let rule = text(object, "rule")?;
    if !RULES.contains(&rule.as_str()) {
        return Err(unsupported("acceptance.rule"));
    }
    if rule == PLAYTEST {
        let parsed = playtest::acceptance(object)?;
        return Ok(Acceptance {
            rule,
            task: parsed.contribution.clone(),
            min_pass_rate: 1.0,
            max_usd_per_run: None,
            recipe: None,
            claim: None,
            playtest: Some(parsed),
        });
    }
    if rule == REPRODUCE {
        reject(object, &["rule", "task", "recipe", "claim"])?;
    } else {
        reject(
            object,
            &["rule", "task", "min_pass_rate", "max_usd_per_run"],
        )?;
    }
    let task = text(object, "task")?;
    if task.is_empty() || task.len() > 256 || task.chars().any(char::is_whitespace) {
        return Err(malformed("acceptance.task"));
    }
    if rule == REPRODUCE {
        let recipe = text(object, "recipe")?;
        if !is_hex(&recipe) {
            return Err(malformed("acceptance.recipe"));
        }
        let claim = require(object, "claim")?
            .as_object()
            .ok_or_else(|| malformed("acceptance.claim"))?;
        reject(claim, &["id", "pubkey", "kind"])?;
        let claim = pointer(claim, kb::EVIDENCE_KIND, "acceptance.claim")?;
        return Ok(Acceptance {
            rule,
            task,
            min_pass_rate: 1.0,
            max_usd_per_run: None,
            recipe: Some(recipe),
            claim: Some(claim),
            playtest: None,
        });
    }
    let min_pass_rate = require(object, "min_pass_rate")?
        .as_f64()
        .filter(|r| (0.0..=1.0).contains(r))
        .ok_or_else(|| malformed("acceptance.min_pass_rate"))?;
    let max_usd_per_run = match require(object, "max_usd_per_run")? {
        Value::Null => None,
        value => Some(
            value
                .as_f64()
                .filter(|usd| usd.is_finite() && *usd > 0.0)
                .ok_or_else(|| malformed("acceptance.max_usd_per_run"))?,
        ),
    };
    Ok(Acceptance {
        rule,
        task,
        min_pass_rate,
        max_usd_per_run,
        recipe: None,
        claim: None,
        playtest: None,
    })
}

fn reference(value: &Value) -> Result<Reference, ContractError> {
    let object = value.as_object().ok_or_else(|| malformed("reference"))?;
    reject(object, &["label", "usd", "seconds", "source"])?;
    let label = bounded(object, "label", MAX_TEXT_CHARS)?;
    let usd = match require(object, "usd")? {
        Value::Null => None,
        value => Some(
            value
                .as_f64()
                .filter(|usd| usd.is_finite() && *usd >= 0.0)
                .ok_or_else(|| malformed("reference.usd"))?,
        ),
    };
    let seconds = match require(object, "seconds")? {
        Value::Null => None,
        value => Some(
            value
                .as_u64()
                .ok_or_else(|| malformed("reference.seconds"))?,
        ),
    };
    let source = match require(object, "source")? {
        Value::Null => None,
        value => Some(
            value
                .as_str()
                .filter(|s| !s.is_empty() && s.chars().count() <= MAX_TEXT_CHARS)
                .ok_or_else(|| malformed("reference.source"))?
                .to_string(),
        ),
    };
    Ok(Reference {
        label,
        usd,
        seconds,
        source,
    })
}

fn award_table(value: &Value, roles: &[&str]) -> Result<BTreeMap<String, u64>, ContractError> {
    let object = value.as_object().ok_or_else(|| malformed("award"))?;
    reject(object, roles)?;
    let mut table = BTreeMap::new();
    for role in roles {
        table.insert((*role).to_string(), number(object, role)?);
    }
    let total: u64 = table.values().sum();
    if total == 0 {
        return Err(malformed("award"));
    }
    if total > MAX_AWARD {
        return Err(ContractError::new(RefusalCode::LimitExceeded, "award"));
    }
    Ok(table)
}

/// The parts of a `3193` accepting one completion of the signed `quest`:
/// the signed `3190` `entry`, shown by the signed `3189` `evidence`.
/// `excluded_tasks` are the tasks the entry was written from, which the
/// reader reads from the entry's document. The award is only built when
/// [`check_transfer`] passes.
///
/// # Errors
///
/// When an event isn't valid, the completion fails the quest's rule, or
/// `accepted_at` is outside the season.
pub fn award(
    quest: &Event,
    entry: &Event,
    evidence: &Event,
    excluded_tasks: &[String],
    accepted_at: u64,
) -> Result<Unsigned, ContractError> {
    let parsed = parse_quest(quest)?;
    if parsed.acceptance.rule != KB_TRANSFER {
        return Err(mismatch("the quest's rule isn't kb-transfer"));
    }
    in_season(&parsed, accepted_at)?;
    let version = check_transfer(&parsed, entry, evidence, excluded_tasks)?;
    let coordinate = coordinate(&quest.pubkey, &parsed.address);
    let awardees: Vec<Value> = ROLES
        .iter()
        .map(|role| {
            let pubkey = if *role == "author" {
                &entry.pubkey
            } else {
                &evidence.pubkey
            };
            json!({"role": role, "pubkey": pubkey, "xp": parsed.award[*role]})
        })
        .collect();
    let content = json!({
        "v": 1, "requires": [], "type": "award",
        "quest": {"id": quest.id, "pubkey": quest.pubkey, "kind": QUEST_KIND, "coordinate": coordinate},
        "key": coordinate,
        "accepted_at": accepted_at,
        "entry": {"id": entry.id, "pubkey": entry.pubkey, "kind": kb::ENTRY_KIND},
        "entry_version": {"id": version.id, "version": version.version, "digest": version.digest},
        "evidence": [{"id": evidence.id, "pubkey": evidence.pubkey, "kind": kb::EVIDENCE_KIND}],
        "awardees": awardees,
    });
    Ok(Unsigned {
        kind: AWARD_KIND,
        tags: vec![
            tag(&["t", "oa:xp:award:v1"]),
            tag(&["a", &coordinate]),
            tag(&["e", &quest.id]),
            tag(&["e", &entry.id]),
            tag(&["e", &evidence.id]),
            tag(&["p", &entry.pubkey]),
            tag(&["p", &evidence.pubkey]),
        ],
        content: content.to_string(),
    })
}

/// Checks a signed `3193` on its own: signature, body, tags, and that it
/// names its own quest, awardees in its rule's order who are different
/// keys, and each awardee as the signer of the event the role names. The
/// awardee roles say which rule the award follows: `author` first under
/// `kb-transfer`, `claimant` first under `reproduce`.
///
/// # Errors
///
/// A typed refusal naming the first check that failed.
pub fn parse_award(event: &Event) -> Result<Award, ContractError> {
    let object = open(event, AWARD_KIND, "award")?;
    let first_role = require(&object, "awardees")?
        .as_array()
        .and_then(|a| a.first())
        .and_then(|a| a.get("role"))
        .and_then(Value::as_str)
        .ok_or_else(|| malformed("awardees"))?;
    let rule = match first_role {
        "author" => KB_TRANSFER,
        "claimant" => REPRODUCE,
        "tester" => PLAYTEST,
        _ => return Err(unsupported("awardee roles")),
    };
    let mut allowed = vec![
        "v",
        "requires",
        "type",
        "quest",
        "key",
        "accepted_at",
        "evidence",
        "awardees",
    ];
    if rule == KB_TRANSFER {
        allowed.extend(["entry", "entry_version"]);
    }
    if rule == PLAYTEST {
        allowed.extend(["issue", "severity", "commit"]);
    }
    reject(&object, &allowed)?;
    let quest_value = require(&object, "quest")?
        .as_object()
        .ok_or_else(|| malformed("quest"))?;
    reject(quest_value, &["id", "pubkey", "kind", "coordinate"])?;
    let quest = pointer(quest_value, QUEST_KIND, "quest")?;
    if quest.pubkey != event.pubkey {
        return Err(mismatch("the award's signer isn't the quest's referee"));
    }
    let coordinate_value = text(quest_value, "coordinate")?;
    let address = coordinate_value
        .strip_prefix(&format!("{QUEST_KIND}:{}:", quest.pubkey))
        .ok_or_else(|| mismatch("quest.coordinate"))?;
    valid_address(address)?;
    let key = text(&object, "key")?;
    // A `playtest` key is derived by its rule from the quest and the
    // contribution; [`bind_quest`] re-derives it. Every other key is the
    // quest version's coordinate.
    // A `per-awardee` key appends the keyed awardee's public key; it's
    // checked below, once the awardees are read.
    if rule == PLAYTEST {
        playtest::check_key_shape(&key)?;
    } else if key != coordinate_value
        && (keyed_role(rule).is_none() || !key.starts_with(&format!("{coordinate_value}:")))
    {
        return Err(mismatch("key"));
    }
    if one_tag(event, "a")? != coordinate_value {
        return Err(mismatch("a tag"));
    }
    let accepted_at = number(&object, "accepted_at")?;
    let (entry, entry_version) = if rule == KB_TRANSFER {
        let (entry, version) = entry_fields(&object)?;
        (Some(entry), Some(version))
    } else {
        (None, None)
    };
    let evidence_values = require(&object, "evidence")?
        .as_array()
        .ok_or_else(|| malformed("evidence"))?;
    // `kb-transfer` names exactly one evidence event, the runner's;
    // `reproduce` names two, the claim and then the reproduction;
    // `playtest` names the tester's report, then, for a moderated or group
    // session, the moderator's session record.
    let counts: &[usize] = match rule {
        KB_TRANSFER => &[1],
        REPRODUCE => &[2],
        _ => &[1, 2],
    };
    if !counts.contains(&evidence_values.len()) {
        return Err(unsupported("evidence count"));
    }
    let mut evidence = Vec::new();
    for (index, value) in evidence_values.iter().enumerate() {
        let item = value.as_object().ok_or_else(|| malformed("evidence"))?;
        reject(item, &["id", "pubkey", "kind"])?;
        let kind = match (rule, index) {
            (PLAYTEST, 0) => playtest::REPORT_KIND,
            (PLAYTEST, _) => playtest::SESSION_KIND,
            _ => kb::EVIDENCE_KIND,
        };
        evidence.push(pointer(item, kind, "evidence")?);
    }
    let awardees = awardees(require(&object, "awardees")?, roles(rule))?;
    let (first, second) = (&awardees[0], &awardees[1]);
    if rule == KB_TRANSFER {
        let entry = entry.as_ref().expect("kb-transfer names an entry");
        if first.pubkey != entry.pubkey {
            return Err(mismatch("the author awardee didn't sign the entry"));
        }
        if second.pubkey != evidence[0].pubkey {
            return Err(mismatch("the runner awardee didn't sign the evidence"));
        }
        if first.pubkey == second.pubkey {
            return Err(ContractError::new(
                RefusalCode::NotAdmitted,
                "the runner is the entry's author: self-evidence earns nothing",
            ));
        }
    } else if rule == PLAYTEST {
        playtest::check_awardees(&event.pubkey, first, second, &evidence)?;
    } else {
        if first.pubkey != evidence[0].pubkey {
            return Err(mismatch("the claimant awardee didn't sign the claim"));
        }
        if second.pubkey != evidence[1].pubkey {
            return Err(mismatch(
                "the reproducer awardee didn't sign the reproduction",
            ));
        }
        if first.pubkey == second.pubkey {
            return Err(ContractError::new(
                RefusalCode::NotAdmitted,
                "the reproducer is the claimant: reproducing your own attempt earns nothing",
            ));
        }
    }
    let playtest = if rule == PLAYTEST {
        Some(playtest::award_fields(&object)?)
    } else {
        None
    };
    if rule != PLAYTEST
        && key != coordinate_value
        && keyed_role(rule)
            .and_then(|role| awardees.iter().find(|a| a.role == role))
            .map(|a| format!("{coordinate_value}:{}", a.pubkey))
            != Some(key.clone())
    {
        return Err(mismatch("key: a per-awardee key names the keyed awardee"));
    }
    let total: u64 = awardees.iter().map(|a| a.xp).sum();
    if total == 0 {
        return Err(malformed("awardees"));
    }
    if total > MAX_AWARD {
        return Err(ContractError::new(RefusalCode::LimitExceeded, "awardees"));
    }
    let mut named: BTreeSet<&str> = BTreeSet::from([quest.id.as_str()]);
    named.extend(entry.iter().map(|e| e.id.as_str()));
    named.extend(evidence.iter().map(|e| e.id.as_str()));
    if tag_set(event, "e") != named || event.tag_values("e").count() != named.len() {
        return Err(mismatch("e tags"));
    }
    let people: BTreeSet<&str> = awardees.iter().map(|a| a.pubkey.as_str()).collect();
    if tag_set(event, "p") != people {
        return Err(mismatch("p tags"));
    }
    Ok(Award {
        rule: rule.to_string(),
        quest,
        coordinate: coordinate_value,
        key,
        accepted_at,
        entry,
        entry_version,
        evidence,
        awardees,
        playtest,
    })
}

fn entry_fields(object: &Map<String, Value>) -> Result<(Pointer, EntryVersionRef), ContractError> {
    let entry_value = require(object, "entry")?
        .as_object()
        .ok_or_else(|| malformed("entry"))?;
    reject(entry_value, &["id", "pubkey", "kind"])?;
    let entry = pointer(entry_value, kb::ENTRY_KIND, "entry")?;
    let version_value = require(object, "entry_version")?
        .as_object()
        .ok_or_else(|| malformed("entry_version"))?;
    reject(version_value, &["id", "version", "digest"])?;
    let entry_version = EntryVersionRef {
        id: text(version_value, "id")?,
        version: number(version_value, "version")?,
        digest: text(version_value, "digest")?,
    };
    if !kb::valid_entry_id(&entry_version.id)
        || entry_version.version == 0
        || !is_hex(&entry_version.digest)
    {
        return Err(malformed("entry_version"));
    }
    Ok((entry, entry_version))
}

fn awardees(value: &Value, roles: &[&str]) -> Result<Vec<Awardee>, ContractError> {
    let items = value.as_array().ok_or_else(|| malformed("awardees"))?;
    if items.len() != roles.len() {
        return Err(malformed("awardees"));
    }
    let mut out = Vec::new();
    for (item, role) in items.iter().zip(roles) {
        let object = item.as_object().ok_or_else(|| malformed("awardees"))?;
        reject(object, &["role", "pubkey", "xp"])?;
        let awardee = Awardee {
            role: text(object, "role")?,
            pubkey: text(object, "pubkey")?,
            xp: number(object, "xp")?,
        };
        if awardee.role != *role {
            return Err(malformed(format!(
                "awardee roles are {}, in that order",
                roles.join(", then ")
            )));
        }
        if !is_hex(&awardee.pubkey) {
            return Err(malformed("awardee pubkey"));
        }
        out.push(awardee);
    }
    Ok(out)
}

/// Binds a parsed award to the signed quest it names: the exact event, the
/// same referee, the season window, and the quest's fixed XP per role.
///
/// # Errors
///
/// [`RefusalCode::IdentityMismatch`] when the quest isn't the one named,
/// [`RefusalCode::Stale`] outside the season, and
/// [`RefusalCode::Conflict`] when the XP differs from the quest's table.
pub fn bind_quest(award: &Award, quest: &Event) -> Result<Quest, ContractError> {
    let parsed = parse_quest(quest)?;
    if award.quest.id != quest.id
        || award.quest.pubkey != quest.pubkey
        || award.coordinate != coordinate(&quest.pubkey, &parsed.address)
    {
        return Err(mismatch("quest"));
    }
    if award.rule != parsed.acceptance.rule {
        return Err(mismatch("the award's roles aren't its quest's rule"));
    }
    if let Some(claim) = &parsed.acceptance.claim
        && award.evidence.first() != Some(claim)
    {
        return Err(mismatch("the award's claim isn't the one its quest pins"));
    }
    in_season(&parsed, award.accepted_at)?;
    if award.rule == PLAYTEST {
        playtest::bind_fields(award, &parsed)?;
    } else {
        let keyed = keyed_role(&parsed.acceptance.rule)
            .and_then(|role| award.role(role))
            .map_or("", |a| a.pubkey.as_str());
        if award.key != uniqueness_key(&quest.pubkey, &parsed, keyed) {
            return Err(mismatch(format!(
                "the award's key isn't the one the quest's {} policy gives",
                parsed.completions
            )));
        }
    }
    for awardee in &award.awardees {
        if parsed.award.get(&awardee.role) != Some(&awardee.xp) {
            return Err(ContractError::new(
                RefusalCode::Conflict,
                format!("{} XP differs from the quest's award", awardee.role),
            ));
        }
    }
    Ok(parsed)
}

/// Checks a parsed award against the signed entry and evidence it names,
/// then the quest's rule over them. `excluded_tasks` are the tasks the
/// entry was written from.
///
/// # Errors
///
/// As [`check_transfer`], and [`RefusalCode::IdentityMismatch`] when an
/// event isn't the one the award names.
pub fn bind_evidence(
    award: &Award,
    quest: &Quest,
    entry: &Event,
    evidence: &Event,
    excluded_tasks: &[String],
) -> Result<(), ContractError> {
    let (Some(named_entry), Some(named_version)) = (&award.entry, &award.entry_version) else {
        return Err(mismatch("the award isn't a kb-transfer award"));
    };
    if named_entry.id != entry.id || named_entry.pubkey != entry.pubkey {
        return Err(mismatch("entry"));
    }
    let named = &award.evidence[0];
    if named.id != evidence.id || named.pubkey != evidence.pubkey {
        return Err(mismatch("evidence"));
    }
    let version = check_transfer(quest, entry, evidence, excluded_tasks)?;
    if version.id != named_version.id
        || version.version != named_version.version
        || version.digest != named_version.digest
    {
        return Err(mismatch("entry_version"));
    }
    if evidence.created_at > award.accepted_at {
        return Err(mismatch("the evidence is newer than its acceptance"));
    }
    Ok(())
}

/// The `kb-transfer` rule. The completion is accepted when all hold:
///
/// 1. `entry` is a valid `3190` and `evidence` a valid `3189` whose subject
///    is exactly that entry event: its EventRef, the entry's qualified ID
///    in its author's namespace, and the digest and size of its document.
/// 2. The evidence's signer (the runner) isn't the entry's author.
/// 3. The quest's task isn't one the entry was written from.
/// 4. The report's verdict is `pass`, and its paired tasks include the
///    quest's task with runs in both arms.
/// 5. On that task the with-entry arm passes at least `min_pass_rate` of
///    its runs and, when the quest sets `max_usd_per_run`, costs less per
///    run.
/// 6. The evidence was published inside the season.
///
/// Returns the entry version.
///
/// # Errors
///
/// [`RefusalCode::NotAdmitted`] when the completion fails the rule; other
/// codes when an event is invalid.
pub fn check_transfer(
    quest: &Quest,
    entry: &Event,
    evidence: &Event,
    excluded_tasks: &[String],
) -> Result<kb::EntryVersion, ContractError> {
    let version = kb::parse_entry(entry)?;
    let published = kb::parse_evidence(evidence)?;
    let subject = published
        .subject
        .event
        .as_ref()
        .ok_or_else(|| malformed("subject.event"))?;
    if subject.id != entry.id || subject.pubkey != entry.pubkey {
        return Err(mismatch("the evidence is about another entry"));
    }
    if published.subject.id != kb::qualified_id(&entry.pubkey, &version.id) {
        return Err(mismatch(
            "the report's subject isn't the entry's qualified ID in its author's namespace",
        ));
    }
    let artifact = &published.subject.artifact;
    if artifact.digest.trim_start_matches("sha256:") != version.digest
        || artifact.size != version.document.len() as u64
    {
        return Err(mismatch(
            "the report measured other document bytes than the entry version it cites",
        ));
    }
    let refuse = |why: String| Err(ContractError::new(RefusalCode::NotAdmitted, why));
    if evidence.pubkey == entry.pubkey {
        return refuse("the runner is the entry's author: self-evidence earns nothing".into());
    }
    let task = &quest.acceptance.task;
    if excluded_tasks.iter().any(|t| t == task) {
        return refuse(format!(
            "the entry was written from {task}, so evidence on it isn't out of sample"
        ));
    }
    let season = &quest.season;
    if evidence.created_at < season.opens_at || evidence.created_at > season.closes_at {
        return refuse(format!("the evidence isn't inside season {}", season.id));
    }
    let report = parse_strict(published.report_bytes.as_bytes())?;
    if report.get("verdict").and_then(Value::as_str) != Some("pass") {
        return refuse("the report's verdict isn't pass".into());
    }
    let pairs = report
        .pointer("/meta/kb/pairs")
        .and_then(Value::as_array)
        .ok_or_else(|| malformed("the report has no paired tasks"))?;
    let Some(pair) = pairs
        .iter()
        .find(|p| p.get("task").and_then(Value::as_str) == Some(task.as_str()))
    else {
        return refuse(format!("the report has no paired runs on {task}"));
    };
    let arm = |name: &str, field: &str| pair.get(name).and_then(|a| a.get(field));
    let with_runs = arm("with", "runs")
        .and_then(Value::as_u64)
        .ok_or_else(|| malformed("pair.with.runs"))?;
    let without_runs = arm("without", "runs")
        .and_then(Value::as_u64)
        .ok_or_else(|| malformed("pair.without.runs"))?;
    let passes = arm("with", "passes")
        .and_then(Value::as_u64)
        .ok_or_else(|| malformed("pair.with.passes"))?;
    let usd = arm("with", "usd")
        .and_then(Value::as_f64)
        .filter(|u| u.is_finite() && *u >= 0.0)
        .ok_or_else(|| malformed("pair.with.usd"))?;
    if with_runs == 0 || without_runs == 0 || passes > with_runs {
        return refuse(format!("{task} isn't paired: it needs runs in both arms"));
    }
    #[allow(clippy::cast_precision_loss)]
    let (rate, per_run) = (passes as f64 / with_runs as f64, usd / with_runs as f64);
    if rate < quest.acceptance.min_pass_rate {
        return refuse(format!(
            "the with-entry arm passes {passes} of {with_runs} runs on {task}, under the quest's {}",
            quest.acceptance.min_pass_rate
        ));
    }
    if let Some(bar) = quest.acceptance.max_usd_per_run
        && per_run >= bar
    {
        return refuse(format!(
            "the with-entry arm costs ${per_run:.4} per run on {task}, not under the quest's ${bar:.4}"
        ));
    }
    Ok(version)
}

/// The parts of a `3194` revoking the signed `3193` `award`.
///
/// # Errors
///
/// When `award` isn't a valid award or the reason is empty or too long.
pub fn revocation(award: &Event, reason: &str) -> Result<Unsigned, ContractError> {
    let parsed = parse_award(award)?;
    check_reason(reason)?;
    let content = json!({
        "v": 1, "requires": [], "type": "revocation",
        "award": {"id": award.id, "pubkey": award.pubkey, "kind": AWARD_KIND},
        "key": parsed.key,
        "reason": reason,
    });
    Ok(Unsigned {
        kind: REVOCATION_KIND,
        tags: vec![
            tag(&["t", "oa:xp:revocation:v1"]),
            tag(&["e", &award.id]),
            // The quest version's coordinate, which is the key except
            // under `playtest`.
            tag(&["a", &parsed.coordinate]),
        ],
        content: content.to_string(),
    })
}

/// Checks a signed `3194`. Only the award's own referee revokes it.
///
/// # Errors
///
/// A bad signature or body, a tag that disagrees with the body, or an
/// award signed by someone else.
pub fn parse_revocation(event: &Event) -> Result<Revocation, ContractError> {
    let object = open(event, REVOCATION_KIND, "revocation")?;
    reject(
        &object,
        &["v", "requires", "type", "award", "key", "reason"],
    )?;
    let value = require(&object, "award")?
        .as_object()
        .ok_or_else(|| malformed("award"))?;
    reject(value, &["id", "pubkey", "kind"])?;
    let award = pointer(value, AWARD_KIND, "award")?;
    if award.pubkey != event.pubkey {
        return Err(mismatch("only the award's referee revokes it"));
    }
    let key = text(&object, "key")?;
    let prefix = format!("{QUEST_KIND}:{}:", event.pubkey);
    let a = one_tag(event, "a")?;
    if key.starts_with(playtest::KEY_PREFIX) {
        // A playtest key is rule-derived; the `a` tag names the quest.
        playtest::check_key_shape(&key)?;
        valid_address(a.strip_prefix(&prefix).ok_or_else(|| mismatch("a tag"))?)?;
    } else {
        let rest = key.strip_prefix(&prefix).ok_or_else(|| mismatch("key"))?;
        // A per-awardee key is the coordinate, a colon, and the keyed
        // awardee's public key; the `a` tag names the quest version.
        let address = match rest.split_once(':') {
            Some((address, awardee)) if is_hex(awardee) => address,
            Some(_) => return Err(malformed("key")),
            None => rest,
        };
        valid_address(address)?;
        if a != format!("{prefix}{address}") {
            return Err(mismatch("a tag"));
        }
    }
    let reason = text(&object, "reason")?;
    check_reason(&reason)?;
    if one_tag(event, "e")? != award.id {
        return Err(mismatch("e tag"));
    }
    Ok(Revocation { award, key, reason })
}

/// The parts of a NIP-32 achievement label pointing at the signed `3193`
/// `award`, with the value `value`, such as `beat-reference`. The label
/// carries no XP; a reader shows it only while the award counts.
///
/// # Errors
///
/// When `award` isn't a valid award or `value` isn't a slug.
pub fn achievement(award: &Event, value: &str) -> Result<Unsigned, ContractError> {
    let parsed = parse_award(award)?;
    if !valid_slug(value) {
        return Err(malformed("label value"));
    }
    let mut tags = vec![
        tag(&["L", LABEL_NAMESPACE]),
        tag(&["l", value, LABEL_NAMESPACE]),
        tag(&["e", &award.id]),
    ];
    let people: BTreeSet<&str> = parsed.awardees.iter().map(|a| a.pubkey.as_str()).collect();
    for pubkey in people {
        tags.push(tag(&["p", pubkey]));
    }
    Ok(Unsigned {
        kind: LABEL_KIND,
        tags,
        content: String::new(),
    })
}

/// Checks a signed achievement label in the `openagents.xp` namespace.
/// Whether it shows depends on the award it names counting for the
/// reader, and on its signer being that award's referee.
///
/// # Errors
///
/// A bad signature, another namespace, or not exactly one label and one
/// award.
pub fn parse_achievement(event: &Event) -> Result<Achievement, ContractError> {
    if event.kind != LABEL_KIND {
        return Err(mismatch("kind"));
    }
    event
        .validate_crypto()
        .map_err(|_| mismatch("event signature"))?;
    let namespaces: Vec<&str> = event.tag_values("L").collect();
    if namespaces != [LABEL_NAMESPACE] {
        return Err(mismatch("L tag"));
    }
    let labels: Vec<&Tag> = event
        .tags
        .iter()
        .filter(|t| t.name() == Some("l"))
        .collect();
    let [label] = labels.as_slice() else {
        return Err(malformed("l tag"));
    };
    if label.0.get(2).map(String::as_str) != Some(LABEL_NAMESPACE) {
        return Err(mismatch("l namespace"));
    }
    let value = label.value().unwrap_or_default().to_string();
    if !valid_slug(&value) {
        return Err(malformed("label value"));
    }
    let award = one_tag(event, "e")?.to_string();
    if !is_hex(&award) {
        return Err(malformed("e tag"));
    }
    Ok(Achievement {
        award,
        value,
        awardees: event.tag_values("p").map(str::to_string).collect(),
    })
}

pub(crate) fn open(
    event: &Event,
    kind: u16,
    record: &str,
) -> Result<Map<String, Value>, ContractError> {
    if event.kind != kind {
        return Err(mismatch("kind"));
    }
    event
        .validate_crypto()
        .map_err(|_| mismatch("event signature"))?;
    let markers: Vec<&str> = t_values(event)
        .filter(|t| {
            t.starts_with("oa:xp:")
                && !t.starts_with("oa:xp:season:")
                && !t.starts_with("oa:xp:rule:")
        })
        .collect();
    if markers != [format!("oa:xp:{record}:v1").as_str()] {
        return Err(mismatch("xp tag"));
    }
    if t_values(event).any(|t| t.chars().any(char::is_uppercase)) {
        return Err(malformed("t tag"));
    }
    let value = parse_strict(event.content.as_bytes())?;
    let object = value.as_object().ok_or_else(|| malformed(record))?.clone();
    if object.get("v").and_then(Value::as_u64) != Some(1) {
        return Err(ContractError::new(RefusalCode::UnsupportedVersion, "v"));
    }
    requires_empty(&object)?;
    if object.get("type").and_then(Value::as_str) != Some(record) {
        return Err(mismatch("type"));
    }
    Ok(object)
}

pub(crate) fn pointer(
    object: &Map<String, Value>,
    kind: u16,
    what: &str,
) -> Result<Pointer, ContractError> {
    let pointer = Pointer {
        id: text(object, "id")?,
        pubkey: text(object, "pubkey")?,
    };
    if !is_hex(&pointer.id) || !is_hex(&pointer.pubkey) {
        return Err(malformed(what));
    }
    if object.get("kind").and_then(Value::as_u64) != Some(u64::from(kind)) {
        return Err(mismatch(format!("{what}.kind")));
    }
    Ok(pointer)
}

pub(crate) fn in_season(quest: &Quest, at: u64) -> Result<(), ContractError> {
    if at < quest.season.opens_at || at > quest.season.closes_at {
        return Err(ContractError::new(
            RefusalCode::Stale,
            format!("accepted outside season {}", quest.season.id),
        ));
    }
    Ok(())
}

fn check_reason(reason: &str) -> Result<(), ContractError> {
    if reason.trim().is_empty() {
        return Err(malformed("reason"));
    }
    if reason.chars().count() > MAX_REASON_CHARS {
        return Err(ContractError::new(RefusalCode::LimitExceeded, "reason"));
    }
    Ok(())
}

pub(crate) fn valid_address(address: &str) -> Result<(), ContractError> {
    let (id, version) = address
        .rsplit_once('@')
        .ok_or_else(|| malformed("quest address"))?;
    let version: u64 = version.parse().map_err(|_| malformed("quest address"))?;
    if !kb::valid_entry_id(id) || version == 0 || address != self::address(id, version) {
        return Err(malformed("quest address"));
    }
    Ok(())
}

pub(crate) fn valid_slug(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
        && value
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

pub(crate) fn tag_set<'a>(event: &'a Event, name: &str) -> BTreeSet<&'a str> {
    event
        .tags
        .iter()
        .filter(|t| t.name() == Some(name))
        .filter_map(Tag::value)
        .collect()
}

pub mod playtest;
pub use playtest::{
    PlaytestAcceptance, PlaytestAward, PlaytestReport, PlaytestSession, bind_playtest,
    check_playtest, parse_playtest_report, parse_playtest_session, playtest_award, playtest_report,
    playtest_session,
};
pub mod reproduce;
pub use reproduce::{
    RECIPE_SCHEMA, RUN_MARKER, Recipe, RunEvidence, RunRecord, bind_reproduction, check_reproduce,
    check_run_record, parse_run_evidence, recipe_digest, recipe_from_summary, record_from_summary,
    reproduce_award, run_evidence,
};

#[cfg(test)]
mod tests;
