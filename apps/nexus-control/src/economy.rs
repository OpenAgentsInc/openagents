use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use openagents_kernel_core::snapshots::{
    ComputeBreakerStatusRow, ComputeRolloutGateRow, ComputeTruthLabelRow,
};
use serde::{Deserialize, Serialize};

const AUTHORITY_NAME: &str = "openagents-hosted-nexus";
const RECEIPT_RETENTION_LIMIT: usize = 8_192;
const PUBLIC_RECENT_RECEIPT_LIMIT: usize = 16;
const PUBLIC_STATS_WINDOW_MS: u64 = 86_400_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct AuthorityReceiptContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount_sats: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment_pointer: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attributes: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthorityReceipt {
    pub seq: u64,
    pub receipt_id: String,
    pub receipt_type: String,
    pub recorded_at_unix_ms: u64,
    pub authority: String,
    #[serde(flatten)]
    pub context: AuthorityReceiptContext,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublicRecentReceipt {
    pub receipt_id: String,
    pub receipt_type: String,
    pub recorded_at_unix_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount_sats: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PublicStatsSnapshot {
    pub service: String,
    pub authority: String,
    pub hosted_nexus_relay_url: String,
    pub as_of_unix_ms: u64,
    pub window_started_at_unix_ms: u64,
    pub receipt_count: usize,
    pub receipt_persistence_enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt_persistence_error: Option<String>,
    pub sessions_active: usize,
    pub sessions_issued_24h: u64,
    pub sync_tokens_active: usize,
    pub sync_tokens_issued_24h: u64,
    pub starter_demand_budget_cap_sats: u64,
    pub starter_demand_budget_allocated_sats: u64,
    pub starter_offers_waiting_ack: usize,
    pub starter_offers_running: usize,
    pub starter_offers_dispatched_24h: u64,
    pub starter_offers_started_24h: u64,
    pub starter_offer_heartbeats_24h: u64,
    pub starter_offers_completed_24h: u64,
    pub starter_offers_released_24h: u64,
    pub starter_offers_expired_24h: u64,
    pub starter_demand_ineligible_polls_24h: u64,
    pub starter_offer_start_rate_24h: f64,
    pub starter_offer_completion_rate_24h: f64,
    pub starter_offer_loss_rate_24h: f64,
    pub starter_demand_paid_sats_24h: u64,
    pub starter_demand_released_sats_24h: u64,
    pub compute_products_active: u64,
    pub compute_capacity_lots_open: u64,
    pub compute_capacity_lots_delivering: u64,
    pub compute_instruments_active: u64,
    pub compute_inventory_quantity_open: u64,
    pub compute_inventory_quantity_reserved: u64,
    pub compute_inventory_quantity_delivering: u64,
    pub compute_delivery_proofs_24h: u64,
    pub compute_delivery_quantity_24h: u64,
    pub compute_delivery_rejections_24h: u64,
    pub compute_delivery_variances_24h: u64,
    pub compute_validator_challenges_open: u64,
    pub compute_validator_challenges_queued: u64,
    pub compute_validator_challenges_verified_24h: u64,
    pub compute_validator_challenges_rejected_24h: u64,
    pub compute_validator_challenges_timed_out_24h: u64,
    pub compute_delivery_accept_rate_24h: f64,
    pub compute_fill_ratio_24h: f64,
    pub compute_priced_instruments_24h: u64,
    pub compute_indices_published_24h: u64,
    pub compute_index_corrections_24h: u64,
    pub compute_index_thin_windows_24h: u64,
    pub compute_index_settlement_eligible_24h: u64,
    pub compute_index_quality_score_24h: f64,
    pub compute_active_provider_count: u64,
    pub compute_provider_concentration_hhi: f64,
    pub compute_forward_physical_instruments_active: u64,
    pub compute_forward_physical_open_quantity: u64,
    pub compute_forward_physical_defaults_24h: u64,
    pub compute_future_cash_instruments_active: u64,
    pub compute_future_cash_open_interest: u64,
    pub compute_future_cash_cash_settlements_24h: u64,
    pub compute_future_cash_cash_flow_24h: u64,
    pub compute_future_cash_defaults_24h: u64,
    pub compute_future_cash_collateral_shortfall_24h: u64,
    pub compute_structured_instruments_active: u64,
    pub compute_structured_instruments_closed_24h: u64,
    pub compute_max_buyer_concentration_share: f64,
    pub compute_paper_to_physical_ratio: f64,
    pub compute_deliverable_coverage_ratio: f64,
    pub compute_breakers_tripped: u64,
    pub compute_breakers_guarded: u64,
    pub compute_breaker_states: Vec<ComputeBreakerStatusRow>,
    pub compute_rollout_gates: Vec<ComputeRolloutGateRow>,
    pub compute_truth_labels: Vec<ComputeTruthLabelRow>,
    pub compute_reconciliation_gap_24h: u64,
    pub compute_policy_bundle_id: String,
    pub compute_policy_version: String,
    pub liquidity_quotes_active: u64,
    pub liquidity_route_plans_active: u64,
    pub liquidity_envelopes_open: u64,
    pub liquidity_settlements_24h: u64,
    pub liquidity_reserve_partitions_active: u64,
    pub liquidity_value_moved_24h: u64,
    pub risk_coverage_offers_open: u64,
    pub risk_coverage_bindings_active: u64,
    pub risk_prediction_positions_open: u64,
    pub risk_claims_open: u64,
    pub risk_signals_active: u64,
    pub risk_implied_fail_probability_bps: u32,
    pub risk_calibration_score: f64,
    pub risk_coverage_concentration_hhi: f64,
    pub recent_receipts: Vec<PublicRecentReceipt>,
}

#[derive(Debug, Clone, Default)]
pub struct PublicRuntimeSnapshot {
    pub hosted_nexus_relay_url: String,
    pub sessions_active: usize,
    pub sync_tokens_active: usize,
    pub starter_demand_budget_cap_sats: u64,
    pub starter_demand_budget_allocated_sats: u64,
    pub starter_offers_waiting_ack: usize,
    pub starter_offers_running: usize,
    pub compute_products_active: u64,
    pub compute_capacity_lots_open: u64,
    pub compute_capacity_lots_delivering: u64,
    pub compute_instruments_active: u64,
    pub compute_inventory_quantity_open: u64,
    pub compute_inventory_quantity_reserved: u64,
    pub compute_inventory_quantity_delivering: u64,
    pub compute_delivery_proofs_24h: u64,
    pub compute_delivery_quantity_24h: u64,
    pub compute_delivery_rejections_24h: u64,
    pub compute_delivery_variances_24h: u64,
    pub compute_validator_challenges_open: u64,
    pub compute_validator_challenges_queued: u64,
    pub compute_validator_challenges_verified_24h: u64,
    pub compute_validator_challenges_rejected_24h: u64,
    pub compute_validator_challenges_timed_out_24h: u64,
    pub compute_delivery_accept_rate_24h: f64,
    pub compute_fill_ratio_24h: f64,
    pub compute_priced_instruments_24h: u64,
    pub compute_indices_published_24h: u64,
    pub compute_index_corrections_24h: u64,
    pub compute_index_thin_windows_24h: u64,
    pub compute_index_settlement_eligible_24h: u64,
    pub compute_index_quality_score_24h: f64,
    pub compute_active_provider_count: u64,
    pub compute_provider_concentration_hhi: f64,
    pub compute_forward_physical_instruments_active: u64,
    pub compute_forward_physical_open_quantity: u64,
    pub compute_forward_physical_defaults_24h: u64,
    pub compute_future_cash_instruments_active: u64,
    pub compute_future_cash_open_interest: u64,
    pub compute_future_cash_cash_settlements_24h: u64,
    pub compute_future_cash_cash_flow_24h: u64,
    pub compute_future_cash_defaults_24h: u64,
    pub compute_future_cash_collateral_shortfall_24h: u64,
    pub compute_structured_instruments_active: u64,
    pub compute_structured_instruments_closed_24h: u64,
    pub compute_max_buyer_concentration_share: f64,
    pub compute_paper_to_physical_ratio: f64,
    pub compute_deliverable_coverage_ratio: f64,
    pub compute_breakers_tripped: u64,
    pub compute_breakers_guarded: u64,
    pub compute_breaker_states: Vec<ComputeBreakerStatusRow>,
    pub compute_rollout_gates: Vec<ComputeRolloutGateRow>,
    pub compute_truth_labels: Vec<ComputeTruthLabelRow>,
    pub compute_reconciliation_gap_24h: u64,
    pub compute_policy_bundle_id: String,
    pub compute_policy_version: String,
    pub liquidity_quotes_active: u64,
    pub liquidity_route_plans_active: u64,
    pub liquidity_envelopes_open: u64,
    pub liquidity_settlements_24h: u64,
    pub liquidity_reserve_partitions_active: u64,
    pub liquidity_value_moved_24h: u64,
    pub risk_coverage_offers_open: u64,
    pub risk_coverage_bindings_active: u64,
    pub risk_prediction_positions_open: u64,
    pub risk_claims_open: u64,
    pub risk_signals_active: u64,
    pub risk_implied_fail_probability_bps: u32,
    pub risk_calibration_score: f64,
    pub risk_coverage_concentration_hhi: f64,
}

#[derive(Debug, Clone, Default)]
pub struct ReceiptLedger {
    next_receipt_seq: u64,
    receipts: Vec<AuthorityReceipt>,
    receipt_log_path: Option<PathBuf>,
    last_persistence_error: Option<String>,
}

impl ReceiptLedger {
    pub fn new(receipt_log_path: Option<PathBuf>) -> Self {
        let mut ledger = Self {
            next_receipt_seq: 1,
            receipts: Vec::new(),
            receipt_log_path,
            last_persistence_error: None,
        };
        ledger.load_existing_receipts();
        ledger
    }

    pub fn record(
        &mut self,
        receipt_type: impl Into<String>,
        recorded_at_unix_ms: u64,
        context: AuthorityReceiptContext,
    ) -> AuthorityReceipt {
        let seq = self.next_receipt_seq;
        self.next_receipt_seq = self.next_receipt_seq.saturating_add(1);
        let receipt = AuthorityReceipt {
            seq,
            receipt_id: format!("nexus-receipt-{seq:08}"),
            receipt_type: receipt_type.into(),
            recorded_at_unix_ms,
            authority: AUTHORITY_NAME.to_string(),
            context,
        };
        self.receipts.push(receipt.clone());
        self.trim_retention();
        self.append_receipt_to_log(&receipt);
        receipt
    }

    pub fn snapshot(
        &self,
        runtime: &PublicRuntimeSnapshot,
        as_of_unix_ms: u64,
    ) -> PublicStatsSnapshot {
        let window_started_at_unix_ms = as_of_unix_ms.saturating_sub(PUBLIC_STATS_WINDOW_MS);
        let mut sessions_issued_24h = 0u64;
        let mut sync_tokens_issued_24h = 0u64;
        let mut starter_offers_dispatched_24h = 0u64;
        let mut starter_offers_started_24h = 0u64;
        let mut starter_offer_heartbeats_24h = 0u64;
        let mut starter_offers_completed_24h = 0u64;
        let mut starter_offers_released_24h = 0u64;
        let mut starter_offers_expired_24h = 0u64;
        let mut starter_demand_ineligible_polls_24h = 0u64;
        let mut starter_demand_paid_sats_24h = 0u64;
        let mut starter_demand_released_sats_24h = 0u64;

        for receipt in &self.receipts {
            if receipt.recorded_at_unix_ms < window_started_at_unix_ms {
                continue;
            }
            match receipt.receipt_type.as_str() {
                "desktop_session.created" => {
                    sessions_issued_24h = sessions_issued_24h.saturating_add(1);
                }
                "sync_token.issued" => {
                    sync_tokens_issued_24h = sync_tokens_issued_24h.saturating_add(1);
                }
                "starter_demand.ineligible" => {
                    starter_demand_ineligible_polls_24h =
                        starter_demand_ineligible_polls_24h.saturating_add(1);
                }
                "starter_offer.dispatched" => {
                    starter_offers_dispatched_24h = starter_offers_dispatched_24h.saturating_add(1);
                }
                "starter_offer.started" => {
                    starter_offers_started_24h = starter_offers_started_24h.saturating_add(1);
                }
                "starter_offer.heartbeat" => {
                    starter_offer_heartbeats_24h = starter_offer_heartbeats_24h.saturating_add(1);
                }
                "starter_offer.completed" => {
                    starter_offers_completed_24h = starter_offers_completed_24h.saturating_add(1);
                    starter_demand_paid_sats_24h = starter_demand_paid_sats_24h
                        .saturating_add(receipt.context.amount_sats.unwrap_or(0));
                }
                "starter_offer.released" => {
                    starter_offers_released_24h = starter_offers_released_24h.saturating_add(1);
                    starter_demand_released_sats_24h = starter_demand_released_sats_24h
                        .saturating_add(receipt.context.amount_sats.unwrap_or(0));
                }
                "starter_offer.expired" => {
                    starter_offers_expired_24h = starter_offers_expired_24h.saturating_add(1);
                    starter_demand_released_sats_24h = starter_demand_released_sats_24h
                        .saturating_add(receipt.context.amount_sats.unwrap_or(0));
                }
                _ => {}
            }
        }

        let starter_offer_start_rate_24h =
            ratio(starter_offers_started_24h, starter_offers_dispatched_24h);
        let starter_offer_completion_rate_24h =
            ratio(starter_offers_completed_24h, starter_offers_started_24h);
        let starter_offer_loss_rate_24h = ratio(
            starter_offers_released_24h.saturating_add(starter_offers_expired_24h),
            starter_offers_dispatched_24h,
        );

        PublicStatsSnapshot {
            service: "nexus-control".to_string(),
            authority: AUTHORITY_NAME.to_string(),
            hosted_nexus_relay_url: runtime.hosted_nexus_relay_url.clone(),
            as_of_unix_ms,
            window_started_at_unix_ms,
            receipt_count: self.receipts.len(),
            receipt_persistence_enabled: self.receipt_log_path.is_some(),
            receipt_persistence_error: self.last_persistence_error.clone(),
            sessions_active: runtime.sessions_active,
            sessions_issued_24h,
            sync_tokens_active: runtime.sync_tokens_active,
            sync_tokens_issued_24h,
            starter_demand_budget_cap_sats: runtime.starter_demand_budget_cap_sats,
            starter_demand_budget_allocated_sats: runtime.starter_demand_budget_allocated_sats,
            starter_offers_waiting_ack: runtime.starter_offers_waiting_ack,
            starter_offers_running: runtime.starter_offers_running,
            starter_offers_dispatched_24h,
            starter_offers_started_24h,
            starter_offer_heartbeats_24h,
            starter_offers_completed_24h,
            starter_offers_released_24h,
            starter_offers_expired_24h,
            starter_demand_ineligible_polls_24h,
            starter_offer_start_rate_24h,
            starter_offer_completion_rate_24h,
            starter_offer_loss_rate_24h,
            starter_demand_paid_sats_24h,
            starter_demand_released_sats_24h,
            compute_products_active: runtime.compute_products_active,
            compute_capacity_lots_open: runtime.compute_capacity_lots_open,
            compute_capacity_lots_delivering: runtime.compute_capacity_lots_delivering,
            compute_instruments_active: runtime.compute_instruments_active,
            compute_inventory_quantity_open: runtime.compute_inventory_quantity_open,
            compute_inventory_quantity_reserved: runtime.compute_inventory_quantity_reserved,
            compute_inventory_quantity_delivering: runtime.compute_inventory_quantity_delivering,
            compute_delivery_proofs_24h: runtime.compute_delivery_proofs_24h,
            compute_delivery_quantity_24h: runtime.compute_delivery_quantity_24h,
            compute_delivery_rejections_24h: runtime.compute_delivery_rejections_24h,
            compute_delivery_variances_24h: runtime.compute_delivery_variances_24h,
            compute_validator_challenges_open: runtime.compute_validator_challenges_open,
            compute_validator_challenges_queued: runtime.compute_validator_challenges_queued,
            compute_validator_challenges_verified_24h: runtime
                .compute_validator_challenges_verified_24h,
            compute_validator_challenges_rejected_24h: runtime
                .compute_validator_challenges_rejected_24h,
            compute_validator_challenges_timed_out_24h: runtime
                .compute_validator_challenges_timed_out_24h,
            compute_delivery_accept_rate_24h: runtime.compute_delivery_accept_rate_24h,
            compute_fill_ratio_24h: runtime.compute_fill_ratio_24h,
            compute_priced_instruments_24h: runtime.compute_priced_instruments_24h,
            compute_indices_published_24h: runtime.compute_indices_published_24h,
            compute_index_corrections_24h: runtime.compute_index_corrections_24h,
            compute_index_thin_windows_24h: runtime.compute_index_thin_windows_24h,
            compute_index_settlement_eligible_24h: runtime.compute_index_settlement_eligible_24h,
            compute_index_quality_score_24h: runtime.compute_index_quality_score_24h,
            compute_active_provider_count: runtime.compute_active_provider_count,
            compute_provider_concentration_hhi: runtime.compute_provider_concentration_hhi,
            compute_forward_physical_instruments_active: runtime
                .compute_forward_physical_instruments_active,
            compute_forward_physical_open_quantity: runtime.compute_forward_physical_open_quantity,
            compute_forward_physical_defaults_24h: runtime.compute_forward_physical_defaults_24h,
            compute_future_cash_instruments_active: runtime.compute_future_cash_instruments_active,
            compute_future_cash_open_interest: runtime.compute_future_cash_open_interest,
            compute_future_cash_cash_settlements_24h: runtime
                .compute_future_cash_cash_settlements_24h,
            compute_future_cash_cash_flow_24h: runtime.compute_future_cash_cash_flow_24h,
            compute_future_cash_defaults_24h: runtime.compute_future_cash_defaults_24h,
            compute_future_cash_collateral_shortfall_24h: runtime
                .compute_future_cash_collateral_shortfall_24h,
            compute_structured_instruments_active: runtime.compute_structured_instruments_active,
            compute_structured_instruments_closed_24h: runtime
                .compute_structured_instruments_closed_24h,
            compute_max_buyer_concentration_share: runtime.compute_max_buyer_concentration_share,
            compute_paper_to_physical_ratio: runtime.compute_paper_to_physical_ratio,
            compute_deliverable_coverage_ratio: runtime.compute_deliverable_coverage_ratio,
            compute_breakers_tripped: runtime.compute_breakers_tripped,
            compute_breakers_guarded: runtime.compute_breakers_guarded,
            compute_breaker_states: runtime.compute_breaker_states.clone(),
            compute_rollout_gates: runtime.compute_rollout_gates.clone(),
            compute_truth_labels: runtime.compute_truth_labels.clone(),
            compute_reconciliation_gap_24h: runtime.compute_reconciliation_gap_24h,
            compute_policy_bundle_id: runtime.compute_policy_bundle_id.clone(),
            compute_policy_version: runtime.compute_policy_version.clone(),
            liquidity_quotes_active: runtime.liquidity_quotes_active,
            liquidity_route_plans_active: runtime.liquidity_route_plans_active,
            liquidity_envelopes_open: runtime.liquidity_envelopes_open,
            liquidity_settlements_24h: runtime.liquidity_settlements_24h,
            liquidity_reserve_partitions_active: runtime.liquidity_reserve_partitions_active,
            liquidity_value_moved_24h: runtime.liquidity_value_moved_24h,
            risk_coverage_offers_open: runtime.risk_coverage_offers_open,
            risk_coverage_bindings_active: runtime.risk_coverage_bindings_active,
            risk_prediction_positions_open: runtime.risk_prediction_positions_open,
            risk_claims_open: runtime.risk_claims_open,
            risk_signals_active: runtime.risk_signals_active,
            risk_implied_fail_probability_bps: runtime.risk_implied_fail_probability_bps,
            risk_calibration_score: runtime.risk_calibration_score,
            risk_coverage_concentration_hhi: runtime.risk_coverage_concentration_hhi,
            recent_receipts: self.recent_receipts(),
        }
    }

    fn recent_receipts(&self) -> Vec<PublicRecentReceipt> {
        self.receipts
            .iter()
            .rev()
            .take(PUBLIC_RECENT_RECEIPT_LIMIT)
            .map(|receipt| PublicRecentReceipt {
                receipt_id: receipt.receipt_id.clone(),
                receipt_type: receipt.receipt_type.clone(),
                recorded_at_unix_ms: receipt.recorded_at_unix_ms,
                request_id: receipt.context.request_id.clone(),
                status: receipt.context.status.clone(),
                reason: receipt.context.reason.clone(),
                amount_sats: receipt.context.amount_sats,
            })
            .collect()
    }

    fn trim_retention(&mut self) {
        if self.receipts.len() > RECEIPT_RETENTION_LIMIT {
            let remove_count = self.receipts.len().saturating_sub(RECEIPT_RETENTION_LIMIT);
            self.receipts.drain(0..remove_count);
        }
    }

    fn load_existing_receipts(&mut self) {
        let Some(path) = self.receipt_log_path.clone() else {
            return;
        };
        if !path.exists() {
            return;
        }
        let Ok(contents) = fs::read_to_string(path.as_path()) else {
            self.last_persistence_error =
                Some(format!("failed_to_read_receipt_log:{}", path.display()));
            return;
        };
        for (index, line) in contents.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<AuthorityReceipt>(line) {
                Ok(receipt) => {
                    self.next_receipt_seq =
                        self.next_receipt_seq.max(receipt.seq.saturating_add(1));
                    self.receipts.push(receipt);
                }
                Err(error) => {
                    self.last_persistence_error = Some(format!(
                        "failed_to_parse_receipt_log_line:{}:{}",
                        index.saturating_add(1),
                        error
                    ));
                }
            }
        }
        self.trim_retention();
    }

    fn append_receipt_to_log(&mut self, receipt: &AuthorityReceipt) {
        let Some(path) = self.receipt_log_path.clone() else {
            return;
        };
        if let Some(parent) = parent_directory(path.as_path())
            && let Err(error) = fs::create_dir_all(parent)
        {
            self.last_persistence_error = Some(format!(
                "failed_to_create_receipt_log_parent:{}:{}",
                parent.display(),
                error
            ));
            return;
        }
        let file_result = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path.as_path());
        let mut file = match file_result {
            Ok(file) => file,
            Err(error) => {
                self.last_persistence_error = Some(format!(
                    "failed_to_open_receipt_log:{}:{}",
                    path.display(),
                    error
                ));
                return;
            }
        };
        match serde_json::to_vec(receipt) {
            Ok(serialized) => {
                if let Err(error) = file.write_all(serialized.as_slice()) {
                    self.last_persistence_error = Some(format!(
                        "failed_to_append_receipt_log:{}:{}",
                        path.display(),
                        error
                    ));
                    return;
                }
                if let Err(error) = file.write_all(b"\n") {
                    self.last_persistence_error = Some(format!(
                        "failed_to_terminate_receipt_log_line:{}:{}",
                        path.display(),
                        error
                    ));
                    return;
                }
                self.last_persistence_error = None;
            }
            Err(error) => {
                self.last_persistence_error = Some(format!(
                    "failed_to_serialize_receipt:{}:{error}",
                    receipt.receipt_id
                ));
            }
        }
    }
}

fn parent_directory(path: &Path) -> Option<&Path> {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
}

fn ratio(numerator: u64, denominator: u64) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

#[cfg(test)]
mod tests {
    use super::{AuthorityReceiptContext, PublicRuntimeSnapshot, ReceiptLedger};

    #[test]
    fn snapshot_aggregates_receipts_by_type() {
        let mut ledger = ReceiptLedger::new(None);
        ledger.record(
            "desktop_session.created",
            1_000,
            AuthorityReceiptContext::default(),
        );
        ledger.record(
            "sync_token.issued",
            2_000,
            AuthorityReceiptContext::default(),
        );
        ledger.record(
            "starter_offer.dispatched",
            3_000,
            AuthorityReceiptContext {
                amount_sats: Some(120),
                ..AuthorityReceiptContext::default()
            },
        );
        ledger.record(
            "starter_offer.started",
            4_000,
            AuthorityReceiptContext::default(),
        );
        ledger.record(
            "starter_offer.completed",
            5_000,
            AuthorityReceiptContext {
                amount_sats: Some(120),
                ..AuthorityReceiptContext::default()
            },
        );
        let snapshot = ledger.snapshot(
            &PublicRuntimeSnapshot {
                hosted_nexus_relay_url: "wss://nexus.openagents.com/".to_string(),
                sessions_active: 1,
                sync_tokens_active: 1,
                starter_demand_budget_cap_sats: 5_000,
                starter_demand_budget_allocated_sats: 0,
                starter_offers_waiting_ack: 0,
                starter_offers_running: 0,
                compute_products_active: 0,
                compute_capacity_lots_open: 0,
                compute_capacity_lots_delivering: 0,
                compute_instruments_active: 0,
                compute_inventory_quantity_open: 0,
                compute_inventory_quantity_reserved: 0,
                compute_inventory_quantity_delivering: 0,
                compute_delivery_proofs_24h: 0,
                compute_delivery_quantity_24h: 0,
                compute_delivery_rejections_24h: 0,
                compute_delivery_variances_24h: 0,
                compute_validator_challenges_open: 0,
                compute_validator_challenges_queued: 0,
                compute_validator_challenges_verified_24h: 0,
                compute_validator_challenges_rejected_24h: 0,
                compute_validator_challenges_timed_out_24h: 0,
                compute_delivery_accept_rate_24h: 0.0,
                compute_fill_ratio_24h: 0.0,
                compute_priced_instruments_24h: 0,
                compute_indices_published_24h: 0,
                compute_index_corrections_24h: 0,
                compute_index_thin_windows_24h: 0,
                compute_index_settlement_eligible_24h: 0,
                compute_index_quality_score_24h: 0.0,
                compute_active_provider_count: 0,
                compute_provider_concentration_hhi: 0.0,
                compute_forward_physical_instruments_active: 0,
                compute_forward_physical_open_quantity: 0,
                compute_forward_physical_defaults_24h: 0,
                compute_future_cash_instruments_active: 0,
                compute_future_cash_open_interest: 0,
                compute_future_cash_cash_settlements_24h: 0,
                compute_future_cash_cash_flow_24h: 0,
                compute_future_cash_defaults_24h: 0,
                compute_future_cash_collateral_shortfall_24h: 0,
                compute_structured_instruments_active: 0,
                compute_structured_instruments_closed_24h: 0,
                compute_max_buyer_concentration_share: 0.0,
                compute_paper_to_physical_ratio: 0.0,
                compute_deliverable_coverage_ratio: 0.0,
                compute_breakers_tripped: 0,
                compute_breakers_guarded: 0,
                compute_breaker_states: Vec::new(),
                compute_rollout_gates: Vec::new(),
                compute_truth_labels: Vec::new(),
                compute_reconciliation_gap_24h: 0,
                compute_policy_bundle_id: String::new(),
                compute_policy_version: String::new(),
                liquidity_quotes_active: 0,
                liquidity_route_plans_active: 0,
                liquidity_envelopes_open: 0,
                liquidity_settlements_24h: 0,
                liquidity_reserve_partitions_active: 0,
                liquidity_value_moved_24h: 0,
                risk_coverage_offers_open: 0,
                risk_coverage_bindings_active: 0,
                risk_prediction_positions_open: 0,
                risk_claims_open: 0,
                risk_signals_active: 0,
                risk_implied_fail_probability_bps: 0,
                risk_calibration_score: 0.0,
                risk_coverage_concentration_hhi: 0.0,
            },
            6_000,
        );
        assert_eq!(snapshot.sessions_issued_24h, 1);
        assert_eq!(snapshot.sync_tokens_issued_24h, 1);
        assert_eq!(snapshot.starter_offers_dispatched_24h, 1);
        assert_eq!(snapshot.starter_offers_started_24h, 1);
        assert_eq!(snapshot.starter_offers_completed_24h, 1);
        assert_eq!(snapshot.starter_demand_paid_sats_24h, 120);
        assert_eq!(snapshot.starter_offer_start_rate_24h, 1.0);
        assert_eq!(snapshot.starter_offer_completion_rate_24h, 1.0);
    }
}
