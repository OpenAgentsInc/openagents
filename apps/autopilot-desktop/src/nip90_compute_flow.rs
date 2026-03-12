use std::time::{Instant, SystemTime, UNIX_EPOCH};

use chrono::{Local, TimeZone};
use wgpui::components::sections::TerminalStream;

use crate::app_state::{
    ActiveJobState, EarnJobLifecycleProjectionState, JobLifecycleStage,
    MISSION_CONTROL_BUY_MODE_INTERVAL_SECONDS, MISSION_CONTROL_BUY_MODE_REQUEST_TYPE,
    MissionControlPaneState,
};
use crate::spark_wallet::{
    SparkPaneState, is_settled_wallet_payment_status, is_terminal_wallet_payment_status,
    wallet_payment_total_debit_sats,
};
use crate::state::operations::{
    BuyerResolutionReason, NetworkRequestStatus, NetworkRequestsState, SubmittedNetworkRequest,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Nip90FlowAuthority {
    Ui,
    Relay,
    Provider,
    Wallet,
}

impl Nip90FlowAuthority {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Ui => "ui",
            Self::Relay => "relay",
            Self::Provider => "provider",
            Self::Wallet => "wallet",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Nip90FlowPhase {
    Preview,
    Submitted,
    Accepted,
    Executing,
    PublishingResult,
    Delivered,
    RequestingPayment,
    AwaitingPayment,
    Paid,
    Failed,
}

impl Nip90FlowPhase {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::Submitted => "submitted",
            Self::Accepted => "accepted",
            Self::Executing => "executing",
            Self::PublishingResult => "publishing-result",
            Self::Delivered => "delivered",
            Self::RequestingPayment => "requesting-payment",
            Self::AwaitingPayment => "awaiting-payment",
            Self::Paid => "paid",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BuyerRequestFlowSnapshot {
    pub request_id: String,
    pub request_type: String,
    pub budget_sats: u64,
    pub status: NetworkRequestStatus,
    pub authority: Nip90FlowAuthority,
    pub phase: Nip90FlowPhase,
    pub next_expected_event: String,
    pub published_request_event_id: Option<String>,
    pub selected_provider_pubkey: Option<String>,
    pub payable_provider_pubkey: Option<String>,
    pub last_feedback_status: Option<String>,
    pub last_feedback_event_id: Option<String>,
    pub last_result_event_id: Option<String>,
    pub winning_result_event_id: Option<String>,
    pub payment_pointer: Option<String>,
    pub payment_required_at_epoch_seconds: Option<u64>,
    pub payment_sent_at_epoch_seconds: Option<u64>,
    pub payment_failed_at_epoch_seconds: Option<u64>,
    pub pending_bolt11: Option<String>,
    pub payment_error: Option<String>,
    pub payment_notice: Option<String>,
    pub timestamp: Option<u64>,
    pub wallet_status: String,
    pub wallet_method: String,
    pub fees_sats: Option<u64>,
    pub total_debit_sats: Option<u64>,
    pub payment_hash: Option<String>,
    pub destination_pubkey: Option<String>,
    pub htlc_status: Option<String>,
    pub htlc_expiry_epoch_seconds: Option<u64>,
    pub wallet_detail: Option<String>,
    pub wallet_description: Option<String>,
    pub wallet_invoice: Option<String>,
    pub loser_provider_count: usize,
    pub loser_reason_summary: Option<String>,
}

impl BuyerRequestFlowSnapshot {
    pub(crate) fn selected_provider_pubkey(&self) -> Option<&str> {
        self.selected_provider_pubkey.as_deref()
    }

    pub(crate) fn provider_pubkey(&self) -> Option<&str> {
        self.payable_provider_pubkey
            .as_deref()
            .or(self.selected_provider_pubkey.as_deref())
    }

    pub(crate) fn selected_provider_label(&self) -> String {
        self.selected_provider_pubkey()
            .map(short_id)
            .unwrap_or_else(|| "none".to_string())
    }

    pub(crate) fn payable_provider_label(&self) -> String {
        self.payable_provider_pubkey
            .as_deref()
            .map(short_id)
            .unwrap_or_else(|| "none".to_string())
    }

    pub(crate) fn provider_label(&self) -> String {
        self.provider_pubkey()
            .map(short_id)
            .unwrap_or_else(|| "none".to_string())
    }

    pub(crate) fn provider_summary(&self) -> String {
        self.provider_pubkey()
            .map(|provider| format!("provider {}", short_id(provider)))
            .unwrap_or_else(|| "awaiting provider".to_string())
    }

    pub(crate) fn winner_selection_summary(&self) -> String {
        match (
            self.selected_provider_pubkey(),
            self.payable_provider_pubkey.as_deref(),
        ) {
            (Some(selected), Some(payable))
                if normalize_pubkey(selected) != normalize_pubkey(payable) =>
            {
                format!(
                    "selected {} // payable {}",
                    short_id(selected),
                    short_id(payable)
                )
            }
            (_, Some(payable)) => format!("payable {}", short_id(payable)),
            (Some(selected), None) => format!("selected {}", short_id(selected)),
            (None, None) => "awaiting provider".to_string(),
        }
    }

    pub(crate) fn work_label(&self) -> String {
        if self.status == NetworkRequestStatus::Failed {
            return "fault".to_string();
        }
        if self.last_result_event_id.is_some() {
            return "done".to_string();
        }
        match self.phase {
            Nip90FlowPhase::Paid => "done".to_string(),
            Nip90FlowPhase::AwaitingPayment => "invoice".to_string(),
            Nip90FlowPhase::RequestingPayment => "invoice".to_string(),
            Nip90FlowPhase::Delivered => "done".to_string(),
            Nip90FlowPhase::Executing => "working".to_string(),
            Nip90FlowPhase::Submitted => {
                if self.published_request_event_id.is_some() {
                    "searching".to_string()
                } else {
                    "queued".to_string()
                }
            }
            Nip90FlowPhase::Accepted => "accepted".to_string(),
            Nip90FlowPhase::PublishingResult => "publishing".to_string(),
            Nip90FlowPhase::Preview => "preview".to_string(),
            Nip90FlowPhase::Failed => "fault".to_string(),
        }
    }

    pub(crate) fn work_summary(&self) -> String {
        if self.status == NetworkRequestStatus::Failed {
            return "request failed".to_string();
        }
        if self.last_result_event_id.is_some() {
            return "result received".to_string();
        }
        match self.phase {
            Nip90FlowPhase::Paid => "payment settled".to_string(),
            Nip90FlowPhase::AwaitingPayment => "invoice received".to_string(),
            Nip90FlowPhase::RequestingPayment => {
                if self.pending_bolt11.is_some() {
                    "invoice received".to_string()
                } else {
                    "awaiting valid invoice".to_string()
                }
            }
            Nip90FlowPhase::Delivered => "result received".to_string(),
            Nip90FlowPhase::Executing => "provider working".to_string(),
            Nip90FlowPhase::Submitted => {
                if self.published_request_event_id.is_some() {
                    "request published".to_string()
                } else {
                    "queued locally".to_string()
                }
            }
            Nip90FlowPhase::Accepted => "provider accepted".to_string(),
            Nip90FlowPhase::PublishingResult => "provider publishing result".to_string(),
            Nip90FlowPhase::Preview => "relay preview".to_string(),
            Nip90FlowPhase::Failed => "request failed".to_string(),
        }
    }

    fn log_work_state(&self) -> String {
        if self.status == NetworkRequestStatus::Failed {
            return "failed".to_string();
        }
        if self.last_result_event_id.is_some() {
            return "result-received".to_string();
        }
        match self.phase {
            Nip90FlowPhase::Paid | Nip90FlowPhase::Delivered => "result-received".to_string(),
            Nip90FlowPhase::AwaitingPayment | Nip90FlowPhase::RequestingPayment => {
                "invoice-requested".to_string()
            }
            Nip90FlowPhase::Executing => "provider-working".to_string(),
            Nip90FlowPhase::Accepted => "accepted".to_string(),
            Nip90FlowPhase::PublishingResult => "publishing-result".to_string(),
            Nip90FlowPhase::Submitted => {
                if self.published_request_event_id.is_some() {
                    "awaiting-provider".to_string()
                } else {
                    "queued".to_string()
                }
            }
            Nip90FlowPhase::Preview => "preview".to_string(),
            Nip90FlowPhase::Failed => "failed".to_string(),
        }
    }

    pub(crate) fn payment_summary(&self) -> String {
        match self.wallet_status.as_str() {
            "sent" => {
                let amount = self
                    .total_debit_sats
                    .or(Some(self.budget_sats))
                    .unwrap_or(self.budget_sats);
                if let Some(fees) = self.fees_sats {
                    if fees > 0 {
                        return format!("payment sent (wallet debit {amount} sats)");
                    }
                }
                "payment sent".to_string()
            }
            "returned" => {
                let amount = self
                    .total_debit_sats
                    .or(Some(self.budget_sats))
                    .unwrap_or(self.budget_sats);
                format!("payment returned (wallet debit {amount} sats)")
            }
            "failed" => self
                .wallet_detail
                .clone()
                .or_else(|| self.payment_error.clone())
                .unwrap_or_else(|| "payment failed".to_string()),
            "pending" => self
                .wallet_detail
                .clone()
                .unwrap_or_else(|| "payment pending Spark confirmation".to_string()),
            "queued" => "payment queued".to_string(),
            "invoice" => "invoice received".to_string(),
            _ => "payment idle".to_string(),
        }
    }

    pub(crate) fn mission_control_log_line(&self) -> String {
        let mut line = format!(
            "Buyer {} [{}] {} {} auth={} phase={} next={} provider={} work={} payment={}",
            mission_control_log_short_id(self.request_id.as_str()),
            self.request_type,
            self.status.label(),
            format_mission_control_amount(self.budget_sats),
            self.authority.as_str(),
            self.phase.as_str(),
            self.next_expected_event,
            self.provider_pubkey()
                .map(mission_control_log_short_id)
                .unwrap_or_else(|| "awaiting".to_string()),
            self.log_work_state(),
            self.wallet_status,
        );
        if let Some(event_id) = self.published_request_event_id.as_deref() {
            line.push_str(" event=");
            line.push_str(mission_control_log_short_id(event_id).as_str());
        }
        if let Some(status) = self.last_feedback_status.as_deref() {
            line.push_str(" feedback=");
            line.push_str(status);
        }
        if self.last_result_event_id.is_some() {
            line.push_str(" result=received");
        }
        if let Some(pointer) = self.payment_pointer.as_deref() {
            line.push_str(" pointer=");
            line.push_str(mission_control_log_short_id(pointer).as_str());
        }
        if let Some(selected) = self.selected_provider_pubkey() {
            line.push_str(" selected=");
            line.push_str(mission_control_log_short_id(selected).as_str());
        }
        if let Some(payable) = self.payable_provider_pubkey.as_deref() {
            if self
                .selected_provider_pubkey()
                .is_none_or(|selected| normalize_pubkey(selected) != normalize_pubkey(payable))
            {
                line.push_str(" payable=");
                line.push_str(mission_control_log_short_id(payable).as_str());
            }
        }
        if self.loser_provider_count > 0 {
            line.push_str(" losers=");
            line.push_str(self.loser_provider_count.to_string().as_str());
        }
        if let Some(summary) = self.loser_reason_summary.as_deref() {
            line.push_str(" loser_summary=");
            line.push_str(summary);
        }
        if let Some(fees) = self.fees_sats {
            line.push_str(" fee_sats=");
            line.push_str(&fees.to_string());
        }
        if let Some(total) = self.total_debit_sats {
            line.push_str(" wallet_debit_sats=");
            line.push_str(&total.to_string());
        }
        if let Some(invoice) = self.pending_bolt11.as_deref() {
            line.push_str(" invoice=");
            line.push_str(compact_payment_invoice(invoice).as_str());
        }
        if let Some(notice) = self.payment_notice.as_deref() {
            line.push_str(" notice=");
            line.push_str(notice);
        }
        if let Some(error) = self.payment_error.as_deref() {
            line.push_str(" error=");
            line.push_str(error);
        }
        line
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ActiveJobFlowSnapshot {
    pub job_id: String,
    pub request_id: String,
    pub capability: String,
    pub quoted_price_sats: u64,
    pub stage: JobLifecycleStage,
    pub authority: Nip90FlowAuthority,
    pub phase: Nip90FlowPhase,
    pub next_expected_event: String,
    pub projection_authority: String,
    pub pending_result_publish_event_id: Option<String>,
    pub result_event_id: Option<String>,
    pub result_signed: bool,
    pub result_publish_status: String,
    pub result_publish_attempt_count: u32,
    pub result_publish_age_seconds: Option<u64>,
    pub payment_pointer: Option<String>,
    pub pending_bolt11: Option<String>,
    pub continuity_window_seconds: Option<u64>,
    pub failure_reason: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Nip90ComputeFlowSnapshot {
    pub recent_requests: Vec<BuyerRequestFlowSnapshot>,
    pub active_job: Option<ActiveJobFlowSnapshot>,
}

impl ActiveJobFlowSnapshot {
    pub(crate) fn mission_control_continuity_summary(&self) -> Option<String> {
        match self.phase {
            Nip90FlowPhase::PublishingResult => Some(format!(
                "{} // relay attempt {} // age {} // window {}",
                if self.result_signed {
                    "result signed"
                } else {
                    "result pending"
                },
                self.result_publish_attempt_count.max(1),
                self.result_publish_age_seconds
                    .map(|age| format!("{age}s"))
                    .unwrap_or_else(|| "-".to_string()),
                self.continuity_window_seconds
                    .map(|window| format!("{window}s"))
                    .unwrap_or_else(|| "-".to_string()),
            )),
            Nip90FlowPhase::AwaitingPayment => Some(format!(
                "waiting on settlement // pointer {} // window {}",
                self.payment_pointer
                    .as_deref()
                    .map(|pointer| short_id(pointer).to_string())
                    .unwrap_or_else(|| "pending".to_string()),
                self.continuity_window_seconds
                    .map(|window| format!("{window}s"))
                    .unwrap_or_else(|| "-".to_string()),
            )),
            Nip90FlowPhase::RequestingPayment => Some(format!(
                "{} // window {}",
                if self.pending_bolt11.is_some() {
                    "waiting on wallet payment"
                } else {
                    "waiting on provider invoice"
                },
                self.continuity_window_seconds
                    .map(|window| format!("{window}s"))
                    .unwrap_or_else(|| "-".to_string()),
            )),
            _ => None,
        }
    }
}

pub(crate) fn build_nip90_compute_flow_snapshot(
    network_requests: &NetworkRequestsState,
    spark_wallet: &SparkPaneState,
    active_job: &ActiveJobState,
    earn_job_lifecycle_projection: &EarnJobLifecycleProjectionState,
) -> Nip90ComputeFlowSnapshot {
    Nip90ComputeFlowSnapshot {
        recent_requests: network_requests
            .submitted
            .iter()
            .rev()
            .map(|request| build_buyer_request_flow_snapshot(request, spark_wallet))
            .collect(),
        active_job: build_active_job_flow_snapshot(active_job, earn_job_lifecycle_projection),
    }
}

pub(crate) fn buy_mode_wallet_payment<'a>(
    request: &SubmittedNetworkRequest,
    spark_wallet: &'a SparkPaneState,
) -> Option<&'a openagents_spark::PaymentSummary> {
    request
        .last_payment_pointer
        .as_deref()
        .and_then(|payment_id| {
            spark_wallet
                .recent_payments
                .iter()
                .find(|payment| payment.id == payment_id)
        })
}

pub(crate) fn buy_mode_wallet_state_label(
    request: &SubmittedNetworkRequest,
    wallet_payment: Option<&openagents_spark::PaymentSummary>,
) -> String {
    if request.payment_sent_at_epoch_seconds.is_some()
        || request.status == NetworkRequestStatus::Paid
    {
        return "sent".to_string();
    }
    if let Some(payment) = wallet_payment {
        if payment.is_returned_htlc_failure() {
            return "returned".to_string();
        }
        if is_terminal_wallet_payment_status(payment.status.as_str()) {
            return "failed".to_string();
        }
        return "pending".to_string();
    }
    if request.last_payment_pointer.is_some() {
        return "pending".to_string();
    }
    if request.pending_bolt11.is_some() {
        return "queued".to_string();
    }
    if request.payment_required_at_epoch_seconds.is_some() {
        return "invoice".to_string();
    }
    if request.payment_error.is_some() {
        return "failed".to_string();
    }
    "idle".to_string()
}

pub(crate) fn compact_payment_invoice(invoice: &str) -> String {
    let trimmed = invoice.trim();
    if trimmed.len() <= 32 {
        trimmed.to_string()
    } else {
        format!("{}..{}", &trimmed[..16], &trimmed[trimmed.len() - 12..])
    }
}

pub(crate) fn continuity_window_seconds(ttl_seconds: u64) -> u64 {
    ttl_seconds.saturating_add(120).max(180)
}

pub(crate) fn build_buyer_request_flow_snapshot(
    request: &SubmittedNetworkRequest,
    spark_wallet: &SparkPaneState,
) -> BuyerRequestFlowSnapshot {
    let wallet_payment = buy_mode_wallet_payment(request, spark_wallet);
    let wallet_status = buy_mode_wallet_state_label(request, wallet_payment);
    let selected_provider_pubkey = request
        .last_provider_pubkey
        .clone()
        .or_else(|| request.winning_provider_pubkey.clone());
    let payable_provider_pubkey = request.winning_provider_pubkey.clone();
    let (loser_provider_count, loser_reason_summary) =
        loser_provider_summary(request, payable_provider_pubkey.as_deref());

    let (authority, phase, next_expected_event) =
        if request.payment_error.is_some() || request.status == NetworkRequestStatus::Failed {
            (
                if request.last_payment_pointer.is_some() {
                    Nip90FlowAuthority::Wallet
                } else {
                    Nip90FlowAuthority::Provider
                },
                Nip90FlowPhase::Failed,
                "none".to_string(),
            )
        } else if request.last_payment_pointer.is_some() {
            if request.payment_sent_at_epoch_seconds.is_some()
                || request.status == NetworkRequestStatus::Paid
            {
                (
                    Nip90FlowAuthority::Wallet,
                    Nip90FlowPhase::Paid,
                    "none".to_string(),
                )
            } else {
                (
                    Nip90FlowAuthority::Wallet,
                    Nip90FlowPhase::AwaitingPayment,
                    "wallet settlement".to_string(),
                )
            }
        } else if request.pending_bolt11.is_some() {
            (
                Nip90FlowAuthority::Wallet,
                Nip90FlowPhase::AwaitingPayment,
                "wallet payment".to_string(),
            )
        } else if request.payment_required_at_epoch_seconds.is_some()
            || request
                .last_feedback_status
                .as_deref()
                .is_some_and(|status| status.eq_ignore_ascii_case("payment-required"))
        {
            let next = if request.pending_bolt11.is_some() {
                "wallet payment"
            } else {
                "valid provider invoice"
            };
            (
                Nip90FlowAuthority::Provider,
                Nip90FlowPhase::RequestingPayment,
                next.to_string(),
            )
        } else if request.last_result_event_id.is_some() {
            (
                Nip90FlowAuthority::Provider,
                Nip90FlowPhase::Delivered,
                "provider invoice".to_string(),
            )
        } else if request.status == NetworkRequestStatus::Processing
            || request
                .last_feedback_status
                .as_deref()
                .is_some_and(|status| status.eq_ignore_ascii_case("processing"))
        {
            (
                Nip90FlowAuthority::Provider,
                Nip90FlowPhase::Executing,
                "provider result".to_string(),
            )
        } else if request.published_request_event_id.is_some() {
            (
                Nip90FlowAuthority::Relay,
                Nip90FlowPhase::Submitted,
                "provider response".to_string(),
            )
        } else {
            (
                Nip90FlowAuthority::Ui,
                Nip90FlowPhase::Submitted,
                "request publish".to_string(),
            )
        };

    BuyerRequestFlowSnapshot {
        request_id: request.request_id.clone(),
        request_type: request.request_type.clone(),
        budget_sats: request.budget_sats,
        status: request.status,
        authority,
        phase,
        next_expected_event,
        published_request_event_id: request.published_request_event_id.clone(),
        selected_provider_pubkey,
        payable_provider_pubkey,
        last_feedback_status: request.last_feedback_status.clone(),
        last_feedback_event_id: request.last_feedback_event_id.clone(),
        last_result_event_id: request.last_result_event_id.clone(),
        winning_result_event_id: request.winning_result_event_id.clone(),
        payment_pointer: request.last_payment_pointer.clone(),
        payment_required_at_epoch_seconds: request.payment_required_at_epoch_seconds,
        payment_sent_at_epoch_seconds: request.payment_sent_at_epoch_seconds,
        payment_failed_at_epoch_seconds: request.payment_failed_at_epoch_seconds,
        pending_bolt11: request.pending_bolt11.clone(),
        payment_error: request.payment_error.clone(),
        payment_notice: request.payment_notice.clone(),
        timestamp: wallet_payment
            .map(|payment| payment.timestamp)
            .or(request.payment_sent_at_epoch_seconds)
            .or(request.payment_failed_at_epoch_seconds)
            .or(request.payment_required_at_epoch_seconds),
        wallet_status,
        wallet_method: wallet_payment
            .map(|payment| payment.method.clone())
            .unwrap_or_else(|| "-".to_string()),
        fees_sats: wallet_payment.map(|payment| payment.fees_sats),
        total_debit_sats: wallet_payment.map(wallet_payment_total_debit_sats),
        payment_hash: wallet_payment.and_then(|payment| payment.payment_hash.clone()),
        destination_pubkey: wallet_payment.and_then(|payment| payment.destination_pubkey.clone()),
        htlc_status: wallet_payment.and_then(|payment| payment.htlc_status.clone()),
        htlc_expiry_epoch_seconds: wallet_payment
            .and_then(|payment| payment.htlc_expiry_epoch_seconds),
        wallet_detail: wallet_payment.and_then(|payment| payment.status_detail.clone()),
        wallet_description: wallet_payment.and_then(|payment| payment.description.clone()),
        wallet_invoice: wallet_payment.and_then(|payment| payment.invoice.clone()),
        loser_provider_count,
        loser_reason_summary,
    }
}

pub(crate) fn build_active_job_flow_snapshot(
    active_job: &ActiveJobState,
    earn_job_lifecycle_projection: &EarnJobLifecycleProjectionState,
) -> Option<ActiveJobFlowSnapshot> {
    let job = active_job.job.as_ref()?;
    let now_epoch_seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    let authoritative_payment = authoritative_payment_pointer(job.payment_id.as_deref())
        || job.ac_settlement_event_id.is_some();
    let continuity_window = continuity_window_seconds(job.ttl_seconds);
    let (authority, phase, next_expected_event, continuity_window_seconds) =
        if job.stage == JobLifecycleStage::Paid || authoritative_payment {
            (
                Nip90FlowAuthority::Wallet,
                Nip90FlowPhase::Paid,
                "none".to_string(),
                None,
            )
        } else if job.stage == JobLifecycleStage::Failed {
            let authority = if active_job.result_publish_in_flight
                || active_job.pending_result_publish_event_id.is_some()
            {
                Nip90FlowAuthority::Relay
            } else if job.sa_tick_result_event_id.is_some()
                || active_job.pending_bolt11.is_some()
                || active_job.payment_required_invoice_requested
                || active_job.payment_required_feedback_in_flight
            {
                Nip90FlowAuthority::Wallet
            } else {
                Nip90FlowAuthority::Provider
            };
            (authority, Nip90FlowPhase::Failed, "none".to_string(), None)
        } else if job.sa_tick_result_event_id.is_none()
            && (active_job.result_publish_in_flight
                || active_job.pending_result_publish_event_id.is_some())
        {
            (
                Nip90FlowAuthority::Relay,
                Nip90FlowPhase::PublishingResult,
                "relay confirmation".to_string(),
                Some(continuity_window),
            )
        } else if job.stage == JobLifecycleStage::Delivered {
            if active_job.pending_bolt11.is_some() {
                (
                    Nip90FlowAuthority::Wallet,
                    Nip90FlowPhase::AwaitingPayment,
                    "wallet settlement".to_string(),
                    Some(continuity_window),
                )
            } else {
                (
                    Nip90FlowAuthority::Provider,
                    Nip90FlowPhase::RequestingPayment,
                    "Lightning invoice".to_string(),
                    Some(continuity_window),
                )
            }
        } else if job.stage == JobLifecycleStage::Running {
            (
                Nip90FlowAuthority::Provider,
                Nip90FlowPhase::Executing,
                "local execution".to_string(),
                None,
            )
        } else {
            (
                Nip90FlowAuthority::Provider,
                Nip90FlowPhase::Accepted,
                "runtime execution".to_string(),
                None,
            )
        };

    Some(ActiveJobFlowSnapshot {
        job_id: job.job_id.clone(),
        request_id: job.request_id.clone(),
        capability: job.capability.clone(),
        quoted_price_sats: job.quoted_price_sats,
        stage: job.stage,
        authority,
        phase,
        next_expected_event,
        projection_authority: earn_job_lifecycle_projection.authority.clone(),
        pending_result_publish_event_id: active_job.pending_result_publish_event_id.clone(),
        result_event_id: job.sa_tick_result_event_id.clone(),
        result_signed: active_job.pending_result_publish_event.is_some()
            || active_job.pending_result_publish_event_id.is_some()
            || job.sa_tick_result_event_id.is_some(),
        result_publish_status: active_job_result_publish_status(active_job),
        result_publish_attempt_count: active_job.result_publish_attempt_count,
        result_publish_age_seconds: active_job
            .result_publish_first_queued_epoch_seconds
            .map(|queued_at| now_epoch_seconds.saturating_sub(queued_at)),
        payment_pointer: job.payment_id.clone(),
        pending_bolt11: active_job.pending_bolt11.clone(),
        continuity_window_seconds,
        failure_reason: job.failure_reason.clone(),
    })
}

pub(crate) fn buy_mode_payments_summary_text(
    network_requests: &NetworkRequestsState,
    spark_wallet: &SparkPaneState,
) -> String {
    let entries = buy_mode_payment_ledger_entries(network_requests, spark_wallet);
    let live_rows = entries
        .iter()
        .filter(|entry| entry.source == "request")
        .count();
    let wallet_backfill_rows = entries.len().saturating_sub(live_rows);
    let mut paid = 0usize;
    let mut pending = 0usize;
    let mut returned = 0usize;
    let mut failed = 0usize;
    let mut sats_sent = 0u64;
    let mut fee_sats = 0u64;
    let mut wallet_debit_sats = 0u64;
    for entry in &entries {
        fee_sats = fee_sats.saturating_add(entry.fees_sats.unwrap_or(0));
        wallet_debit_sats = wallet_debit_sats.saturating_add(entry.total_debit_sats.unwrap_or(0));
        if entry.wallet_status == "sent" {
            paid = paid.saturating_add(1);
            sats_sent = sats_sent.saturating_add(entry.amount_sats);
        } else if entry.wallet_status == "returned" {
            returned = returned.saturating_add(1);
        } else if entry.wallet_status == "failed" {
            failed = failed.saturating_add(1);
        } else {
            pending = pending.saturating_add(1);
        }
    }

    format!(
        "{} rows  //  {} live  //  {} wallet-backfill  //  {} sent  //  {} pending  //  {} returned  //  {} failed  //  {} sats  //  {} fee sats  //  {} wallet debit sats",
        entries.len(),
        live_rows,
        wallet_backfill_rows,
        paid,
        pending,
        returned,
        failed,
        sats_sent,
        fee_sats,
        wallet_debit_sats
    )
}

pub(crate) fn buy_mode_payments_status_lines(
    mission_control: &MissionControlPaneState,
    network_requests: &NetworkRequestsState,
    spark_wallet: &SparkPaneState,
    now: Instant,
) -> Vec<String> {
    let snapshots = buy_mode_request_flow_snapshots(network_requests, spark_wallet);
    let in_flight = snapshots
        .iter()
        .find(|request| !request.status.is_terminal());
    let loop_line = if !mission_control.buy_mode_loop_enabled {
        format!(
            "Dispatch loop: off // cadence={}s // policy=single-flight",
            MISSION_CONTROL_BUY_MODE_INTERVAL_SECONDS
        )
    } else if let Some(request) = in_flight {
        format!(
            "Dispatch loop: on // cadence={}s // policy=single-flight // blocked by {} [{} phase={} auth={} next={}]",
            MISSION_CONTROL_BUY_MODE_INTERVAL_SECONDS,
            compact_buy_mode_request_id(request.request_id.as_str()),
            request.status.label(),
            request.phase.as_str(),
            request.authority.as_str(),
            request.next_expected_event,
        )
    } else {
        let next = mission_control
            .buy_mode_next_dispatch_countdown_seconds(now)
            .map(|seconds| {
                if seconds == 0 {
                    "now".to_string()
                } else {
                    format!("{seconds}s")
                }
            })
            .unwrap_or_else(|| "now".to_string());
        format!(
            "Dispatch loop: on // cadence={}s // policy=single-flight // next={}",
            MISSION_CONTROL_BUY_MODE_INTERVAL_SECONDS, next
        )
    };

    let recent_statuses = if snapshots.is_empty() {
        "Recent live request statuses: none yet".to_string()
    } else {
        let preview = snapshots
            .iter()
            .take(4)
            .map(|request| {
                format!(
                    "{}={}({}/{})",
                    compact_buy_mode_request_id(request.request_id.as_str()),
                    request.status.label(),
                    request.phase.as_str(),
                    request.authority.as_str()
                )
            })
            .collect::<Vec<_>>()
            .join(" // ");
        if snapshots.len() > 4 {
            format!(
                "Recent live request statuses: {} // +{} more",
                preview,
                snapshots.len().saturating_sub(4)
            )
        } else {
            format!("Recent live request statuses: {preview}")
        }
    };

    vec![loop_line, recent_statuses]
}

pub(crate) fn buy_mode_payments_clipboard_text(
    mission_control: &MissionControlPaneState,
    network_requests: &NetworkRequestsState,
    spark_wallet: &SparkPaneState,
) -> String {
    let mut sections = vec![
        "Buy Mode Payments".to_string(),
        buy_mode_payments_summary_text(network_requests, spark_wallet),
        buy_mode_payments_status_lines(mission_control, network_requests, spark_wallet, Instant::now())
            .join("\n"),
        "Rows are sourced from buy-mode requests plus wallet-backed Spark send history. Live requests stay linked by wallet pointer; older buy-mode sends are backfilled from Spark payment metadata.".to_string(),
        String::new(),
    ];
    for (_, text) in build_buy_mode_payment_rows(network_requests, spark_wallet) {
        if text.trim().is_empty() {
            sections.push(String::new());
        } else {
            sections.push(text);
        }
    }
    sections.join("\n")
}

pub(crate) fn build_buy_mode_payment_rows(
    network_requests: &NetworkRequestsState,
    spark_wallet: &SparkPaneState,
) -> Vec<(TerminalStream, String)> {
    let mut rows = Vec::<(TerminalStream, String)>::new();
    let entries = buy_mode_payment_ledger_entries(network_requests, spark_wallet);
    let request_entries = entries
        .iter()
        .filter(|entry| entry.source == "request")
        .collect::<Vec<_>>();
    let wallet_backfill_entries = entries
        .iter()
        .filter(|entry| entry.source == "wallet-backfill")
        .collect::<Vec<_>>();

    if !request_entries.is_empty() {
        rows.push((TerminalStream::Stdout, "LIVE BUY MODE REQUESTS".to_string()));
        rows.push((TerminalStream::Stdout, String::new()));
        for entry in request_entries {
            push_buy_mode_payment_entry_rows(&mut rows, entry);
        }
    }

    if !wallet_backfill_entries.is_empty() {
        if !rows.is_empty() {
            rows.push((TerminalStream::Stdout, String::new()));
        }
        rows.push((
            TerminalStream::Stdout,
            "WALLET-BACKFILL HISTORY".to_string(),
        ));
        rows.push((
            TerminalStream::Stdout,
            "These rows are inferred from Spark send history; they are not live request records."
                .to_string(),
        ));
        rows.push((TerminalStream::Stdout, String::new()));
        for entry in wallet_backfill_entries {
            push_buy_mode_payment_entry_rows(&mut rows, entry);
        }
    }

    if rows.is_empty() {
        rows.push((
            TerminalStream::Stdout,
            "No Buy Mode requests yet.".to_string(),
        ));
    }

    rows
}

pub(crate) fn buy_mode_request_flow_snapshots(
    network_requests: &NetworkRequestsState,
    spark_wallet: &SparkPaneState,
) -> Vec<BuyerRequestFlowSnapshot> {
    network_requests
        .submitted
        .iter()
        .rev()
        .filter(|request| request.request_type == MISSION_CONTROL_BUY_MODE_REQUEST_TYPE)
        .map(|request| build_buyer_request_flow_snapshot(request, spark_wallet))
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BuyModePaymentLedgerEntry {
    timestamp: Option<u64>,
    sort_epoch_seconds: u64,
    stream: TerminalStream,
    status: String,
    amount_sats: u64,
    fees_sats: Option<u64>,
    total_debit_sats: Option<u64>,
    wallet_status: String,
    wallet_method: String,
    provider_pubkey: String,
    request_id: String,
    request_type: String,
    authority: Nip90FlowAuthority,
    phase: Nip90FlowPhase,
    next_expected_event: String,
    selected_provider_pubkey: String,
    payable_provider_pubkey: String,
    loser_provider_count: usize,
    loser_reason_summary: Option<String>,
    payment_pointer: String,
    request_event_id: String,
    result_event_id: String,
    payment_hash: String,
    destination_pubkey: String,
    htlc_status: String,
    htlc_expiry_epoch_seconds: Option<u64>,
    wallet_detail: Option<String>,
    wallet_description: Option<String>,
    wallet_invoice: Option<String>,
    pending_bolt11: Option<String>,
    payment_error: Option<String>,
    payment_notice: Option<String>,
    source: &'static str,
}

fn buy_mode_payment_ledger_entries(
    network_requests: &NetworkRequestsState,
    spark_wallet: &SparkPaneState,
) -> Vec<BuyModePaymentLedgerEntry> {
    let mut matched_payment_pointers = std::collections::HashSet::<String>::new();
    let mut entries = Vec::<BuyModePaymentLedgerEntry>::new();

    for (index, request) in buy_mode_request_flow_snapshots(network_requests, spark_wallet)
        .into_iter()
        .enumerate()
    {
        if let Some(payment_pointer) = request.payment_pointer.as_deref() {
            matched_payment_pointers.insert(payment_pointer.to_string());
        }
        entries.push(buy_mode_request_ledger_entry(index, &request));
    }

    for payment in &spark_wallet.recent_payments {
        if matched_payment_pointers.contains(payment.id.as_str()) {
            continue;
        }
        if let Some(entry) = buy_mode_wallet_backfill_entry(payment) {
            entries.push(entry);
        }
    }

    entries.sort_by(|left, right| {
        right
            .sort_epoch_seconds
            .cmp(&left.sort_epoch_seconds)
            .then_with(|| right.payment_pointer.cmp(&left.payment_pointer))
    });
    entries
}

fn observation_has_non_error_result(
    observation: &crate::state::operations::NetworkRequestProviderObservation,
) -> bool {
    observation.last_result_event_id.is_some()
        && !matches!(
            observation
                .last_result_status
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_ascii_lowercase)
                .as_deref(),
            Some("error")
        )
}

fn observation_has_valid_invoice(
    observation: &crate::state::operations::NetworkRequestProviderObservation,
) -> bool {
    observation
        .last_feedback_bolt11
        .as_deref()
        .is_some_and(|bolt11| !bolt11.trim().is_empty())
}

fn observation_has_error_only_signal(
    observation: &crate::state::operations::NetworkRequestProviderObservation,
) -> bool {
    let feedback_error = matches!(
        observation
            .last_feedback_status
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("error")
    );
    let result_error = matches!(
        observation
            .last_result_status
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("error")
    );
    (feedback_error || result_error)
        && !observation_has_non_error_result(observation)
        && !observation_has_valid_invoice(observation)
}

fn buy_mode_request_ledger_entry(
    index: usize,
    request: &BuyerRequestFlowSnapshot,
) -> BuyModePaymentLedgerEntry {
    let stream =
        if request.status == NetworkRequestStatus::Failed || request.payment_error.is_some() {
            TerminalStream::Stderr
        } else {
            TerminalStream::Stdout
        };
    BuyModePaymentLedgerEntry {
        timestamp: request.timestamp,
        sort_epoch_seconds: request
            .timestamp
            .unwrap_or(u64::MAX.saturating_sub(index as u64)),
        stream,
        status: request.status.label().to_string(),
        amount_sats: request.budget_sats,
        fees_sats: request.fees_sats,
        total_debit_sats: request.total_debit_sats,
        wallet_status: request.wallet_status.clone(),
        wallet_method: request.wallet_method.clone(),
        provider_pubkey: request.provider_pubkey().unwrap_or("-").to_string(),
        request_id: request.request_id.clone(),
        request_type: request.request_type.clone(),
        authority: request.authority,
        phase: request.phase,
        next_expected_event: request.next_expected_event.clone(),
        selected_provider_pubkey: request
            .selected_provider_pubkey
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        payable_provider_pubkey: request
            .payable_provider_pubkey
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        loser_provider_count: request.loser_provider_count,
        loser_reason_summary: request.loser_reason_summary.clone(),
        payment_pointer: request
            .payment_pointer
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        request_event_id: request
            .published_request_event_id
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        result_event_id: request
            .winning_result_event_id
            .clone()
            .or_else(|| request.last_result_event_id.clone())
            .unwrap_or_else(|| "-".to_string()),
        payment_hash: request
            .payment_hash
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        destination_pubkey: request
            .destination_pubkey
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        htlc_status: request
            .htlc_status
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        htlc_expiry_epoch_seconds: request.htlc_expiry_epoch_seconds,
        wallet_detail: request.wallet_detail.clone(),
        wallet_description: request.wallet_description.clone(),
        wallet_invoice: request
            .wallet_invoice
            .as_deref()
            .map(compact_payment_invoice),
        pending_bolt11: request
            .pending_bolt11
            .as_deref()
            .map(compact_payment_invoice),
        payment_error: request.payment_error.clone(),
        payment_notice: request.payment_notice.clone(),
        source: "request",
    }
}

fn buy_mode_wallet_backfill_entry(
    payment: &openagents_spark::PaymentSummary,
) -> Option<BuyModePaymentLedgerEntry> {
    if !buy_mode_wallet_backfill_candidate(payment) {
        return None;
    }

    let wallet_status = buy_mode_wallet_status_for_wallet_backfill(payment).to_string();
    let stream = if matches!(wallet_status.as_str(), "failed" | "returned") {
        TerminalStream::Stderr
    } else {
        TerminalStream::Stdout
    };
    let request_hint = buy_mode_wallet_request_hint(payment)
        .map(|hint| format!("wallet-inferred:{hint}"))
        .unwrap_or_else(|| "wallet-inferred".to_string());

    Some(BuyModePaymentLedgerEntry {
        timestamp: Some(payment.timestamp),
        sort_epoch_seconds: payment.timestamp,
        stream,
        status: "wallet-backfill".to_string(),
        amount_sats: payment.amount_sats,
        fees_sats: Some(payment.fees_sats),
        total_debit_sats: Some(wallet_payment_total_debit_sats(payment)),
        wallet_status,
        wallet_method: payment.method.clone(),
        provider_pubkey: payment
            .destination_pubkey
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        request_id: request_hint,
        request_type: MISSION_CONTROL_BUY_MODE_REQUEST_TYPE.to_string(),
        authority: Nip90FlowAuthority::Wallet,
        phase: if payment.is_returned_htlc_failure() {
            Nip90FlowPhase::Failed
        } else if is_settled_wallet_payment_status(payment.status.as_str()) {
            Nip90FlowPhase::Paid
        } else if is_terminal_wallet_payment_status(payment.status.as_str()) {
            Nip90FlowPhase::Failed
        } else {
            Nip90FlowPhase::AwaitingPayment
        },
        next_expected_event: if is_settled_wallet_payment_status(payment.status.as_str())
            || is_terminal_wallet_payment_status(payment.status.as_str())
        {
            "none".to_string()
        } else {
            "wallet settlement".to_string()
        },
        selected_provider_pubkey: "-".to_string(),
        payable_provider_pubkey: "-".to_string(),
        loser_provider_count: 0,
        loser_reason_summary: None,
        payment_pointer: payment.id.clone(),
        request_event_id: "-".to_string(),
        result_event_id: "-".to_string(),
        payment_hash: payment
            .payment_hash
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        destination_pubkey: payment
            .destination_pubkey
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        htlc_status: payment
            .htlc_status
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        htlc_expiry_epoch_seconds: payment.htlc_expiry_epoch_seconds,
        wallet_detail: payment.status_detail.clone(),
        wallet_description: payment.description.clone(),
        wallet_invoice: payment.invoice.as_deref().map(compact_payment_invoice),
        pending_bolt11: None,
        payment_error: None,
        payment_notice: None,
        source: "wallet-backfill",
    })
}

fn push_buy_mode_payment_entry_rows(
    rows: &mut Vec<(TerminalStream, String)>,
    entry: &BuyModePaymentLedgerEntry,
) {
    rows.push((
        entry.stream.clone(),
        format!(
            "{}  status={}  amount={} sats  fee={}  total_debit={}  wallet_status={}  wallet_method={}  provider_pubkey={}",
            buy_mode_payment_timestamp_label(entry.timestamp),
            entry.status,
            entry.amount_sats,
            buy_mode_optional_sats_label(entry.fees_sats),
            buy_mode_optional_sats_label(entry.total_debit_sats),
            entry.wallet_status,
            entry.wallet_method,
            entry.provider_pubkey,
        ),
    ));
    rows.push((
        entry.stream.clone(),
        format!(
            "request_id={}  request_type={}  authority={}  phase={}  next={}  payment_pointer={}  request_event_id={}  result_event_id={}  payment_hash={}  source={}",
            entry.request_id,
            entry.request_type,
            entry.authority.as_str(),
            entry.phase.as_str(),
            entry.next_expected_event,
            entry.payment_pointer,
            entry.request_event_id,
            entry.result_event_id,
            entry.payment_hash,
            entry.source,
        ),
    ));
    if entry.source == "request" {
        rows.push((
            entry.stream.clone(),
            format!(
                "selected_provider={}  payable_provider={}  losers={}  loser_summary={}",
                entry.selected_provider_pubkey,
                entry.payable_provider_pubkey,
                entry.loser_provider_count,
                entry.loser_reason_summary.as_deref().unwrap_or("-"),
            ),
        ));
    }
    if entry.destination_pubkey != "-"
        || entry.htlc_status != "-"
        || entry.htlc_expiry_epoch_seconds.is_some()
    {
        rows.push((
            entry.stream.clone(),
            format!(
                "destination_pubkey={}  htlc_status={}  htlc_expiry={}",
                entry.destination_pubkey,
                entry.htlc_status,
                buy_mode_payment_timestamp_label(entry.htlc_expiry_epoch_seconds),
            ),
        ));
    }
    if let Some(detail) = entry.wallet_detail.as_deref() {
        rows.push((entry.stream.clone(), format!("wallet_detail={detail}")));
    }
    if let Some(description) = entry.wallet_description.as_deref() {
        rows.push((
            TerminalStream::Stdout,
            format!("wallet_description={description}"),
        ));
    }
    if let Some(invoice) = entry.wallet_invoice.as_deref() {
        rows.push((TerminalStream::Stdout, format!("wallet_invoice={invoice}")));
    }
    if let Some(invoice) = entry.pending_bolt11.as_deref() {
        rows.push((TerminalStream::Stdout, format!("pending_bolt11={invoice}")));
    }
    if let Some(error) = entry.payment_error.as_deref() {
        rows.push((TerminalStream::Stderr, format!("payment_error={error}")));
    }
    if let Some(notice) = entry.payment_notice.as_deref() {
        rows.push((TerminalStream::Stderr, format!("payment_notice={notice}")));
    }
    rows.push((TerminalStream::Stdout, String::new()));
}

fn loser_provider_summary(
    request: &SubmittedNetworkRequest,
    payable_provider_pubkey: Option<&str>,
) -> (usize, Option<String>) {
    let winner = payable_provider_pubkey.map(normalize_pubkey);
    let mut loser_pubkeys = std::collections::BTreeSet::<String>::new();
    let mut no_invoice = 0usize;
    let mut error_only = 0usize;
    let mut late_result = 0usize;
    let mut non_winning_noise = 0usize;
    let mut other = 0usize;

    for observation in &request.provider_observations {
        let provider_pubkey = normalize_pubkey(observation.provider_pubkey.as_str());
        if winner.as_deref() == Some(provider_pubkey.as_str()) {
            continue;
        }
        loser_pubkeys.insert(provider_pubkey);
        if observation_has_non_error_result(observation)
            && !observation_has_valid_invoice(observation)
        {
            no_invoice = no_invoice.saturating_add(1);
        } else if observation_has_error_only_signal(observation) {
            error_only = error_only.saturating_add(1);
        }
    }

    for outcome in &request.duplicate_outcomes {
        let provider_pubkey = normalize_pubkey(outcome.provider_pubkey.as_str());
        if winner.as_deref() == Some(provider_pubkey.as_str()) {
            continue;
        }
        loser_pubkeys.insert(provider_pubkey);
        if outcome.reason_code == BuyerResolutionReason::LateResultUnpaid.code() {
            late_result = late_result.saturating_add(1);
        } else if outcome.reason_code == BuyerResolutionReason::LostRace.code() {
            non_winning_noise = non_winning_noise.saturating_add(1);
        } else {
            other = other.saturating_add(1);
        }
    }

    let losers = loser_pubkeys.len();
    if losers == 0 {
        return (0, None);
    }

    let mut reasons = Vec::new();
    if no_invoice > 0 {
        reasons.push("no invoice");
    }
    if error_only > 0 {
        reasons.push("error-only");
    }
    if late_result > 0 {
        reasons.push("late result");
    }
    if non_winning_noise > 0 {
        reasons.push("non-winning provider noise ignored");
    }
    if other > 0 {
        reasons.push("other");
    }
    (
        losers,
        Some(format!("{losers} losers ignored: {}", reasons.join(", "))),
    )
}

fn buy_mode_wallet_backfill_candidate(payment: &openagents_spark::PaymentSummary) -> bool {
    payment.direction.eq_ignore_ascii_case("send")
        && payment.description.as_deref().is_some_and(|description| {
            description
                .trim()
                .to_ascii_lowercase()
                .starts_with("dvm textgen")
        })
}

fn buy_mode_wallet_request_hint(payment: &openagents_spark::PaymentSummary) -> Option<String> {
    let description = payment.description.as_deref()?.trim();
    let candidate = description.split_whitespace().last()?;
    if candidate.eq_ignore_ascii_case("textgen") {
        return None;
    }
    let trimmed = candidate.trim_matches(|ch: char| !ch.is_ascii_hexdigit());
    if trimmed.is_empty() || !trimmed.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }
    Some(trimmed.to_string())
}

fn buy_mode_wallet_status_for_wallet_backfill(
    payment: &openagents_spark::PaymentSummary,
) -> &'static str {
    if payment.is_returned_htlc_failure() {
        "returned"
    } else if is_settled_wallet_payment_status(payment.status.as_str()) {
        "sent"
    } else if is_terminal_wallet_payment_status(payment.status.as_str()) {
        "failed"
    } else {
        "pending"
    }
}

fn authoritative_payment_pointer(pointer: Option<&str>) -> bool {
    let Some(pointer) = pointer else {
        return false;
    };
    let pointer = pointer.trim();
    !pointer.is_empty()
        && !pointer.starts_with("pending:")
        && !pointer.starts_with("pay:")
        && !pointer.starts_with("inv-")
        && !pointer.starts_with("pay-req-")
}

fn active_job_result_publish_status(active_job: &ActiveJobState) -> String {
    let Some(job) = active_job.job.as_ref() else {
        return "n/a".to_string();
    };
    if job.sa_tick_result_event_id.is_some() {
        return "confirmed on relays".to_string();
    }
    let age_suffix = active_job
        .result_publish_last_queued_epoch_seconds
        .map(|queued_at| {
            let now_epoch_seconds = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| duration.as_secs());
            format!(
                " queued {}s ago",
                now_epoch_seconds.saturating_sub(queued_at)
            )
        })
        .unwrap_or_default();
    if active_job.result_publish_in_flight {
        return format!(
            "awaiting relay confirmation attempt #{}{}",
            active_job.result_publish_attempt_count.max(1),
            age_suffix
        );
    }
    if active_job.pending_result_publish_event_id.is_some() {
        return format!(
            "retry pending attempt #{}{}",
            active_job.result_publish_attempt_count.max(1),
            age_suffix
        );
    }
    "not queued".to_string()
}

fn compact_buy_mode_request_id(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() <= 18 {
        trimmed.to_string()
    } else {
        format!("{}..{}", &trimmed[..12], &trimmed[trimmed.len() - 6..])
    }
}

fn buy_mode_payment_timestamp_label(epoch_seconds: Option<u64>) -> String {
    epoch_seconds
        .and_then(|value| Local.timestamp_opt(value as i64, 0).single())
        .map(|value| value.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "timestamp=-".to_string())
}

fn buy_mode_optional_sats_label(value: Option<u64>) -> String {
    value
        .map(|amount| format!("{amount} sats"))
        .unwrap_or_else(|| "-".to_string())
}

fn format_mission_control_amount(amount_sats: u64) -> String {
    if amount_sats >= 1_000 {
        format!("\u{20BF} {}", thousands(amount_sats))
    } else {
        format!("\u{20BF} {amount_sats}")
    }
}

fn thousands(value: u64) -> String {
    let digits = value.to_string();
    let mut formatted = String::new();
    for (index, ch) in digits.chars().rev().enumerate() {
        if index != 0 && index % 3 == 0 {
            formatted.push(' ');
        }
        formatted.push(ch);
    }
    formatted.chars().rev().collect()
}

fn short_id(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() <= 12 {
        trimmed.to_string()
    } else {
        format!("{}..{}", &trimmed[..6], &trimmed[trimmed.len() - 4..])
    }
}

fn mission_control_log_short_id(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() <= 12 {
        trimmed.to_string()
    } else {
        format!("{}..", &trimmed[..12])
    }
}

fn normalize_pubkey(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::{
        Nip90FlowAuthority, Nip90FlowPhase, build_active_job_flow_snapshot,
        build_buyer_request_flow_snapshot, buy_mode_payments_status_lines,
        continuity_window_seconds,
    };
    use crate::app_state::{
        ActiveJobState, EarnJobLifecycleProjectionState, MissionControlPaneState,
    };
    use crate::spark_wallet::SparkPaneState;
    use crate::state::operations::{
        BuyerResolutionMode, NetworkRequestSubmission, NetworkRequestsState,
    };

    #[test]
    fn buyer_snapshot_waiting_for_wallet_settlement_is_wallet_authoritative() {
        let mut requests = NetworkRequestsState::default();
        let request_id = requests
            .queue_request_submission(NetworkRequestSubmission {
                request_id: Some("req-snapshot-wallet".to_string()),
                request_type: crate::app_state::MISSION_CONTROL_BUY_MODE_REQUEST_TYPE.to_string(),
                payload: "BUY MODE OK".to_string(),
                resolution_mode: BuyerResolutionMode::Race,
                target_provider_pubkeys: Vec::new(),
                skill_scope_id: None,
                credit_envelope_ref: None,
                budget_sats: 2,
                timeout_seconds: 75,
                authority_command_seq: 1,
            })
            .expect("queue request");
        requests.apply_nip90_request_publish_outcome(
            request_id.as_str(),
            "event-wallet-snapshot",
            1,
            0,
            None,
        );
        requests.apply_nip90_buyer_feedback_event(
            request_id.as_str(),
            "aa".repeat(32).as_str(),
            "feedback-pay-001",
            Some("payment-required"),
            Some("lightning settlement required"),
            Some(2_000),
            Some("lnbc1snapshotwallet"),
        );
        requests
            .prepare_auto_payment_attempt(
                request_id.as_str(),
                "lnbc1snapshotwallet",
                Some(2_000),
                1_700_000_000,
            )
            .expect("payment-required invoice should queue");
        requests.record_auto_payment_pointer(request_id.as_str(), "wallet-snapshot-001");

        let request = requests
            .latest_request_by_type(crate::app_state::MISSION_CONTROL_BUY_MODE_REQUEST_TYPE)
            .expect("latest request");
        let snapshot = build_buyer_request_flow_snapshot(request, &SparkPaneState::default());

        assert_eq!(snapshot.authority, Nip90FlowAuthority::Wallet);
        assert_eq!(snapshot.phase, Nip90FlowPhase::AwaitingPayment);
        assert_eq!(snapshot.next_expected_event, "wallet settlement");
    }

    #[test]
    fn active_job_snapshot_publishing_result_is_relay_authoritative() {
        let mut active_job = ActiveJobState::default();
        let request = crate::state::job_inbox::JobInboxRequest {
            request_id: "req-active-snapshot".to_string(),
            requester: "npub1requester".to_string(),
            demand_source: crate::app_state::JobDemandSource::OpenNetwork,
            request_kind: 5050,
            capability: "text.generation".to_string(),
            execution_input: None,
            execution_prompt: Some("BUY MODE OK".to_string()),
            execution_params: Vec::new(),
            requested_model: None,
            requested_output_mime: None,
            skill_scope_id: None,
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_request_event_id: None,
            sa_tick_result_event_id: None,
            ac_envelope_event_id: None,
            price_sats: 2,
            ttl_seconds: 75,
            validation: crate::state::job_inbox::JobInboxValidation::Valid,
            arrival_seq: 1,
            decision: crate::state::job_inbox::JobInboxDecision::Pending,
        };
        active_job.start_from_request(&request);
        active_job.job.as_mut().expect("job").stage = crate::app_state::JobLifecycleStage::Running;
        active_job.execution_turn_completed = true;
        active_job.result_publish_in_flight = true;
        active_job.pending_result_publish_event_id = Some("result-publish-001".to_string());
        active_job.result_publish_attempt_count = 2;
        active_job.result_publish_last_queued_epoch_seconds = Some(1_700_000_000);

        let snapshot = build_active_job_flow_snapshot(
            &active_job,
            &EarnJobLifecycleProjectionState::default(),
        )
        .expect("active snapshot");

        assert_eq!(snapshot.authority, Nip90FlowAuthority::Relay);
        assert_eq!(snapshot.phase, Nip90FlowPhase::PublishingResult);
        assert_eq!(snapshot.next_expected_event, "relay confirmation");
        assert_eq!(snapshot.continuity_window_seconds, Some(195));
    }

    #[test]
    fn buy_mode_status_lines_use_snapshot_phase_and_authority() {
        let mut mission_control = MissionControlPaneState::default();
        mission_control.buy_mode_loop_enabled = true;

        let mut requests = NetworkRequestsState::default();
        let request_id = requests
            .queue_request_submission(NetworkRequestSubmission {
                request_id: Some("req-status-lines".to_string()),
                request_type: crate::app_state::MISSION_CONTROL_BUY_MODE_REQUEST_TYPE.to_string(),
                payload: "BUY MODE OK".to_string(),
                resolution_mode: BuyerResolutionMode::Race,
                target_provider_pubkeys: Vec::new(),
                skill_scope_id: None,
                credit_envelope_ref: None,
                budget_sats: 2,
                timeout_seconds: 75,
                authority_command_seq: 9,
            })
            .expect("queue request");
        requests.apply_nip90_request_publish_outcome(
            request_id.as_str(),
            "event-status-lines",
            1,
            0,
            None,
        );

        let lines = buy_mode_payments_status_lines(
            &mission_control,
            &requests,
            &SparkPaneState::default(),
            Instant::now(),
        );
        assert!(lines[0].contains("phase=submitted"));
        assert!(lines[0].contains("auth=relay"));
    }

    #[test]
    fn buyer_snapshot_surfaces_selected_payable_and_loser_reasons() {
        let mut requests = NetworkRequestsState::default();
        let request_id = requests
            .queue_request_submission(NetworkRequestSubmission {
                request_id: Some("req-snapshot-winner-001".to_string()),
                request_type: crate::app_state::MISSION_CONTROL_BUY_MODE_REQUEST_TYPE.to_string(),
                payload: "BUY MODE OK".to_string(),
                resolution_mode: BuyerResolutionMode::Race,
                target_provider_pubkeys: Vec::new(),
                skill_scope_id: None,
                credit_envelope_ref: None,
                budget_sats: 2,
                timeout_seconds: 75,
                authority_command_seq: 10,
            })
            .expect("queue request");
        let payable_provider = "31".repeat(32);
        let losing_provider = "41".repeat(32);

        requests.apply_nip90_request_publish_outcome(
            request_id.as_str(),
            "event-snapshot-winner-001",
            3,
            1,
            None,
        );
        requests.apply_nip90_buyer_feedback_event(
            request_id.as_str(),
            payable_provider.as_str(),
            "feedback-snapshot-winner-001",
            Some("payment-required"),
            Some("invoice ready"),
            Some(2_000),
            Some("lnbc1snapshotwinner001"),
        );
        requests.apply_nip90_buyer_result_event(
            request_id.as_str(),
            payable_provider.as_str(),
            "result-snapshot-winner-001",
            Some("success"),
        );
        requests.apply_nip90_buyer_result_event(
            request_id.as_str(),
            losing_provider.as_str(),
            "result-snapshot-loser-001",
            Some("success"),
        );
        requests.apply_nip90_buyer_feedback_event(
            request_id.as_str(),
            losing_provider.as_str(),
            "feedback-snapshot-loser-001",
            Some("processing"),
            Some("still working"),
            None,
            None,
        );

        let request = requests
            .latest_request_by_type(crate::app_state::MISSION_CONTROL_BUY_MODE_REQUEST_TYPE)
            .expect("latest request");
        let snapshot = build_buyer_request_flow_snapshot(request, &SparkPaneState::default());

        assert_eq!(
            snapshot.selected_provider_pubkey.as_deref(),
            Some(losing_provider.as_str())
        );
        assert_eq!(
            snapshot.payable_provider_pubkey.as_deref(),
            Some(payable_provider.as_str())
        );
        assert_eq!(snapshot.loser_provider_count, 1);
        assert!(
            snapshot
                .loser_reason_summary
                .as_deref()
                .is_some_and(|summary| summary.contains("no invoice"))
        );
        assert!(
            snapshot
                .loser_reason_summary
                .as_deref()
                .is_some_and(|summary| summary.contains("late result"))
        );
        assert!(
            snapshot
                .loser_reason_summary
                .as_deref()
                .is_some_and(|summary| summary.contains("non-winning provider noise ignored"))
        );
    }

    #[test]
    fn continuity_window_matches_runtime_grace_policy() {
        assert_eq!(continuity_window_seconds(60), 180);
        assert_eq!(continuity_window_seconds(75), 195);
        assert_eq!(continuity_window_seconds(300), 420);
    }
}
