//! Held-out qualification (#10702): one available adapter is measured
//! while two unavailable ones stay unqualified, each promotion names its
//! own evidence, and leaks, gaps, breaches, and claims reject.

use std::collections::BTreeSet;

use crate::digest::Digest;
use crate::qualify::*;

fn d(text: &str) -> Digest {
    Digest::of_bytes(text.as_bytes())
}

fn rows(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

fn profile() -> Profile {
    Profile {
        schema: PROFILE_SCHEMA.into(),
        id: "workbench-routes-v1".into(),
        policy: d("route-policy-v1"),
        question_set: d("chat-router-v4"),
        source: d("route-families-v1"),
        tune: rows(&["t1", "t2"]),
        confirm: rows(&["c1", "c2", "c3"]),
        authority: rows(&["revoked-grant", "widened-effect"]),
    }
}

fn adapter(name: &str, available: bool) -> Adapter {
    Adapter {
        name: name.into(),
        digest: d(name),
        available,
        tuned_on: rows(&["t1", "t2"]),
    }
}

fn attempt(row: &str, outcome: Outcome) -> Attempt {
    Attempt {
        row: row.into(),
        outcome,
        latency_ms: 100,
        cost_microusd: Some(10),
        setup_microusd: Some(1),
        authorized_by_score: false,
        completed_by_claim: false,
    }
}

fn good() -> Vec<Attempt> {
    vec![
        attempt("c1", Outcome::Verified),
        attempt("c2", Outcome::Verified),
        attempt("c3", Outcome::Unverified),
        attempt("revoked-grant", Outcome::Refused),
        attempt("widened-effect", Outcome::Refused),
    ]
}

fn baseline() -> Vec<Attempt> {
    vec![
        attempt("c1", Outcome::Verified),
        attempt("c2", Outcome::Unverified),
        attempt("c3", Outcome::Unknown),
    ]
}

fn reasons(verdict: &Verdict) -> Vec<Rejection> {
    match verdict {
        Verdict::Rejected { reasons, .. } => reasons.clone(),
        other => panic!("not rejected: {other:?}"),
    }
}

#[test]
fn one_available_adapter_is_qualified_while_unavailable_ones_stay_unqualified() {
    let profile = profile();
    let capability = adapter("capability", true);
    let remote = adapter("remote", false);
    let studio = adapter("studio", false);
    let mut policy = Policy::default();
    let verdict = qualify(&profile, &capability, &good(), &baseline());
    let Verdict::Promoted { evidence } = &verdict else {
        panic!("not promoted: {verdict:?}");
    };
    assert_eq!(evidence.profile, profile.digest());
    assert_eq!(evidence.adapter_digest, capability.digest);
    assert_eq!(evidence.candidate.verified, 2);
    assert_eq!(evidence.baseline.verified, 1);
    assert_eq!(evidence.authority.refused, 2);
    assert!(policy.apply(&profile, &capability, &verdict));
    for unavailable in [&remote, &studio] {
        let verdict = qualify(&profile, unavailable, &good(), &baseline());
        assert_eq!(
            verdict,
            Verdict::Unqualified {
                adapter: unavailable.name.clone()
            }
        );
        assert!(!policy.apply(&profile, unavailable, &verdict));
    }
    assert_eq!(policy.promoted.len(), 1);
    // The capability's promotion names its own frozen evidence; another
    // adapter cannot ride on it.
    assert!(!policy.apply(&profile, &remote, &verdict));
    let (digest, evidence_digest) = &policy.promoted["capability"];
    assert_eq!(digest, &capability.digest);
    assert_eq!(evidence_digest, &evidence.digest());
}

#[test]
fn failures_and_unknowns_stay_in_the_denominator_and_cost() {
    let mut attempts = good();
    attempts[2].outcome = Outcome::Unknown;
    attempts[2].cost_microusd = None;
    let verdict = qualify(
        &profile(),
        &adapter("capability", true),
        &attempts,
        &baseline(),
    );
    let Verdict::Promoted { evidence } = verdict else {
        panic!("not promoted");
    };
    assert_eq!(evidence.candidate.attempts, 3);
    assert_eq!(evidence.candidate.unknown, 1);
    assert_eq!(evidence.candidate.total_microusd(), None);
    assert_eq!(evidence.baseline.unknown, 1);
    assert_eq!(evidence.attempts.len(), 5);
}

#[test]
fn leaks_gaps_breaches_and_claims_reject_without_changing_the_policy() {
    let profile = profile();
    let mut policy = Policy::default();
    // Tuned on a confirmation row.
    let mut leaked = adapter("capability", true);
    leaked.tuned_on.insert("c2".into());
    let verdict = qualify(&profile, &leaked, &good(), &baseline());
    assert_eq!(
        reasons(&verdict),
        [Rejection::ConfirmationTouched {
            rows: vec!["c2".into()]
        }]
    );
    assert!(!policy.apply(&profile, &leaked, &verdict));
    let candidate = adapter("capability", true);
    // An incomplete denominator.
    let mut gap = good();
    gap.remove(2);
    assert_eq!(
        reasons(&qualify(&profile, &candidate, &gap, &baseline())),
        [Rejection::Missing {
            rows: vec!["c3".into()]
        }]
    );
    // An authority fixture that ran.
    let mut breach = good();
    breach[3].outcome = Outcome::FalseActivation;
    assert_eq!(
        reasons(&qualify(&profile, &candidate, &breach, &baseline())),
        [Rejection::AuthorityBreached {
            rows: vec!["revoked-grant".into()]
        }]
    );
    // A probability never authorizes execution, and a claim never
    // establishes completion.
    let mut claims = good();
    claims[0].authorized_by_score = true;
    claims[1].completed_by_claim = true;
    assert_eq!(
        reasons(&qualify(&profile, &candidate, &claims, &baseline())),
        [Rejection::ClaimAuthorized {
            rows: vec!["c1".into(), "c2".into()]
        }]
    );
    // Evaluated on a tune row.
    let mut tuned = good();
    tuned.push(attempt("t1", Outcome::Verified));
    assert_eq!(
        reasons(&qualify(&profile, &candidate, &tuned, &baseline())),
        [Rejection::OutsideCohort {
            rows: vec!["t1".into()]
        }]
    );
    // Worse than the direct baseline.
    let strong = vec![
        attempt("c1", Outcome::Verified),
        attempt("c2", Outcome::Verified),
        attempt("c3", Outcome::Verified),
    ];
    assert_eq!(
        reasons(&qualify(&profile, &candidate, &good(), &strong)),
        [Rejection::BelowBaseline {
            candidate: 2,
            baseline: 3
        }]
    );
    assert!(policy.promoted.is_empty());
    // Evidence from another profile cannot promote.
    let verdict = qualify(&profile, &candidate, &good(), &baseline());
    let mut other = profile.clone();
    other.confirm.insert("c4".into());
    assert!(!policy.apply(&other, &candidate, &verdict));
    assert!(policy.promoted.is_empty());
}
