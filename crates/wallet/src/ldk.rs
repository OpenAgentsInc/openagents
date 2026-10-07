//! The `ldk-node` wallet. One process opens the node, runs the command, and
//! stops it; `serve` keeps it up so payers can reach it.

use std::path::Path;
use std::str::FromStr;
use std::time::{Duration, Instant};

use ldk_node::bitcoin::hashes::Hash as _;
use ldk_node::bitcoin::hashes::sha256;
use ldk_node::bitcoin::secp256k1::PublicKey;
use ldk_node::lightning::ln::msgs::SocketAddress;
use ldk_node::lightning::routing::router::RouteParametersConfig;
use ldk_node::lightning_invoice::{Bolt11Invoice, Bolt11InvoiceDescription, Sha256};
use ldk_node::lightning_liquidity::lsps1::msgs::LSPS1OrderId;
use ldk_node::liquidity::LSPS1OrderStatus;
use ldk_node::payment::{PaymentDetails, PaymentDirection as LdkDirection, PaymentKind};
use ldk_node::{Builder, Event, Node};

use crate::config::{LspProtocol, STORE_DIR, WalletConfig};
use crate::model::{
    Balance, Channel, IssuedInvoice, PaymentDirection, PaymentRecord, PaymentStatus, Proof,
    WalletError,
};
use crate::{LightningWallet, parse_hash32};

/// How long a send waits for the node to reconnect its channel peers
/// after a start.
const PEER_RECONNECT_WAIT: Duration = Duration::from_secs(30);

/// How often a wait or a resident dials stored peers that are not
/// connected; the node's own retry runs once a minute.
const PEER_DIAL_INTERVAL: Duration = Duration::from_secs(5);

pub struct LdkWallet {
    home: std::path::PathBuf,
    node: Node,
    network: crate::config::Network,
    /// The configured LSP's protocol, when one opens channels just in time.
    jit: Option<LspProtocol>,
}

/// A new 24-word BIP39 mnemonic.
pub fn generate_mnemonic() -> String {
    ldk_node::generate_entropy_mnemonic(None).to_string()
}

/// The BIP39 mnemonic for `entropy`, which a platform key store holds: 32
/// bytes give 24 words. The error never repeats the entropy.
pub fn mnemonic_from_entropy(entropy: &[u8]) -> Result<String, WalletError> {
    if entropy.len() != 32 {
        return Err(WalletError::Setup(
            "wallet entropy must be 32 bytes".to_string(),
        ));
    }
    ldk_node::bip39::Mnemonic::from_entropy(entropy)
        .map(|mnemonic| mnemonic.to_string())
        .map_err(|_| WalletError::Setup("wallet entropy is not usable".to_string()))
}

impl LdkWallet {
    /// Build and start the node from `config` with its store under
    /// `home/ldk`. The first start on a fresh store syncs against Esplora.
    pub fn open(home: &Path, config: &WalletConfig, mnemonic: &str) -> Result<Self, WalletError> {
        let wallet = Self::build(home, config, mnemonic)?;
        wallet
            .node
            .start()
            .map_err(|error| WalletError::Node(format!("start: {error}")))?;
        Ok(wallet)
    }

    /// The node id and the next `addresses` fresh funding addresses for
    /// `mnemonic`, without starting the node or reaching the chain source.
    /// This is what a restore compares against the wallet it came from: a
    /// restored store continues the address sequence where the original
    /// stopped; a seed alone starts it over.
    pub fn identity(
        home: &Path,
        config: &WalletConfig,
        mnemonic: &str,
        addresses: usize,
    ) -> Result<(String, Vec<String>), WalletError> {
        let wallet = Self::build(home, config, mnemonic)?;
        let addresses = (0..addresses)
            .map(|_| wallet.funding_address())
            .collect::<Result<Vec<_>, _>>()?;
        Ok((wallet.node_id(), addresses))
    }

    fn build(home: &Path, config: &WalletConfig, mnemonic: &str) -> Result<Self, WalletError> {
        let mnemonic = ldk_node::bip39::Mnemonic::from_str(mnemonic)
            .map_err(|error| WalletError::Setup(format!("seed: {error}")))?;
        let mut node_config = ldk_node::config::Config::default();
        if let Some(anchors) = node_config.anchor_channels_config.as_mut() {
            for peer in &config.trusted_peers {
                anchors.trusted_peers_no_reserve.push(pubkey(peer)?);
            }
            // A just-in-time LSP opens its channel toward a node that holds
            // no coins yet, so the anchor reserve check would refuse it.
            if let Some(lsp) = config
                .lsp
                .as_ref()
                .filter(|lsp| lsp.protocol.just_in_time())
            {
                anchors.trusted_peers_no_reserve.push(pubkey(&lsp.node_id)?);
            }
        }
        let mut builder = Builder::from_config(node_config);
        builder.set_network(network(config.network));
        builder.set_storage_dir_path(home.join(STORE_DIR).display().to_string());
        builder.set_entropy_bip39_mnemonic(mnemonic, None);
        builder.set_chain_source_esplora(config.esplora_url.clone(), None);
        if let Some(listen) = &config.listen {
            let address = socket(listen)?;
            builder
                .set_listening_addresses(vec![address])
                .map_err(|error| WalletError::Setup(format!("listen: {error}")))?;
        }
        if let Some(lsp) = &config.lsp {
            let (node_id, address) = (pubkey(&lsp.node_id)?, socket(&lsp.address)?);
            match lsp.protocol {
                LspProtocol::Lsps1 => {
                    builder.set_liquidity_source_lsps1(node_id, address, lsp.token.clone());
                }
                LspProtocol::Lsps2 => {
                    builder.set_liquidity_source_lsps2(node_id, address, lsp.token.clone());
                }
                LspProtocol::Lsps4 => {
                    builder.set_liquidity_source_lsps4(node_id, address, lsp.token.clone());
                }
            }
        }
        let node = builder
            .build()
            .map_err(|error| WalletError::Setup(format!("node: {error}")))?;
        Ok(Self {
            home: home.into(),
            node,
            network: config.network,
            jit: config
                .lsp
                .as_ref()
                .map(|lsp| lsp.protocol)
                .filter(|protocol| protocol.just_in_time()),
        })
    }

    /// Order an inbound channel from the configured LSPS1 provider: the LSP
    /// funds `lsp_balance_sat` on its side, `client_balance_sat` is pushed
    /// to us, and the channel stays open for `channel_expiry_blocks`. The
    /// order quotes a fee payable by Lightning or on chain; the channel
    /// opens once that is paid.
    pub fn buy_channel(
        &self,
        lsp_balance_sat: u64,
        client_balance_sat: u64,
        channel_expiry_blocks: u32,
        announce: bool,
    ) -> Result<serde_json::Value, WalletError> {
        crate::custody::refuse_raw(&self.home)?;
        let status = self
            .node
            .lsps1_liquidity()
            .request_channel(
                lsp_balance_sat,
                client_balance_sat,
                channel_expiry_blocks,
                announce,
            )
            .map_err(|error| WalletError::Node(format!("lsps1 order: {error}")))?;
        Ok(describe_order(&status))
    }

    /// The current state of an LSPS1 order, by its id.
    pub fn channel_order(&self, order_id: &str) -> Result<serde_json::Value, WalletError> {
        let status = self
            .node
            .lsps1_liquidity()
            .check_order_status(LSPS1OrderId(order_id.to_owned()))
            .map_err(|error| WalletError::Node(format!("lsps1 order {order_id}: {error}")))?;
        Ok(describe_order(&status))
    }

    /// Send `amount_sats` from the on-chain wallet to `address`; returns the
    /// transaction id.
    pub fn send_onchain(&self, address: &str, amount_sats: u64) -> Result<String, WalletError> {
        crate::custody::refuse_raw(&self.home)?;
        let address = ldk_node::bitcoin::Address::from_str(address)
            .map_err(|error| WalletError::Invalid(format!("address: {error}")))?
            .require_network(network(self.network))
            .map_err(|_| {
                WalletError::Invalid(format!("address is not for {}", self.network.as_str()))
            })?;
        self.node
            .onchain_payment()
            .send_to_address(&address, amount_sats, None)
            .map(|txid| txid.to_string())
            .map_err(|error| WalletError::Node(format!("onchain send: {error}")))
    }

    /// Sync the on-chain and Lightning wallets with the chain source now,
    /// rather than at the next background interval. Blocks until it ends.
    pub fn sync(&self) -> Result<(), WalletError> {
        self.node
            .sync_wallets()
            .map_err(|error| WalletError::Node(format!("sync: {error}")))
    }

    /// When the on-chain wallet last finished a sync, in Unix seconds.
    pub fn onchain_synced_at(&self) -> Option<u64> {
        self.node.status().latest_onchain_wallet_sync_timestamp
    }

    pub fn stop(&self) -> Result<(), WalletError> {
        self.node
            .stop()
            .map_err(|error| WalletError::Node(format!("stop: {error}")))
    }

    /// Whether the node is running and when it last synced.
    pub fn status(&self) -> serde_json::Value {
        let status = self.node.status();
        serde_json::json!({
            "running": status.is_running,
            "best_block_height": status.current_best_block.height,
            "best_block_hash": status.current_best_block.block_hash.to_string(),
            "lightning_synced_at": status.latest_lightning_wallet_sync_timestamp,
            "onchain_synced_at": status.latest_onchain_wallet_sync_timestamp,
            "listening": self.node.listening_addresses().map(|addresses| {
                addresses.iter().map(ToString::to_string).collect::<Vec<_>>()
            }),
        })
    }

    /// Take the next queued node event, if any, marking it handled, and
    /// describe it as a JSON document.
    pub fn next_event(&self) -> Result<Option<serde_json::Value>, WalletError> {
        let Some(event) = self.node.next_event() else {
            return Ok(None);
        };
        let value = describe(&event);
        self.node
            .event_handled()
            .map_err(|error| WalletError::Node(format!("event: {error}")))?;
        Ok(Some(value))
    }

    /// Drain queued events so the queue stays bounded during a wait.
    fn drain_events(&self) {
        while let Ok(Some(_)) = self.next_event() {}
    }

    /// A node that just started has channels but no connected peers yet;
    /// give reconnection up to `wait` before a send that needs a usable
    /// channel. Returns as soon as one channel is usable, or when the node
    /// has no channels at all.
    fn await_usable_channel(&self, wait: Duration) {
        let deadline = Instant::now() + wait;
        let mut next_dial = Instant::now();
        loop {
            let channels = self.node.list_channels();
            if channels.is_empty() || channels.iter().any(|channel| channel.is_usable) {
                return;
            }
            if Instant::now() >= deadline {
                return;
            }
            if Instant::now() >= next_dial {
                self.dial_stored_peers();
                next_dial = Instant::now() + PEER_DIAL_INTERVAL;
            }
            self.drain_events();
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    /// Dial every stored peer that is not connected. Returns how many
    /// connections the calls made; a peer that does not answer is left for
    /// the next round.
    pub fn dial_stored_peers(&self) -> usize {
        self.node
            .list_peers()
            .into_iter()
            .filter(|peer| peer.is_persisted && !peer.is_connected)
            .filter(|peer| {
                self.node
                    .connect(peer.node_id, peer.address.clone(), true)
                    .is_ok()
            })
            .count()
    }

    /// How often a resident should call `dial_stored_peers`.
    pub const fn peer_dial_interval() -> Duration {
        PEER_DIAL_INTERVAL
    }

    /// Whether a usable channel already has `amount_msat` of inbound
    /// capacity, so a plain invoice can be paid without the LSP opening a
    /// channel.
    fn can_receive(&self, amount_msat: u64) -> bool {
        self.node
            .list_channels()
            .iter()
            .any(|channel| channel.is_usable && channel.inbound_capacity_msat >= amount_msat)
    }

    fn payment(&self, hash: [u8; 32]) -> Option<PaymentDetails> {
        self.node
            .payment(&ldk_node::lightning::ln::channelmanager::PaymentId(hash))
    }
}

impl LdkWallet {
    fn receive_core(
        &self,
        amount_msat: u64,
        request_hash: [u8; 32],
        expiry_secs: u32,
    ) -> Result<IssuedInvoice, WalletError> {
        if amount_msat == 0 {
            return Err(WalletError::Invalid("amount must be positive".into()));
        }
        let description =
            Bolt11InvoiceDescription::Hash(Sha256(sha256::Hash::from_byte_array(request_hash)));
        let payment = self.node.bolt11_payment();
        let invoice = match self.jit.filter(|_| !self.can_receive(amount_msat)) {
            None => payment
                .receive(amount_msat, &description, expiry_secs)
                .map_err(|error| WalletError::Node(format!("receive: {error}")))?,
            Some(LspProtocol::Lsps4) => payment
                .receive_via_lsps4_jit_channel(Some(amount_msat), &description, expiry_secs)
                .map_err(|error| WalletError::Node(format!("lsps4 receive: {error}")))?,
            Some(_) => payment
                .receive_via_jit_channel(amount_msat, &description, expiry_secs, None)
                .map_err(|error| WalletError::Node(format!("lsps2 receive: {error}")))?,
        };
        Ok(IssuedInvoice {
            bolt11: invoice.to_string(),
            payment_hash: hex::encode(invoice.payment_hash().to_byte_array()),
            amount_msat,
            description_hash: hex::encode(request_hash),
            expiry_secs,
            pay_to: self.node_id(),
        })
    }

    fn pay_core(
        &self,
        invoice: &str,
        max_fee_msat: u64,
        wait: Duration,
    ) -> Result<Proof, WalletError> {
        let (parsed, hash, amount_msat) =
            check_payable(invoice, network(self.network), unix_now())?;
        let hash_hex = hex::encode(hash);

        if let Some(existing) = self.payment(hash) {
            let record = record(&existing);
            if record.direction == PaymentDirection::Inbound {
                return Err(WalletError::Invalid(format!(
                    "invoice {hash_hex} was issued by this wallet; it cannot pay itself"
                )));
            }
            if record.status == PaymentStatus::Succeeded {
                return proof_from(&record, invoice).ok_or_else(|| {
                    WalletError::Node(format!(
                        "payment {hash_hex} succeeded but the store holds no preimage"
                    ))
                });
            }
        } else {
            self.await_usable_channel(PEER_RECONNECT_WAIT);
            let route =
                RouteParametersConfig::default().with_max_total_routing_fee_msat(max_fee_msat);
            self.node
                .bolt11_payment()
                .send(&parsed, Some(route))
                .map_err(|error| WalletError::Node(format!("send: {error}")))?;
        }

        let started = Instant::now();
        loop {
            self.drain_events();
            if let Some(details) = self.payment(hash) {
                let record = record(&details);
                match record.status {
                    PaymentStatus::Succeeded => {
                        return proof_from(&record, invoice).ok_or_else(|| {
                            WalletError::Node(format!(
                                "payment {hash_hex} succeeded but the store holds no preimage"
                            ))
                        });
                    }
                    PaymentStatus::Failed => {
                        return Err(WalletError::Failed {
                            payment_hash: hash_hex,
                            reason: format!(
                                "no route within a {max_fee_msat} msat fee cap, or the payee rejected {amount_msat} msat"
                            ),
                        });
                    }
                    PaymentStatus::Pending => {}
                }
            }
            if started.elapsed() >= wait {
                return Err(WalletError::Pending {
                    payment_hash: hash_hex,
                    waited_secs: wait.as_secs(),
                });
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }
}

impl LightningWallet for LdkWallet {
    fn node_id(&self) -> String {
        self.node.node_id().to_string()
    }

    fn receive_exact(
        &self,
        amount_msat: u64,
        request_hash: [u8; 32],
        expiry_secs: u32,
    ) -> Result<IssuedInvoice, WalletError> {
        crate::custody::refuse_raw(&self.home)?;
        self.receive_core(amount_msat, request_hash, expiry_secs)
    }
    fn pay(&self, invoice: &str, max_fee_msat: u64, wait: Duration) -> Result<Proof, WalletError> {
        crate::custody::refuse_raw(&self.home)?;
        self.pay_core(invoice, max_fee_msat, wait)
    }
    fn lookup(&self, payment_hash: [u8; 32]) -> Result<Option<PaymentRecord>, WalletError> {
        Ok(self.payment(payment_hash).as_ref().map(record))
    }

    fn payments(&self) -> Result<Vec<PaymentRecord>, WalletError> {
        Ok(self
            .node
            .list_payments_with_filter(|p| !matches!(p.kind, PaymentKind::Onchain { .. }))
            .iter()
            .map(|details| PaymentRecord {
                preimage: None,
                bolt11: None,
                ..record(details)
            })
            .collect())
    }

    fn balance(&self) -> Result<Balance, WalletError> {
        let balances = self.node.list_balances();
        Ok(Balance {
            onchain_total_sats: balances.total_onchain_balance_sats,
            onchain_spendable_sats: balances.spendable_onchain_balance_sats,
            lightning_total_sats: balances.total_lightning_balance_sats,
            anchor_reserve_sats: balances.total_anchor_channels_reserve_sats,
        })
    }

    fn channels(&self) -> Result<Vec<Channel>, WalletError> {
        Ok(self
            .node
            .list_channels()
            .into_iter()
            .map(|channel| Channel {
                channel_id: channel.channel_id.to_string(),
                user_channel_id: channel.user_channel_id.0.to_string(),
                counterparty: channel.counterparty_node_id.to_string(),
                value_sats: channel.channel_value_sats,
                outbound_msat: channel.outbound_capacity_msat,
                inbound_msat: channel.inbound_capacity_msat,
                confirmations: channel.confirmations,
                confirmations_required: channel.confirmations_required,
                ready: channel.is_channel_ready,
                usable: channel.is_usable,
                announced: channel.is_announced,
            })
            .collect())
    }

    fn funding_address(&self) -> Result<String, WalletError> {
        crate::custody::refuse_raw(&self.home)?;
        self.node
            .onchain_payment()
            .new_address()
            .map(|address| address.to_string())
            .map_err(|error| WalletError::Node(format!("address: {error}")))
    }

    fn open_channel(
        &self,
        node_id: &str,
        address: &str,
        amount_sats: u64,
        announce: bool,
    ) -> Result<String, WalletError> {
        crate::custody::refuse_raw(&self.home)?;
        let node_id = pubkey(node_id)?;
        let address = socket(address)?;
        let result = if announce {
            self.node
                .open_announced_channel(node_id, address, amount_sats, None, None)
        } else {
            self.node
                .open_channel(node_id, address, amount_sats, None, None)
        };
        result
            .map(|id| id.0.to_string())
            .map_err(|error| WalletError::Node(format!("open channel: {error}")))
    }

    fn close_channel(
        &self,
        user_channel_id: &str,
        counterparty: &str,
        force: bool,
    ) -> Result<(), WalletError> {
        crate::custody::refuse_raw(&self.home)?;
        let id = ldk_node::UserChannelId(user_channel_id.parse::<u128>().map_err(|_| {
            WalletError::Invalid(format!(
                "channel id `{user_channel_id}` is not the decimal user_channel_id `channel list` prints"
            ))
        })?);
        let counterparty = pubkey(counterparty)?;
        let result = if force {
            self.node.force_close_channel(
                &id,
                counterparty,
                Some("openagents x402 node channel close --force".to_owned()),
            )
        } else {
            self.node.close_channel(&id, counterparty)
        };
        result.map_err(|error| WalletError::Node(format!("close channel: {error}")))
    }
}

/// Parse `invoice` and refuse, before anything is dispatched, one that is
/// malformed, on another network, expired, or without an amount. Returns the
/// invoice, its payment hash, and its amount.
pub fn check_payable(
    invoice: &str,
    network: ldk_node::bitcoin::Network,
    now: Duration,
) -> Result<(Bolt11Invoice, [u8; 32], u64), WalletError> {
    let parsed = Bolt11Invoice::from_str(invoice.trim())
        .map_err(|error| WalletError::Invalid(format!("invoice: {error}")))?;
    if parsed.network() != network {
        return Err(WalletError::Invalid(format!(
            "invoice is for {}, this wallet is on {network}",
            parsed.network()
        )));
    }
    if parsed.would_expire(now) {
        return Err(WalletError::Invalid("invoice has expired".into()));
    }
    let amount_msat = parsed
        .amount_milli_satoshis()
        .ok_or_else(|| WalletError::Invalid("invoice has no amount".into()))?;
    let hash = parsed.payment_hash().to_byte_array();
    Ok((parsed, hash, amount_msat))
}

fn unix_now() -> Duration {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
}

fn describe_order(status: &LSPS1OrderStatus) -> serde_json::Value {
    let bolt11 = status.payment_options.bolt11.as_ref().map(|pay| {
        serde_json::json!({
            "state": format!("{:?}", pay.state).to_lowercase(),
            "expires_at": pay.expires_at.to_rfc3339(),
            "fee_total_sat": pay.fee_total_sat,
            "order_total_sat": pay.order_total_sat,
            "invoice": pay.invoice.to_string(),
        })
    });
    let onchain = status.payment_options.onchain.as_ref().map(|pay| {
        serde_json::json!({
            "state": format!("{:?}", pay.state).to_lowercase(),
            "expires_at": pay.expires_at.to_rfc3339(),
            "fee_total_sat": pay.fee_total_sat,
            "order_total_sat": pay.order_total_sat,
            "address": pay.address.to_string(),
            "min_onchain_payment_confirmations": pay.min_onchain_payment_confirmations,
        })
    });
    let channel = status.channel_state.as_ref().map(|channel| {
        serde_json::json!({
            "funded_at": channel.funded_at.to_rfc3339(),
            "funding_outpoint": channel.funding_outpoint.to_string(),
            "expires_at": channel.expires_at.to_rfc3339(),
        })
    });
    serde_json::json!({
        "order_id": status.order_id.0,
        "lsp_balance_sat": status.order_params.lsp_balance_sat,
        "client_balance_sat": status.order_params.client_balance_sat,
        "required_channel_confirmations": status.order_params.required_channel_confirmations,
        "funding_confirms_within_blocks": status.order_params.funding_confirms_within_blocks,
        "channel_expiry_blocks": status.order_params.channel_expiry_blocks,
        "announce_channel": status.order_params.announce_channel,
        "bolt11": bolt11,
        "onchain": onchain,
        "channel": channel,
    })
}

fn network(network: crate::config::Network) -> ldk_node::bitcoin::Network {
    use crate::config::Network as N;
    match network {
        N::Bitcoin => ldk_node::bitcoin::Network::Bitcoin,
        N::Testnet => ldk_node::bitcoin::Network::Testnet,
        N::Signet => ldk_node::bitcoin::Network::Signet,
        N::Regtest => ldk_node::bitcoin::Network::Regtest,
    }
}

fn pubkey(text: &str) -> Result<PublicKey, WalletError> {
    PublicKey::from_str(text)
        .map_err(|error| WalletError::Invalid(format!("node id {text:?}: {error}")))
}

fn socket(text: &str) -> Result<SocketAddress, WalletError> {
    SocketAddress::from_str(text)
        .map_err(|error| WalletError::Invalid(format!("address {text:?}: {error:?}")))
}

fn record(details: &PaymentDetails) -> PaymentRecord {
    let (hash, preimage, bolt11) = match &details.kind {
        PaymentKind::Bolt11 { hash, preimage, .. }
        | PaymentKind::Bolt11Jit { hash, preimage, .. } => (
            hex::encode(hash.0),
            preimage.map(|p| hex::encode(p.0)),
            None,
        ),
        PaymentKind::Spontaneous { hash, preimage, .. } => (
            hex::encode(hash.0),
            preimage.map(|p| hex::encode(p.0)),
            None,
        ),
        PaymentKind::Bolt12Offer { hash, preimage, .. }
        | PaymentKind::Bolt12Refund { hash, preimage, .. } => (
            hash.map(|h| hex::encode(h.0)).unwrap_or_default(),
            preimage.map(|p| hex::encode(p.0)),
            None,
        ),
        PaymentKind::Onchain { txid, .. } => (txid.to_string(), None, None),
    };
    let direction = match details.direction {
        LdkDirection::Inbound => PaymentDirection::Inbound,
        LdkDirection::Outbound => PaymentDirection::Outbound,
    };
    let status = match details.status {
        ldk_node::payment::PaymentStatus::Pending => PaymentStatus::Pending,
        ldk_node::payment::PaymentStatus::Succeeded => PaymentStatus::Succeeded,
        ldk_node::payment::PaymentStatus::Failed => PaymentStatus::Failed,
    };
    // An unpaid inbound preimage is the secret a payer would need to forge
    // proof; it is only a receipt once the payment settled.
    let preimage = if direction == PaymentDirection::Inbound && status != PaymentStatus::Succeeded {
        None
    } else {
        preimage
    };
    PaymentRecord {
        payment_hash: hash,
        direction,
        status,
        amount_msat: details.amount_msat,
        fee_msat: details.fee_paid_msat,
        preimage,
        bolt11,
        updated_at: details.latest_update_timestamp,
    }
}

fn proof_from(record: &PaymentRecord, bolt11: &str) -> Option<Proof> {
    let preimage = record.preimage.clone()?;
    parse_hash32(&preimage).ok()?;
    Some(Proof {
        payment_hash: record.payment_hash.clone(),
        preimage,
        amount_msat: record.amount_msat.unwrap_or(0),
        fee_msat: record.fee_msat.unwrap_or(0),
        bolt11: bolt11.trim().to_string(),
    })
}

fn describe(event: &Event) -> serde_json::Value {
    use serde_json::json;
    match event {
        Event::PaymentSuccessful {
            payment_hash,
            payment_preimage,
            fee_paid_msat,
            ..
        } => json!({
            "event": "payment_successful",
            "payment_hash": hex::encode(payment_hash.0),
            "preimage": payment_preimage.map(|p| hex::encode(p.0)),
            "fee_msat": fee_paid_msat,
        }),
        Event::PaymentFailed {
            payment_hash,
            reason,
            ..
        } => json!({
            "event": "payment_failed",
            "payment_hash": payment_hash.map(|h| hex::encode(h.0)),
            "reason": reason.map(|r| format!("{r:?}")),
        }),
        Event::PaymentReceived {
            payment_hash,
            amount_msat,
            ..
        } => json!({
            "event": "payment_received",
            "payment_hash": hex::encode(payment_hash.0),
            "amount_msat": amount_msat,
        }),
        Event::PaymentClaimable {
            payment_hash,
            claimable_amount_msat,
            ..
        } => json!({
            "event": "payment_claimable",
            "payment_hash": hex::encode(payment_hash.0),
            "amount_msat": claimable_amount_msat,
        }),
        Event::ChannelPending {
            channel_id,
            counterparty_node_id,
            ..
        } => json!({
            "event": "channel_pending",
            "channel_id": channel_id.to_string(),
            "counterparty": counterparty_node_id.to_string(),
        }),
        Event::ChannelReady {
            channel_id,
            counterparty_node_id,
            ..
        } => json!({
            "event": "channel_ready",
            "channel_id": channel_id.to_string(),
            "counterparty": counterparty_node_id.map(|k| k.to_string()),
        }),
        Event::ChannelClosed {
            channel_id,
            counterparty_node_id,
            reason,
            ..
        } => json!({
            "event": "channel_closed",
            "channel_id": channel_id.to_string(),
            "counterparty": counterparty_node_id.map(|k| k.to_string()),
            "reason": reason.as_ref().map(|r| r.to_string()),
        }),
        other => json!({ "event": "other", "detail": format!("{other:?}") }),
    }
}

#[cfg(unix)]
impl crate::resident::Served for LdkWallet {
    fn status(&self) -> serde_json::Value {
        LdkWallet::status(self)
    }

    fn custodial_pay(&self, admitted: &crate::custody::Admitted) -> Result<Proof, WalletError> {
        let permit = admitted.take()?;
        let manifest = crate::custody::read(&self.home)?
            .ok_or_else(|| WalletError::Setup("Shared custody is required.".into()))?;
        if manifest.node != self.node_id()
            || manifest.node != permit.node
            || manifest.origin != permit.origin
        {
            return Err(WalletError::Setup(
                "Shared custodian identity changed.".into(),
            ));
        }
        let crate::custody::Terms::Pay {
            invoice,
            max_fee_msat,
            wait_secs,
        } = &permit.terms
        else {
            return Err(WalletError::Invalid(
                "Exact custody payment required.".into(),
            ));
        };
        self.pay_core(invoice, *max_fee_msat, Duration::from_secs(*wait_secs))
    }
    fn custodial_receive(
        &self,
        admitted: &crate::custody::Admitted,
    ) -> Result<IssuedInvoice, WalletError> {
        let permit = admitted.take()?;
        let manifest = crate::custody::read(&self.home)?
            .ok_or_else(|| WalletError::Setup("Shared custody is required.".into()))?;
        if manifest.node != self.node_id()
            || manifest.node != permit.node
            || manifest.origin != permit.origin
        {
            return Err(WalletError::Setup(
                "Shared custodian identity changed.".into(),
            ));
        }
        let crate::custody::Terms::Receive {
            amount_msat,
            request_hash,
            expiry_secs,
        } = &permit.terms
        else {
            return Err(WalletError::Invalid(
                "Exact custody funding required.".into(),
            ));
        };
        self.receive_core(
            *amount_msat,
            crate::parse_hash32(request_hash)?,
            *expiry_secs,
        )
    }
    fn buy_channel(
        &self,
        lsp_balance_sat: u64,
        client_balance_sat: u64,
        channel_expiry_blocks: u32,
        announce: bool,
    ) -> Result<serde_json::Value, WalletError> {
        LdkWallet::buy_channel(
            self,
            lsp_balance_sat,
            client_balance_sat,
            channel_expiry_blocks,
            announce,
        )
    }

    fn channel_order(&self, order_id: &str) -> Result<serde_json::Value, WalletError> {
        LdkWallet::channel_order(self, order_id)
    }

    fn send_onchain(&self, address: &str, amount_sats: u64) -> Result<String, WalletError> {
        LdkWallet::send_onchain(self, address, amount_sats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Issued by this wallet on signet at 1790542223 for 1000 msat with
    // description hash aa..aa and a 600 s expiry.
    const SIGNET: &str = "lntbs10n1p4tnqv0hp5424242424242424242424242424242424242424242424242424qnp4qgr8w0u4scfnaxrqv2hjaw6xvq7jfmqactz27fwnrju8x9e7vrar2pp5zf733s3jrl38kzspmmx038d6jkfce5psal06hvtrm0zwd20tu8lqsp5r3ctfk3dlesws3jjejl8qsj0uyl9a6ek7nwyr9vpuqx6z3v7a6ns9qyysgqcqzp2xqzjcuj9y7t5aae4z5g95mw2cdgzma3fxzjxjs9t9a8tj7ur53sc6eaczwhcz07aw82a6jl38ssjg7uyf60llg58aq0uqrxq8pszfzmk8lxqps636mq";

    #[test]
    fn restored_wallet_keeps_its_node_id_and_addresses() {
        let root =
            std::env::temp_dir().join(format!("openagents-wallet-restore-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (home, copy, seed_only, dest) = (
            root.join("a"),
            root.join("b"),
            root.join("c"),
            root.join("backup"),
        );
        std::fs::create_dir_all(&home).unwrap();
        let config =
            WalletConfig::new(crate::Network::Regtest, Some("http://127.0.0.1:1")).unwrap();
        config.save(&home).unwrap();
        let (mnemonic, created) =
            crate::config::load_or_create_seed(&home, true, generate_mnemonic).unwrap();
        assert!(created);
        let (node_id, addresses) = LdkWallet::identity(&home, &config, &mnemonic, 3).unwrap();
        assert_eq!(node_id.len(), 66);

        let manifest = crate::backup::write(&home, &dest).unwrap();
        assert!(manifest.store, "the built node left a store to snapshot");
        let (_, next) = LdkWallet::identity(&home, &config, &mnemonic, 1).unwrap();
        assert!(!addresses.contains(&next[0]));
        crate::backup::restore(&dest, &copy).unwrap();
        let (restored, _) = crate::config::load_or_create_seed(&copy, false, String::new).unwrap();
        assert_eq!(restored, mnemonic);
        let copied = WalletConfig::load(&copy).unwrap();
        assert_eq!(
            LdkWallet::identity(&copy, &copied, &restored, 1).unwrap(),
            (node_id.clone(), next),
            "the restored store continues the address sequence"
        );

        std::fs::create_dir_all(&seed_only).unwrap();
        config.save(&seed_only).unwrap();
        crate::config::load_or_create_seed(&seed_only, true, || mnemonic.clone()).unwrap();
        assert_eq!(
            LdkWallet::identity(&seed_only, &config, &mnemonic, 3).unwrap(),
            (node_id, addresses),
            "the seed alone gives the same node id and the same addresses from the start"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn entropy_gives_one_mnemonic_and_bad_entropy_is_refused_quietly() {
        let words = mnemonic_from_entropy(&[7; 32]).unwrap();
        assert_eq!(words.split_whitespace().count(), 24);
        assert_eq!(mnemonic_from_entropy(&[7; 32]).unwrap(), words);
        assert_ne!(mnemonic_from_entropy(&[8; 32]).unwrap(), words);
        let refusal = mnemonic_from_entropy(&[7; 16]).unwrap_err().to_string();
        assert!(!refusal.contains("0707"), "{refusal}");
    }

    #[test]
    fn refuses_before_dispatch() {
        let bitcoin = ldk_node::bitcoin::Network::Bitcoin;
        let signet = ldk_node::bitcoin::Network::Signet;
        let issued = Duration::from_secs(1_790_542_223);
        assert!(matches!(
            check_payable("not an invoice", bitcoin, issued),
            Err(WalletError::Invalid(_))
        ));
        let wrong = check_payable(SIGNET, bitcoin, issued)
            .unwrap_err()
            .to_string();
        assert!(wrong.contains("signet"), "{wrong}");
        let (_, hash, amount) = check_payable(SIGNET, signet, issued).unwrap();
        assert_eq!(amount, 1000);
        assert_eq!(
            hex::encode(hash),
            "127d18c2321fe27b0a01deccf89dba95938cd030efdfabb163dbc4e6a9ebe1fe"
        );
        let expired = check_payable(SIGNET, signet, issued + Duration::from_secs(601))
            .unwrap_err()
            .to_string();
        assert!(expired.contains("expired"), "{expired}");
    }

    #[test]
    fn proof_needs_a_full_preimage() {
        let mut record = PaymentRecord {
            payment_hash: "00".repeat(32),
            direction: PaymentDirection::Outbound,
            status: PaymentStatus::Succeeded,
            amount_msat: Some(1000),
            fee_msat: Some(3),
            preimage: None,
            bolt11: None,
            updated_at: 0,
        };
        assert!(proof_from(&record, SIGNET).is_none());
        record.preimage = Some("ab".repeat(31));
        assert!(proof_from(&record, SIGNET).is_none());
        record.preimage = Some("ab".repeat(32));
        let proof = proof_from(&record, " lntbs1 ").unwrap();
        assert_eq!(proof.fee_msat, 3);
        assert_eq!(proof.bolt11, "lntbs1");
    }
}
