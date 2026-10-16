//! Bitcoin Taproot Assets channels with frozen RFQ prices and receipt-backed credits.
use crate::{
    Client, Error, Result,
    liquidity::Journal,
    macaroon::Macaroon,
    proto::{lnrpc, rfqrpc, routerrpc, tapchannelrpc, taprpc},
    service::LightningRail,
    transport::{self, AuthChannel, Config},
};
use pay_ledger::assets::{Quote, Rate, Receipt};
use serde::{Deserialize, Serialize};
use serde_json::json;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Approval {
    pub mainnet_receive_approved: bool,
    pub custody_review_approved: bool,
    pub mainnet_payout_approved: bool,
    pub asset_ids: Vec<String>,
    pub maximum_units: u64,
    pub maximum_credit_msat: i64,
    pub maximum_fee_msat: u64,
}
impl Approval {
    pub fn receive(&self, network: &str, asset: &str, units: u64) -> Result<()> {
        if network == "mainnet" && (!self.mainnet_receive_approved || !self.custody_review_approved)
        {
            return Err(Error::PermissionDenied);
        }
        if asset.len() != 64
            || !asset
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || !self.asset_ids.iter().any(|id| id == asset)
            || units == 0
            || units > self.maximum_units
            || self.maximum_credit_msat <= 0
        {
            return Err(Error::Invalid("approved asset or amount"));
        }
        Ok(())
    }
    pub fn payout(&self, network: &str, asset: &str, units: u64) -> Result<()> {
        self.receive(network, asset, units)?;
        if network == "mainnet" && !self.mainnet_payout_approved {
            return Err(Error::PermissionDenied);
        }
        Ok(())
    }
}
#[derive(Clone)]
pub struct Assets {
    channel: AuthChannel,
    lnd: Client,
}
impl Assets {
    pub async fn connect(config: &Config, mac: Macaroon, lnd: Client) -> Result<Self> {
        let s = Self {
            channel: transport::connect(config, mac).await?,
            lnd,
        };
        s.ready().await?;
        Ok(s)
    }
    fn rpc(&self) -> taprpc::taproot_assets_client::TaprootAssetsClient<AuthChannel> {
        taprpc::taproot_assets_client::TaprootAssetsClient::new(self.channel.clone())
    }
    fn channels(
        &self,
    ) -> tapchannelrpc::taproot_asset_channels_client::TaprootAssetChannelsClient<AuthChannel> {
        tapchannelrpc::taproot_asset_channels_client::TaprootAssetChannelsClient::new(
            self.channel.clone(),
        )
    }
    #[cfg(feature = "supervisor")]
    pub async fn fund_regtest_channel(
        &self,
        asset: [u8; 32],
        peer: [u8; 33],
        units: u64,
    ) -> Result<tapchannelrpc::FundChannelResponse> {
        if self.lnd.network() != "regtest" {
            return Err(Error::PermissionDenied);
        }
        self.ready().await?;
        transport::once(
            self.channels()
                .fund_channel(tapchannelrpc::FundChannelRequest {
                    asset_id: asset.to_vec(),
                    peer_pubkey: peer.to_vec(),
                    asset_amount: units,
                    fee_rate_sat_per_vbyte: 2,
                    push_sat: 1062,
                    ..Default::default()
                }),
        )
        .await
    }
    /// Open only the bounded synthetic signet experiment, never a mainnet channel.
    /// The pinned tapd implementation fixes the BTC funding value at 100,000 sats.
    #[cfg(feature = "supervisor")]
    pub async fn fund_signet_research_channel(
        &self,
        asset: [u8; 32],
        peer: [u8; 33],
        units: u64,
    ) -> Result<tapchannelrpc::FundChannelResponse> {
        if self.lnd.network() != "signet" {
            return Err(Error::PermissionDenied);
        }
        if units == 0 || units > 1_000_000 || asset == [0; 32] {
            return Err(Error::Invalid("signet research asset funding"));
        }
        self.ready().await?;
        transport::once(
            self.channels()
                .fund_channel(tapchannelrpc::FundChannelRequest {
                    asset_id: asset.to_vec(),
                    peer_pubkey: peer.to_vec(),
                    asset_amount: units,
                    fee_rate_sat_per_vbyte: 2,
                    push_sat: 1062,
                    ..Default::default()
                }),
        )
        .await
    }
    pub async fn ready(&self) -> Result<()> {
        let i = transport::read(|| {
            let mut c = self.rpc();
            async move { c.get_info(taprpc::GetInfoRequest {}).await }
        })
        .await?;
        if !i.version.trim_start_matches('v').starts_with("0.8.") {
            return Err(Error::VersionMismatch);
        }
        if i.lnd_identity_pubkey != self.lnd.node_id() || i.network != self.lnd.network() {
            return Err(Error::NodeMismatch);
        }
        if !i.sync_to_chain {
            return Err(Error::NotSynced);
        }
        self.lnd.ensure_current().await
    }
    /// The action ID is single use. An interrupted invoice creation needs operator recovery.
    pub async fn receive(
        &self,
        j: &mut Journal,
        id: &str,
        account: &str,
        asset: [u8; 32],
        edge: [u8; 33],
        units: u64,
        approval: &Approval,
        now: i64,
    ) -> Result<Quote> {
        approval.receive(self.lnd.network(), &hex::encode(asset), units)?;
        self.ready().await?;
        let key = format!("receive:{id}");
        if let Some(v) = j.entries.get(&key) {
            let q: Quote =
                serde_json::from_value(v["quote"].clone()).map_err(|_| Error::InFlight)?;
            if q.node != self.lnd.node_id()
                || q.network != self.lnd.network()
                || q.account != account
                || q.asset_id != hex::encode(asset)
                || q.edge != hex::encode(edge)
                || q.units != units
            {
                return Err(Error::Invalid("saved asset receive"));
            }
            return Ok(q);
        }
        if id.is_empty() || id.len() > 128 || now < 0 {
            return Err(Error::Invalid("asset receive"));
        }
        j.begin(&key,json!({"node":self.lnd.node_id(),"asset":hex::encode(asset),"units":units,"account":account,"edge":hex::encode(edge)}))?;
        let r = transport::once(
            self.channels()
                .add_invoice(tapchannelrpc::AddInvoiceRequest {
                    asset_id: asset.to_vec(),
                    asset_amount: units,
                    peer_pubkey: edge.to_vec(),
                    invoice_request: Some(lnrpc::Invoice {
                        expiry: 600,
                        private: true,
                        ..Default::default()
                    }),
                    execution_policy: 1,
                    asset_min_amt: Some(units),
                    ..Default::default()
                }),
        )
        .await?;
        let buy = r.accepted_buy_quote.ok_or(Error::Invalid("asset quote"))?;
        let invoice = r.invoice_result.ok_or(Error::Invalid("asset invoice"))?;
        let spec = buy.asset_spec.ok_or(Error::Invalid("quoted asset"))?;
        if buy.peer != hex::encode(edge)
            || spec.id != asset
            || !spec.group_pub_key.is_empty()
            || buy.id.len() != 32
            || buy.scid == 0
            || buy.asset_max_amount < units
            || (buy.accepted_max_amount != 0 && buy.accepted_max_amount < units)
            || buy.expiry <= now as u64
            || units < buy.min_transportable_units
        {
            return Err(Error::Invalid("asset quote terms"));
        }
        let rate = buy.ask_asset_rate.ok_or(Error::Invalid("asset rate"))?;
        let rate = Rate {
            coefficient: rate.coefficient,
            scale: rate.scale,
        };
        let credit_msat = rate
            .value_msat(units)
            .map_err(|_| Error::Invalid("asset rate arithmetic"))?;
        if credit_msat > approval.maximum_credit_msat {
            return Err(Error::Invalid("asset credit limit"));
        }
        let decoded = crate::router::invoice(&invoice.payment_request)
            .map_err(|_| Error::Invalid("asset invoice"))?;
        if invoice.r_hash.len() != 32
            || decoded.payment_hash().to_string() != hex::encode(&invoice.r_hash)
            || decoded.recover_payee_pub_key().to_string() != self.lnd.node_id()
            || decoded.currency() != self.lnd.invoice_currency()
        {
            return Err(Error::Invalid("asset invoice identity"));
        }
        let expires_at = i64::try_from(buy.expiry)
            .map_err(|_| Error::Invalid("asset quote expiry"))?
            .min(
                now.checked_add(600)
                    .ok_or(Error::Invalid("asset quote expiry"))?,
            );
        let q = Quote {
            node: self.lnd.node_id(),
            network: self.lnd.network().into(),
            account: account.into(),
            asset_id: hex::encode(asset),
            edge: hex::encode(edge),
            rfq_id: hex::encode(buy.id),
            payment_hash: hex::encode(invoice.r_hash),
            invoice: invoice.payment_request,
            units,
            rate,
            credit_msat,
            created_at: now,
            expires_at,
        };
        j.save(&key, json!({"quote":q}))?;
        Ok(q)
    }
    pub async fn settled(&self, q: &Quote) -> Result<Receipt> {
        self.ready().await?;
        if q.node != self.lnd.node_id() || q.network != self.lnd.network() {
            return Err(Error::NodeMismatch);
        }
        let hash = openagents_wallet::parse_hash32(&q.payment_hash)
            .map_err(|_| Error::Invalid("asset payment hash"))?;
        let invoice = self.lnd.lookup_invoice(hash).await?;
        receipt(q, &invoice)
    }
    /// Send once; interrupted streams are recovered by the original LND hash.
    pub async fn pay(
        &self,
        j: &mut Journal,
        q: &Quote,
        edge: [u8; 33],
        approval: &Approval,
    ) -> Result<serde_json::Value> {
        self.ready().await?;
        if q.network != self.lnd.network() {
            return Err(Error::NodeMismatch);
        }
        let decoded = crate::router::invoice(&q.invoice)
            .map_err(|_| Error::Invalid("asset payout invoice"))?;
        if decoded.payment_hash().to_string() != q.payment_hash
            || decoded.recover_payee_pub_key().to_string() != q.node
            || decoded.currency() != self.lnd.invoice_currency()
        {
            return Err(Error::Invalid("asset payout invoice"));
        }
        let key = format!("payout:{}:{}", self.lnd.node_id(), q.payment_hash);
        if let Some(v) = j.entries.get(&key) {
            if v["quote"] != serde_json::to_value(q).map_err(|_| Error::Checkpoint)? {
                return Err(Error::Invalid("saved asset payout"));
            }
            let p = self
                .lnd
                .track(
                    openagents_wallet::parse_hash32(&q.payment_hash)
                        .map_err(|_| Error::Invalid("asset payment hash"))?,
                )
                .await?;
            let value = payment_record(&p, q, &j.entries[&key]["sell_quote"])?;
            let mut saved = j.entries[&key].clone();
            saved["payment"] = value.clone();
            j.save(&key, saved)?;
            return Ok(value);
        }
        approval.payout(self.lnd.network(), &q.asset_id, q.units)?;
        if q.credit_msat <= 0 || q.credit_msat > approval.maximum_credit_msat {
            return Err(Error::Invalid("asset payout limit"));
        }
        let amount = decoded
            .amount_milli_satoshis()
            .ok_or(Error::Invalid("asset payout amount"))?;
        if amount > approval.maximum_credit_msat as u64 {
            return Err(Error::Invalid("asset payout amount limit"));
        }
        let maximum = amount
            .checked_add(approval.maximum_fee_msat)
            .ok_or(Error::Invalid("asset payout fee"))?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| Error::Invalid("clock"))?
            .as_secs();
        let asset = hex::decode(&q.asset_id).map_err(|_| Error::Invalid("asset id"))?;
        let mut rfq = rfqrpc::rfq_client::RfqClient::new(self.channel.clone());
        let offered = transport::once(rfq.add_asset_sell_order(rfqrpc::AddAssetSellOrderRequest {
            asset_specifier: Some(rfqrpc::AssetSpecifier {
                id: Some(rfqrpc::asset_specifier::Id::AssetId(asset.clone())),
            }),
            payment_max_amt: maximum,
            payment_min_amt: Some(maximum),
            expiry: now + 120,
            peer_pub_key: edge.to_vec(),
            timeout_seconds: 15,
            execution_policy: 1,
            ..Default::default()
        }))
        .await?;
        let Some(rfqrpc::add_asset_sell_order_response::Response::AcceptedQuote(sell)) =
            offered.response
        else {
            return Err(Error::Refused);
        };
        let spec = sell
            .asset_spec
            .as_ref()
            .ok_or(Error::Invalid("asset payout quote"))?;
        if sell.id.len() != 32
            || sell.peer != hex::encode(edge)
            || spec.id != asset
            || !spec.group_pub_key.is_empty()
            || sell.asset_amount > approval.maximum_units
            || sell.asset_amount == 0
            || sell.expiry <= now
            || (sell.accepted_max_amount != 0 && sell.accepted_max_amount < maximum)
        {
            return Err(Error::Invalid("asset payout quote limit"));
        }
        let rate = sell
            .bid_asset_rate
            .as_ref()
            .ok_or(Error::Invalid("asset payout rate"))?;
        let quote = json!({"rfq_id":hex::encode(&sell.id),"edge":sell.peer,"maximum_units":sell.asset_amount,"maximum_msat":maximum,"rate":{"coefficient":rate.coefficient,"scale":rate.scale},"expires_at":sell.expiry});
        j.save(
            &key,
            json!({"quote":q,"sell_quote":quote,"state":"unknown"}),
        )?;
        let stream = transport::once(self.channels().send_payment(
            tapchannelrpc::SendPaymentRequest {
                asset_id: hex::decode(&q.asset_id).map_err(|_| Error::Invalid("asset id"))?,
                asset_amount: q.units,
                peer_pubkey: edge.to_vec(),
                rfq_id: sell.id,
                payment_request: Some(routerrpc::SendPaymentRequest {
                    payment_request: q.invoice.clone(),
                    timeout_seconds: 30,
                    fee_limit_msat: crate::error::signed(
                        approval.maximum_fee_msat,
                        "asset routing fee",
                    )?,
                    ..Default::default()
                }),
                ..Default::default()
            },
        ))
        .await?;
        let mut stream = stream;
        loop {
            let update = tokio::time::timeout(std::time::Duration::from_secs(45), stream.message())
                .await
                .map_err(|_| Error::Timeout)?
                .map_err(Error::from)?
                .ok_or(Error::Unavailable)?;
            if let Some(tapchannelrpc::send_payment_response::Result::PaymentResult(p)) =
                update.result
            {
                if p.payment_hash != q.payment_hash {
                    return Err(Error::Invalid("asset payment identity"));
                }
                let value = payment_record(&p, q, &j.entries[&key]["sell_quote"])?;
                let mut saved = j.entries[&key].clone();
                saved["payment"] = value.clone();
                j.save(&key, saved)?;
                if matches!(p.status, 2 | 3) {
                    return Ok(value);
                }
            }
        }
    }
}
fn payment_record(
    p: &lnrpc::Payment,
    q: &Quote,
    sell: &serde_json::Value,
) -> Result<serde_json::Value> {
    let record = crate::router::record(p)?;
    let mut units = 0u64;
    for h in p.htlcs.iter().filter(|h| h.status == 1) {
        let route = h
            .route
            .as_ref()
            .ok_or(Error::Invalid("asset payout HTLC"))?;
        let data: serde_json::Value = serde_json::from_slice(&route.custom_channel_data)
            .map_err(|_| Error::Invalid("asset payout HTLC data"))?;
        if data["rfq_id"] != sell["rfq_id"] {
            return Err(Error::Invalid("asset payout RFQ"));
        }
        for b in data["balances"]
            .as_array()
            .ok_or(Error::Invalid("asset payout balances"))?
        {
            if b["asset_id"].as_str() != Some(&q.asset_id) {
                return Err(Error::Invalid("asset payout identity"));
            }
            units = units
                .checked_add(
                    b["amount"]
                        .as_u64()
                        .ok_or(Error::Invalid("asset payout units"))?,
                )
                .ok_or(Error::Invalid("asset payout units"))?;
        }
    }
    if p.status == 2
        && (units == 0
            || units
                > sell["maximum_units"]
                    .as_u64()
                    .ok_or(Error::Invalid("saved asset payout limit"))?)
    {
        return Err(Error::Invalid("actual asset payout limit"));
    }
    Ok(
        json!({"payment_hash":record.payment_hash,"status":p.status,"amount_msat":record.amount_msat,"fee_msat":record.fee_msat,"asset_units":units,"custom_channel_data":p.htlcs.iter().filter_map(|h| h.route.as_ref()).map(|r| hex::encode(&r.custom_channel_data)).collect::<Vec<_>>()}),
    )
}
/// Trust only settled HTLC data from the authenticated receiver node.
pub fn receipt(q: &Quote, i: &lnrpc::Invoice) -> Result<Receipt> {
    if i.state != 1 || i.settle_index == 0 {
        return Err(Error::InFlight);
    }
    if hex::encode(&i.r_hash) != q.payment_hash
        || i.payment_request != q.invoice
        || i.settle_date < q.created_at
    {
        return Err(Error::Invalid("asset settlement identity or expiry"));
    }
    #[derive(Deserialize)]
    struct Data {
        balances: Vec<Tranche>,
        rfq_id: String,
    }
    #[derive(Deserialize)]
    struct Tranche {
        asset_id: String,
        amount: u64,
    }
    let mut units = 0u64;
    let mut accepted_at = q.created_at;
    let mut evidence = Vec::new();
    let mut htlcs = std::collections::BTreeSet::new();
    for h in &i.htlcs {
        if h.state != 1 {
            continue;
        }
        if !htlcs.insert((h.chan_id, h.htlc_index))
            || h.custom_channel_data.len() > 64 * 1024
            || h.accept_time < q.created_at
            || h.accept_time > q.expires_at
            || h.resolve_time < h.accept_time
        {
            return Err(Error::Invalid("asset HTLC"));
        }
        accepted_at = accepted_at.max(h.accept_time);
        let d: Data = serde_json::from_slice(&h.custom_channel_data)
            .map_err(|_| Error::Invalid("asset HTLC data"))?;
        // Direct asset payments can use the sender's sell RFQ instead of the
        // invoice's buy RFQ. The original invoice and actual asset units bind
        // the credit; retain both RFQ identities for accounting.
        if openagents_wallet::parse_hash32(&d.rfq_id).is_err() {
            return Err(Error::Invalid("asset HTLC quote"));
        }
        for b in d.balances {
            if b.asset_id != q.asset_id {
                return Err(Error::Invalid("received asset identity"));
            }
            units = units
                .checked_add(b.amount)
                .ok_or(Error::Invalid("asset amount"))?;
        }
        evidence.push(json!({"channel":h.chan_id,"htlc":h.htlc_index,"amount_msat":h.amt_msat,"accepted_at":h.accept_time,"resolved_at":h.resolve_time,"data":serde_json::from_slice::<serde_json::Value>(&h.custom_channel_data).map_err(|_|Error::Invalid("asset HTLC"))?}));
    }
    if units < q.units {
        return Err(Error::Invalid("asset receive amount"));
    }
    evidence.sort_by_key(|v| {
        (
            v["channel"].as_u64().unwrap_or(0),
            v["htlc"].as_u64().unwrap_or(0),
        )
    });
    Ok(Receipt {
        settle_index: i.settle_index,
        accepted_at,
        settled_at: i.settle_date,
        units,
        evidence: serde_json::to_string(&evidence).map_err(|_| Error::Invalid("asset receipt"))?,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bitcoin_only_htlcs_wrong_assets_and_stale_quotes_cannot_credit() {
        let q = Quote {
            node: "node".into(),
            network: "regtest".into(),
            account: "a".into(),
            asset_id: "aa".repeat(32),
            edge: "edge".into(),
            rfq_id: "bb".repeat(32),
            payment_hash: "cc".repeat(32),
            invoice: "invoice".into(),
            units: 1000,
            rate: Rate {
                coefficient: "100000000".into(),
                scale: 0,
            },
            credit_msat: 1000000,
            created_at: 100,
            expires_at: 200,
        };
        let mut i = lnrpc::Invoice {
            state: 1,
            settle_index: 1,
            settle_date: 150,
            r_hash: hex::decode(&q.payment_hash).unwrap(),
            payment_request: q.invoice.clone(),
            htlcs: vec![lnrpc::InvoiceHtlc {
                state: 1,
                chan_id: 1,
                htlc_index: 1,
                accept_time: 150,
                resolve_time: 150,
                amt_msat: 1000000,
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(receipt(&q, &i).is_err());
        i.htlcs[0].custom_channel_data = serde_json::to_vec(
            &json!({"rfq_id":q.rfq_id,"balances":[{"asset_id":q.asset_id,"amount":1000}]}),
        )
        .unwrap();
        assert_eq!(receipt(&q, &i).unwrap().units, 1000);
        i.htlcs[0].custom_channel_data = serde_json::to_vec(
            &json!({"rfq_id":"dd".repeat(32),"balances":[{"asset_id":q.asset_id,"amount":1000}]}),
        )
        .unwrap();
        let actual = receipt(&q, &i).unwrap();
        assert_eq!(actual.units, 1000);
        assert!(actual.evidence.contains(&"dd".repeat(32)));
        assert_eq!(q.rate.value_msat(actual.units).unwrap(), q.credit_msat);

        i.htlcs.push(i.htlcs[0].clone());
        assert!(receipt(&q, &i).is_err());
        i.htlcs.pop();
        i.settle_date = 201;
        assert!(receipt(&q, &i).is_ok());
        i.htlcs[0].accept_time = 201;
        i.htlcs[0].resolve_time = 201;
        assert!(receipt(&q, &i).is_err());
        i.htlcs[0].accept_time = 150;
        i.htlcs[0].resolve_time = 150;
        i.settle_date = 150;
        i.htlcs[0].custom_channel_data = serde_json::to_vec(
            &json!({"rfq_id":q.rfq_id,"balances":[{"asset_id":"wrong","amount":1000}]}),
        )
        .unwrap();
        assert!(receipt(&q, &i).is_err());
    }
    #[test]
    fn mainnet_asset_operations_need_separate_approvals() {
        let mut a = Approval {
            mainnet_receive_approved: false,
            custody_review_approved: false,
            mainnet_payout_approved: false,
            asset_ids: vec!["aa".repeat(32)],
            maximum_units: 1000,
            maximum_credit_msat: 1000000,
            maximum_fee_msat: 1000,
        };
        assert!(a.receive("mainnet", &"aa".repeat(32), 100).is_err());
        a.mainnet_receive_approved = true;
        assert!(a.receive("mainnet", &"aa".repeat(32), 100).is_err());
        a.custody_review_approved = true;
        assert!(a.receive("mainnet", &"aa".repeat(32), 100).is_ok());
        assert!(a.payout("mainnet", &"aa".repeat(32), 100).is_err());
        a.mainnet_payout_approved = true;
        assert!(a.payout("mainnet", &"aa".repeat(32), 100).is_ok());
        assert!(a.payout("mainnet", "evm:asset", 100).is_err());
    }
}
