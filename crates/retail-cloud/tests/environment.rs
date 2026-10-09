//! Saved customer environments (ENV-10) against fake payments: holds,
//! settlement, prepaid retention, refusals, and restart recovery.
mod common;
use common::*;
use pay_ledger::Ledger;
use pay_ledger::compute::HoldState;
use retail_cloud::Error;
use retail_cloud::authority::Source;
use retail_cloud::environment::{
    self, BookStatus, Ending, EnvironmentRequest, Gate, Phase, PriceBook, Refusal, Saved, Usage,
};
use retail_cloud::journal::Journal;

const DAY: i64 = 86_400;

fn book() -> PriceBook {
    let mut b = environment::price_book();
    b.status = BookStatus::Published;
    b
}
fn gate(b: &PriceBook) -> Gate {
    Gate {
        contract_reviewed: true,
        book: Some(b.digest()),
        qualification: Some("funded-qualification-receipt".into()),
    }
}
fn request() -> EnvironmentRequest {
    EnvironmentRequest {
        source: Source {
            repository: "https://github.com/OpenAgentsInc/example".into(),
            commit: "c".repeat(40),
        },
        objective: "Build and test the Rust workspace.".into(),
        profile: "rust-library".into(),
        checks: vec!["cargo test -p parser".into()],
        max_seconds: 3600,
        retention_days: 30,
        ceiling_sats: None,
    }
}
fn saved() -> Saved {
    Saved {
        environment: "env-1".into(),
        version: "env-1-v1".into(),
        image_id: "oaenv-build-1-abc".into(),
    }
}
fn balance(l: &Ledger, account: &str) -> pay_ledger::compute::ComputeBalance {
    l.compute_balance(account).unwrap()
}
fn refusal(e: Error) -> Refusal {
    match e {
        Error::Environment(r) => r,
        other => panic!("expected an environment refusal, got {other:?}"),
    }
}

#[test]
fn quotes_follow_the_book() {
    let b = book();
    let q = b.quote(&request()).unwrap();
    // 3600 s x 4 machines x 40 msat = 576; 50 GB x 30 days x 3 sat = 4500.
    assert_eq!(q.max_sats, 576 + 200 + 4500);
    let mut r = request();
    r.retention_days = 91;
    assert!(b.quote(&r).is_err());
    r.retention_days = 30;
    r.max_seconds = 7201;
    assert!(b.quote(&r).is_err());
    r.max_seconds = 3600;
    r.ceiling_sats = Some(1000);
    assert!(matches!(b.quote(&r), Err(Refusal::AboveCeiling { .. })));
    r.ceiling_sats = None;
    r.source.repository = "https://gitlab.com/o/r".into();
    assert!(b.quote(&r).is_err());
    assert_eq!(b.renewal(10, 30).unwrap(), 900);
    assert!(b.renewal(51, 30).is_err());
    // Settlements: measured seconds, coordination, and the real image.
    let none = q.settle(Ending::ProviderUnavailable, None);
    assert_eq!(
        (none.charge_sats, none.released_sats),
        (Some(0), q.max_sats)
    );
    let unknown = q.settle(Ending::Unknown, None);
    assert_eq!((unknown.charge_sats, unknown.held_sats), (None, q.max_sats));
    let failed = q.settle(
        Ending::Ended,
        Some(Usage {
            machine_seconds: 1000,
            image_gb: None,
        }),
    );
    assert_eq!(failed.charge_sats, Some(40 + 200));
    let kept = q.settle(
        Ending::Saved,
        Some(Usage {
            machine_seconds: 1000,
            image_gb: Some(10),
        }),
    );
    assert_eq!(kept.charge_sats, Some(40 + 200 + 900));
    let capped = q.settle(
        Ending::Ended,
        Some(Usage {
            machine_seconds: 10 * 3600 * 4,
            image_gb: None,
        }),
    );
    assert_eq!(capped.charge_sats, Some(576 + 200));
}

#[test]
fn nothing_sells_until_the_owner_opens_it() {
    let mut j = Journal::in_memory().unwrap();
    let proposed = environment::price_book();
    let closed = Gate {
        contract_reviewed: true,
        book: Some(proposed.digest()),
        qualification: Some("x".into()),
    };
    let e =
        environment::offer(&mut j, &proposed, &closed, "acct", "p1", &request(), NOW).unwrap_err();
    assert_eq!(refusal(e), Refusal::Closed);
    let b = book();
    for g in [
        Gate::default(),
        Gate {
            qualification: None,
            ..gate(&b)
        },
        Gate {
            contract_reviewed: false,
            ..gate(&b)
        },
    ] {
        let e = environment::offer(&mut j, &b, &g, "acct", "p1", &request(), NOW).unwrap_err();
        assert_eq!(refusal(e), Refusal::Closed);
    }
    assert!(environment::purchases(&j).unwrap().is_empty());
}

#[test]
fn a_saved_environment_is_held_settled_once_and_kept_for_its_paid_days() {
    let mut j = Journal::in_memory().unwrap();
    let mut l = Ledger::in_memory().unwrap();
    funded_account(&mut l, "acct", 10_000);
    let b = book();
    let g = gate(&b);
    let p = environment::offer(&mut j, &b, &g, "acct", "p1", &request(), NOW).unwrap();
    assert_eq!(p.admission.selectable_by, "acct");
    assert!(!p.admission.terminal && p.admission.publication.is_empty());
    assert_eq!(p.admission.credentials, vec!["customer:openai".to_string()]);
    // A retry returns the offer; other terms conflict.
    assert_eq!(
        environment::offer(&mut j, &b, &g, "acct", "p1", &request(), NOW + 1).unwrap(),
        p
    );
    let mut other = request();
    other.retention_days = 7;
    assert!(matches!(
        environment::offer(&mut j, &b, &g, "acct", "p1", &other, NOW),
        Err(Error::Conflict(_))
    ));
    // Confirm holds the maximum, once.
    let max = i64::try_from(p.quote.max_sats * 1000).unwrap();
    let c = environment::confirm(
        &mut j,
        &mut l,
        &b,
        &g,
        "acct",
        "p1",
        &p.digest,
        true,
        NOW + 2,
    )
    .unwrap();
    assert!(matches!(c.phase, Phase::Confirmed { .. }));
    assert_eq!(balance(&l, "acct").held_msat, max);
    environment::confirm(
        &mut j,
        &mut l,
        &b,
        &g,
        "acct",
        "p1",
        &p.digest,
        true,
        NOW + 3,
    )
    .unwrap();
    assert_eq!(balance(&l, "acct").held_msat, max);
    // Saved with a 10 GB image after 1000 machine-seconds.
    let usage = Usage {
        machine_seconds: 1000,
        image_gb: Some(10),
    };
    environment::end(
        &mut j,
        &mut l,
        "p1",
        Ending::Saved,
        Some(usage),
        Some(saved()),
        NOW + 4,
    )
    .unwrap();
    let receipt = environment::settle(&mut j, &mut l, "p1", NOW + 5).unwrap();
    assert_eq!(receipt.charge_msat, Some((40 + 200 + 900) * 1000));
    let after = balance(&l, "acct");
    assert_eq!(after.held_msat, 0);
    assert_eq!(after.settled_msat, (40 + 200 + 900) * 1000);
    assert_eq!(after.available_msat, 10_000_000 - (40 + 200 + 900) * 1000);
    // Settling again changes nothing.
    assert_eq!(
        environment::settle(&mut j, &mut l, "p1", NOW + 6).unwrap(),
        receipt
    );
    assert_eq!(balance(&l, "acct"), after);
    // Only the buying account may select it, while its days are paid.
    let p = environment::purchase(&j, "acct", "p1").unwrap().unwrap();
    assert!(environment::may_select(&p, "acct", NOW + 10));
    assert!(!environment::may_select(&p, "other", NOW + 10));
    assert_eq!(
        refusal(environment::purchase(&j, "other", "p1").unwrap_err()),
        Refusal::NotYours
    );
    // A renewal is prepaid at the measured size; a retry is the same one.
    let r = environment::renew(
        &mut j,
        &mut l,
        &b,
        &g,
        "acct",
        "p1",
        "r1",
        30,
        true,
        NOW + 20,
    )
    .unwrap();
    let paid = r.retention.as_ref().unwrap().paid_until;
    assert_eq!(paid, NOW + 5 + 60 * DAY);
    let charged = balance(&l, "acct").settled_msat;
    assert_eq!(charged, (40 + 200 + 900 + 900) * 1000);
    environment::renew(
        &mut j,
        &mut l,
        &b,
        &g,
        "acct",
        "p1",
        "r1",
        30,
        true,
        NOW + 21,
    )
    .unwrap();
    assert_eq!(balance(&l, "acct").settled_msat, charged);
    assert!(matches!(
        environment::renew(
            &mut j,
            &mut l,
            &b,
            &g,
            "acct",
            "p1",
            "r1",
            7,
            true,
            NOW + 21
        ),
        Err(Error::Conflict(_))
    ));
    // Past its paid days the version lapses and its image is due for
    // deletion; it cannot be selected or renewed.
    assert!(environment::lapse(&mut j, paid - 1).unwrap().is_empty());
    let due = environment::lapse(&mut j, paid).unwrap();
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].saved, saved());
    let p = environment::purchase(&j, "acct", "p1").unwrap().unwrap();
    assert!(!environment::may_select(&p, "acct", paid));
    assert_eq!(
        refusal(
            environment::renew(
                &mut j,
                &mut l,
                &b,
                &g,
                "acct",
                "p1",
                "r2",
                7,
                true,
                paid + 1
            )
            .unwrap_err()
        ),
        Refusal::Lapsed
    );
    assert!(environment::lapse(&mut j, paid + 1).unwrap().is_empty());
}

#[test]
fn refusals_hold_nothing() {
    let mut j = Journal::in_memory().unwrap();
    let mut l = Ledger::in_memory().unwrap();
    funded_account(&mut l, "poor", 100);
    funded_account(&mut l, "acct", 10_000);
    let b = book();
    let g = gate(&b);
    let p = environment::offer(&mut j, &b, &g, "poor", "p1", &request(), NOW).unwrap();
    // Not enough balance: nothing held, still an offer.
    assert!(matches!(
        environment::confirm(&mut j, &mut l, &b, &g, "poor", "p1", &p.digest, true, NOW),
        Err(Error::Ledger(pay_ledger::Error::Insufficient { .. }))
    ));
    assert_eq!(balance(&l, "poor").held_msat, 0);
    assert!(matches!(
        environment::purchase(&j, "poor", "p1")
            .unwrap()
            .unwrap()
            .phase,
        Phase::Offered
    ));
    // Another account, another digest, or no spend right.
    let p = environment::offer(&mut j, &b, &g, "acct", "p2", &request(), NOW).unwrap();
    assert_eq!(
        refusal(
            environment::confirm(&mut j, &mut l, &b, &g, "poor", "p2", &p.digest, true, NOW)
                .unwrap_err()
        ),
        Refusal::NotYours
    );
    let wrong = route_contract::digest_of(&"other");
    assert!(matches!(
        environment::confirm(&mut j, &mut l, &b, &g, "acct", "p2", &wrong, true, NOW),
        Err(Error::Conflict(_))
    ));
    assert_eq!(
        refusal(
            environment::confirm(&mut j, &mut l, &b, &g, "acct", "p2", &p.digest, false, NOW)
                .unwrap_err()
        ),
        Refusal::NoSpendRight
    );
    // A new book after the offer: the shown quote no longer holds.
    let mut moved = book();
    moved.class.coordination_sats = 300;
    let moved_gate = gate(&moved);
    assert_eq!(
        refusal(
            environment::confirm(
                &mut j,
                &mut l,
                &moved,
                &moved_gate,
                "acct",
                "p2",
                &p.digest,
                true,
                NOW
            )
            .unwrap_err()
        ),
        Refusal::Changed
    );
    assert_eq!(balance(&l, "acct").held_msat, 0);
    // A saved image above the class's largest is refused.
    environment::confirm(&mut j, &mut l, &b, &g, "acct", "p2", &p.digest, true, NOW).unwrap();
    let big = Usage {
        machine_seconds: 10,
        image_gb: Some(51),
    };
    assert_eq!(
        refusal(
            environment::end(
                &mut j,
                &mut l,
                "p2",
                Ending::Saved,
                Some(big),
                Some(saved()),
                NOW
            )
            .unwrap_err()
        ),
        Refusal::ImageTooLarge
    );
}

#[test]
fn unknown_endings_stay_held_until_known_and_no_machine_means_no_charge() {
    let mut j = Journal::in_memory().unwrap();
    let mut l = Ledger::in_memory().unwrap();
    funded_account(&mut l, "acct", 20_000);
    let b = book();
    let g = gate(&b);
    for id in ["lost", "never"] {
        let p = environment::offer(&mut j, &b, &g, "acct", id, &request(), NOW).unwrap();
        environment::confirm(&mut j, &mut l, &b, &g, "acct", id, &p.digest, true, NOW).unwrap();
    }
    let max = i64::try_from(b.quote(&request()).unwrap().max_sats * 1000).unwrap();
    environment::end(&mut j, &mut l, "lost", Ending::Unknown, None, None, NOW + 1).unwrap();
    assert_eq!(
        l.hold("env:lost").unwrap().unwrap().state,
        HoldState::Unknown
    );
    let held = environment::settle(&mut j, &mut l, "lost", NOW + 2).unwrap();
    assert_eq!((held.charge_msat, held.held_msat), (None, max));
    environment::end(
        &mut j,
        &mut l,
        "never",
        Ending::ProviderUnavailable,
        None,
        None,
        NOW + 1,
    )
    .unwrap();
    let free = environment::settle(&mut j, &mut l, "never", NOW + 2).unwrap();
    assert_eq!(free.charge_msat, Some(0));
    // The operator learns the usage: the known ending settles it.
    let usage = Usage {
        machine_seconds: 500,
        image_gb: None,
    };
    environment::end(
        &mut j,
        &mut l,
        "lost",
        Ending::Cancelled,
        Some(usage),
        None,
        NOW + 3,
    )
    .unwrap();
    // A known ending never changes.
    assert!(matches!(
        environment::end(
            &mut j,
            &mut l,
            "lost",
            Ending::Ended,
            Some(usage),
            None,
            NOW + 4
        ),
        Err(Error::Conflict(_))
    ));
    let r = environment::settle(&mut j, &mut l, "lost", NOW + 5).unwrap();
    assert_eq!(r.charge_msat, Some((20 + 200) * 1000));
    let after = balance(&l, "acct");
    assert_eq!(after.held_msat, 0);
    assert_eq!(
        after.credited_msat,
        after.available_msat + after.held_msat + after.settled_msat
    );
}

#[test]
fn a_restart_finishes_each_step_once() {
    let dir = tempfile::tempdir().unwrap();
    let jp = dir.path().join("journal.sqlite");
    let lp = dir.path().join("ledger.sqlite");
    let b = book();
    let g = gate(&b);
    let (digest, max) = {
        let mut j = Journal::open(&jp).unwrap();
        let mut l = Ledger::open(&lp).unwrap();
        funded_account(&mut l, "acct", 20_000);
        let p = environment::offer(&mut j, &b, &g, "acct", "a", &request(), NOW).unwrap();
        let q = environment::offer(&mut j, &b, &g, "acct", "b", &request(), NOW).unwrap();
        // "a": the hold landed but the reply was lost before the journal
        // recorded it (a confirm on another connection that crashed).
        let mut other = Journal::open(&jp).unwrap();
        environment::confirm(
            &mut other, &mut l, &b, &g, "acct", "a", &p.digest, true, NOW,
        )
        .unwrap();
        // Roll the journal's record back to the offer, as if never written.
        let mut offered = environment::purchase(&j, "acct", "a").unwrap().unwrap();
        offered.phase = Phase::Offered;
        drop(other);
        let raw = rusqlite::Connection::open(&jp).unwrap();
        raw.execute(
            "UPDATE environment_purchase SET bytes=? WHERE id='a'",
            [serde_json::to_string(&offered).unwrap()],
        )
        .unwrap();
        // "b": ended but not yet settled.
        environment::confirm(&mut j, &mut l, &b, &g, "acct", "b", &q.digest, true, NOW).unwrap();
        let usage = Usage {
            machine_seconds: 100,
            image_gb: Some(2),
        };
        environment::end(
            &mut j,
            &mut l,
            "b",
            Ending::Saved,
            Some(usage),
            Some(saved()),
            NOW + 1,
        )
        .unwrap();
        (p.digest, i64::try_from(q.quote.max_sats * 1000).unwrap())
    };
    let mut j = Journal::open(&jp).unwrap();
    let mut l = Ledger::open(&lp).unwrap();
    let before = balance(&l, "acct");
    assert_eq!(before.held_msat, 2 * max);
    let r = environment::recover(&mut j, &mut l, NOW + 10).unwrap();
    assert_eq!(r.confirmed, vec!["a".to_string()]);
    assert_eq!(r.settled, vec!["b".to_string()]);
    let after = balance(&l, "acct");
    let charge = (4 + 200 + 2 * 30 * 3) * 1000;
    assert_eq!(after.settled_msat, charge);
    assert_eq!(after.held_msat, max);
    // A second visit and a repeated confirmation change nothing.
    let again = environment::recover(&mut j, &mut l, NOW + 11).unwrap();
    assert!(again.confirmed.is_empty() && again.settled.is_empty());
    environment::confirm(&mut j, &mut l, &b, &g, "acct", "a", &digest, true, NOW + 12).unwrap();
    assert_eq!(balance(&l, "acct"), after);
    // Recovery also lapses retention that ran out while the host was down.
    let r = environment::recover(&mut j, &mut l, NOW + 1 + 31 * DAY).unwrap();
    assert_eq!(r.lapsed.len(), 1);
    assert_eq!(r.lapsed[0].purchase, "b");
}
