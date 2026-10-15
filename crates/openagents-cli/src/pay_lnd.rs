//! A second, receive-only node for agent credits. Credentials never grant sends.
use lnd_sdk::{
    Client,
    macaroon::{Macaroon, Role},
    service::Wallet,
    transport::Config,
};
use openagents_x402::{front::Front, replay::ReplayStore, server::Receiver};
use serde::Deserialize;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Backend {
    #[default]
    Mdk,
    Lnd,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReceiveSettings {
    #[serde(default)]
    pub backend: Backend,
    pub endpoint: String,
    pub certificate_sha256: String,
    pub node_id: String,
    pub invoice_macaroon: PathBuf,
    pub readonly_macaroon: PathBuf,
    #[serde(default = "mainnet")]
    pub network: String,
}
fn mainnet() -> String {
    "mainnet".into()
}
impl ReceiveSettings {
    pub fn anchor(&mut self, base: &Path) {
        for path in [&mut self.invoice_macaroon, &mut self.readonly_macaroon] {
            if path.is_relative() {
                *path = base.join(&*path);
            }
        }
    }
    fn receiver(&self) -> Result<Arc<LndReceiver>, String> {
        let pin = openagents_wallet::parse_hash32(&self.certificate_sha256)
            .map_err(|_| "The LND certificate fingerprint is invalid.")?;
        let _: secp256k1::PublicKey = self
            .node_id
            .parse()
            .map_err(|_| "The LND node identity is invalid.")?;
        if !self.endpoint.starts_with("https://") {
            return Err("The LND address must use HTTPS.".into());
        }
        Ok(Arc::new(LndReceiver {
            settings: self.clone(),
            config: Config::new(self.endpoint.clone(), pin),
            wallet: Mutex::new(None),
        }))
    }
    pub fn readonly_wallet(&self) -> Result<Wallet<Client>, String> {
        let receiver = self.receiver()?;
        let macaroon = Macaroon::from_file(Role::ReadOnly, 102, &self.readonly_macaroon)
            .map_err(|e| e.to_string())?;
        Wallet::connect_receiver(
            receiver.config.clone(),
            macaroon,
            self.node_id.clone(),
            self.network.clone(),
        )
        .map_err(|e| e.to_string())
    }
    pub fn attach<S: ReplayStore>(&self, front: Front<S>) -> Result<Front<S>, String> {
        let expected = match self.network.as_str() {
            "mainnet" => nostr::x402::MAINNET,
            "testnet" => nostr::x402::TESTNET,
            _ => return Err("LND needs mainnet or testnet for payments.".into()),
        };
        if front.config().network != expected {
            return Err("The two receiving wallets use different Bitcoin networks.".into());
        }
        // Exact resource identity, never a path prefix shared with other sellers.
        // Keep this node known after rollback to observe its original invoices.
        front.with_resource_receiver(
            "x:openagents-agent-credit",
            self.receiver()?,
            self.backend == Backend::Lnd,
        )
    }
}
struct LndReceiver {
    settings: ReceiveSettings,
    config: Config,
    wallet: Mutex<Option<Wallet<Client>>>,
}
impl LndReceiver {
    fn with_wallet<T>(
        &self,
        call: impl FnOnce(&Wallet<Client>) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut wallet = self
            .wallet
            .lock()
            .map_err(|_| "The receiving wallet is unavailable.")?;
        if wallet.is_none() {
            let credential =
                Macaroon::from_file(Role::Invoice, 101, &self.settings.invoice_macaroon)
                    .map_err(|e| e.to_string())?;
            let connected = Wallet::connect_receiver(
                self.config.clone(),
                credential,
                self.settings.node_id.clone(),
                self.settings.network.clone(),
            )
            .map_err(|e| e.to_string())?;
            *wallet = Some(connected);
        }
        call(wallet.as_ref().unwrap())
    }
}
impl Receiver for LndReceiver {
    fn pay_to(&self) -> String {
        self.settings.node_id.clone()
    }
    fn invoice(&self, amount: u64, hash: [u8; 32], expiry: u32) -> Result<String, String> {
        self.with_wallet(|wallet| {
            wallet
                .receive_hinted(amount, hash, expiry)
                .map(|i| i.bolt11)
                .map_err(|e| e.to_string())
        })
    }
    fn received_msat(&self, hash: [u8; 32]) -> Result<Option<u64>, String> {
        self.with_wallet(|wallet| wallet.received_msat(hash).map_err(|e| e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rollback_settings_do_not_open_a_network_connection_or_read_a_credential() {
        let settings: ReceiveSettings = toml::from_str(
            r#"
backend = "mdk"
endpoint = "https://127.0.0.1:1"
certificate_sha256 = "1111111111111111111111111111111111111111111111111111111111111111"
node_id = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
invoice_macaroon = "/absent/credential"
readonly_macaroon = "/absent/readonly"
"#,
        )
        .unwrap();
        assert_eq!(settings.backend, Backend::Mdk);
        let receiver = settings.receiver().unwrap();
        assert_eq!(receiver.pay_to(), settings.node_id);
        assert!(receiver.wallet.lock().unwrap().is_none());
    }
}
