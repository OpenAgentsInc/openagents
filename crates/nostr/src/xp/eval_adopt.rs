//! NIP-XP's `eval-adopt` rule (`nips/openagents/NIP-XP.md`, "`eval-adopt`").
//!
//! **Adoption** makes an extension part of an agent host's defaults: the
//! host's operator issues an `openagents.eval-admission.v1` decision citing
//! the extension's published results, then publishes a release of the
//! defaults package (for Coder, `coder-defaults`) that depends on the
//! extension's release and cites the admission in its manifest's
//! `provenance.receipts`. That release is the completion.
//!
//! It credits the **extension author** (the root key of the adopted
//! release), the **suite author** of each suite whose cited results were
//! confirmed, and the **evaluator** (trainer) of each cited result that a
//! check confirmed under the `eval-check` conditions. Each award credits
//! one role under the key `eval-adopt:<subject release>:<role>:<pubkey>`, so
//! each role is paid once per subject release; a key holding two roles is
//! paid once, in the larger.

use std::collections::BTreeSet;

use super::eval_check::{Payee, award_parts, collapse, confirmed_check, paid};
use super::{Award, EVAL_ADOPT, Quest, in_season, parse_quest};
use crate::contracts::{ContractError, RefusalCode, check_artifact_bytes};
use crate::domain::Event;
use crate::eval_ext::{Publication, parse_admission, parse_publication, parse_release};
use crate::kb::{self, Pointer, Unsigned, malformed, mismatch};
use crate::kinds;

/// Every `eval-adopt` uniqueness key starts with this.
pub const KEY_PREFIX: &str = "eval-adopt:";

/// What a reader needs to check one adoption: the defaults release and
/// its manifest's bytes, the admission's bytes, the cited results, the
/// checks that may confirm them, and hosted runs' signed requests.
#[derive(Debug, Clone, Copy)]
pub struct Adoption<'a> {
    /// The defaults package's `3184` release.
    pub release: &'a Event,
    /// The exact bytes of that release's manifest.
    pub manifest: &'a [u8],
    /// The exact bytes of the `openagents.eval-admission.v1` the manifest
    /// cites.
    pub admission: &'a [u8],
    /// Result publications; those the admission doesn't cite, or on
    /// another subject, are ignored.
    pub results: &'a [Event],
    /// Check publications; those that don't confirm are ignored.
    pub checks: &'a [Event],
    /// Signed NIP-CJ execution requests of hosted results and checks.
    pub requests: &'a [Event],
}

/// A confirmed result the admission cites, and the check that confirmed
/// it.
#[derive(Debug, Clone, PartialEq)]
pub struct Confirmed {
    pub result: Publication,
    pub check: Publication,
    /// The result's trainer.
    pub evaluator: String,
}

/// An accepted `eval-adopt` completion.
#[derive(Debug, Clone, PartialEq)]
pub struct AdoptCompletion {
    /// In order of result, then check, event ID.
    pub confirmed: Vec<Confirmed>,
    /// The keys it pays, after role collapse, in role order.
    pub payees: Vec<Payee>,
}

impl AdoptCompletion {
    /// The confirmed pair an award to `payee` names: the evaluator's own
    /// result, or for an author the first confirmed pair.
    #[must_use]
    pub fn evidence_for(&self, payee: &Payee) -> Option<&Confirmed> {
        if payee.role == "evaluator" {
            self.confirmed.iter().find(|c| c.evaluator == payee.pubkey)
        } else if payee.role == "suite-author" {
            self.confirmed
                .iter()
                .find(|c| c.result.suite_author() == payee.pubkey)
        } else {
            self.confirmed.first()
        }
    }
}

/// The `eval-adopt` rule. A defaults release completes the quest when all
/// hold:
///
/// 1. The release is a valid `3184` of the quest's `defaults` package,
///    signed by its root, published inside the season, and its manifest's
///    bytes match.
/// 2. The manifest depends on the quest's subject release and cites, in
///    `provenance.receipts`, the admission whose bytes are supplied.
/// 3. The admission decides `admit`, for the quest's subject release, and
///    hadn't expired when the release was published.
/// 4. At least one result the admission cites (by report digest), on the
///    subject release, is confirmed by a check under the `eval-check`
///    conditions.
/// 5. At least one result the admission cites in `validation` (by report
///    digest) is a **Better** result on the subject release that names a
///    cited result with the `validates` marker and ran a suite whose
///    release is signed by someone other than the subject's author. The
///    chronology half of independence (the suite released after the
///    subject was locked) needs the release events, which this rule isn't
///    handed; the operator's adopt command checks it before writing the
///    admission, and a reader with the releases uses
///    [`crate::eval_ext::validation`].
///
/// # Errors
///
/// [`RefusalCode::NotAdmitted`] when the completion fails the rule; other
/// codes when an event or document is invalid or isn't the one named.
pub fn check_eval_adopt(
    quest: &Quest,
    adoption: &Adoption<'_>,
) -> Result<AdoptCompletion, ContractError> {
    if quest.acceptance.rule != EVAL_ADOPT {
        return Err(mismatch("the quest's rule isn't eval-adopt"));
    }
    let accepted = quest
        .acceptance
        .eval
        .as_ref()
        .ok_or_else(|| mismatch("the quest's rule isn't eval-adopt"))?;
    let defaults = accepted
        .defaults
        .as_deref()
        .ok_or_else(|| malformed("defaults"))?;
    let refuse = |why: String| Err(ContractError::new(RefusalCode::NotAdmitted, why));
    let release = parse_release(adoption.release, adoption.manifest)?;
    if release.package != defaults {
        return Err(mismatch("the release isn't the quest's defaults package"));
    }
    let season = &quest.season;
    let at = adoption.release.created_at;
    if at < season.opens_at || at > season.closes_at {
        return refuse(format!("the release isn't inside season {}", season.id));
    }
    if !release.manifest.dependencies.contains(&accepted.subject.id) {
        return refuse("the release doesn't depend on the adopted extension's release".into());
    }
    if !release
        .admissions
        .iter()
        .any(|a| check_artifact_bytes(a, adoption.admission).is_ok())
    {
        return Err(mismatch(
            "the release's provenance doesn't cite this admission",
        ));
    }
    let admission = parse_admission(adoption.admission)?;
    if admission.decision != "admit" {
        return refuse(format!(
            "the admission decided {}, not admit",
            admission.decision
        ));
    }
    let subject_release = admission.subject.event.as_ref().ok_or_else(|| {
        malformed("admission.subject.event: an adoption names the extension's release")
    })?;
    if subject_release.id != accepted.subject.id
        || subject_release.pubkey != accepted.subject.pubkey
    {
        return Err(mismatch("the admission admits another extension release"));
    }
    if at > admission.expires_at {
        return refuse("the admission had expired when the release was published".into());
    }
    let cited: BTreeSet<&str> = admission.reports.iter().map(String::as_str).collect();
    let mut results: Vec<Publication> = adoption
        .results
        .iter()
        .filter_map(|e| parse_publication(e).ok())
        .filter(|p| {
            cited.contains(p.report_ref.digest.as_str())
                && p.subject_release.as_ref() == Some(&accepted.subject)
        })
        .collect();
    results.sort_by(|a, b| a.id.cmp(&b.id));
    let mut checks: Vec<Publication> = adoption
        .checks
        .iter()
        .filter_map(|e| parse_publication(e).ok())
        .collect();
    checks.sort_by(|a, b| a.id.cmp(&b.id));
    let mut confirmed = Vec::new();
    for result in results {
        if let Some((check, evaluator)) = checks.iter().find_map(|check| {
            confirmed_check(&result, check, adoption.requests)
                .ok()
                .map(|(_, evaluator)| (check.clone(), evaluator))
        }) {
            confirmed.push(Confirmed {
                result,
                check,
                evaluator,
            });
        }
    }
    if confirmed.is_empty() {
        return refuse(
            "the admission cites no result on the extension that a check confirmed".into(),
        );
    }
    let cited_ids: BTreeSet<&str> = confirmed.iter().map(|c| c.result.id.as_str()).collect();
    let validated = adoption
        .results
        .iter()
        .filter_map(|e| parse_publication(e).ok())
        .any(|p| {
            admission.validation.contains(&p.report_ref.digest)
                && p.subject_release.as_ref() == Some(&accepted.subject)
                && p.validates
                    .as_deref()
                    .is_some_and(|id| cited_ids.contains(id))
                && p.suite_release.pubkey != accepted.subject.pubkey
                && p.verdict() == crate::eval_ext::Verdict::Pass
        });
    if !validated {
        return refuse(
            "the admission cites no externally validating result: a Better result on a second \
             suite by another author, naming a confirmed result with the validates marker"
                .into(),
        );
    }
    let mut candidates = vec![Payee {
        role: "extension-author",
        pubkey: accepted.subject.pubkey.clone(),
    }];
    for c in &confirmed {
        candidates.push(Payee {
            role: "suite-author",
            pubkey: c.result.suite_author().to_string(),
        });
        candidates.push(Payee {
            role: "evaluator",
            pubkey: c.evaluator.clone(),
        });
    }
    let payees = collapse(quest, candidates);
    Ok(AdoptCompletion { confirmed, payees })
}

/// The parts of one `3193` for each key the adoption pays. Each award
/// credits one role and names the defaults release, then a confirmed
/// result and its check: the evaluator's own, or for an author the first
/// that credits them. A referee skips a key it has already awarded.
///
/// # Errors
///
/// When an event or document isn't valid, the completion fails the rule,
/// or `accepted_at` is outside the season or before the release.
pub fn eval_adopt_awards(
    quest: &Event,
    adoption: &Adoption<'_>,
    accepted_at: u64,
) -> Result<Vec<Unsigned>, ContractError> {
    let parsed = parse_quest(quest)?;
    in_season(&parsed, accepted_at)?;
    let completion = check_eval_adopt(&parsed, adoption)?;
    if adoption.release.created_at > accepted_at {
        return Err(mismatch("the release is newer than its acceptance"));
    }
    let find = |id: &str, events: &'_ [Event]| events.iter().find(|e| e.id == id).cloned();
    completion
        .payees
        .iter()
        .map(|payee| {
            let pair = completion
                .evidence_for(payee)
                .ok_or_else(|| malformed("no confirmed pair for the payee"))?;
            let result =
                find(&pair.result.id, adoption.results).ok_or_else(|| malformed("result"))?;
            let check = find(&pair.check.id, adoption.checks).ok_or_else(|| malformed("check"))?;
            award_parts(
                quest,
                &parsed,
                payee,
                &[
                    (adoption.release, kinds::EXT_RELEASE),
                    (&result, kb::EVIDENCE_KIND),
                    (&check, kb::EVIDENCE_KIND),
                ],
                accepted_at,
            )
        })
        .collect()
}

/// Checks a parsed `eval-adopt` award against the adoption it names: the
/// release, result, and check are the award's evidence, the rule accepts
/// the adoption, and it pays the award's one awardee in that role.
///
/// # Errors
///
/// As [`check_eval_adopt`], and [`RefusalCode::IdentityMismatch`] when an
/// event isn't the one the award names or the awardee isn't paid.
pub fn bind_eval_adopt(
    award: &Award,
    quest: &Quest,
    adoption: &Adoption<'_>,
) -> Result<(), ContractError> {
    if award.rule != EVAL_ADOPT || award.evidence.len() != 3 {
        return Err(mismatch("the award isn't an eval-adopt award"));
    }
    let named = |p: &Pointer, e: &Event| p.id == e.id && p.pubkey == e.pubkey;
    if !named(&award.evidence[0], adoption.release) {
        return Err(mismatch("the award names another release"));
    }
    let completion = check_eval_adopt(quest, adoption)?;
    paid(award, &completion.payees)?;
    let awardee = &award.awardees[0];
    let pair_named = completion.confirmed.iter().any(|c| {
        award.evidence[1].id == c.result.id
            && award.evidence[2].id == c.check.id
            && match awardee.role.as_str() {
                "evaluator" => c.evaluator == awardee.pubkey,
                "suite-author" => c.result.suite_author() == awardee.pubkey,
                _ => true,
            }
    });
    if !pair_named {
        return Err(mismatch(
            "the award's result and check aren't a confirmed pair of this adoption",
        ));
    }
    if adoption.release.created_at > award.accepted_at {
        return Err(mismatch("the release is newer than its acceptance"));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
