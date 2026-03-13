use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, UNIX_EPOCH};

use crate::app_state::{JobHistoryState, NetworkRequestsState, PaneLoadState};
use crate::nip90_compute_flow::{Nip90FlowPhase, build_buyer_request_flow_snapshot};
use crate::spark_wallet::{SparkPaneState, is_settled_wallet_payment_status};
use crate::state::operations::{
    NetworkRequestProviderObservationHistoryEvent, NetworkRequestProviderObservationHistoryKind,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const NIP90_PAYMENT_FACT_SCHEMA_VERSION: u16 = 1;
const NIP90_PAYMENT_FACT_STREAM_ID: &str = "stream.nip90_payment_facts.v1";
const NIP90_PAYMENT_FACT_ROW_LIMIT: usize = 4096;
const NIP90_ACTOR_ROW_LIMIT: usize = 4096;
const NIP90_RELAY_HOP_ROW_LIMIT: usize = 8192;
const LIVE_PROJECTION_REFRESH_INTERVAL: Duration = Duration::from_millis(250);
const LOG_BACKFILL_REFRESH_INTERVAL: Duration = Duration::from_secs(10);
const STARTUP_LOG_BACKFILL_DELAY: Duration = Duration::from_secs(8);
const NIP90_ACTOR_ROLE_BUYER: u32 = 1 << 0;
const NIP90_ACTOR_ROLE_PROVIDER: u32 = 1 << 1;
const NIP90_ACTOR_ROLE_INVOICE_PROVIDER: u32 = 1 << 2;
const NIP90_ACTOR_ROLE_RESULT_PROVIDER: u32 = 1 << 3;
const NIP90_ACTOR_ROLE_LIGHTNING_DESTINATION: u32 = 1 << 4;
const NIP90_ACTOR_ROLE_LOCAL: u32 = 1 << 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, Ord, PartialOrd, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Nip90ActorNamespace {
    Nostr,
    LightningDestination,
}

impl Nip90ActorNamespace {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Nostr => "nostr",
            Self::LightningDestination => "lightning_destination",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, Ord, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum Nip90PaymentFactStatus {
    RequestPublished,
    ResultObserved,
    InvoiceObserved,
    BuyerPaymentPending,
    BuyerWalletSettled,
    SellerSettlementObserved,
    SellerWalletSettled,
    Failed,
}

impl Nip90PaymentFactStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::RequestPublished => "request-published",
            Self::ResultObserved => "result-observed",
            Self::InvoiceObserved => "invoice-observed",
            Self::BuyerPaymentPending => "buyer-payment-pending",
            Self::BuyerWalletSettled => "buyer-wallet-settled",
            Self::SellerSettlementObserved => "seller-settlement-observed",
            Self::SellerWalletSettled => "seller-wallet-settled",
            Self::Failed => "failed",
        }
    }

    const fn rank(self) -> u8 {
        match self {
            Self::Failed => 0,
            Self::RequestPublished => 1,
            Self::ResultObserved => 2,
            Self::InvoiceObserved => 3,
            Self::BuyerPaymentPending => 4,
            Self::BuyerWalletSettled => 5,
            Self::SellerSettlementObserved => 6,
            Self::SellerWalletSettled => 7,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, Ord, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum Nip90PaymentFactSourceQuality {
    RequestProjection,
    BuyerWalletReconciled,
    SellerReceiptProjection,
    SellerWalletReconciled,
    LogBackfill,
}

impl Nip90PaymentFactSourceQuality {
    pub const fn label(self) -> &'static str {
        match self {
            Self::RequestProjection => "request-projection",
            Self::BuyerWalletReconciled => "buyer-wallet-reconciled",
            Self::SellerReceiptProjection => "seller-receipt-projection",
            Self::SellerWalletReconciled => "seller-wallet-reconciled",
            Self::LogBackfill => "log-backfill",
        }
    }

    const fn rank(self) -> u8 {
        match self {
            Self::RequestProjection => 1,
            Self::BuyerWalletReconciled => 2,
            Self::SellerReceiptProjection => 3,
            Self::SellerWalletReconciled => 4,
            Self::LogBackfill => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Nip90RelayHopKind {
    RequestIngress,
    ResultIngress,
    InvoiceIngress,
    PublishAccepted,
    PublishRejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Nip90RelayHopDirection {
    Inbound,
    Outbound,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Nip90Actor {
    pub actor_id: String,
    pub namespace: Nip90ActorNamespace,
    pub pubkey: String,
    pub display_label: String,
    pub is_local: bool,
    pub role_mask: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Nip90RelayHop {
    pub request_id: String,
    pub event_id: String,
    pub hop_kind: Nip90RelayHopKind,
    pub relay_url: String,
    pub direction: Nip90RelayHopDirection,
    pub accepted: bool,
    pub observed_at: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Nip90PaymentFact {
    pub fact_id: String,
    pub request_id: String,
    pub request_type: String,
    pub request_event_id: Option<String>,
    pub result_event_id: Option<String>,
    pub invoice_event_id: Option<String>,
    pub seller_feedback_event_id: Option<String>,
    pub buyer_nostr_pubkey: Option<String>,
    pub provider_nostr_pubkey: Option<String>,
    pub invoice_provider_pubkey: Option<String>,
    pub result_provider_pubkey: Option<String>,
    pub invoice_observed_relays: Vec<String>,
    pub result_observed_relays: Vec<String>,
    pub lightning_destination_pubkey: Option<String>,
    pub buyer_payment_pointer: Option<String>,
    pub seller_payment_pointer: Option<String>,
    pub buyer_payment_hash: Option<String>,
    pub amount_sats: Option<u64>,
    pub fees_sats: Option<u64>,
    pub total_debit_sats: Option<u64>,
    pub wallet_method: Option<String>,
    pub status: Nip90PaymentFactStatus,
    pub settlement_authority: String,
    pub request_published_at: Option<u64>,
    pub result_observed_at: Option<u64>,
    pub invoice_observed_at: Option<u64>,
    pub buyer_payment_pointer_at: Option<u64>,
    pub seller_settlement_feedback_at: Option<u64>,
    pub buyer_wallet_confirmed_at: Option<u64>,
    pub seller_wallet_confirmed_at: Option<u64>,
    pub selected_relays: Vec<String>,
    pub publish_accepted_relays: Vec<String>,
    pub publish_rejected_relays: Vec<String>,
    #[serde(default)]
    pub provider_observation_history: Vec<NetworkRequestProviderObservationHistoryEvent>,
    pub source_quality: Nip90PaymentFactSourceQuality,
}

impl Nip90PaymentFact {
    pub fn latest_event_epoch_seconds(&self) -> Option<u64> {
        [
            self.seller_wallet_confirmed_at,
            self.seller_settlement_feedback_at,
            self.buyer_wallet_confirmed_at,
            self.buyer_payment_pointer_at,
            self.invoice_observed_at,
            self.result_observed_at,
            self.request_published_at,
        ]
        .into_iter()
        .flatten()
        .max()
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Nip90PaymentFactDocumentV1 {
    schema_version: u16,
    stream_id: String,
    facts: Vec<Nip90PaymentFact>,
    actors: Vec<Nip90Actor>,
    relay_hops: Vec<Nip90RelayHop>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct SessionLogBackfillCache {
    signature: Option<String>,
    facts: Vec<Nip90PaymentFact>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct SessionLogBackfillRefresh {
    changed: bool,
    fact_count: usize,
    notice: Option<String>,
}

impl SessionLogBackfillRefresh {
    fn decorate_action(&self, action: impl Into<String>) -> String {
        let action = action.into();
        let mut detail = format!("{} log-backfill facts cached", self.fact_count);
        if let Some(notice) = self.notice.as_deref() {
            detail.push_str("; ");
            detail.push_str(notice);
        }
        format!("{action} ({detail})")
    }
}

pub struct Nip90PaymentFactLedgerState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub facts: Vec<Nip90PaymentFact>,
    pub actors: Vec<Nip90Actor>,
    pub relay_hops: Vec<Nip90RelayHop>,
    ledger_path: PathBuf,
    log_backfill: SessionLogBackfillCache,
    next_live_projection_refresh_at: Option<Instant>,
    next_log_backfill_refresh_at: Option<Instant>,
}

impl Default for Nip90PaymentFactLedgerState {
    fn default() -> Self {
        Self::from_path(default_nip90_payment_fact_path())
    }
}

impl Nip90PaymentFactLedgerState {
    fn from_path(ledger_path: PathBuf) -> Self {
        let (facts, actors, relay_hops, load_state, last_error, last_action) =
            match load_nip90_payment_fact_document(ledger_path.as_path()) {
                Ok(document) => (
                    document.facts,
                    document.actors,
                    document.relay_hops,
                    PaneLoadState::Ready,
                    None,
                    Some("Loaded NIP-90 payment fact ledger".to_string()),
                ),
                Err(error) => (
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    PaneLoadState::Error,
                    Some(error),
                    Some("NIP-90 payment fact ledger load failed".to_string()),
                ),
            };
        Self {
            load_state,
            last_error,
            last_action: last_action.map(|action| format!("{action} ({} facts)", facts.len())),
            facts,
            actors,
            relay_hops,
            ledger_path,
            log_backfill: SessionLogBackfillCache::default(),
            next_live_projection_refresh_at: None,
            next_log_backfill_refresh_at: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn from_path_for_tests(ledger_path: PathBuf) -> Self {
        Self::from_path(ledger_path)
    }

    pub fn recent_facts(&self, limit: usize) -> Vec<&Nip90PaymentFact> {
        self.facts.iter().take(limit).collect()
    }

    pub fn fact_for_request(&self, request_id: &str) -> Option<&Nip90PaymentFact> {
        self.facts.iter().find(|fact| fact.request_id == request_id)
    }

    pub fn facts_for_actor(
        &self,
        namespace: Nip90ActorNamespace,
        pubkey: &str,
    ) -> Vec<&Nip90PaymentFact> {
        let normalized = normalize_pubkey_key(pubkey);
        self.facts
            .iter()
            .filter(|fact| match namespace {
                Nip90ActorNamespace::Nostr => [
                    fact.buyer_nostr_pubkey.as_deref(),
                    fact.provider_nostr_pubkey.as_deref(),
                    fact.invoice_provider_pubkey.as_deref(),
                    fact.result_provider_pubkey.as_deref(),
                ]
                .into_iter()
                .flatten()
                .any(|value| normalize_pubkey_key(value) == normalized),
                Nip90ActorNamespace::LightningDestination => fact
                    .lightning_destination_pubkey
                    .as_deref()
                    .is_some_and(|value| normalize_pubkey_key(value) == normalized),
            })
            .collect()
    }

    pub fn sync_from_current_truth(
        &mut self,
        network_requests: &NetworkRequestsState,
        job_history: &JobHistoryState,
        spark_wallet: &SparkPaneState,
        local_nostr_pubkey_hex: Option<&str>,
    ) -> bool {
        let now = Instant::now();
        self.next_live_projection_refresh_at = Some(now + LIVE_PROJECTION_REFRESH_INTERVAL);
        self.next_log_backfill_refresh_at = Some(now + LOG_BACKFILL_REFRESH_INTERVAL);
        self.sync_from_current_truth_with_session_log_dir(
            network_requests,
            job_history,
            spark_wallet,
            local_nostr_pubkey_hex,
            session_log_backfill_dir().as_path(),
            true,
        )
    }

    pub fn sync_from_background_tick(
        &mut self,
        network_requests: &NetworkRequestsState,
        job_history: &JobHistoryState,
        spark_wallet: &SparkPaneState,
        local_nostr_pubkey_hex: Option<&str>,
        now: Instant,
    ) -> bool {
        self.sync_from_background_tick_with_session_log_dir(
            network_requests,
            job_history,
            spark_wallet,
            local_nostr_pubkey_hex,
            session_log_backfill_dir().as_path(),
            now,
        )
    }

    fn sync_from_background_tick_with_session_log_dir(
        &mut self,
        network_requests: &NetworkRequestsState,
        job_history: &JobHistoryState,
        spark_wallet: &SparkPaneState,
        local_nostr_pubkey_hex: Option<&str>,
        session_log_dir: &Path,
        now: Instant,
    ) -> bool {
        if self.next_log_backfill_refresh_at.is_none() {
            self.next_log_backfill_refresh_at = Some(now + STARTUP_LOG_BACKFILL_DELAY);
        }

        let backfill_due = self
            .next_log_backfill_refresh_at
            .is_some_and(|deadline| now >= deadline);
        if backfill_due {
            self.next_log_backfill_refresh_at = Some(now + LOG_BACKFILL_REFRESH_INTERVAL);
            self.next_live_projection_refresh_at = Some(now + LIVE_PROJECTION_REFRESH_INTERVAL);
            return self.sync_from_current_truth_with_session_log_dir(
                network_requests,
                job_history,
                spark_wallet,
                local_nostr_pubkey_hex,
                session_log_dir,
                true,
            );
        }

        if self
            .next_live_projection_refresh_at
            .is_some_and(|deadline| now < deadline)
        {
            return false;
        }

        self.next_live_projection_refresh_at = Some(now + LIVE_PROJECTION_REFRESH_INTERVAL);
        self.sync_from_current_truth_with_session_log_dir(
            network_requests,
            job_history,
            spark_wallet,
            local_nostr_pubkey_hex,
            session_log_dir,
            false,
        )
    }

    fn sync_from_current_truth_with_session_log_dir(
        &mut self,
        network_requests: &NetworkRequestsState,
        job_history: &JobHistoryState,
        spark_wallet: &SparkPaneState,
        local_nostr_pubkey_hex: Option<&str>,
        session_log_dir: &Path,
        refresh_log_backfill: bool,
    ) -> bool {
        let local_nostr_pubkey_hex = local_nostr_pubkey_hex
            .map(normalize_pubkey_key)
            .filter(|value| !value.is_empty());
        let log_backfill_refresh = if refresh_log_backfill {
            self.refresh_log_backfill_cache(session_log_dir, local_nostr_pubkey_hex.as_deref())
        } else {
            SessionLogBackfillRefresh {
                changed: false,
                fact_count: self.log_backfill.facts.len(),
                notice: None,
            }
        };
        let mut facts_by_request = BTreeMap::<String, Nip90PaymentFact>::new();
        for fact in self.facts.iter().cloned() {
            merge_fact(&mut facts_by_request, fact);
        }
        for fact in self.log_backfill.facts.iter().cloned() {
            merge_fact(&mut facts_by_request, fact);
        }

        for snapshot in network_requests
            .submitted
            .iter()
            .map(|request| build_buyer_request_flow_snapshot(request, spark_wallet))
        {
            let fact = buyer_fact_from_snapshot(&snapshot, local_nostr_pubkey_hex.as_deref());
            merge_fact(&mut facts_by_request, fact);
        }

        let seller_wallet_by_pointer = spark_wallet
            .recent_payments
            .iter()
            .filter(|payment| {
                payment.direction.eq_ignore_ascii_case("receive")
                    && is_settled_wallet_payment_status(payment.status.as_str())
            })
            .map(|payment| (payment.id.as_str(), payment))
            .collect::<BTreeMap<_, _>>();
        for row in &job_history.rows {
            let wallet_payment = seller_wallet_by_pointer
                .get(row.payment_pointer.as_str())
                .copied();
            let fact = seller_fact_from_history_row(
                row,
                wallet_payment,
                local_nostr_pubkey_hex.as_deref(),
            );
            merge_fact(&mut facts_by_request, fact);
        }

        let facts = normalize_facts(facts_by_request.into_values().collect());
        let actors = derive_actors(facts.as_slice(), local_nostr_pubkey_hex.as_deref());
        let relay_hops = derive_relay_hops(facts.as_slice());

        if self.facts == facts && self.actors == actors && self.relay_hops == relay_hops {
            let mut changed = false;
            if self.load_state == PaneLoadState::Loading {
                self.load_state = PaneLoadState::Ready;
                changed = true;
            }
            if self.last_error.take().is_some() {
                changed = true;
            }
            if log_backfill_refresh.changed || log_backfill_refresh.notice.is_some() {
                let action = log_backfill_refresh
                    .decorate_action("NIP-90 payment facts unchanged after sync");
                if self.last_action.as_deref() != Some(action.as_str()) {
                    self.last_action = Some(action);
                    changed = true;
                }
            }
            return changed;
        }

        let fact_count = facts.len();
        self.facts = facts;
        self.actors = actors;
        self.relay_hops = relay_hops;
        if let Err(error) = persist_nip90_payment_fact_document(
            self.ledger_path.as_path(),
            self.facts.as_slice(),
            self.actors.as_slice(),
            self.relay_hops.as_slice(),
        ) {
            let had_same_error = self.last_error.as_deref() == Some(error.as_str());
            self.last_error = Some(error);
            self.load_state = PaneLoadState::Error;
            self.last_action = Some("NIP-90 payment fact persist failed".to_string());
            return !had_same_error;
        }

        self.last_error = None;
        self.load_state = PaneLoadState::Ready;
        self.last_action = Some(log_backfill_refresh.decorate_action(format!(
            "Rebuilt NIP-90 payment fact ledger ({} facts)",
            fact_count
        )));
        true
    }

    fn refresh_log_backfill_cache(
        &mut self,
        session_log_dir: &Path,
        local_nostr_pubkey_hex: Option<&str>,
    ) -> SessionLogBackfillRefresh {
        let signature = match session_log_directory_signature(session_log_dir) {
            Ok(signature) => signature,
            Err(error) => {
                return SessionLogBackfillRefresh {
                    changed: false,
                    fact_count: self.log_backfill.facts.len(),
                    notice: Some(format!("session-log backfill unavailable: {error}")),
                };
            }
        };

        if self.log_backfill.signature == signature {
            return SessionLogBackfillRefresh {
                changed: false,
                fact_count: self.log_backfill.facts.len(),
                notice: None,
            };
        }

        let previous_had_cache = self.log_backfill.signature.is_some();
        match signature {
            None => {
                self.log_backfill.signature = None;
                self.log_backfill.facts.clear();
                SessionLogBackfillRefresh {
                    changed: previous_had_cache,
                    fact_count: 0,
                    notice: if previous_had_cache {
                        Some("session-log backfill cache cleared".to_string())
                    } else {
                        None
                    },
                }
            }
            Some(signature) => match load_log_backfill_facts_from_session_dir(
                session_log_dir,
                local_nostr_pubkey_hex,
            ) {
                Ok(facts) => {
                    let changed = self.log_backfill.facts != facts;
                    self.log_backfill.signature = Some(signature);
                    self.log_backfill.facts = facts;
                    SessionLogBackfillRefresh {
                        changed,
                        fact_count: self.log_backfill.facts.len(),
                        notice: None,
                    }
                }
                Err(error) => SessionLogBackfillRefresh {
                    changed: false,
                    fact_count: self.log_backfill.facts.len(),
                    notice: Some(format!("session-log backfill import failed: {error}")),
                },
            },
        }
    }
}

fn default_nip90_payment_fact_path() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".openagents")
        .join("autopilot-nip90-payment-facts-v1.json")
}

fn session_log_backfill_dir() -> PathBuf {
    crate::runtime_log::autopilot_log_dir().join("sessions")
}

fn buyer_fact_from_snapshot(
    snapshot: &crate::nip90_compute_flow::BuyerRequestFlowSnapshot,
    local_nostr_pubkey_hex: Option<&str>,
) -> Nip90PaymentFact {
    let status = match snapshot.phase {
        Nip90FlowPhase::Failed => Nip90PaymentFactStatus::Failed,
        Nip90FlowPhase::SellerSettledPendingWallet => {
            Nip90PaymentFactStatus::SellerSettlementObserved
        }
        Nip90FlowPhase::Paid => {
            if snapshot.seller_success_feedback_event_id.is_some() {
                Nip90PaymentFactStatus::SellerSettlementObserved
            } else {
                Nip90PaymentFactStatus::BuyerWalletSettled
            }
        }
        Nip90FlowPhase::AwaitingPayment => {
            if snapshot.payment_pointer.is_some() {
                Nip90PaymentFactStatus::BuyerPaymentPending
            } else {
                Nip90PaymentFactStatus::InvoiceObserved
            }
        }
        Nip90FlowPhase::RequestingPayment => Nip90PaymentFactStatus::InvoiceObserved,
        Nip90FlowPhase::Delivered | Nip90FlowPhase::DeliveredUnpaid => {
            Nip90PaymentFactStatus::ResultObserved
        }
        Nip90FlowPhase::Preview
        | Nip90FlowPhase::Submitted
        | Nip90FlowPhase::Accepted
        | Nip90FlowPhase::Executing
        | Nip90FlowPhase::PublishingResult => Nip90PaymentFactStatus::RequestPublished,
    };
    let source_quality = if snapshot.payment_pointer.is_some() && snapshot.payment_hash.is_some() {
        Nip90PaymentFactSourceQuality::BuyerWalletReconciled
    } else {
        Nip90PaymentFactSourceQuality::RequestProjection
    };
    let result_observed_at = first_provider_history_epoch_seconds(
        snapshot.provider_observation_history.as_slice(),
        |event| event.kind == NetworkRequestProviderObservationHistoryKind::ResultObserved,
    );
    let invoice_observed_at = merge_timestamp(
        first_provider_history_epoch_seconds(
            snapshot.provider_observation_history.as_slice(),
            |event| provider_history_event_has_invoice_signal(event),
        ),
        snapshot.payment_required_at_epoch_seconds,
    );
    let seller_settlement_feedback_at = first_provider_history_epoch_seconds(
        snapshot.provider_observation_history.as_slice(),
        |event| provider_history_event_is_seller_settlement(event),
    );

    Nip90PaymentFact {
        fact_id: payment_fact_id(snapshot.request_id.as_str()),
        request_id: snapshot.request_id.clone(),
        request_type: snapshot.request_type.clone(),
        request_event_id: normalize_optional_string(snapshot.published_request_event_id.as_deref()),
        result_event_id: normalize_optional_string(
            snapshot
                .winning_result_event_id
                .as_deref()
                .or(snapshot.last_result_event_id.as_deref()),
        ),
        invoice_event_id: normalize_optional_string(snapshot.last_feedback_event_id.as_deref()),
        seller_feedback_event_id: normalize_optional_string(
            snapshot.seller_success_feedback_event_id.as_deref(),
        ),
        buyer_nostr_pubkey: local_nostr_pubkey_hex.map(ToString::to_string),
        provider_nostr_pubkey: normalize_optional_string(
            snapshot.payable_provider_pubkey.as_deref(),
        )
        .or_else(|| normalize_optional_string(snapshot.result_provider_pubkey.as_deref()))
        .or_else(|| normalize_optional_string(snapshot.invoice_provider_pubkey.as_deref()))
        .or_else(|| normalize_optional_string(snapshot.selected_provider_pubkey.as_deref())),
        invoice_provider_pubkey: normalize_optional_string(
            snapshot.invoice_provider_pubkey.as_deref(),
        ),
        result_provider_pubkey: normalize_optional_string(
            snapshot.result_provider_pubkey.as_deref(),
        ),
        invoice_observed_relays: snapshot.invoice_relay_urls.clone(),
        result_observed_relays: snapshot.result_relay_urls.clone(),
        lightning_destination_pubkey: normalize_optional_string(
            snapshot.destination_pubkey.as_deref(),
        ),
        buyer_payment_pointer: normalize_optional_string(snapshot.payment_pointer.as_deref()),
        seller_payment_pointer: None,
        buyer_payment_hash: normalize_optional_string(snapshot.payment_hash.as_deref()),
        amount_sats: snapshot.invoice_amount_sats,
        fees_sats: snapshot.fees_sats,
        total_debit_sats: snapshot.total_debit_sats,
        wallet_method: normalize_optional_string(Some(snapshot.wallet_method.as_str())),
        status,
        settlement_authority: snapshot.authority.as_str().to_string(),
        request_published_at: snapshot.request_published_at_epoch_seconds,
        result_observed_at,
        invoice_observed_at,
        buyer_payment_pointer_at: snapshot.payment_sent_at_epoch_seconds,
        seller_settlement_feedback_at,
        buyer_wallet_confirmed_at: if status == Nip90PaymentFactStatus::BuyerWalletSettled {
            snapshot.timestamp
        } else {
            None
        },
        seller_wallet_confirmed_at: None,
        selected_relays: snapshot.request_publish_selected_relays.clone(),
        publish_accepted_relays: snapshot.request_publish_accepted_relays.clone(),
        publish_rejected_relays: snapshot.request_publish_rejected_relays.clone(),
        provider_observation_history: snapshot.provider_observation_history.clone(),
        source_quality,
    }
}

fn seller_fact_from_history_row(
    row: &crate::app_state::JobHistoryReceiptRow,
    wallet_payment: Option<&openagents_spark::PaymentSummary>,
    local_nostr_pubkey_hex: Option<&str>,
) -> Nip90PaymentFact {
    let request_id = infer_request_id_from_job_id(row.job_id.as_str());
    let status = if wallet_payment.is_some() {
        Nip90PaymentFactStatus::SellerWalletSettled
    } else if row.status == crate::app_state::JobHistoryStatus::Succeeded {
        Nip90PaymentFactStatus::SellerSettlementObserved
    } else {
        Nip90PaymentFactStatus::Failed
    };
    let source_quality = if wallet_payment.is_some() {
        Nip90PaymentFactSourceQuality::SellerWalletReconciled
    } else {
        Nip90PaymentFactSourceQuality::SellerReceiptProjection
    };

    Nip90PaymentFact {
        fact_id: payment_fact_id(request_id.as_str()),
        request_id,
        request_type: "unknown".to_string(),
        request_event_id: None,
        result_event_id: normalize_optional_string(row.sa_tick_result_event_id.as_deref()),
        invoice_event_id: None,
        seller_feedback_event_id: None,
        buyer_nostr_pubkey: normalize_optional_string(row.requester_nostr_pubkey.as_deref()),
        provider_nostr_pubkey: normalize_optional_string(row.provider_nostr_pubkey.as_deref())
            .or_else(|| local_nostr_pubkey_hex.map(ToString::to_string)),
        invoice_provider_pubkey: normalize_optional_string(row.provider_nostr_pubkey.as_deref())
            .or_else(|| local_nostr_pubkey_hex.map(ToString::to_string)),
        result_provider_pubkey: normalize_optional_string(row.provider_nostr_pubkey.as_deref())
            .or_else(|| local_nostr_pubkey_hex.map(ToString::to_string)),
        invoice_observed_relays: Vec::new(),
        result_observed_relays: Vec::new(),
        lightning_destination_pubkey: None,
        buyer_payment_pointer: None,
        seller_payment_pointer: normalize_optional_string(Some(row.payment_pointer.as_str())),
        buyer_payment_hash: None,
        amount_sats: wallet_payment
            .map(|payment| payment.amount_sats)
            .or(Some(row.payout_sats)),
        fees_sats: wallet_payment.map(|payment| payment.fees_sats),
        total_debit_sats: wallet_payment.map(|payment| payment.amount_sats),
        wallet_method: wallet_payment.map(|payment| payment.method.clone()),
        status,
        settlement_authority: if wallet_payment.is_some() {
            "wallet.reconciliation".to_string()
        } else {
            "earn.receipts".to_string()
        },
        request_published_at: None,
        result_observed_at: None,
        invoice_observed_at: None,
        buyer_payment_pointer_at: None,
        seller_settlement_feedback_at: Some(row.completed_at_epoch_seconds),
        buyer_wallet_confirmed_at: None,
        seller_wallet_confirmed_at: wallet_payment.map(|payment| payment.timestamp),
        selected_relays: Vec::new(),
        publish_accepted_relays: Vec::new(),
        publish_rejected_relays: Vec::new(),
        provider_observation_history: Vec::new(),
        source_quality,
    }
}

fn session_log_directory_signature(session_log_dir: &Path) -> Result<Option<String>, String> {
    let files = session_log_jsonl_files(session_log_dir)?;
    if files.is_empty() {
        return Ok(None);
    }

    let mut signature = Vec::with_capacity(files.len());
    for path in files {
        let metadata = fs::metadata(path.as_path())
            .map_err(|error| format!("read session log metadata {}: {error}", path.display()))?;
        let modified_ms = metadata
            .modified()
            .ok()
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("session.jsonl");
        signature.push(format!("{file_name}:{}:{modified_ms}", metadata.len()));
    }
    Ok(Some(signature.join("|")))
}

fn session_log_jsonl_files(session_log_dir: &Path) -> Result<Vec<PathBuf>, String> {
    if !session_log_dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = fs::read_dir(session_log_dir)
        .map_err(|error| {
            format!(
                "read session log directory {}: {error}",
                session_log_dir.display()
            )
        })?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .file_type()
                .map(|kind| kind.is_file())
                .unwrap_or(false)
                && entry.path().extension().and_then(|ext| ext.to_str()) == Some("jsonl")
        })
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    files.sort();
    Ok(files)
}

fn load_log_backfill_facts_from_session_dir(
    session_log_dir: &Path,
    local_nostr_pubkey_hex: Option<&str>,
) -> Result<Vec<Nip90PaymentFact>, String> {
    let local_nostr_pubkey_hex = local_nostr_pubkey_hex
        .map(normalize_pubkey_key)
        .filter(|value| !value.is_empty());
    let mut facts_by_request = BTreeMap::<String, Nip90PaymentFact>::new();
    for path in session_log_jsonl_files(session_log_dir)? {
        let contents = fs::read_to_string(path.as_path())
            .map_err(|error| format!("read session log {}: {error}", path.display()))?;
        for (line_idx, line) in contents.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let entry = serde_json::from_str::<Value>(trimmed).map_err(|error| {
                format!(
                    "parse session log {}:{}: {error}",
                    path.display(),
                    line_idx + 1
                )
            })?;
            if let Some(fact) =
                log_backfill_fact_from_entry(&entry, local_nostr_pubkey_hex.as_deref())
            {
                merge_fact(&mut facts_by_request, fact);
            }
        }
    }
    Ok(normalize_facts(facts_by_request.into_values().collect()))
}

fn log_backfill_fact_from_entry(
    entry: &Value,
    local_nostr_pubkey_hex: Option<&str>,
) -> Option<Nip90PaymentFact> {
    let object = entry.as_object()?;
    if let Some(fact) = log_backfill_fact_from_domain_entry(object, local_nostr_pubkey_hex) {
        return Some(fact);
    }
    log_backfill_fact_from_request_publish_line(object, local_nostr_pubkey_hex)
}

fn log_backfill_fact_from_domain_entry(
    object: &serde_json::Map<String, Value>,
    local_nostr_pubkey_hex: Option<&str>,
) -> Option<Nip90PaymentFact> {
    let domain = object.get("domain")?.as_object()?;
    let event = domain_string(domain, "event")?;
    let request_id = domain_string(domain, "request_id")?;
    let timestamp_seconds = timestamp_seconds_from_log_entry_map(object)?;
    let role = if event.starts_with("provider.") {
        "provider"
    } else {
        "buyer"
    };
    let mut fact = empty_log_backfill_fact(
        request_id.as_str(),
        role,
        local_nostr_pubkey_hex,
        timestamp_seconds,
    );
    let provider_pubkey = preferred_provider_pubkey(domain, role, local_nostr_pubkey_hex);
    let invoice_amount_sats = domain_u64(domain, "invoice_amount_sats")
        .or_else(|| domain_u64(domain, "amount_sats"))
        .or_else(|| domain_u64(domain, "amount_msats").map(|value| value / 1_000));

    match event.as_str() {
        "buyer.result_candidate_observed" => {
            fact.provider_nostr_pubkey = provider_pubkey.clone();
            fact.result_provider_pubkey = provider_pubkey;
            fact.result_event_id = domain_string(domain, "result_event_id");
            fact.result_observed_at = Some(timestamp_seconds);
            fact.status = Nip90PaymentFactStatus::ResultObserved;
            if let Some(history) = log_backfill_history_event(
                request_id.as_str(),
                domain,
                timestamp_seconds,
                NetworkRequestProviderObservationHistoryKind::ResultObserved,
            ) {
                fact.provider_observation_history.push(history);
            }
        }
        "buyer.invoice_candidate_observed" | "buyer.invoice_rejected_over_budget" => {
            fact.provider_nostr_pubkey = provider_pubkey.clone();
            fact.invoice_provider_pubkey = provider_pubkey;
            fact.invoice_event_id = domain_string(domain, "feedback_event_id");
            fact.invoice_observed_at = Some(timestamp_seconds);
            fact.amount_sats = invoice_amount_sats;
            fact.status = Nip90PaymentFactStatus::InvoiceObserved;
            if let Some(mut history) = log_backfill_history_event(
                request_id.as_str(),
                domain,
                timestamp_seconds,
                NetworkRequestProviderObservationHistoryKind::FeedbackObserved,
            ) {
                if history.status.is_none() {
                    history.status = Some("payment-required".to_string());
                }
                history.bolt11_present |= domain_bool(domain, "bolt11_present").unwrap_or(false);
                history.amount_msats = history
                    .amount_msats
                    .or_else(|| domain_u64(domain, "amount_msats"));
                fact.provider_observation_history.push(history);
            }
        }
        "buyer.selected_payable_provider" => {
            fact.provider_nostr_pubkey = provider_pubkey.clone();
            fact.result_provider_pubkey = provider_pubkey.clone();
            fact.invoice_provider_pubkey = provider_pubkey;
            fact.result_event_id = domain_string(domain, "result_event_id");
            fact.invoice_event_id = domain_string(domain, "feedback_event_id");
            fact.amount_sats = invoice_amount_sats;
            fact.status = if fact.invoice_event_id.is_some() {
                Nip90PaymentFactStatus::BuyerPaymentPending
            } else {
                Nip90PaymentFactStatus::ResultObserved
            };
            if let Some(history) = log_backfill_history_event(
                request_id.as_str(),
                domain,
                timestamp_seconds,
                NetworkRequestProviderObservationHistoryKind::PayableWinnerSelected,
            ) {
                fact.provider_observation_history.push(history);
            }
        }
        "buyer.queued_payment" => {
            fact.provider_nostr_pubkey = provider_pubkey;
            fact.invoice_event_id = domain_string(domain, "feedback_event_id");
            fact.amount_sats = domain_u64(domain, "amount_sats");
            fact.status = Nip90PaymentFactStatus::BuyerPaymentPending;
        }
        "buyer.payment_settled" => {
            fact.provider_nostr_pubkey = provider_pubkey;
            fact.buyer_payment_pointer = domain_string(domain, "payment_pointer");
            fact.buyer_payment_pointer_at = Some(timestamp_seconds);
            fact.buyer_wallet_confirmed_at = Some(timestamp_seconds);
            fact.amount_sats = domain_u64(domain, "amount_sats");
            fact.fees_sats = domain_u64(domain, "fees_sats");
            fact.total_debit_sats = domain_u64(domain, "total_debit_sats");
            fact.status = Nip90PaymentFactStatus::BuyerWalletSettled;
        }
        "buyer.seller_settled_pending_wallet_confirmation" => {
            fact.provider_nostr_pubkey = provider_pubkey;
            fact.seller_feedback_event_id = domain_string(domain, "feedback_event_id");
            fact.buyer_payment_pointer = domain_string(domain, "payment_pointer");
            fact.seller_settlement_feedback_at = Some(timestamp_seconds);
            fact.status = Nip90PaymentFactStatus::SellerSettlementObserved;
            if let Some(mut history) = log_backfill_history_event(
                request_id.as_str(),
                domain,
                timestamp_seconds,
                NetworkRequestProviderObservationHistoryKind::FeedbackObserved,
            ) {
                if history.status.is_none() {
                    history.status = Some("success".to_string());
                }
                fact.provider_observation_history.push(history);
            }
        }
        "buyer.payment_blocked" => {
            fact.provider_nostr_pubkey = provider_pubkey;
            fact.status = Nip90PaymentFactStatus::Failed;
        }
        "provider.result_published" => {
            fact.provider_nostr_pubkey = provider_pubkey;
            fact.result_provider_pubkey = fact.provider_nostr_pubkey.clone();
            fact.result_event_id = domain_string(domain, "event_id");
            fact.result_observed_at = Some(timestamp_seconds);
            fact.status = Nip90PaymentFactStatus::ResultObserved;
        }
        "provider.payment_requested" => {
            fact.provider_nostr_pubkey = provider_pubkey;
            fact.invoice_provider_pubkey = fact.provider_nostr_pubkey.clone();
            fact.invoice_event_id = domain_string(domain, "feedback_event_id");
            fact.invoice_observed_at = Some(timestamp_seconds);
            fact.amount_sats = domain_u64(domain, "amount_sats");
            fact.status = Nip90PaymentFactStatus::InvoiceObserved;
        }
        "provider.settlement_confirmed" => {
            fact.provider_nostr_pubkey = provider_pubkey;
            fact.seller_feedback_event_id = domain_string(domain, "success_feedback_id");
            fact.seller_payment_pointer = domain_string(domain, "payment_id");
            fact.seller_wallet_confirmed_at = Some(timestamp_seconds);
            fact.amount_sats = domain_u64(domain, "amount_sats");
            fact.fees_sats = domain_u64(domain, "fees_sats");
            fact.status = Nip90PaymentFactStatus::SellerWalletSettled;
            fact.settlement_authority = "runtime.session_log.provider".to_string();
        }
        "provider.delivered_unpaid_timeout" => {
            fact.provider_nostr_pubkey = provider_pubkey;
            fact.status = Nip90PaymentFactStatus::Failed;
        }
        _ => return None,
    }

    Some(fact)
}

fn log_backfill_fact_from_request_publish_line(
    object: &serde_json::Map<String, Value>,
    local_nostr_pubkey_hex: Option<&str>,
) -> Option<Nip90PaymentFact> {
    let source = object.get("source")?.as_str()?.trim();
    let target = object.get("target")?.as_str()?.trim();
    if source != "tracing" || target != "autopilot_desktop::buyer" {
        return None;
    }

    let line = object.get("line")?.as_str()?.trim();
    let request_id = extract_line_field(line, "request_id")?;
    let timestamp_seconds = timestamp_seconds_from_log_entry_map(object)?;
    let mut fact = empty_log_backfill_fact(
        request_id.as_str(),
        "buyer",
        local_nostr_pubkey_hex,
        timestamp_seconds,
    );
    fact.request_type =
        extract_line_field(line, "request_type").unwrap_or_else(|| "unknown".to_string());
    fact.request_event_id = extract_line_field(line, "event_id");
    fact.request_published_at = Some(timestamp_seconds);
    fact.status = if line.starts_with("Failed NIP-90 request publish") {
        Nip90PaymentFactStatus::Failed
    } else if line.starts_with("Published NIP-90 request") {
        Nip90PaymentFactStatus::RequestPublished
    } else {
        return None;
    };
    Some(fact)
}

fn empty_log_backfill_fact(
    request_id: &str,
    role: &str,
    local_nostr_pubkey_hex: Option<&str>,
    request_published_at: u64,
) -> Nip90PaymentFact {
    Nip90PaymentFact {
        fact_id: payment_fact_id(request_id),
        request_id: request_id.to_string(),
        request_type: "unknown".to_string(),
        request_event_id: None,
        result_event_id: None,
        invoice_event_id: None,
        seller_feedback_event_id: None,
        buyer_nostr_pubkey: if role == "buyer" {
            local_nostr_pubkey_hex.map(ToString::to_string)
        } else {
            None
        },
        provider_nostr_pubkey: if role == "provider" {
            local_nostr_pubkey_hex.map(ToString::to_string)
        } else {
            None
        },
        invoice_provider_pubkey: None,
        result_provider_pubkey: None,
        invoice_observed_relays: Vec::new(),
        result_observed_relays: Vec::new(),
        lightning_destination_pubkey: None,
        buyer_payment_pointer: None,
        seller_payment_pointer: None,
        buyer_payment_hash: None,
        amount_sats: None,
        fees_sats: None,
        total_debit_sats: None,
        wallet_method: None,
        status: Nip90PaymentFactStatus::RequestPublished,
        settlement_authority: "runtime.session_log".to_string(),
        request_published_at: Some(request_published_at),
        result_observed_at: None,
        invoice_observed_at: None,
        buyer_payment_pointer_at: None,
        seller_settlement_feedback_at: None,
        buyer_wallet_confirmed_at: None,
        seller_wallet_confirmed_at: None,
        selected_relays: Vec::new(),
        publish_accepted_relays: Vec::new(),
        publish_rejected_relays: Vec::new(),
        provider_observation_history: Vec::new(),
        source_quality: Nip90PaymentFactSourceQuality::LogBackfill,
    }
}

fn log_backfill_history_event(
    request_id: &str,
    domain: &serde_json::Map<String, Value>,
    timestamp_seconds: u64,
    kind: NetworkRequestProviderObservationHistoryKind,
) -> Option<NetworkRequestProviderObservationHistoryEvent> {
    let provider_pubkey = preferred_provider_pubkey(domain, "buyer", None);
    let observed_event_id = match kind {
        NetworkRequestProviderObservationHistoryKind::ResultObserved => {
            domain_string(domain, "result_event_id")
        }
        NetworkRequestProviderObservationHistoryKind::FeedbackObserved => {
            domain_string(domain, "feedback_event_id")
        }
        NetworkRequestProviderObservationHistoryKind::PayableWinnerSelected
        | NetworkRequestProviderObservationHistoryKind::PayableWinnerCleared => {
            domain_string(domain, "feedback_event_id")
                .or_else(|| domain_string(domain, "result_event_id"))
        }
    };

    if provider_pubkey.is_none()
        && observed_event_id.is_none()
        && kind != NetworkRequestProviderObservationHistoryKind::PayableWinnerSelected
    {
        return None;
    }

    let event_key = observed_event_id.as_deref().unwrap_or_else(|| kind.label());
    Some(NetworkRequestProviderObservationHistoryEvent {
        history_id: format!("log_backfill:{request_id}:{}:{event_key}", kind.label()),
        observed_order: 0,
        observed_at_epoch_ms: timestamp_seconds.saturating_mul(1_000),
        kind,
        provider_pubkey,
        relay_urls: Vec::new(),
        observed_event_id,
        status: domain_string(domain, "status"),
        status_extra: domain_string(domain, "status_extra"),
        amount_msats: domain_u64(domain, "amount_msats"),
        bolt11_present: domain_bool(domain, "bolt11_present").unwrap_or(false),
        previous_provider_pubkey: domain_string(domain, "previous_provider_pubkey"),
        winner_result_event_id: domain_string(domain, "result_event_id"),
        winner_feedback_event_id: domain_string(domain, "feedback_event_id"),
        selection_source: domain_string(domain, "selection_source"),
    })
}

fn preferred_provider_pubkey(
    domain: &serde_json::Map<String, Value>,
    role: &str,
    local_nostr_pubkey_hex: Option<&str>,
) -> Option<String> {
    [
        domain_string(domain, "provider_pubkey"),
        domain_string(domain, "payable_provider_pubkey"),
        domain_string(domain, "result_provider_pubkey"),
        domain_string(domain, "invoice_provider_pubkey"),
        if role == "provider" {
            local_nostr_pubkey_hex.map(ToString::to_string)
        } else {
            None
        },
    ]
    .into_iter()
    .flatten()
    .find(|value| !value.is_empty())
}

fn domain_string(domain: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    let value = domain.get(key)?;
    match value {
        Value::String(value) => normalize_optional_string(Some(value.as_str())),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn domain_u64(domain: &serde_json::Map<String, Value>, key: &str) -> Option<u64> {
    let value = domain.get(key)?;
    match value {
        Value::Number(number) => number.as_u64(),
        Value::String(value) => {
            let normalized = value.trim();
            if normalized.is_empty() || normalized.eq_ignore_ascii_case("none") {
                None
            } else {
                normalized.parse::<u64>().ok()
            }
        }
        _ => None,
    }
}

fn domain_bool(domain: &serde_json::Map<String, Value>, key: &str) -> Option<bool> {
    let value = domain.get(key)?;
    match value {
        Value::Bool(value) => Some(*value),
        Value::String(value) => match value.trim().to_ascii_lowercase().as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn timestamp_seconds_from_log_entry(entry: &Value) -> Option<u64> {
    timestamp_seconds_from_log_entry_map(entry.as_object()?)
}

fn timestamp_seconds_from_log_entry_map(object: &serde_json::Map<String, Value>) -> Option<u64> {
    object
        .get("timestamp_ms")
        .and_then(Value::as_u64)
        .map(|value| value / 1_000)
}

fn extract_line_field(line: &str, key: &str) -> Option<String> {
    let needle = format!("{key}=");
    let start = line.find(needle.as_str())?;
    let value = &line[start + needle.len()..];
    let end = value.find(char::is_whitespace).unwrap_or(value.len());
    normalize_optional_string(Some(&value[..end]))
}

fn first_provider_history_epoch_seconds(
    history: &[NetworkRequestProviderObservationHistoryEvent],
    predicate: impl Fn(&NetworkRequestProviderObservationHistoryEvent) -> bool,
) -> Option<u64> {
    history
        .iter()
        .filter(|event| predicate(event))
        .map(|event| event.observed_at_epoch_ms / 1_000)
        .find(|observed_at| *observed_at > 0)
}

fn provider_history_event_has_invoice_signal(
    event: &NetworkRequestProviderObservationHistoryEvent,
) -> bool {
    event.kind == NetworkRequestProviderObservationHistoryKind::FeedbackObserved
        && (event.bolt11_present
            || event.amount_msats.is_some()
            || event
                .status
                .as_deref()
                .is_some_and(|status| status.eq_ignore_ascii_case("payment-required")))
}

fn provider_history_event_is_seller_settlement(
    event: &NetworkRequestProviderObservationHistoryEvent,
) -> bool {
    event.kind == NetworkRequestProviderObservationHistoryKind::FeedbackObserved
        && event
            .status
            .as_deref()
            .is_some_and(|status| status.eq_ignore_ascii_case("success"))
}

fn merge_fact(map: &mut BTreeMap<String, Nip90PaymentFact>, mut incoming: Nip90PaymentFact) {
    if let Some(existing) = map.get_mut(incoming.request_id.as_str()) {
        existing.request_type = merge_request_type(
            existing.request_type.as_str(),
            incoming.request_type.as_str(),
        );
        existing.request_event_id = merge_string_field(
            existing.request_event_id.take(),
            incoming.request_event_id.take(),
        );
        existing.result_event_id = merge_string_field(
            existing.result_event_id.take(),
            incoming.result_event_id.take(),
        );
        existing.invoice_event_id = merge_string_field(
            existing.invoice_event_id.take(),
            incoming.invoice_event_id.take(),
        );
        existing.seller_feedback_event_id = merge_string_field(
            existing.seller_feedback_event_id.take(),
            incoming.seller_feedback_event_id.take(),
        );
        existing.buyer_nostr_pubkey = merge_string_field(
            existing.buyer_nostr_pubkey.take(),
            incoming.buyer_nostr_pubkey.take(),
        );
        existing.provider_nostr_pubkey = merge_string_field(
            existing.provider_nostr_pubkey.take(),
            incoming.provider_nostr_pubkey.take(),
        );
        existing.invoice_provider_pubkey = merge_string_field(
            existing.invoice_provider_pubkey.take(),
            incoming.invoice_provider_pubkey.take(),
        );
        existing.result_provider_pubkey = merge_string_field(
            existing.result_provider_pubkey.take(),
            incoming.result_provider_pubkey.take(),
        );
        existing.invoice_observed_relays = merge_string_vecs(
            std::mem::take(&mut existing.invoice_observed_relays),
            std::mem::take(&mut incoming.invoice_observed_relays),
        );
        existing.result_observed_relays = merge_string_vecs(
            std::mem::take(&mut existing.result_observed_relays),
            std::mem::take(&mut incoming.result_observed_relays),
        );
        existing.lightning_destination_pubkey = merge_string_field(
            existing.lightning_destination_pubkey.take(),
            incoming.lightning_destination_pubkey.take(),
        );
        existing.buyer_payment_pointer = merge_string_field(
            existing.buyer_payment_pointer.take(),
            incoming.buyer_payment_pointer.take(),
        );
        existing.seller_payment_pointer = merge_string_field(
            existing.seller_payment_pointer.take(),
            incoming.seller_payment_pointer.take(),
        );
        existing.buyer_payment_hash = merge_string_field(
            existing.buyer_payment_hash.take(),
            incoming.buyer_payment_hash.take(),
        );
        existing.amount_sats = merge_u64_field(existing.amount_sats, incoming.amount_sats);
        existing.fees_sats = merge_u64_field(existing.fees_sats, incoming.fees_sats);
        existing.total_debit_sats =
            merge_u64_field(existing.total_debit_sats, incoming.total_debit_sats);
        existing.wallet_method =
            merge_string_field(existing.wallet_method.take(), incoming.wallet_method.take());
        existing.status = merge_status(existing.status, incoming.status);
        if incoming.source_quality.rank() > existing.source_quality.rank() {
            existing.source_quality = incoming.source_quality;
        }
        existing.settlement_authority = merge_string_field(
            Some(existing.settlement_authority.clone()),
            Some(incoming.settlement_authority.clone()),
        )
        .unwrap_or_else(|| "unknown".to_string());
        existing.request_published_at =
            merge_timestamp(existing.request_published_at, incoming.request_published_at);
        existing.result_observed_at =
            merge_timestamp(existing.result_observed_at, incoming.result_observed_at);
        existing.invoice_observed_at =
            merge_timestamp(existing.invoice_observed_at, incoming.invoice_observed_at);
        existing.buyer_payment_pointer_at = merge_timestamp(
            existing.buyer_payment_pointer_at,
            incoming.buyer_payment_pointer_at,
        );
        existing.seller_settlement_feedback_at = merge_timestamp(
            existing.seller_settlement_feedback_at,
            incoming.seller_settlement_feedback_at,
        );
        existing.buyer_wallet_confirmed_at = merge_timestamp(
            existing.buyer_wallet_confirmed_at,
            incoming.buyer_wallet_confirmed_at,
        );
        existing.seller_wallet_confirmed_at = merge_timestamp(
            existing.seller_wallet_confirmed_at,
            incoming.seller_wallet_confirmed_at,
        );
        existing.selected_relays = merge_string_vecs(
            std::mem::take(&mut existing.selected_relays),
            std::mem::take(&mut incoming.selected_relays),
        );
        existing.publish_accepted_relays = merge_string_vecs(
            std::mem::take(&mut existing.publish_accepted_relays),
            std::mem::take(&mut incoming.publish_accepted_relays),
        );
        existing.publish_rejected_relays = merge_string_vecs(
            std::mem::take(&mut existing.publish_rejected_relays),
            std::mem::take(&mut incoming.publish_rejected_relays),
        );
        existing.provider_observation_history = merge_provider_observation_history(
            std::mem::take(&mut existing.provider_observation_history),
            std::mem::take(&mut incoming.provider_observation_history),
        );
        return;
    }

    incoming.provider_observation_history =
        normalize_provider_observation_history(incoming.provider_observation_history);
    map.insert(incoming.request_id.clone(), incoming);
}

fn derive_actors(
    facts: &[Nip90PaymentFact],
    local_nostr_pubkey_hex: Option<&str>,
) -> Vec<Nip90Actor> {
    let mut actors = BTreeMap::<String, Nip90Actor>::new();
    for fact in facts {
        if let Some(pubkey) = fact.buyer_nostr_pubkey.as_deref() {
            upsert_actor(
                &mut actors,
                Nip90ActorNamespace::Nostr,
                pubkey,
                NIP90_ACTOR_ROLE_BUYER,
                local_nostr_pubkey_hex,
            );
        }
        if let Some(pubkey) = fact.provider_nostr_pubkey.as_deref() {
            upsert_actor(
                &mut actors,
                Nip90ActorNamespace::Nostr,
                pubkey,
                NIP90_ACTOR_ROLE_PROVIDER,
                local_nostr_pubkey_hex,
            );
        }
        if let Some(pubkey) = fact.invoice_provider_pubkey.as_deref() {
            upsert_actor(
                &mut actors,
                Nip90ActorNamespace::Nostr,
                pubkey,
                NIP90_ACTOR_ROLE_INVOICE_PROVIDER,
                local_nostr_pubkey_hex,
            );
        }
        if let Some(pubkey) = fact.result_provider_pubkey.as_deref() {
            upsert_actor(
                &mut actors,
                Nip90ActorNamespace::Nostr,
                pubkey,
                NIP90_ACTOR_ROLE_RESULT_PROVIDER,
                local_nostr_pubkey_hex,
            );
        }
        if let Some(pubkey) = fact.lightning_destination_pubkey.as_deref() {
            upsert_actor(
                &mut actors,
                Nip90ActorNamespace::LightningDestination,
                pubkey,
                NIP90_ACTOR_ROLE_LIGHTNING_DESTINATION,
                None,
            );
        }
    }
    let mut actors = actors.into_values().collect::<Vec<_>>();
    actors.sort_by(|left, right| {
        left.namespace
            .cmp(&right.namespace)
            .then_with(|| left.pubkey.cmp(&right.pubkey))
    });
    actors.truncate(NIP90_ACTOR_ROW_LIMIT);
    actors
}

fn upsert_actor(
    actors: &mut BTreeMap<String, Nip90Actor>,
    namespace: Nip90ActorNamespace,
    pubkey: &str,
    role_mask: u32,
    local_nostr_pubkey_hex: Option<&str>,
) {
    let normalized = normalize_pubkey_key(pubkey);
    if normalized.is_empty() {
        return;
    }
    let actor_id = actor_id(namespace, normalized.as_str());
    let is_local = namespace == Nip90ActorNamespace::Nostr
        && local_nostr_pubkey_hex.is_some_and(|local| local == normalized);
    let role_mask = if is_local {
        role_mask | NIP90_ACTOR_ROLE_LOCAL
    } else {
        role_mask
    };

    if let Some(existing) = actors.get_mut(actor_id.as_str()) {
        existing.role_mask |= role_mask;
        existing.is_local |= is_local;
        return;
    }

    actors.insert(
        actor_id.clone(),
        Nip90Actor {
            actor_id,
            namespace,
            pubkey: normalized.clone(),
            display_label: actor_display_label(namespace, normalized.as_str()),
            is_local,
            role_mask,
        },
    );
}

fn normalize_facts(mut facts: Vec<Nip90PaymentFact>) -> Vec<Nip90PaymentFact> {
    for fact in &mut facts {
        fact.provider_observation_history = normalize_provider_observation_history(std::mem::take(
            &mut fact.provider_observation_history,
        ));
    }
    facts.sort_by(|left, right| {
        right
            .latest_event_epoch_seconds()
            .unwrap_or(0)
            .cmp(&left.latest_event_epoch_seconds().unwrap_or(0))
            .then_with(|| left.request_id.cmp(&right.request_id))
    });
    let mut seen = BTreeSet::<String>::new();
    facts.retain(|fact| seen.insert(fact.fact_id.clone()));
    facts.truncate(NIP90_PAYMENT_FACT_ROW_LIMIT);
    facts
}

fn normalize_provider_observation_history(
    mut history: Vec<NetworkRequestProviderObservationHistoryEvent>,
) -> Vec<NetworkRequestProviderObservationHistoryEvent> {
    history.sort_by(|left, right| {
        left.observed_at_epoch_ms
            .cmp(&right.observed_at_epoch_ms)
            .then_with(|| left.observed_order.cmp(&right.observed_order))
            .then_with(|| left.history_id.cmp(&right.history_id))
    });
    let mut seen = BTreeSet::<String>::new();
    history.retain(|event| seen.insert(event.history_id.clone()));
    history
}

fn merge_provider_observation_history(
    existing: Vec<NetworkRequestProviderObservationHistoryEvent>,
    incoming: Vec<NetworkRequestProviderObservationHistoryEvent>,
) -> Vec<NetworkRequestProviderObservationHistoryEvent> {
    let mut events = BTreeMap::<String, NetworkRequestProviderObservationHistoryEvent>::new();
    for event in existing.into_iter().chain(incoming) {
        match events.entry(event.history_id.clone()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(event);
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                let current = entry.get_mut();
                current.observed_order = current.observed_order.min(event.observed_order);
                current.observed_at_epoch_ms =
                    current.observed_at_epoch_ms.min(event.observed_at_epoch_ms);
                current.relay_urls =
                    merge_string_vecs(std::mem::take(&mut current.relay_urls), event.relay_urls);
                current.status = merge_string_field(current.status.take(), event.status);
                current.status_extra =
                    merge_string_field(current.status_extra.take(), event.status_extra);
                current.amount_msats = merge_u64_field(current.amount_msats, event.amount_msats);
                current.bolt11_present |= event.bolt11_present;
                current.provider_pubkey =
                    merge_string_field(current.provider_pubkey.take(), event.provider_pubkey);
                current.previous_provider_pubkey = merge_string_field(
                    current.previous_provider_pubkey.take(),
                    event.previous_provider_pubkey,
                );
                current.observed_event_id =
                    merge_string_field(current.observed_event_id.take(), event.observed_event_id);
                current.winner_result_event_id = merge_string_field(
                    current.winner_result_event_id.take(),
                    event.winner_result_event_id,
                );
                current.winner_feedback_event_id = merge_string_field(
                    current.winner_feedback_event_id.take(),
                    event.winner_feedback_event_id,
                );
                current.selection_source =
                    merge_string_field(current.selection_source.take(), event.selection_source);
            }
        }
    }
    normalize_provider_observation_history(events.into_values().collect())
}

fn normalize_relay_hops(mut relay_hops: Vec<Nip90RelayHop>) -> Vec<Nip90RelayHop> {
    relay_hops.sort_by(|left, right| {
        right
            .observed_at
            .cmp(&left.observed_at)
            .then_with(|| left.request_id.cmp(&right.request_id))
            .then_with(|| left.event_id.cmp(&right.event_id))
    });
    let mut seen = BTreeSet::<(String, String, String, u64)>::new();
    relay_hops.retain(|hop| {
        seen.insert((
            hop.request_id.clone(),
            hop.event_id.clone(),
            hop.relay_url.clone(),
            hop.observed_at,
        ))
    });
    relay_hops.truncate(NIP90_RELAY_HOP_ROW_LIMIT);
    relay_hops
}

fn derive_relay_hops(facts: &[Nip90PaymentFact]) -> Vec<Nip90RelayHop> {
    let mut relay_hops = Vec::new();
    for fact in facts {
        let request_publish_observed_at = fact
            .request_published_at
            .or_else(|| fact.latest_event_epoch_seconds())
            .unwrap_or(0);
        if let Some(event_id) = fact.request_event_id.as_deref() {
            for relay_url in &fact.publish_accepted_relays {
                relay_hops.push(Nip90RelayHop {
                    request_id: fact.request_id.clone(),
                    event_id: event_id.to_string(),
                    hop_kind: Nip90RelayHopKind::PublishAccepted,
                    relay_url: relay_url.clone(),
                    direction: Nip90RelayHopDirection::Outbound,
                    accepted: true,
                    observed_at: request_publish_observed_at,
                });
            }
            for relay_url in &fact.publish_rejected_relays {
                relay_hops.push(Nip90RelayHop {
                    request_id: fact.request_id.clone(),
                    event_id: event_id.to_string(),
                    hop_kind: Nip90RelayHopKind::PublishRejected,
                    relay_url: relay_url.clone(),
                    direction: Nip90RelayHopDirection::Outbound,
                    accepted: false,
                    observed_at: request_publish_observed_at,
                });
            }
        }
        if let Some(event_id) = fact.invoice_event_id.as_deref() {
            let observed_at = fact
                .invoice_observed_at
                .or_else(|| fact.latest_event_epoch_seconds())
                .unwrap_or(0);
            for relay_url in &fact.invoice_observed_relays {
                relay_hops.push(Nip90RelayHop {
                    request_id: fact.request_id.clone(),
                    event_id: event_id.to_string(),
                    hop_kind: Nip90RelayHopKind::InvoiceIngress,
                    relay_url: relay_url.clone(),
                    direction: Nip90RelayHopDirection::Inbound,
                    accepted: true,
                    observed_at,
                });
            }
        }
        if let Some(event_id) = fact.result_event_id.as_deref() {
            let observed_at = fact
                .result_observed_at
                .or_else(|| fact.latest_event_epoch_seconds())
                .unwrap_or(0);
            for relay_url in &fact.result_observed_relays {
                relay_hops.push(Nip90RelayHop {
                    request_id: fact.request_id.clone(),
                    event_id: event_id.to_string(),
                    hop_kind: Nip90RelayHopKind::ResultIngress,
                    relay_url: relay_url.clone(),
                    direction: Nip90RelayHopDirection::Inbound,
                    accepted: true,
                    observed_at,
                });
            }
        }
    }
    normalize_relay_hops(relay_hops)
}

fn payment_fact_id(request_id: &str) -> String {
    format!("nip90-payment:{}", request_id.trim())
}

fn actor_id(namespace: Nip90ActorNamespace, pubkey: &str) -> String {
    format!("{}:{}", namespace.label(), pubkey)
}

fn actor_display_label(namespace: Nip90ActorNamespace, pubkey: &str) -> String {
    format!("{}:{}", namespace.label(), short_id(pubkey))
}

fn short_id(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() <= 12 {
        trimmed.to_string()
    } else {
        format!("{}..{}", &trimmed[..6], &trimmed[trimmed.len() - 4..])
    }
}

fn infer_request_id_from_job_id(job_id: &str) -> String {
    job_id
        .strip_prefix("job-")
        .map(ToString::to_string)
        .unwrap_or_else(|| job_id.to_string())
}

fn merge_string_field(left: Option<String>, right: Option<String>) -> Option<String> {
    right
        .or(left)
        .and_then(|value| normalize_optional_string(Some(value.as_str())))
}

fn merge_request_type(left: &str, right: &str) -> String {
    let normalized_left =
        normalize_optional_string(Some(left)).unwrap_or_else(|| "unknown".to_string());
    let normalized_right =
        normalize_optional_string(Some(right)).unwrap_or_else(|| "unknown".to_string());
    if normalized_right.eq_ignore_ascii_case("unknown") && !normalized_left.is_empty() {
        normalized_left
    } else {
        normalized_right
    }
}

fn merge_u64_field(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    right.or(left)
}

fn merge_timestamp(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

fn merge_string_vecs(left: Vec<String>, right: Vec<String>) -> Vec<String> {
    let mut merged = left
        .into_iter()
        .chain(right)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    merged.sort();
    merged.dedup();
    merged
}

fn merge_status(
    left: Nip90PaymentFactStatus,
    right: Nip90PaymentFactStatus,
) -> Nip90PaymentFactStatus {
    if left == Nip90PaymentFactStatus::Failed && right != Nip90PaymentFactStatus::Failed {
        return right;
    }
    if right == Nip90PaymentFactStatus::Failed && left != Nip90PaymentFactStatus::Failed {
        return left;
    }
    if right.rank() >= left.rank() {
        right
    } else {
        left
    }
}

fn normalize_optional_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "-")
        .map(ToString::to_string)
}

fn normalize_pubkey_key(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn persist_nip90_payment_fact_document(
    path: &Path,
    facts: &[Nip90PaymentFact],
    actors: &[Nip90Actor],
    relay_hops: &[Nip90RelayHop],
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create NIP-90 payment fact dir: {error}"))?;
    }
    let document = Nip90PaymentFactDocumentV1 {
        schema_version: NIP90_PAYMENT_FACT_SCHEMA_VERSION,
        stream_id: NIP90_PAYMENT_FACT_STREAM_ID.to_string(),
        facts: normalize_facts(facts.to_vec()),
        actors: actors.to_vec(),
        relay_hops: normalize_relay_hops(relay_hops.to_vec()),
    };
    let payload = serde_json::to_string_pretty(&document)
        .map_err(|error| format!("Failed to encode NIP-90 payment fact ledger: {error}"))?;
    let temp_path = path.with_extension("tmp");
    std::fs::write(&temp_path, payload)
        .map_err(|error| format!("Failed to write NIP-90 payment fact temp file: {error}"))?;
    std::fs::rename(&temp_path, path)
        .map_err(|error| format!("Failed to persist NIP-90 payment fact ledger: {error}"))?;
    Ok(())
}

fn load_nip90_payment_fact_document(path: &Path) -> Result<Nip90PaymentFactDocumentV1, String> {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Nip90PaymentFactDocumentV1 {
                schema_version: NIP90_PAYMENT_FACT_SCHEMA_VERSION,
                stream_id: NIP90_PAYMENT_FACT_STREAM_ID.to_string(),
                facts: Vec::new(),
                actors: Vec::new(),
                relay_hops: Vec::new(),
            });
        }
        Err(error) => {
            return Err(format!(
                "Failed to read NIP-90 payment fact ledger: {error}"
            ));
        }
    };
    let document = serde_json::from_str::<Nip90PaymentFactDocumentV1>(&raw)
        .map_err(|error| format!("Failed to parse NIP-90 payment fact ledger: {error}"))?;
    if document.schema_version != NIP90_PAYMENT_FACT_SCHEMA_VERSION {
        return Err(format!(
            "Unsupported NIP-90 payment fact schema version: {}",
            document.schema_version
        ));
    }
    if document.stream_id != NIP90_PAYMENT_FACT_STREAM_ID {
        return Err(format!(
            "Unsupported NIP-90 payment fact stream id: {}",
            document.stream_id
        ));
    }
    Ok(Nip90PaymentFactDocumentV1 {
        schema_version: document.schema_version,
        stream_id: document.stream_id,
        facts: normalize_facts(document.facts),
        actors: document.actors,
        relay_hops: normalize_relay_hops(document.relay_hops),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        Nip90ActorNamespace, Nip90PaymentFactLedgerState, Nip90PaymentFactSourceQuality,
        Nip90PaymentFactStatus, STARTUP_LOG_BACKFILL_DELAY,
    };
    use crate::app_state::{
        BuyerResolutionMode, JobDemandSource, JobHistoryReceiptRow, JobHistoryState,
        JobHistoryStatus, NetworkRequestStatus,
    };
    use crate::spark_wallet::SparkPaneState;
    use crate::state::operations::{
        NetworkRequestProviderObservationHistoryKind, NetworkRequestSubmission,
        NetworkRequestsState,
    };
    use openagents_spark::PaymentSummary;
    use serde_json::json;
    use std::path::PathBuf;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    fn temp_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should advance")
            .as_nanos();
        std::env::temp_dir().join(format!("openagents-{label}-{nonce}.json"))
    }

    fn temp_session_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should advance")
            .as_nanos();
        std::env::temp_dir().join(format!("openagents-{label}-{nonce}-sessions"))
    }

    fn fixture_send_payment() -> PaymentSummary {
        PaymentSummary {
            id: "wallet-send-001".to_string(),
            direction: "send".to_string(),
            status: "succeeded".to_string(),
            amount_sats: 21,
            fees_sats: 1,
            timestamp: 1_762_700_777,
            method: "bolt11".to_string(),
            description: Some("DVM textgen".to_string()),
            invoice: Some("lnbc1buyer".to_string()),
            destination_pubkey: Some("02BuyerDest".to_string()),
            payment_hash: Some("buyerhash001".to_string()),
            htlc_status: Some("settled".to_string()),
            htlc_expiry_epoch_seconds: None,
            status_detail: Some("wallet confirmed".to_string()),
        }
    }

    fn fixture_receive_payment() -> PaymentSummary {
        PaymentSummary {
            id: "wallet-recv-001".to_string(),
            direction: "receive".to_string(),
            status: "succeeded".to_string(),
            amount_sats: 34,
            fees_sats: 0,
            timestamp: 1_762_700_888,
            method: "spark-address".to_string(),
            description: Some("Provider payout".to_string()),
            invoice: None,
            destination_pubkey: None,
            payment_hash: Some("sellerhash001".to_string()),
            htlc_status: None,
            htlc_expiry_epoch_seconds: None,
            status_detail: Some("wallet confirmed".to_string()),
        }
    }

    #[test]
    fn payment_fact_ledger_persists_buyer_send_and_seller_settle_roundtrip() {
        let path = temp_path("nip90-payment-facts");
        let mut ledger = Nip90PaymentFactLedgerState::from_path_for_tests(path.clone());
        let mut network_requests = NetworkRequestsState::default();
        let request_id = network_requests
            .queue_request_submission(NetworkRequestSubmission {
                request_id: Some("req-buy-001".to_string()),
                request_type: "text-generation".to_string(),
                payload: "hello".to_string(),
                resolution_mode: BuyerResolutionMode::Race,
                target_provider_pubkeys: vec!["providerhex001".to_string()],
                skill_scope_id: None,
                credit_envelope_ref: None,
                budget_sats: 21,
                timeout_seconds: 30,
                authority_command_seq: 7,
            })
            .expect("request should queue");
        let selected_relays = vec![
            "wss://relay.publish.one/".to_string(),
            "wss://relay.publish.two".to_string(),
        ];
        let accepted_relay_urls = vec!["wss://relay.publish.one/".to_string()];
        let rejected_relay_urls = vec!["wss://relay.publish.two".to_string()];
        network_requests.apply_nip90_request_publish_outcome_with_relays(
            request_id.as_str(),
            "event-request-001",
            selected_relays.as_slice(),
            accepted_relay_urls.as_slice(),
            rejected_relay_urls.as_slice(),
            1,
            1,
            None,
        );
        network_requests.apply_nip90_buyer_feedback_event_with_relay(
            request_id.as_str(),
            "providerhex001",
            "event-feedback-001",
            Some("wss://relay.invoice.test/"),
            Some("payment-required"),
            Some("invoice required"),
            Some(21_000),
            Some("lnbc1buyer"),
        );
        network_requests.apply_nip90_buyer_result_event_with_relay(
            request_id.as_str(),
            "providerhex001",
            "event-result-001",
            Some("wss://relay.result.test/"),
            Some("success"),
        );
        let request = network_requests
            .submitted
            .iter_mut()
            .find(|request| request.request_id == request_id)
            .expect("request should exist");
        request.status = NetworkRequestStatus::Paid;
        request.winning_provider_pubkey = Some("providerhex001".to_string());
        request.winning_result_event_id = Some("event-result-001".to_string());
        request.last_payment_pointer = Some("wallet-send-001".to_string());
        request.payment_required_at_epoch_seconds = Some(1_762_700_700);
        request.payment_sent_at_epoch_seconds = Some(1_762_700_777);

        let mut job_history = JobHistoryState::default();
        job_history.rows.push(JobHistoryReceiptRow {
            job_id: "job-req-sell-001".to_string(),
            status: JobHistoryStatus::Succeeded,
            demand_source: JobDemandSource::OpenNetwork,
            completed_at_epoch_seconds: 1_762_700_850,
            requester_nostr_pubkey: Some("npub1buyer".to_string()),
            provider_nostr_pubkey: Some("providerpubkey-local".to_string()),
            skill_scope_id: None,
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_result_event_id: Some("event-seller-result-001".to_string()),
            sa_trajectory_session_id: Some("traj:req-sell-001".to_string()),
            ac_envelope_event_id: None,
            ac_settlement_event_id: None,
            ac_default_event_id: None,
            delivery_proof_id: None,
            delivery_metering_rule_id: None,
            delivery_proof_status_label: None,
            delivery_metered_quantity: None,
            delivery_accepted_quantity: None,
            delivery_variance_reason_label: None,
            delivery_rejection_reason_label: None,
            payout_sats: 34,
            result_hash: "sha256:seller".to_string(),
            payment_pointer: "wallet-recv-001".to_string(),
            failure_reason: None,
            execution_provenance: None,
        });

        let mut spark_wallet = SparkPaneState::default();
        spark_wallet.recent_payments = vec![fixture_send_payment(), fixture_receive_payment()];

        ledger.sync_from_current_truth(
            &network_requests,
            &job_history,
            &spark_wallet,
            Some("LOCALPUBKEY001"),
        );

        let buyer_fact = ledger
            .fact_for_request("req-buy-001")
            .expect("buyer fact should exist");
        assert_eq!(
            buyer_fact.status,
            Nip90PaymentFactStatus::BuyerWalletSettled
        );
        assert_eq!(
            buyer_fact.buyer_nostr_pubkey.as_deref(),
            Some("localpubkey001")
        );
        assert_eq!(
            buyer_fact.provider_nostr_pubkey.as_deref(),
            Some("providerhex001")
        );
        assert_eq!(
            buyer_fact.lightning_destination_pubkey.as_deref(),
            Some("02BuyerDest")
        );
        assert_eq!(
            buyer_fact.source_quality,
            Nip90PaymentFactSourceQuality::BuyerWalletReconciled
        );
        assert_eq!(
            buyer_fact.selected_relays,
            vec![
                "wss://relay.publish.one".to_string(),
                "wss://relay.publish.two".to_string(),
            ]
        );
        assert_eq!(
            buyer_fact.publish_accepted_relays,
            vec!["wss://relay.publish.one".to_string()]
        );
        assert_eq!(
            buyer_fact.publish_rejected_relays,
            vec!["wss://relay.publish.two".to_string()]
        );
        assert_eq!(
            buyer_fact.invoice_observed_relays,
            vec!["wss://relay.invoice.test".to_string()]
        );
        assert_eq!(
            buyer_fact.result_observed_relays,
            vec!["wss://relay.result.test".to_string()]
        );
        assert!(buyer_fact.result_observed_at.is_some());
        assert_eq!(buyer_fact.invoice_observed_at, Some(1_762_700_700));
        assert_eq!(buyer_fact.provider_observation_history.len(), 3);
        assert!(buyer_fact.provider_observation_history.iter().any(|event| {
            event.kind == NetworkRequestProviderObservationHistoryKind::PayableWinnerSelected
                && event.provider_pubkey.as_deref() == Some("providerhex001")
                && event.selection_source.as_deref() == Some("preferred_provider_became_payable")
        }));

        let seller_fact = ledger
            .fact_for_request("req-sell-001")
            .expect("seller fact should exist");
        assert_eq!(
            seller_fact.status,
            Nip90PaymentFactStatus::SellerWalletSettled
        );
        assert_eq!(
            seller_fact.provider_nostr_pubkey.as_deref(),
            Some("providerpubkey-local")
        );
        assert_eq!(
            seller_fact.seller_payment_pointer.as_deref(),
            Some("wallet-recv-001")
        );

        let buyer_actor_facts =
            ledger.facts_for_actor(Nip90ActorNamespace::Nostr, "localpubkey001");
        assert_eq!(buyer_actor_facts.len(), 1);
        let seller_actor_facts =
            ledger.facts_for_actor(Nip90ActorNamespace::Nostr, "providerpubkey-local");
        assert_eq!(seller_actor_facts.len(), 1);
        let lightning_actor_facts =
            ledger.facts_for_actor(Nip90ActorNamespace::LightningDestination, "02buyerdest");
        assert_eq!(lightning_actor_facts.len(), 1);
        assert_eq!(lightning_actor_facts[0].request_id, "req-buy-001");
        assert!(ledger.relay_hops.iter().any(|hop| {
            hop.request_id == "req-buy-001"
                && hop.event_id == "event-request-001"
                && hop.hop_kind == super::Nip90RelayHopKind::PublishAccepted
                && hop.relay_url == "wss://relay.publish.one"
        }));
        assert!(ledger.relay_hops.iter().any(|hop| {
            hop.request_id == "req-buy-001"
                && hop.event_id == "event-request-001"
                && hop.hop_kind == super::Nip90RelayHopKind::PublishRejected
                && hop.relay_url == "wss://relay.publish.two"
        }));
        assert!(ledger.relay_hops.iter().any(|hop| {
            hop.request_id == "req-buy-001"
                && hop.event_id == "event-feedback-001"
                && hop.hop_kind == super::Nip90RelayHopKind::InvoiceIngress
                && hop.relay_url == "wss://relay.invoice.test"
        }));
        assert!(ledger.relay_hops.iter().any(|hop| {
            hop.request_id == "req-buy-001"
                && hop.event_id == "event-result-001"
                && hop.hop_kind == super::Nip90RelayHopKind::ResultIngress
                && hop.relay_url == "wss://relay.result.test"
        }));

        let reloaded = Nip90PaymentFactLedgerState::from_path_for_tests(path.clone());
        assert_eq!(reloaded.facts, ledger.facts);
        assert_eq!(reloaded.actors, ledger.actors);
        assert_eq!(reloaded.relay_hops, ledger.relay_hops);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn payment_fact_ledger_replays_multi_provider_history_after_reload_without_live_request_state()
    {
        let path = temp_path("nip90-payment-race-history");
        let mut ledger = Nip90PaymentFactLedgerState::from_path_for_tests(path.clone());
        let mut network_requests = NetworkRequestsState::default();
        let request_id = network_requests
            .queue_request_submission(NetworkRequestSubmission {
                request_id: Some("req-race-001".to_string()),
                request_type: "text-generation".to_string(),
                payload: "race".to_string(),
                resolution_mode: BuyerResolutionMode::Race,
                target_provider_pubkeys: vec![
                    "provideralpha001".to_string(),
                    "providerbeta002".to_string(),
                ],
                skill_scope_id: None,
                credit_envelope_ref: None,
                budget_sats: 25,
                timeout_seconds: 45,
                authority_command_seq: 9,
            })
            .expect("request should queue");

        network_requests.apply_nip90_request_publish_outcome_with_relays(
            request_id.as_str(),
            "event-request-race-001",
            &["wss://relay.publish.one".to_string()],
            &["wss://relay.publish.one".to_string()],
            &[],
            1,
            0,
            None,
        );
        network_requests.apply_nip90_buyer_result_event_with_relay(
            request_id.as_str(),
            "provideralpha001",
            "event-result-alpha-001",
            Some("wss://relay.result.alpha"),
            Some("success"),
        );
        network_requests.apply_nip90_buyer_result_event_with_relay(
            request_id.as_str(),
            "providerbeta002",
            "event-result-beta-001",
            Some("wss://relay.result.beta"),
            Some("success"),
        );
        network_requests.apply_nip90_buyer_result_event_with_relay(
            request_id.as_str(),
            "providerbeta002",
            "event-result-beta-001",
            Some("wss://relay.result.beta.alt/"),
            Some("success"),
        );
        network_requests.apply_nip90_buyer_feedback_event_with_relay(
            request_id.as_str(),
            "providerbeta002",
            "event-feedback-beta-001",
            Some("wss://relay.invoice.beta"),
            Some("payment-required"),
            Some("invoice ready"),
            Some(25_000),
            Some("lnbc25n1beta"),
        );
        network_requests.apply_nip90_buyer_feedback_event_with_relay(
            request_id.as_str(),
            "provideralpha001",
            "event-feedback-alpha-001",
            Some("wss://relay.invoice.alpha"),
            Some("payment-required"),
            Some("invoice late"),
            Some(25_000),
            Some("lnbc25n1alpha"),
        );

        let spark_wallet = SparkPaneState::default();
        let job_history = JobHistoryState::default();
        ledger.sync_from_current_truth(
            &network_requests,
            &job_history,
            &spark_wallet,
            Some("LOCALBUYERKEY"),
        );

        let fact = ledger
            .fact_for_request("req-race-001")
            .expect("race fact should exist");
        let history = &fact.provider_observation_history;
        assert_eq!(history.len(), 5);
        assert_eq!(
            history
                .iter()
                .map(|event| (event.kind, event.provider_pubkey.clone()))
                .collect::<Vec<_>>(),
            vec![
                (
                    NetworkRequestProviderObservationHistoryKind::ResultObserved,
                    Some("provideralpha001".to_string()),
                ),
                (
                    NetworkRequestProviderObservationHistoryKind::ResultObserved,
                    Some("providerbeta002".to_string()),
                ),
                (
                    NetworkRequestProviderObservationHistoryKind::FeedbackObserved,
                    Some("providerbeta002".to_string()),
                ),
                (
                    NetworkRequestProviderObservationHistoryKind::PayableWinnerSelected,
                    Some("providerbeta002".to_string()),
                ),
                (
                    NetworkRequestProviderObservationHistoryKind::FeedbackObserved,
                    Some("provideralpha001".to_string()),
                ),
            ]
        );
        let beta_result = history
            .iter()
            .find(|event| event.observed_event_id.as_deref() == Some("event-result-beta-001"))
            .expect("beta result history should exist");
        assert_eq!(
            beta_result.relay_urls,
            vec![
                "wss://relay.result.beta".to_string(),
                "wss://relay.result.beta.alt".to_string(),
            ]
        );
        let winner_event = history
            .iter()
            .find(|event| {
                event.kind == NetworkRequestProviderObservationHistoryKind::PayableWinnerSelected
            })
            .expect("winner selection history should exist");
        assert_eq!(
            winner_event.provider_pubkey.as_deref(),
            Some("providerbeta002")
        );
        assert_eq!(
            winner_event.winner_result_event_id.as_deref(),
            Some("event-result-beta-001")
        );
        assert_eq!(
            winner_event.winner_feedback_event_id.as_deref(),
            Some("event-feedback-beta-001")
        );
        assert_eq!(
            winner_event.selection_source.as_deref(),
            Some("preferred_provider_became_payable")
        );

        let mut reloaded = Nip90PaymentFactLedgerState::from_path_for_tests(path.clone());
        reloaded.sync_from_current_truth(
            &NetworkRequestsState::default(),
            &JobHistoryState::default(),
            &SparkPaneState::default(),
            Some("LOCALBUYERKEY"),
        );
        let reloaded_fact = reloaded
            .fact_for_request("req-race-001")
            .expect("reloaded race fact should remain available");
        assert_eq!(
            reloaded_fact.provider_observation_history,
            fact.provider_observation_history
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn load_log_backfill_facts_from_session_dir_builds_degraded_payment_facts() {
        let session_dir = temp_session_dir("nip90-log-backfill");
        std::fs::create_dir_all(&session_dir).expect("create session dir");
        let session_path = session_dir.join("20260313T230000Z-pid777.jsonl");
        write_session_log(
            &session_path,
            &[
                json!({
                    "timestamp_ms": 1_762_700_001_000_u64,
                    "source": "tracing",
                    "target": "autopilot_desktop::buyer",
                    "line": "Published NIP-90 request request_id=req-log-001 request_type=text-generation event_id=event-request-001 accepted_relays=2 rejected_relays=0"
                }),
                json!({
                    "timestamp_ms": 1_762_700_002_000_u64,
                    "domain": {
                        "event": "buyer.result_candidate_observed",
                        "request_id": "req-log-001",
                        "provider_pubkey": "provideralpha001",
                        "result_event_id": "event-result-001",
                        "status": "success"
                    }
                }),
                json!({
                    "timestamp_ms": 1_762_700_003_000_u64,
                    "domain": {
                        "event": "buyer.invoice_candidate_observed",
                        "request_id": "req-log-001",
                        "provider_pubkey": "provideralpha001",
                        "feedback_event_id": "event-feedback-001",
                        "amount_msats": "2000",
                        "invoice_amount_sats": "2",
                        "bolt11_present": true
                    }
                }),
                json!({
                    "timestamp_ms": 1_762_700_004_000_u64,
                    "domain": {
                        "event": "buyer.selected_payable_provider",
                        "request_id": "req-log-001",
                        "provider_pubkey": "provideralpha001",
                        "previous_provider_pubkey": "providerbeta002",
                        "result_event_id": "event-result-001",
                        "feedback_event_id": "event-feedback-001",
                        "amount_msats": "2000",
                        "selection_source": "first_payable"
                    }
                }),
                json!({
                    "timestamp_ms": 1_762_700_005_000_u64,
                    "domain": {
                        "event": "buyer.payment_settled",
                        "request_id": "req-log-001",
                        "provider_pubkey": "provideralpha001",
                        "payment_pointer": "wallet-send-009",
                        "amount_sats": "2",
                        "fees_sats": "1",
                        "total_debit_sats": "3"
                    }
                }),
                json!({
                    "timestamp_ms": 1_762_700_006_000_u64,
                    "domain": {
                        "event": "buyer.seller_settled_pending_wallet_confirmation",
                        "request_id": "req-log-001",
                        "provider_pubkey": "provideralpha001",
                        "feedback_event_id": "event-feedback-success-001",
                        "payment_pointer": "wallet-send-009",
                        "local_wallet_status": "pending_receive"
                    }
                }),
            ],
        );

        let facts = super::load_log_backfill_facts_from_session_dir(
            session_dir.as_path(),
            Some("LOCALBUYERKEY"),
        )
        .expect("load log backfill facts");
        assert_eq!(facts.len(), 1);

        let fact = &facts[0];
        assert_eq!(fact.request_id, "req-log-001");
        assert_eq!(fact.request_event_id.as_deref(), Some("event-request-001"));
        assert_eq!(fact.request_type, "text-generation");
        assert_eq!(fact.buyer_nostr_pubkey.as_deref(), Some("localbuyerkey"));
        assert_eq!(
            fact.provider_nostr_pubkey.as_deref(),
            Some("provideralpha001")
        );
        assert_eq!(fact.invoice_event_id.as_deref(), Some("event-feedback-001"));
        assert_eq!(
            fact.seller_feedback_event_id.as_deref(),
            Some("event-feedback-success-001")
        );
        assert_eq!(
            fact.buyer_payment_pointer.as_deref(),
            Some("wallet-send-009")
        );
        assert_eq!(fact.amount_sats, Some(2));
        assert_eq!(fact.fees_sats, Some(1));
        assert_eq!(fact.total_debit_sats, Some(3));
        assert_eq!(
            fact.status,
            Nip90PaymentFactStatus::SellerSettlementObserved
        );
        assert_eq!(
            fact.source_quality,
            Nip90PaymentFactSourceQuality::LogBackfill
        );
        assert!(fact.provider_observation_history.iter().any(|event| {
            event.kind == NetworkRequestProviderObservationHistoryKind::ResultObserved
                && event.observed_event_id.as_deref() == Some("event-result-001")
        }));
        assert!(fact.provider_observation_history.iter().any(|event| {
            event.kind == NetworkRequestProviderObservationHistoryKind::PayableWinnerSelected
                && event.previous_provider_pubkey.as_deref() == Some("providerbeta002")
        }));

        let _ = std::fs::remove_dir_all(session_dir);
    }

    #[test]
    fn payment_fact_ledger_sync_imports_session_log_backfill_without_live_state() {
        let path = temp_path("nip90-payment-log-sync");
        let session_dir = temp_session_dir("nip90-payment-log-sync");
        std::fs::create_dir_all(&session_dir).expect("create session dir");
        let session_path = session_dir.join("20260313T231500Z-pid888.jsonl");
        write_session_log(
            &session_path,
            &[
                json!({
                    "timestamp_ms": 1_762_700_101_000_u64,
                    "source": "tracing",
                    "target": "autopilot_desktop::buyer",
                    "line": "Published NIP-90 request request_id=req-log-sync-001 request_type=text-generation event_id=event-request-sync-001 accepted_relays=1 rejected_relays=0"
                }),
                json!({
                    "timestamp_ms": 1_762_700_102_000_u64,
                    "domain": {
                        "event": "buyer.payment_settled",
                        "request_id": "req-log-sync-001",
                        "provider_pubkey": "providersync001",
                        "payment_pointer": "wallet-send-sync-001",
                        "amount_sats": "2",
                        "fees_sats": "0",
                        "total_debit_sats": "2"
                    }
                }),
            ],
        );

        let mut ledger = Nip90PaymentFactLedgerState::from_path_for_tests(path.clone());
        ledger.sync_from_current_truth_with_session_log_dir(
            &NetworkRequestsState::default(),
            &JobHistoryState::default(),
            &SparkPaneState::default(),
            Some("LOCALBUYERKEY"),
            session_dir.as_path(),
            true,
        );

        let fact = ledger
            .fact_for_request("req-log-sync-001")
            .expect("log-backed fact should be imported");
        assert_eq!(
            fact.source_quality,
            Nip90PaymentFactSourceQuality::LogBackfill
        );
        assert_eq!(
            fact.buyer_payment_pointer.as_deref(),
            Some("wallet-send-sync-001")
        );
        assert_eq!(
            fact.request_event_id.as_deref(),
            Some("event-request-sync-001")
        );
        assert!(
            ledger
                .last_action
                .as_deref()
                .is_some_and(|action| action.contains("log-backfill facts cached"))
        );

        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_dir_all(session_dir);
    }

    #[test]
    fn payment_fact_ledger_background_tick_defers_session_log_backfill_until_after_startup_delay() {
        let path = temp_path("nip90-payment-log-deferred");
        let session_dir = temp_session_dir("nip90-payment-log-deferred");
        std::fs::create_dir_all(&session_dir).expect("create session dir");
        let session_path = session_dir.join("20260313T232500Z-pid999.jsonl");
        write_session_log(
            &session_path,
            &[
                json!({
                    "timestamp_ms": 1_762_700_201_000_u64,
                    "source": "tracing",
                    "target": "autopilot_desktop::buyer",
                    "line": "Published NIP-90 request request_id=req-log-deferred-001 request_type=text-generation event_id=event-request-deferred-001 accepted_relays=1 rejected_relays=0"
                }),
                json!({
                    "timestamp_ms": 1_762_700_202_000_u64,
                    "domain": {
                        "event": "buyer.payment_settled",
                        "request_id": "req-log-deferred-001",
                        "provider_pubkey": "providerdeferred001",
                        "payment_pointer": "wallet-send-deferred-001",
                        "amount_sats": "5",
                        "fees_sats": "1",
                        "total_debit_sats": "6"
                    }
                }),
            ],
        );

        let mut ledger = Nip90PaymentFactLedgerState::from_path_for_tests(path.clone());
        let start = Instant::now();

        assert!(!ledger.sync_from_background_tick_with_session_log_dir(
            &NetworkRequestsState::default(),
            &JobHistoryState::default(),
            &SparkPaneState::default(),
            Some("LOCALBUYERKEY"),
            session_dir.as_path(),
            start,
        ));
        assert!(ledger.fact_for_request("req-log-deferred-001").is_none());

        assert!(ledger.sync_from_background_tick_with_session_log_dir(
            &NetworkRequestsState::default(),
            &JobHistoryState::default(),
            &SparkPaneState::default(),
            Some("LOCALBUYERKEY"),
            session_dir.as_path(),
            start + STARTUP_LOG_BACKFILL_DELAY + Duration::from_millis(1),
        ));
        let fact = ledger
            .fact_for_request("req-log-deferred-001")
            .expect("deferred log-backed fact should be imported");
        assert_eq!(
            fact.buyer_payment_pointer.as_deref(),
            Some("wallet-send-deferred-001")
        );

        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_dir_all(session_dir);
    }

    fn write_session_log(path: &std::path::Path, rows: &[serde_json::Value]) {
        let contents = rows
            .iter()
            .map(|row| serde_json::to_string(row).expect("encode session row"))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(path, format!("{contents}\n")).expect("write session log");
    }
}
