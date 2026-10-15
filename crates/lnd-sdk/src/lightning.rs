//! Node identity, balances, invoice issuance, and authenticated service access.
use crate::{
    Error, Result,
    error::{signed, unsigned},
    macaroon::Macaroon,
    proto::{lnrpc, verrpc},
    service::{Health, LightningRail},
    transport::{self, AuthChannel, Config},
};
use async_trait::async_trait;
use openagents_wallet::{Balance, Channel, IssuedInvoice, PaymentRecord, Proof};
use std::time::Duration;

#[derive(Clone)]
pub struct Client {
    pub(crate) channel: AuthChannel,
    node_id: String,
    allow_version_mismatch: bool,
    pub(crate) network: String,
}
impl Client {
    pub async fn connect(config: &Config, macaroon: Macaroon) -> Result<Self> {
        Self::attach(
            transport::connect(config, macaroon).await?,
            config.allow_version_mismatch,
        )
        .await
    }
    pub(crate) async fn attach(channel: AuthChannel, allow_version_mismatch: bool) -> Result<Self> {
        let info = transport::read(|| {
            let mut rpc = lnrpc::lightning_client::LightningClient::new(channel.clone());
            async move { rpc.get_info(lnrpc::GetInfoRequest {}).await }
        })
        .await?;
        let version = transport::read(|| {
            let mut rpc = verrpc::versioner_client::VersionerClient::new(channel.clone());
            async move { rpc.get_version(verrpc::VersionRequest {}).await }
        })
        .await?;
        if !allow_version_mismatch && (version.app_major != 0 || version.app_minor != 21) {
            return Err(Error::VersionMismatch);
        }
        let node =
            hex::decode(&info.identity_pubkey).map_err(|_| Error::Invalid("node identity"))?;
        if node.len() != 33 || !matches!(node[0], 2 | 3) {
            return Err(Error::Invalid("node identity"));
        }
        #[allow(deprecated)]
        let network = info
            .chains
            .iter()
            .find(|c| c.chain == "bitcoin")
            .ok_or(Error::Invalid("Bitcoin chain"))?
            .network
            .clone();
        if !["mainnet", "testnet", "regtest", "signet"].contains(&network.as_str()) {
            return Err(Error::Invalid("Bitcoin network"));
        }
        Ok(Self {
            channel,
            node_id: info.identity_pubkey,
            allow_version_mismatch,
            network,
        })
    }
    pub(crate) fn rpc(&self) -> lnrpc::lightning_client::LightningClient<AuthChannel> {
        lnrpc::lightning_client::LightningClient::new(self.channel.clone())
    }
    pub fn network(&self) -> &str {
        &self.network
    }
    pub async fn info(&self) -> Result<lnrpc::GetInfoResponse> {
        transport::read(|| {
            let mut rpc = self.rpc();
            async move { rpc.get_info(lnrpc::GetInfoRequest {}).await }
        })
        .await
    }
    pub async fn lookup_invoice(&self, hash: [u8; 32]) -> Result<lnrpc::Invoice> {
        transport::read(|| {
            let mut rpc = self.rpc();
            async move {
                rpc.lookup_invoice(lnrpc::PaymentHash {
                    r_hash: hash.to_vec(),
                    ..Default::default()
                })
                .await
            }
        })
        .await
    }
    pub async fn create_invoice(
        &self,
        amount_msat: u64,
        description_hash: [u8; 32],
        expiry_secs: u32,
    ) -> Result<IssuedInvoice> {
        self.create_invoice_with_hints(amount_msat, description_hash, expiry_secs, Vec::new())
            .await
    }
    /// Supply explicit hints when the private peer has no public graph entry.
    /// The hints must come from the receiver's current incoming channel policy.
    pub async fn create_invoice_with_hints(
        &self,
        amount_msat: u64,
        description_hash: [u8; 32],
        expiry_secs: u32,
        route_hints: Vec<lnrpc::RouteHint>,
    ) -> Result<IssuedInvoice> {
        self.ensure_current().await?;
        if amount_msat == 0 || expiry_secs == 0 {
            return Err(Error::Invalid("invoice terms"));
        }
        let req = lnrpc::Invoice {
            value_msat: signed(amount_msat, "amount")?,
            description_hash: description_hash.to_vec(),
            expiry: i64::from(expiry_secs),
            private: true,
            route_hints,
            ..Default::default()
        };
        // AddInvoice is a mutation: do not blindly repeat an uncertain outcome.
        let reply = transport::once(self.rpc().add_invoice(req)).await?;
        let decoded = crate::router::invoice(&reply.payment_request)
            .map_err(|_| Error::Invalid("issued invoice"))?;
        use lightning_invoice::Bolt11InvoiceDescriptionRef;
        let correct_hash = match decoded.description() {
            Bolt11InvoiceDescriptionRef::Hash(h) => {
                h.0.to_string() == hex::encode(description_hash)
            }
            _ => false,
        };
        if decoded.amount_milli_satoshis() != Some(amount_msat)
            || decoded.recover_payee_pub_key().to_string() != self.node_id
            || !correct_hash
            || decoded.payment_hash().to_string() != hex::encode(&reply.r_hash)
            || decoded.currency() != self.invoice_currency()
            || decoded.expiry_time().as_secs() != u64::from(expiry_secs)
        {
            return Err(Error::Invalid("issued invoice terms"));
        }
        Ok(IssuedInvoice {
            bolt11: reply.payment_request,
            payment_hash: hex::encode(reply.r_hash),
            amount_msat,
            description_hash: hex::encode(description_hash),
            expiry_secs,
            pay_to: self.node_id.clone(),
        })
    }
}
#[async_trait]
impl LightningRail for Client {
    fn node_id(&self) -> String {
        self.node_id.clone()
    }
    async fn receive_exact(&self, a: u64, h: [u8; 32], e: u32) -> Result<IssuedInvoice> {
        self.create_invoice(a, h, e).await
    }
    async fn pay(&self, i: &str, f: u64, w: Duration) -> Result<Proof> {
        self.send_payment(i, f, w).await
    }
    async fn lookup(&self, h: [u8; 32]) -> Result<Option<PaymentRecord>> {
        match self.track(h).await {
            Ok(p) => return crate::router::record(&p).map(Some),
            Err(Error::NotFound) => {}
            Err(e) => return Err(e),
        }
        match self.lookup_invoice(h).await {
            Ok(i) => Ok(Some(crate::invoices::record(&i)?)),
            Err(Error::NotFound) => Ok(None),
            Err(e) => Err(e),
        }
    }
    async fn balance(&self) -> Result<Balance> {
        let onchain = transport::read(|| {
            let mut rpc = self.rpc();
            async move {
                rpc.wallet_balance(lnrpc::WalletBalanceRequest::default())
                    .await
            }
        })
        .await?;
        let lightning = transport::read(|| {
            let mut rpc = self.rpc();
            async move { rpc.channel_balance(lnrpc::ChannelBalanceRequest {}).await }
        })
        .await?;
        Ok(Balance {
            onchain_total_sats: unsigned(onchain.total_balance)?,
            onchain_spendable_sats: unsigned(onchain.confirmed_balance)?
                .saturating_sub(unsigned(onchain.locked_balance)?)
                .saturating_sub(unsigned(onchain.reserved_balance_anchor_chan)?),
            lightning_total_sats: lightning.local_balance.map(|a| a.sat).unwrap_or_default(),
            anchor_reserve_sats: unsigned(onchain.reserved_balance_anchor_chan)?,
        })
    }
    async fn channels(&self) -> Result<Vec<Channel>> {
        let channels = transport::read(|| {
            let mut rpc = self.rpc();
            async move {
                rpc.list_channels(lnrpc::ListChannelsRequest::default())
                    .await
            }
        })
        .await?;
        channels
            .channels
            .into_iter()
            .map(|c| {
                Ok(Channel {
                    channel_id: c.channel_point.clone(),
                    user_channel_id: c.channel_point,
                    counterparty: c.remote_pubkey,
                    value_sats: unsigned(c.capacity)?,
                    outbound_msat: unsigned(c.local_balance)?
                        .checked_mul(1000)
                        .ok_or(Error::Invalid("balance"))?,
                    inbound_msat: unsigned(c.remote_balance)?
                        .checked_mul(1000)
                        .ok_or(Error::Invalid("balance"))?,
                    confirmations: None,
                    confirmations_required: None,
                    ready: true,
                    usable: c.active,
                    announced: !c.private,
                })
            })
            .collect()
    }
    async fn health(&self) -> Result<Health> {
        let info = self.info().await?;
        let channels = self.channels().await?;
        let pending = transport::read(|| {
            let mut rpc = self.rpc();
            async move {
                rpc.pending_channels(lnrpc::PendingChannelsRequest::default())
                    .await
            }
        })
        .await?;
        Ok(Health {
            synced_to_chain: info.synced_to_chain,
            synced_to_graph: info.synced_to_graph,
            active_channels: info.num_active_channels,
            peers: info.num_peers,
            inbound_msat: channels.iter().map(|c| c.inbound_msat).sum(),
            outbound_msat: channels.iter().map(|c| c.outbound_msat).sum(),
            pending_force_closes: pending.pending_force_closing_channels.len(),
        })
    }
    async fn payments(&self) -> Result<Vec<PaymentRecord>> {
        self.payment_history().await
    }
    async fn open_channel(&self, n: &str, a: &str, s: u64, p: bool) -> Result<String> {
        self.open_peer_channel(n, a, s, p).await
    }
    async fn close_channel(&self, i: &str, c: &str, f: bool) -> Result<()> {
        self.close_peer_channel(i, c, f).await
    }
    async fn funding_address(&self) -> Result<String> {
        self.new_address().await
    }
}

impl Client {
    /// Read explicit incoming hints for active private channels. This needs
    /// offchain:read, which the receiving service's credential includes.
    pub async fn private_route_hints(&self) -> Result<Vec<lnrpc::RouteHint>> {
        let response = transport::read(|| {
            let mut rpc = self.rpc();
            async move {
                rpc.list_channels(lnrpc::ListChannelsRequest {
                    active_only: true,
                    private_only: true,
                    ..Default::default()
                })
                .await
            }
        })
        .await?;
        let mut hints = Vec::new();
        for channel in response.channels {
            let graph = transport::read(|| {
                let mut rpc = self.rpc();
                async move {
                    rpc.get_chan_info(lnrpc::ChanInfoRequest {
                        chan_id: channel.chan_id,
                        ..Default::default()
                    })
                    .await
                }
            })
            .await
            .map_err(|error| match error {
                Error::NotFound => Error::NotSynced,
                error => error,
            })?;
            let policy = if graph.node1_pub == channel.remote_pubkey {
                graph.node1_policy
            } else if graph.node2_pub == channel.remote_pubkey {
                graph.node2_policy
            } else {
                return Err(Error::Invalid("channel peer"));
            };
            let policy = policy.ok_or(Error::NotSynced)?;
            if policy.disabled {
                continue;
            }
            hints.push(lnrpc::RouteHint {
                hop_hints: vec![lnrpc::HopHint {
                    node_id: channel.remote_pubkey,
                    chan_id: channel.chan_id,
                    fee_base_msat: u32::try_from(policy.fee_base_msat)
                        .map_err(|_| Error::Invalid("channel fee"))?,
                    fee_proportional_millionths: u32::try_from(policy.fee_rate_milli_msat)
                        .map_err(|_| Error::Invalid("channel fee"))?,
                    cltv_expiry_delta: policy.time_lock_delta,
                }],
            });
        }
        Ok(hints)
    }
}

impl Client {
    /// Run throughout the LSPS1 purchase. Reject other peers, zero-conf, and non-anchor channels.
    pub async fn guard_inbound_channel(&self, peer: &str, minimum_sats: u64) -> Result<()> {
        self.ensure_current().await?;
        let (tx, rx) = tokio::sync::mpsc::channel(8);
        let mut stream = self
            .rpc()
            .channel_acceptor(tokio_stream::wrappers::ReceiverStream::new(rx))
            .await
            .map_err(Error::from)?
            .into_inner();
        while let Some(request) = stream.message().await.map_err(Error::from)? {
            let accept = hex::encode(&request.node_pubkey) == peer
                && request.funding_amt >= minimum_sats
                && request.commitment_type == lnrpc::CommitmentType::Anchors as i32
                && !request.wants_zero_conf;
            tx.send(lnrpc::ChannelAcceptResponse {
                accept,
                pending_chan_id: request.pending_chan_id,
                min_accept_depth: if accept { 3 } else { 0 },
                error: if accept {
                    String::new()
                } else {
                    "This channel does not meet our requirements.".into()
                },
                ..Default::default()
            })
            .await
            .map_err(|_| Error::Unavailable)?;
        }
        Err(Error::Unavailable)
    }
    pub async fn open_peer_channel(
        &self,
        node_id: &str,
        address: &str,
        amount_sats: u64,
        announce: bool,
    ) -> Result<String> {
        self.open_peer_channel_at_rate(node_id, address, amount_sats, announce, 0)
            .await
    }
    pub async fn open_peer_channel_at_rate(
        &self,
        node_id: &str,
        address: &str,
        amount_sats: u64,
        announce: bool,
        sat_per_vbyte: u64,
    ) -> Result<String> {
        self.ensure_current().await?;
        let key = hex::decode(node_id).map_err(|_| Error::Invalid("peer identity"))?;
        if key.len() != 33 || amount_sats == 0 {
            return Err(Error::Invalid("channel terms"));
        }
        match transport::once(self.rpc().connect_peer(lnrpc::ConnectPeerRequest {
            addr: Some(lnrpc::LightningAddress {
                pubkey: node_id.into(),
                host: address.into(),
            }),
            ..Default::default()
        }))
        .await
        {
            Ok(_) => {}
            Err(Error::AlreadyPaid) => {}
            Err(e) => return Err(e),
        }
        let point = transport::once(self.rpc().open_channel_sync(lnrpc::OpenChannelRequest {
            node_pubkey: key,
            local_funding_amount: signed(amount_sats, "channel amount")?,
            private: !announce,
            commitment_type: lnrpc::CommitmentType::Anchors as i32,
            sat_per_vbyte,
            min_confs: 1,
            ..Default::default()
        }))
        .await?;
        let txid = match point.funding_txid.ok_or(Error::Invalid("channel point"))? {
            lnrpc::channel_point::FundingTxid::FundingTxidStr(txid) => txid,
            lnrpc::channel_point::FundingTxid::FundingTxidBytes(mut txid) => {
                txid.reverse();
                hex::encode(txid)
            }
        };
        Ok(format!("{txid}:{}", point.output_index))
    }
    pub async fn channel_opening_fee(&self, point: &str) -> Result<u64> {
        let txid = point
            .split_once(':')
            .ok_or(Error::Invalid("channel point"))?
            .0;
        let transactions = transport::read(|| {
            let mut rpc = self.rpc();
            async move {
                rpc.get_transactions(lnrpc::GetTransactionsRequest {
                    end_height: -1,
                    ..Default::default()
                })
                .await
            }
        })
        .await?;
        let tx = transactions
            .transactions
            .iter()
            .find(|tx| tx.tx_hash == txid)
            .ok_or(Error::NotFound)?;
        unsigned(tx.total_fees)
    }
    pub async fn close_peer_channel(
        &self,
        point: &str,
        counterparty: &str,
        force: bool,
    ) -> Result<()> {
        self.ensure_current().await?;
        if !self
            .channels()
            .await?
            .iter()
            .any(|c| c.channel_id == point && c.counterparty == counterparty)
        {
            return Err(Error::Invalid("channel identity"));
        }
        let (txid, index) = point
            .split_once(':')
            .ok_or(Error::Invalid("channel point"))?;
        if hex::decode(txid)
            .map_err(|_| Error::Invalid("channel point"))?
            .len()
            != 32
        {
            return Err(Error::Invalid("channel point"));
        }
        let output_index = index
            .parse()
            .map_err(|_| Error::Invalid("channel output"))?;
        let mut stream = transport::once(self.rpc().close_channel(lnrpc::CloseChannelRequest {
            channel_point: Some(lnrpc::ChannelPoint {
                funding_txid: Some(lnrpc::channel_point::FundingTxid::FundingTxidStr(
                    txid.into(),
                )),
                output_index,
            }),
            force,
            ..Default::default()
        }))
        .await?;
        tokio::time::timeout(Duration::from_secs(15), stream.message())
            .await
            .map_err(|_| Error::Timeout)?
            .map_err(Error::from)?
            .ok_or(Error::Unavailable)?;
        Ok(())
    }
    pub async fn payment_history(&self) -> Result<Vec<PaymentRecord>> {
        let mut out = Vec::new();
        let mut offset = 0;
        loop {
            let page = transport::read(|| {
                let mut rpc = self.rpc();
                async move {
                    rpc.list_invoices(lnrpc::ListInvoiceRequest {
                        index_offset: offset,
                        num_max_invoices: 500,
                        ..Default::default()
                    })
                    .await
                }
            })
            .await?;
            let length = page.invoices.len();
            for i in page.invoices {
                let mut p = crate::invoices::record(&i)?;
                p.bolt11 = None;
                out.push(p);
            }
            if length < 500 {
                break;
            }
            if page.last_index_offset <= offset {
                return Err(Error::Invalid("invoice position"));
            }
            offset = page.last_index_offset;
        }
        offset = 0;
        loop {
            let page = transport::read(|| {
                let mut rpc = self.rpc();
                async move {
                    rpc.list_payments(lnrpc::ListPaymentsRequest {
                        index_offset: offset,
                        max_payments: 500,
                        include_incomplete: true,
                        omit_hops: true,
                        ..Default::default()
                    })
                    .await
                }
            })
            .await?;
            let length = page.payments.len();
            for p in page.payments {
                let mut p = crate::router::record(&p)?;
                p.bolt11 = None;
                p.preimage = None;
                out.push(p);
            }
            if length < 500 {
                break;
            }
            if page.last_index_offset <= offset {
                return Err(Error::Invalid("payment position"));
            }
            offset = page.last_index_offset;
        }
        Ok(out)
    }
}

impl Client {
    // Tonic can reconnect a channel after a daemon restart. Recheck the node
    // and release before effects, including after those automatic reconnects.
    pub(crate) async fn ensure_current(&self) -> Result<()> {
        let info = self.info().await?;
        if info.identity_pubkey != self.node_id
            || !info.chains.iter().any(|c| c.network == self.network)
        {
            return Err(Error::NodeMismatch);
        }
        let version = transport::read(|| {
            let mut rpc = verrpc::versioner_client::VersionerClient::new(self.channel.clone());
            async move { rpc.get_version(verrpc::VersionRequest {}).await }
        })
        .await?;
        if !self.allow_version_mismatch && (version.app_major != 0 || version.app_minor != 21) {
            return Err(Error::VersionMismatch);
        }
        Ok(())
    }
    pub(crate) fn invoice_currency(&self) -> lightning_invoice::Currency {
        use lightning_invoice::Currency;
        match self.network.as_str() {
            "mainnet" => Currency::Bitcoin,
            "testnet" => Currency::BitcoinTestnet,
            "signet" => Currency::Signet,
            _ => Currency::Regtest,
        }
    }
}
