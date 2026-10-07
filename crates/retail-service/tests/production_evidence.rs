//! Exact signed synthetic receiver evidence exercises the production verifier.
//! No real funds, provider credentials, owner computer, or home is used.
use nostr::x402::test_invoice;
use openagents_wallet::*;
use retail_cloud::boat::{BoatAdapter, BoatConfig};
use retail_qualify::{
    bound,
    qualify::{self, DeploymentBinding, Mode},
    sim::FakeBoat,
};
use retail_service::package::verify_qualification;
use std::{
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
#[derive(Default)]
struct Receiver {
    record: Mutex<Option<PaymentRecord>>,
    changed: AtomicBool,
    short: AtomicBool,
    no_preimage: AtomicBool,
    after_lookup: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}
impl LightningWallet for Receiver {
    fn node_id(&self) -> String {
        hex(&test_invoice::payee())
    }
    fn receive_exact(
        &self,
        amount: u64,
        hash: [u8; 32],
        expiry: u32,
    ) -> Result<IssuedInvoice, WalletError> {
        let payment_hash = retail_cloud::sha256_hex(&[7; 32]);
        let mut fields = test_invoice::tag(1, &test_invoice::words(&parse_hash32(&payment_hash)?));
        fields.extend(test_invoice::tag(16, &test_invoice::words(&[2; 32])));
        fields.extend(test_invoice::tag(23, &test_invoice::words(&hash)));
        fields.extend(test_invoice::tag(6, &test_invoice::number(expiry as u64)));
        let now = retail_service::http::now();
        let invoice = test_invoice::signed_at(
            &format!("lnbc{}n", amount / 100),
            fields,
            false,
            false,
            now as u64,
        );
        *self.record.lock().unwrap() = Some(PaymentRecord {
            payment_hash: payment_hash.clone(),
            direction: PaymentDirection::Inbound,
            status: PaymentStatus::Succeeded,
            amount_msat: Some(amount),
            fee_msat: None,
            preimage: Some(hex(&[7; 32])),
            bolt11: None,
            updated_at: now as u64,
        });
        Ok(IssuedInvoice {
            bolt11: invoice,
            payment_hash,
            amount_msat: amount,
            description_hash: hex(&hash),
            expiry_secs: expiry,
            pay_to: self.node_id(),
        })
    }
    fn lookup(&self, _: [u8; 32]) -> Result<Option<PaymentRecord>, WalletError> {
        if let Some(change) = self.after_lookup.lock().unwrap().take() {
            change();
        }
        let mut r = self.record.lock().unwrap().clone();
        if let Some(r) = &mut r {
            if self.changed.load(Ordering::SeqCst) {
                r.direction = PaymentDirection::Outbound;
            }
            if self.short.load(Ordering::SeqCst) {
                r.amount_msat = r.amount_msat.map(|a| a - 1);
            }
            if self.no_preimage.load(Ordering::SeqCst) {
                r.preimage = None;
            }
        }
        Ok(r)
    }
    fn pay(&self, _: &str, _: u64, _: Duration) -> Result<Proof, WalletError> {
        panic!("the receiver never pays")
    }
    fn balance(&self) -> Result<Balance, WalletError> {
        panic!("no wallet balance is inferred")
    }
    fn channels(&self) -> Result<Vec<Channel>, WalletError> {
        panic!("no channel access")
    }
    fn funding_address(&self) -> Result<String, WalletError> {
        panic!("no chain funding")
    }
    fn open_channel(&self, _: &str, _: &str, _: u64, _: bool) -> Result<String, WalletError> {
        panic!("no channel mutation")
    }
    fn close_channel(&self, _: &str, _: &str, _: bool) -> Result<(), WalletError> {
        panic!("no channel mutation")
    }
}
struct SyntheticPaid;
impl bound::Payer for SyntheticPaid {
    fn pay(&self, _: &IssuedInvoice) -> Result<Option<Proof>, String> {
        Ok(None)
    }
}
#[test]
fn qualification_is_rebuilt_from_native_delivery_and_exact_ldk_shaped_receiver() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let state = root.join("qualification");
    bound::prepare_state_dir(&state).unwrap();
    bound::prepare_state_dir(&state.join("boat")).unwrap();
    let plan = qualify::fixture();
    let receiver = Receiver::default();
    let server = FakeBoat::start("synthetic-retail-key").unwrap();
    let boat = BoatAdapter::new(
        boat::ApiKey::new("synthetic-retail-key").unwrap(),
        &BoatConfig {
            base_url: server.base().into(),
            org: None,
            state_dir: state.join("boat"),
            retry: Some(boat::RetryPolicy {
                max_retries: 2,
                base_delay: Duration::from_millis(1),
                max_delay: Duration::from_millis(5),
            }),
        },
    )
    .unwrap();
    let mut receipt = bound::run(
        &bound::Run {
            plan: &plan,
            wallet: &receiver,
            boat: &boat,
            payer: &SyntheticPaid,
            model_provider: "openai".into(),
            model_key: "synthetic-customer-model-key".into(),
            template: "oa-coder-main-20261007".into(),
            state_dir: &state,
            poll: Duration::from_millis(1),
            payment_wait: Duration::from_secs(2),
        },
        Mode::Funded,
        "Signed synthetic verifier fixture; no real payment or live qualification.",
    );
    assert!(receipt.qualified, "{receipt:#?}");
    let binding = DeploymentBinding {
        receiver: receiver.node_id(),
        boat_api_base: server.base().into(),
        boat_org: None,
        boat_key_digest: retail_cloud::sha256_hex(b"synthetic-retail-key"),
        template: "oa-coder-main-20261007".into(),
        model_provider: "openai".into(),
        price_book: route_contract::digest_of(&retail_cloud::contract::price_book()),
    };
    receipt.deployment = Some(binding.clone());
    let verify = |r: &qualify::QualificationReceipt, b: &DeploymentBinding| {
        verify_qualification(&plan, r, b, &state, &state.join("ledger.sqlite"), &receiver)
    };
    assert_eq!(verify(&receipt, &binding), Ok(()));
    // LDK has no returned invoice or inbound routing fee, so the retained
    // signed invoice and actual full claimed amount supply the evidence.
    assert!(
        receiver
            .record
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .bolt11
            .is_none()
    );
    assert!(
        receiver
            .record
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .fee_msat
            .is_none()
    );
    let mut fake = receipt.clone();
    fake.mode = Mode::Fake;
    assert_eq!(verify(&fake, &binding), Err("qualification_not_valid"));
    let mut renamed = qualify::run_fake(&plan);
    renamed.mode = Mode::Funded;
    renamed.deployment = Some(binding.clone());
    assert!(
        verify(&renamed, &binding).is_err(),
        "relabeling a fake cannot supply native funding or delivery"
    );
    let mut mismatch = binding.clone();
    mismatch.template = "oa-coder-main-20261008".into();
    assert_eq!(verify(&receipt, &mismatch), Err("qualification_not_valid"));
    for change in 0..4 {
        let mut invalid = receipt.clone();
        match change {
            0 => invalid.charge_msat = Some(1),
            1 => invalid.provider_seconds = Some(1),
            2 => invalid.settlement_source = Some("another-debit".into()),
            _ => invalid.hold = Some("another-hold".into()),
        };
        assert!(verify(&invalid, &binding).is_err());
    }
    receiver.changed.store(true, Ordering::SeqCst);
    assert_eq!(
        verify(&receipt, &binding),
        Err("qualification_receiver_not_paid")
    );
    receiver.changed.store(false, Ordering::SeqCst);
    receiver.short.store(true, Ordering::SeqCst);
    assert_eq!(
        verify(&receipt, &binding),
        Err("qualification_receiver_not_paid")
    );
    receiver.short.store(false, Ordering::SeqCst);
    receiver.no_preimage.store(true, Ordering::SeqCst);
    assert_eq!(
        verify(&receipt, &binding),
        Err("qualification_receiver_not_paid")
    );
    receiver.no_preimage.store(false, Ordering::SeqCst);
    let ledger_path = state.join("ledger.sqlite");
    let displaced = state.join("original-ledger.sqlite");
    let original = ledger_path.clone();
    let backup = displaced.clone();
    *receiver.after_lookup.lock().unwrap() = Some(Box::new(move || {
        std::fs::rename(&original, &backup).unwrap();
        std::fs::copy(&backup, &original).unwrap();
    }));
    assert_eq!(
        verify(&receipt, &binding),
        Err("qualification_sources_changed")
    );
    std::fs::remove_file(&ledger_path).unwrap();
    std::fs::rename(displaced, &ledger_path).unwrap();
    assert_eq!(server.sandboxes_created(), 1);
    assert_eq!(server.executors_started(), 1);
    assert_eq!(server.active(), 0);
    let db = rusqlite::Connection::open(state.join("ledger.sqlite")).unwrap();
    // Deliberate private-fixture corruption bypasses the native foreign key;
    // production code never performs this mutation.
    db.execute_batch("PRAGMA foreign_keys=OFF;").unwrap();
    db.execute(
        "DELETE FROM settlement WHERE payment_hash=?",
        [receipt.settlement_source.as_ref().unwrap()],
    )
    .unwrap();
    drop(db);
    assert_eq!(
        verify(&receipt, &binding),
        Err("qualification_debit_absent")
    );
}
