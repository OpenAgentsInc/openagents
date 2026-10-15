//! In-process services for caller tests. Fakes never connect to a node.
use crate::{
    Error, Result,
    service::{Health, LightningRail},
};
use openagents_wallet::{Balance, Channel, IssuedInvoice, PaymentRecord, Proof};
use std::{
    collections::BTreeMap,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
#[derive(Default)]
pub struct Fake {
    pub node_id: String,
    pub records: Mutex<BTreeMap<[u8; 32], PaymentRecord>>,
    pub outcome: Mutex<Option<Result<Proof>>>,
    pub sends: AtomicUsize,
}
#[async_trait::async_trait]
impl LightningRail for Fake {
    fn node_id(&self) -> String {
        self.node_id.clone()
    }
    async fn receive_exact(&self, _: u64, _: [u8; 32], _: u32) -> Result<IssuedInvoice> {
        Err(Error::PermissionDenied)
    }
    async fn pay(&self, _: &str, _: u64, _: Duration) -> Result<Proof> {
        self.sends.fetch_add(1, Ordering::SeqCst);
        self.outcome
            .lock()
            .unwrap()
            .clone()
            .unwrap_or(Err(Error::Unavailable))
    }
    async fn lookup(&self, h: [u8; 32]) -> Result<Option<PaymentRecord>> {
        Ok(self.records.lock().unwrap().get(&h).cloned())
    }
    async fn balance(&self) -> Result<Balance> {
        Ok(Balance {
            onchain_total_sats: 0,
            onchain_spendable_sats: 0,
            lightning_total_sats: 0,
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
