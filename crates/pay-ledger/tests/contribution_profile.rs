//! Fake funded optimization: accepted evidence creates shares, activity and XP never do.
use pay_ledger::markets::{contribution::*, worker::FundingReceipt};
use pay_ledger::{Ledger, OPENAGENTS, Payee, PayoutState};
fn pin(c: char) -> String {
    c.to_string().repeat(64)
}
const START: i64 = 1_792_022_400;
fn fixture() -> (Terms, Acceptance, Trust, FundingReceipt) {
    let terms = Terms {
        class: Class::VerifiedOptimization,
        obligation: pin('1'),
        source: pin('2'),
        source_group: pin('a'),
        evaluation_group: pin('b'),
        license: pin('3'),
        attribution: pin('4'),
        beneficiary: pin('5'),
        protected_evaluator: pin('6'),
        evaluation_policy: pin('7'),
        acceptance_authority: pin('8'),
        funding_authority: pin('9'),
        artifact_contract: pin('0'),
        reward_msat: 10000,
        committed_at: START - 3,
        expires_at: START + 100,
    };
    let accepted = Acceptance {
        terms_fingerprint: terms.fingerprint().unwrap(),
        artifact: pin('a'),
        evaluation_receipt: pin('b'),
        acceptance_receipt: pin('c'),
        improvement: 1,
        evaluated_at: START - 2,
        accepted_at: START - 1,
    };
    let trust = Trust {
        frozen_terms_fingerprint: terms.fingerprint().unwrap(),
        rights_verified: true,
        source_groups_verified: true,
        independently_controlled_evaluator: true,
        beneficiary_destination_verified: true,
        verified_acceptance: accepted.clone(),
        current_funding_authority: terms.funding_authority.clone(),
    };
    let receipt = FundingReceipt {
        payment_hash: pin('d'),
        received_msat: 10500,
        platform_fee_msat: 500,
        received_at: START,
    };
    (terms, accepted, trust, receipt)
}
#[test]
fn no_token_activity_xp_or_unfunded_claim_creates_reward() {
    let (terms, accepted, trust, mut receipt) = fixture();
    let mut ledger = Ledger::in_memory().unwrap();
    let mut unsupported = serde_json::to_value(&terms).unwrap();
    unsupported["tokens"] = 100.into();
    assert!(serde_json::from_value::<Terms>(unsupported).is_err());
    let mut forged = accepted.clone();
    forged.improvement = 100;
    assert!(
        ledger
            .record_contribution_earned(&terms, &forged, &trust, &receipt)
            .is_err()
    );
    receipt.received_msat = 0;
    assert!(
        ledger
            .record_contribution_earned(&terms, &accepted, &trust, &receipt)
            .is_err()
    );
    assert_eq!(ledger.accrued(&terms.beneficiary).unwrap(), 0);
    let mut leaked = terms.clone();
    leaked.evaluation_group = leaked.source_group.clone();
    assert!(leaked.validate(START).is_err());
    let mut no_rights = trust.clone();
    no_rights.rights_verified = false;
    assert!(
        ledger
            .record_contribution_earned(&terms, &accepted, &no_rights, &fixture().3)
            .is_err()
    );
}
#[test]
fn one_funded_obligation_replays_without_bonus_or_duplicate_payment_and_unknown_stays_reserved() {
    let (terms, accepted, trust, receipt) = fixture();
    let scratch = tempfile::tempdir().unwrap();
    let path = scratch.path().join("central.sqlite");
    let mut ledger = Ledger::open(&path).unwrap();
    let recorded = ledger
        .record_contribution_earned(&terms, &accepted, &trust, &receipt)
        .unwrap();
    assert!(recorded.bonuses.is_empty());
    assert_eq!(ledger.accrued(&terms.beneficiary).unwrap(), 10000);
    assert_eq!(ledger.accrued(OPENAGENTS).unwrap(), 500);
    drop(ledger);
    let mut ledger = Ledger::open(&path).unwrap();
    assert_eq!(
        ledger
            .record_contribution_earned(&terms, &accepted, &trust, &receipt)
            .unwrap()
            .seq,
        recorded.seq
    );
    let mut replacement = receipt.clone();
    replacement.payment_hash = pin('e');
    assert!(
        ledger
            .record_contribution_earned(&terms, &accepted, &trust, &replacement)
            .is_err()
    );
    let mut changed = terms.clone();
    changed.reward_msat += 1;
    assert!(
        ledger
            .record_contribution_earned(&changed, &accepted, &trust, &receipt)
            .is_err()
    );
    ledger
        .register_payee(Payee {
            party: terms.beneficiary.clone(),
            destination_kind: "spark".into(),
            destination_value: "fake-destination".into(),
            source: "scratch-admission".into(),
            verified_at: START,
        })
        .unwrap();
    let items: Vec<_> = recorded
        .shares
        .iter()
        .filter(|s| s.party == terms.beneficiary)
        .cloned()
        .collect();
    ledger
        .reserve_payout("contributor-payout", &terms.beneficiary, &items, START)
        .unwrap();
    ledger
        .set_payout_state(
            "contributor-payout",
            PayoutState::Unknown,
            Some("fake-wallet-reference"),
            START,
        )
        .unwrap();
    drop(ledger);
    let mut ledger = Ledger::open(&path).unwrap();
    assert_eq!(ledger.accrued(&terms.beneficiary).unwrap(), 0);
    assert!(
        ledger
            .reserve_payout("duplicate-payout", &terms.beneficiary, &items, START)
            .is_err()
    );
    assert_eq!(
        ledger
            .record_contribution_earned(&terms, &accepted, &trust, &receipt)
            .unwrap()
            .seq,
        recorded.seq
    );
    assert_eq!(ledger.accrued(&terms.beneficiary).unwrap(), 0);
    println!(
        "{}",
        serde_json::json!({"schema":"openagents.contribution-qualification.v1","synthetic":true,"fake_funding":true,
      "independent_operators":false,"terms":terms,"acceptance":accepted,"received_msat":10500,
      "reward_msat":10000,"platform_fee_msat":500,"bonus_msat":0,"duplicate_obligations":0,
      "payout_outcome":"unknown","reserved_reward_msat":10000,"funded_qualification":"unverified"})
    );
}
