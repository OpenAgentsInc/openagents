//! NIP-XP's `eval-check` rule (`nips/openagents/NIP-XP.md`, "`eval-check`"),
//! and the award shape it shares with `eval-adopt`.
//!
//! A **check** is a published extension evaluation result
//! ([`crate::eval_ext`]) that reruns another trainer's published result on
//! the same suite, subject, and subject lock, and cites it with the `check`
//! marker. A check that followed the protocol credits three people whether
//! it confirms or disputes the result: the **checker**, the **evaluator**
//! of the result, and the **suite author**. Credit is for verification
//! work, not for agreement; a dispute carries at least as much information
//! as a fourth confirmation. Adoption (`eval-adopt`) still needs a
//! confirming check.
//!
//! Uniqueness is rule-derived and per role: each award credits exactly one
//! role, and its key is `eval-check:<season>:<suite release>:<role>:<pubkey>`,
//! so a season pays each key at most once in each role per suite version,
//! however many checks it gets. When one key holds two roles in a
//! completion, it is paid once, in the larger role (the earlier role on a
//! tie). An inconclusive check, or a check of an inconclusive result, earns
//! nothing: neither is a verdict to verify.

use std::collections::BTreeSet;

use serde_json::{Map, Value, json};

use super::{
    AWARD_KIND, Award, Awardee, EVAL_ADOPT, EVAL_CHECK, MAX_AWARD, MAX_PER_AWARDEE, QUEST_KIND,
    Quest, coordinate, in_season, parse_quest, pointer, roles, tag_set, valid_address, valid_slug,
};
use crate::contracts::{ContractError, RefusalCode};
use crate::domain::Event;
use crate::eval_ext::{
    EventPointer, Linkage, Publication, Verdict, event_pointer, linkage, parse_publication,
    verified_trainer,
};
use crate::kb::{
    self, Pointer, Unsigned, is_hex, malformed, mismatch, number, one_tag, reject, require, tag,
    text,
};
use crate::kinds;

/// Every `eval-check` uniqueness key starts with this.
pub const KEY_PREFIX: &str = "eval-check:";

/// What an `eval-check` or `eval-adopt` quest pins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvalAcceptance {
    /// `eval-check`: the suite's NIP-EXT release.
    pub suite: Option<EventPointer>,
    /// The extension's NIP-EXT release.
    pub subject: EventPointer,
    /// `eval-adopt`: the defaults package, `<root pubkey>:<slug>`.
    pub defaults: Option<String>,
    /// `eval-check`: the most live awards the quest version pays.
    pub max_awards: Option<u64>,
}

/// One key a completion pays, in one role.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Payee {
    pub role: &'static str,
    pub pubkey: String,
}

/// An accepted `eval-check` completion.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckCompletion {
    pub result: Publication,
    pub check: Publication,
    /// The keys it pays, after role collapse, in role order.
    pub payees: Vec<Payee>,
}

pub(crate) fn acceptance(
    object: &Map<String, Value>,
    rule: &str,
) -> Result<EvalAcceptance, ContractError> {
    let subject = event_pointer(
        require(object, "subject")?,
        kinds::EXT_RELEASE,
        "acceptance.subject",
    )?;
    if rule == EVAL_CHECK {
        reject(object, &["rule", "suite", "subject", "max_awards"])?;
        let suite = event_pointer(
            require(object, "suite")?,
            kinds::EXT_RELEASE,
            "acceptance.suite",
        )?;
        let max_awards = number(object, "max_awards")?;
        if !(1..=MAX_PER_AWARDEE).contains(&max_awards) {
            return Err(malformed("acceptance.max_awards"));
        }
        return Ok(EvalAcceptance {
            suite: Some(suite),
            subject,
            defaults: None,
            max_awards: Some(max_awards),
        });
    }
    reject(object, &["rule", "defaults", "subject"])?;
    let defaults = text(object, "defaults")?;
    if !valid_package(&defaults) {
        return Err(malformed("acceptance.defaults"));
    }
    Ok(EvalAcceptance {
        suite: None,
        subject,
        defaults: Some(defaults),
        max_awards: None,
    })
}

/// `<64 hex>:<slug>`.
pub(crate) fn valid_package(value: &str) -> bool {
    value
        .split_once(':')
        .is_some_and(|(root, slug)| is_hex(root) && valid_slug(slug))
}

/// The eval rule a uniqueness key belongs to, by its prefix.
#[must_use]
pub fn rule_of_key(key: &str) -> Option<&'static str> {
    if key.starts_with(KEY_PREFIX) {
        Some(EVAL_CHECK)
    } else if key.starts_with(super::eval_adopt::KEY_PREFIX) {
        Some(EVAL_ADOPT)
    } else if key.starts_with(super::pylon_check::KEY_PREFIX) {
        Some(super::PYLON_CHECK)
    } else {
        None
    }
}

/// Checks a key's shape and returns its role and pubkey.
pub(crate) fn check_key_shape(key: &str) -> Result<(&str, &str), ContractError> {
    if key.starts_with(super::pylon_check::KEY_PREFIX) {
        return super::pylon_check::check_key_shape(key);
    }
    let parts: Vec<&str> = key.split(':').collect();
    let (role, pubkey) = match parts.as_slice() {
        ["eval-check", season, release, role, pubkey] if valid_slug(season) && is_hex(release) => {
            (*role, *pubkey)
        }
        ["eval-adopt", release, role, pubkey] if is_hex(release) => (*role, *pubkey),
        _ => return Err(malformed("key")),
    };
    let rule = rule_of_key(key).ok_or_else(|| malformed("key"))?;
    if !roles(rule).contains(&role) || !is_hex(pubkey) {
        return Err(malformed("key"));
    }
    Ok((role, pubkey))
}

fn eval_acceptance(quest: &Quest) -> Result<&EvalAcceptance, ContractError> {
    quest
        .acceptance
        .eval
        .as_ref()
        .ok_or_else(|| mismatch("the quest's rule isn't eval-check or eval-adopt"))
}

/// The uniqueness key of an award paying `pubkey` in `role` under `quest`:
/// `eval-check:<season>:<suite release>:<role>:<pubkey>`, or
/// `eval-adopt:<subject release>:<role>:<pubkey>`.
///
/// # Errors
///
/// When the quest isn't an eval quest or `role` isn't one of its rule's.
pub fn key(quest: &Quest, role: &str, pubkey: &str) -> Result<String, ContractError> {
    if quest.acceptance.rule == super::PYLON_CHECK {
        if role != super::pylon_check::ROLE {
            return Err(malformed("role or pubkey"));
        }
        return super::pylon_check::key(quest, pubkey);
    }
    let accepted = eval_acceptance(quest)?;
    let rule = quest.acceptance.rule.as_str();
    if !roles(rule).contains(&role) || !is_hex(pubkey) {
        return Err(malformed("role or pubkey"));
    }
    Ok(if rule == EVAL_CHECK {
        let suite = accepted.suite.as_ref().ok_or_else(|| malformed("suite"))?;
        format!(
            "{KEY_PREFIX}{}:{}:{role}:{pubkey}",
            quest.season.id, suite.id
        )
    } else {
        format!(
            "{}{}:{role}:{pubkey}",
            super::eval_adopt::KEY_PREFIX,
            accepted.subject.id
        )
    })
}

/// Pays each key once, in its largest role by the quest's award table (the
/// earlier role on a tie), and drops roles the quest pays nothing.
pub(crate) fn collapse(quest: &Quest, candidates: Vec<Payee>) -> Vec<Payee> {
    let order = roles(&quest.acceptance.rule);
    let rank = |role: &str| {
        let xp = quest.award.get(role).copied().unwrap_or(0);
        let index = order.iter().position(|r| *r == role).unwrap_or(usize::MAX);
        (xp, std::cmp::Reverse(index))
    };
    let mut best: std::collections::BTreeMap<String, &'static str> =
        std::collections::BTreeMap::new();
    for payee in candidates {
        let entry = best.entry(payee.pubkey).or_insert(payee.role);
        if rank(payee.role) > rank(entry) {
            *entry = payee.role;
        }
    }
    let mut payees: Vec<Payee> = best
        .into_iter()
        .filter(|(_, role)| quest.award.get(*role).copied().unwrap_or(0) > 0)
        .map(|(pubkey, role)| Payee { role, pubkey })
        .collect();
    payees.sort_by_key(|p| (order.iter().position(|r| *r == p.role), p.pubkey.clone()));
    payees
}

fn context(what: &str, error: ContractError) -> ContractError {
    ContractError::new(error.code, format!("{what}: {}", error.detail))
}

/// Whether `check` is a rerun of `result` that earns credit: it cites the
/// result with the `check` marker, its trainer is neither the result's
/// trainer nor the suite author, it is a check by [`linkage`] (confirming
/// or disputing), neither verdict is `inconclusive`, and it was published
/// after the result. Hosted results' requests must be among `requests` and
/// verify.
///
/// Returns the checker's and the result's trainers, and whether the check
/// confirms.
///
/// # Errors
///
/// [`RefusalCode::NotAdmitted`] when the check earns nothing; other codes
/// when an event isn't the one it must be.
pub fn credited_check(
    result: &Publication,
    check: &Publication,
    requests: &[Event],
) -> Result<(String, String, Linkage), ContractError> {
    let refuse = |why: &str| {
        Err(ContractError::new(
            RefusalCode::NotAdmitted,
            why.to_string(),
        ))
    };
    if check.checks.as_deref() != Some(result.id.as_str()) {
        return Err(mismatch(
            "the check doesn't cite the result with the check marker",
        ));
    }
    let evaluator = verified_trainer(result, requests).map_err(|e| context("the result", e))?;
    let checker = verified_trainer(check, requests).map_err(|e| context("the check", e))?;
    if checker == evaluator {
        return refuse(
            "the checker is the result's evaluator: checking your own result earns nothing",
        );
    }
    if checker == result.suite_author() {
        return refuse(
            "the checker wrote the suite: checking results on your own tests earns nothing",
        );
    }
    let linkage = match linkage(result, check) {
        linkage @ (Linkage::Confirm | Linkage::Dispute) => linkage,
        Linkage::NotACheck => {
            return refuse("not a check of this result: another suite, subject, or subject lock");
        }
    };
    if result.verdict() == Verdict::Inconclusive {
        return refuse("an inconclusive result has no verdict to verify");
    }
    if check.verdict() == Verdict::Inconclusive {
        return refuse("an inconclusive check verifies nothing");
    }
    if check.created_at <= result.created_at {
        return refuse("the check isn't newer than the result it checks");
    }
    Ok((checker.to_string(), evaluator.to_string(), linkage))
}

/// [`credited_check`] narrowed to a check that **confirms** the result:
/// what adoption counts and what the candidate policy's "confirmed by
/// three trainers" reads. A dispute earns credit but confirms nothing.
///
/// Returns the checker's and the result's trainers.
///
/// # Errors
///
/// As [`credited_check`], and [`RefusalCode::NotAdmitted`] for a dispute.
pub fn confirmed_check(
    result: &Publication,
    check: &Publication,
    requests: &[Event],
) -> Result<(String, String), ContractError> {
    let (checker, evaluator, linkage) = credited_check(result, check, requests)?;
    if linkage == Linkage::Dispute {
        return Err(ContractError::new(
            RefusalCode::NotAdmitted,
            "the check disputes the result: it earns credit but confirms nothing",
        ));
    }
    Ok((checker, evaluator))
}

/// The `eval-check` rule. A check completes the quest when all hold:
///
/// 1. `result` and `check` are valid result publications on the quest's
///    suite release and subject release.
/// 2. The check cites the result with the `check` marker and has the same
///    suite ArtifactRef, subject DefinitionRef, and subject-arm lock.
/// 3. The checker is neither the evaluator nor the suite author; a hosted
///    result's requester stands in for its evaluator, and its request is
///    among `requests` and verifies.
/// 4. Neither verdict is `inconclusive`. The check may confirm or dispute
///    the result: credit is for the rerun, not for agreement.
/// 5. The check was published after the result, and both inside the
///    season.
///
/// # Errors
///
/// [`RefusalCode::NotAdmitted`] when the completion fails the rule; other
/// codes when an event is invalid or isn't the one the quest names.
pub fn check_eval_check(
    quest: &Quest,
    result: &Event,
    check: &Event,
    requests: &[Event],
) -> Result<CheckCompletion, ContractError> {
    if quest.acceptance.rule != EVAL_CHECK {
        return Err(mismatch("the quest's rule isn't eval-check"));
    }
    let accepted = eval_acceptance(quest)?;
    let suite = accepted.suite.as_ref().ok_or_else(|| malformed("suite"))?;
    let parsed_result = parse_publication(result).map_err(|e| context("the result", e))?;
    let parsed_check = parse_publication(check).map_err(|e| context("the check", e))?;
    for (what, publication) in [("the result", &parsed_result), ("the check", &parsed_check)] {
        if publication.suite_release != *suite {
            return Err(mismatch(format!(
                "{what} ran another suite than the quest's"
            )));
        }
        if publication.subject_release.as_ref() != Some(&accepted.subject) {
            return Err(mismatch(format!(
                "{what} tested another subject than the quest's"
            )));
        }
    }
    let (checker, evaluator, _) = credited_check(&parsed_result, &parsed_check, requests)?;
    let season = &quest.season;
    for (what, event) in [("result", result), ("check", check)] {
        if event.created_at < season.opens_at || event.created_at > season.closes_at {
            return Err(ContractError::new(
                RefusalCode::NotAdmitted,
                format!("the {what} isn't inside season {}", season.id),
            ));
        }
    }
    let payees = collapse(
        quest,
        vec![
            Payee {
                role: "checker",
                pubkey: checker,
            },
            Payee {
                role: "evaluator",
                pubkey: evaluator,
            },
            Payee {
                role: "suite-author",
                pubkey: suite.pubkey.clone(),
            },
        ],
    );
    Ok(CheckCompletion {
        result: parsed_result,
        check: parsed_check,
        payees,
    })
}

pub(crate) fn award_parts(
    quest: &Event,
    parsed: &Quest,
    payee: &Payee,
    evidence: &[(&Event, u16)],
    accepted_at: u64,
) -> Result<Unsigned, ContractError> {
    let coordinate = coordinate(&quest.pubkey, &parsed.address);
    let key = key(parsed, payee.role, &payee.pubkey)?;
    let content = json!({
        "v": 1, "requires": [], "type": "award",
        "quest": {"id": quest.id, "pubkey": quest.pubkey, "kind": QUEST_KIND, "coordinate": coordinate},
        "key": key,
        "accepted_at": accepted_at,
        "evidence": evidence.iter().map(|(e, kind)| json!({"id": e.id, "pubkey": e.pubkey, "kind": kind})).collect::<Vec<_>>(),
        "awardees": [{"role": payee.role, "pubkey": payee.pubkey, "xp": parsed.award[payee.role]}],
    });
    let mut tags = vec![
        tag(&["t", "oa:xp:award:v1"]),
        tag(&["a", &coordinate]),
        tag(&["e", &quest.id]),
    ];
    for (event, _) in evidence {
        tags.push(tag(&["e", &event.id]));
    }
    tags.push(tag(&["p", &payee.pubkey]));
    Ok(Unsigned {
        kind: AWARD_KIND,
        tags,
        content: content.to_string(),
    })
}

/// The parts of one `3193` for each key the completion pays: each award
/// credits one role, names the result then the check, and carries the
/// rule-derived key. A referee skips a key it has already awarded. Awards
/// are only built when [`check_eval_check`] passes.
///
/// # Errors
///
/// When an event isn't valid, the completion fails the rule, or
/// `accepted_at` is outside the season or before the check.
pub fn eval_check_awards(
    quest: &Event,
    result: &Event,
    check: &Event,
    requests: &[Event],
    accepted_at: u64,
) -> Result<Vec<Unsigned>, ContractError> {
    let parsed = parse_quest(quest)?;
    in_season(&parsed, accepted_at)?;
    let completion = check_eval_check(&parsed, result, check, requests)?;
    if check.created_at > accepted_at {
        return Err(mismatch("the check is newer than its acceptance"));
    }
    let evidence = [(result, kb::EVIDENCE_KIND), (check, kb::EVIDENCE_KIND)];
    completion
        .payees
        .iter()
        .map(|payee| award_parts(quest, &parsed, payee, &evidence, accepted_at))
        .collect()
}

/// Checks a parsed `eval-check` award against the signed result and check
/// it names, then the rule over them, and that the rule pays its one
/// awardee in that role.
///
/// # Errors
///
/// As [`check_eval_check`], and [`RefusalCode::IdentityMismatch`] when an
/// event isn't the one the award names or the rule doesn't pay the
/// awardee in that role.
pub fn bind_eval_check(
    award: &Award,
    quest: &Quest,
    result: &Event,
    check: &Event,
    requests: &[Event],
) -> Result<(), ContractError> {
    if award.rule != EVAL_CHECK || award.evidence.len() != 2 {
        return Err(mismatch("the award isn't an eval-check award"));
    }
    let named = |p: &Pointer, e: &Event| p.id == e.id && p.pubkey == e.pubkey;
    if !named(&award.evidence[0], result) || !named(&award.evidence[1], check) {
        return Err(mismatch("the award names other events"));
    }
    let completion = check_eval_check(quest, result, check, requests)?;
    paid(award, &completion.payees)?;
    if check.created_at > award.accepted_at {
        return Err(mismatch("the check is newer than its acceptance"));
    }
    Ok(())
}

pub(crate) fn paid(award: &Award, payees: &[Payee]) -> Result<(), ContractError> {
    let [awardee] = award.awardees.as_slice() else {
        return Err(malformed("awardees"));
    };
    if !payees
        .iter()
        .any(|p| p.role == awardee.role && p.pubkey == awardee.pubkey)
    {
        return Err(mismatch(format!(
            "the rule doesn't pay {} as {}",
            awardee.pubkey, awardee.role
        )));
    }
    Ok(())
}

/// Reads a signed `3193` whose key is an eval rule's. Called by
/// [`super::parse_award`] after it has opened the event.
pub(crate) fn parse_award(
    event: &Event,
    object: &Map<String, Value>,
    rule: &'static str,
) -> Result<Award, ContractError> {
    reject(
        object,
        &[
            "v",
            "requires",
            "type",
            "quest",
            "key",
            "accepted_at",
            "evidence",
            "awardees",
        ],
    )?;
    let quest_value = require(object, "quest")?
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
    if one_tag(event, "a")? != coordinate_value {
        return Err(mismatch("a tag"));
    }
    let key = text(object, "key")?;
    let (key_role, key_pubkey) = check_key_shape(&key)?;
    let accepted_at = number(object, "accepted_at")?;
    let kinds_expected: &[u16] = if rule == super::PYLON_CHECK {
        super::pylon_check::EVIDENCE_KINDS
    } else if rule == EVAL_CHECK {
        &[kb::EVIDENCE_KIND, kb::EVIDENCE_KIND]
    } else {
        &[kinds::EXT_RELEASE, kb::EVIDENCE_KIND, kb::EVIDENCE_KIND]
    };
    let items = require(object, "evidence")?
        .as_array()
        .ok_or_else(|| malformed("evidence"))?;
    if items.len() != kinds_expected.len() {
        return Err(kb::unsupported("evidence count"));
    }
    let mut evidence = Vec::new();
    for (item, kind) in items.iter().zip(kinds_expected) {
        let item = item.as_object().ok_or_else(|| malformed("evidence"))?;
        reject(item, &["id", "pubkey", "kind"])?;
        evidence.push(pointer(item, *kind, "evidence")?);
    }
    let people = require(object, "awardees")?
        .as_array()
        .ok_or_else(|| malformed("awardees"))?;
    let [person] = people.as_slice() else {
        return Err(malformed(
            "awardees: an eval award credits exactly one role",
        ));
    };
    let person = person.as_object().ok_or_else(|| malformed("awardees"))?;
    reject(person, &["role", "pubkey", "xp"])?;
    let awardee = Awardee {
        role: text(person, "role")?,
        pubkey: text(person, "pubkey")?,
        xp: number(person, "xp")?,
    };
    if !roles(rule).contains(&awardee.role.as_str()) || !is_hex(&awardee.pubkey) {
        return Err(malformed("awardee"));
    }
    if awardee.role != key_role || awardee.pubkey != key_pubkey {
        return Err(mismatch("key: it names the awardee's role and key"));
    }
    if awardee.xp == 0 {
        return Err(malformed("awardees"));
    }
    if awardee.xp > MAX_AWARD {
        return Err(ContractError::new(RefusalCode::LimitExceeded, "awardees"));
    }
    let mut named: BTreeSet<&str> = BTreeSet::from([quest.id.as_str()]);
    named.extend(evidence.iter().map(|e| e.id.as_str()));
    if tag_set(event, "e") != named || event.tag_values("e").count() != named.len() {
        return Err(mismatch("e tags"));
    }
    if tag_set(event, "p") != BTreeSet::from([awardee.pubkey.as_str()])
        || event.tag_values("p").count() != 1
    {
        return Err(mismatch("p tags"));
    }
    Ok(Award {
        rule: rule.to_string(),
        quest,
        coordinate: coordinate_value,
        key,
        accepted_at,
        entry: None,
        entry_version: None,
        evidence,
        awardees: vec![awardee],
        playtest: None,
    })
}

/// The quest-bound checks of an eval award: its key is the one the quest
/// derives for its awardee.
pub(crate) fn bind_fields(award: &Award, quest: &Quest) -> Result<(), ContractError> {
    let [awardee] = award.awardees.as_slice() else {
        return Err(malformed("awardees"));
    };
    if award.key != key(quest, &awardee.role, &awardee.pubkey)? {
        return Err(mismatch(
            "the award's key isn't the one its quest derives for the awardee",
        ));
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests;
