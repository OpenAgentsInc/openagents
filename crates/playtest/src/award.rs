//! Playtest awards from triage-log acceptances.
//!
//! An accepted contribution in the triage log ([`Acceptance`]) names the
//! tester's key and the digest of their private report. The tester's app
//! published a content-free NIP-XP playtest report (kind `3197`) that
//! commits to the same digest. [`plan`] joins the two with a `playtest`
//! quest and builds the NIP-XP award the playtest referee signs, after
//! [`nostr::xp::playtest_award`] checks the rule. [`admit`] refuses an
//! award whose rule-derived key already has a live award from the referee
//! or whose quest version is out of `max_awards`.
//!
//! Signing is gated on the playtest referee key: [`referee`] refuses every
//! key while the build's `PLAYTEST_REFEREE` is unset, so the production
//! path stays off until the owner creates that key. Tests pass their own
//! expected key.

use std::collections::BTreeSet;

use nostr::domain::Event;
use nostr::kb::Unsigned;
use nostr::xp;

use crate::session;
use crate::triage::Acceptance;

/// The refusal while no playtest referee key exists.
pub const NO_REFEREE: &str = "the playtest referee key doesn't exist yet (PLAYTEST_REFEREE is unset), so playtest awards aren't signed; the owner creates it with `microcoder xp playtest-keygen`";

/// Checks that `referee` (hex) is the playtest referee this build trusts.
///
/// # Errors
///
/// [`NO_REFEREE`] while `expected` is `None`, or a mismatch.
pub fn referee(referee: &str, expected: Option<&str>) -> Result<(), String> {
    match expected {
        None => Err(NO_REFEREE.into()),
        Some(key) if key.eq_ignore_ascii_case(referee) => Ok(()),
        Some(_) => Err("this key isn't the playtest referee this build trusts".into()),
    }
}

/// The tester's public playtest report for `acceptance`, among `events`:
/// signed by the tester and committing to the accepted report's digest.
#[must_use]
pub fn find_report<'a>(acceptance: &Acceptance, events: &'a [Event]) -> Option<&'a Event> {
    let digest = acceptance.digest.as_deref()?;
    events.iter().find(|event| {
        event.pubkey == acceptance.tester
            && xp::playtest::parse_playtest_report(event).is_ok_and(|r| r.digest == digest)
    })
}

/// An award ready for the referee to sign.
#[derive(Clone, Debug)]
pub struct Plan {
    /// The rule-derived uniqueness key.
    pub key: String,
    /// `30193:<referee>:<address>`.
    pub coordinate: String,
    pub max_awards: u64,
    pub unsigned: Unsigned,
}

/// Builds the award for `acceptance` from its `quest`, the tester's public
/// `report`, and, for a moderated or group session, the moderator's
/// `session` record. `repo` is `owner/repo` for the issue; `commit` is the
/// commit that shipped a `design` change.
///
/// # Errors
///
/// A sentence naming what doesn't match: the quest's contribution, the
/// report's signer or digest, a missing triager, or the NIP-XP rule.
pub fn plan(
    acceptance: &Acceptance,
    quest: &Event,
    report: &Event,
    session: Option<&Event>,
    repo: &str,
    commit: Option<&str>,
) -> Result<Plan, String> {
    let parsed = xp::parse_quest(quest).map_err(|e| format!("the quest: {e}"))?;
    let rule = parsed
        .acceptance
        .playtest
        .as_ref()
        .ok_or("the quest's rule isn't playtest")?;
    if rule.contribution != acceptance.contribution {
        return Err(format!(
            "the quest pays for `{}`, and this acceptance is `{}`",
            rule.contribution, acceptance.contribution
        ));
    }
    let public = xp::playtest::parse_playtest_report(report)
        .map_err(|e| format!("the public report: {e}"))?;
    if report.pubkey != acceptance.tester {
        return Err("the public report isn't signed by the accepted tester".into());
    }
    if acceptance.digest.as_deref() != Some(public.digest.as_str()) {
        return Err("the public report commits to a different private report".into());
    }
    let triager = acceptance
        .triager
        .as_deref()
        .ok_or("the triage log doesn't name who accepted it; record it with --triager")?;
    let fields = xp::PlaytestAward {
        issue: acceptance.issue.map(|n| format!("{repo}#{n}")),
        severity: acceptance.severity.map(|s| session::name(&s)),
        commit: commit.map(str::to_owned),
    };
    let key = xp::playtest::key(&parsed, &acceptance.tester, fields.issue.as_deref())
        .map_err(|e| e.to_string())?;
    let unsigned = xp::playtest_award(
        quest,
        report,
        session,
        triager,
        &fields,
        acceptance.accepted_at,
    )
    .map_err(|e| format!("not accepted by the playtest rule: {e}"))?;
    Ok(Plan {
        key,
        coordinate: xp::coordinate(&quest.pubkey, &parsed.address),
        max_awards: rule.max_awards,
        unsigned,
    })
}

/// Checks `plan` against the referee's published awards and revocations
/// (`existing`): its key has no live award, and its quest version has
/// fewer live awards than `max_awards`.
///
/// # Errors
///
/// A sentence naming the award already there or the limit.
pub fn admit(plan: &Plan, existing: &[Event]) -> Result<(), String> {
    let revoked: BTreeSet<String> = existing
        .iter()
        .filter_map(|e| xp::parse_revocation(e).ok())
        .map(|r| r.award.id)
        .collect();
    let live: Vec<(&Event, xp::Award)> = existing
        .iter()
        .filter(|e| !revoked.contains(&e.id))
        .filter_map(|e| xp::parse_award(e).ok().map(|a| (e, a)))
        .collect();
    if let Some((event, _)) = live.iter().find(|(_, a)| a.key == plan.key) {
        return Err(format!(
            "{} already has award {}; a contribution pays once",
            plan.key, event.id
        ));
    }
    let used = live
        .iter()
        .filter(|(_, a)| a.coordinate == plan.coordinate)
        .count() as u64;
    if used >= plan.max_awards {
        return Err(format!(
            "the quest version already has {used} live awards, its max_awards"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
