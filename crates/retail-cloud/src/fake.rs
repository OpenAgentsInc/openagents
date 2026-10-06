//! Simulated collaborators for tests and the fake-payment acceptance run.
//! Everything here is labeled a simulation; none of it moves money, starts
//! a machine, or reads a credential.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::Duration;

use openagents_wallet::{
    Balance, Channel, IssuedInvoice, LightningWallet, PaymentDirection, PaymentRecord,
    PaymentStatus, Proof, WalletError,
};

use crate::sha256_hex;

/// A receiver wallet that issues fake invoices and reports whatever the test
/// says happened to them.
#[derive(Default)]
pub struct FakeWallet {
    state: Mutex<WalletState>,
}

#[derive(Default)]
struct WalletState {
    issued: u64,
    invoices: BTreeMap<String, Invoice>,
    /// Make the next lookups fail as the node would.
    unreachable: bool,
}

#[derive(Clone)]
struct Invoice {
    amount_msat: u64,
    bolt11: String,
    status: Option<PaymentStatus>,
    received_msat: Option<u64>,
}

impl FakeWallet {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn with<T>(&self, f: impl FnOnce(&mut WalletState) -> T) -> T {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f(&mut state)
    }

    /// The customer pays the invoice in full.
    pub fn pay_in_full(&self, payment_hash: &str) {
        self.with(|s| {
            if let Some(invoice) = s.invoices.get_mut(payment_hash) {
                invoice.status = Some(PaymentStatus::Succeeded);
                invoice.received_msat = Some(invoice.amount_msat);
            }
        });
    }

    /// The wallet receives a different amount than the invoice asked.
    pub fn pay_amount(&self, payment_hash: &str, received_msat: u64) {
        self.with(|s| {
            if let Some(invoice) = s.invoices.get_mut(payment_hash) {
                invoice.status = Some(PaymentStatus::Succeeded);
                invoice.received_msat = Some(received_msat);
            }
        });
    }

    /// The wallet fails the invoice (for example at expiry).
    pub fn fail(&self, payment_hash: &str) {
        self.with(|s| {
            if let Some(invoice) = s.invoices.get_mut(payment_hash) {
                invoice.status = Some(PaymentStatus::Failed);
            }
        });
    }

    /// The wallet forgets the invoice, as a restored node might.
    pub fn forget(&self, payment_hash: &str) {
        self.with(|s| {
            s.invoices.remove(payment_hash);
        });
    }

    /// Lookups fail until set back.
    pub fn set_unreachable(&self, unreachable: bool) {
        self.with(|s| s.unreachable = unreachable);
    }

    /// How many invoices the wallet issued.
    #[must_use]
    pub fn issued(&self) -> u64 {
        self.with(|s| s.issued)
    }
}

impl LightningWallet for FakeWallet {
    fn node_id(&self) -> String {
        format!("02{}", "ab".repeat(32))
    }

    fn receive_exact(
        &self,
        amount_msat: u64,
        request_hash: [u8; 32],
        expiry_secs: u32,
    ) -> Result<IssuedInvoice, WalletError> {
        if amount_msat == 0 {
            return Err(WalletError::Invalid("amount".into()));
        }
        Ok(self.with(|s| {
            s.issued += 1;
            let payment_hash = sha256_hex(format!("fake-invoice:{}", s.issued).as_bytes());
            let bolt11 = format!("lnfake{amount_msat}n1{}", &payment_hash[..16]);
            s.invoices.insert(
                payment_hash.clone(),
                Invoice {
                    amount_msat,
                    bolt11: bolt11.clone(),
                    status: Some(PaymentStatus::Pending),
                    received_msat: None,
                },
            );
            IssuedInvoice {
                bolt11,
                payment_hash,
                amount_msat,
                description_hash: hex::encode(request_hash),
                expiry_secs,
                pay_to: self.node_id(),
            }
        }))
    }

    fn pay(&self, _: &str, _: u64, _: Duration) -> Result<Proof, WalletError> {
        Err(WalletError::Node("the fake receiver never pays".into()))
    }

    fn lookup(&self, payment_hash: [u8; 32]) -> Result<Option<PaymentRecord>, WalletError> {
        let hash = hex::encode(payment_hash);
        self.with(|s| {
            if s.unreachable {
                return Err(WalletError::Node("fake node unreachable".into()));
            }
            Ok(s.invoices.get(&hash).map(|invoice| PaymentRecord {
                payment_hash: hash.clone(),
                direction: PaymentDirection::Inbound,
                status: invoice.status.unwrap_or(PaymentStatus::Pending),
                amount_msat: invoice.received_msat,
                fee_msat: None,
                preimage: None,
                bolt11: Some(invoice.bolt11.clone()),
                updated_at: 0,
            }))
        })
    }

    fn balance(&self) -> Result<Balance, WalletError> {
        Err(WalletError::Node("the fake receiver has no balance".into()))
    }

    fn channels(&self) -> Result<Vec<Channel>, WalletError> {
        Ok(Vec::new())
    }

    fn funding_address(&self) -> Result<String, WalletError> {
        Err(WalletError::Node("the fake receiver has no chain".into()))
    }

    fn open_channel(&self, _: &str, _: &str, _: u64, _: bool) -> Result<String, WalletError> {
        Err(WalletError::Node(
            "the fake receiver opens no channels".into(),
        ))
    }

    fn close_channel(&self, _: &str, _: &str, _: bool) -> Result<(), WalletError> {
        Err(WalletError::Node(
            "the fake receiver has no channels".into(),
        ))
    }
}
