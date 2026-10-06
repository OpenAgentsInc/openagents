//! #10710: a funded request holds its maximum before provisioning.

mod common;

use common::{NOW, confirmed, funded_account, request, rights};
use pay_ledger::Ledger;
use pay_ledger::compute::HoldState;
use retail_cloud::Error;
use retail_cloud::authority::Authority;
use retail_cloud::journal::Journal;
use retail_cloud::reserve;

#[test]
fn a_confirmed_offer_holds_its_maximum_once() {
    let mut ledger = Ledger::in_memory().unwrap();
    let mut journal = Journal::in_memory().unwrap();
    funded_account(&mut ledger, "acct", 300);
    let funded = confirmed(&mut journal, "acct", "cf_1", &request(3_600));
    let current = rights(&funded);
    let hold = reserve::reserve(&mut ledger, &funded, &current, NOW + 2).unwrap();
    assert_eq!(hold.request.amount_msat, 244_000);
    assert_eq!(hold.state, HoldState::Held);
    assert_eq!(hold.request.execution, funded.execution);
    // A retry reuses the hold.
    assert_eq!(
        reserve::reserve(&mut ledger, &funded, &current, NOW + 9).unwrap(),
        hold
    );
    let balance = ledger.compute_balance("acct").unwrap();
    assert_eq!(balance.available_msat, 56_000);
    // A second offer does not fit.
    let second = confirmed(&mut journal, "acct", "cf_2", &request(3_600));
    assert!(matches!(
        reserve::reserve(&mut ledger, &second, &rights(&second), NOW + 3),
        Err(Error::Ledger(pay_ledger::Error::Insufficient { .. }))
    ));
}

#[test]
fn reserving_needs_the_spend_right_now() {
    let mut ledger = Ledger::in_memory().unwrap();
    let mut journal = Journal::in_memory().unwrap();
    funded_account(&mut ledger, "acct", 300);
    let funded = confirmed(&mut journal, "acct", "cf_1", &request(600));
    let mut current = rights(&funded);
    current.spend = None;
    current.invoice_paid = true;
    current.balance_msat = 300_000;
    match reserve::reserve(&mut ledger, &funded, &current, NOW + 2) {
        Err(Error::Denied(denial)) => assert_eq!(denial.authority, Authority::Spend),
        other => panic!("{other:?}"),
    }
    assert!(ledger.holds("acct").unwrap().is_empty());
}
