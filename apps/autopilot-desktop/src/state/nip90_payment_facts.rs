use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::app_state::{JobHistoryState, NetworkRequestsState, PaneLoadState};
use crate::nip90_compute_flow::{Nip90FlowPhase, build_buyer_request_flow_snapshot};
use crate::spark_wallet::{SparkPaneState, is_settled_wallet_payment_status};
use serde::{Deserialize, Serialize};

const NIP90_PAYMENT_FACT_SCHEMA_VERSION: u16 = 1;
const NIP90_PAYMENT_FACT_STREAM_ID: &str = "stream.nip90_payment_facts.v1";
const NIP90_PAYMENT_FACT_ROW_LIMIT: usize = 4096;
const NIP90_ACTOR_ROW_LIMIT: usize = 4096;
const NIP90_RELAY_HOP_ROW_LIMIT: usize = 8192;
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

pub struct Nip90PaymentFactLedgerState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub facts: Vec<Nip90PaymentFact>,
    pub actors: Vec<Nip90Actor>,
    pub relay_hops: Vec<Nip90RelayHop>,
    ledger_path: PathBuf,
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
    ) {
        let local_nostr_pubkey_hex = local_nostr_pubkey_hex
            .map(normalize_pubkey_key)
            .filter(|value| !value.is_empty());
        let mut facts_by_request = BTreeMap::<String, Nip90PaymentFact>::new();

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
            let wallet_payment = seller_wallet_by_pointer.get(row.payment_pointer.as_str()).copied();
            let fact = seller_fact_from_history_row(
                row,
                wallet_payment,
                local_nostr_pubkey_hex.as_deref(),
            );
            merge_fact(&mut facts_by_request, fact);
        }

        let facts = normalize_facts(facts_by_request.into_values().collect());
        let actors = derive_actors(facts.as_slice(), local_nostr_pubkey_hex.as_deref());
        let relay_hops = normalize_relay_hops(Vec::new());

        if self.facts == facts && self.actors == actors && self.relay_hops == relay_hops {
            if self.load_state == PaneLoadState::Loading {
                self.load_state = PaneLoadState::Ready;
            }
            return;
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
            self.last_error = Some(error);
            self.load_state = PaneLoadState::Error;
            self.last_action = Some("NIP-90 payment fact persist failed".to_string());
            return;
        }

        self.last_error = None;
        self.load_state = PaneLoadState::Ready;
        self.last_action = Some(format!("Rebuilt NIP-90 payment fact ledger ({} facts)", fact_count));
    }
}

fn default_nip90_payment_fact_path() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".openagents")
        .join("autopilot-nip90-payment-facts-v1.json")
}

fn buyer_fact_from_snapshot(
    snapshot: &crate::nip90_compute_flow::BuyerRequestFlowSnapshot,
    local_nostr_pubkey_hex: Option<&str>,
) -> Nip90PaymentFact {
    let status = match snapshot.phase {
        Nip90FlowPhase::Failed => Nip90PaymentFactStatus::Failed,
        Nip90FlowPhase::SellerSettledPendingWallet => Nip90PaymentFactStatus::SellerSettlementObserved,
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
        provider_nostr_pubkey: normalize_optional_string(snapshot.payable_provider_pubkey.as_deref())
            .or_else(|| normalize_optional_string(snapshot.result_provider_pubkey.as_deref()))
            .or_else(|| normalize_optional_string(snapshot.invoice_provider_pubkey.as_deref()))
            .or_else(|| normalize_optional_string(snapshot.selected_provider_pubkey.as_deref())),
        invoice_provider_pubkey: normalize_optional_string(snapshot.invoice_provider_pubkey.as_deref()),
        result_provider_pubkey: normalize_optional_string(snapshot.result_provider_pubkey.as_deref()),
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
        request_published_at: None,
        result_observed_at: None,
        invoice_observed_at: snapshot.payment_required_at_epoch_seconds,
        buyer_payment_pointer_at: snapshot.payment_sent_at_epoch_seconds,
        seller_settlement_feedback_at: None,
        buyer_wallet_confirmed_at: if status == Nip90PaymentFactStatus::BuyerWalletSettled {
            snapshot.timestamp
        } else {
            None
        },
        seller_wallet_confirmed_at: None,
        selected_relays: Vec::new(),
        publish_accepted_relays: Vec::new(),
        publish_rejected_relays: Vec::new(),
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
        lightning_destination_pubkey: None,
        buyer_payment_pointer: None,
        seller_payment_pointer: normalize_optional_string(Some(row.payment_pointer.as_str())),
        buyer_payment_hash: None,
        amount_sats: wallet_payment.map(|payment| payment.amount_sats).or(Some(row.payout_sats)),
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
        source_quality,
    }
}

fn merge_fact(map: &mut BTreeMap<String, Nip90PaymentFact>, mut incoming: Nip90PaymentFact) {
    if let Some(existing) = map.get_mut(incoming.request_id.as_str()) {
        existing.request_type = merge_string_field(
            Some(existing.request_type.clone()),
            Some(incoming.request_type.clone()),
        )
        .unwrap_or_else(|| "unknown".to_string());
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
        existing.total_debit_sats = merge_u64_field(existing.total_debit_sats, incoming.total_debit_sats);
        existing.wallet_method = merge_string_field(
            existing.wallet_method.take(),
            incoming.wallet_method.take(),
        );
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
        return;
    }

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
    right.or(left).and_then(|value| normalize_optional_string(Some(value.as_str())))
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

fn merge_status(left: Nip90PaymentFactStatus, right: Nip90PaymentFactStatus) -> Nip90PaymentFactStatus {
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
        Nip90PaymentFactStatus,
    };
    use crate::app_state::{
        BuyerResolutionMode, JobDemandSource, JobHistoryReceiptRow, JobHistoryState,
        JobHistoryStatus, NetworkRequestStatus,
    };
    use crate::spark_wallet::SparkPaneState;
    use crate::state::operations::{NetworkRequestSubmission, NetworkRequestsState};
    use openagents_spark::PaymentSummary;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should advance")
            .as_nanos();
        std::env::temp_dir().join(format!("openagents-{label}-{nonce}.json"))
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
        let request = network_requests
            .submitted
            .iter_mut()
            .find(|request| request.request_id == request_id)
            .expect("request should exist");
        request.published_request_event_id = Some("event-request-001".to_string());
        request.status = NetworkRequestStatus::Paid;
        request.result_provider_pubkey = Some("providerhex001".to_string());
        request.invoice_provider_pubkey = Some("providerhex001".to_string());
        request.winning_provider_pubkey = Some("providerhex001".to_string());
        request.winning_result_event_id = Some("event-result-001".to_string());
        request.last_feedback_event_id = Some("event-feedback-001".to_string());
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
        assert_eq!(buyer_fact.buyer_nostr_pubkey.as_deref(), Some("localpubkey001"));
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
        assert_eq!(seller_fact.seller_payment_pointer.as_deref(), Some("wallet-recv-001"));

        let buyer_actor_facts = ledger.facts_for_actor(Nip90ActorNamespace::Nostr, "localpubkey001");
        assert_eq!(buyer_actor_facts.len(), 1);
        let seller_actor_facts =
            ledger.facts_for_actor(Nip90ActorNamespace::Nostr, "providerpubkey-local");
        assert_eq!(seller_actor_facts.len(), 1);
        let lightning_actor_facts =
            ledger.facts_for_actor(Nip90ActorNamespace::LightningDestination, "02buyerdest");
        assert_eq!(lightning_actor_facts.len(), 1);
        assert_eq!(lightning_actor_facts[0].request_id, "req-buy-001");

        let reloaded = Nip90PaymentFactLedgerState::from_path_for_tests(path.clone());
        assert_eq!(reloaded.facts, ledger.facts);
        assert_eq!(reloaded.actors, ledger.actors);
        assert_eq!(reloaded.relay_hops, ledger.relay_hops);

        let _ = std::fs::remove_file(path);
    }
}
