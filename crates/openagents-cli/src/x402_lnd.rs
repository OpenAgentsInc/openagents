//! Buying an x402 call from an LND node over gRPC (`--pay-with lnd`).
//!
//! The settings file names the node's gRPC address, its TLS certificate's
//! SHA-256 fingerprint, its identity key, the network, and a macaroon file
//! that allows sending (`offchain:write`). The send goes through
//! `SendPaymentV2` once, with the fee cap as LND's fee limit; an uncertain
//! outcome is never resent, and LND itself refuses a second send for the
//! same payment hash.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use lnd_sdk::{
    Client,
    macaroon::{Macaroon, Role},
    service::Wallet,
    transport::Config,
};
use openagents_wallet::{LightningWallet, Proof, WalletError};
use serde::Deserialize;
use sha2::{Digest, Sha256};

/// The settings file named by `--lnd FILE` for this process.
static NAMED: OnceLock<PathBuf> = OnceLock::new();

/// Remember `--lnd FILE` for the payment this process makes.
pub(crate) fn name_settings(path: &str) {
    let _ = NAMED.set(PathBuf::from(path));
}

/// `--lnd FILE`, else `$OPENAGENTS_X402_LND`, else `lnd.toml` beside the
/// x402 policy.
pub(crate) fn settings_path() -> PathBuf {
    if let Some(path) = NAMED.get() {
        return path.clone();
    }
    match std::env::var_os("OPENAGENTS_X402_LND") {
        Some(path) => PathBuf::from(path),
        None => crate::x402::x402_home().join("lnd.toml"),
    }
}

/// How to reach the node that pays.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SendSettings {
    /// `https://HOST:10009`.
    pub endpoint: String,
    /// SHA-256 of the node's TLS certificate in DER form, 64 hex digits:
    /// `openssl x509 -in tls.cert -outform DER | shasum -a 256`.
    pub certificate_sha256: String,
    /// The node's identity key (`lncli getinfo`), checked on connect.
    pub node_id: String,
    /// A macaroon allowing `offchain:write` (owner-only file, mode 0600).
    pub macaroon: PathBuf,
    /// `mainnet` or `testnet`.
    #[serde(default = "mainnet")]
    pub network: String,
}

fn mainnet() -> String {
    "mainnet".into()
}

impl SendSettings {
    pub(crate) fn parse(text: &str, base: &Path) -> Result<Self, String> {
        let mut settings: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        if settings.macaroon.is_relative() {
            settings.macaroon = base.join(&settings.macaroon);
        }
        settings.check()?;
        Ok(settings)
    }

    fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| {
            format!(
                "{}: {e}; --pay-with lnd needs a settings file (endpoint, certificate_sha256, node_id, macaroon, network); nothing was paid",
                path.display()
            )
        })?;
        let base = path.parent().unwrap_or(Path::new("."));
        Self::parse(&text, base).map_err(|e| format!("{}: {e}; nothing was paid", path.display()))
    }

    fn check(&self) -> Result<(), String> {
        if !self.endpoint.starts_with("https://") {
            return Err("the LND endpoint must be https://HOST:PORT".into());
        }
        openagents_wallet::parse_hash32(&self.certificate_sha256)
            .map_err(|_| "certificate_sha256 must be 64 hex digits")?;
        let _: secp256k1::PublicKey = self
            .node_id
            .parse()
            .map_err(|_| "node_id is not a node identity key")?;
        self.x402_network()?;
        Ok(())
    }

    /// The x402 network this node pays on.
    pub(crate) fn x402_network(&self) -> Result<&'static str, String> {
        match self.network.as_str() {
            "mainnet" => Ok(nostr::x402::MAINNET),
            "testnet" => Ok(nostr::x402::TESTNET),
            _ => Err("network must be mainnet or testnet; x402 admits no other".into()),
        }
    }

    fn connect(&self) -> Result<Wallet<Client>, String> {
        let pin = openagents_wallet::parse_hash32(&self.certificate_sha256)
            .map_err(|_| "certificate_sha256 must be 64 hex digits".to_string())?;
        let macaroon = Macaroon::from_file(Role::Payout, 0, &self.macaroon).map_err(|_| {
            format!(
                "{}: the macaroon can't be read; it must be a file only you can read (chmod 600); nothing was paid",
                self.macaroon.display()
            )
        })?;
        Wallet::connect_receiver(
            Config::new(self.endpoint.clone(), pin),
            macaroon,
            self.node_id.clone(),
            self.network.clone(),
        )
        .map_err(|e| {
            format!(
                "could not reach the LND node {} at {}: {e}; nothing was paid",
                self.node_id, self.endpoint
            )
        })
    }
}

/// Pay `bolt11` for an x402 challenge on `network` from the LND node the
/// settings name, within the fee cap and the wait.
pub(crate) fn pay(
    bolt11: &str,
    network: &str,
    max_fee_msat: u64,
    wait: u64,
) -> Result<Proof, String> {
    let settings = SendSettings::load(&settings_path())?;
    if settings.x402_network()? != network {
        return Err(format!(
            "the invoice is on {network} but this LND node is on {}; nothing was paid",
            settings.network
        ));
    }
    let wallet = settings.connect()?;
    pay_from(&wallet, bolt11, max_fee_msat, wait)
}

/// Send once and check the proof against the invoice itself.
pub(crate) fn pay_from(
    wallet: &impl LightningWallet,
    bolt11: &str,
    max_fee_msat: u64,
    wait: u64,
) -> Result<Proof, String> {
    let invoice = nostr::x402::decode_invoice(bolt11)
        .map_err(|_| "the invoice can't be read; nothing was paid".to_string())?;
    let hash = to_hex(&invoice.payment_hash());
    let proof = match wallet.pay(bolt11, max_fee_msat, Duration::from_secs(wait.max(1))) {
        Ok(proof) => proof,
        Err(WalletError::Pending { waited_secs, .. }) => {
            return Err(format!(
                "payment {hash} is still in flight on the LND node after {waited_secs}s; check it with `lncli trackpayment {hash}`. Don't repeat the call: a new challenge would be paid again"
            ));
        }
        Err(WalletError::Failed { reason, .. }) => {
            return Err(format!("payment {hash} failed: {reason}; nothing was sent"));
        }
        Err(error) => return Err(format!("{error}; payment {hash}")),
    };
    let opens = openagents_wallet::parse_hash32(&proof.preimage)
        .is_ok_and(|preimage| Sha256::digest(preimage).as_slice() == invoice.payment_hash());
    if proof.payment_hash != hash || !opens {
        return Err(format!(
            "the LND node reported payment {hash} without a matching proof; check `lncli trackpayment {hash}` before repeating the call"
        ));
    }
    if proof.amount_msat != invoice.amount_msat() || proof.fee_msat > max_fee_msat {
        return Err(format!(
            "the LND node paid {hash} with amount {} msat and fee {} msat, outside the invoice's {} msat and the {max_fee_msat} msat fee cap",
            proof.amount_msat,
            proof.fee_msat,
            invoice.amount_msat()
        ));
    }
    Ok(proof)
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lnd_sdk::fake::Fake;
    use std::sync::{Arc, atomic::Ordering};

    const NODE: &str = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";

    fn settings(network: &str) -> String {
        format!(
            r#"
endpoint = "https://127.0.0.1:1"
certificate_sha256 = "1111111111111111111111111111111111111111111111111111111111111111"
node_id = "{NODE}"
macaroon = "pay.macaroon"
network = "{network}"
"#
        )
    }

    #[test]
    fn settings_anchor_the_macaroon_and_admit_only_x402_networks() {
        let parsed = SendSettings::parse(&settings("mainnet"), Path::new("/etc/x")).unwrap();
        assert_eq!(parsed.macaroon, PathBuf::from("/etc/x/pay.macaroon"));
        assert_eq!(parsed.x402_network().unwrap(), nostr::x402::MAINNET);
        let testnet = SendSettings::parse(&settings("testnet"), Path::new("/")).unwrap();
        assert_eq!(testnet.x402_network().unwrap(), nostr::x402::TESTNET);
        assert!(SendSettings::parse(&settings("regtest"), Path::new("/")).is_err());
        let plain = settings("mainnet").replace("https://", "http://");
        assert!(SendSettings::parse(&plain, Path::new("/")).is_err());
    }

    /// A signed x402-shaped testnet invoice (description hash, never an
    /// inline description) for `preimage`, with its hash.
    fn invoice(preimage: [u8; 32]) -> (String, [u8; 32]) {
        use nostr::x402::test_invoice::{number, signed_at, tag, words};
        let hash: [u8; 32] = Sha256::digest(preimage).into();
        let mut fields = tag(1, &words(&hash));
        fields.extend(tag(16, &words(&[2; 32])));
        fields.extend(tag(23, &words(&[4; 32])));
        fields.extend(tag(6, &number(600)));
        // var_onion_optin (8) and payment_secret (14), both required.
        fields.extend(tag(5, &number((1 << 8) | (1 << 14))));
        let bolt11 = signed_at(
            "lntb21010p",
            fields,
            false,
            false,
            openagents_x402::unix_now(),
        );
        (bolt11, hash)
    }

    fn proof(bolt11: &str, preimage: [u8; 32], hash: [u8; 32], fee: u64) -> Proof {
        Proof {
            payment_hash: to_hex(&hash),
            preimage: to_hex(&preimage),
            amount_msat: 2_101,
            fee_msat: fee,
            bolt11: bolt11.into(),
        }
    }

    #[test]
    fn one_send_returns_the_checked_preimage() {
        let (bolt11, hash) = invoice([7; 32]);
        let fake = Arc::new(Fake {
            node_id: NODE.into(),
            ..Fake::default()
        });
        *fake.outcome.lock().unwrap() = Some(Ok(proof(&bolt11, [7; 32], hash, 3)));
        let wallet = Wallet::new(fake.clone()).unwrap();
        let paid = pay_from(&wallet, &bolt11, 10, 5).unwrap();
        assert_eq!(paid.preimage, to_hex(&[7; 32]));
        assert_eq!(fake.sends.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn an_uncertain_send_is_reported_never_resent() {
        let (bolt11, _) = invoice([8; 32]);
        let fake = Arc::new(Fake::default());
        *fake.outcome.lock().unwrap() = Some(Err(lnd_sdk::Error::InFlight));
        let wallet = Wallet::new(fake.clone()).unwrap();
        let error = pay_from(&wallet, &bolt11, 10, 5).unwrap_err();
        assert!(error.contains("still in flight"), "{error}");
        assert_eq!(fake.sends.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_proof_that_does_not_open_the_hash_or_breaks_the_cap_is_refused() {
        let (bolt11, hash) = invoice([9; 32]);
        for (outcome, words) in [
            (proof(&bolt11, [1; 32], hash, 0), "matching proof"),
            (proof(&bolt11, [9; 32], hash, 11), "fee cap"),
        ] {
            let fake = Arc::new(Fake::default());
            *fake.outcome.lock().unwrap() = Some(Ok(outcome));
            let wallet = Wallet::new(fake).unwrap();
            let error = pay_from(&wallet, &bolt11, 10, 5).unwrap_err();
            assert!(error.contains(words), "{error}");
        }
    }
}
