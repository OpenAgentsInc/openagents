use std::time::{Instant, SystemTime, UNIX_EPOCH};

use chrono::{Local, TimeZone};
use wgpui::components::sections::TerminalStream;

use crate::app_state::{
    ActiveJobRecord, ActiveJobState, BuyModePaneState, EarnJobLifecycleProjectionState,
    JobLifecycleStage, MISSION_CONTROL_BUY_MODE_REQUEST_TYPE,
    mission_control_buy_mode_interval_label,
};
use crate::nip90_compute_semantics::analyze_invoice_amount_msats;
use crate::spark_wallet::{
    SparkPaneState, is_settled_wallet_payment_status, is_terminal_wallet_payment_status,
    wallet_payment_net_delta_sats, wallet_payment_total_debit_sats,
};
use crate::state::operations::{
    BuyerResolutionReason, NetworkRequestStatus, NetworkRequestsState, SubmittedNetworkRequest,
    buyer_request_seller_settled_pending_local_wallet, buyer_request_seller_settlement_feedback,
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
    SellerSettledPendingWallet,
    DeliveredUnpaid,
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
            Self::SellerSettledPendingWallet => "seller-settled-pending-wallet",
            Self::DeliveredUnpaid => "delivered-unpaid",
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
    pub result_provider_pubkey: Option<String>,
    pub invoice_provider_pubkey: Option<String>,
    pub payable_provider_pubkey: Option<String>,
    pub payment_blocker_codes: Vec<String>,
    pub payment_blocker_summary: Option<String>,
    pub last_feedback_status: Option<String>,
    pub last_feedback_event_id: Option<String>,
    pub last_result_event_id: Option<String>,
    pub winning_result_event_id: Option<String>,
    pub payment_pointer: Option<String>,
    pub seller_success_feedback_event_id: Option<String>,
    pub payment_required_at_epoch_seconds: Option<u64>,
    pub payment_sent_at_epoch_seconds: Option<u64>,
    pub payment_failed_at_epoch_seconds: Option<u64>,
    pub pending_bolt11: Option<String>,
    pub payment_error: Option<String>,
    pub payment_notice: Option<String>,
    pub timestamp: Option<u64>,
    pub wallet_status: String,
    pub wallet_method: String,
    pub invoice_amount_sats: Option<u64>,
    pub fees_sats: Option<u64>,
    pub total_debit_sats: Option<u64>,
    pub net_wallet_delta_sats: Option<i64>,
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

    pub(crate) fn result_provider_pubkey(&self) -> Option<&str> {
        self.result_provider_pubkey.as_deref()
    }

    pub(crate) fn invoice_provider_pubkey(&self) -> Option<&str> {
        self.invoice_provider_pubkey.as_deref()
    }

    pub(crate) fn provider_pubkey(&self) -> Option<&str> {
        self.payable_provider_pubkey
            .as_deref()
            .or(self.result_provider_pubkey.as_deref())
            .or(self.invoice_provider_pubkey.as_deref())
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
        match (
            self.payable_provider_pubkey.as_deref(),
            self.result_provider_pubkey(),
            self.invoice_provider_pubkey(),
        ) {
            (Some(payable), _, _) => format!("payable {}", short_id(payable)),
            (None, Some(result), Some(invoice))
                if normalize_pubkey(result) != normalize_pubkey(invoice) =>
            {
                format!(
                    "result {} // invoice {}",
                    short_id(result),
                    short_id(invoice)
                )
            }
            (None, Some(result), _) => format!("result {}", short_id(result)),
            (None, None, Some(invoice)) => format!("invoice {}", short_id(invoice)),
            (None, None, None) => "awaiting provider".to_string(),
        }
    }

    pub(crate) fn winner_selection_summary(&self) -> String {
        match (
            self.result_provider_pubkey(),
            self.invoice_provider_pubkey(),
            self.payable_provider_pubkey.as_deref(),
        ) {
            (_, _, Some(payable)) => format!("payable {}", short_id(payable)),
            (Some(result), Some(invoice), None)
                if normalize_pubkey(result) != normalize_pubkey(invoice) =>
            {
                format!(
                    "result {} // invoice {} // no payable winner",
                    short_id(result),
                    short_id(invoice)
                )
            }
            (Some(result), Some(_), None) => {
                format!("result {} // awaiting payable selection", short_id(result))
            }
            (Some(result), None, None) => {
                format!("result {} // awaiting invoice", short_id(result))
            }
            (None, Some(invoice), None) => {
                format!("invoice {} // awaiting result", short_id(invoice))
            }
            (None, None, None) => "awaiting provider".to_string(),
        }
    }

    pub(crate) fn payment_blocker_codes_label(&self) -> Option<String> {
        (!self.payment_blocker_codes.is_empty()).then(|| self.payment_blocker_codes.join(","))
    }

    pub(crate) fn work_label(&self) -> String {
        if self.status == NetworkRequestStatus::Failed {
            return "fault".to_string();
        }
        if self.last_result_event_id.is_some()
            && self.phase != Nip90FlowPhase::SellerSettledPendingWallet
        {
            return "done".to_string();
        }
        match self.phase {
            Nip90FlowPhase::Paid => "done".to_string(),
            Nip90FlowPhase::AwaitingPayment => "invoice".to_string(),
            Nip90FlowPhase::SellerSettledPendingWallet => "settled".to_string(),
            Nip90FlowPhase::RequestingPayment => "invoice".to_string(),
            Nip90FlowPhase::DeliveredUnpaid => "unpaid".to_string(),
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
        if self.last_result_event_id.is_some()
            && self.phase != Nip90FlowPhase::SellerSettledPendingWallet
        {
            return "result received".to_string();
        }
        match self.phase {
            Nip90FlowPhase::Paid => "payment settled".to_string(),
            Nip90FlowPhase::AwaitingPayment => "invoice received".to_string(),
            Nip90FlowPhase::SellerSettledPendingWallet => "seller settlement confirmed".to_string(),
            Nip90FlowPhase::RequestingPayment => {
                if self.pending_bolt11.is_some() {
                    "invoice received".to_string()
                } else {
                    "awaiting valid invoice".to_string()
                }
            }
            Nip90FlowPhase::DeliveredUnpaid => "result delivered but unpaid".to_string(),
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
        if self.last_result_event_id.is_some()
            && self.phase != Nip90FlowPhase::SellerSettledPendingWallet
        {
            return "result-received".to_string();
        }
        match self.phase {
            Nip90FlowPhase::Paid | Nip90FlowPhase::Delivered => "result-received".to_string(),
            Nip90FlowPhase::AwaitingPayment | Nip90FlowPhase::RequestingPayment => {
                "invoice-requested".to_string()
            }
            Nip90FlowPhase::SellerSettledPendingWallet => {
                "seller-settled-local-wallet-pending".to_string()
            }
            Nip90FlowPhase::DeliveredUnpaid => "delivered-unpaid".to_string(),
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
        let invoice_summary = self
            .invoice_amount_sats
            .map(|amount| format!("{amount} sats invoice"));
        let net_delta_summary = self
            .net_wallet_delta_sats
            .map(crate::spark_wallet::format_wallet_delta_sats)
            .map(|delta| format!("wallet delta {delta}"));
        if self.phase == Nip90FlowPhase::SellerSettledPendingWallet {
            let mut parts = Vec::new();
            if let Some(invoice_summary) = invoice_summary.clone() {
                parts.push(invoice_summary);
            }
            if let Some(pointer) = self.payment_pointer.as_deref() {
                parts.push(format!("pointer {}", short_id(pointer)));
            }
            if let Some(fees) = self.fees_sats {
                parts.push(format!("{fees} sats fee"));
            }
            if let Some(total) = self.total_debit_sats {
                parts.push(format!("{total} sats total debit"));
            }
            if let Some(net_delta_summary) = net_delta_summary.clone() {
                parts.push(net_delta_summary);
            }
            let detail = "seller settled; awaiting local wallet confirmation";
            return if parts.is_empty() {
                detail.to_string()
            } else {
                format!("{detail} ({})", parts.join("; "))
            };
        }
        match self.wallet_status.as_str() {
            "sent" => {
                let mut parts = Vec::new();
                if let Some(invoice_summary) = invoice_summary.clone() {
                    parts.push(invoice_summary);
                }
                if let Some(fees) = self.fees_sats {
                    parts.push(format!("{fees} sats fee"));
                }
                if let Some(total) = self.total_debit_sats {
                    parts.push(format!("{total} sats total debit"));
                }
                if let Some(net_delta_summary) = net_delta_summary.clone() {
                    parts.push(net_delta_summary);
                }
                if parts.is_empty() {
                    "payment sent".to_string()
                } else {
                    format!("payment sent ({})", parts.join("; "))
                }
            }
            "returned" => {
                let mut parts = Vec::new();
                if let Some(invoice_summary) = invoice_summary.clone() {
                    parts.push(invoice_summary);
                }
                if let Some(fees) = self.fees_sats {
                    parts.push(format!("{fees} sats fee"));
                }
                if let Some(total) = self.total_debit_sats {
                    parts.push(format!("{total} sats total debit"));
                }
                if let Some(net_delta_summary) = net_delta_summary.clone() {
                    parts.push(net_delta_summary);
                }
                if parts.is_empty() {
                    "payment returned".to_string()
                } else {
                    format!("payment returned ({})", parts.join("; "))
                }
            }
            "failed" => {
                let detail = self
                    .wallet_detail
                    .clone()
                    .or_else(|| self.payment_error.clone())
                    .unwrap_or_else(|| "payment failed".to_string());
                let mut parts = Vec::new();
                if let Some(invoice_summary) = invoice_summary.clone() {
                    parts.push(invoice_summary);
                }
                if let Some(fees) = self.fees_sats {
                    parts.push(format!("{fees} sats fee"));
                }
                if let Some(total) = self.total_debit_sats {
                    parts.push(format!("{total} sats total debit"));
                }
                if let Some(net_delta_summary) = net_delta_summary.clone() {
                    parts.push(net_delta_summary);
                }
                if parts.is_empty() {
                    detail
                } else {
                    format!("{detail} ({})", parts.join("; "))
                }
            }
            "pending" => {
                let detail = self
                    .wallet_detail
                    .clone()
                    .unwrap_or_else(|| "payment pending Spark confirmation".to_string());
                let mut parts = Vec::new();
                if let Some(invoice_summary) = invoice_summary.clone() {
                    parts.push(invoice_summary);
                }
                if let Some(fees) = self.fees_sats {
                    parts.push(format!("{fees} sats fee"));
                }
                if let Some(total) = self.total_debit_sats {
                    parts.push(format!("{total} sats total debit"));
                }
                if let Some(net_delta_summary) = net_delta_summary {
                    parts.push(net_delta_summary);
                }
                if parts.is_empty() {
                    detail
                } else {
                    format!("{detail} ({})", parts.join("; "))
                }
            }
            "queued" => invoice_summary
                .map(|invoice| format!("payment queued ({invoice})"))
                .unwrap_or_else(|| "payment queued".to_string()),
            "invoice" => {
                if self.pending_bolt11.is_some() {
                    invoice_summary
                        .map(|invoice| format!("invoice received ({invoice})"))
                        .unwrap_or_else(|| "invoice received".to_string())
                } else if let Some(blocker) = self.payment_blocker_summary.as_deref() {
                    format!("blocked ({blocker})")
                } else {
                    "invoice received".to_string()
                }
            }
            _ => self
                .payment_blocker_summary
                .as_deref()
                .map(|blocker| format!("blocked ({blocker})"))
                .unwrap_or_else(|| "payment idle".to_string()),
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
        if let Some(feedback_event_id) = self.seller_success_feedback_event_id.as_deref() {
            line.push_str(" seller_settlement_feedback=");
            line.push_str(mission_control_log_short_id(feedback_event_id).as_str());
        }
        if let Some(result_provider) = self.result_provider_pubkey() {
            line.push_str(" result_provider=");
            line.push_str(mission_control_log_short_id(result_provider).as_str());
        }
        if let Some(invoice_provider) = self.invoice_provider_pubkey() {
            line.push_str(" invoice_provider=");
            line.push_str(mission_control_log_short_id(invoice_provider).as_str());
        }
        if let Some(payable) = self.payable_provider_pubkey.as_deref() {
            line.push_str(" payable_provider=");
            line.push_str(mission_control_log_short_id(payable).as_str());
        }
        if let Some(blocker_codes) = self.payment_blocker_codes_label() {
            line.push_str(" blocker_codes=");
            line.push_str(blocker_codes.as_str());
        }
        if let Some(blocker_summary) = self.payment_blocker_summary.as_deref() {
            line.push_str(" blocker=");
            line.push_str(blocker_summary);
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
        if let Some(invoice) = self.invoice_amount_sats {
            line.push_str(" invoice_sats=");
            line.push_str(&invoice.to_string());
        }
        if let Some(total) = self.total_debit_sats {
            line.push_str(" wallet_debit_sats=");
            line.push_str(&total.to_string());
        }
        if let Some(delta) = self.net_wallet_delta_sats {
            line.push_str(" wallet_delta_sats=");
            line.push_str(&delta.to_string());
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
    pub settlement_status: Option<String>,
    pub settlement_method: Option<String>,
    pub settlement_amount_sats: Option<u64>,
    pub settlement_fees_sats: Option<u64>,
    pub settlement_net_wallet_delta_sats: Option<i64>,
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
                "awaiting buyer payment // pointer {} // window {}",
                self.payment_pointer
                    .as_deref()
                    .map(|pointer| short_id(pointer).to_string())
                    .unwrap_or_else(|| "pending".to_string()),
                self.continuity_window_seconds
                    .map(|window| format!("{window}s"))
                    .unwrap_or_else(|| "-".to_string()),
            )),
            Nip90FlowPhase::DeliveredUnpaid => Some(format!(
                "{} // result {} // window {}",
                if self.pending_bolt11.is_some() {
                    "buyer never settled"
                } else {
                    "buyer never paid after delivery"
                },
                self.result_event_id
                    .as_deref()
                    .map(short_id)
                    .unwrap_or_else(|| "n/a".to_string()),
                self.continuity_window_seconds
                    .map(|window| format!("{window}s"))
                    .unwrap_or_else(|| "-".to_string()),
            )),
            Nip90FlowPhase::RequestingPayment => Some(format!(
                "preparing buyer invoice // window {}",
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
        active_job: build_active_job_flow_snapshot(
            active_job,
            earn_job_lifecycle_projection,
            spark_wallet,
        ),
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
    if request.payment_required_at_epoch_seconds.is_some()
        || request
            .last_feedback_status
            .as_deref()
            .is_some_and(|status| status.eq_ignore_ascii_case("payment-required"))
    {
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
    let seller_settlement_feedback = buyer_request_seller_settlement_feedback(request).map(
        |(provider_pubkey, feedback_event_id)| {
            (provider_pubkey.to_string(), feedback_event_id.to_string())
        },
    );
    let seller_settled_pending_wallet = buyer_request_seller_settled_pending_local_wallet(request);
    let result_provider_pubkey = request.result_provider_pubkey.clone();
    let invoice_provider_pubkey = request.invoice_provider_pubkey.clone();
    let payable_provider_pubkey = request.winning_provider_pubkey.clone();
    let selected_provider_pubkey = payable_provider_pubkey
        .clone()
        .or_else(|| result_provider_pubkey.clone())
        .or_else(|| invoice_provider_pubkey.clone());
    let (loser_provider_count, loser_reason_summary) =
        loser_provider_summary(request, payable_provider_pubkey.as_deref());
    let invoice_amount_sats = wallet_payment
        .map(|payment| payment.amount_sats)
        .or_else(|| {
            request_provider_invoice_amount_sats(
                request,
                payable_provider_pubkey
                    .as_deref()
                    .or(invoice_provider_pubkey.as_deref())
                    .or(result_provider_pubkey.as_deref())
                    .or(selected_provider_pubkey.as_deref()),
            )
        });
    let (payment_blocker_codes, payment_blocker_summary) = if request.last_payment_pointer.is_some()
        || request.pending_bolt11.is_some()
        || request.winning_provider_pubkey.is_some()
        || request.payment_error.is_some()
    {
        (Vec::new(), None)
    } else {
        derive_buyer_payment_blockers(request)
    };

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
        } else if seller_settled_pending_wallet {
            (
                Nip90FlowAuthority::Provider,
                Nip90FlowPhase::SellerSettledPendingWallet,
                "buyer local wallet confirmation".to_string(),
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
        result_provider_pubkey,
        invoice_provider_pubkey,
        payable_provider_pubkey,
        payment_blocker_codes,
        payment_blocker_summary,
        last_feedback_status: request.last_feedback_status.clone(),
        last_feedback_event_id: request.last_feedback_event_id.clone(),
        last_result_event_id: request.last_result_event_id.clone(),
        winning_result_event_id: request.winning_result_event_id.clone(),
        payment_pointer: request.last_payment_pointer.clone(),
        seller_success_feedback_event_id: seller_settlement_feedback
            .as_ref()
            .map(|(_, feedback_event_id)| feedback_event_id.clone()),
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
        invoice_amount_sats,
        fees_sats: wallet_payment.map(|payment| payment.fees_sats),
        total_debit_sats: wallet_payment.map(wallet_payment_total_debit_sats),
        net_wallet_delta_sats: wallet_payment.map(wallet_payment_net_delta_sats),
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
    spark_wallet: &SparkPaneState,
) -> Option<ActiveJobFlowSnapshot> {
    let job = active_job.job.as_ref()?;
    let now_epoch_seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    let settlement_payment = job.payment_id.as_deref().and_then(|payment_id| {
        spark_wallet
            .recent_payments
            .iter()
            .find(|payment| payment.id == payment_id)
    });
    let authoritative_payment =
        active_job_has_wallet_authoritative_settlement(job, settlement_payment);
    let effective_stage =
        active_job_effective_stage(job, active_job, authoritative_payment, settlement_payment);
    let continuity_window = continuity_window_seconds(job.ttl_seconds);
    let (authority, phase, next_expected_event, continuity_window_seconds) =
        if effective_stage == JobLifecycleStage::Paid || authoritative_payment {
            (
                Nip90FlowAuthority::Wallet,
                Nip90FlowPhase::Paid,
                "none".to_string(),
                None,
            )
        } else if effective_stage == JobLifecycleStage::Failed
            && job
                .failure_reason
                .as_deref()
                .is_some_and(active_job_is_settlement_timeout_reason)
        {
            (
                Nip90FlowAuthority::Wallet,
                Nip90FlowPhase::DeliveredUnpaid,
                "buyer settlement timed out".to_string(),
                Some(continuity_window),
            )
        } else if effective_stage == JobLifecycleStage::Failed {
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
        } else if effective_stage == JobLifecycleStage::Delivered {
            if active_job.pending_bolt11.is_some() {
                (
                    Nip90FlowAuthority::Wallet,
                    Nip90FlowPhase::AwaitingPayment,
                    "buyer Lightning payment".to_string(),
                    Some(continuity_window),
                )
            } else {
                (
                    Nip90FlowAuthority::Provider,
                    Nip90FlowPhase::RequestingPayment,
                    "publish buyer Lightning invoice".to_string(),
                    Some(continuity_window),
                )
            }
        } else if effective_stage == JobLifecycleStage::Running {
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
        settlement_status: settlement_payment.map(|payment| payment.status.clone()),
        settlement_method: settlement_payment.map(|payment| payment.method.clone()),
        settlement_amount_sats: settlement_payment.map(|payment| payment.amount_sats),
        settlement_fees_sats: settlement_payment.map(|payment| payment.fees_sats),
        settlement_net_wallet_delta_sats: settlement_payment.map(wallet_payment_net_delta_sats),
        continuity_window_seconds,
        failure_reason: job.failure_reason.clone(),
    })
}

fn active_job_is_settlement_timeout_reason(reason: &str) -> bool {
    let normalized = reason.trim().to_ascii_lowercase();
    normalized.contains("job settlement timed out")
        || normalized.contains("delivered but unpaid")
        || normalized.contains("awaiting buyer settlement")
}

fn active_job_has_wallet_authoritative_settlement(
    job: &ActiveJobRecord,
    settlement_payment: Option<&openagents_spark::PaymentSummary>,
) -> bool {
    authoritative_payment_pointer(job.payment_id.as_deref())
        || settlement_payment
            .is_some_and(|payment| is_settled_wallet_payment_status(payment.status.as_str()))
}

fn active_job_effective_stage(
    job: &ActiveJobRecord,
    active_job: &ActiveJobState,
    authoritative_payment: bool,
    settlement_payment: Option<&openagents_spark::PaymentSummary>,
) -> JobLifecycleStage {
    if job.stage != JobLifecycleStage::Paid || authoritative_payment {
        return job.stage;
    }
    if settlement_payment
        .is_some_and(|payment| is_settled_wallet_payment_status(payment.status.as_str()))
    {
        return JobLifecycleStage::Paid;
    }
    if job.sa_tick_result_event_id.is_some() {
        return JobLifecycleStage::Delivered;
    }
    if active_job.result_publish_in_flight || active_job.pending_result_publish_event_id.is_some() {
        return JobLifecycleStage::Running;
    }
    if job.sa_tick_request_event_id.is_some() {
        return JobLifecycleStage::Running;
    }
    JobLifecycleStage::Accepted
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
    buy_mode: &BuyModePaneState,
    network_requests: &NetworkRequestsState,
    spark_wallet: &SparkPaneState,
    now: Instant,
) -> Vec<String> {
    let snapshots = buy_mode_request_flow_snapshots(network_requests, spark_wallet);
    let in_flight = snapshots
        .iter()
        .find(|request| !request.status.is_terminal());
    let loop_line = if !buy_mode.buy_mode_loop_enabled {
        format!(
            "Dispatch loop: off // cadence={} // policy=single-flight",
            mission_control_buy_mode_interval_label()
        )
    } else if let Some(request) = in_flight {
        format!(
            "Dispatch loop: on // cadence={} // policy=single-flight // blocked by {} [{} phase={} auth={} next={}]",
            mission_control_buy_mode_interval_label(),
            compact_buy_mode_request_id(request.request_id.as_str()),
            request.status.label(),
            request.phase.as_str(),
            request.authority.as_str(),
            request.next_expected_event,
        )
    } else {
        let next = buy_mode
            .buy_mode_next_dispatch_countdown_label(now)
            .unwrap_or_else(|| "now".to_string());
        format!(
            "Dispatch loop: on // cadence={} // policy=single-flight // next={}",
            mission_control_buy_mode_interval_label(),
            next
        )
    };

    let recent_statuses = if snapshots.is_empty() {
        "Recent live request statuses: none yet".to_string()
    } else {
        let preview = snapshots
            .iter()
            .take(4)
            .map(|request| {
                let mut line = format!(
                    "{}={}({}/{})",
                    compact_buy_mode_request_id(request.request_id.as_str()),
                    request.status.label(),
                    request.phase.as_str(),
                    request.authority.as_str()
                );
                if let Some(blocker_codes) = request.payment_blocker_codes_label() {
                    line.push_str(" blocker=");
                    line.push_str(blocker_codes.as_str());
                }
                line
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
    buy_mode: &BuyModePaneState,
    network_requests: &NetworkRequestsState,
    spark_wallet: &SparkPaneState,
) -> String {
    let mut sections = vec![
        "Buy Mode Payments".to_string(),
        buy_mode_payments_summary_text(network_requests, spark_wallet),
        buy_mode_payments_status_lines(buy_mode, network_requests, spark_wallet, Instant::now())
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
    budget_sats: Option<u64>,
    fees_sats: Option<u64>,
    total_debit_sats: Option<u64>,
    net_wallet_delta_sats: Option<i64>,
    wallet_status: String,
    wallet_method: String,
    provider_pubkey: String,
    request_id: String,
    request_type: String,
    authority: Nip90FlowAuthority,
    phase: Nip90FlowPhase,
    next_expected_event: String,
    selected_provider_pubkey: String,
    result_provider_pubkey: String,
    invoice_provider_pubkey: String,
    payable_provider_pubkey: String,
    payment_blocker_codes: Vec<String>,
    payment_blocker_summary: Option<String>,
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
        amount_sats: request.invoice_amount_sats.unwrap_or(request.budget_sats),
        budget_sats: Some(request.budget_sats),
        fees_sats: request.fees_sats,
        total_debit_sats: request.total_debit_sats,
        net_wallet_delta_sats: request.net_wallet_delta_sats,
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
        result_provider_pubkey: request
            .result_provider_pubkey
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        invoice_provider_pubkey: request
            .invoice_provider_pubkey
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        payable_provider_pubkey: request
            .payable_provider_pubkey
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        payment_blocker_codes: request.payment_blocker_codes.clone(),
        payment_blocker_summary: request.payment_blocker_summary.clone(),
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
        budget_sats: None,
        fees_sats: Some(payment.fees_sats),
        total_debit_sats: Some(wallet_payment_total_debit_sats(payment)),
        net_wallet_delta_sats: Some(wallet_payment_net_delta_sats(payment)),
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
        result_provider_pubkey: "-".to_string(),
        invoice_provider_pubkey: "-".to_string(),
        payable_provider_pubkey: "-".to_string(),
        payment_blocker_codes: Vec::new(),
        payment_blocker_summary: None,
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
            "{}  status={}  invoice={} sats  fee={}  total_debit={}  wallet_delta={}  wallet_status={}  wallet_method={}  provider_pubkey={}",
            buy_mode_payment_timestamp_label(entry.timestamp),
            entry.status,
            entry.amount_sats,
            buy_mode_optional_sats_label(entry.fees_sats),
            buy_mode_optional_sats_label(entry.total_debit_sats),
            buy_mode_optional_signed_sats_label(entry.net_wallet_delta_sats),
            entry.wallet_status,
            entry.wallet_method,
            entry.provider_pubkey,
        ),
    ));
    rows.push((
        entry.stream.clone(),
        format!(
            "request_id={}  request_type={}  budget={}  authority={}  phase={}  next={}  payment_pointer={}  request_event_id={}  result_event_id={}  payment_hash={}  source={}",
            entry.request_id,
            entry.request_type,
            buy_mode_optional_sats_label(entry.budget_sats),
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
                "selected_provider={}  result_provider={}  invoice_provider={}  payable_provider={}  blockers={}  losers={}  loser_summary={}",
                entry.selected_provider_pubkey,
                entry.result_provider_pubkey,
                entry.invoice_provider_pubkey,
                entry.payable_provider_pubkey,
                if entry.payment_blocker_codes.is_empty() {
                    "-".to_string()
                } else {
                    entry.payment_blocker_codes.join(",")
                },
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
    if let Some(blocker_summary) = entry.payment_blocker_summary.as_deref() {
        rows.push((
            TerminalStream::Stderr,
            format!("payment_blocker={blocker_summary}"),
        ));
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

fn buy_mode_optional_signed_sats_label(value: Option<i64>) -> String {
    value
        .map(crate::spark_wallet::format_wallet_delta_sats)
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

fn msats_to_sats_ceil(msats: u64) -> u64 {
    let sats = msats / 1_000;
    if msats % 1_000 == 0 {
        sats
    } else {
        sats.saturating_add(1)
    }
}

fn request_provider_invoice_amount_sats(
    request: &SubmittedNetworkRequest,
    preferred_provider_pubkey: Option<&str>,
) -> Option<u64> {
    let preferred_amount = preferred_provider_pubkey.and_then(|provider_pubkey| {
        request
            .provider_observations
            .iter()
            .find(|observation| {
                normalize_pubkey(observation.provider_pubkey.as_str())
                    == normalize_pubkey(provider_pubkey)
            })
            .and_then(|observation| {
                analyze_invoice_amount_msats(
                    observation.last_feedback_amount_msats,
                    observation.last_feedback_bolt11.as_deref(),
                )
                .effective_amount_msats
            })
    });

    preferred_amount
        .or_else(|| {
            request
                .provider_observations
                .iter()
                .find_map(|observation| {
                    analyze_invoice_amount_msats(
                        observation.last_feedback_amount_msats,
                        observation.last_feedback_bolt11.as_deref(),
                    )
                    .effective_amount_msats
                })
        })
        .map(msats_to_sats_ceil)
        .filter(|amount| *amount > 0)
}

fn observation_by_provider<'a>(
    request: &'a SubmittedNetworkRequest,
    provider_pubkey: Option<&str>,
) -> Option<&'a crate::state::operations::NetworkRequestProviderObservation> {
    let provider_pubkey = provider_pubkey?;
    request.provider_observations.iter().find(|observation| {
        normalize_pubkey(observation.provider_pubkey.as_str()) == normalize_pubkey(provider_pubkey)
    })
}

fn observation_has_payment_feedback(
    observation: &crate::state::operations::NetworkRequestProviderObservation,
) -> bool {
    observation
        .last_feedback_status
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some_and(|status| status.eq_ignore_ascii_case("payment-required"))
}

fn observation_has_payment_feedback_without_bolt11(
    observation: &crate::state::operations::NetworkRequestProviderObservation,
) -> bool {
    observation_has_payment_feedback(observation) && !observation_has_valid_invoice(observation)
}

fn push_unique_blocker_code(blockers: &mut Vec<String>, code: &str) {
    if blockers.iter().all(|existing| existing != code) {
        blockers.push(code.to_string());
    }
}

fn push_unique_blocker_detail(details: &mut Vec<String>, detail: String) {
    if details.iter().all(|existing| existing != &detail) {
        details.push(detail);
    }
}

pub(crate) fn derive_buyer_payment_blockers(
    request: &SubmittedNetworkRequest,
) -> (Vec<String>, Option<String>) {
    let mut blocker_codes = Vec::<String>::new();
    let mut blocker_details = Vec::<String>::new();

    let result_observation =
        observation_by_provider(request, request.result_provider_pubkey.as_deref());
    let invoice_observation =
        observation_by_provider(request, request.invoice_provider_pubkey.as_deref());

    if let Some(observation) = result_observation {
        if !observation_has_valid_invoice(observation) {
            push_unique_blocker_code(&mut blocker_codes, "result_without_invoice");
            push_unique_blocker_detail(
                &mut blocker_details,
                format!(
                    "result provider {} has no valid invoice",
                    short_id(observation.provider_pubkey.as_str())
                ),
            );
        }
    }

    if let Some(observation) = invoice_observation {
        if !observation_has_non_error_result(observation) {
            push_unique_blocker_code(&mut blocker_codes, "invoice_without_result");
            push_unique_blocker_detail(
                &mut blocker_details,
                format!(
                    "invoice provider {} has no non-error result",
                    short_id(observation.provider_pubkey.as_str())
                ),
            );
        }
        if let Some(amount_msats) = analyze_invoice_amount_msats(
            observation.last_feedback_amount_msats,
            observation.last_feedback_bolt11.as_deref(),
        )
        .effective_amount_msats
        {
            let invoice_sats = msats_to_sats_ceil(amount_msats);
            if invoice_sats > request.budget_sats {
                push_unique_blocker_code(&mut blocker_codes, "invoice_over_budget");
                push_unique_blocker_detail(
                    &mut blocker_details,
                    format!(
                        "invoice provider {} requested {} sats above approved budget {}",
                        short_id(observation.provider_pubkey.as_str()),
                        invoice_sats,
                        request.budget_sats
                    ),
                );
            }
        }
    }

    if let Some(observation) = request
        .provider_observations
        .iter()
        .rev()
        .find(|observation| observation_has_payment_feedback_without_bolt11(observation))
    {
        push_unique_blocker_code(&mut blocker_codes, "invoice_missing_bolt11");
        push_unique_blocker_detail(
            &mut blocker_details,
            format!(
                "provider {} sent payment-required without bolt11 invoice",
                short_id(observation.provider_pubkey.as_str())
            ),
        );
    }

    let error_only_count = request
        .provider_observations
        .iter()
        .filter(|observation| observation_has_error_only_signal(observation))
        .count();
    if blocker_codes.is_empty()
        && request.winning_provider_pubkey.is_none()
        && error_only_count > 0
        && request.result_provider_pubkey.is_none()
        && request.invoice_provider_pubkey.is_none()
    {
        push_unique_blocker_code(&mut blocker_codes, "loser_provider_noise_only");
        push_unique_blocker_detail(
            &mut blocker_details,
            format!("only loser-provider noise remains from {error_only_count} provider(s)"),
        );
    }

    if blocker_codes.is_empty() {
        if let Some(notice) = request.payment_notice.as_deref() {
            let notice = notice.trim();
            if !notice.is_empty() {
                push_unique_blocker_detail(&mut blocker_details, notice.to_string());
            }
        }
    }

    let blocker_summary = (!blocker_details.is_empty()).then(|| blocker_details.join(" // "));
    (blocker_codes, blocker_summary)
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
        ActiveJobState, BuyModePaneState, EarnJobLifecycleProjectionState, JobLifecycleStage,
    };
    use crate::spark_wallet::SparkPaneState;
    use crate::state::operations::{
        BuyerResolutionMode, NetworkRequestStatus, NetworkRequestSubmission, NetworkRequestsState,
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

        assert_eq!(snapshot.status, NetworkRequestStatus::PaymentRequired);
        assert_eq!(snapshot.authority, Nip90FlowAuthority::Wallet);
        assert_eq!(snapshot.phase, Nip90FlowPhase::AwaitingPayment);
        assert_eq!(snapshot.next_expected_event, "buyer Lightning payment");
        assert_eq!(snapshot.wallet_status, "pending");
    }

    #[test]
    fn buyer_snapshot_surfaces_invoice_amount_before_wallet_evidence() {
        let mut requests = NetworkRequestsState::default();
        let request_id = requests
            .queue_request_submission(NetworkRequestSubmission {
                request_id: Some("req-snapshot-invoice-amount".to_string()),
                request_type: crate::app_state::MISSION_CONTROL_BUY_MODE_REQUEST_TYPE.to_string(),
                payload: "BUY MODE OK".to_string(),
                resolution_mode: BuyerResolutionMode::Race,
                target_provider_pubkeys: Vec::new(),
                skill_scope_id: None,
                credit_envelope_ref: None,
                budget_sats: 2,
                timeout_seconds: 75,
                authority_command_seq: 2,
            })
            .expect("queue request");
        let provider_pubkey = "ab".repeat(32);
        requests.apply_nip90_request_publish_outcome(
            request_id.as_str(),
            "event-snapshot-invoice-amount",
            1,
            0,
            None,
        );
        requests.apply_nip90_buyer_feedback_event(
            request_id.as_str(),
            provider_pubkey.as_str(),
            "feedback-invoice-amount-001",
            Some("payment-required"),
            Some("invoice ready"),
            Some(25_000),
            Some("lnbc1snapshotinvoiceamount"),
        );

        let request = requests
            .latest_request_by_type(crate::app_state::MISSION_CONTROL_BUY_MODE_REQUEST_TYPE)
            .expect("latest request");
        let snapshot = build_buyer_request_flow_snapshot(request, &SparkPaneState::default());

        assert_eq!(snapshot.invoice_amount_sats, Some(25));
        assert_eq!(snapshot.net_wallet_delta_sats, None);
        assert!(snapshot.payment_summary().contains("25 sats invoice"));
    }

    #[test]
    fn buyer_snapshot_distinguishes_seller_settlement_from_local_wallet_confirmation() {
        let mut requests = NetworkRequestsState::default();
        let request_id = requests
            .queue_request_submission(NetworkRequestSubmission {
                request_id: Some("req-snapshot-seller-settled".to_string()),
                request_type: crate::app_state::MISSION_CONTROL_BUY_MODE_REQUEST_TYPE.to_string(),
                payload: "BUY MODE OK".to_string(),
                resolution_mode: BuyerResolutionMode::Race,
                target_provider_pubkeys: Vec::new(),
                skill_scope_id: None,
                credit_envelope_ref: None,
                budget_sats: 2,
                timeout_seconds: 75,
                authority_command_seq: 3,
            })
            .expect("queue request");
        let provider_pubkey = "cd".repeat(32);
        requests.apply_nip90_request_publish_outcome(
            request_id.as_str(),
            "event-snapshot-seller-settled",
            1,
            0,
            None,
        );
        requests.apply_nip90_buyer_result_event(
            request_id.as_str(),
            provider_pubkey.as_str(),
            "result-snapshot-seller-settled",
            Some("success"),
        );
        requests.apply_nip90_buyer_feedback_event(
            request_id.as_str(),
            provider_pubkey.as_str(),
            "feedback-snapshot-payment-required",
            Some("payment-required"),
            Some("invoice ready"),
            Some(2_000),
            Some("lnbc1snapshotsettled"),
        );
        requests
            .prepare_auto_payment_attempt(
                request_id.as_str(),
                "lnbc1snapshotsettled",
                Some(2_000),
                1_762_700_090,
            )
            .expect("payment-required invoice should queue");
        requests.record_auto_payment_pointer(
            request_id.as_str(),
            "wallet-snapshot-settled-pending-001",
        );
        requests.apply_nip90_buyer_feedback_event(
            request_id.as_str(),
            provider_pubkey.as_str(),
            "feedback-snapshot-seller-settled",
            Some("success"),
            Some("wallet-confirmed settlement recorded"),
            Some(2_000),
            None,
        );

        let request = requests
            .latest_request_by_type(crate::app_state::MISSION_CONTROL_BUY_MODE_REQUEST_TYPE)
            .expect("latest request");
        let snapshot = build_buyer_request_flow_snapshot(request, &SparkPaneState::default());

        assert_eq!(snapshot.status, NetworkRequestStatus::PaymentRequired);
        assert_eq!(snapshot.authority, Nip90FlowAuthority::Provider);
        assert_eq!(snapshot.phase, Nip90FlowPhase::SellerSettledPendingWallet);
        assert_eq!(
            snapshot.next_expected_event,
            "buyer local wallet confirmation"
        );
        assert_eq!(snapshot.wallet_status, "pending");
        assert_eq!(
            snapshot.seller_success_feedback_event_id.as_deref(),
            Some("feedback-snapshot-seller-settled")
        );
        assert_eq!(snapshot.work_label(), "settled");
        assert!(
            snapshot
                .work_summary()
                .contains("seller settlement confirmed")
        );
        assert!(
            snapshot
                .payment_summary()
                .contains("seller settled; awaiting local wallet confirmation")
        );
        assert!(
            snapshot
                .mission_control_log_line()
                .contains("seller_settlement_feedback=")
        );
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
            target_provider_pubkeys: Vec::new(),
            skill_scope_id: None,
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_request_event_id: None,
            sa_tick_result_event_id: None,
            ac_envelope_event_id: None,
            price_sats: 2,
            ttl_seconds: 75,
            created_at_epoch_seconds: Some(1_760_000_000),
            expires_at_epoch_seconds: Some(1_760_000_075),
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
            &SparkPaneState::default(),
        )
        .expect("active snapshot");

        assert_eq!(snapshot.authority, Nip90FlowAuthority::Relay);
        assert_eq!(snapshot.phase, Nip90FlowPhase::PublishingResult);
        assert_eq!(snapshot.next_expected_event, "relay confirmation");
        assert_eq!(snapshot.continuity_window_seconds, Some(195));
    }

    #[test]
    fn active_job_snapshot_distinguishes_delivered_unpaid_timeout() {
        let mut active_job = ActiveJobState::default();
        let request = crate::state::job_inbox::JobInboxRequest {
            request_id: "req-active-unpaid-snapshot".to_string(),
            requester: "npub1requester".to_string(),
            demand_source: crate::app_state::JobDemandSource::OpenNetwork,
            request_kind: 5050,
            capability: "text.generation".to_string(),
            execution_input: None,
            execution_prompt: Some("BUY MODE OK".to_string()),
            execution_params: Vec::new(),
            requested_model: None,
            requested_output_mime: None,
            target_provider_pubkeys: Vec::new(),
            skill_scope_id: None,
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_request_event_id: None,
            sa_tick_result_event_id: None,
            ac_envelope_event_id: None,
            price_sats: 2,
            ttl_seconds: 75,
            created_at_epoch_seconds: Some(1_760_000_000),
            expires_at_epoch_seconds: Some(1_760_000_075),
            validation: crate::state::job_inbox::JobInboxValidation::Valid,
            arrival_seq: 1,
            decision: crate::state::job_inbox::JobInboxDecision::Pending,
        };
        active_job.start_from_request(&request);
        active_job.pending_bolt11 = Some("lnbc20n1activeunpaid".to_string());
        let job = active_job.job.as_mut().expect("job");
        job.stage = JobLifecycleStage::Failed;
        job.sa_tick_result_event_id = Some("result-active-unpaid-001".to_string());
        job.failure_reason = Some(
            "job delivered but unpaid timed out after 195s while awaiting buyer settlement"
                .to_string(),
        );

        let snapshot = build_active_job_flow_snapshot(
            &active_job,
            &EarnJobLifecycleProjectionState::default(),
            &SparkPaneState::default(),
        )
        .expect("active snapshot");

        assert_eq!(snapshot.authority, Nip90FlowAuthority::Wallet);
        assert_eq!(snapshot.phase, Nip90FlowPhase::DeliveredUnpaid);
        assert_eq!(snapshot.next_expected_event, "buyer settlement timed out");
        assert_eq!(snapshot.continuity_window_seconds, Some(195));
        assert_eq!(
            snapshot.result_event_id.as_deref(),
            Some("result-active-unpaid-001")
        );
        assert_eq!(
            snapshot.pending_bolt11.as_deref(),
            Some("lnbc20n1activeunpaid")
        );
        assert_eq!(
            snapshot.mission_control_continuity_summary().as_deref(),
            Some("buyer never settled // result result..-001 // window 195s")
        );
    }

    #[test]
    fn active_job_snapshot_surfaces_wallet_settlement_fee_truth() {
        let request = crate::state::job_inbox::JobInboxRequest {
            request_id: "req-active-settlement-snapshot".to_string(),
            requester: "npub1requester".to_string(),
            demand_source: crate::app_state::JobDemandSource::OpenNetwork,
            request_kind: 5050,
            capability: "text.generation".to_string(),
            execution_input: None,
            execution_prompt: Some("BUY MODE OK".to_string()),
            execution_params: Vec::new(),
            requested_model: None,
            requested_output_mime: None,
            target_provider_pubkeys: Vec::new(),
            skill_scope_id: None,
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_request_event_id: None,
            sa_tick_result_event_id: None,
            ac_envelope_event_id: None,
            price_sats: 2,
            ttl_seconds: 75,
            created_at_epoch_seconds: Some(1_760_000_000),
            expires_at_epoch_seconds: Some(1_760_000_075),
            validation: crate::state::job_inbox::JobInboxValidation::Valid,
            arrival_seq: 1,
            decision: crate::state::job_inbox::JobInboxDecision::Pending,
        };
        let mut active_job = ActiveJobState::default();
        active_job.start_from_request(&request);
        let job = active_job.job.as_mut().expect("job");
        job.stage = JobLifecycleStage::Paid;
        job.payment_id = Some("wallet-provider-settlement-001".to_string());

        let mut spark_wallet = SparkPaneState::default();
        spark_wallet
            .recent_payments
            .push(openagents_spark::PaymentSummary {
                id: "wallet-provider-settlement-001".to_string(),
                direction: "receive".to_string(),
                status: "succeeded".to_string(),
                amount_sats: 2,
                fees_sats: 1,
                method: "lightning".to_string(),
                timestamp: 1_762_700_777,
                ..Default::default()
            });

        let snapshot = build_active_job_flow_snapshot(
            &active_job,
            &EarnJobLifecycleProjectionState::default(),
            &spark_wallet,
        )
        .expect("active snapshot");

        assert_eq!(snapshot.settlement_status.as_deref(), Some("succeeded"));
        assert_eq!(snapshot.settlement_method.as_deref(), Some("lightning"));
        assert_eq!(snapshot.settlement_amount_sats, Some(2));
        assert_eq!(snapshot.settlement_fees_sats, Some(1));
        assert_eq!(snapshot.settlement_net_wallet_delta_sats, Some(2));
    }

    #[test]
    fn active_job_snapshot_does_not_project_paid_from_feedback_without_wallet_settlement() {
        let request = crate::state::job_inbox::JobInboxRequest {
            request_id: "req-active-invalid-paid".to_string(),
            requester: "npub1requester".to_string(),
            demand_source: crate::app_state::JobDemandSource::OpenNetwork,
            request_kind: 5050,
            capability: "text.generation".to_string(),
            execution_input: None,
            execution_prompt: Some("BUY MODE OK".to_string()),
            execution_params: Vec::new(),
            requested_model: None,
            requested_output_mime: None,
            target_provider_pubkeys: Vec::new(),
            skill_scope_id: None,
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_request_event_id: None,
            sa_tick_result_event_id: None,
            ac_envelope_event_id: None,
            price_sats: 2,
            ttl_seconds: 75,
            created_at_epoch_seconds: Some(1_760_000_000),
            expires_at_epoch_seconds: Some(1_760_000_075),
            validation: crate::state::job_inbox::JobInboxValidation::Valid,
            arrival_seq: 1,
            decision: crate::state::job_inbox::JobInboxDecision::Pending,
        };
        let mut active_job = ActiveJobState::default();
        active_job.start_from_request(&request);
        active_job.pending_bolt11 = Some("lnbc20n1invalidpaid".to_string());
        let job = active_job.job.as_mut().expect("job");
        job.stage = JobLifecycleStage::Paid;
        job.sa_tick_result_event_id = Some("result-invalid-paid-001".to_string());
        job.ac_settlement_event_id = Some("feedback-invalid-paid-001".to_string());

        let snapshot = build_active_job_flow_snapshot(
            &active_job,
            &EarnJobLifecycleProjectionState::default(),
            &SparkPaneState::default(),
        )
        .expect("active snapshot");

        assert_eq!(snapshot.phase, Nip90FlowPhase::AwaitingPayment);
        assert_eq!(snapshot.authority, Nip90FlowAuthority::Wallet);
        assert_eq!(snapshot.next_expected_event, "buyer Lightning payment");
        assert_eq!(snapshot.payment_pointer, None);
    }

    #[test]
    fn active_job_snapshot_running_stage_ignores_nonwallet_settlement_feedback() {
        let request = crate::state::job_inbox::JobInboxRequest {
            request_id: "req-active-running-feedback".to_string(),
            requester: "npub1requester".to_string(),
            demand_source: crate::app_state::JobDemandSource::OpenNetwork,
            request_kind: 5050,
            capability: "text.generation".to_string(),
            execution_input: None,
            execution_prompt: Some("BUY MODE OK".to_string()),
            execution_params: Vec::new(),
            requested_model: None,
            requested_output_mime: None,
            target_provider_pubkeys: Vec::new(),
            skill_scope_id: None,
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_request_event_id: Some("request-running-feedback-001".to_string()),
            sa_tick_result_event_id: None,
            ac_envelope_event_id: None,
            price_sats: 2,
            ttl_seconds: 75,
            created_at_epoch_seconds: Some(1_760_000_000),
            expires_at_epoch_seconds: Some(1_760_000_075),
            validation: crate::state::job_inbox::JobInboxValidation::Valid,
            arrival_seq: 1,
            decision: crate::state::job_inbox::JobInboxDecision::Pending,
        };
        let mut active_job = ActiveJobState::default();
        active_job.start_from_request(&request);
        let job = active_job.job.as_mut().expect("job");
        job.stage = JobLifecycleStage::Running;
        job.ac_settlement_event_id = Some("feedback-running-001".to_string());

        let snapshot = build_active_job_flow_snapshot(
            &active_job,
            &EarnJobLifecycleProjectionState::default(),
            &SparkPaneState::default(),
        )
        .expect("active snapshot");

        assert_eq!(snapshot.phase, Nip90FlowPhase::Executing);
        assert_eq!(snapshot.authority, Nip90FlowAuthority::Provider);
        assert_eq!(snapshot.next_expected_event, "local execution");
    }

    #[test]
    fn active_job_snapshot_failed_stage_ignores_nonwallet_settlement_feedback() {
        let request = crate::state::job_inbox::JobInboxRequest {
            request_id: "req-active-failed-feedback".to_string(),
            requester: "npub1requester".to_string(),
            demand_source: crate::app_state::JobDemandSource::OpenNetwork,
            request_kind: 5050,
            capability: "text.generation".to_string(),
            execution_input: None,
            execution_prompt: Some("BUY MODE OK".to_string()),
            execution_params: Vec::new(),
            requested_model: None,
            requested_output_mime: None,
            target_provider_pubkeys: Vec::new(),
            skill_scope_id: None,
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_request_event_id: None,
            sa_tick_result_event_id: None,
            ac_envelope_event_id: None,
            price_sats: 2,
            ttl_seconds: 75,
            created_at_epoch_seconds: Some(1_760_000_000),
            expires_at_epoch_seconds: Some(1_760_000_075),
            validation: crate::state::job_inbox::JobInboxValidation::Valid,
            arrival_seq: 1,
            decision: crate::state::job_inbox::JobInboxDecision::Pending,
        };
        let mut active_job = ActiveJobState::default();
        active_job.start_from_request(&request);
        let job = active_job.job.as_mut().expect("job");
        job.stage = JobLifecycleStage::Failed;
        job.ac_settlement_event_id = Some("feedback-failed-001".to_string());
        job.failure_reason = Some("provider runtime failed".to_string());

        let snapshot = build_active_job_flow_snapshot(
            &active_job,
            &EarnJobLifecycleProjectionState::default(),
            &SparkPaneState::default(),
        )
        .expect("active snapshot");

        assert_eq!(snapshot.phase, Nip90FlowPhase::Failed);
        assert_eq!(snapshot.authority, Nip90FlowAuthority::Provider);
        assert_eq!(snapshot.next_expected_event, "none");
    }

    #[test]
    fn buy_mode_status_lines_use_snapshot_phase_and_authority() {
        let mut buy_mode = BuyModePaneState::default();
        buy_mode.buy_mode_loop_enabled = true;

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
            &buy_mode,
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
            Some(payable_provider.as_str())
        );
        assert_eq!(
            snapshot.result_provider_pubkey.as_deref(),
            Some(losing_provider.as_str())
        );
        assert_eq!(
            snapshot.invoice_provider_pubkey.as_deref(),
            Some(payable_provider.as_str())
        );
        assert_eq!(
            snapshot.payable_provider_pubkey.as_deref(),
            Some(payable_provider.as_str())
        );
        assert_eq!(snapshot.loser_provider_count, 1);
        assert!(snapshot.payment_blocker_codes.is_empty());
        assert_eq!(snapshot.payment_blocker_summary, None);
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
    fn buyer_snapshot_derives_missing_bolt11_and_over_budget_blockers() {
        let mut requests = NetworkRequestsState::default();
        let request_id = requests
            .queue_request_submission(NetworkRequestSubmission {
                request_id: Some("req-snapshot-blockers-001".to_string()),
                request_type: crate::app_state::MISSION_CONTROL_BUY_MODE_REQUEST_TYPE.to_string(),
                payload: "BUY MODE OK".to_string(),
                resolution_mode: BuyerResolutionMode::Race,
                target_provider_pubkeys: Vec::new(),
                skill_scope_id: None,
                credit_envelope_ref: None,
                budget_sats: 2,
                timeout_seconds: 75,
                authority_command_seq: 11,
            })
            .expect("queue request");
        let result_provider = "51".repeat(32);
        let invoice_provider = "61".repeat(32);
        let missing_bolt11_provider = "71".repeat(32);

        requests.apply_nip90_request_publish_outcome(
            request_id.as_str(),
            "event-snapshot-blockers-001",
            3,
            1,
            None,
        );
        requests.apply_nip90_buyer_result_event(
            request_id.as_str(),
            result_provider.as_str(),
            "result-snapshot-blockers-001",
            Some("success"),
        );
        requests.apply_nip90_buyer_feedback_event(
            request_id.as_str(),
            invoice_provider.as_str(),
            "feedback-snapshot-blockers-001",
            Some("payment-required"),
            Some("invoice ready"),
            Some(25_000),
            Some("lnbc250n1snapshotblockers"),
        );
        requests.apply_nip90_buyer_feedback_event(
            request_id.as_str(),
            missing_bolt11_provider.as_str(),
            "feedback-snapshot-blockers-missing-001",
            Some("payment-required"),
            Some("send sats"),
            Some(2_000),
            None,
        );

        let request = requests
            .latest_request_by_type(crate::app_state::MISSION_CONTROL_BUY_MODE_REQUEST_TYPE)
            .expect("latest request");
        let snapshot = build_buyer_request_flow_snapshot(request, &SparkPaneState::default());

        assert_eq!(snapshot.payable_provider_pubkey, None);
        assert_eq!(
            snapshot.payment_blocker_codes,
            vec![
                "result_without_invoice".to_string(),
                "invoice_without_result".to_string(),
                "invoice_over_budget".to_string(),
                "invoice_missing_bolt11".to_string(),
            ]
        );
        assert!(
            snapshot
                .payment_blocker_summary
                .as_deref()
                .is_some_and(|summary| summary.contains(
                    "invoice provider 616161..6161 requested 25 sats above approved budget 2"
                ))
        );
        assert!(
            snapshot
                .payment_blocker_summary
                .as_deref()
                .is_some_and(|summary| summary.contains(
                    "provider 717171..7171 sent payment-required without bolt11 invoice"
                ))
        );
    }

    #[test]
    fn continuity_window_matches_runtime_grace_policy() {
        assert_eq!(continuity_window_seconds(60), 180);
        assert_eq!(continuity_window_seconds(75), 195);
        assert_eq!(continuity_window_seconds(300), 420);
    }
}
