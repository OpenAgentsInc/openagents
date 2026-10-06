//! Synthetic later-worker admission; this does not prove independent operation.
use pay_ledger::markets::{
    Deadlines,
    worker::{Admission, Earned, PROFILE, WorkerTerms},
};
fn pin(c: char) -> String {
    c.to_string().repeat(64)
}
fn fixture() -> (WorkerTerms, Admission) {
    let terms = WorkerTerms {
        profile: PROFILE.into(),
        buyer: pin('a'),
        provider: pin('b'),
        buyer_operator: "scratch-buyer".into(),
        provider_operator: "scratch-provider".into(),
        market: pin('c'),
        order: pin('d'),
        labor_terms: pin('e'),
        source: pin('f'),
        checker: pin('1'),
        disclosure: pin('2'),
        execution_requirements: pin('3'),
        cancellation_policy: pin('4'),
        delivery_rights: pin('5'),
        price_msat: 10_000,
        fee_limit_msat: 100,
        capacity_units: 1,
        max_rework: 2,
        deadlines: Deadlines {
            delivery: 10,
            review: 20,
            dispute: 30,
            resolution: 40,
            payment: 50,
            retain_until: 60,
        },
    };
    let admission = Admission {
        buyer: terms.buyer.clone(),
        provider: terms.provider.clone(),
        buyer_operator: terms.buyer_operator.clone(),
        provider_operator: terms.provider_operator.clone(),
        independent_operators_verified: true,
        execution_requirements: terms.execution_requirements.clone(),
        disclosure: terms.disclosure.clone(),
        available_capacity: 1,
        expires_at: 9,
        payout_destination_verified: true,
    };
    (terms, admission)
}
#[test]
fn current_independent_admission_is_external_to_terms() {
    let (terms, mut admission) = fixture();
    terms.validate(1, &admission).unwrap();
    admission.independent_operators_verified = false;
    assert!(terms.validate(1, &admission).is_err());
    admission.independent_operators_verified = true;
    admission.available_capacity = 0;
    assert!(terms.validate(1, &admission).is_err());
    admission.available_capacity = 1;
    admission.disclosure = pin('6');
    assert!(terms.validate(1, &admission).is_err());
    admission.disclosure = terms.disclosure.clone();
    assert!(terms.validate(9, &admission).is_err());
}
#[test]
fn distinct_acceptance_and_obligation_survive_serialized_recovery() {
    let (terms, admission) = fixture();
    terms.validate(1, &admission).unwrap();
    let earned = Earned {
        obligation: terms.obligation(),
        order: terms.order.clone(),
        labor_terms: terms.labor_terms.clone(),
        delivery: pin('6'),
        verification: pin('7'),
        acceptance: pin('8'),
        buyer: terms.buyer.clone(),
        provider: terms.provider.clone(),
        accepted_msat: terms.price_msat,
    };
    let bytes = serde_json::to_vec(&earned).unwrap();
    let recovered: Earned = serde_json::from_slice(&bytes).unwrap();
    recovered.validate(&terms).unwrap();
    assert_eq!(recovered, earned);
    let mut altered = terms.clone();
    altered.order = pin('9');
    assert!(recovered.validate(&altered).is_err());
    altered = terms.clone();
    altered.price_msat += 1;
    assert!(recovered.validate(&altered).is_err());
}
#[test]
fn no_spend_profile_has_no_fees_or_wallet_requirement() {
    let (mut terms, mut admission) = fixture();
    terms.price_msat = 0;
    terms.fee_limit_msat = 0;
    admission.payout_destination_verified = false;
    terms.validate(1, &admission).unwrap();
    terms.fee_limit_msat = 1;
    assert!(terms.validate(1, &admission).is_err());
}
#[test]
fn fake_worker_funding_uses_central_shares_and_does_not_reward_plugin_activity() {
    use pay_ledger::markets::worker::FundingReceipt;
    use pay_ledger::{Ledger, OPENAGENTS};
    let (mut terms, _) = fixture();
    let start = 1_792_022_400;
    terms.deadlines.payment = start + 100;
    let earned = Earned {
        obligation: terms.obligation(),
        order: terms.order.clone(),
        labor_terms: terms.labor_terms.clone(),
        delivery: pin('6'),
        verification: pin('7'),
        acceptance: pin('8'),
        buyer: terms.buyer.clone(),
        provider: terms.provider.clone(),
        accepted_msat: terms.price_msat,
    };
    let receipt = FundingReceipt {
        payment_hash: pin('9'),
        received_msat: 10_500,
        platform_fee_msat: 500,
        received_at: start,
    };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("central.sqlite");
    let mut ledger = Ledger::open(&path).unwrap();
    let recorded = ledger
        .record_worker_earned(&terms, &earned, &receipt)
        .unwrap();
    assert!(recorded.bonuses.is_empty());
    assert_eq!(ledger.accrued(&terms.provider).unwrap(), 10_000);
    assert_eq!(ledger.accrued(OPENAGENTS).unwrap(), 500);
    drop(ledger);
    let mut ledger = Ledger::open(&path).unwrap();
    assert_eq!(
        ledger
            .record_worker_earned(&terms, &earned, &receipt)
            .unwrap()
            .seq,
        recorded.seq
    );
    assert_eq!(ledger.accrued(&terms.provider).unwrap(), 10_000);
    let mut changed = receipt.clone();
    changed.payment_hash = pin('0');
    assert!(
        ledger
            .record_worker_earned(&terms, &earned, &changed)
            .is_err()
    );
    changed = receipt.clone();
    changed.received_msat -= 1;
    assert!(
        ledger
            .record_worker_earned(&terms, &earned, &changed)
            .is_err()
    );
    let mut terms2 = terms.clone();
    terms2.disclosure = pin('0');
    assert!(
        ledger
            .record_worker_earned(&terms2, &earned, &receipt)
            .is_err()
    );
    assert_eq!(ledger.accrued(&terms.provider).unwrap(), 10_000);
    ledger
        .register_payee(pay_ledger::Payee {
            party: terms.provider.clone(),
            destination_kind: "spark".into(),
            destination_value: "fake-destination".into(),
            source: "scratch-admission".into(),
            verified_at: start,
        })
        .unwrap();
    let shares: Vec<_> = recorded
        .shares
        .iter()
        .filter(|s| s.party == terms.provider)
        .cloned()
        .collect();
    ledger
        .reserve_payout("worker-payout", &terms.provider, &shares, start)
        .unwrap();
    ledger
        .set_payout_state(
            "worker-payout",
            pay_ledger::PayoutState::Unknown,
            Some("fake-wallet-ref"),
            start,
        )
        .unwrap();
    drop(ledger);
    let mut ledger = Ledger::open(&path).unwrap();
    assert_eq!(ledger.accrued(&terms.provider).unwrap(), 0);
    assert!(
        ledger
            .reserve_payout("duplicate-payout", &terms.provider, &shares, start)
            .is_err()
    );
    assert_eq!(
        ledger
            .record_worker_earned(&terms, &earned, &receipt)
            .unwrap()
            .seq,
        recorded.seq
    );
    assert_eq!(ledger.accrued(&terms.provider).unwrap(), 0);
}
