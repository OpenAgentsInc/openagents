//! A fake conserved custody scenario; there is no production deposit or refund call.
use pay_ledger::markets::custody::*;
fn pin(c: char) -> String {
    c.to_string().repeat(64)
}
fn main() {
    let terms = Terms {
        order: pin('1'),
        buyer: pin('2'),
        provider: pin('3'),
        resolver: pin('4'),
        custody_policy: pin('5'),
        milestones_msat: vec![1000],
        deposit_msat: 1100,
        fee_budget_msat: 100,
        max_rework: 1,
        acceptance_due_at: 10,
        resolution_due_at: 20,
    };
    let grant = Authority {
        buyer_acceptance_verified: true,
        protected_check_passed: true,
        resolver_resolution_verified: false,
        refund_authorized: true,
    };
    let mut study = Study::new(terms).expect("fake custody terms");
    let release = Attempt {
        id: pin('a'),
        effect: Effect::Release {
            milestone: 0,
            delivery: pin('6'),
            verification: pin('7'),
            acceptance: pin('8'),
        },
        amount_msat: 1000,
        fee_msat: 10,
        outcome: Outcome::Unknown,
    };
    study
        .attempt(release.clone(), &grant, 1)
        .expect("fake unknown release");
    let saved = serde_json::to_vec(&study).expect("fake retained study");
    let mut recovered: Study = serde_json::from_slice(&saved).expect("fake restart");
    recovered
        .reconcile(&release.id, Outcome::Confirmed, 1000, 10)
        .expect("fake lookup without resend");
    recovered
        .attempt(
            Attempt {
                id: pin('b'),
                effect: Effect::Refund,
                amount_msat: 90,
                fee_msat: 0,
                outcome: Outcome::Confirmed,
            },
            &grant,
            11,
        )
        .expect("fake authorized remainder refund");
    println!(
        "{}",
        serde_json::json!({"schema":"openagents.custody-qualification.v1","fake_rail":true,
  "production_custody_available":production_custody_available(),"study":recovered,
  "deposit_msat":1100,"released_msat":1000,"refunded_msat":90,"fees_msat":10,"held_msat":recovered.held_msat().unwrap(),
  "funded_qualification":"unverified"})
    );
}
