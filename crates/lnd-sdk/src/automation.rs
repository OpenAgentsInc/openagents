//! Reviewed liquidity policy and restart-safe Loop operations.
use crate::{
    Error, Result,
    liquidity::{Journal, Provider},
    macaroon::Macaroon,
    proto::looprpc as rpc,
    transport::{self, AuthChannel, Config},
};
use async_trait::async_trait;
impl crate::Client {
    pub async fn channel_details(&self) -> Result<Vec<crate::proto::lnrpc::Channel>> {
        Ok(transport::read(|| {
            let mut c = self.rpc();
            async move {
                c.list_channels(crate::proto::lnrpc::ListChannelsRequest::default())
                    .await
            }
        })
        .await?
        .channels)
    }
}

use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub version: u32,
    pub revision: String,
    pub enabled: bool,
    pub mainnet_approved: bool,
    pub hot_cap_sats: u64,
    pub cold_address: String,
    pub inbound_floor_sats: u64,
    pub outbound_floor_sats: u64,
    pub swap_sats: u64,
    pub daily_budget_sats: u64,
    pub cooldown_secs: u64,
    pub reserve_sats: u64,
    pub maximum_swap_fee_sats: u64,
    pub maximum_miner_fee_sats: u64,
    pub maximum_routing_fee_sats: u64,
    pub maximum_prepay_sats: u64,
    pub confirmation_target: i32,
    pub providers: Vec<Provider>,
    pub channels: ChannelPolicy,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelPolicy {
    pub target_count: u32,
    pub minimum_size_sats: u64,
    pub base_fee_msat: u64,
    pub fee_ppm: u32,
    pub time_lock_delta: u32,
    pub idle_close_secs: u64,
}
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Balances {
    pub hot_sats: u64,
    pub inbound_sats: u64,
    pub outbound_sats: u64,
    pub confirmed_sats: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Out,
    In,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Plan {
    Alert {
        reason: String,
    },
    Wait,
    Swap {
        direction: Direction,
        amount_sats: u64,
    },
}
impl Policy {
    pub fn parse(text: &str) -> Result<Self> {
        let p: Self = toml::from_str(text).map_err(|_| Error::Invalid("liquidity policy"))?;
        p.validate()?;
        Ok(p)
    }
    pub fn validate(&self) -> Result<()> {
        if self.version != 1
            || self.revision.is_empty()
            || self.revision.len() > 64
            || !self
                .revision
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || self.providers.len() != 2
            || self.providers[0].base() == self.providers[1].base()
            || self.cooldown_secs < 300
            || self.confirmation_target < 2
            || self.channels.target_count == 0
            || self.channels.minimum_size_sats == 0
            || self.channels.idle_close_secs < 86400
            || self.channels.time_lock_delta < 18
        {
            return Err(Error::Invalid("liquidity policy"));
        }
        self.reservation()?;
        if [
            self.swap_sats,
            self.maximum_swap_fee_sats,
            self.maximum_miner_fee_sats,
            self.maximum_routing_fee_sats,
            self.maximum_prepay_sats,
        ]
        .iter()
        .any(|n| *n > i64::MAX as u64)
            || self.maximum_prepay_sats > self.maximum_swap_fee_sats
        {
            return Err(Error::Invalid("swap budget"));
        }
        Ok(())
    }
    fn reservation(&self) -> Result<u64> {
        self.swap_sats
            .checked_add(self.maximum_swap_fee_sats)
            .and_then(|v| v.checked_add(self.maximum_miner_fee_sats))
            .and_then(|v| {
                self.maximum_routing_fee_sats
                    .checked_mul(2)
                    .and_then(|f| v.checked_add(f))
            })
            .ok_or(Error::Invalid("swap budget"))
    }
    pub fn plan(&self, network: &str, b: Balances) -> Result<Plan> {
        self.validate()?;
        // Missing owner decisions disable both directions, not just sweeping.
        if !self.enabled
            || (network == "mainnet" && !self.mainnet_approved)
            || self.hot_cap_sats == 0
            || self.cold_address.is_empty()
            || self.daily_budget_sats == 0
        {
            return Ok(Plan::Alert {
                reason: "Liquidity spending needs an approved cap, address, and budget.".into(),
            });
        }
        let net = match network {
            "mainnet" => bitcoin::Network::Bitcoin,
            "regtest" => bitcoin::Network::Regtest,
            "signet" => bitcoin::Network::Signet,
            "testnet" => bitcoin::Network::Testnet,
            _ => return Err(Error::Invalid("Bitcoin network")),
        };
        self.cold_address
            .parse::<bitcoin::Address<bitcoin::address::NetworkUnchecked>>()
            .map_err(|_| Error::Invalid("cold address"))?
            .require_network(net)
            .map_err(|_| Error::Invalid("cold address network"))?;
        if self.swap_sats == 0
            || self.maximum_swap_fee_sats == 0
            || self.maximum_miner_fee_sats == 0
            || self.reservation()? > self.daily_budget_sats
            || self.reserve_sats < 100_000
        {
            return Err(Error::Invalid("swap budget"));
        }
        let reserve = self
            .reservation()?
            .checked_add(self.reserve_sats)
            .ok_or(Error::Invalid("swap budget"))?;
        if b.hot_sats > self.hot_cap_sats || b.inbound_sats < self.inbound_floor_sats {
            if b.outbound_sats >= self.reservation()?
                && b.outbound_sats.saturating_sub(self.reservation()?) >= self.outbound_floor_sats
            {
                return Ok(Plan::Swap {
                    direction: Direction::Out,
                    amount_sats: self.swap_sats,
                });
            }
            return Ok(Plan::Alert {
                reason: "Inbound capacity is low; a Loop Out would leave too little for payouts."
                    .into(),
            });
        }
        if b.outbound_sats < self.outbound_floor_sats {
            if b.confirmed_sats >= reserve {
                return Ok(Plan::Swap {
                    direction: Direction::In,
                    amount_sats: self.swap_sats,
                });
            }
            return Ok(Plan::Alert {
                reason: "Payout capacity is low; Loop In needs more confirmed funds.".into(),
            });
        }
        Ok(Plan::Wait)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Swap {
    pub id: String,
    pub label: String,
    pub direction: Direction,
    pub amount_sats: u64,
    pub state: i32,
    pub server_fee_sats: u64,
    pub onchain_fee_sats: u64,
    pub routing_fee_sats: u64,
}
impl Swap {
    fn terminal(&self) -> bool {
        matches!(self.state, 3 | 4)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Intent {
    pub label: String,
    pub direction: Direction,
    pub amount_sats: u64,
    pub started_at: u64,
    pub reserved_sats: u64,
    pub policy: Policy,
    pub swap: Option<Swap>,
}
#[async_trait]
pub trait LoopRail: Send + Sync {
    async fn swaps(&self) -> Result<Vec<Swap>>;
    async fn quote(&self, p: &Policy, direction: Direction) -> Result<()>;
    async fn start(&self, intent: &Intent) -> Result<()>;
}
pub struct Loop {
    channel: AuthChannel,
}
impl Loop {
    pub async fn connect(config: &Config, mac: Macaroon, network: &str) -> Result<Self> {
        let s = Self {
            channel: transport::connect(config, mac).await?,
        };
        let info = transport::read(|| {
            let mut c = s.rpc();
            async move { c.get_info(rpc::GetInfoRequest {}).await }
        })
        .await?;
        if !info.version.trim_start_matches('v').starts_with("0.34.") {
            return Err(Error::VersionMismatch);
        }
        if info.network != network {
            return Err(Error::NodeMismatch);
        }
        Ok(s)
    }
    fn rpc(&self) -> rpc::swap_client_client::SwapClientClient<AuthChannel> {
        rpc::swap_client_client::SwapClientClient::new(self.channel.clone())
    }
}
#[async_trait]
impl LoopRail for Loop {
    async fn swaps(&self) -> Result<Vec<Swap>> {
        let r = transport::read(|| {
            let mut c = self.rpc();
            async move { c.list_swaps(rpc::ListSwapsRequest::default()).await }
        })
        .await?;
        if r.next_start_time != 0 {
            return Err(Error::Invalid("incomplete swap history"));
        }
        r.swaps
            .into_iter()
            .map(|s| {
                if s.id_bytes.len() != 32 || !(0..=5).contains(&s.state) {
                    return Err(Error::Invalid("swap status"));
                }
                Ok(Swap {
                    id: hex::encode(s.id_bytes),
                    label: s.label,
                    direction: match s.r#type {
                        0 => Direction::Out,
                        1 => Direction::In,
                        _ => return Err(Error::Invalid("swap direction")),
                    },
                    amount_sats: crate::error::unsigned(s.amt)?,
                    state: s.state,
                    server_fee_sats: crate::error::unsigned(s.cost_server)?,
                    onchain_fee_sats: crate::error::unsigned(s.cost_onchain)?,
                    routing_fee_sats: crate::error::unsigned(s.cost_offchain)?,
                })
            })
            .collect()
    }
    async fn quote(&self, p: &Policy, direction: Direction) -> Result<()> {
        let request = rpc::QuoteRequest {
            amt: crate::error::signed(p.swap_sats, "swap amount")?,
            conf_target: p.confirmation_target,
            ..Default::default()
        };
        let (server, miner, prepay) = match direction {
            Direction::Out => {
                let q = transport::read(|| {
                    let mut c = self.rpc();
                    let r = request.clone();
                    async move { c.loop_out_quote(r).await }
                })
                .await?;
                (q.swap_fee_sat, q.htlc_sweep_fee_sat, q.prepay_amt_sat)
            }
            Direction::In => {
                let q = transport::read(|| {
                    let mut c = self.rpc();
                    let r = request.clone();
                    async move { c.get_loop_in_quote(r).await }
                })
                .await?;
                (q.swap_fee_sat, q.htlc_publish_fee_sat, 0)
            }
        };
        if crate::error::unsigned(server)? > p.maximum_swap_fee_sats
            || crate::error::unsigned(miner)? > p.maximum_miner_fee_sats
            || crate::error::unsigned(prepay)? > p.maximum_prepay_sats
        {
            return Err(Error::Invalid("swap quote exceeds policy"));
        }
        Ok(())
    }
    async fn start(&self, i: &Intent) -> Result<()> {
        let p = &i.policy;
        let amount = crate::error::signed(i.amount_sats, "swap amount")?;
        let server = crate::error::signed(p.maximum_swap_fee_sats, "swap fee")?;
        let miner = crate::error::signed(p.maximum_miner_fee_sats, "miner fee")?;
        match i.direction {
            Direction::Out => {
                transport::once(self.rpc().loop_out(rpc::LoopOutRequest {
                    amt: amount,
                    dest: p.cold_address.clone(),
                    is_external_addr: true,
                    max_swap_fee: server,
                    max_miner_fee: miner,
                    max_prepay_amt: crate::error::signed(p.maximum_prepay_sats, "prepay")?,
                    max_swap_routing_fee: crate::error::signed(
                        p.maximum_routing_fee_sats,
                        "routing fee",
                    )?,
                    max_prepay_routing_fee: crate::error::signed(
                        p.maximum_routing_fee_sats,
                        "routing fee",
                    )?,
                    sweep_conf_target: p.confirmation_target,
                    htlc_confirmations: 3,
                    label: i.label.clone(),
                    initiator: "openagents".into(),
                    ..Default::default()
                }))
                .await?;
            }
            Direction::In => {
                transport::once(self.rpc().loop_in(rpc::LoopInRequest {
                    amt: amount,
                    max_swap_fee: server,
                    max_miner_fee: miner,
                    htlc_conf_target: p.confirmation_target,
                    label: i.label.clone(),
                    initiator: "openagents".into(),
                    ..Default::default()
                }))
                .await?;
            }
        }
        Ok(())
    }
}
/// Record all daemon swaps, then match managed intents by label and immutable terms.
/// An absent or ambiguous intent never authorizes a replacement send.
pub async fn reconcile(j: &mut Journal, rail: &impl LoopRail) -> Result<bool> {
    let swaps = rail.swaps().await?;
    for s in &swaps {
        j.save(
            &format!("observed:{}", s.id),
            serde_json::to_value(s).map_err(|_| Error::Checkpoint)?,
        )?;
    }
    let keys: Vec<_> = j
        .entries
        .keys()
        .filter(|k| k.starts_with("swap:"))
        .cloned()
        .collect();
    let mut pending = swaps.iter().any(|s| !s.terminal());
    for key in keys {
        let mut i: Intent =
            serde_json::from_value(j.entries[&key].clone()).map_err(|_| Error::Checkpoint)?;
        let found: Vec<_> = swaps.iter().filter(|s| s.label == i.label).collect();
        if found.len() != 1 {
            pending = true;
            continue;
        }
        let s = found[0];
        if s.amount_sats != i.amount_sats
            || s.direction != i.direction
            || i.swap.as_ref().is_some_and(|old| old.id != s.id)
        {
            return Err(Error::Invalid("saved swap identity"));
        }
        pending |= !s.terminal();
        i.swap = Some(s.clone());
        j.save(
            &key,
            serde_json::to_value(i).map_err(|_| Error::Checkpoint)?,
        )?;
    }
    Ok(pending)
}
pub async fn tick(
    j: &mut Journal,
    rail: &impl LoopRail,
    p: &Policy,
    network: &str,
    b: Balances,
    now: u64,
) -> Result<Plan> {
    if reconcile(j, rail).await? {
        return Ok(Plan::Alert {
            reason: "A swap is still pending or needs review.".into(),
        });
    }
    let plan = p.plan(network, b)?;
    let Plan::Swap {
        direction,
        amount_sats,
    } = plan
    else {
        return Ok(plan);
    };
    let mut reserved = 0u64;
    for (key, value) in &j.entries {
        if !key.starts_with("swap:") {
            continue;
        }
        let i: Intent = serde_json::from_value(value.clone()).map_err(|_| Error::Checkpoint)?;
        if now < i.started_at.saturating_add(p.cooldown_secs) {
            return Ok(Plan::Wait);
        }
        if now / 86400 == i.started_at / 86400 {
            let charged = if let Some(s) = &i.swap {
                s.server_fee_sats
                    .checked_add(s.onchain_fee_sats)
                    .and_then(|v| v.checked_add(s.routing_fee_sats))
                    .and_then(|v| v.checked_add(i.amount_sats))
                    .ok_or(Error::Invalid("actual swap costs"))?
                    .max(i.reserved_sats)
            } else {
                i.reserved_sats
            };
            reserved = reserved
                .checked_add(charged)
                .ok_or(Error::Invalid("daily swap budget"))?;
        }
    }
    let reservation = p.reservation()?;
    if reserved
        .checked_add(reservation)
        .is_none_or(|v| v > p.daily_budget_sats)
    {
        return Ok(Plan::Alert {
            reason: "Today's liquidity budget is used up.".into(),
        });
    }
    rail.quote(p, direction).await?;
    let label = format!("oa-{}-{now}", p.revision);
    let i = Intent {
        label: label.clone(),
        direction,
        amount_sats,
        started_at: now,
        reserved_sats: reservation,
        policy: p.clone(),
        swap: None,
    };
    let key = format!("swap:{label}");
    if j.entries.contains_key(&key) {
        return Err(Error::InFlight);
    }
    j.save(
        &key,
        serde_json::to_value(&i).map_err(|_| Error::Checkpoint)?,
    )?;
    rail.start(&i).await?;
    reconcile(j, rail).await?;
    Ok(plan)
}
/// Bind history to a particular node and Loop certificate, across policy revisions.
pub fn bind(j: &mut Journal, node: &str, network: &str, loop_pin: &str) -> Result<()> {
    let binding = json!({"node":node,"network":network,"loop_pin":loop_pin});
    if let Some(old) = j.entries.get("binding") {
        if old != &binding {
            return Err(Error::NodeMismatch);
        }
    } else {
        j.save("binding", binding)?;
    }
    Ok(())
}

/// Keep activity observations across restarts; report channel changes for review.
pub fn channel_review(
    j: &mut Journal,
    p: &ChannelPolicy,
    channels: &[crate::proto::lnrpc::Channel],
    now: u64,
) -> Result<serde_json::Value> {
    let mut review = Vec::new();
    for c in channels {
        if c.chan_id == 0 || c.capacity < 0 {
            return Err(Error::Invalid("channel"));
        }
        let key = format!("channel:{}", c.channel_point);
        let totals = json!([c.total_satoshis_sent, c.total_satoshis_received]);
        let last = j
            .entries
            .get(&key)
            .filter(|v| v["totals"] == totals)
            .and_then(|v| v["last_activity"].as_u64())
            .unwrap_or(now);
        j.save(&key, json!({"totals":totals,"last_activity":last}))?;
        review.push(json!({"channel":c.channel_point,"below_minimum":(c.capacity as u64) < p.minimum_size_sats,
            "idle_close_candidate":channels.len() > p.target_count as usize && c.active && c.pending_htlcs.is_empty() && now.saturating_sub(last) >= p.idle_close_secs,
            "fee_schedule":{"base_fee_msat":p.base_fee_msat,"fee_ppm":p.fee_ppm,"time_lock_delta":p.time_lock_delta}}));
    }
    Ok(
        json!({"open_count":p.target_count.saturating_sub(channels.len().try_into().unwrap_or(u32::MAX)),"minimum_size_sats":p.minimum_size_sats,"channels":review}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    #[derive(Default)]
    struct Fake {
        swaps: Mutex<Vec<Swap>>,
        sends: Mutex<usize>,
        interrupted: bool,
    }
    #[async_trait]
    impl LoopRail for Fake {
        async fn swaps(&self) -> Result<Vec<Swap>> {
            Ok(self.swaps.lock().unwrap().clone())
        }
        async fn quote(&self, _: &Policy, _: Direction) -> Result<()> {
            Ok(())
        }
        async fn start(&self, i: &Intent) -> Result<()> {
            *self.sends.lock().unwrap() += 1;
            if self.interrupted {
                return Err(Error::Timeout);
            }
            self.swaps.lock().unwrap().push(Swap {
                id: "ab".repeat(32),
                label: i.label.clone(),
                direction: i.direction,
                amount_sats: i.amount_sats,
                state: 3,
                server_fee_sats: 10,
                onchain_fee_sats: 5,
                routing_fee_sats: 1,
            });
            Ok(())
        }
    }
    fn policy() -> Policy {
        let mut p = Policy::parse(include_str!("../../../deploy/lnd/channel-policy.toml")).unwrap();
        p.enabled = true;
        p.mainnet_approved = true;
        p.hot_cap_sats = 2_000_000;
        let key = bitcoin::secp256k1::SecretKey::from_slice(&[17; 32]).unwrap();
        let (public, _) = key.x_only_public_key(&bitcoin::secp256k1::Secp256k1::new());
        p.cold_address = bitcoin::Address::p2tr(
            &bitcoin::secp256k1::Secp256k1::new(),
            public,
            None,
            bitcoin::Network::Bitcoin,
        )
        .to_string();
        p.swap_sats = 100_000;
        p.maximum_swap_fee_sats = 1000;
        p.maximum_miner_fee_sats = 2000;
        p.maximum_routing_fee_sats = 100;
        p.daily_budget_sats = 200_000;
        p
    }
    fn balances() -> Balances {
        Balances {
            hot_sats: 3_000_000,
            inbound_sats: 0,
            outbound_sats: 1_000_000,
            confirmed_sats: 1_000_000,
        }
    }
    #[test]
    fn missing_owner_decisions_and_mismatched_addresses_cannot_spend() {
        let original =
            Policy::parse(include_str!("../../../deploy/lnd/channel-policy.toml")).unwrap();
        assert!(matches!(
            original.plan("mainnet", balances()).unwrap(),
            Plan::Alert { .. }
        ));
        let mut p = policy();
        p.hot_cap_sats = 0;
        assert!(matches!(
            p.plan("mainnet", balances()).unwrap(),
            Plan::Alert { .. }
        ));
        p = policy();
        p.mainnet_approved = false;
        assert!(matches!(
            p.plan("mainnet", balances()).unwrap(),
            Plan::Alert { .. }
        ));
        p = policy();
        assert!(p.plan("regtest", balances()).is_err());
        p = policy();
        p.providers[1] = Provider::Zeus;
        assert!(p.validate().is_err());
        assert!(Policy::parse(&(toml::to_string(&policy()).unwrap() + "\nunknown=1")).is_err());
    }
    #[test]
    fn planner_preserves_payout_capacity_and_confirmed_reserve() {
        let p = policy();
        let mut b = balances();
        assert!(matches!(
            p.plan("mainnet", b).unwrap(),
            Plan::Swap {
                direction: Direction::Out,
                ..
            }
        ));
        b.outbound_sats = 110_000;
        assert!(matches!(p.plan("mainnet", b).unwrap(), Plan::Alert { .. }));
        b.hot_sats = 1_000_000;
        b.inbound_sats = 1_000_000;
        b.outbound_sats = 0;
        assert!(matches!(
            p.plan("mainnet", b).unwrap(),
            Plan::Swap {
                direction: Direction::In,
                ..
            }
        ));
        b.confirmed_sats = 200_000;
        assert!(matches!(p.plan("mainnet", b).unwrap(), Plan::Alert { .. }));
    }
    #[tokio::test]
    async fn interrupted_start_is_never_repeated_after_restart() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("loop.json");
        let p = policy();
        let f = Fake {
            interrupted: true,
            ..Default::default()
        };
        {
            let mut j = Journal::open(&path).unwrap();
            bind(&mut j, "node", "mainnet", "pin").unwrap();
            assert_eq!(
                tick(&mut j, &f, &p, "mainnet", balances(), 86400).await,
                Err(Error::Timeout)
            );
        }
        let mut j = Journal::open(&path).unwrap();
        assert!(bind(&mut j, "other", "mainnet", "pin").is_err());
        assert!(matches!(
            tick(&mut j, &f, &p, "mainnet", balances(), 200000)
                .await
                .unwrap(),
            Plan::Alert { .. }
        ));
        assert_eq!(*f.sends.lock().unwrap(), 1);
        let i: Intent = serde_json::from_value(
            j.entries
                .values()
                .find(|v| v.get("label").is_some())
                .unwrap()
                .clone(),
        )
        .unwrap();
        f.swaps.lock().unwrap().push(Swap {
            id: "ab".repeat(32),
            label: i.label,
            direction: i.direction,
            amount_sats: i.amount_sats,
            state: 3,
            server_fee_sats: 10,
            onchain_fee_sats: 5,
            routing_fee_sats: 1,
        });
        assert!(!reconcile(&mut j, &f).await.unwrap());
        let observed = &j.entries[&format!("observed:{}", "ab".repeat(32))];
        assert_eq!(observed["server_fee_sats"], 10);
    }
    #[tokio::test]
    async fn terminal_swap_still_consumes_daily_budget_across_policy_revisions() {
        let d = tempfile::tempdir().unwrap();
        let mut j = Journal::open(&d.path().join("swaps")).unwrap();
        let f = Fake::default();
        let mut p = policy();
        p.cooldown_secs = 300;
        tick(&mut j, &f, &p, "mainnet", balances(), 86400)
            .await
            .unwrap();
        p.revision = "new-review".into();
        assert!(matches!(
            tick(&mut j, &f, &p, "mainnet", balances(), 87000)
                .await
                .unwrap(),
            Plan::Alert { .. }
        ));
        assert_eq!(*f.sends.lock().unwrap(), 1);
        f.swaps.lock().unwrap()[0].amount_sats += 1;
        assert!(reconcile(&mut j, &f).await.is_err());
    }
    #[test]
    fn idle_close_needs_observed_inactivity_and_excess_channels() {
        let d = tempfile::tempdir().unwrap();
        let mut j = Journal::open(&d.path().join("channels")).unwrap();
        let mut p = policy().channels;
        p.target_count = 1;
        p.idle_close_secs = 86400;
        let c = crate::proto::lnrpc::Channel {
            chan_id: 1,
            channel_point: "aa:0".into(),
            capacity: 2_000_000,
            active: true,
            ..Default::default()
        };
        let mut channels = vec![
            c.clone(),
            crate::proto::lnrpc::Channel {
                chan_id: 2,
                channel_point: "bb:0".into(),
                ..c
            },
        ];
        assert_eq!(
            channel_review(&mut j, &p, &channels, 1).unwrap()["channels"][0]["idle_close_candidate"],
            false
        );
        assert_eq!(
            channel_review(&mut j, &p, &channels, 86402).unwrap()["channels"][0]["idle_close_candidate"],
            true
        );
        channels[0].total_satoshis_sent = 1;
        assert_eq!(
            channel_review(&mut j, &p, &channels, 86403).unwrap()["channels"][0]["idle_close_candidate"],
            false
        );
    }
}
