//! #10707: fake-wallet top-ups credit the shared balance exactly once.

use std::sync::{Arc, Barrier};
use std::thread;

use pay_ledger::{
    Ledger,
    compute::{Binding, PrincipalKind, PurchaseState, Rights, credential_digest},
};
use retail_cloud::{
    Error,
    fake::FakeWallet,
    topup::{self, TopUpRequest},
};

const NOW: i64 = 1_791_000_000;

fn setup(ledger: &mut Ledger) {
    ledger.create_compute_account("acct-a", NOW).unwrap();
    for (principal, spend) in [("cli:laptop", true), ("phone:p1", false)] {
        ledger
            .bind_principal(&Binding {
                principal: principal.into(),
                account: "acct-a".into(),
                kind: PrincipalKind::Cli,
                credential: credential_digest(principal),
                rights: Rights { read: true, spend },
                at: NOW,
            })
            .unwrap();
    }
}

fn request(purchase: &str, sats: u64) -> TopUpRequest {
    TopUpRequest {
        principal: "cli:laptop".into(),
        credential: credential_digest("cli:laptop"),
        purchase: purchase.into(),
        amount_sats: sats,
        now: NOW,
    }
}

#[test]
fn exact_retry_reuses_one_invoice_and_duplicate_callbacks_credit_once() {
    let mut ledger = Ledger::in_memory().unwrap();
    setup(&mut ledger);
    let wallet = FakeWallet::new();
    let first = topup::request_top_up(&mut ledger, &wallet, &request("p-1", 500)).unwrap();
    let again = topup::request_top_up(&mut ledger, &wallet, &request("p-1", 500)).unwrap();
    assert_eq!(first, again);
    assert_eq!(wallet.issued(), 1);
    assert!(matches!(
        topup::request_top_up(&mut ledger, &wallet, &request("p-1", 600)),
        Err(Error::Conflict(_))
    ));
    assert_eq!(ledger.credited("acct-a").unwrap(), 0);

    let hash = first.top_up.payment_hash.clone();
    wallet.pay_in_full(&hash);
    for _ in 0..3 {
        let paid = topup::on_paid(&mut ledger, &hash, 500_000, NOW + 10).unwrap();
        assert_eq!(paid.state, PurchaseState::Paid);
    }
    let pass = topup::reconcile(&mut ledger, &wallet, NOW + 20).unwrap();
    assert_eq!(pass.paid, 0, "a paid purchase is no longer open");
    assert_eq!(ledger.credited("acct-a").unwrap(), 500_000);
}

#[test]
fn a_read_only_principal_cannot_buy() {
    let mut ledger = Ledger::in_memory().unwrap();
    setup(&mut ledger);
    let wallet = FakeWallet::new();
    let mut phone = request("p-2", 100);
    phone.principal = "phone:p1".into();
    phone.credential = credential_digest("phone:p1");
    assert!(matches!(
        topup::request_top_up(&mut ledger, &wallet, &phone),
        Err(Error::Ledger(pay_ledger::Error::Denied(_)))
    ));
    assert_eq!(wallet.issued(), 0);
}

#[test]
fn unpaid_expired_unknown_and_short_receipts_never_credit() {
    let mut ledger = Ledger::in_memory().unwrap();
    setup(&mut ledger);
    let wallet = FakeWallet::new();
    let pending = topup::request_top_up(&mut ledger, &wallet, &request("pending", 100)).unwrap();
    let expired = topup::request_top_up(&mut ledger, &wallet, &request("expired", 100)).unwrap();
    let forgotten = topup::request_top_up(&mut ledger, &wallet, &request("forgot", 100)).unwrap();
    let short = topup::request_top_up(&mut ledger, &wallet, &request("short", 100)).unwrap();
    wallet.fail(&expired.top_up.payment_hash);
    wallet.forget(&forgotten.top_up.payment_hash);
    wallet.pay_amount(&short.top_up.payment_hash, 99_000);

    // Before expiry, a failure report leaves the invoice payable.
    let pass = topup::reconcile(&mut ledger, &wallet, NOW + 60).unwrap();
    assert_eq!(pass.pending, 2);
    assert_eq!(pass.unknown, 2);
    // After expiry, unpaid invoices expire.
    let pass = topup::reconcile(&mut ledger, &wallet, NOW + 1_000).unwrap();
    assert_eq!(pass.expired, 2);
    assert_eq!(
        pass.unknown, 2,
        "unknown purchases stay unknown without a full payment"
    );
    assert_eq!(ledger.credited("acct-a").unwrap(), 0);
    let states: Vec<_> = ledger
        .top_ups("acct-a")
        .unwrap()
        .into_iter()
        .map(|p| (p.top_up.id, p.state))
        .collect();
    assert!(states.contains(&("pending".into(), PurchaseState::Expired)));
    assert!(states.contains(&("expired".into(), PurchaseState::Expired)));
    assert!(states.contains(&("forgot".into(), PurchaseState::Unknown)));
    assert!(states.contains(&("short".into(), PurchaseState::Unknown)));
    let _ = pending;
}

#[test]
fn a_delayed_receipt_after_expiry_still_credits_once() {
    let mut ledger = Ledger::in_memory().unwrap();
    setup(&mut ledger);
    let wallet = FakeWallet::new();
    let p = topup::request_top_up(&mut ledger, &wallet, &request("late", 250)).unwrap();
    topup::reconcile(&mut ledger, &wallet, NOW + 1_000).unwrap();
    assert_eq!(
        ledger.top_up("late").unwrap().unwrap().state,
        PurchaseState::Expired
    );
    // The wallet reports the payment after all: money arrived, so it counts.
    wallet.pay_in_full(&p.top_up.payment_hash);
    topup::on_paid(&mut ledger, &p.top_up.payment_hash, 250_000, NOW + 1_100).unwrap();
    topup::on_paid(&mut ledger, &p.top_up.payment_hash, 250_000, NOW + 1_200).unwrap();
    assert_eq!(ledger.credited("acct-a").unwrap(), 250_000);
}

#[test]
fn a_crash_between_receipt_and_credit_recovers_on_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.sqlite");
    let wallet = FakeWallet::new();
    let hash = {
        let mut ledger = Ledger::open(&path).unwrap();
        setup(&mut ledger);
        let p = topup::request_top_up(&mut ledger, &wallet, &request("crash", 300)).unwrap();
        // The wallet receives the payment; the process dies before any
        // callback reaches the ledger.
        wallet.pay_in_full(&p.top_up.payment_hash);
        p.top_up.payment_hash
    };
    let mut ledger = Ledger::open(&path).unwrap();
    assert_eq!(ledger.credited("acct-a").unwrap(), 0);
    wallet.set_unreachable(true);
    let pass = topup::reconcile(&mut ledger, &wallet, NOW + 30).unwrap();
    assert_eq!(pass.unreachable, 1);
    assert_eq!(ledger.credited("acct-a").unwrap(), 0);
    wallet.set_unreachable(false);
    let pass = topup::reconcile(&mut ledger, &wallet, NOW + 40).unwrap();
    assert_eq!(pass.paid, 1);
    topup::on_paid(&mut ledger, &hash, 300_000, NOW + 41).unwrap();
    assert_eq!(ledger.credited("acct-a").unwrap(), 300_000);
    let purchase = ledger.top_up("crash").unwrap().unwrap();
    assert_eq!(purchase.top_up.payment_hash, hash);
}

#[test]
fn concurrent_observers_credit_one_account_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.sqlite");
    let wallet = FakeWallet::new();
    let hash = {
        let mut ledger = Ledger::open(&path).unwrap();
        setup(&mut ledger);
        topup::request_top_up(&mut ledger, &wallet, &request("race", 700))
            .unwrap()
            .top_up
            .payment_hash
    };
    let barrier = Arc::new(Barrier::new(8));
    let handles: Vec<_> = (0..8)
        .map(|i| {
            let path = path.clone();
            let hash = hash.clone();
            let barrier = barrier.clone();
            thread::spawn(move || {
                let mut ledger = Ledger::open(&path).unwrap();
                barrier.wait();
                topup::on_paid(&mut ledger, &hash, 700_000, NOW + i).unwrap();
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
    let ledger = Ledger::open(&path).unwrap();
    assert_eq!(ledger.credited("acct-a").unwrap(), 700_000);
}
