//! Persisted payout selection. References, rather than current policy, choose recovery.
use crate::{
    Client, Error, Result,
    liquidity::Journal,
    service::{LightningRail, Wallet},
};
use openagents_wallet::LightningWallet;
use pay_ledger::payout::{Invoice, Lookup, Outcome, Rails};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Mdk,
    Shadow,
    Canary,
    Lnd,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub version: u32,
    pub mode: Mode,
    pub mainnet_approved: bool,
    pub canary_parties: Vec<String>,
    pub account_id: String,
    pub account_limit_sats: u64,
}
impl Policy {
    pub fn load(path: &Path) -> Result<Self> {
        let p: Self =
            serde_json::from_slice(&std::fs::read(path).map_err(|_| Error::PermissionDenied)?)
                .map_err(|_| Error::Invalid("payout policy"))?;
        if p.version != 1 {
            return Err(Error::Invalid("payout policy version"));
        }
        if matches!(p.mode, Mode::Canary | Mode::Lnd)
            && (!p.mainnet_approved || p.account_limit_sats == 0 || p.account_id.is_empty())
        {
            return Err(Error::PermissionDenied);
        }
        Ok(p)
    }
    pub fn lnd_for(&self, party: &str) -> bool {
        self.mode == Mode::Lnd
            || self.mode == Mode::Canary && self.canary_parties.iter().any(|p| p == party)
    }
}

pub struct Shared<D>(pub Arc<D>);
impl<D: Rails> Rails for Shared<D> {
    fn lightning_invoice(&self, a: &str, n: i64) -> std::result::Result<Invoice, String> {
        self.0.lightning_invoice(a, n)
    }
    fn pay_lightning(&self, i: &Invoice, f: i64) -> Outcome {
        self.0.pay_lightning(i, f)
    }
    fn lookup_lightning(&self, h: &str) -> std::result::Result<Lookup, String> {
        self.0.lookup_lightning(h)
    }
    fn fund_spark(&self, n: u64) -> std::result::Result<(), String> {
        self.0.fund_spark(n)
    }
    fn spark_funding_invoice(&self, n: u64) -> std::result::Result<Option<Invoice>, String> {
        self.0.spark_funding_invoice(n)
    }
    fn await_spark_funding(&self, n: u64) -> std::result::Result<(), String> {
        self.0.await_spark_funding(n)
    }
    fn pay_spark(&self, a: &str, n: u64, k: &str) -> Outcome {
        self.0.pay_spark(a, n, k)
    }
    fn lookup_spark(&self, k: &str) -> std::result::Result<Lookup, String> {
        self.0.lookup_spark(k)
    }
}

pub struct Router<R: LightningRail, D: Rails> {
    pub old: Arc<D>,
    pub wallet: Wallet<R>,
    /// Outside the deployment directory; never written by startup or an installer.
    pub policy: PathBuf,
    pub observations: PathBuf,
    pub funding_journal: PathBuf,
}
impl<R: LightningRail, D: Rails> Router<R, D> {
    fn new_rail(&self) -> crate::service::Rails<R, Shared<D>> {
        crate::service::Rails {
            wallet: self.wallet.clone(),
            delegate: Shared(self.old.clone()),
            payment_timeout: Duration::from_secs(60),
            funding_journal: self.funding_journal.clone(),
        }
    }
    fn observe(&self, party: &str, invoice: &Invoice) {
        let estimate = self.wallet.estimate(&invoice.bolt11);
        let at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let observed = Journal::open(&self.observations).and_then(|mut log| log.save(&invoice.payment_hash, json!({"at": at, "party": party, "amount_msat": invoice.amount_msat,
                "lnd_estimate_msat": estimate.as_ref().ok(), "error": estimate.err().map(|e| e.to_string()), "sent_on": "mdk"}))
                );
        if observed.is_err() {
            eprintln!("could not save the payout fee estimate");
        }
    }
    fn choose(&self, party: &str, invoice: &Invoice) -> std::result::Result<String, String> {
        let p = Policy::load(&self.policy).map_err(|e| e.to_string())?;
        if p.mode == Mode::Shadow {
            self.observe(party, invoice);
        }
        Ok(if p.lnd_for(party) {
            format!("lnd:{}:{}", self.wallet.node_id(), invoice.payment_hash)
        } else {
            invoice.payment_hash.clone()
        })
    }
}
impl<R: LightningRail, D: Rails> Rails for Router<R, D> {
    fn lightning_invoice(&self, a: &str, n: i64) -> std::result::Result<Invoice, String> {
        self.old.lightning_invoice(a, n)
    }
    fn lightning_reference(
        &self,
        party: &str,
        invoice: &Invoice,
    ) -> std::result::Result<String, String> {
        self.choose(party, invoice)
    }
    fn pay_lightning(&self, _: &Invoice, _: i64) -> Outcome {
        Outcome::Failed("a saved payout reference is required".into())
    }
    fn pay_lightning_reference(&self, invoice: &Invoice, reference: &str, cap: i64) -> Outcome {
        if reference == invoice.payment_hash || reference == format!("mdk:{}", invoice.payment_hash)
        {
            return self.old.pay_lightning(invoice, cap);
        }
        match reference
            .strip_prefix("lnd:")
            .and_then(|r| r.split_once(':'))
        {
            Some((node, hash)) if node == self.wallet.node_id() && hash == invoice.payment_hash => {
                self.new_rail().pay_lightning(invoice, cap)
            }
            _ => Outcome::Failed("invalid payout reference".into()),
        }
    }
    fn lookup_lightning(&self, reference: &str) -> std::result::Result<Lookup, String> {
        if let Some(saved) = reference.strip_prefix("lnd:") {
            let (node, hash) = saved.split_once(':').ok_or("invalid payout reference")?;
            if node != self.wallet.node_id() {
                return Err("the original payout node is required".into());
            }
            return self.new_rail().lookup_lightning(hash);
        }
        if let Some(hash) = reference.strip_prefix("mdk:") {
            return self.old.lookup_lightning(hash);
        }
        if reference.contains(':') {
            return Err("unknown payout backend".into());
        }
        self.old.lookup_lightning(reference)
    }

    fn fund_spark_for(&self, party: &str, amount: u64) -> std::result::Result<(), String> {
        let p = Policy::load(&self.policy).map_err(|e| e.to_string())?;
        // Finish an earlier LND refill on its original rail, including after rollback.
        let pending = Journal::open(&self.funding_journal)
            .map_err(|e| e.to_string())?
            .entries
            .get("funding")
            .is_some_and(|v| v["state"] != "confirmed" && v["state"] != "failed");
        if pending || p.lnd_for(party) {
            self.new_rail().fund_spark(amount)
        } else if p.mode == Mode::Shadow {
            self.old
                .fund_spark_shadow(amount, &|invoice| self.observe(party, invoice))
        } else {
            self.old.fund_spark(amount)
        }
    }
    fn fund_spark(&self, _: u64) -> std::result::Result<(), String> {
        Err("a payout party is required".into())
    }
    fn pay_spark(&self, a: &str, n: u64, k: &str) -> Outcome {
        self.old.pay_spark(a, n, k)
    }
    fn lookup_spark(&self, k: &str) -> std::result::Result<Lookup, String> {
        self.old.lookup_spark(k)
    }
}

/// Operations credential for litd only; the payout process never receives it.
pub struct Accounts {
    channel: crate::transport::AuthChannel,
}
pub struct AccountAccess {
    pub id: String,
    pub limit_sats: u64,
    pub macaroon: crate::macaroon::Macaroon,
}
impl Accounts {
    pub async fn connect(
        config: &crate::transport::Config,
        macaroon: crate::macaroon::Macaroon,
    ) -> Result<Self> {
        let channel = crate::transport::connect(config, macaroon).await?;
        let info = crate::transport::read(|| {
            let mut rpc = crate::proto::litrpc::proxy_client::ProxyClient::new(channel.clone());
            async move { rpc.get_info(crate::proto::litrpc::GetInfoRequest {}).await }
        })
        .await?;
        if !info.version.trim_start_matches('v').starts_with("0.17.")
            && !config.allow_version_mismatch
        {
            return Err(Error::VersionMismatch);
        }
        Ok(Self { channel })
    }
    pub async fn create(
        &self,
        label: String,
        cap_sats: u64,
        expiration: i64,
    ) -> Result<AccountAccess> {
        if cap_sats == 0 || label.is_empty() || expiration <= 0 {
            return Err(Error::Invalid("payout account terms"));
        }
        let mut rpc =
            crate::proto::litrpc::accounts_client::AccountsClient::new(self.channel.clone());
        let result = crate::transport::once(rpc.create_account(
            crate::proto::litrpc::CreateAccountRequest {
                account_balance: cap_sats,
                expiration_date: expiration,
                label,
            },
        ))
        .await?;
        let account = result.account.ok_or(Error::Invalid("payout account"))?;
        if account.initial_balance != cap_sats {
            return Err(Error::Invalid("payout account limit"));
        }
        Ok(AccountAccess {
            id: account.id,
            limit_sats: cap_sats,
            macaroon: crate::macaroon::Macaroon::new(
                crate::macaroon::Role::Payout,
                0,
                result.macaroon,
            )?,
        })
    }
}

impl Wallet<Client> {
    pub fn route_estimate(&self, invoice: &str) -> Result<u64> {
        self.estimate(invoice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[derive(Default)]
    struct Old {
        sends: AtomicUsize,
        invoices: AtomicUsize,
        funded: AtomicUsize,
    }
    fn invoice() -> Invoice {
        let bolt11 = crate::tests::invoice_fixture(1_000_000);
        Invoice {
            payment_hash: crate::router::invoice(&bolt11)
                .unwrap()
                .payment_hash()
                .to_string(),
            bolt11,
            amount_msat: 1_000_000,
        }
    }
    impl Rails for Old {
        fn lightning_invoice(&self, _: &str, _: i64) -> std::result::Result<Invoice, String> {
            Ok(invoice())
        }
        fn pay_lightning(&self, _: &Invoice, _: i64) -> Outcome {
            self.sends.fetch_add(1, Ordering::SeqCst);
            Outcome::Sent { fee_msat: 1 }
        }
        fn lookup_lightning(&self, _: &str) -> std::result::Result<Lookup, String> {
            Ok(Lookup::Absent)
        }
        fn fund_spark(&self, _: u64) -> std::result::Result<(), String> {
            panic!("refill used the old sender")
        }
        fn spark_funding_invoice(&self, _: u64) -> std::result::Result<Option<Invoice>, String> {
            self.invoices.fetch_add(1, Ordering::SeqCst);
            Ok(Some(invoice()))
        }
        fn await_spark_funding(&self, _: u64) -> std::result::Result<(), String> {
            self.funded.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn pay_spark(&self, _: &str, _: u64, _: &str) -> Outcome {
            panic!("unexpected transfer")
        }
        fn lookup_spark(&self, _: &str) -> std::result::Result<Lookup, String> {
            Ok(Lookup::Absent)
        }
    }
    fn policy(path: &Path, mode: Mode) {
        std::fs::write(
            path,
            serde_json::to_vec(&Policy {
                version: 1,
                mode,
                mainnet_approved: true,
                canary_parties: vec!["our-test-payee".into()],
                account_id: "test-account".into(),
                account_limit_sats: 50_000,
            })
            .unwrap(),
        )
        .unwrap();
    }
    #[test]
    fn shadow_and_canary_do_not_dispatch_on_the_other_backend() {
        let dir = tempfile::tempdir().unwrap();
        let fake = Arc::new(crate::fake::Fake {
            node_id: "test-node".into(),
            available_sats: 1_000_000,
            ..Default::default()
        });
        let old = Arc::new(Old::default());
        let router = Router {
            old: old.clone(),
            wallet: Wallet::new(fake.clone()).unwrap(),
            policy: dir.path().join("policy.json"),
            observations: dir.path().join("shadow.json"),
            funding_journal: dir.path().join("refill.json"),
        };
        policy(&router.policy, Mode::Shadow);
        let i = invoice();
        let reference = router.lightning_reference("our-test-payee", &i).unwrap();
        assert_eq!(reference, i.payment_hash);
        assert!(matches!(
            router.pay_lightning_reference(&i, &reference, 1000),
            Outcome::Sent { .. }
        ));
        assert_eq!(fake.sends.load(Ordering::SeqCst), 0);
        assert_eq!(old.sends.load(Ordering::SeqCst), 1);
        assert!(
            Journal::open(&router.observations)
                .unwrap()
                .entries
                .contains_key(&i.payment_hash)
        );
        policy(&router.policy, Mode::Canary);
        assert!(
            router
                .lightning_reference("our-test-payee", &i)
                .unwrap()
                .starts_with("lnd:test-node:")
        );
        assert_eq!(
            router.lightning_reference("someone-else", &i).unwrap(),
            i.payment_hash
        );
        assert!(
            router
                .lookup_lightning(&format!("lnd:replacement-node:{}", i.payment_hash))
                .is_err()
        );
        assert_eq!(fake.sends.load(Ordering::SeqCst), 0);
    }
    #[test]
    fn interrupted_refill_tracks_its_original_hash_without_a_second_invoice_or_send() {
        use openagents_wallet::{PaymentDirection, PaymentRecord, PaymentStatus};
        let dir = tempfile::tempdir().unwrap();
        let old = Arc::new(Old::default());
        let fake = Arc::new(crate::fake::Fake {
            node_id: "test-node".into(),
            available_sats: 1_000_000,
            ..Default::default()
        });
        let rail = crate::service::Rails {
            wallet: Wallet::new(fake.clone()).unwrap(),
            delegate: Shared(old.clone()),
            payment_timeout: Duration::from_secs(1),
            funding_journal: dir.path().join("refill.json"),
        };
        assert!(rail.fund_spark(500).is_err());
        assert!(rail.fund_spark(500).is_err());
        let i = invoice();
        let hash = openagents_wallet::parse_hash32(&i.payment_hash).unwrap();
        fake.records.lock().unwrap().insert(
            hash,
            PaymentRecord {
                payment_hash: i.payment_hash.clone(),
                direction: PaymentDirection::Outbound,
                status: PaymentStatus::Succeeded,
                amount_msat: Some(1_000_000),
                fee_msat: Some(1),
                preimage: None,
                bolt11: Some(i.bolt11),
                updated_at: 1,
            },
        );
        rail.fund_spark(500).unwrap();
        assert_eq!(fake.sends.load(Ordering::SeqCst), 1);
        assert_eq!(old.invoices.load(Ordering::SeqCst), 1);
        assert_eq!(old.sends.load(Ordering::SeqCst), 0);
        assert_eq!(old.funded.load(Ordering::SeqCst), 1);
        assert_eq!(
            Journal::open(&rail.funding_journal).unwrap().entries["funding"]["state"],
            "confirmed"
        );
    }
}
