//! #10710 and #10718: holds never overspend, retries reuse one hold,
//! unknown holds survive restarts, and settlement posts one debit.

use std::sync::{Arc, Barrier};
use std::thread;

use pay_ledger::{
    Error, Ledger,
    compute::{ComputeBalance, HoldRequest, HoldState, Receipt, TopUp},
};

const AT: i64 = 1_791_000_000;

fn fund(ledger: &mut Ledger, account: &str, purchase: &str, msat: i64) {
    ledger.create_compute_account(account, AT).unwrap();
    let hash = pay_ledger::digest(purchase);
    ledger
        .open_top_up(&TopUp {
            id: purchase.into(),
            account: account.into(),
            amount_msat: msat,
            payment_hash: hash.clone(),
            invoice: format!("lnfake-{purchase}"),
            created_at: AT,
            expires_at: AT + 900,
        })
        .unwrap();
    ledger
        .observe_top_up(
            &hash,
            &Receipt::Paid {
                received_msat: msat,
                at: AT + 1,
            },
        )
        .unwrap();
}

fn hold(id: &str, msat: i64) -> HoldRequest {
    HoldRequest {
        id: id.into(),
        account: "acct".into(),
        quote: format!("quote-{id}"),
        execution: format!("exec-{id}"),
        terms: format!("terms-{id}"),
        amount_msat: msat,
        at: AT + 2,
    }
}

fn conserved(balance: ComputeBalance) {
    assert_eq!(
        balance.credited_msat,
        balance.available_msat + balance.held_msat + balance.settled_msat
    );
    assert!(balance.available_msat >= 0);
}

#[test]
fn concurrent_requests_cannot_overspend() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.sqlite");
    {
        let mut ledger = Ledger::open(&path).unwrap();
        // Room for exactly three 244-sat holds.
        fund(&mut ledger, "acct", "p", 800_000);
    }
    let barrier = Arc::new(Barrier::new(10));
    let handles: Vec<_> = (0..10)
        .map(|i| {
            let path = path.clone();
            let barrier = barrier.clone();
            thread::spawn(move || {
                let mut ledger = Ledger::open(&path).unwrap();
                barrier.wait();
                ledger.reserve(&hold(&format!("r{i}"), 244_000)).is_ok()
            })
        })
        .collect();
    let granted = handles
        .into_iter()
        .map(|h| h.join().unwrap())
        .filter(|ok| *ok)
        .count();
    assert_eq!(granted, 3);
    let ledger = Ledger::open(&path).unwrap();
    let balance = ledger.compute_balance("acct").unwrap();
    assert_eq!(balance.held_msat, 732_000);
    conserved(balance);
}

#[test]
fn retries_reuse_one_hold_and_changed_terms_conflict() {
    let mut ledger = Ledger::in_memory().unwrap();
    fund(&mut ledger, "acct", "p", 1_000_000);
    let first = ledger.reserve(&hold("r", 244_000)).unwrap();
    let mut retry = hold("r", 244_000);
    retry.at += 30;
    assert_eq!(ledger.reserve(&retry).unwrap(), first);
    for change in 0..4 {
        let mut changed = hold("r", 244_000);
        match change {
            0 => changed.terms = "other bytes".into(),
            1 => changed.quote = "other quote".into(),
            2 => changed.amount_msat = 100_000,
            _ => changed.execution = "exec-other".into(),
        }
        assert!(matches!(ledger.reserve(&changed), Err(Error::Conflict(_))));
    }
    // Another request may not reuse the execution identity.
    let mut thief = hold("r2", 1_000);
    thief.execution = "exec-r".into();
    assert!(matches!(ledger.reserve(&thief), Err(Error::Conflict(_))));
    assert_eq!(ledger.compute_balance("acct").unwrap().held_msat, 244_000);
    assert!(matches!(
        ledger.reserve(&hold("big", 900_000)),
        Err(Error::Insufficient {
            available_msat: 756_000
        })
    ));
}

#[test]
fn a_restart_never_frees_an_uncertain_hold() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.sqlite");
    {
        let mut ledger = Ledger::open(&path).unwrap();
        fund(&mut ledger, "acct", "p", 500_000);
        ledger.reserve(&hold("r", 244_000)).unwrap();
        ledger.mark_hold_unknown("r").unwrap();
    }
    let mut ledger = Ledger::open(&path).unwrap();
    let found = ledger.hold("r").unwrap().unwrap();
    assert_eq!(found.state, HoldState::Unknown);
    let balance = ledger.compute_balance("acct").unwrap();
    assert_eq!(balance.held_msat, 244_000);
    assert_eq!(balance.available_msat, 256_000);
    // Marking it unknown again changes nothing; only a known charge settles.
    ledger.mark_hold_unknown("r").unwrap();
    let (settled, recorded) = ledger.settle_hold("r", 103_000, AT + 99).unwrap();
    assert_eq!(settled.released_msat(), Some(141_000));
    assert_eq!(recorded.unwrap().price_msat, 103_000);
    conserved(ledger.compute_balance("acct").unwrap());
}

#[test]
fn settlement_replays_post_one_debit_and_conserve() {
    let mut ledger = Ledger::in_memory().unwrap();
    fund(&mut ledger, "acct", "p", 1_000_000);
    ledger.reserve(&hold("ok", 244_000)).unwrap();
    ledger.reserve(&hold("none", 244_000)).unwrap();
    ledger.reserve(&hold("full", 124_000)).unwrap();
    let (_, first) = ledger.settle_hold("ok", 148_000, AT + 10).unwrap();
    let (_, again) = ledger.settle_hold("ok", 148_000, AT + 11).unwrap();
    assert_eq!(first, again);
    assert!(matches!(
        ledger.settle_hold("ok", 149_000, AT + 12),
        Err(Error::Conflict(_))
    ));
    assert!(matches!(
        ledger.settle_hold("full", 124_001, AT + 12),
        Err(Error::Invalid(_))
    ));
    let (none, recorded) = ledger.settle_hold("none", 0, AT + 13).unwrap();
    assert!(recorded.is_none());
    assert_eq!(none.released_msat(), Some(244_000));
    ledger.settle_hold("full", 124_000, AT + 14).unwrap();
    let balance = ledger.compute_balance("acct").unwrap();
    assert_eq!(balance.settled_msat, 272_000);
    assert_eq!(balance.released_msat, 96_000 + 244_000);
    assert_eq!(balance.held_msat, 0);
    conserved(balance);
    // One debit per settled hold, all to OpenAgents, never above receipts.
    let totals = ledger.totals().unwrap();
    assert_eq!(totals.settlements, 2);
    assert_eq!(totals.received_msat, 272_000);
}

#[test]
fn random_sequences_conserve() {
    let mut ledger = Ledger::in_memory().unwrap();
    fund(&mut ledger, "acct", "p0", 2_000_000);
    let mut seed: u64 = 7;
    let mut next = || {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        seed >> 33
    };
    let mut open: Vec<String> = Vec::new();
    for step in 0..400 {
        match next() % 5 {
            0 | 1 => {
                let id = format!("h{step}");
                if ledger
                    .reserve(&hold(&id, 1_000 * (1 + (next() % 244) as i64)))
                    .is_ok()
                {
                    open.push(id);
                }
            }
            2 if !open.is_empty() => {
                let id = open.remove((next() as usize) % open.len());
                let amount = ledger.hold(&id).unwrap().unwrap().request.amount_msat;
                let charge = (next() as i64) % (amount + 1);
                ledger.settle_hold(&id, charge, AT + step).unwrap();
            }
            3 if !open.is_empty() => {
                let id = &open[(next() as usize) % open.len()];
                ledger.mark_hold_unknown(id).unwrap();
            }
            _ => fund(&mut ledger, "acct", &format!("p{step}"), 50_000),
        }
        conserved(ledger.compute_balance("acct").unwrap());
    }
}
