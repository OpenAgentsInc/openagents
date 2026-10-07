use nostr::x402::test_invoice::{number, payee_of, signed_by, tag, words};
use openagents_wallet::{Balance, Channel, PaymentRecord, Proof, WalletError};
use openagents_wallet::{IssuedInvoice, LightningWallet, PaymentDirection, PaymentStatus};
use sha2::{Digest, Sha256};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::{
    cell::{Cell, RefCell},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
const NODE: [u8; 32] = [9; 32];
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
struct Wallet {
    calls: Cell<usize>,
    payments: RefCell<Option<PaymentRecord>>,
    lost: bool,
    replace_lock: RefCell<Option<PathBuf>>,
}
impl Wallet {
    fn new(_now: u64) -> Self {
        Self {
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
        let issued_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
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
            issued_at,
        );
        *self.payments.borrow_mut() = Some(PaymentRecord {
            payment_hash: hex(&payment),
            direction: PaymentDirection::Inbound,
            status: PaymentStatus::Pending,
            amount_msat: Some(amount),
            fee_msat: None,
            preimage: Some(hex(&preimage)),
            bolt11: Some(invoice.clone()),
            updated_at: issued_at,
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

// The synchronized wrapper lets the actual resident protocol serve the fixture.
pub struct Receiver(std::sync::Mutex<Wallet>);
impl Receiver {
    pub fn new(now: u64) -> Self {
        Self(std::sync::Mutex::new(Wallet::new(now)))
    }
    pub fn confirm(&self) {
        let wallet = self.0.lock().unwrap();
        let mut payment = wallet.payments.borrow_mut();
        let payment = payment.as_mut().unwrap();
        payment.status = PaymentStatus::Succeeded;
        payment.bolt11 = None;
        payment.fee_msat = None;
    }
    pub fn invoice_count(&self) -> usize {
        self.0.lock().unwrap().calls.get()
    }
}
impl LightningWallet for Receiver {
    fn node_id(&self) -> String {
        self.0.lock().unwrap().node_id()
    }
    fn receive_exact(&self, a: u64, h: [u8; 32], e: u32) -> Result<IssuedInvoice, WalletError> {
        self.0.lock().unwrap().receive_exact(a, h, e)
    }
    fn lookup(&self, h: [u8; 32]) -> Result<Option<PaymentRecord>, WalletError> {
        self.0.lock().unwrap().lookup(h)
    }
    fn pay(&self, _: &str, _: u64, _: Duration) -> Result<Proof, WalletError> {
        panic!("Receiver cannot pay.")
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
impl openagents_wallet::resident::Served for Receiver {
    fn status(&self) -> serde_json::Value {
        serde_json::json!({"fixture":true})
    }
    fn buy_channel(
        &self,
        _: u64,
        _: u64,
        _: u32,
        _: bool,
    ) -> Result<serde_json::Value, WalletError> {
        unimplemented!()
    }
    fn channel_order(&self, _: &str) -> Result<serde_json::Value, WalletError> {
        unimplemented!()
    }
    fn send_onchain(&self, _: &str, _: u64) -> Result<String, WalletError> {
        unimplemented!()
    }
}
