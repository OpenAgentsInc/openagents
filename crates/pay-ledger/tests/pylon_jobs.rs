//! Brokered pylon jobs (P3): the v2 provider share bound to its receipt,
//! the compute balance path, forfeits for failed checks, and sweeps.

use std::sync::Mutex;

use pay_ledger::compute::{HoldRequest, Receipt as TopUpReceipt, TopUp};
use pay_ledger::payout::{Invoice, Lookup, Outcome, Policy, Rails, tick};
use pay_ledger::{Error, Ledger, OPENAGENTS, Payee, Rail, SettlementInput, Split};

/// After v2 takes effect.
const AT: i64 = 1_792_022_400;
const PROVIDER: &str = "npub-provider";

fn id(n: u32) -> String {
    format!("{n:064x}")
}

fn job(key: &str, receipt: &str, msat: i64) -> SettlementInput {
    SettlementInput {
        key: key.into(),
        resource: pay_ledger::pylon::RESOURCE.into(),
        plugin_id: None,
        release_id: None,
        price_msat: msat,
        received_msat: msat,
        rail: Rail::Lightning,
        payer_alias: None,
        settled_at: AT,
        split: Split::PylonJob {
            provider: PROVIDER.into(),
            receipt: receipt.into(),
        },
    }
}

fn share(ledger: &Ledger, key: &str, party: &str, role: &str) -> i64 {
    ledger
        .settlement(key)
        .unwrap()
        .unwrap()
        .shares
        .iter()
        .find(|s| s.party == party && s.role == role)
        .map_or(0, |s| s.amount_msat)
}

#[test]
fn the_v2_split_pays_the_provider_and_names_the_receipt() {
    let mut ledger = Ledger::in_memory().unwrap();
    // Without the rule, there is no provider share to give.
    assert!(matches!(
        ledger.record_settlement(job(&id(1), &id(101), 10_000)),
        Err(Error::Invalid(_))
    ));
    ledger.install_pylon_rule().unwrap();
    ledger.install_pylon_rule().unwrap();
    let recorded = ledger
        .record_settlement(job(&id(1), &id(101), 10_000))
        .unwrap();
    assert_eq!(recorded.rule_version, 2);
    assert_eq!(share(&ledger, &id(1), PROVIDER, "provider"), 8_500);
    assert_eq!(share(&ledger, &id(1), OPENAGENTS, "openagents"), 1_500);
    let row = ledger.pylon_job(&id(101)).unwrap().unwrap();
    assert_eq!(row.settlement, id(1));
    assert_eq!(row.provider_msat, 8_500);
    // A replay is the same row; a second payment for the receipt conflicts.
    assert_eq!(
        ledger
            .record_settlement(job(&id(1), &id(101), 10_000))
            .unwrap(),
        recorded
    );
    assert!(matches!(
        ledger.record_settlement(job(&id(2), &id(101), 10_000)),
        Err(Error::Conflict(_))
    ));
    // A malformed receipt and OpenAgents as provider are refused.
    assert!(ledger.record_settlement(job(&id(3), "nope", 10)).is_err());
    let mut own = job(&id(4), &id(104), 10);
    own.split = Split::PylonJob {
        provider: OPENAGENTS.into(),
        receipt: id(104),
    };
    assert!(ledger.record_settlement(own).is_err());
    // Plugin calls keep their v1 terms under v2.
    assert_eq!(ledger.pylon_jobs().unwrap().len(), 1);
}

#[test]
fn a_compute_balance_debit_pays_a_pylon_job() {
    let mut ledger = Ledger::in_memory().unwrap();
    ledger.install_pylon_rule().unwrap();
    ledger.create_compute_account("acct", AT).unwrap();
    let hash = pay_ledger::digest("top");
    ledger
        .open_top_up(&TopUp {
            id: "top".into(),
            account: "acct".into(),
            amount_msat: 100_000,
            payment_hash: hash.clone(),
            invoice: "lnfake-top".into(),
            created_at: AT,
            expires_at: AT + 900,
        })
        .unwrap();
    ledger
        .observe_top_up(
            &hash,
            &TopUpReceipt::Paid {
                received_msat: 100_000,
                at: AT + 1,
            },
        )
        .unwrap();
    ledger
        .reserve(&HoldRequest {
            id: "h1".into(),
            account: "acct".into(),
            quote: "q".into(),
            execution: "e".into(),
            terms: "t".into(),
            amount_msat: 20_000,
            at: AT + 2,
        })
        .unwrap();
    let (_, recorded) = ledger
        .settle_pylon_hold("h1", 12_000, AT + 3, PROVIDER, &id(201))
        .unwrap();
    let recorded = recorded.unwrap();
    assert_eq!(recorded.key, "debit:h1");
    assert_eq!(recorded.rail, Rail::Balance);
    assert_eq!(share(&ledger, "debit:h1", PROVIDER, "provider"), 10_200);
    let balance = ledger.compute_balance("acct").unwrap();
    assert_eq!(balance.available_msat, 88_000);
    assert_eq!(
        ledger.pylon_job(&id(201)).unwrap().unwrap().settlement,
        "debit:h1"
    );
}

#[derive(Default)]
struct Wallet(Mutex<Vec<(String, i64)>>);

impl Rails for Wallet {
    fn lightning_invoice(&self, address: &str, amount_msat: i64) -> Result<Invoice, String> {
        let n = self.0.lock().unwrap().len();
        Ok(Invoice {
            bolt11: format!("lnbcrt-{address}-{n}"),
            payment_hash: format!("{:064x}", 9_000 + n),
            amount_msat,
        })
    }
    fn pay_lightning(&self, invoice: &Invoice, _max_fee_msat: i64) -> Outcome {
        self.0
            .lock()
            .unwrap()
            .push((invoice.payment_hash.clone(), invoice.amount_msat));
        Outcome::Sent { fee_msat: 0 }
    }
    fn lookup_lightning(&self, _payment_hash: &str) -> Result<Lookup, String> {
        Ok(Lookup::Sent { fee_msat: 0 })
    }
    fn fund_spark(&self, _amount_sats: u64) -> Result<(), String> {
        Err("no spark in this test".into())
    }
    fn pay_spark(&self, _address: &str, _amount_sats: u64, _key: &str) -> Outcome {
        Outcome::Failed("no spark in this test".into())
    }
    fn lookup_spark(&self, _key: &str) -> Result<Lookup, String> {
        Ok(Lookup::Absent)
    }
}

#[test]
fn a_failed_check_forfeits_only_the_unpaid_share_and_sweeps_pay_the_rest() {
    let mut ledger = Ledger::in_memory().unwrap();
    ledger.install_pylon_rule().unwrap();
    ledger
        .register_payee(Payee {
            party: PROVIDER.into(),
            destination_kind: "lud16".into(),
            destination_value: "provider@regtest.example".into(),
            source: "fixture".into(),
            verified_at: AT,
        })
        .unwrap();
    // Three 600-sat jobs: 510 sats each to the provider.
    for n in 1..=3 {
        ledger
            .record_settlement(job(&id(n), &id(100 + n), 600_000))
            .unwrap();
    }
    // Job 2 fails its check before any sweep: its share is forfeited.
    let forfeit = ledger.forfeit_pylon_job(&id(102), &id(902), AT).unwrap();
    assert_eq!(forfeit.reduced_msat, 510_000);
    assert_eq!(forfeit.loss_msat, 0);
    assert_eq!(
        ledger.forfeit_pylon_job(&id(102), &id(902), AT).unwrap(),
        forfeit
    );
    // A second check on the same receipt forfeits nothing more.
    assert_eq!(
        ledger.forfeit_pylon_job(&id(102), &id(903), AT).unwrap().id,
        forfeit.id
    );
    assert_eq!(ledger.accrued(PROVIDER).unwrap(), 1_020_000);
    // Under the sweep threshold and not ten minutes old: nothing goes out.
    let wallet = Wallet::default();
    let policy = Policy::pylon_sweeps();
    let mut n = 0;
    let mut next = || {
        n += 1;
        format!("payout-{n}")
    };
    let mut resolve = |l: &mut Ledger, party: &str, _at: i64| l.payee(party);
    // 1,020 sats owed is over the 1,000-sat threshold: one sweep pays both.
    tick(
        &mut ledger,
        &wallet,
        &policy,
        AT + 1,
        &mut resolve,
        &mut next,
    )
    .unwrap();
    assert_eq!(
        wallet.0.lock().unwrap().as_slice(),
        [(format!("{:064x}", 9_000), 1_020_000)]
    );
    assert_eq!(ledger.accrued(PROVIDER).unwrap(), 0);
    // A check that fails after the sweep cannot claw back: a recorded loss.
    let late = ledger
        .forfeit_pylon_job(&id(101), &id(904), AT + 2)
        .unwrap();
    assert_eq!((late.reduced_msat, late.loss_msat), (0, 510_000));
    // A late forfeit leaves nothing unbacked, so payouts keep going.
    assert!(!ledger.commission_payouts_held().unwrap());
    let rows = ledger.pylon_jobs().unwrap();
    assert_eq!(
        rows.iter()
            .map(|r| (r.forfeited_msat, r.paid_msat))
            .collect::<Vec<_>>(),
        [(0, 510_000), (510_000, 0), (0, 510_000)]
    );
    // Forfeiting an unknown receipt or a non-pylon share is refused.
    assert!(ledger.forfeit_pylon_job(&id(999), &id(905), AT).is_err());
    assert!(
        ledger
            .reduce_payable("x", &id(1), OPENAGENTS, "provider", "e", 1, AT)
            .is_err()
    );
}

#[test]
fn a_small_balance_sweeps_after_ten_minutes() {
    let mut ledger = Ledger::in_memory().unwrap();
    ledger.install_pylon_rule().unwrap();
    ledger
        .register_payee(Payee {
            party: PROVIDER.into(),
            destination_kind: "lud16".into(),
            destination_value: "provider@regtest.example".into(),
            source: "fixture".into(),
            verified_at: AT,
        })
        .unwrap();
    ledger
        .record_settlement(job(&id(1), &id(101), 20_000))
        .unwrap();
    let wallet = Wallet::default();
    let policy = Policy::pylon_sweeps();
    let mut next = || "p".to_string();
    let mut resolve = |l: &mut Ledger, party: &str, _at: i64| l.payee(party);
    tick(
        &mut ledger,
        &wallet,
        &policy,
        AT + 599,
        &mut resolve,
        &mut next,
    )
    .unwrap();
    assert!(wallet.0.lock().unwrap().is_empty());
    tick(
        &mut ledger,
        &wallet,
        &policy,
        AT + 600,
        &mut resolve,
        &mut next,
    )
    .unwrap();
    assert_eq!(
        wallet.0.lock().unwrap().as_slice(),
        [(format!("{:064x}", 9_000), 17_000)]
    );
}
