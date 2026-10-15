//! Portable service contract and the blocking wallet adapter.
use crate::{Error, Result};
use async_trait::async_trait;
use openagents_wallet::{
    Balance, Channel, IssuedInvoice, LightningWallet, PaymentRecord, Proof, WalletError,
};
use std::{sync::Arc, time::Duration};

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Health {
    pub synced_to_chain: bool,
    pub synced_to_graph: bool,
    pub active_channels: u32,
    pub peers: u32,
    pub inbound_msat: u64,
    pub outbound_msat: u64,
    pub pending_force_closes: usize,
}
#[async_trait]
pub trait LightningRail: Send + Sync + 'static {
    fn node_id(&self) -> String;
    async fn receive_exact(
        &self,
        amount_msat: u64,
        description_hash: [u8; 32],
        expiry_secs: u32,
    ) -> Result<IssuedInvoice>;
    async fn pay(&self, invoice: &str, max_fee_msat: u64, wait: Duration) -> Result<Proof>;
    async fn lookup(&self, payment_hash: [u8; 32]) -> Result<Option<PaymentRecord>>;
    async fn payments(&self) -> Result<Vec<PaymentRecord>> {
        Err(Error::PermissionDenied)
    }
    async fn balance(&self) -> Result<Balance>;
    async fn channels(&self) -> Result<Vec<Channel>>;
    async fn health(&self) -> Result<Health>;
    async fn funding_address(&self) -> Result<String>;
    async fn open_channel(&self, _: &str, _: &str, _: u64, _: bool) -> Result<String> {
        Err(Error::PermissionDenied)
    }
    async fn close_channel(&self, _: &str, _: &str, _: bool) -> Result<()> {
        Err(Error::PermissionDenied)
    }
}

/// Owns its runtime on a dedicated thread; safe even inside an async caller.
/// No application runtime is blocked or entered recursively.
type Job<R> = Box<dyn FnOnce(&tokio::runtime::Runtime, &Arc<R>) + Send>;
pub struct Wallet<R: LightningRail> {
    jobs: std::sync::mpsc::Sender<Job<R>>,
    node_id: String,
}
impl<R: LightningRail> Clone for Wallet<R> {
    fn clone(&self) -> Self {
        Self {
            jobs: self.jobs.clone(),
            node_id: self.node_id.clone(),
        }
    }
}
impl<R: LightningRail> Wallet<R> {
    pub fn new(rail: Arc<R>) -> Result<Self> {
        Self::start(move |_| Ok(rail))
    }
    fn start(
        create: impl FnOnce(&tokio::runtime::Runtime) -> Result<Arc<R>> + Send + 'static,
    ) -> Result<Self> {
        let (jobs, receiver) = std::sync::mpsc::channel::<Job<R>>();
        let (ready, wait) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
            {
                Ok(r) => r,
                Err(_) => {
                    let _ = ready.send(Err(Error::Unavailable));
                    return;
                }
            };
            let rail = match create(&runtime) {
                Ok(r) => r,
                Err(e) => {
                    let _ = ready.send(Err(e));
                    return;
                }
            };
            if ready.send(Ok(rail.node_id())).is_err() {
                return;
            }
            while let Ok(job) = receiver.recv() {
                job(&runtime, &rail);
            }
        });
        Ok(Self {
            jobs,
            node_id: wait.recv().map_err(|_| Error::Unavailable)??,
        })
    }
    fn run<T: Send + 'static>(
        &self,
        f: impl FnOnce(Arc<R>) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T>> + Send>>
        + Send
        + 'static,
    ) -> Result<T> {
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        self.jobs
            .send(Box::new(move |runtime, rail| {
                let _ = send.send(runtime.block_on(f(rail.clone())));
            }))
            .map_err(|_| Error::Unavailable)?;
        receive.recv().map_err(|_| Error::Unavailable)?
    }
}
impl Wallet<crate::Client> {
    pub fn receive_hinted(&self, a: u64, h: [u8; 32], e: u32) -> Result<IssuedInvoice> {
        self.run(move |r| {
            Box::pin(async move {
                let hints = r.private_route_hints().await?;
                r.create_invoice_with_hints(a, h, e, hints).await
            })
        })
    }
    pub fn received_msat(&self, hash: [u8; 32]) -> Result<Option<u64>> {
        self.run(move |r| Box::pin(async move { r.received_msat(hash).await }))
    }
    pub fn connect(
        config: crate::transport::Config,
        macaroon: crate::macaroon::Macaroon,
    ) -> Result<Self> {
        Self::start(move |runtime| {
            runtime
                .block_on(crate::Client::connect(&config, macaroon))
                .map(Arc::new)
        })
    }
    pub fn connect_receiver(
        config: crate::transport::Config,
        macaroon: crate::macaroon::Macaroon,
        node: String,
        network: String,
    ) -> Result<Self> {
        Self::start(move |runtime| {
            let client = runtime.block_on(crate::Client::connect(&config, macaroon))?;
            if client.node_id() != node || client.network() != network {
                return Err(Error::NodeMismatch);
            }
            Ok(Arc::new(client))
        })
    }
}
fn wallet_error(e: Error) -> WalletError {
    WalletError::Node(e.to_string())
}
impl<R: LightningRail> LightningWallet for Wallet<R> {
    fn node_id(&self) -> String {
        self.node_id.clone()
    }
    fn receive_exact(
        &self,
        a: u64,
        h: [u8; 32],
        e: u32,
    ) -> std::result::Result<IssuedInvoice, WalletError> {
        self.run(move |r| Box::pin(async move { r.receive_exact(a, h, e).await }))
            .map_err(wallet_error)
    }
    fn pay(&self, i: &str, f: u64, w: Duration) -> std::result::Result<Proof, WalletError> {
        let invoice = i.to_owned();
        let hash = crate::router::invoice(&invoice)?.payment_hash().to_string();
        self.run(move |r| Box::pin(async move { r.pay(&invoice, f, w).await }))
            .map_err(|e| match e {
                Error::InFlight | Error::Unavailable | Error::Timeout | Error::AlreadyPaid => {
                    WalletError::Pending {
                        payment_hash: hash,
                        waited_secs: w.as_secs(),
                    }
                }
                Error::NoRoute | Error::InsufficientBalance | Error::IncorrectPaymentDetails => {
                    WalletError::Failed {
                        payment_hash: hash,
                        reason: e.to_string(),
                    }
                }
                other => wallet_error(other),
            })
    }
    fn lookup(&self, h: [u8; 32]) -> std::result::Result<Option<PaymentRecord>, WalletError> {
        self.run(move |r| Box::pin(async move { r.lookup(h).await }))
            .map_err(wallet_error)
    }
    fn payments(&self) -> std::result::Result<Vec<PaymentRecord>, WalletError> {
        self.run(move |r| Box::pin(async move { r.payments().await }))
            .map_err(wallet_error)
    }
    fn balance(&self) -> std::result::Result<Balance, WalletError> {
        self.run(move |r| Box::pin(async move { r.balance().await }))
            .map_err(wallet_error)
    }
    fn channels(&self) -> std::result::Result<Vec<Channel>, WalletError> {
        self.run(move |r| Box::pin(async move { r.channels().await }))
            .map_err(wallet_error)
    }
    fn funding_address(&self) -> std::result::Result<String, WalletError> {
        self.run(move |r| Box::pin(async move { r.funding_address().await }))
            .map_err(wallet_error)
    }
    fn open_channel(
        &self,
        n: &str,
        a: &str,
        s: u64,
        p: bool,
    ) -> std::result::Result<String, WalletError> {
        let n = n.to_owned();
        let a = a.to_owned();
        self.run(move |r| Box::pin(async move { r.open_channel(&n, &a, s, p).await }))
            .map_err(wallet_error)
    }
    fn close_channel(&self, i: &str, c: &str, f: bool) -> std::result::Result<(), WalletError> {
        let i = i.to_owned();
        let c = c.to_owned();
        self.run(move |r| Box::pin(async move { r.close_channel(&i, &c, f).await }))
            .map_err(wallet_error)
    }
}

/// Uses LND for Lightning and delegates address resolution and Spark custody to
/// the existing provider. The adapter never starts a second Spark wallet.
pub struct Rails<R: LightningRail, D> {
    pub wallet: Wallet<R>,
    pub delegate: D,
    pub payment_timeout: Duration,
}
impl<R: LightningRail, D: pay_ledger::payout::Rails> pay_ledger::payout::Rails for Rails<R, D> {
    fn lightning_invoice(
        &self,
        address: &str,
        amount_msat: i64,
    ) -> std::result::Result<pay_ledger::payout::Invoice, String> {
        self.delegate.lightning_invoice(address, amount_msat)
    }
    fn pay_lightning(
        &self,
        invoice: &pay_ledger::payout::Invoice,
        max_fee_msat: i64,
    ) -> pay_ledger::payout::Outcome {
        use pay_ledger::payout::Outcome;
        let valid = crate::router::invoice(&invoice.bolt11).is_ok_and(|decoded| {
            decoded.payment_hash().to_string() == invoice.payment_hash
                && decoded
                    .amount_milli_satoshis()
                    .and_then(|a| i64::try_from(a).ok())
                    == Some(invoice.amount_msat)
        });
        if !valid || invoice.amount_msat <= 0 || max_fee_msat < 0 {
            return Outcome::Failed("invalid payment terms".into());
        }
        match self
            .wallet
            .pay(&invoice.bolt11, max_fee_msat as u64, self.payment_timeout)
        {
            Ok(proof) => match i64::try_from(proof.fee_msat) {
                Ok(fee_msat) => Outcome::Sent { fee_msat },
                Err(_) => Outcome::Unknown("invalid payment fee".into()),
            },
            Err(WalletError::Failed { reason, .. }) => Outcome::Failed(reason),
            Err(e) => Outcome::Unknown(e.to_string()),
        }
    }
    fn lookup_lightning(
        &self,
        payment_hash: &str,
    ) -> std::result::Result<pay_ledger::payout::Lookup, String> {
        use pay_ledger::payout::Lookup;
        let hash = openagents_wallet::parse_hash32(payment_hash).map_err(|e| e.to_string())?;
        match self.wallet.lookup(hash).map_err(|e| e.to_string())? {
            Some(p) if p.direction != openagents_wallet::PaymentDirection::Outbound => {
                Err("payment direction mismatch".into())
            }
            Some(p) => match p.status {
                openagents_wallet::PaymentStatus::Succeeded => Ok(Lookup::Sent {
                    fee_msat: i64::try_from(p.fee_msat.ok_or("missing payment fee")?)
                        .map_err(|_| "invalid payment fee")?,
                }),
                openagents_wallet::PaymentStatus::Failed => {
                    Ok(Lookup::Failed("payment failed".into()))
                }
                openagents_wallet::PaymentStatus::Pending => Ok(Lookup::Pending),
            },
            None => Ok(Lookup::Absent),
        }
    }
    fn fund_spark(&self, amount_sats: u64) -> std::result::Result<(), String> {
        self.delegate.fund_spark(amount_sats)
    }
    fn pay_spark(&self, address: &str, amount_sats: u64, key: &str) -> pay_ledger::payout::Outcome {
        self.delegate.pay_spark(address, amount_sats, key)
    }
    fn lookup_spark(&self, key: &str) -> std::result::Result<pay_ledger::payout::Lookup, String> {
        self.delegate.lookup_spark(key)
    }
}
