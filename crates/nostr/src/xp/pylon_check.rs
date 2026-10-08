//! NIP-XP's `pylon-check` rule: XP for compute work a trusted checker
//! verified (`nips/openagents/NIP-PYLON.md`, "Check verdicts").
//!
//! A completion is one NIP-PYLON `3201` receipt for an accepted job and the
//! `check-pass` label (`1985`, `openagents.pylon`) the quest's checker
//! signed on it, made with the quest's pinned Gym suite. It credits the
//! pylon key that did the work, in the one role `provider`. Uniqueness is
//! rule-derived: `pylon-check:<season>:<suite>:provider:<pubkey>`, so a
//! season pays each pylon at most once per suite version, up to the quest's
//! `max_awards`. XP is a level, never money: nothing here or in NIP-PYLON
//! converts it to sats.

use serde_json::{Map, Value, json};

use super::{
    AWARD_KIND, Award, PYLON_CHECK, QUEST_KIND, Quest, coordinate, in_season, parse_quest,
};
use crate::contracts::{ContractError, RefusalCode};
use crate::domain::Event;
use crate::kb::{Pointer, Unsigned, is_hex, malformed, mismatch, number, reject, tag, text};
use crate::pylon::{self, Outcome, Verdict};

/// Every `pylon-check` uniqueness key starts with this.
pub const KEY_PREFIX: &str = "pylon-check:";
/// The one role a `pylon-check` award credits.
pub const ROLE: &str = "provider";

/// What a `pylon-check` quest pins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PylonAcceptance {
    /// The Gym suite's digest the check must name (`suite:<digest>`).
    pub suite: String,
    /// The checker whose `check-pass` counts.
    pub checker: String,
    /// The most live awards the quest version pays.
    pub max_awards: u64,
}

pub(crate) fn acceptance(object: &Map<String, Value>) -> Result<PylonAcceptance, ContractError> {
    reject(object, &["rule", "suite", "checker", "max_awards"])?;
    let suite = text(object, "suite")?;
    let checker = text(object, "checker")?;
    if !is_hex(&suite) {
        return Err(malformed("acceptance.suite"));
    }
    if !is_hex(&checker) {
        return Err(malformed("acceptance.checker"));
    }
    let max_awards = number(object, "max_awards")?;
    if !(1..=super::MAX_PER_AWARDEE).contains(&max_awards) {
        return Err(malformed("acceptance.max_awards"));
    }
    Ok(PylonAcceptance {
        suite,
        checker,
        max_awards,
    })
}

fn pylon_acceptance(quest: &Quest) -> Result<&PylonAcceptance, ContractError> {
    quest
        .acceptance
        .pylon
        .as_ref()
        .ok_or_else(|| mismatch("the quest's rule isn't pylon-check"))
}

/// The uniqueness key of an award paying the pylon `pubkey` under `quest`.
///
/// # Errors
///
/// When the quest isn't a `pylon-check` quest or `pubkey` isn't hex.
pub fn key(quest: &Quest, pubkey: &str) -> Result<String, ContractError> {
    let accepted = pylon_acceptance(quest)?;
    if !is_hex(pubkey) {
        return Err(malformed("pubkey"));
    }
    Ok(format!(
        "{KEY_PREFIX}{}:{}:{ROLE}:{pubkey}",
        quest.season.id, accepted.suite
    ))
}

/// Checks a key's shape and returns its role and pubkey.
pub(crate) fn check_key_shape(key: &str) -> Result<(&str, &str), ContractError> {
    match key.split(':').collect::<Vec<_>>().as_slice() {
        ["pylon-check", season, suite, role, pubkey]
            if super::valid_slug(season) && is_hex(suite) && *role == ROLE && is_hex(pubkey) =>
        {
            Ok((role, pubkey))
        }
        _ => Err(malformed("key")),
    }
}

/// The `pylon-check` rule. A receipt and a label complete the quest when
/// all hold:
///
/// 1. `receipt` is a valid `3201` for an accepted job, and `label` a valid
///    `check-pass` verdict that names exactly that receipt and its pylon.
/// 2. The label's signer is the quest's checker, who is neither the
///    receipt's buyer nor its provider.
/// 3. The label's method names the quest's suite (`suite:<digest>`).
/// 4. Both were published inside the season, the label after the receipt.
///
/// Returns the pylon key it pays.
///
/// # Errors
///
/// [`RefusalCode::NotAdmitted`] when the completion fails the rule; other
/// codes when an event is invalid.
pub fn check_pylon_check(
    quest: &Quest,
    receipt: &Event,
    label: &Event,
) -> Result<String, ContractError> {
    let accepted = pylon_acceptance(quest)?;
    let refuse = |why: &str| ContractError::new(RefusalCode::NotAdmitted, why.to_string());
    let parsed =
        pylon::parse_receipt(receipt, None).map_err(|e| malformed(format!("receipt: {e}")))?;
    let check = pylon::parse_check(label).map_err(|e| malformed(format!("label: {e}")))?;
    if check.receipt != receipt.id {
        return Err(mismatch("the label names another receipt"));
    }
    pylon::bind_check(&check, &parsed).map_err(|e| refuse(&e))?;
    if check.checker != accepted.checker {
        return Err(refuse("the label isn't from the quest's checker"));
    }
    if check.verdict != Verdict::Pass {
        return Err(refuse("only a check-pass verifies the work"));
    }
    if check.suite() != Some(accepted.suite.as_str()) {
        return Err(refuse("the check didn't run the quest's suite"));
    }
    if parsed.outcome != Outcome::Accepted {
        return Err(refuse("the receipt isn't for an accepted job"));
    }
    if label.created_at < receipt.created_at {
        return Err(refuse("the label is older than the receipt it checks"));
    }
    let season = &quest.season;
    for (what, event) in [("receipt", receipt), ("label", label)] {
        if event.created_at < season.opens_at || event.created_at > season.closes_at {
            return Err(refuse(&format!(
                "the {what} isn't inside season {}",
                season.id
            )));
        }
    }
    Ok(parsed.provider)
}

/// The parts of the `3193` paying the pylon for one verified job. The
/// award is only built when [`check_pylon_check`] passes.
///
/// # Errors
///
/// When an event isn't valid, the completion fails the rule, or
/// `accepted_at` is outside the season or before the label.
pub fn pylon_check_award(
    quest: &Event,
    receipt: &Event,
    label: &Event,
    accepted_at: u64,
) -> Result<Unsigned, ContractError> {
    let parsed = parse_quest(quest)?;
    in_season(&parsed, accepted_at)?;
    let provider = check_pylon_check(&parsed, receipt, label)?;
    if label.created_at > accepted_at {
        return Err(mismatch("the label is newer than its acceptance"));
    }
    let coordinate = coordinate(&quest.pubkey, &parsed.address);
    let content = json!({
        "v": 1, "requires": [], "type": "award",
        "quest": {"id": quest.id, "pubkey": quest.pubkey, "kind": QUEST_KIND, "coordinate": coordinate},
        "key": key(&parsed, &provider)?,
        "accepted_at": accepted_at,
        "evidence": [
            {"id": receipt.id, "pubkey": receipt.pubkey, "kind": pylon::RECEIPT_KIND},
            {"id": label.id, "pubkey": label.pubkey, "kind": pylon::CHECK_KIND},
        ],
        "awardees": [{"role": ROLE, "pubkey": provider, "xp": parsed.award[ROLE]}],
    });
    Ok(Unsigned {
        kind: AWARD_KIND,
        tags: vec![
            tag(&["t", "oa:xp:award:v1"]),
            tag(&["a", &coordinate]),
            tag(&["e", &quest.id]),
            tag(&["e", &receipt.id]),
            tag(&["e", &label.id]),
            tag(&["p", &provider]),
        ],
        content: content.to_string(),
    })
}

/// Checks a parsed `pylon-check` award against the signed receipt and label
/// it names, then the rule over them.
///
/// # Errors
///
/// As [`check_pylon_check`], and [`RefusalCode::IdentityMismatch`] when an
/// event isn't the one the award names or the rule pays someone else.
pub fn bind_pylon_check(
    award: &Award,
    quest: &Quest,
    receipt: &Event,
    label: &Event,
) -> Result<(), ContractError> {
    if award.rule != PYLON_CHECK || award.evidence.len() != 2 {
        return Err(mismatch("the award isn't a pylon-check award"));
    }
    let named = |p: &Pointer, e: &Event| p.id == e.id && p.pubkey == e.pubkey;
    if !named(&award.evidence[0], receipt) || !named(&award.evidence[1], label) {
        return Err(mismatch("the award names other events"));
    }
    let provider = check_pylon_check(quest, receipt, label)?;
    let [awardee] = award.awardees.as_slice() else {
        return Err(malformed("awardees"));
    };
    if awardee.pubkey != provider {
        return Err(mismatch("the rule pays the pylon that did the work"));
    }
    if label.created_at > award.accepted_at {
        return Err(mismatch("the label is newer than its acceptance"));
    }
    Ok(())
}

/// The evidence kinds a `pylon-check` award names, in order.
pub(crate) const EVIDENCE_KINDS: &[u16] = &[pylon::RECEIPT_KIND, pylon::CHECK_KIND];

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::domain::RelaySigner;
    use crate::pylon::{
        Lane, RECEIPT_V, Receipt, UnitKind, Units, check_event, receipt_event, sha256_hex,
    };
    use crate::xp::{bind_quest, parse_award, quest};

    const AT: u64 = 1_790_000_000;

    fn signer(byte: u8) -> RelaySigner {
        RelaySigner::from_secret_hex(&format!("{byte:02x}").repeat(32)).unwrap()
    }

    fn receipt(buyer: &RelaySigner, provider: &RelaySigner, outcome: Outcome) -> Event {
        let body = Receipt {
            v: RECEIPT_V.into(),
            requires: Vec::new(),
            meta: None,
            buyer: buyer.pubkey().into(),
            provider: provider.pubkey().into(),
            pylon: "studio".into(),
            lane: Lane::CjConversation,
            capability: format!("{}:pylon/text-generation", provider.pubkey()),
            request: "ab".repeat(32),
            request_digest: sha256_hex(b"request"),
            result_digest: Some(sha256_hex(b"result")),
            started_at: AT - 5,
            finished_at: AT - 1,
            units: Units {
                kind: UnitKind::Tokens,
                count: 12,
            },
            outcome,
            payment: None,
        };
        receipt_event(buyer, &body, AT).unwrap()
    }

    fn label(checker: &RelaySigner, verdict: Verdict, job: &Event, method: &str) -> Event {
        check_event(checker, verdict, job, &sha256_hex(b"42"), method, AT + 1).unwrap()
    }

    #[test]
    fn a_passing_check_earns_the_pylon_xp_and_nothing_else_does() {
        let (referee, checker, buyer, provider) = (signer(1), signer(2), signer(3), signer(4));
        let suite = sha256_hex(b"pinned suite");
        let spec = json!({
            "id": "pylon.verified-work",
            "version": 1,
            "season": {"id": "2026-q4", "opens_at": AT - 1_000, "closes_at": AT + 1_000_000},
            "title": "Serve a job a checker verified",
            "objective": "Pass Victor's canaries on the pinned suite.",
            "acceptance": {"rule": "pylon-check", "suite": suite, "checker": checker.pubkey(), "max_awards": 100},
            "reference": null,
            "award": {"provider": 25},
        });
        let parts = quest(&spec).unwrap();
        let quest_event = referee.sign(AT - 10, parts.kind, parts.tags, parts.content);
        let job = receipt(&buyer, &provider, Outcome::Accepted);
        let method = format!("canary exact-match suite:{suite}");
        let pass = label(&checker, Verdict::Pass, &job, &method);
        let parts = pylon_check_award(&quest_event, &job, &pass, AT + 2).unwrap();
        let award_event = referee.sign(AT + 2, parts.kind, parts.tags, parts.content);
        let award = parse_award(&award_event).unwrap();
        assert_eq!(award.rule, PYLON_CHECK);
        assert_eq!(award.total(), 25);
        assert_eq!(award.awardees[0].pubkey, provider.pubkey());
        let bound = bind_quest(&award, &quest_event).unwrap();
        assert_eq!(bound.award_limit(), Some(100));
        bind_pylon_check(&award, &bound, &job, &pass).unwrap();

        // A fail, another checker, another suite, and a failed job earn
        // nothing.
        let fail = label(&checker, Verdict::Fail, &job, &method);
        assert!(pylon_check_award(&quest_event, &job, &fail, AT + 2).is_err());
        let stranger = label(&signer(5), Verdict::Pass, &job, &method);
        assert!(pylon_check_award(&quest_event, &job, &stranger, AT + 2).is_err());
        let other = label(&checker, Verdict::Pass, &job, "canary");
        assert!(pylon_check_award(&quest_event, &job, &other, AT + 2).is_err());
        let failed = receipt(&buyer, &provider, Outcome::Failed);
        let on_failed = label(&checker, Verdict::Pass, &failed, &method);
        assert!(pylon_check_award(&quest_event, &failed, &on_failed, AT + 2).is_err());
    }
}
