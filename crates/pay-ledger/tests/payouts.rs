//! The payout worker against a fake wallet: batching, both rails, the
//! restart-safe state machine, retries with backoff, and payees without a
//! destination.

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::Mutex;

use pay_ledger::payout::{Invoice, Lookup, Outcome, Policy, Rails, Step, tick};
use pay_ledger::{Ledger, OPENAGENTS, Payee, PayoutState, Rail, SettlementInput, Split};

const START: i64 = 1_792_022_400;
const SPARK: &str = "spark1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq";
const LUD16: &str = "alice@example.com";

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Fault {
    #[default]
    None,
    /// Panic after the payment went out, before the outcome is recorded.
    CrashAfterPay,
    /// Panic after the reference was journaled, before anything is sent.
    CrashBeforePay,
    /// Panic while resolving the address, before the reference exists.
    CrashResolving,
    /// The send is refused; nothing goes out.
    Refuse,
    /// The send goes out but the wallet cannot tell yet.
    Hang,
    /// The address returns an invoice for another amount.
    WrongAmount,
}

#[derive(Default)]
struct State {
    fault: Fault,
    /// Every payment that actually went out: (destination, msat, reference).
    paid: Vec<(String, i64, String)>,
    records: HashMap<String, Lookup>,
    invoices: u32,
    funded_sats: u64,
}

#[derive(Default)]
struct FakeWallet(Mutex<State>);

impl FakeWallet {
    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    fn fault(&self, fault: Fault) {
        self.state().fault = fault;
    }
    fn paid(&self) -> usize {
        self.state().paid.len()
    }
    fn send(&self, destination: &str, msat: i64, reference: &str) -> Outcome {
        let fault = self.state().fault;
        match fault {
            Fault::CrashBeforePay => panic!("crash before pay"),
            Fault::Refuse => {
                self.state()
                    .records
                    .insert(reference.into(), Lookup::Failed("refused".into()));
                return Outcome::Failed("refused".into());
            }
            _ => {}
        }
        {
            let mut state = self.state();
            state
                .paid
                .push((destination.into(), msat, reference.into()));
            let record = if fault == Fault::Hang {
                Lookup::Pending
            } else {
                Lookup::Sent { fee_msat: 7 }
            };
            state.records.insert(reference.into(), record);
        }
        match fault {
            Fault::CrashAfterPay => panic!("crash after pay"),
            Fault::Hang => Outcome::Unknown("still pending after 60s".into()),
            _ => Outcome::Sent { fee_msat: 7 },
        }
    }
    fn lookup(&self, reference: &str) -> Result<Lookup, String> {
        Ok(self
            .state()
            .records
            .get(reference)
            .cloned()
            .unwrap_or(Lookup::Absent))
    }
}

impl Rails for FakeWallet {
    fn lightning_invoice(&self, address: &str, amount_msat: i64) -> Result<Invoice, String> {
        let mut state = self.state();
        if state.fault == Fault::CrashResolving {
            drop(state);
            panic!("crash while resolving");
        }
        state.invoices += 1;
        let amount = if state.fault == Fault::WrongAmount {
            amount_msat + 1000
        } else {
            amount_msat
        };
        Ok(Invoice {
            bolt11: format!("lnbc-{address}-{}", state.invoices),
            payment_hash: format!("{:064x}", state.invoices),
            amount_msat: amount,
        })
    }
    fn pay_lightning(&self, invoice: &Invoice, max_fee_msat: i64) -> Outcome {
        assert!(max_fee_msat >= 5_000);
        self.send(&invoice.bolt11, invoice.amount_msat, &invoice.payment_hash)
    }
    fn lookup_lightning(&self, payment_hash: &str) -> Result<Lookup, String> {
        self.lookup(payment_hash)
    }
    fn fund_spark(&self, amount_sats: u64) -> Result<(), String> {
        self.state().funded_sats += amount_sats;
        Ok(())
    }
    fn pay_spark(&self, address: &str, amount_sats: u64, key: &str) -> Outcome {
        self.send(address, i64::try_from(amount_sats).unwrap() * 1000, key)
    }
    fn lookup_spark(&self, key: &str) -> Result<Lookup, String> {
        self.lookup(key)
    }
}

fn settle(ledger: &mut Ledger, key: &str, author: &str, fee_msat: i64, at: i64) {
    ledger
        .record_settlement(SettlementInput {
            key: key.into(),
            resource: "/v1/plugins/demo/invoke".into(),
            plugin_id: Some(format!("plugin-{author}")),
            release_id: None,
            price_msat: fee_msat + 5_000,
            received_msat: fee_msat + 5_000,
            rail: Rail::Lightning,
            payer_alias: None,
            settled_at: at,
            split: Split::Plugin {
                author: author.into(),
                fee_msat,
            },
        })
        .unwrap();
}

fn payee(ledger: &mut Ledger, party: &str, kind: &str, value: &str) {
    ledger
        .register_payee(Payee {
            party: party.into(),
            destination_kind: kind.into(),
            destination_value: value.into(),
            source: "release".into(),
            verified_at: START,
        })
        .unwrap();
}

struct Run {
    next: u32,
}

impl Run {
    fn new() -> Self {
        Self { next: 0 }
    }
    fn tick(&mut self, ledger: &mut Ledger, wallet: &FakeWallet, now: i64) -> Vec<Step> {
        let mut resolve = |l: &mut Ledger, party: &str, _: i64| l.payee(party);
        let next = &mut self.next;
        let mut new_id = || {
            *next += 1;
            format!("00000000-0000-4000-8000-{:012x}", *next)
        };
        tick(
            ledger,
            wallet,
            &Policy::default(),
            now,
            &mut resolve,
            &mut new_id,
        )
        .unwrap()
    }
    /// A tick that crashes; the process "dies" and the ledger is reopened.
    fn crash(&mut self, path: &Path, wallet: &FakeWallet, now: i64) -> Ledger {
        let mut ledger = Ledger::open(path).unwrap();
        let crashed = catch_unwind(AssertUnwindSafe(|| self.tick(&mut ledger, wallet, now)));
        assert!(crashed.is_err(), "the injected crash did not happen");
        drop(ledger);
        Ledger::open(path).unwrap()
    }
}

fn states(ledger: &Ledger) -> Vec<PayoutState> {
    ledger
        .payouts(None)
        .unwrap()
        .into_iter()
        .map(|p| p.state)
        .collect()
}

/// A ledger where "alice" is owed at least the Lightning threshold.
fn owed(path: &Path, kind: &str, value: &str) -> (Ledger, i64) {
    let mut ledger = Ledger::open(path).unwrap();
    settle(&mut ledger, "h1", "alice", 600_000, START);
    settle(&mut ledger, "h2", "alice", 600_000, START + 1);
    payee(&mut ledger, "alice", kind, value);
    let owed = ledger.accrued("alice").unwrap();
    assert!(owed >= 1_000_000);
    (ledger, owed)
}

#[test]
fn a_payee_is_paid_once_in_one_batch_on_each_rail() {
    for (kind, value) in [("spark", SPARK), ("lud16", LUD16)] {
        let dir = tempfile::tempdir().unwrap();
        let (mut ledger, owed) = owed(&dir.path().join("l.sqlite"), kind, value);
        let wallet = FakeWallet::default();
        let mut run = Run::new();
        run.tick(&mut ledger, &wallet, START + 10);
        assert_eq!(wallet.paid(), 1, "{kind}");
        let paid = wallet.state().paid[0].clone();
        assert_eq!(paid.1, owed / 1000 * 1000);
        let payout = &ledger.payouts(None).unwrap()[0];
        assert_eq!(payout.state, PayoutState::Sent);
        assert_eq!(payout.amount_msat, owed);
        assert_eq!(payout.sent_msat, Some(owed / 1000 * 1000));
        assert_eq!(payout.fee_msat, Some(7));
        assert_eq!(
            payout.rail,
            if kind == "spark" {
                "spark"
            } else {
                "lightning"
            }
        );
        assert_eq!(payout.wallet_reference.as_deref(), Some(paid.2.as_str()));
        if kind == "spark" {
            assert_eq!(payout.wallet_reference.as_deref(), Some(payout.id.as_str()));
            assert_eq!(
                wallet.state().funded_sats,
                u64::try_from(owed / 1000).unwrap()
            );
        }
        assert_eq!(ledger.accrued("alice").unwrap(), 0);
        assert_eq!(ledger.totals().unwrap().paid_msat, owed);
        run.tick(&mut ledger, &wallet, START + 20);
        assert_eq!(wallet.paid(), 1);
        // OpenAgents' own share is never paid out.
        assert!(
            ledger
                .payouts(None)
                .unwrap()
                .iter()
                .all(|p| p.party != OPENAGENTS)
        );
    }
}

#[test]
fn a_crash_after_the_payment_resolves_by_lookup_without_paying_again() {
    for (kind, value) in [("spark", SPARK), ("lud16", LUD16)] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("l.sqlite");
        let (ledger, owed) = owed(&path, kind, value);
        drop(ledger);
        let wallet = FakeWallet::default();
        let mut run = Run::new();
        wallet.fault(Fault::CrashAfterPay);
        let mut ledger = run.crash(&path, &wallet, START + 10);
        assert_eq!(wallet.paid(), 1);
        assert_eq!(states(&ledger), vec![PayoutState::Sending], "{kind}");
        assert_eq!(ledger.accrued("alice").unwrap(), 0);
        wallet.fault(Fault::None);
        run.tick(&mut ledger, &wallet, START + 20);
        run.tick(&mut ledger, &wallet, START + 30 + 86_400);
        assert_eq!(wallet.paid(), 1, "{kind} was paid twice");
        assert_eq!(states(&ledger), vec![PayoutState::Sent]);
        assert_eq!(ledger.totals().unwrap().paid_msat, owed);
    }
}

#[test]
fn a_crash_before_the_payment_stays_unknown_and_is_never_resent() {
    for (kind, value) in [("spark", SPARK), ("lud16", LUD16)] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("l.sqlite");
        let (ledger, _) = owed(&path, kind, value);
        drop(ledger);
        let wallet = FakeWallet::default();
        let mut run = Run::new();
        wallet.fault(Fault::CrashBeforePay);
        let mut ledger = run.crash(&path, &wallet, START + 10);
        wallet.fault(Fault::None);
        for at in [20, 100_000, 1_000_000] {
            run.tick(&mut ledger, &wallet, START + at);
        }
        // The wallet has no record, which after a crash does not prove
        // nothing went out: the shares stay reserved, nothing is resent.
        assert_eq!(wallet.paid(), 0);
        assert_eq!(states(&ledger), vec![PayoutState::Unknown], "{kind}");
        assert_eq!(ledger.accrued("alice").unwrap(), 0);
        assert!(ledger.totals().unwrap().reserved_msat > 0);
    }
}

#[test]
fn a_crash_before_the_reference_is_written_fails_the_plan_and_retries() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("l.sqlite");
    let (ledger, owed) = owed(&path, "lud16", LUD16);
    drop(ledger);
    let wallet = FakeWallet::default();
    let mut run = Run::new();
    wallet.fault(Fault::CrashResolving);
    let mut ledger = run.crash(&path, &wallet, START + 10);
    assert_eq!(states(&ledger), vec![PayoutState::Planned]);
    wallet.fault(Fault::None);
    let steps = run.tick(&mut ledger, &wallet, START + 20);
    assert_eq!(states(&ledger), vec![PayoutState::Failed]);
    assert_eq!(ledger.accrued("alice").unwrap(), owed);
    assert!(steps.iter().any(|s| matches!(s, Step::BackingOff { .. })));
    run.tick(&mut ledger, &wallet, START + 20 + 300);
    assert_eq!(
        states(&ledger),
        vec![PayoutState::Failed, PayoutState::Sent]
    );
    assert_eq!(wallet.paid(), 1);
    assert_eq!(ledger.payouts(None).unwrap()[1].attempts, 2);
}

#[test]
fn a_failed_payout_returns_its_shares_and_retries_with_backoff() {
    let dir = tempfile::tempdir().unwrap();
    let (mut ledger, owed) = owed(&dir.path().join("l.sqlite"), "spark", SPARK);
    let wallet = FakeWallet::default();
    let mut run = Run::new();
    wallet.fault(Fault::Refuse);
    run.tick(&mut ledger, &wallet, START);
    assert_eq!(states(&ledger), vec![PayoutState::Failed]);
    assert_eq!(ledger.accrued("alice").unwrap(), owed);
    assert_eq!(
        ledger.payouts(None).unwrap()[0].error.as_deref(),
        Some("refused")
    );
    // Twice more: the waits are 300 s, then 600 s.
    run.tick(&mut ledger, &wallet, START + 299);
    assert_eq!(ledger.payouts(None).unwrap().len(), 1);
    run.tick(&mut ledger, &wallet, START + 300);
    assert_eq!(ledger.payouts(None).unwrap().len(), 2);
    run.tick(&mut ledger, &wallet, START + 300 + 599);
    assert_eq!(ledger.payouts(None).unwrap().len(), 2);
    wallet.fault(Fault::None);
    run.tick(&mut ledger, &wallet, START + 300 + 600);
    assert_eq!(
        states(&ledger),
        vec![PayoutState::Failed, PayoutState::Failed, PayoutState::Sent]
    );
    assert_eq!(ledger.payouts(None).unwrap()[2].attempts, 3);
    assert_eq!(wallet.paid(), 1);
    assert_eq!(ledger.accrued("alice").unwrap(), 0);
    // A success ends the streak.
    assert_eq!(ledger.failure_streak("alice").unwrap(), (0, None));
}

#[test]
fn an_unknown_send_is_settled_by_a_later_lookup() {
    let dir = tempfile::tempdir().unwrap();
    let (mut ledger, _) = owed(&dir.path().join("l.sqlite"), "lud16", LUD16);
    let wallet = FakeWallet::default();
    let mut run = Run::new();
    wallet.fault(Fault::Hang);
    run.tick(&mut ledger, &wallet, START);
    assert_eq!(states(&ledger), vec![PayoutState::Unknown]);
    wallet.fault(Fault::None);
    run.tick(&mut ledger, &wallet, START + 60);
    assert_eq!(states(&ledger), vec![PayoutState::Unknown]);
    let reference = ledger.payouts(None).unwrap()[0]
        .wallet_reference
        .clone()
        .unwrap();
    wallet
        .state()
        .records
        .insert(reference, Lookup::Sent { fee_msat: 3 });
    run.tick(&mut ledger, &wallet, START + 120);
    assert_eq!(states(&ledger), vec![PayoutState::Sent]);
    assert_eq!(ledger.payouts(None).unwrap()[0].fee_msat, Some(3));
    assert_eq!(wallet.paid(), 1);
}

#[test]
fn a_payee_without_a_destination_stays_owed() {
    let mut ledger = Ledger::in_memory().unwrap();
    settle(&mut ledger, "h1", "bob", 2_000_000, START);
    let owed = ledger.accrued("bob").unwrap();
    let wallet = FakeWallet::default();
    let steps = Run::new().tick(&mut ledger, &wallet, START + 2 * 86_400);
    assert!(steps.contains(&Step::NoDestination {
        party: "bob".into(),
        owed_msat: owed
    }));
    assert!(ledger.payouts(None).unwrap().is_empty());
    assert_eq!(ledger.accrued("bob").unwrap(), owed);
    // A node key has no rail yet: owed, too.
    payee(
        &mut ledger,
        "bob",
        "node",
        &format!("02{}", "ab".repeat(32)),
    );
    let steps = Run::new().tick(&mut ledger, &wallet, START + 2 * 86_400);
    assert!(steps.iter().any(|s| matches!(s, Step::Unpayable { .. })));
    assert_eq!(ledger.accrued("bob").unwrap(), owed);
    assert_eq!(wallet.paid(), 0);
}

#[test]
fn small_amounts_wait_for_the_threshold_or_a_day() {
    let mut ledger = Ledger::in_memory().unwrap();
    // 5 sats owed on each rail, plus the launch bonus.
    settle(&mut ledger, "h1", "carol", 2_000, START);
    settle(&mut ledger, "h2", "dave", 2_000, START);
    payee(&mut ledger, "carol", "spark", SPARK);
    payee(&mut ledger, "dave", "lud16", LUD16);
    let wallet = FakeWallet::default();
    let mut run = Run::new();
    run.tick(&mut ledger, &wallet, START + 86_399);
    assert_eq!(wallet.paid(), 0);
    run.tick(&mut ledger, &wallet, START + 86_400);
    assert_eq!(wallet.paid(), 2);
    // Spark reaches its threshold (100 sats) at once.
    settle(&mut ledger, "h3", "carol", 100_000, START + 90_000);
    run.tick(&mut ledger, &wallet, START + 90_001);
    assert_eq!(wallet.paid(), 3);
    // Lightning waits for 1,000 sats.
    settle(&mut ledger, "h4", "dave", 300_000, START + 90_000);
    run.tick(&mut ledger, &wallet, START + 90_001);
    assert_eq!(wallet.paid(), 3);
}

#[test]
fn an_invoice_for_the_wrong_amount_is_never_paid() {
    let dir = tempfile::tempdir().unwrap();
    let (mut ledger, owed) = owed(&dir.path().join("l.sqlite"), "lud16", LUD16);
    let wallet = FakeWallet::default();
    wallet.fault(Fault::WrongAmount);
    Run::new().tick(&mut ledger, &wallet, START);
    assert_eq!(wallet.paid(), 0);
    assert_eq!(states(&ledger), vec![PayoutState::Failed]);
    assert_eq!(ledger.accrued("alice").unwrap(), owed);
}

#[test]
fn payout_items_link_each_payout_to_the_shares_it_drained() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("l.sqlite");
    let (mut ledger, _) = owed(&path, "spark", SPARK);
    let shares = ledger.available_shares("alice").unwrap();
    Run::new().tick(&mut ledger, &FakeWallet::default(), START + 5);
    let id = ledger.payouts(None).unwrap()[0].id.clone();
    drop(ledger);
    let c = rusqlite::Connection::open(&path).unwrap();
    let mut linked: Vec<(String, String)> = c
        .prepare("SELECT settlement,role FROM payout_item WHERE payout=? UNION ALL SELECT settlement,role FROM bonus_payout_item WHERE payout=? ORDER BY 1,2")
        .unwrap()
        .query_map([&id, &id], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let mut expected: Vec<_> = shares.into_iter().map(|s| (s.settlement, s.role)).collect();
    expected.sort();
    linked.sort();
    assert_eq!(linked, expected);
}

#[test]
fn an_old_ledger_moves_to_the_payout_states() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("l.sqlite");
    {
        let c = rusqlite::Connection::open(&path).unwrap();
        c.execute_batch(
            "PRAGMA foreign_keys=OFF;
            CREATE TABLE payout (
                id TEXT PRIMARY KEY,
                party TEXT NOT NULL REFERENCES payee(party),
                amount_msat INTEGER NOT NULL CHECK(amount_msat > 0),
                destination TEXT NOT NULL,
                rail TEXT NOT NULL,
                state TEXT NOT NULL CHECK(state IN ('pending','unknown','succeeded','failed')),
                wallet_reference TEXT,
                attempts INTEGER NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            INSERT INTO payout VALUES('a','p',1,'spark:x','spark','pending',NULL,1,1,1);
            INSERT INTO payout VALUES('b','p',1,'spark:x','spark','pending','ref',1,1,1);
            INSERT INTO payout VALUES('c','p',1,'spark:x','spark','succeeded','ref2',1,1,1);
            INSERT INTO payout VALUES('d','p',1,'spark:x','spark','failed',NULL,1,1,1);",
        )
        .unwrap();
    }
    let ledger = Ledger::open(&path).unwrap();
    assert_eq!(
        states(&ledger),
        vec![
            PayoutState::Planned,
            PayoutState::Unknown,
            PayoutState::Sent,
            PayoutState::Failed
        ]
    );
    drop(ledger);
    // Opening again leaves it alone, and foreign keys still hold.
    let ledger = Ledger::open(&path).unwrap();
    assert_eq!(ledger.payouts(None).unwrap().len(), 4);
    let c = rusqlite::Connection::open(&path).unwrap();
    let sql: String = c
        .query_row(
            "SELECT sql FROM sqlite_master WHERE name='payout_item'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(sql.contains("REFERENCES payout(id)"));
}
