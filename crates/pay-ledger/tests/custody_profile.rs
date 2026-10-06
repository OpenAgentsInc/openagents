//! Fake rail only: conservation, restart, disputes, failed refunds, and unknown custody.
use pay_ledger::markets::custody::*;
fn pin(c: char) -> String {
    c.to_string().repeat(64)
}
fn terms() -> Terms {
    Terms {
        order: pin('1'),
        buyer: pin('2'),
        provider: pin('3'),
        resolver: pin('4'),
        custody_policy: pin('5'),
        milestones_msat: vec![600, 400],
        deposit_msat: 1100,
        fee_budget_msat: 100,
        max_rework: 2,
        acceptance_due_at: 10,
        resolution_due_at: 20,
    }
}
fn authority() -> Authority {
    Authority {
        buyer_acceptance_verified: true,
        protected_check_passed: true,
        resolver_resolution_verified: true,
        refund_authorized: true,
    }
}
fn release() -> Attempt {
    Attempt {
        id: pin('a'),
        effect: Effect::Release {
            milestone: 0,
            delivery: pin('6'),
            verification: pin('7'),
            acceptance: pin('8'),
        },
        amount_msat: 600,
        fee_msat: 10,
        outcome: Outcome::Confirmed,
    }
}
#[test]
fn release_requires_separate_acceptance_check_and_never_repeats() {
    assert!(!production_custody_available());
    let mut study = Study::new(terms()).unwrap();
    let mut grant = authority();
    grant.buyer_acceptance_verified = false;
    assert!(study.attempt(release(), &grant, 1).is_err());
    grant.buyer_acceptance_verified = true;
    grant.protected_check_passed = false;
    assert!(study.attempt(release(), &grant, 1).is_err());
    grant.protected_check_passed = true;
    study.attempt(release(), &grant, 1).unwrap();
    study.attempt(release(), &grant, 1).unwrap();
    assert_eq!(study.held_msat().unwrap(), 490);
    let mut duplicate = release();
    duplicate.id = pin('b');
    assert!(study.attempt(duplicate, &grant, 1).is_err());
}
#[test]
fn dispute_release_and_exact_remaining_refund_conserve() {
    let mut study = Study::new(terms()).unwrap();
    study.attempt(release(), &authority(), 1).unwrap();
    let dispute = Attempt {
        id: pin('b'),
        effect: Effect::DisputeRelease {
            milestone: 1,
            resolution: pin('9'),
        },
        amount_msat: 400,
        fee_msat: 10,
        outcome: Outcome::Confirmed,
    };
    let mut grant = authority();
    grant.resolver_resolution_verified = false;
    assert!(study.attempt(dispute.clone(), &grant, 11).is_err());
    grant.resolver_resolution_verified = true;
    study.attempt(dispute, &grant, 11).unwrap();
    assert_eq!(study.held_msat().unwrap(), 80);
    study
        .attempt(
            Attempt {
                id: pin('c'),
                effect: Effect::Refund,
                amount_msat: 80,
                fee_msat: 0,
                outcome: Outcome::Confirmed,
            },
            &grant,
            21,
        )
        .unwrap();
    assert_eq!(study.held_msat().unwrap(), 0);
    assert!(study.attempt(release(), &grant, 1).is_ok()); // exact retained retry is observation only
}
#[test]
fn unknown_after_crash_never_releases_or_refunds_twice() {
    let mut study = Study::new(terms()).unwrap();
    let mut attempt = release();
    attempt.outcome = Outcome::Unknown;
    attempt.fee_msat = 0;
    study.attempt(attempt.clone(), &authority(), 1).unwrap();
    let mut recovered: Study =
        serde_json::from_slice(&serde_json::to_vec(&study).unwrap()).unwrap();
    assert_eq!(recovered.held_msat().unwrap(), 500);
    assert!(
        recovered
            .attempt(
                Attempt {
                    id: pin('b'),
                    effect: Effect::Refund,
                    amount_msat: 500,
                    fee_msat: 0,
                    outcome: Outcome::Confirmed
                },
                &authority(),
                11
            )
            .is_err()
    );
    assert!(
        recovered
            .reconcile(&attempt.id, Outcome::Confirmed, 599, 0)
            .is_err()
    );
    recovered
        .reconcile(&attempt.id, Outcome::Confirmed, 600, 0)
        .unwrap();
    recovered
        .reconcile(&attempt.id, Outcome::Confirmed, 600, 0)
        .unwrap();
    assert_eq!(recovered.held_msat().unwrap(), 500);
}
#[test]
fn failed_refund_preserves_liability_and_expired_or_over_fee_release_refuses() {
    let mut study = Study::new(terms()).unwrap();
    let mut late = release();
    assert!(study.attempt(late.clone(), &authority(), 11).is_err());
    late.fee_msat = 101;
    assert!(study.attempt(late, &authority(), 1).is_err());
    let refund = Attempt {
        id: pin('b'),
        effect: Effect::Refund,
        amount_msat: 1100,
        fee_msat: 0,
        outcome: Outcome::Failed,
    };
    study.attempt(refund, &authority(), 1).unwrap();
    assert_eq!(study.held_msat().unwrap(), 1100);
    study
        .attempt(
            Attempt {
                id: pin('c'),
                effect: Effect::Refund,
                amount_msat: 1100,
                fee_msat: 0,
                outcome: Outcome::Confirmed,
            },
            &authority(),
            1,
        )
        .unwrap();
    assert_eq!(study.held_msat().unwrap(), 0);
}
