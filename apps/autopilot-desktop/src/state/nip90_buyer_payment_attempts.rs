use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::app_state::PaneLoadState;
use crate::spark_wallet::{
    SparkPaneState, is_settled_wallet_payment_status, is_terminal_wallet_payment_status,
    wallet_payment_total_debit_sats,
};
use crate::state::nip90_payment_facts::{Nip90PaymentFact, Nip90PaymentFactLedgerState};
use crate::state::operations::{
    BuyerPaymentAttemptObservation, NetworkRequestProviderObservationHistoryEvent,
    NetworkRequestProviderObservationHistoryKind, NetworkRequestsState, SubmittedNetworkRequest,
};

const NIP90_BUYER_PAYMENT_ATTEMPT_SCHEMA_VERSION: u16 = 1;
const NIP90_BUYER_PAYMENT_ATTEMPT_STREAM_ID: &str = "stream.nip90_buyer_payment_attempts.v1";
const LIVE_PROJECTION_REFRESH_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, Ord, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum Nip90BuyerPaymentBindingQuality {
    AppObserved,
    RequestFactBackfill,
}

impl Nip90BuyerPaymentBindingQuality {
    fn rank(self) -> u8 {
        match self {
            Self::RequestFactBackfill => 0,
            Self::AppObserved => 1,
        }
    }

    pub const fn is_degraded(self) -> bool {
        matches!(self, Self::RequestFactBackfill)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, Ord, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum Nip90BuyerPaymentSourceQuality {
    DegradedRecovery,
    PendingRequestObservation,
    WalletAuthoritative,
}

impl Nip90BuyerPaymentSourceQuality {
    fn rank(self) -> u8 {
        match self {
            Self::DegradedRecovery => 0,
            Self::PendingRequestObservation => 1,
            Self::WalletAuthoritative => 2,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Nip90BuyerPaymentRelayEvidence {
    pub request_event_id: Option<String>,
    pub result_event_id: Option<String>,
    pub invoice_event_id: Option<String>,
    pub request_publish_selected_relays: Vec<String>,
    pub request_publish_accepted_relays: Vec<String>,
    pub request_publish_rejected_relays: Vec<String>,
    pub result_observed_relays: Vec<String>,
    pub invoice_observed_relays: Vec<String>,
}

impl Nip90BuyerPaymentRelayEvidence {
    pub fn deduped_relay_count(&self) -> usize {
        let mut relays = BTreeSet::new();
        for relay in self
            .request_publish_selected_relays
            .iter()
            .chain(self.request_publish_accepted_relays.iter())
            .chain(self.request_publish_rejected_relays.iter())
            .chain(self.result_observed_relays.iter())
            .chain(self.invoice_observed_relays.iter())
        {
            relays.insert(relay.as_str());
        }
        relays.len()
    }

    fn merge_from(&mut self, other: &Self) {
        self.request_event_id = merge_optional_string(
            self.request_event_id.take(),
            other.request_event_id.clone(),
            true,
        );
        self.result_event_id = merge_optional_string(
            self.result_event_id.take(),
            other.result_event_id.clone(),
            true,
        );
        self.invoice_event_id = merge_optional_string(
            self.invoice_event_id.take(),
            other.invoice_event_id.clone(),
            true,
        );
        merge_string_vec(
            &mut self.request_publish_selected_relays,
            other.request_publish_selected_relays.as_slice(),
        );
        merge_string_vec(
            &mut self.request_publish_accepted_relays,
            other.request_publish_accepted_relays.as_slice(),
        );
        merge_string_vec(
            &mut self.request_publish_rejected_relays,
            other.request_publish_rejected_relays.as_slice(),
        );
        merge_string_vec(
            &mut self.result_observed_relays,
            other.result_observed_relays.as_slice(),
        );
        merge_string_vec(
            &mut self.invoice_observed_relays,
            other.invoice_observed_relays.as_slice(),
        );
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Nip90BuyerPaymentAttempt {
    pub payment_pointer: String,
    pub request_id: String,
    pub request_type: String,
    pub wallet_direction: String,
    pub wallet_status: String,
    pub wallet_confirmed_at: Option<u64>,
    pub wallet_first_seen_at: Option<u64>,
    pub amount_sats: Option<u64>,
    pub fees_sats: Option<u64>,
    pub total_debit_sats: Option<u64>,
    pub payment_hash: Option<String>,
    pub destination_pubkey: Option<String>,
    pub buyer_nostr_pubkey: Option<String>,
    pub provider_nostr_pubkey: Option<String>,
    pub binding_quality: Nip90BuyerPaymentBindingQuality,
    pub source_quality: Nip90BuyerPaymentSourceQuality,
    pub relay_evidence: Nip90BuyerPaymentRelayEvidence,
}

impl Nip90BuyerPaymentAttempt {
    pub fn effective_timestamp_epoch_seconds(&self) -> Option<u64> {
        self.wallet_confirmed_at.or(self.wallet_first_seen_at)
    }

    pub fn counts_in_definitive_totals(&self) -> bool {
        self.source_quality == Nip90BuyerPaymentSourceQuality::WalletAuthoritative
            && is_settled_wallet_payment_status(self.wallet_status.as_str())
            && self.wallet_direction.eq_ignore_ascii_case("send")
            && self.wallet_confirmed_at.is_some()
    }

    pub fn is_degraded(&self) -> bool {
        self.binding_quality.is_degraded()
            || self.source_quality == Nip90BuyerPaymentSourceQuality::DegradedRecovery
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Nip90BuyerPaymentWindowReport {
    pub start_epoch_seconds: u64,
    pub end_epoch_seconds: u64,
    pub payment_count: usize,
    pub total_sats_sent: u64,
    pub total_fee_sats: u64,
    pub total_wallet_debit_sats: u64,
    pub deduped_request_count: usize,
    pub degraded_binding_count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct Nip90BuyerPaymentAttemptDocumentV1 {
    schema_version: u16,
    stream_id: String,
    attempts: Vec<Nip90BuyerPaymentAttempt>,
}

pub struct Nip90BuyerPaymentAttemptLedgerState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub attempts: Vec<Nip90BuyerPaymentAttempt>,
    ledger_path: PathBuf,
    next_live_projection_refresh_at: Option<Instant>,
}

impl Default for Nip90BuyerPaymentAttemptLedgerState {
    fn default() -> Self {
        Self::from_path(default_nip90_buyer_payment_attempt_path())
    }
}

impl Nip90BuyerPaymentAttemptLedgerState {
    fn from_path(ledger_path: PathBuf) -> Self {
        let (attempts, load_state, last_error, last_action) =
            match load_nip90_buyer_payment_attempt_document(ledger_path.as_path()) {
                Ok(document) => (
                    document.attempts,
                    PaneLoadState::Ready,
                    None,
                    Some("Loaded NIP-90 buyer payment attempt ledger".to_string()),
                ),
                Err(error) => (
                    Vec::new(),
                    PaneLoadState::Error,
                    Some(error),
                    Some("NIP-90 buyer payment attempt ledger load failed".to_string()),
                ),
            };
        Self {
            load_state,
            last_error,
            last_action: last_action
                .map(|action| format!("{action} ({} attempts)", attempts.len())),
            attempts,
            ledger_path,
            next_live_projection_refresh_at: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn from_path_for_tests(ledger_path: PathBuf) -> Self {
        Self::from_path(ledger_path)
    }

    pub fn sync_from_current_truth(
        &mut self,
        network_requests: &NetworkRequestsState,
        spark_wallet: &SparkPaneState,
        payment_facts: &Nip90PaymentFactLedgerState,
        local_nostr_pubkey_hex: Option<&str>,
    ) -> bool {
        let now = Instant::now();
        self.next_live_projection_refresh_at = Some(now + LIVE_PROJECTION_REFRESH_INTERVAL);
        self.rebuild_from_current_truth(
            network_requests,
            spark_wallet,
            payment_facts,
            local_nostr_pubkey_hex,
        )
    }

    pub fn sync_from_background_tick(
        &mut self,
        network_requests: &NetworkRequestsState,
        spark_wallet: &SparkPaneState,
        payment_facts: &Nip90PaymentFactLedgerState,
        local_nostr_pubkey_hex: Option<&str>,
        now: Instant,
    ) -> bool {
        if matches!(
            self.next_live_projection_refresh_at,
            Some(next_refresh_at) if now < next_refresh_at
        ) {
            return false;
        }
        self.next_live_projection_refresh_at = Some(now + LIVE_PROJECTION_REFRESH_INTERVAL);
        self.rebuild_from_current_truth(
            network_requests,
            spark_wallet,
            payment_facts,
            local_nostr_pubkey_hex,
        )
    }

    pub fn window_report(
        &self,
        start_epoch_seconds: u64,
        end_epoch_seconds: u64,
    ) -> Nip90BuyerPaymentWindowReport {
        let mut report = Nip90BuyerPaymentWindowReport {
            start_epoch_seconds,
            end_epoch_seconds,
            ..Nip90BuyerPaymentWindowReport::default()
        };
        let mut counted_request_ids = BTreeSet::new();

        for attempt in self.attempts.iter().filter(|attempt| {
            attempt
                .effective_timestamp_epoch_seconds()
                .is_some_and(|timestamp| {
                    timestamp >= start_epoch_seconds && timestamp < end_epoch_seconds
                })
        }) {
            if attempt.is_degraded() {
                report.degraded_binding_count = report.degraded_binding_count.saturating_add(1);
            }
            if !attempt.counts_in_definitive_totals() {
                continue;
            }
            report.payment_count = report.payment_count.saturating_add(1);
            report.total_sats_sent = report
                .total_sats_sent
                .saturating_add(attempt.amount_sats.unwrap_or(0));
            report.total_fee_sats = report
                .total_fee_sats
                .saturating_add(attempt.fees_sats.unwrap_or(0));
            report.total_wallet_debit_sats = report
                .total_wallet_debit_sats
                .saturating_add(attempt.total_debit_sats.unwrap_or(0));
            counted_request_ids.insert(attempt.request_id.as_str());
        }

        report.deduped_request_count = counted_request_ids.len();
        report
    }

    fn rebuild_from_current_truth(
        &mut self,
        network_requests: &NetworkRequestsState,
        spark_wallet: &SparkPaneState,
        payment_facts: &Nip90PaymentFactLedgerState,
        local_nostr_pubkey_hex: Option<&str>,
    ) -> bool {
        let mut attempts_by_pointer = self
            .attempts
            .iter()
            .cloned()
            .map(|attempt| (attempt.payment_pointer.clone(), attempt))
            .collect::<BTreeMap<_, _>>();
        let wallet_payments = spark_wallet
            .recent_payments
            .iter()
            .filter(|payment| payment.direction.eq_ignore_ascii_case("send"))
            .map(|payment| (payment.id.as_str(), payment))
            .collect::<BTreeMap<_, _>>();

        for request in &network_requests.submitted {
            let relay_evidence = relay_evidence_from_request(request);
            let request_attempts = request_attempt_history(request);
            for attempt in request_attempts {
                let wallet_payment = wallet_payments
                    .get(attempt.payment_pointer.as_str())
                    .copied();
                let candidate = attempt_from_request(
                    request,
                    &attempt,
                    wallet_payment,
                    relay_evidence.clone(),
                    local_nostr_pubkey_hex,
                );
                merge_attempt(&mut attempts_by_pointer, candidate);
            }
        }

        for fact in &payment_facts.facts {
            let Some(candidate) = degraded_attempt_from_fact(fact, local_nostr_pubkey_hex) else {
                continue;
            };
            merge_attempt(&mut attempts_by_pointer, candidate);
        }

        let attempts = normalize_attempts(attempts_by_pointer.into_values().collect());
        if self.attempts == attempts {
            let mut changed = false;
            if self.load_state == PaneLoadState::Loading {
                self.load_state = PaneLoadState::Ready;
                changed = true;
            }
            if self.last_error.take().is_some() {
                changed = true;
            }
            if self.last_action.as_deref()
                != Some("NIP-90 buyer payment attempt ledger unchanged after sync")
            {
                self.last_action =
                    Some("NIP-90 buyer payment attempt ledger unchanged after sync".to_string());
                changed = true;
            }
            return changed;
        }

        if let Err(error) = persist_nip90_buyer_payment_attempt_document(
            self.ledger_path.as_path(),
            attempts.as_slice(),
        ) {
            let had_same_error = self.last_error.as_deref() == Some(error.as_str());
            self.last_error = Some(error);
            self.load_state = PaneLoadState::Error;
            self.last_action = Some("NIP-90 buyer payment attempt persist failed".to_string());
            return !had_same_error;
        }

        let attempt_count = attempts.len();
        self.attempts = attempts;
        self.load_state = PaneLoadState::Ready;
        self.last_error = None;
        self.last_action = Some(format!(
            "Rebuilt NIP-90 buyer payment attempt ledger ({} attempts)",
            attempt_count
        ));
        true
    }
}

fn default_nip90_buyer_payment_attempt_path() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".openagents")
        .join("autopilot-nip90-buyer-payment-attempts-v1.json")
}

fn request_attempt_history(
    request: &SubmittedNetworkRequest,
) -> Vec<BuyerPaymentAttemptObservation> {
    if !request.buyer_payment_attempts.is_empty() {
        return request.buyer_payment_attempts.clone();
    }
    let mut fallback = Vec::new();
    if let Some(payment_pointer) = request.last_payment_pointer.as_deref() {
        let observed_at = request
            .payment_sent_at_epoch_seconds
            .or(request.payment_required_at_epoch_seconds)
            .or(request.payment_failed_at_epoch_seconds)
            .unwrap_or(0);
        fallback.push(BuyerPaymentAttemptObservation {
            payment_pointer: payment_pointer.to_string(),
            first_observed_at_epoch_seconds: observed_at,
            last_updated_at_epoch_seconds: observed_at,
            settled_at_epoch_seconds: request.payment_sent_at_epoch_seconds,
            failed_at_epoch_seconds: request.payment_failed_at_epoch_seconds,
            failure_detail: request.payment_error.clone(),
        });
    }
    fallback
}

fn attempt_from_request(
    request: &SubmittedNetworkRequest,
    attempt: &BuyerPaymentAttemptObservation,
    wallet_payment: Option<&openagents_spark::PaymentSummary>,
    relay_evidence: Nip90BuyerPaymentRelayEvidence,
    local_nostr_pubkey_hex: Option<&str>,
) -> Nip90BuyerPaymentAttempt {
    let wallet_first_seen_at = match wallet_payment {
        Some(payment) => min_non_zero(
            Some(payment.timestamp),
            non_zero_epoch_seconds(attempt.first_observed_at_epoch_seconds),
        ),
        None => non_zero_epoch_seconds(attempt.first_observed_at_epoch_seconds),
    };
    let wallet_confirmed_at = wallet_payment.and_then(|payment| {
        if is_terminal_wallet_payment_status(payment.status.as_str()) {
            Some(payment.timestamp)
        } else {
            None
        }
    });

    Nip90BuyerPaymentAttempt {
        payment_pointer: attempt.payment_pointer.clone(),
        request_id: request.request_id.clone(),
        request_type: request.request_type.clone(),
        wallet_direction: wallet_payment
            .map(|payment| payment.direction.clone())
            .unwrap_or_else(|| "send".to_string()),
        wallet_status: wallet_payment
            .map(|payment| payment.status.clone())
            .unwrap_or_else(|| {
                if attempt.failed_at_epoch_seconds.is_some() {
                    "failed".to_string()
                } else if attempt.settled_at_epoch_seconds.is_some() {
                    "settled".to_string()
                } else {
                    "pending".to_string()
                }
            }),
        wallet_confirmed_at: wallet_confirmed_at
            .or(attempt.settled_at_epoch_seconds)
            .or(attempt.failed_at_epoch_seconds),
        wallet_first_seen_at,
        amount_sats: wallet_payment.map(|payment| payment.amount_sats),
        fees_sats: wallet_payment.map(|payment| payment.fees_sats),
        total_debit_sats: wallet_payment.map(wallet_payment_total_debit_sats),
        payment_hash: wallet_payment.and_then(|payment| payment.payment_hash.clone()),
        destination_pubkey: wallet_payment.and_then(|payment| payment.destination_pubkey.clone()),
        buyer_nostr_pubkey: normalize_optional_string(local_nostr_pubkey_hex),
        provider_nostr_pubkey: request
            .winning_provider_pubkey
            .clone()
            .or_else(|| request.result_provider_pubkey.clone())
            .or_else(|| request.invoice_provider_pubkey.clone())
            .or_else(|| request.last_provider_pubkey.clone()),
        binding_quality: Nip90BuyerPaymentBindingQuality::AppObserved,
        source_quality: if wallet_payment.is_some() {
            Nip90BuyerPaymentSourceQuality::WalletAuthoritative
        } else {
            Nip90BuyerPaymentSourceQuality::PendingRequestObservation
        },
        relay_evidence,
    }
}

fn degraded_attempt_from_fact(
    fact: &Nip90PaymentFact,
    local_nostr_pubkey_hex: Option<&str>,
) -> Option<Nip90BuyerPaymentAttempt> {
    let payment_pointer = normalize_optional_string(fact.buyer_payment_pointer.as_deref())?;
    Some(Nip90BuyerPaymentAttempt {
        payment_pointer,
        request_id: fact.request_id.clone(),
        request_type: fact.request_type.clone(),
        wallet_direction: "send".to_string(),
        wallet_status: fact.status.label().to_string(),
        wallet_confirmed_at: fact.buyer_wallet_confirmed_at,
        wallet_first_seen_at: fact
            .buyer_payment_pointer_at
            .or(fact.invoice_observed_at)
            .or(fact.result_observed_at)
            .or(fact.request_published_at),
        amount_sats: fact.amount_sats,
        fees_sats: fact.fees_sats,
        total_debit_sats: fact.total_debit_sats,
        payment_hash: fact.buyer_payment_hash.clone(),
        destination_pubkey: fact.lightning_destination_pubkey.clone(),
        buyer_nostr_pubkey: fact
            .buyer_nostr_pubkey
            .clone()
            .or_else(|| normalize_optional_string(local_nostr_pubkey_hex)),
        provider_nostr_pubkey: fact
            .provider_nostr_pubkey
            .clone()
            .or_else(|| fact.result_provider_pubkey.clone())
            .or_else(|| fact.invoice_provider_pubkey.clone()),
        binding_quality: Nip90BuyerPaymentBindingQuality::RequestFactBackfill,
        source_quality: Nip90BuyerPaymentSourceQuality::DegradedRecovery,
        relay_evidence: Nip90BuyerPaymentRelayEvidence {
            request_event_id: fact.request_event_id.clone(),
            result_event_id: fact.result_event_id.clone(),
            invoice_event_id: fact.invoice_event_id.clone(),
            request_publish_selected_relays: fact.selected_relays.clone(),
            request_publish_accepted_relays: fact.publish_accepted_relays.clone(),
            request_publish_rejected_relays: fact.publish_rejected_relays.clone(),
            result_observed_relays: fact.result_observed_relays.clone(),
            invoice_observed_relays: fact.invoice_observed_relays.clone(),
        },
    })
}

fn relay_evidence_from_request(
    request: &SubmittedNetworkRequest,
) -> Nip90BuyerPaymentRelayEvidence {
    let result_observed_relays =
        deduped_relays_from_history(request.provider_observation_history.as_slice(), |event| {
            event.kind == NetworkRequestProviderObservationHistoryKind::ResultObserved
        });
    let invoice_observed_relays = deduped_relays_from_history(
        request.provider_observation_history.as_slice(),
        provider_history_event_has_invoice_signal,
    );
    Nip90BuyerPaymentRelayEvidence {
        request_event_id: request.published_request_event_id.clone(),
        result_event_id: request
            .winning_result_event_id
            .clone()
            .or_else(|| request.last_result_event_id.clone()),
        invoice_event_id: request.last_feedback_event_id.clone(),
        request_publish_selected_relays: normalize_string_vec(
            request.request_publish_selected_relays.as_slice(),
        ),
        request_publish_accepted_relays: normalize_string_vec(
            request.request_publish_accepted_relays.as_slice(),
        ),
        request_publish_rejected_relays: normalize_string_vec(
            request.request_publish_rejected_relays.as_slice(),
        ),
        result_observed_relays,
        invoice_observed_relays,
    }
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

fn deduped_relays_from_history(
    history: &[NetworkRequestProviderObservationHistoryEvent],
    predicate: impl Fn(&NetworkRequestProviderObservationHistoryEvent) -> bool,
) -> Vec<String> {
    let mut relays = BTreeSet::new();
    for event in history.iter().filter(|event| predicate(event)) {
        for relay in &event.relay_urls {
            if let Some(relay) = normalize_optional_string(Some(relay.as_str())) {
                relays.insert(relay);
            }
        }
    }
    relays.into_iter().collect()
}

fn merge_attempt(
    attempts_by_pointer: &mut BTreeMap<String, Nip90BuyerPaymentAttempt>,
    incoming: Nip90BuyerPaymentAttempt,
) {
    let key = incoming.payment_pointer.clone();
    let Some(existing) = attempts_by_pointer.get_mut(key.as_str()) else {
        attempts_by_pointer.insert(key, incoming);
        return;
    };

    let incoming_is_stronger = incoming.source_quality.rank() > existing.source_quality.rank()
        || (incoming.source_quality.rank() == existing.source_quality.rank()
            && incoming.binding_quality.rank() >= existing.binding_quality.rank());

    if incoming.binding_quality.rank() > existing.binding_quality.rank() {
        existing.binding_quality = incoming.binding_quality;
    }
    if incoming.source_quality.rank() > existing.source_quality.rank() {
        existing.source_quality = incoming.source_quality;
    }
    existing.request_id = merge_required_string(
        existing.request_id.as_str(),
        incoming.request_id,
        incoming_is_stronger,
    );
    existing.request_type = merge_required_string(
        existing.request_type.as_str(),
        incoming.request_type,
        incoming_is_stronger,
    );
    existing.wallet_direction = merge_required_string(
        existing.wallet_direction.as_str(),
        incoming.wallet_direction,
        incoming_is_stronger,
    );
    existing.wallet_status = merge_required_string(
        existing.wallet_status.as_str(),
        incoming.wallet_status,
        incoming_is_stronger,
    );
    existing.wallet_confirmed_at =
        max_non_zero(existing.wallet_confirmed_at, incoming.wallet_confirmed_at);
    existing.wallet_first_seen_at =
        min_non_zero(existing.wallet_first_seen_at, incoming.wallet_first_seen_at);
    existing.amount_sats = merge_optional_u64(
        existing.amount_sats,
        incoming.amount_sats,
        incoming_is_stronger,
    );
    existing.fees_sats =
        merge_optional_u64(existing.fees_sats, incoming.fees_sats, incoming_is_stronger);
    existing.total_debit_sats = merge_optional_u64(
        existing.total_debit_sats,
        incoming.total_debit_sats,
        incoming_is_stronger,
    );
    existing.payment_hash = merge_optional_string(
        existing.payment_hash.take(),
        incoming.payment_hash,
        incoming_is_stronger,
    );
    existing.destination_pubkey = merge_optional_string(
        existing.destination_pubkey.take(),
        incoming.destination_pubkey,
        incoming_is_stronger,
    );
    existing.buyer_nostr_pubkey = merge_optional_string(
        existing.buyer_nostr_pubkey.take(),
        incoming.buyer_nostr_pubkey,
        incoming_is_stronger,
    );
    existing.provider_nostr_pubkey = merge_optional_string(
        existing.provider_nostr_pubkey.take(),
        incoming.provider_nostr_pubkey,
        incoming_is_stronger,
    );
    existing.relay_evidence.merge_from(&incoming.relay_evidence);
}

fn normalize_attempts(
    mut attempts: Vec<Nip90BuyerPaymentAttempt>,
) -> Vec<Nip90BuyerPaymentAttempt> {
    attempts.retain(|attempt| !attempt.payment_pointer.trim().is_empty());
    attempts.sort_by(|left, right| {
        right
            .effective_timestamp_epoch_seconds()
            .unwrap_or(0)
            .cmp(&left.effective_timestamp_epoch_seconds().unwrap_or(0))
            .then_with(|| left.payment_pointer.cmp(&right.payment_pointer))
    });
    attempts
}

fn normalize_optional_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn normalize_string_vec(values: &[String]) -> Vec<String> {
    let mut deduped = values
        .iter()
        .filter_map(|value| normalize_optional_string(Some(value.as_str())))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    deduped.sort();
    deduped
}

fn merge_string_vec(target: &mut Vec<String>, incoming: &[String]) {
    let mut merged = target
        .iter()
        .filter_map(|value| normalize_optional_string(Some(value.as_str())))
        .collect::<BTreeSet<_>>();
    for value in incoming {
        if let Some(value) = normalize_optional_string(Some(value.as_str())) {
            merged.insert(value);
        }
    }
    *target = merged.into_iter().collect();
}

fn merge_optional_string(
    existing: Option<String>,
    incoming: Option<String>,
    incoming_is_stronger: bool,
) -> Option<String> {
    let existing = existing.and_then(|value| normalize_optional_string(Some(value.as_str())));
    let incoming = incoming.and_then(|value| normalize_optional_string(Some(value.as_str())));
    if incoming_is_stronger {
        incoming.or(existing)
    } else {
        existing.or(incoming)
    }
}

fn merge_required_string(existing: &str, incoming: String, incoming_is_stronger: bool) -> String {
    merge_optional_string(
        normalize_optional_string(Some(existing)),
        normalize_optional_string(Some(incoming.as_str())),
        incoming_is_stronger,
    )
    .unwrap_or_default()
}

fn merge_optional_u64(
    existing: Option<u64>,
    incoming: Option<u64>,
    incoming_is_stronger: bool,
) -> Option<u64> {
    if incoming_is_stronger {
        incoming.or(existing)
    } else {
        existing.or(incoming)
    }
}

fn non_zero_epoch_seconds(value: u64) -> Option<u64> {
    (value > 0).then_some(value)
}

fn min_non_zero(existing: Option<u64>, incoming: Option<u64>) -> Option<u64> {
    match (
        existing.filter(|value| *value > 0),
        incoming.filter(|value| *value > 0),
    ) {
        (Some(existing), Some(incoming)) => Some(existing.min(incoming)),
        (Some(existing), None) => Some(existing),
        (None, Some(incoming)) => Some(incoming),
        (None, None) => None,
    }
}

fn max_non_zero(existing: Option<u64>, incoming: Option<u64>) -> Option<u64> {
    match (
        existing.filter(|value| *value > 0),
        incoming.filter(|value| *value > 0),
    ) {
        (Some(existing), Some(incoming)) => Some(existing.max(incoming)),
        (Some(existing), None) => Some(existing),
        (None, Some(incoming)) => Some(incoming),
        (None, None) => None,
    }
}

fn load_nip90_buyer_payment_attempt_document(
    path: &Path,
) -> Result<Nip90BuyerPaymentAttemptDocumentV1, String> {
    match fs::read_to_string(path) {
        Ok(contents) => {
            let document = serde_json::from_str::<Nip90BuyerPaymentAttemptDocumentV1>(&contents)
                .map_err(|error| {
                    format!(
                        "Parse NIP-90 buyer payment attempt ledger {}: {error}",
                        path.display()
                    )
                })?;
            if document.schema_version != NIP90_BUYER_PAYMENT_ATTEMPT_SCHEMA_VERSION {
                return Err(format!(
                    "Unsupported NIP-90 buyer payment attempt schema {} in {}",
                    document.schema_version,
                    path.display()
                ));
            }
            if document.stream_id != NIP90_BUYER_PAYMENT_ATTEMPT_STREAM_ID {
                return Err(format!(
                    "Unexpected NIP-90 buyer payment attempt stream {} in {}",
                    document.stream_id,
                    path.display()
                ));
            }
            Ok(document)
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {
            Ok(Nip90BuyerPaymentAttemptDocumentV1 {
                schema_version: NIP90_BUYER_PAYMENT_ATTEMPT_SCHEMA_VERSION,
                stream_id: NIP90_BUYER_PAYMENT_ATTEMPT_STREAM_ID.to_string(),
                attempts: Vec::new(),
            })
        }
        Err(error) => Err(format!(
            "Read NIP-90 buyer payment attempt ledger {}: {error}",
            path.display()
        )),
    }
}

fn persist_nip90_buyer_payment_attempt_document(
    path: &Path,
    attempts: &[Nip90BuyerPaymentAttempt],
) -> Result<(), String> {
    let document = Nip90BuyerPaymentAttemptDocumentV1 {
        schema_version: NIP90_BUYER_PAYMENT_ATTEMPT_SCHEMA_VERSION,
        stream_id: NIP90_BUYER_PAYMENT_ATTEMPT_STREAM_ID.to_string(),
        attempts: attempts.to_vec(),
    };
    let parent = path.parent().ok_or_else(|| {
        format!(
            "Resolve NIP-90 buyer payment attempt ledger parent for {}",
            path.display()
        )
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "Create NIP-90 buyer payment attempt ledger directory {}: {error}",
            parent.display()
        )
    })?;
    let json = serde_json::to_string_pretty(&document)
        .map_err(|error| format!("Serialize NIP-90 buyer payment attempt ledger: {error}"))?;
    fs::write(path, json).map_err(|error| {
        format!(
            "Persist NIP-90 buyer payment attempt ledger {}: {error}",
            path.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::{
        Nip90BuyerPaymentAttemptLedgerState, Nip90BuyerPaymentBindingQuality,
        Nip90BuyerPaymentSourceQuality,
    };
    use crate::spark_wallet::SparkPaneState;
    use crate::state::nip90_payment_facts::{
        Nip90PaymentFact, Nip90PaymentFactLedgerState, Nip90PaymentFactSourceQuality,
        Nip90PaymentFactStatus,
    };
    use crate::state::operations::{
        BuyerResolutionMode, BuyerResolutionReason, NetworkRequestProviderObservationHistoryEvent,
        NetworkRequestProviderObservationHistoryKind, NetworkRequestStatus, NetworkRequestsState,
        SubmittedNetworkRequest,
    };
    use openagents_spark::PaymentSummary;
    use std::path::PathBuf;

    fn unique_temp_path(label: &str) -> PathBuf {
        let unique = format!(
            "openagents-{}-{}-{}.json",
            label,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        );
        std::env::temp_dir().join(unique)
    }

    fn empty_payment_facts(label: &str) -> Nip90PaymentFactLedgerState {
        Nip90PaymentFactLedgerState::from_path_for_tests(unique_temp_path(label))
    }

    fn request_with_attempts(
        request_id: &str,
        attempts: &[(&str, u64, Option<u64>, Option<u64>)],
    ) -> SubmittedNetworkRequest {
        let mut request = SubmittedNetworkRequest {
            request_id: request_id.to_string(),
            published_request_event_id: Some(format!("event:{request_id}")),
            request_published_at_epoch_seconds: Some(1_762_700_000),
            request_publish_selected_relays: vec!["wss://relay.one".to_string()],
            request_publish_accepted_relays: vec!["wss://relay.one".to_string()],
            request_publish_rejected_relays: Vec::new(),
            request_type: "nip90.textgen".to_string(),
            payload: "hello".to_string(),
            resolution_mode: BuyerResolutionMode::Race,
            target_provider_pubkeys: Vec::new(),
            last_provider_pubkey: Some("provider-001".to_string()),
            result_provider_pubkey: Some("provider-001".to_string()),
            invoice_provider_pubkey: Some("provider-001".to_string()),
            last_feedback_status: Some("payment-required".to_string()),
            last_feedback_event_id: Some(format!("feedback:{request_id}")),
            last_result_event_id: Some(format!("result:{request_id}")),
            last_payment_pointer: attempts
                .last()
                .map(|(pointer, _, _, _)| (*pointer).to_string()),
            payment_required_at_epoch_seconds: Some(
                attempts
                    .first()
                    .map(|(_, observed_at, _, _)| *observed_at)
                    .unwrap_or(0),
            ),
            payment_sent_at_epoch_seconds: attempts
                .iter()
                .filter_map(|(_, _, settled_at, _)| *settled_at)
                .max(),
            payment_failed_at_epoch_seconds: attempts
                .iter()
                .filter_map(|(_, _, _, failed_at)| *failed_at)
                .max(),
            payment_error: attempts
                .iter()
                .rev()
                .find(|(_, _, _, failed_at)| failed_at.is_some())
                .map(|(pointer, _, _, _)| format!("failed {pointer}")),
            payment_notice: None,
            pending_bolt11: Some("lnbc20n1test".to_string()),
            skill_scope_id: None,
            credit_envelope_ref: None,
            budget_sats: 20,
            timeout_seconds: 90,
            response_stream_id: format!("stream:{request_id}"),
            status: NetworkRequestStatus::Paid,
            authority_command_seq: 1,
            authority_status: Some("accepted".to_string()),
            authority_event_id: Some(format!("authority:{request_id}")),
            authority_error_class: None,
            winning_provider_pubkey: Some("provider-001".to_string()),
            winning_result_event_id: Some(format!("result:{request_id}")),
            resolution_reason_code: Some(
                BuyerResolutionReason::FirstValidResult.code().to_string(),
            ),
            duplicate_outcomes: Vec::new(),
            resolution_feedbacks: Vec::new(),
            observed_buyer_event_ids: Vec::new(),
            provider_observations: Vec::new(),
            provider_observation_history: vec![
                NetworkRequestProviderObservationHistoryEvent {
                    history_id: format!("result:{request_id}"),
                    observed_order: 1,
                    observed_at_epoch_ms: 1_762_700_050_000,
                    kind: NetworkRequestProviderObservationHistoryKind::ResultObserved,
                    provider_pubkey: Some("provider-001".to_string()),
                    relay_urls: vec!["wss://relay.one".to_string(), "wss://relay.two".to_string()],
                    observed_event_id: Some(format!("result:{request_id}")),
                    status: Some("success".to_string()),
                    status_extra: None,
                    amount_msats: None,
                    bolt11_present: false,
                    previous_provider_pubkey: None,
                    winner_result_event_id: None,
                    winner_feedback_event_id: None,
                    selection_source: None,
                },
                NetworkRequestProviderObservationHistoryEvent {
                    history_id: format!("feedback:{request_id}"),
                    observed_order: 2,
                    observed_at_epoch_ms: 1_762_700_060_000,
                    kind: NetworkRequestProviderObservationHistoryKind::FeedbackObserved,
                    provider_pubkey: Some("provider-001".to_string()),
                    relay_urls: vec![
                        "wss://relay.two".to_string(),
                        "wss://relay.three".to_string(),
                    ],
                    observed_event_id: Some(format!("feedback:{request_id}")),
                    status: Some("payment-required".to_string()),
                    status_extra: None,
                    amount_msats: Some(20_000),
                    bolt11_present: true,
                    previous_provider_pubkey: None,
                    winner_result_event_id: None,
                    winner_feedback_event_id: None,
                    selection_source: None,
                },
            ],
            buyer_payment_attempts: attempts
                .iter()
                .map(|(pointer, observed_at, settled_at, failed_at)| {
                    crate::state::operations::BuyerPaymentAttemptObservation {
                        payment_pointer: (*pointer).to_string(),
                        first_observed_at_epoch_seconds: *observed_at,
                        last_updated_at_epoch_seconds: settled_at
                            .or(*failed_at)
                            .unwrap_or(*observed_at),
                        settled_at_epoch_seconds: *settled_at,
                        failed_at_epoch_seconds: *failed_at,
                        failure_detail: failed_at.map(|_| format!("failed {pointer}")),
                    }
                })
                .collect(),
        };
        if attempts
            .iter()
            .any(|(_, _, _, failed_at)| failed_at.is_some())
            && !attempts
                .iter()
                .any(|(_, _, settled_at, _)| settled_at.is_some())
        {
            request.status = NetworkRequestStatus::Failed;
        }
        request
    }

    fn wallet_payment(
        pointer: &str,
        status: &str,
        timestamp: u64,
        amount_sats: u64,
    ) -> PaymentSummary {
        PaymentSummary {
            id: pointer.to_string(),
            direction: "send".to_string(),
            status: status.to_string(),
            amount_sats,
            fees_sats: 1,
            timestamp,
            method: "bolt11".to_string(),
            description: Some("nip90 payment".to_string()),
            invoice: Some("lnbc20n1test".to_string()),
            destination_pubkey: Some("ln-destination".to_string()),
            payment_hash: Some(format!("hash:{pointer}")),
            htlc_status: None,
            htlc_expiry_epoch_seconds: None,
            status_detail: None,
        }
    }

    #[test]
    fn one_successful_send_counts_once() {
        let path = unique_temp_path("single-success");
        let mut ledger = Nip90BuyerPaymentAttemptLedgerState::from_path_for_tests(path);
        let mut requests = NetworkRequestsState::default();
        requests.submitted.push(request_with_attempts(
            "req-001",
            &[("wallet-001", 1_762_700_061, Some(1_762_700_062), None)],
        ));
        let mut wallet = SparkPaneState::default();
        wallet
            .recent_payments
            .push(wallet_payment("wallet-001", "settled", 1_762_700_062, 20));

        assert!(ledger.sync_from_current_truth(
            &requests,
            &wallet,
            &empty_payment_facts("single-success-facts"),
            Some("buyer-001"),
        ));

        let report = ledger.window_report(1_762_700_000, 1_762_700_100);
        assert_eq!(report.payment_count, 1);
        assert_eq!(report.total_sats_sent, 20);
        assert_eq!(report.total_fee_sats, 1);
        assert_eq!(report.total_wallet_debit_sats, 21);
        assert_eq!(report.deduped_request_count, 1);
        assert_eq!(report.degraded_binding_count, 0);
    }

    #[test]
    fn failed_send_then_successful_retry_are_retained_as_two_attempts() {
        let path = unique_temp_path("retry");
        let mut ledger = Nip90BuyerPaymentAttemptLedgerState::from_path_for_tests(path);
        let mut requests = NetworkRequestsState::default();
        requests.submitted.push(request_with_attempts(
            "req-retry",
            &[
                ("wallet-fail", 1_762_700_061, None, Some(1_762_700_062)),
                ("wallet-ok", 1_762_700_063, Some(1_762_700_064), None),
            ],
        ));
        let mut wallet = SparkPaneState::default();
        wallet
            .recent_payments
            .push(wallet_payment("wallet-fail", "failed", 1_762_700_062, 20));
        wallet
            .recent_payments
            .push(wallet_payment("wallet-ok", "settled", 1_762_700_064, 20));

        ledger.sync_from_current_truth(
            &requests,
            &wallet,
            &empty_payment_facts("retry-facts"),
            Some("buyer-001"),
        );

        assert_eq!(ledger.attempts.len(), 2);
        let report = ledger.window_report(1_762_700_000, 1_762_700_100);
        assert_eq!(report.payment_count, 1);
        assert_eq!(report.deduped_request_count, 1);
    }

    #[test]
    fn relay_fanin_dedupes_to_one_counted_attempt() {
        let path = unique_temp_path("relay-dedupe");
        let mut ledger = Nip90BuyerPaymentAttemptLedgerState::from_path_for_tests(path);
        let mut requests = NetworkRequestsState::default();
        requests.submitted.push(request_with_attempts(
            "req-relays",
            &[("wallet-relays", 1_762_700_061, Some(1_762_700_062), None)],
        ));
        let mut wallet = SparkPaneState::default();
        wallet.recent_payments.push(wallet_payment(
            "wallet-relays",
            "settled",
            1_762_700_062,
            20,
        ));

        ledger.sync_from_current_truth(
            &requests,
            &wallet,
            &empty_payment_facts("relay-dedupe-facts"),
            Some("buyer-001"),
        );

        let attempt = ledger.attempts.first().expect("attempt should exist");
        assert_eq!(
            attempt.binding_quality,
            Nip90BuyerPaymentBindingQuality::AppObserved
        );
        assert_eq!(attempt.relay_evidence.deduped_relay_count(), 3);
        assert_eq!(
            ledger
                .window_report(1_762_700_000, 1_762_700_100)
                .payment_count,
            1
        );
    }

    #[test]
    fn restart_preserves_same_daily_result() {
        let path = unique_temp_path("persist-reload");
        let mut ledger = Nip90BuyerPaymentAttemptLedgerState::from_path_for_tests(path.clone());
        let mut requests = NetworkRequestsState::default();
        requests.submitted.push(request_with_attempts(
            "req-persist",
            &[("wallet-persist", 1_762_700_061, Some(1_762_700_062), None)],
        ));
        let mut wallet = SparkPaneState::default();
        wallet.recent_payments.push(wallet_payment(
            "wallet-persist",
            "settled",
            1_762_700_062,
            20,
        ));

        ledger.sync_from_current_truth(
            &requests,
            &wallet,
            &empty_payment_facts("persist-facts"),
            Some("buyer-001"),
        );
        let first = ledger.window_report(1_762_700_000, 1_762_700_100);

        let reloaded = Nip90BuyerPaymentAttemptLedgerState::from_path_for_tests(path);
        let second = reloaded.window_report(1_762_700_000, 1_762_700_100);
        assert_eq!(first, second);
    }

    #[test]
    fn payment_attempt_history_is_not_truncated_by_request_fact_cap() {
        let path = unique_temp_path("over-cap");
        let mut ledger = Nip90BuyerPaymentAttemptLedgerState::from_path_for_tests(path);
        let mut requests = NetworkRequestsState::default();
        let mut wallet = SparkPaneState::default();

        for idx in 0..4_200_u64 {
            let request_id = format!("req-cap-{idx:04}");
            let pointer = format!("wallet-cap-{idx:04}");
            let timestamp = 1_762_700_000 + idx;
            requests.submitted.push(request_with_attempts(
                request_id.as_str(),
                &[(pointer.as_str(), timestamp, Some(timestamp), None)],
            ));
            wallet
                .recent_payments
                .push(wallet_payment(pointer.as_str(), "settled", timestamp, 1));
        }

        ledger.sync_from_current_truth(
            &requests,
            &wallet,
            &empty_payment_facts("over-cap-facts"),
            Some("buyer-001"),
        );

        let report = ledger.window_report(1_762_700_000, 1_762_705_000);
        assert_eq!(report.payment_count, 4_200);
        assert_eq!(ledger.attempts.len(), 4_200);
    }

    #[test]
    fn degraded_fact_backfill_rows_are_marked_and_excluded_from_topline() {
        let path = unique_temp_path("degraded");
        let mut ledger = Nip90BuyerPaymentAttemptLedgerState::from_path_for_tests(path);
        let mut facts = Nip90PaymentFactLedgerState::default();
        facts.facts.push(Nip90PaymentFact {
            fact_id: "fact-001".to_string(),
            request_id: "req-fact".to_string(),
            request_type: "nip90.textgen".to_string(),
            request_event_id: Some("event:req-fact".to_string()),
            result_event_id: Some("result:req-fact".to_string()),
            invoice_event_id: Some("feedback:req-fact".to_string()),
            seller_feedback_event_id: None,
            buyer_nostr_pubkey: Some("buyer-001".to_string()),
            provider_nostr_pubkey: Some("provider-001".to_string()),
            invoice_provider_pubkey: Some("provider-001".to_string()),
            result_provider_pubkey: Some("provider-001".to_string()),
            invoice_observed_relays: vec!["wss://relay.one".to_string()],
            result_observed_relays: vec!["wss://relay.two".to_string()],
            lightning_destination_pubkey: Some("ln-destination".to_string()),
            buyer_payment_pointer: Some("wallet-fact".to_string()),
            seller_payment_pointer: None,
            buyer_payment_hash: Some("hash:wallet-fact".to_string()),
            amount_sats: Some(20),
            fees_sats: Some(1),
            total_debit_sats: Some(21),
            wallet_method: Some("bolt11".to_string()),
            status: Nip90PaymentFactStatus::BuyerWalletSettled,
            settlement_authority: "wallet.reconciliation".to_string(),
            request_published_at: Some(1_762_700_000),
            result_observed_at: Some(1_762_700_050),
            invoice_observed_at: Some(1_762_700_060),
            buyer_payment_pointer_at: Some(1_762_700_061),
            seller_settlement_feedback_at: None,
            buyer_wallet_confirmed_at: Some(1_762_700_062),
            seller_wallet_confirmed_at: None,
            selected_relays: vec!["wss://relay.one".to_string()],
            publish_accepted_relays: vec!["wss://relay.one".to_string()],
            publish_rejected_relays: Vec::new(),
            provider_observation_history: Vec::new(),
            source_quality: Nip90PaymentFactSourceQuality::BuyerWalletReconciled,
        });

        ledger.sync_from_current_truth(
            &NetworkRequestsState::default(),
            &SparkPaneState::default(),
            &facts,
            Some("buyer-001"),
        );

        let attempt = ledger.attempts.first().expect("attempt should exist");
        assert_eq!(
            attempt.source_quality,
            Nip90BuyerPaymentSourceQuality::DegradedRecovery
        );
        let report = ledger.window_report(1_762_700_000, 1_762_700_100);
        assert_eq!(report.payment_count, 0);
        assert_eq!(report.degraded_binding_count, 1);
    }
}
