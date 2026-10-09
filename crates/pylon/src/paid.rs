//! Paid pylon jobs (P3 of `docs/compute/verse-compute.md`): the direct
//! per-job Lightning payment between a buyer and a pylon.
//!
//! A priced pylon answers an admitted request with `payment-required`
//! feedback carrying a BOLT11 invoice from its [`Invoicer`], waits for that
//! invoice to settle, and only then runs the job. The buyer checks the
//! terms against its own ceiling and network, pays with its [`Payer`], and
//! puts the payment hash and the preimage in its `3201` receipt, which a
//! reader verifies (`nostr::pylon::Receipt::validate`).
//!
//! Test networks first: [`TestLightning`] is an in-memory Lightning
//! network of worthless sats that serves both sides in fixtures, and it
//! refuses `bitcoin`. A pylon or a buyer on `bitcoin` needs a real wallet
//! adapter and the owner's standing grant ([`Grant`]); neither ships here.

use std::collections::HashMap;
use std::sync::Mutex;

use nostr::pylon::{Payment, sha256_hex};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The feedback status a priced pylon sends before it runs a job.
pub const PAYMENT_REQUIRED: &str = "payment-required";
/// The settlement profile a priced pylon advertises and its receipts name.
pub const PROFILE: &str = "lightning-bolt11";

/// A Lightning network.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Network {
    Bitcoin,
    Testnet,
    Signet,
    Regtest,
}

impl Network {
    /// The receipt's `network` name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bitcoin => "bitcoin",
            Self::Testnet => "testnet",
            Self::Signet => "signet",
            Self::Regtest => "regtest",
        }
    }

    /// Parse a receipt's `network` name.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "bitcoin" => Self::Bitcoin,
            "testnet" => Self::Testnet,
            "signet" => Self::Signet,
            "regtest" => Self::Regtest,
            _ => return None,
        })
    }

    /// Worthless sats: everything but `bitcoin`. The world marks these
    /// amounts **TEST** and never sums them with mainnet.
    #[must_use]
    pub fn is_test(self) -> bool {
        self != Self::Bitcoin
    }
}

/// A pylon's posted price per job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Price {
    pub msat: u64,
    pub network: Network,
}

/// The owner's standing grant for mainnet: nothing pays on `bitcoin`
/// without one (`INVARIANTS.md`, Agent spending), and every payment stays
/// under both ceilings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grant {
    pub per_payment_msat: u64,
    pub daily_msat: u64,
}

/// An invoice one side issued.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Invoice {
    pub bolt11: String,
    /// 64 lowercase hex digits.
    pub payment_hash: String,
    pub amount_msat: u64,
}

/// The pylon's side: issue an invoice for one job and learn when it
/// settled.
pub trait Invoicer: Send + Sync {
    fn network(&self) -> Network;
    /// An invoice for exactly `amount_msat`.
    ///
    /// # Errors
    ///
    /// When the wallet cannot issue one.
    fn invoice(&self, amount_msat: u64, memo: &str) -> Result<Invoice, String>;
    /// Whether the invoice with this hash has been paid.
    fn settled(&self, payment_hash: &str) -> bool;
}

/// The buyer's side: pay an invoice and return its preimage.
pub trait Payer: Send + Sync {
    fn network(&self) -> Network;
    /// Pay `invoice` and return the 64-hex preimage.
    ///
    /// # Errors
    ///
    /// When the payment did not settle; nothing is owed then.
    fn pay(&self, invoice: &Invoice) -> Result<String, String>;
}

/// The terms a priced pylon sends as feedback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Terms {
    pub network: Network,
    pub invoice: Invoice,
}

/// The `payment-required` feedback body for `terms`.
#[must_use]
pub fn terms_body(version: u64, terms: &Terms) -> Value {
    json!({
        "v": version,
        "requires": [],
        "type": "status",
        "status": PAYMENT_REQUIRED,
        "profile": PROFILE,
        "network": terms.network.as_str(),
        "amount_msat": terms.invoice.amount_msat,
        "bolt11": terms.invoice.bolt11,
        "payment_hash": terms.invoice.payment_hash,
    })
}

/// Read `payment-required` feedback; `None` when the body is other
/// feedback.
///
/// # Errors
///
/// When it is payment feedback that does not parse.
pub fn parse_terms(body: &Value) -> Result<Option<Terms>, String> {
    if body["status"] != PAYMENT_REQUIRED {
        return Ok(None);
    }
    if body["profile"] != PROFILE {
        return Err("unknown payment profile".into());
    }
    let network = body["network"]
        .as_str()
        .and_then(Network::parse)
        .ok_or("unknown payment network")?;
    let amount_msat = body["amount_msat"].as_u64().ok_or("amount is missing")?;
    let bolt11 = body["bolt11"].as_str().ok_or("invoice is missing")?;
    let payment_hash = body["payment_hash"]
        .as_str()
        .filter(|h| is_hex64(h))
        .ok_or("payment hash is not 64 hex digits")?;
    if bolt11.is_empty() || bolt11.len() > 4_096 {
        return Err("invoice is empty or too long".into());
    }
    Ok(Some(Terms {
        network,
        invoice: Invoice {
            bolt11: bolt11.into(),
            payment_hash: payment_hash.into(),
            amount_msat,
        },
    }))
}

/// Check terms against the buyer's wallet and ceiling, pay them, and
/// return the receipt's `payment`.
///
/// # Errors
///
/// Terms on another network, over the ceiling, or a payment that failed or
/// returned a preimage that does not hash to the invoice's payment hash.
pub fn pay(payer: &dyn Payer, terms: &Terms, max_msat: u64) -> Result<Payment, String> {
    if terms.network != payer.network() {
        return Err(format!(
            "the pylon asks for {} sats; this wallet is on {}",
            terms.network.as_str(),
            payer.network().as_str()
        ));
    }
    if terms.invoice.amount_msat > max_msat {
        return Err(format!(
            "the pylon asks {} msat, over this buyer's {max_msat} msat ceiling",
            terms.invoice.amount_msat
        ));
    }
    let preimage = payer.pay(&terms.invoice)?;
    if !preimage_matches(&preimage, &terms.invoice.payment_hash) {
        return Err("the wallet returned a preimage that does not match".into());
    }
    Ok(Payment {
        profile: PROFILE.into(),
        network: terms.network.as_str().into(),
        amount_msat: terms.invoice.amount_msat,
        payment_hash: terms.invoice.payment_hash.clone(),
        preimage,
    })
}

/// Whether `preimage` (64 hex) hashes to `payment_hash`.
#[must_use]
pub fn preimage_matches(preimage: &str, payment_hash: &str) -> bool {
    if !is_hex64(preimage) {
        return false;
    }
    let raw: Vec<u8> = (0..32)
        .filter_map(|i| u8::from_str_radix(&preimage[2 * i..2 * i + 2], 16).ok())
        .collect();
    raw.len() == 32 && sha256_hex(&raw) == payment_hash
}

fn is_hex64(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

struct Pending {
    preimage: String,
    amount_msat: u64,
    paid: bool,
}

/// An in-memory Lightning network of worthless sats for fixtures: it
/// issues invoices (the pylon's [`Invoicer`]) and pays them (the buyer's
/// [`Payer`]), so a fixture's receipts carry real preimages. It never
/// touches a wallet, and it refuses `bitcoin`.
pub struct TestLightning {
    network: Network,
    invoices: Mutex<HashMap<String, Pending>>,
    /// The bolt11 text's hash, by bolt11.
    by_bolt11: Mutex<HashMap<String, String>>,
}

impl TestLightning {
    /// A test network.
    ///
    /// # Errors
    ///
    /// For `bitcoin`.
    pub fn new(network: Network) -> Result<Self, String> {
        if !network.is_test() {
            return Err("TestLightning never runs on bitcoin".into());
        }
        Ok(Self {
            network,
            invoices: Mutex::new(HashMap::new()),
            by_bolt11: Mutex::new(HashMap::new()),
        })
    }

    /// How many invoices were paid, and their total.
    #[must_use]
    pub fn paid(&self) -> (usize, u64) {
        let invoices = self.invoices.lock().unwrap_or_else(|e| e.into_inner());
        invoices
            .values()
            .filter(|p| p.paid)
            .fold((0, 0), |(n, sum), p| (n + 1, sum + p.amount_msat))
    }
}

impl Invoicer for TestLightning {
    fn network(&self) -> Network {
        self.network
    }

    fn invoice(&self, amount_msat: u64, memo: &str) -> Result<Invoice, String> {
        let raw: [u8; 32] = secp256k1::rand::random();
        let preimage: String = raw.iter().map(|b| format!("{b:02x}")).collect();
        let payment_hash = sha256_hex(&raw);
        let prefix = match self.network {
            Network::Testnet => "lntb",
            Network::Signet => "lntbs",
            _ => "lnbcrt",
        };
        let bolt11 = format!(
            "{prefix}{}n1test{}{}",
            amount_msat / 100,
            &payment_hash[..16],
            sha256_hex(memo.as_bytes()).get(..8).unwrap_or_default()
        );
        self.invoices
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                payment_hash.clone(),
                Pending {
                    preimage,
                    amount_msat,
                    paid: false,
                },
            );
        self.by_bolt11
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(bolt11.clone(), payment_hash.clone());
        Ok(Invoice {
            bolt11,
            payment_hash,
            amount_msat,
        })
    }

    fn settled(&self, payment_hash: &str) -> bool {
        self.invoices
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(payment_hash)
            .is_some_and(|p| p.paid)
    }
}

impl Payer for TestLightning {
    fn network(&self) -> Network {
        self.network
    }

    fn pay(&self, invoice: &Invoice) -> Result<String, String> {
        let hash = self
            .by_bolt11
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&invoice.bolt11)
            .cloned()
            .ok_or("no route: the invoice is not on this test network")?;
        let mut invoices = self.invoices.lock().unwrap_or_else(|e| e.into_inner());
        let pending = invoices.get_mut(&hash).ok_or("unknown invoice")?;
        if pending.amount_msat != invoice.amount_msat || hash != invoice.payment_hash {
            return Err("the invoice's terms differ from the network's".into());
        }
        pending.paid = true;
        Ok(pending.preimage.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lightning_pays_with_a_matching_preimage_and_refuses_mainnet() {
        assert!(TestLightning::new(Network::Bitcoin).is_err());
        let net = TestLightning::new(Network::Regtest).unwrap();
        let invoice = Invoicer::invoice(&net, 2_000, "job").unwrap();
        assert!(!net.settled(&invoice.payment_hash));
        let terms = Terms {
            network: Network::Regtest,
            invoice: invoice.clone(),
        };
        let parsed = parse_terms(&terms_body(1, &terms)).unwrap().unwrap();
        assert_eq!(parsed, terms);
        assert!(pay(&net, &terms, 1_999).is_err());
        let signet = TestLightning::new(Network::Signet).unwrap();
        assert!(pay(&signet, &terms, 5_000).is_err());
        let payment = pay(&net, &terms, 2_000).unwrap();
        assert!(net.settled(&invoice.payment_hash));
        assert!(preimage_matches(&payment.preimage, &payment.payment_hash));
        assert_eq!(net.paid(), (1, 2_000));
        assert!(!preimage_matches(&"0".repeat(64), &payment.payment_hash));
        assert_eq!(parse_terms(&json!({"status": "processing"})).unwrap(), None);
    }
}
