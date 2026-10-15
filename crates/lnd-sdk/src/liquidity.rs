//! Explicit-budget LSPS1 purchases and restart-safe operator actions.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Zeus,
    Megalith,
}
impl Provider {
    pub fn base(self) -> &'static str {
        match self {
            Self::Zeus => "https://lsps1.zeuslsp.com/api/v1",
            Self::Megalith => "https://megalithic.me/api/lsps1/v1",
        }
    }
    pub fn peer(self) -> (&'static str, &'static str) {
        match self {
            Self::Zeus => (
                "031b301307574bbe9b9ac7b79cbe1700e31e544513eae0b5d7497483083f99e581",
                "45.79.192.236:9735",
            ),
            Self::Megalith => (
                "038a9e56512ec98da2b5789761f7af8f280baf98a09282360cd6ff1381b5e889bf",
                "64.23.162.51:9735",
            ),
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Budget {
    pub mainnet_approved: bool,
    pub inbound_sats: u64,
    pub channel_sats: u64,
    pub reserve_sats: u64,
    pub maximum_lsp_fee_sats: u64,
    pub maximum_routing_fee_msat: u64,
    pub opening_sat_per_vbyte: u64,
    /// Conservative allowance, checked before opening; actual fee is recorded separately.
    pub opening_allowance_sats: u64,
    pub expiry_blocks: u32,
}
impl Budget {
    pub fn authorize(&self, network: &str) -> Result<()> {
        if network == "mainnet" && !self.mainnet_approved {
            return Err(Error::PermissionDenied);
        }
        if !(100_000..=500_000).contains(&self.reserve_sats)
            || self.inbound_sats == 0
            || self.channel_sats == 0
            || self.maximum_lsp_fee_sats == 0
            || self.opening_sat_per_vbyte == 0
            || self.opening_allowance_sats == 0
            || self.expiry_blocks == 0
        {
            return Err(Error::Invalid("liquidity budget"));
        }
        Ok(())
    }
    pub fn check_open(&self, spendable: u64) -> Result<()> {
        let required = self
            .channel_sats
            .checked_add(self.reserve_sats)
            .and_then(|n| n.checked_add(self.opening_allowance_sats))
            .ok_or(Error::Invalid("liquidity budget"))?;
        if spendable < required {
            return Err(Error::InsufficientBalance);
        }
        Ok(())
    }
    pub fn terms(&self, info: &Value, node: &str) -> Result<Value> {
        let options = info.get("options").unwrap_or(info);
        for (min, max) in [
            ("min_channel_balance_sat", "max_channel_balance_sat"),
            ("min_initial_lsp_balance_sat", "max_initial_lsp_balance_sat"),
        ] {
            if self.inbound_sats < sat(options, min)? || self.inbound_sats > sat(options, max)? {
                return Err(Error::Invalid("LSP capacity range"));
            }
        }
        if sat(options, "min_initial_client_balance_sat")? != 0
            || self.expiry_blocks > number(options, "max_channel_expiry_blocks")?
        {
            return Err(Error::Invalid("LSP channel terms"));
        }
        Ok(
            json!({"lsp_balance_sat": self.inbound_sats.to_string(), "client_balance_sat": "0",
            "required_channel_confirmations": number(options, "min_required_channel_confirmations")?.max(3),
            "funding_confirms_within_blocks": number(options, "min_funding_confirms_within_blocks")?,
            "channel_expiry_blocks": self.expiry_blocks, "announce_channel": true,
            "public_key": node, "token": ""}),
        )
    }
}
fn number(v: &Value, key: &str) -> Result<u32> {
    v[key]
        .as_u64()
        .and_then(|n| n.try_into().ok())
        .ok_or(Error::Invalid("LSP integer"))
}
fn sat(v: &Value, key: &str) -> Result<u64> {
    v[key]
        .as_str()
        .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|s| s.parse().ok())
        .ok_or(Error::Invalid("LSP satoshi amount"))
}
/// Validate the signed payment and echoed terms before any money is sent.
pub fn validate_order(
    order: &Value,
    terms: &Value,
    budget: &Budget,
    network: &str,
    peer: &str,
) -> Result<String> {
    let id = order["order_id"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 64)
        .ok_or(Error::Invalid("LSP order identity"))?;
    let _ = id;
    for key in [
        "lsp_balance_sat",
        "client_balance_sat",
        "required_channel_confirmations",
        "funding_confirms_within_blocks",
        "channel_expiry_blocks",
        "announce_channel",
    ] {
        if order[key] != terms[key] {
            return Err(Error::Invalid("LSP order terms"));
        }
    }
    if order["order_state"] != "CREATED" {
        return Err(Error::Invalid("LSP order state"));
    }
    let payment = &order["payment"]["bolt11"];
    if payment["state"] != "EXPECT_PAYMENT" {
        return Err(Error::InFlight);
    }
    let fee = sat(payment, "fee_total_sat")?;
    if fee == 0 || fee > budget.maximum_lsp_fee_sats || sat(payment, "order_total_sat")? != fee {
        return Err(Error::Invalid("LSP fee"));
    }
    let text = payment["invoice"]
        .as_str()
        .filter(|s| s.len() <= 2048)
        .ok_or(Error::Invalid("LSP invoice"))?;
    let invoice = crate::router::invoice(text).map_err(|_| Error::Invalid("LSP invoice"))?;
    let currency = match network {
        "mainnet" => lightning_invoice::Currency::Bitcoin,
        "regtest" => lightning_invoice::Currency::Regtest,
        "testnet" => lightning_invoice::Currency::BitcoinTestnet,
        _ => return Err(Error::Invalid("LSP network")),
    };
    if invoice.currency() != currency
        || invoice.is_expired()
        || invoice.amount_milli_satoshis() != fee.checked_mul(1000)
        || invoice.recover_payee_pub_key().to_string() != peer
    {
        return Err(Error::Invalid("LSP invoice terms"));
    }
    Ok(text.into())
}

pub async fn request(
    provider: Provider,
    method: &str,
    body: Option<&Value>,
    order_id: Option<&str>,
) -> Result<Value> {
    if !["get_info", "create_order", "get_order"].contains(&method) {
        return Err(Error::Invalid("LSP method"));
    }
    let client = reqwest::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|_| Error::Unavailable)?;
    let url = format!("{}/{method}", provider.base());
    // In particular, create_order is never retried: LSPS1 has no idempotency key.
    let mut request = if let Some(body) = body {
        client.post(url).json(body)
    } else {
        client.get(url)
    };
    if let Some(id) = order_id {
        request = request.query(&[("order_id", id)]);
    }
    let mut response = request.send().await.map_err(|_| Error::Unavailable)?;
    if !response.status().is_success() {
        return Err(Error::Refused);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| Error::Unavailable)? {
        if bytes.len() + chunk.len() > 65_536 {
            return Err(Error::Invalid("LSP response size"));
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| Error::Invalid("LSP response"))
}

/// One locked file per node/budget. Persist intent before dispatch; uncertain actions cannot repeat.
pub struct Journal {
    path: PathBuf,
    _lock: std::fs::File,
    pub entries: BTreeMap<String, Value>,
}
impl Journal {
    pub fn open(path: &Path) -> Result<Self> {
        let options = || {
            let mut o = std::fs::OpenOptions::new();
            o.read(true).write(true).create(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                o.mode(0o600);
            }
            o
        };
        let lock = options()
            .open(path.with_extension("lock"))
            .map_err(|_| Error::Checkpoint)?;
        fs2::FileExt::try_lock_exclusive(&lock).map_err(|_| Error::InFlight)?;
        let entries = match std::fs::read(path) {
            Ok(b) => serde_json::from_slice(&b).map_err(|_| Error::Checkpoint)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(_) => return Err(Error::Checkpoint),
        };
        Ok(Self {
            path: path.into(),
            _lock: lock,
            entries,
        })
    }
    pub fn save(&mut self, key: &str, value: Value) -> Result<()> {
        self.entries.insert(key.into(), value);
        let temporary = self.path.with_extension("new");
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary).map_err(|_| Error::Checkpoint)?;
        file.write_all(&serde_json::to_vec(&self.entries).map_err(|_| Error::Checkpoint)?)
            .and_then(|()| file.sync_all())
            .map_err(|_| Error::Checkpoint)?;
        std::fs::rename(&temporary, &self.path).map_err(|_| Error::Checkpoint)?;
        std::fs::File::open(
            self.path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new(".")),
        )
        .and_then(|f| f.sync_all())
        .map_err(|_| Error::Checkpoint)
    }
    pub fn begin(&mut self, key: &str, terms: Value) -> Result<()> {
        if self.entries.contains_key(key) {
            return Err(Error::InFlight);
        }
        self.save(key, json!({"state": "unknown", "terms": terms}))
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Costs {
    pub lsp_fee_sats: u64,
    pub opening_fee_sats: u64,
    pub routing_fee_msat: u64,
    pub received_msat: u64,
}
impl Costs {
    pub fn comparison(&self) -> Value {
        let cost = (u128::from(self.lsp_fee_sats) + u128::from(self.opening_fee_sats)) * 1000
            + u128::from(self.routing_fee_msat);
        json!({"lnd_cost_msat": cost, "mdk_two_percent_msat": u128::from(self.received_msat) * 2 / 100,
            "lnd_parts_per_million": if self.received_msat == 0 { None } else { Some(cost * 1_000_000 / u128::from(self.received_msat)) }})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signed_invoice_cannot_change_the_approved_fee_or_terms() {
        use bitcoin::hashes::Hash;
        let key = bitcoin::secp256k1::SecretKey::from_slice(&[42; 32]).unwrap();
        let invoice =
            lightning_invoice::InvoiceBuilder::new(lightning_invoice::Currency::BitcoinTestnet)
                .description("channel lease".into())
                .payment_hash(bitcoin::hashes::sha256::Hash::hash(&[7; 32]))
                .payment_secret(lightning_invoice::PaymentSecret([3; 32]))
                .amount_milli_satoshis(1_000_000)
                .current_timestamp()
                .min_final_cltv_expiry_delta(18)
                .build_signed(|m| {
                    bitcoin::secp256k1::Secp256k1::new().sign_ecdsa_recoverable(m, &key)
                })
                .unwrap();
        let b = Budget {
            mainnet_approved: false,
            inbound_sats: 100_000,
            channel_sats: 1_000_000,
            reserve_sats: 100_000,
            maximum_lsp_fee_sats: 1000,
            maximum_routing_fee_msat: 1000,
            opening_sat_per_vbyte: 2,
            opening_allowance_sats: 10_000,
            expiry_blocks: 12960,
        };
        let terms = json!({"lsp_balance_sat": "100000", "client_balance_sat": "0", "required_channel_confirmations": 3,
            "funding_confirms_within_blocks": 6, "channel_expiry_blocks": 12960, "announce_channel": true});
        let mut order = terms.clone();
        order["order_id"] = json!("one-order");
        order["order_state"] = json!("CREATED");
        order["payment"] = json!({"bolt11": {"state": "EXPECT_PAYMENT", "fee_total_sat": "1000", "order_total_sat": "1000", "invoice": invoice.to_string()}});
        let peer = invoice.recover_payee_pub_key().to_string();
        validate_order(&order, &terms, &b, "testnet", &peer).unwrap();
        order["payment"]["bolt11"]["order_total_sat"] = json!("1001");
        assert!(validate_order(&order, &terms, &b, "testnet", &peer).is_err());
        order["payment"]["bolt11"]["order_total_sat"] = json!("1000");
        order["required_channel_confirmations"] = json!(0);
        assert!(validate_order(&order, &terms, &b, "testnet", &peer).is_err());
    }
    #[test]
    fn uncertain_creation_and_open_cannot_repeat_after_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("actions.json");
        let mut first = Journal::open(&path).unwrap();
        first.begin("purchase", json!({"sats": 100_000})).unwrap();
        assert!(Journal::open(&path).is_err());
        drop(first);
        let mut second = Journal::open(&path).unwrap();
        assert_eq!(second.begin("purchase", json!({})), Err(Error::InFlight));
        assert_eq!(second.entries["purchase"]["state"], "unknown");
    }
    #[test]
    fn explicit_budget_preserves_reserve_and_comparison_handles_no_receipts() {
        let b = Budget {
            mainnet_approved: false,
            inbound_sats: 100_000,
            channel_sats: 1_000_000,
            reserve_sats: 100_000,
            maximum_lsp_fee_sats: 1000,
            maximum_routing_fee_msat: 1000,
            opening_sat_per_vbyte: 2,
            opening_allowance_sats: 10_000,
            expiry_blocks: 12960,
        };
        assert_eq!(b.authorize("mainnet"), Err(Error::PermissionDenied));
        b.authorize("regtest").unwrap();
        assert_eq!(b.check_open(1_109_999), Err(Error::InsufficientBalance));
        b.check_open(1_110_000).unwrap();
        assert_eq!(
            Costs {
                lsp_fee_sats: 10,
                opening_fee_sats: 20,
                routing_fee_msat: 3,
                received_msat: 0
            }
            .comparison()["lnd_parts_per_million"],
            Value::Null
        );
    }
}
