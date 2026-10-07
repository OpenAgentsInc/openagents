//! One handle for a command that needs the node: the resident node when a
//! `x402 node serve` answers under the wallet home, else a node this process
//! opens and stops itself.

use std::path::Path;
use std::time::Duration;

use crate::LightningWallet;
use crate::config::WalletConfig;
use crate::ldk::LdkWallet;
use crate::model::{Balance, Channel, IssuedInvoice, PaymentRecord, Proof, WalletError};
use crate::resident::RemoteWallet;

pub enum Opened {
    Resident(RemoteWallet),
    Local(Box<LdkWallet>),
}

impl Opened {
    /// Use the resident when one answers, else open the store here.
    pub fn open(home: &Path, config: &WalletConfig, mnemonic: &str) -> Result<Self, WalletError> {
        if let Some(remote) = RemoteWallet::probe(home) {
            return Ok(Self::Resident(remote));
        }
        Ok(Self::Local(Box::new(LdkWallet::open(
            home, config, mnemonic,
        )?)))
    }

    pub fn is_resident(&self) -> bool {
        matches!(self, Self::Resident(_))
    }

    /// Stop a node this process opened; a resident keeps running.
    pub fn buy_channel(
        &self,
        lsp_balance_sat: u64,
        client_balance_sat: u64,
        channel_expiry_blocks: u32,
        announce: bool,
    ) -> Result<serde_json::Value, WalletError> {
        match self {
            Self::Resident(wallet) => wallet.buy_channel(
                lsp_balance_sat,
                client_balance_sat,
                channel_expiry_blocks,
                announce,
            ),
            Self::Local(wallet) => wallet.buy_channel(
                lsp_balance_sat,
                client_balance_sat,
                channel_expiry_blocks,
                announce,
            ),
        }
    }

    pub fn channel_order(&self, order_id: &str) -> Result<serde_json::Value, WalletError> {
        match self {
            Self::Resident(wallet) => wallet.channel_order(order_id),
            Self::Local(wallet) => wallet.channel_order(order_id),
        }
    }

    pub fn send_onchain(&self, address: &str, amount_sats: u64) -> Result<String, WalletError> {
        match self {
            Self::Resident(wallet) => wallet.send_onchain(address, amount_sats),
            Self::Local(wallet) => wallet.send_onchain(address, amount_sats),
        }
    }

    pub fn stop(&self) -> Result<(), WalletError> {
        match self {
            Self::Resident(_) => Ok(()),
            Self::Local(wallet) => wallet.stop(),
        }
    }

    /// The node's status, with a `resident` member that names the serving
    /// process or is null.
    pub fn status(&self) -> Result<serde_json::Value, WalletError> {
        match self {
            Self::Resident(remote) => {
                let resident = remote.status()?;
                let mut node = resident.node;
                node["resident"] = serde_json::json!({
                    "pid": resident.pid,
                    "started_at": resident.started_at,
                    "uptime_secs": resident.uptime_secs,
                    "socket": remote.path().display().to_string(),
                });
                Ok(node)
            }
            Self::Local(wallet) => {
                let mut node = wallet.status();
                node["resident"] = serde_json::Value::Null;
                Ok(node)
            }
        }
    }

    /// The next queued node event. A resident drains its own events and
    /// prints them, so through a resident there are none to take.
    pub fn next_event(&self) -> Result<Option<serde_json::Value>, WalletError> {
        match self {
            Self::Resident(_) => Ok(None),
            Self::Local(wallet) => wallet.next_event(),
        }
    }
}

impl LightningWallet for Opened {
    fn node_id(&self) -> String {
        match self {
            Self::Resident(w) => w.node_id(),
            Self::Local(w) => w.node_id(),
        }
    }

    fn receive_exact(
        &self,
        amount_msat: u64,
        request_hash: [u8; 32],
        expiry_secs: u32,
    ) -> Result<IssuedInvoice, WalletError> {
        match self {
            Self::Resident(w) => w.receive_exact(amount_msat, request_hash, expiry_secs),
            Self::Local(w) => w.receive_exact(amount_msat, request_hash, expiry_secs),
        }
    }

    fn receive_exact_from_node(
        &self,
        expected_node: &str,
        amount_msat: u64,
        request_hash: [u8; 32],
        expiry_secs: u32,
    ) -> Result<IssuedInvoice, WalletError> {
        match self {
            Self::Resident(w) => {
                w.receive_exact_from_node(expected_node, amount_msat, request_hash, expiry_secs)
            }
            Self::Local(w) => {
                w.receive_exact_from_node(expected_node, amount_msat, request_hash, expiry_secs)
            }
        }
    }

    fn pay(&self, invoice: &str, max_fee_msat: u64, wait: Duration) -> Result<Proof, WalletError> {
        match self {
            Self::Resident(w) => w.pay(invoice, max_fee_msat, wait),
            Self::Local(w) => w.pay(invoice, max_fee_msat, wait),
        }
    }

    fn pay_from_node(
        &self,
        expected_node: &str,
        invoice: &str,
        max_fee_msat: u64,
        wait: Duration,
    ) -> Result<Proof, WalletError> {
        match self {
            Self::Resident(w) => w.pay_from_node(expected_node, invoice, max_fee_msat, wait),
            Self::Local(w) => w.pay_from_node(expected_node, invoice, max_fee_msat, wait),
        }
    }

    fn lookup(&self, payment_hash: [u8; 32]) -> Result<Option<PaymentRecord>, WalletError> {
        match self {
            Self::Resident(w) => w.lookup(payment_hash),
            Self::Local(w) => w.lookup(payment_hash),
        }
    }

    fn lookup_from_node(
        &self,
        expected_node: &str,
        payment_hash: [u8; 32],
    ) -> Result<Option<PaymentRecord>, WalletError> {
        match self {
            Self::Resident(w) => w.lookup_from_node(expected_node, payment_hash),
            Self::Local(w) => w.lookup_from_node(expected_node, payment_hash),
        }
    }

    fn payments(&self) -> Result<Vec<PaymentRecord>, WalletError> {
        match self {
            Self::Resident(w) => w.payments(),
            Self::Local(w) => w.payments(),
        }
    }

    fn balance(&self) -> Result<Balance, WalletError> {
        match self {
            Self::Resident(w) => w.balance(),
            Self::Local(w) => w.balance(),
        }
    }

    fn channels(&self) -> Result<Vec<Channel>, WalletError> {
        match self {
            Self::Resident(w) => w.channels(),
            Self::Local(w) => w.channels(),
        }
    }

    fn funding_address(&self) -> Result<String, WalletError> {
        match self {
            Self::Resident(w) => w.funding_address(),
            Self::Local(w) => w.funding_address(),
        }
    }

    fn open_channel(
        &self,
        node_id: &str,
        address: &str,
        amount_sats: u64,
        announce: bool,
    ) -> Result<String, WalletError> {
        match self {
            Self::Resident(w) => w.open_channel(node_id, address, amount_sats, announce),
            Self::Local(w) => w.open_channel(node_id, address, amount_sats, announce),
        }
    }

    fn close_channel(
        &self,
        user_channel_id: &str,
        counterparty: &str,
        force: bool,
    ) -> Result<(), WalletError> {
        match self {
            Self::Resident(w) => w.close_channel(user_channel_id, counterparty, force),
            Self::Local(w) => w.close_channel(user_channel_id, counterparty, force),
        }
    }
}
