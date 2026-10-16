use pay_ledger::{
    Ledger,
    assets::{Quote, Rate, Receipt},
};
fn quote() -> Quote {
    Quote {
        node: format!("02{}", "11".repeat(32)),
        network: "regtest".into(),
        account: "test".into(),
        asset_id: "22".repeat(32),
        edge: format!("03{}", "33".repeat(32)),
        rfq_id: "44".repeat(32),
        payment_hash: "55".repeat(32),
        invoice: "regtest-invoice".into(),
        units: 1000,
        rate: Rate {
            coefficient: "100000000".into(),
            scale: 0,
        },
        credit_msat: 1_000_000,
        created_at: 100,
        expires_at: 200,
    }
}
#[test]
fn asset_credit_survives_restart_and_cannot_be_repriced_or_replayed() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("ledger.db");
    let q = quote();
    let r = Receipt {
        settle_index: 1,
        accepted_at: 150,
        settled_at: 250,
        units: 1000,
        evidence: "authenticated settled HTLCs".into(),
    };
    {
        let mut l = Ledger::open(&path).unwrap();
        l.create_compute_account("test", 1).unwrap();
        l.open_asset_quote(&q).unwrap();
        let mut short = r.clone();
        short.units = 999;
        assert!(l.credit_asset(&q.node, &q.payment_hash, &short).is_err());
        assert_eq!(l.credited("test").unwrap(), 0);
        l.credit_asset(&q.node, &q.payment_hash, &r).unwrap();
    }
    let mut l = Ledger::open(&path).unwrap();
    l.credit_asset(&q.node, &q.payment_hash, &r).unwrap();
    assert_eq!(l.credited("test").unwrap(), 1_000_000);
    assert_eq!(
        l.asset_receipts().unwrap(),
        vec![(q.clone(), r.clone(), 1_000_000)]
    );
    let mut other = q.clone();
    other.rate.coefficient = "200000000".into();
    other.credit_msat = 500000;
    assert!(l.open_asset_quote(&other).is_err());
    other = q.clone();
    other.account = "other".into();
    assert!(l.open_asset_quote(&other).is_err());
    assert!(l.credit_asset("other", &q.payment_hash, &r).is_err());
    let topup = pay_ledger::compute::purchase::TopUp {
        id: "double".into(),
        account: q.account.clone(),
        amount_msat: 1_000_000,
        payment_hash: q.payment_hash.clone(),
        invoice: q.invoice.clone(),
        created_at: 100,
        expires_at: 200,
    };
    assert!(l.open_top_up(&topup).is_err());
}
#[test]
fn exact_rate_arithmetic_rejects_overflow_and_never_rounds_up() {
    assert_eq!(
        Rate {
            coefficient: "3".into(),
            scale: 0
        }
        .value_msat(1)
        .unwrap(),
        33_333_333_333
    );
    assert_eq!(
        Rate {
            coefficient: "30".into(),
            scale: 1
        }
        .value_msat(1)
        .unwrap(),
        33_333_333_333
    );
    assert!(
        Rate {
            coefficient: "0".into(),
            scale: 0
        }
        .value_msat(1)
        .is_err()
    );
    assert!(
        Rate {
            coefficient: "1".into(),
            scale: 18
        }
        .value_msat(u64::MAX)
        .is_err()
    );
}
