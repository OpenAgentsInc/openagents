use super::*;
use nostr::x402::test_invoice::{number, payee_of, signed_by, tag, words};
use openagents_wallet::{Balance, Channel, PaymentRecord, Proof, WalletError};
use sha2::{Digest, Sha256};
use std::{
    cell::{Cell, RefCell},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
const NODE: [u8; 32] = [9; 32];
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
struct Wallet {
    now: u64,
    calls: Cell<usize>,
    payments: RefCell<Option<PaymentRecord>>,
    lost: bool,
    replace_lock: RefCell<Option<PathBuf>>,
}
impl Wallet {
    fn new(now: u64) -> Self {
        Self {
            now,
            calls: Cell::new(0),
            payments: RefCell::new(None),
            lost: false,
            replace_lock: RefCell::new(None),
        }
    }
}
impl LightningWallet for Wallet {
    fn node_id(&self) -> String {
        hex(&payee_of(NODE))
    }
    fn receive_exact(
        &self,
        amount: u64,
        hash: [u8; 32],
        expiry: u32,
    ) -> Result<IssuedInvoice, WalletError> {
        self.calls.set(self.calls.get() + 1);
        if self.lost {
            return Err(WalletError::Node("private provider refusal".into()));
        }
        let preimage: [u8; 32] = Sha256::digest(hash).into();
        let payment: [u8; 32] = Sha256::digest(preimage).into();
        let mut fields = tag(1, &words(&payment));
        fields.extend(tag(16, &words(&[2; 32])));
        fields.extend(tag(23, &words(&hash)));
        fields.extend(tag(6, &number(u64::from(expiry))));
        let invoice = signed_by(
            NODE,
            &format!("lnbc{}p", amount * 10),
            fields,
            false,
            false,
            self.now,
        );
        *self.payments.borrow_mut() = Some(PaymentRecord {
            payment_hash: hex(&payment),
            direction: PaymentDirection::Inbound,
            status: PaymentStatus::Pending,
            amount_msat: Some(amount),
            fee_msat: None,
            preimage: Some(hex(&preimage)),
            bolt11: Some(invoice.clone()),
            updated_at: self.now,
        });
        Ok(IssuedInvoice {
            bolt11: invoice,
            payment_hash: hex(&payment),
            amount_msat: amount,
            description_hash: hex(&hash),
            expiry_secs: expiry,
            pay_to: self.node_id(),
        })
    }
    fn lookup(&self, _: [u8; 32]) -> Result<Option<PaymentRecord>, WalletError> {
        if let Some(path) = self.replace_lock.borrow_mut().take() {
            std::fs::rename(&path, path.with_extension("old")).unwrap();
            std::fs::write(&path, b"new-lock").unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        Ok(self.payments.borrow().clone())
    }
    fn pay(&self, _: &str, _: u64, _: Duration) -> Result<Proof, WalletError> {
        panic!("Funding receiver never pays.")
    }
    fn balance(&self) -> Result<Balance, WalletError> {
        unimplemented!()
    }
    fn channels(&self) -> Result<Vec<Channel>, WalletError> {
        unimplemented!()
    }
    fn funding_address(&self) -> Result<String, WalletError> {
        unimplemented!()
    }
    fn open_channel(&self, _: &str, _: &str, _: u64, _: bool) -> Result<String, WalletError> {
        unimplemented!()
    }
    fn close_channel(&self, _: &str, _: &str, _: bool) -> Result<(), WalletError> {
        unimplemented!()
    }
}
fn context() -> Context {
    serde_json::from_value(serde_json::json!({"schema":receipts::purchase::SCHEMA,"account":"customer","workspace":"workspace","payer_workspace":"workspace","tenant":"tenant","credential_reference":"key","membership_epoch":1,"workspace_members_epoch":1,"role":"owner","door":"decision","registry_digest":format!("sha256:{}","a".repeat(64)),"artifact_digest":format!("sha256:{}","b".repeat(64)),"price":{"version":"price","currency":"BTC","policy":"observed-usage-v1","terms_digest":format!("sha256:{}","c".repeat(64)),"maximum_usage_digest":format!("sha256:{}","d".repeat(64)),"maximum_charge":1},"can_invoke":true})).unwrap()
}
fn fixture() -> (tempfile::TempDir, Config, Ledger, Store, Wallet) {
    let root = tempfile::tempdir().unwrap();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let policy:Policy=serde_json::from_value(serde_json::json!({"schema":funding::POLICY_SCHEMA,"version":"btc-v1","unit":{"kind":"currency-millionths","currency":"BTC"},"conversions":[{"version":"msat-v1","source":{"kind":"millisatoshis"},"target":{"kind":"currency-millionths","currency":"BTC"},"numerator":1,"denominator":100000,"source_ref":"fixture:exact-same-currency","valid_from":0,"valid_until":u64::MAX,"rounding":"exact","fee_payer":"customer","max_fee_units":0}],"purchases":{"required_finality":"final","refunds_allowed":false,"disputes_allowed":false,"spent_credit_loss":"operator"},"promotions":{"total_cap":0,"grant_cap":0,"max_lifetime_seconds":0,"max_admissions":0,"price_policies":[],"reversible":false}})).unwrap();
    let config = Config {
        state: root.path().join("funding"),
        wallet_home: root.path().join("wallet"),
        receiver_node: hex(&payee_of(NODE)),
        network: "bitcoin".into(),
        policy: policy.clone(),
        conversion: "msat-v1".into(),
        maximum_msat: 1_000_000,
    };
    config.check().unwrap();
    let mut ledger = Ledger::open(&root.path().join("money.jsonl")).unwrap();
    for (source, operation) in [
        (
            "create",
            Operation::Create {
                currency: "BTC".into(),
                spend_limit: 100,
                topups_allowed: true,
            },
        ),
        ("policy", Operation::FundingPolicy { policy }),
    ] {
        ledger
            .apply(Mutation {
                workspace: "workspace".into(),
                source: source.into(),
                audit: "fixture".into(),
                operation,
            })
            .unwrap();
    }
    let store = Store::open(&config.state).unwrap();
    (root, config, ledger, store, Wallet::new(now))
}
#[test]
fn short_confirmed_collection_cannot_credit_the_invoice_amount() {
    let (_root, cfg, mut ledger, mut store, wallet) = fixture();
    let context = context();
    let quote = store
        .quote(&cfg, &ledger, &context, "short", 200_000, wallet.now)
        .unwrap();
    store
        .issue(
            &mut ledger,
            &wallet,
            &context,
            "short",
            &quote.quote.digest(),
            wallet.now,
        )
        .unwrap();
    let mut observation = wallet.payments.borrow_mut();
    let payment = observation.as_mut().unwrap();
    payment.status = PaymentStatus::Succeeded;
    payment.amount_msat = Some(100_000);
    payment.bolt11 = None;
    payment.fee_msat = None;
    drop(observation);
    assert!(
        store
            .reconcile(&mut ledger, &wallet, &context, "short", wallet.now)
            .is_err()
    );
    assert_eq!(ledger.balance("workspace").unwrap().credited, 0);
    assert_eq!(store.read("short", &context).unwrap().phase, Phase::Invoice);
}

#[test]
fn confirmed_collection_credits_original_workspace_once_after_restart() {
    let (_root, cfg, mut ledger, mut store, wallet) = fixture();
    let context = context();
    let quote = store
        .quote(&cfg, &ledger, &context, "one", 200_000, wallet.now)
        .unwrap();
    assert_eq!(quote.quote.credit_millionths_btc, 2);
    let invoice = store
        .issue(
            &mut ledger,
            &wallet,
            &context,
            "one",
            &quote.quote.digest(),
            wallet.now,
        )
        .unwrap();
    assert_eq!(invoice.phase, Phase::Invoice);
    assert_eq!(
        store
            .issue(
                &mut ledger,
                &wallet,
                &context,
                "one",
                &quote.quote.digest(),
                wallet.now
            )
            .unwrap(),
        invoice
    );
    assert_eq!(wallet.calls.get(), 1);
    assert_eq!(ledger.balance("workspace").unwrap().credited, 0);
    assert_eq!(
        store
            .reconcile(&mut ledger, &wallet, &context, "one", wallet.now)
            .unwrap()
            .phase,
        Phase::Invoice
    );
    wallet.payments.borrow_mut().as_mut().unwrap().status = PaymentStatus::Succeeded;
    wallet.payments.borrow_mut().as_mut().unwrap().bolt11 = None;
    let funded = store
        .reconcile(&mut ledger, &wallet, &context, "one", wallet.now)
        .unwrap();
    assert_eq!(funded.phase, Phase::Funded);
    assert!(funded.observation.unwrap().preimage.is_none());
    assert_eq!(ledger.balance("workspace").unwrap().credited, 2);
    drop(store);
    let mut store = Store::open(&cfg.state).unwrap();
    store
        .reconcile(&mut ledger, &wallet, &context, "one", wallet.now)
        .unwrap();
    assert_eq!(ledger.balance("workspace").unwrap().credited, 2);
    assert_eq!(wallet.calls.get(), 1);
}
#[test]
fn changed_customer_rights_terms_or_fractional_amount_never_issue() {
    let (_root, cfg, mut ledger, mut store, wallet) = fixture();
    let context = context();
    assert!(
        store
            .quote(&cfg, &ledger, &context, "dust", 999, wallet.now)
            .is_err()
    );
    let quote = store
        .quote(&cfg, &ledger, &context, "one", 200_000, wallet.now)
        .unwrap();
    for kind in ["account", "workspace", "key", "rights", "price"] {
        let mut changed = context.clone();
        match kind {
            "account" => changed.account = "other".into(),
            "workspace" => {
                changed.workspace = "other".into();
                changed.payer_workspace = "other".into();
            }
            "key" => changed.credential_reference = "new-key".into(),
            "rights" => changed.can_invoke = false,
            _ => changed.price.version = "new-price".into(),
        }
        assert!(
            store
                .issue(
                    &mut ledger,
                    &wallet,
                    &changed,
                    "one",
                    &quote.quote.digest(),
                    wallet.now
                )
                .is_err()
        );
    }
    assert!(
        store
            .issue(&mut ledger, &wallet, &context, "one", "wrong", wallet.now)
            .is_err()
    );
    assert_eq!(wallet.calls.get(), 0);
    let mut newer = cfg.policy.clone();
    newer.version = "aaa-new-active".into();
    ledger
        .apply(Mutation {
            workspace: "workspace".into(),
            source: "new-policy".into(),
            audit: "fixture".into(),
            operation: Operation::FundingPolicy {
                policy: newer.clone(),
            },
        })
        .unwrap();
    assert_eq!(ledger.funding_policy("workspace"), Some(&newer));
    assert!(
        store
            .issue(
                &mut ledger,
                &wallet,
                &context,
                "one",
                &quote.quote.digest(),
                wallet.now
            )
            .is_err()
    );
    assert_eq!(wallet.calls.get(), 0);
}
#[test]
fn lost_issuance_is_unknown_and_never_reissued_after_restart() {
    let (_root, cfg, mut ledger, mut store, mut wallet) = fixture();
    let context = context();
    wallet.lost = true;
    let quote = store
        .quote(&cfg, &ledger, &context, "one", 200_000, wallet.now)
        .unwrap();
    let error = store
        .issue(
            &mut ledger,
            &wallet,
            &context,
            "one",
            &quote.quote.digest(),
            wallet.now,
        )
        .unwrap_err();
    assert!(!error.contains("private provider"));
    drop(store);
    let mut store = Store::open(&cfg.state).unwrap();
    assert_eq!(
        store
            .issue(
                &mut ledger,
                &wallet,
                &context,
                "one",
                &quote.quote.digest(),
                wallet.now
            )
            .unwrap()
            .phase,
        Phase::Unknown
    );
    assert_eq!(wallet.calls.get(), 1);
    assert_eq!(ledger.balance("workspace").unwrap().credited, 0);
}
#[test]
fn conflicting_lookup_never_changes_final_credit_or_original_attribution() {
    let (_root, cfg, mut ledger, mut store, wallet) = fixture();
    let context = context();
    let quote = store
        .quote(&cfg, &ledger, &context, "one", 200_000, wallet.now)
        .unwrap();
    store
        .issue(
            &mut ledger,
            &wallet,
            &context,
            "one",
            &quote.quote.digest(),
            wallet.now,
        )
        .unwrap();
    wallet.payments.borrow_mut().as_mut().unwrap().direction = PaymentDirection::Outbound;
    assert!(
        store
            .reconcile(&mut ledger, &wallet, &context, "one", wallet.now)
            .is_err()
    );
    assert_eq!(ledger.balance("workspace").unwrap().credited, 0);
    {
        let mut payment = wallet.payments.borrow_mut();
        let p = payment.as_mut().unwrap();
        p.direction = PaymentDirection::Inbound;
        p.status = PaymentStatus::Succeeded;
    }
    store
        .reconcile(&mut ledger, &wallet, &context, "one", wallet.now)
        .unwrap();
    wallet.payments.borrow_mut().as_mut().unwrap().status = PaymentStatus::Failed;
    assert!(
        store
            .reconcile(&mut ledger, &wallet, &context, "one", wallet.now)
            .is_err()
    );
    assert_eq!(ledger.balance("workspace").unwrap().credited, 2);
}
#[test]
fn funding_writer_refuses_shared_files_symlinks_and_second_writers() {
    let (root, cfg, _ledger, store, _wallet) = fixture();
    assert!(Store::open(&cfg.state).is_err());
    drop(store);
    std::fs::set_permissions(
        cfg.state.join("book.json"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert!(Store::open(&cfg.state).is_err());
    let link = root.path().join("link");
    std::os::unix::fs::symlink(&cfg.state, &link).unwrap();
    assert!(Store::open(&link).is_err());
}

#[test]
fn replaced_lock_during_lookup_stops_before_authoritative_credit() {
    let (_root, cfg, mut ledger, mut store, wallet) = fixture();
    let context = context();
    let quote = store
        .quote(&cfg, &ledger, &context, "one", 200_000, wallet.now)
        .unwrap();
    store
        .issue(
            &mut ledger,
            &wallet,
            &context,
            "one",
            &quote.quote.digest(),
            wallet.now,
        )
        .unwrap();
    wallet.payments.borrow_mut().as_mut().unwrap().status = PaymentStatus::Succeeded;
    *wallet.replace_lock.borrow_mut() = Some(cfg.state.join("lock"));
    assert!(
        store
            .reconcile(&mut ledger, &wallet, &context, "one", wallet.now)
            .is_err()
    );
    assert_eq!(ledger.balance("workspace").unwrap().credited, 0);
    assert!(
        store
            .issue(
                &mut ledger,
                &wallet,
                &context,
                "one",
                &quote.quote.digest(),
                wallet.now
            )
            .is_err()
    );
    assert_eq!(wallet.calls.get(), 1);
}

#[test]
fn interrupted_invoice_seal_recovers_without_a_second_invoice_or_credit() {
    let (_root, cfg, mut ledger, mut store, wallet) = fixture();
    let context = context();
    let quote = store
        .quote(&cfg, &ledger, &context, "one", 200_000, wallet.now)
        .unwrap();
    store
        .issue(
            &mut ledger,
            &wallet,
            &context,
            "one",
            &quote.quote.digest(),
            wallet.now,
        )
        .unwrap();
    store
        .book
        .records
        .get_mut(&record_key(&context, "one"))
        .unwrap()
        .phase = Phase::Issuing;
    store.save().unwrap();
    drop(store);
    let mut store = Store::open(&cfg.state).unwrap();
    let unknown = store
        .issue(
            &mut ledger,
            &wallet,
            &context,
            "one",
            &quote.quote.digest(),
            wallet.now,
        )
        .unwrap();
    assert_eq!(unknown.phase, Phase::Unknown);
    assert!(unknown.invoice.is_none());
    assert_eq!(wallet.calls.get(), 1);
    wallet.payments.borrow_mut().as_mut().unwrap().status = PaymentStatus::Succeeded;
    assert_eq!(
        store
            .reconcile(&mut ledger, &wallet, &context, "one", wallet.now)
            .unwrap()
            .phase,
        Phase::Funded
    );
    assert_eq!(ledger.balance("workspace").unwrap().credited, 2);
    assert_eq!(wallet.calls.get(), 1);
}

#[test]
fn two_customers_can_use_the_same_local_id_without_crossing_funding_records() {
    let (_root, cfg, mut ledger, mut store, wallet) = fixture();
    let first = context();
    let mut second = first.clone();
    second.account = "customer-two".into();
    second.workspace = "workspace-two".into();
    second.payer_workspace = "workspace-two".into();
    second.tenant = "tenant-two".into();
    for (source, operation) in [
        (
            "create-two",
            Operation::Create {
                currency: "BTC".into(),
                spend_limit: 100,
                topups_allowed: true,
            },
        ),
        (
            "policy-two",
            Operation::FundingPolicy {
                policy: cfg.policy.clone(),
            },
        ),
    ] {
        ledger
            .apply(Mutation {
                workspace: second.workspace.clone(),
                source: source.into(),
                audit: "fixture".into(),
                operation,
            })
            .unwrap();
    }
    let a = store
        .quote(&cfg, &ledger, &first, "same-id", 200_000, wallet.now)
        .unwrap();
    let b = store
        .quote(&cfg, &ledger, &second, "same-id", 300_000, wallet.now)
        .unwrap();
    assert_ne!(a.quote.digest(), b.quote.digest());
    assert_eq!(store.read("same-id", &first).unwrap(), a);
    assert_eq!(store.read("same-id", &second).unwrap(), b);
    assert!(
        store
            .issue(
                &mut ledger,
                &wallet,
                &second,
                "same-id",
                &a.quote.digest(),
                wallet.now
            )
            .is_err()
    );
    assert_eq!(wallet.calls.get(), 0);
    store
        .issue(
            &mut ledger,
            &wallet,
            &first,
            "same-id",
            &a.quote.digest(),
            wallet.now,
        )
        .unwrap();
    wallet.payments.borrow_mut().as_mut().unwrap().status = PaymentStatus::Succeeded;
    store
        .reconcile(&mut ledger, &wallet, &first, "same-id", wallet.now)
        .unwrap();
    assert_eq!(ledger.balance(&first.workspace).unwrap().credited, 2);
    assert_eq!(ledger.balance(&second.workspace).unwrap().credited, 0);
}
