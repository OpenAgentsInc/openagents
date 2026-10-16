use openagents_wallet as contract_wallet;
#[path = "../../wallet/tests/support/contract.rs"]
mod contract;
use lnd_sdk::{
    Error, Result,
    service::{Health, LightningRail, Wallet},
};
use openagents_wallet::{Balance, Channel, IssuedInvoice, PaymentRecord, PaymentStatus, Proof};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
#[derive(Default)]
struct Rail {
    nonce: AtomicUsize,
    sends: AtomicUsize,
    records: Mutex<BTreeMap<[u8; 32], PaymentRecord>>,
}
#[async_trait::async_trait]
impl LightningRail for Rail {
    fn node_id(&self) -> String {
        contract::fixture(1, 1, [8; 32], 600).0.pay_to
    }
    async fn receive_exact(
        &self,
        amount: u64,
        request: [u8; 32],
        expiry: u32,
    ) -> Result<IssuedInvoice> {
        if amount == 0 {
            return Err(Error::Invalid("amount"));
        }
        Ok(contract::fixture(
            self.nonce.fetch_add(1, Ordering::SeqCst) as u8 + 1,
            amount,
            request,
            expiry,
        )
        .0)
    }
    async fn pay(&self, invoice: &str, max_fee: u64, _: Duration) -> Result<Proof> {
        let decoded = lnd_sdk::router::invoice(invoice).map_err(|_| Error::Invalid("invoice"))?;
        let hash = openagents_wallet::parse_hash32(&decoded.payment_hash().to_string()).unwrap();
        let mut records = self.records.lock().unwrap();
        if let Some(r) = records.get(&hash) {
            return Ok(Proof {
                payment_hash: r.payment_hash.clone(),
                preimage: r.preimage.clone().unwrap(),
                amount_msat: r.amount_msat.unwrap(),
                fee_msat: r.fee_msat.unwrap(),
                bolt11: invoice.into(),
            });
        }
        if max_fee < 3 {
            return Err(Error::NoRoute);
        }
        let (issued, record) = contract::fixture(7, contract::AMOUNT, contract::REQUEST, 600);
        if issued.payment_hash != decoded.payment_hash().to_string() {
            return Err(Error::IncorrectPaymentDetails);
        }
        self.sends.fetch_add(1, Ordering::SeqCst);
        let proof = Proof {
            payment_hash: record.payment_hash.clone(),
            preimage: record.preimage.clone().unwrap(),
            amount_msat: record.amount_msat.unwrap(),
            fee_msat: 3,
            bolt11: invoice.into(),
        };
        records.insert(hash, record);
        Ok(proof)
    }
    async fn lookup(&self, h: [u8; 32]) -> Result<Option<PaymentRecord>> {
        Ok(self.records.lock().unwrap().get(&h).cloned())
    }
    async fn payments(&self) -> Result<Vec<PaymentRecord>> {
        Ok(self
            .records
            .lock()
            .unwrap()
            .values()
            .cloned()
            .map(|mut r| {
                r.preimage = None;
                r.bolt11 = None;
                r
            })
            .collect())
    }
    async fn balance(&self) -> Result<Balance> {
        Ok(Balance {
            onchain_total_sats: 0,
            onchain_spendable_sats: 0,
            lightning_total_sats: 100000,
            anchor_reserve_sats: 0,
        })
    }
    async fn channels(&self) -> Result<Vec<Channel>> {
        Ok(vec![])
    }
    async fn health(&self) -> Result<Health> {
        Ok(Health::default())
    }
    async fn funding_address(&self) -> Result<String> {
        Err(Error::PermissionDenied)
    }
}
#[test]
fn hosted_wallet_uses_the_shared_contract_and_preserves_fee_refusals() {
    use openagents_wallet::LightningWallet;
    let rail = Arc::new(Rail::default());
    let wallet = Wallet::new(rail.clone()).unwrap();
    contract::bound(&wallet);
    assert_eq!(rail.nonce.load(Ordering::SeqCst), 0);
    contract::receive(&wallet);
    let (invoice, _) = contract::fixture(7, contract::AMOUNT, contract::REQUEST, 600);
    assert!(matches!(
        wallet.pay(&invoice.bolt11, 2, Duration::from_secs(1)),
        Err(openagents_wallet::WalletError::Failed { .. })
    ));
    assert_eq!(rail.sends.load(Ordering::SeqCst), 0);
    contract::paid(&wallet, &invoice.bolt11, 10);
    assert_eq!(rail.sends.load(Ordering::SeqCst), 1);
    assert_eq!(
        wallet
            .lookup(openagents_wallet::parse_hash32(&invoice.payment_hash).unwrap())
            .unwrap()
            .unwrap()
            .status,
        PaymentStatus::Succeeded
    );
}
