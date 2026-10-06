//! #10721: metered Lightning sessions on a mock rail conserve every
//! deposit across debits, disconnects, expiry, closure, and refunds.

use pay_ledger::{
    Error, Ledger,
    session::{Debit, OpenSession, RefundState, SessionState, Summary},
};

const AT: i64 = 1_792_000_000;

fn open(id: &str, hash_seed: &str) -> OpenSession {
    OpenSession {
        id: id.into(),
        deposit_hash: pay_ledger::digest(hash_seed),
        deposit_msat: 100_000,
        ceiling_msat: 80_000,
        rate_msat_per_unit: 40,
        unit: "second".into(),
        admission: "sha256:admission".into(),
        recipients: vec!["openagents".into()],
        return_destination: "lno1fakeofferforrefunds".into(),
        opened_at: AT,
        expires_at: AT + 3_600,
    }
}

fn debit(record: &str, units: i64, at: i64) -> Debit {
    Debit {
        record: record.into(),
        node: "task-1".into(),
        admission: "sha256:admission".into(),
        recipient: "openagents".into(),
        units,
        at,
    }
}

fn conserved(s: &Summary) {
    assert_eq!(
        s.deposit_msat,
        s.debited_msat + s.refunded_msat + s.owed_msat + s.open_remainder_msat
    );
}

#[test]
fn deposit_debits_disconnect_close_and_remainder_return_conserve() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.sqlite");
    {
        let mut ledger = Ledger::open(&path).unwrap();
        let s = ledger.open_session(&open("s1", "deposit-1")).unwrap();
        assert_eq!(s.open_remainder_msat, 100_000);
        let s = ledger
            .debit_session("s1", &debit("r1", 500, AT + 10))
            .unwrap();
        assert_eq!(s.debited_msat, 20_000);
        conserved(&s);
    }
    // The client disconnects and reconnects: the same deposit, the same
    // record, and the same terms change nothing.
    let mut ledger = Ledger::open(&path).unwrap();
    let again = ledger.open_session(&open("s1", "deposit-1")).unwrap();
    assert_eq!(again.debited_msat, 20_000);
    assert!(matches!(
        ledger.open_session(&open("s2", "deposit-1")),
        Err(Error::Conflict(_))
    ));
    ledger
        .debit_session("s1", &debit("r1", 500, AT + 11))
        .unwrap();
    let mut changed = debit("r1", 501, AT + 11);
    changed.record = "r1".into();
    assert!(matches!(
        ledger.debit_session("s1", &changed),
        Err(Error::Conflict(_))
    ));
    let s = ledger
        .debit_session("s1", &debit("r2", 250, AT + 20))
        .unwrap();
    assert_eq!(s.debited_msat, 30_000);
    // The ceiling holds.
    assert!(matches!(
        ledger.debit_session("s1", &debit("r3", 1_300, AT + 30)),
        Err(Error::Insufficient {
            available_msat: 50_000
        })
    ));
    // Each debit posted one settlement.
    assert_eq!(ledger.totals().unwrap().settlements, 2);
    assert_eq!(ledger.totals().unwrap().received_msat, 30_000);

    let closed = ledger.close_session("s1", AT + 40).unwrap();
    assert_eq!(closed.state, SessionState::Closed);
    assert_eq!(closed.owed_msat, 70_000);
    assert_eq!(closed.refund, Some(RefundState::Planned));
    conserved(&closed);
    assert_eq!(ledger.close_session("s1", AT + 41).unwrap(), closed);
    assert!(matches!(
        ledger.debit_session("s1", &debit("r4", 1, AT + 42)),
        Err(Error::Denied(_))
    ));
    // Return the remainder.
    assert!(
        ledger
            .set_refund("s1", RefundState::Sending, None, AT + 50)
            .is_err()
    );
    ledger
        .set_refund("s1", RefundState::Sending, Some("payment-hash-r"), AT + 50)
        .unwrap();
    let sent = ledger
        .set_refund("s1", RefundState::Sent, None, AT + 60)
        .unwrap();
    assert_eq!(sent.refunded_msat, 70_000);
    assert_eq!(sent.owed_msat, 0);
    conserved(&sent);
    assert!(
        ledger
            .set_refund("s1", RefundState::Planned, None, AT + 70)
            .is_err()
    );
    // A session's money never touches the prepaid compute balance.
    assert_eq!(ledger.compute_balance("s1").unwrap().credited_msat, 0);
}

#[test]
fn expiry_and_unknown_refunds_remain_liabilities() {
    let mut ledger = Ledger::in_memory().unwrap();
    ledger.open_session(&open("s1", "d1")).unwrap();
    ledger
        .debit_session("s1", &debit("r1", 100, AT + 5))
        .unwrap();
    // A debit at or after expiry is refused.
    assert!(matches!(
        ledger.debit_session("s1", &debit("late", 1, AT + 3_600)),
        Err(Error::Denied(_))
    ));
    assert_eq!(ledger.expire_sessions(AT + 3_600).unwrap(), vec!["s1"]);
    let s = ledger.session("s1").unwrap().unwrap();
    assert_eq!(s.state, SessionState::Expired);
    assert_eq!(s.owed_msat, 96_000);
    ledger
        .set_refund("s1", RefundState::Sending, Some("ref"), AT + 3_601)
        .unwrap();
    let unknown = ledger
        .set_refund("s1", RefundState::Unknown, None, AT + 3_602)
        .unwrap();
    assert_eq!(unknown.owed_msat, 96_000, "unknown stays owed");
    assert_eq!(unknown.refunded_msat, 0);
    conserved(&unknown);
    // Unknown cannot be retried blindly; only a lookup settles it.
    assert!(
        ledger
            .set_refund("s1", RefundState::Planned, None, AT + 3_603)
            .is_err()
    );
    ledger
        .set_refund("s1", RefundState::Failed, None, AT + 3_604)
        .unwrap();
    let again = ledger
        .set_refund("s1", RefundState::Planned, None, AT + 3_605)
        .unwrap();
    assert_eq!(again.owed_msat, 96_000);
    conserved(&again);
}

#[test]
fn session_authority_cannot_widen_recipients_or_effects() {
    let mut ledger = Ledger::in_memory().unwrap();
    ledger.open_session(&open("s1", "d1")).unwrap();
    let mut stranger = debit("r1", 10, AT + 1);
    stranger.recipient = "someone-else".into();
    assert!(matches!(
        ledger.debit_session("s1", &stranger),
        Err(Error::Denied(_))
    ));
    let mut wider = debit("r2", 10, AT + 1);
    wider.admission = "sha256:wider-effects".into();
    assert!(matches!(
        ledger.debit_session("s1", &wider),
        Err(Error::Denied(_))
    ));
    let mut no_return = open("s2", "d2");
    no_return.return_destination = " ".into();
    assert!(matches!(
        ledger.open_session(&no_return),
        Err(Error::Invalid(_))
    ));
    let s = ledger.session("s1").unwrap().unwrap();
    assert_eq!(s.debited_msat, 0);
    conserved(&s);
}
