//! Wallet reconciliation for autonomous goal progress.

use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use crate::app_state::{JobHistoryState, JobHistoryStatus};
use crate::spark_wallet::SparkPaneState;
use crate::state::swap_contract::{
    GoalSwapExecutionReceipt, SwapAmountUnit, SwapDirection, SwapExecutionStatus,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WalletLedgerEventKind {
    EarnPayout,
    SwapConversionDebit,
    SwapConversionCredit,
    SwapFeeDebit,
    WalletSpend,
    WalletReceiveUnattributed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WalletLedgerEvent {
    pub event_id: String,
    pub occurred_at_epoch_seconds: u64,
    pub kind: WalletLedgerEventKind,
    pub sats_delta: i64,
    pub cents_delta: i64,
    pub job_id: Option<String>,
    pub payment_pointer: Option<String>,
    pub quote_id: Option<String>,
    pub transaction_id: Option<String>,
    pub note: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WalletReconciliationReport {
    pub wallet_delta_sats_raw: i64,
    pub wallet_delta_excluding_swaps_sats: i64,
    pub earned_wallet_delta_sats: i64,
    pub swap_converted_out_sats: u64,
    pub swap_converted_in_sats: u64,
    pub swap_fee_sats: u64,
    pub non_swap_spend_sats: u64,
    pub unattributed_receive_sats: u64,
    pub total_swap_cents: u64,
    pub events: Vec<WalletLedgerEvent>,
}

pub fn reconcile_wallet_events_for_goal(
    started_at_epoch_seconds: u64,
    initial_wallet_sats: u64,
    current_wallet_sats: u64,
    goal_id: &str,
    job_history: &JobHistoryState,
    spark_wallet: &SparkPaneState,
    swap_execution_receipts: &[GoalSwapExecutionReceipt],
) -> WalletReconciliationReport {
    let wallet_delta_sats_raw = current_wallet_sats as i64 - initial_wallet_sats as i64;
    let mut events = Vec::<WalletLedgerEvent>::new();
    let mut matched_payment_ids = BTreeSet::<String>::new();

    let wallet_payments = spark_wallet
        .recent_payments
        .iter()
        .filter(|payment| {
            payment.status.eq_ignore_ascii_case("succeeded")
                && payment.timestamp >= started_at_epoch_seconds
        })
        .collect::<Vec<_>>();
    let payment_by_id = wallet_payments
        .iter()
        .map(|payment| (payment.id.as_str(), *payment))
        .collect::<HashMap<_, _>>();

    let mut earned_wallet_delta_sats = 0i64;
    for row in job_history.rows.iter().filter(|row| {
        row.status == JobHistoryStatus::Succeeded
            && row.completed_at_epoch_seconds >= started_at_epoch_seconds
            && !row.payment_pointer.trim().is_empty()
            && !is_synthetic_payment_pointer(row.payment_pointer.as_str())
    }) {
        let Some(payment) = payment_by_id.get(row.payment_pointer.as_str()) else {
            continue;
        };
        if !payment.direction.eq_ignore_ascii_case("receive") {
            continue;
        }
        let payout_sats = payment.amount_sats;
        earned_wallet_delta_sats = earned_wallet_delta_sats.saturating_add(payout_sats as i64);
        matched_payment_ids.insert(payment.id.clone());
        events.push(WalletLedgerEvent {
            event_id: format!("earn:{}:{}", row.job_id, payment.id),
            occurred_at_epoch_seconds: payment.timestamp,
            kind: WalletLedgerEventKind::EarnPayout,
            sats_delta: payout_sats as i64,
            cents_delta: 0,
            job_id: Some(row.job_id.clone()),
            payment_pointer: Some(payment.id.clone()),
            quote_id: None,
            transaction_id: None,
            note: if row.payout_sats != payout_sats {
                Some(format!(
                    "history_payout_sats={} wallet_payout_sats={}",
                    row.payout_sats, payout_sats
                ))
            } else {
                None
            },
        });
    }

    let mut swap_converted_out_sats = 0u64;
    let mut swap_converted_in_sats = 0u64;
    let mut swap_fee_sats = 0u64;
    let mut total_swap_cents = 0u64;

    for receipt in swap_execution_receipts.iter().filter(|receipt| {
        receipt.goal_id == goal_id
            && receipt.finished_at_epoch_seconds >= started_at_epoch_seconds
            && matches!(
                receipt.status,
                SwapExecutionStatus::Success | SwapExecutionStatus::AlreadyPaid
            )
    }) {
        let swap_sats_out = swap_sats_out(receipt);
        let swap_sats_in = swap_sats_in(receipt);
        let swap_cents = swap_cents_volume(receipt);
        swap_converted_out_sats = swap_converted_out_sats.saturating_add(swap_sats_out);
        swap_converted_in_sats = swap_converted_in_sats.saturating_add(swap_sats_in);
        swap_fee_sats = swap_fee_sats.saturating_add(receipt.fee_sats);
        total_swap_cents = total_swap_cents.saturating_add(swap_cents);

        if let Some(transaction_id) = receipt
            .transaction_id
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            matched_payment_ids.insert(transaction_id.to_string());
        }

        if swap_sats_out > 0 {
            events.push(WalletLedgerEvent {
                event_id: format!("swap:{}:debit", receipt.receipt_id),
                occurred_at_epoch_seconds: receipt.finished_at_epoch_seconds,
                kind: WalletLedgerEventKind::SwapConversionDebit,
                sats_delta: -(swap_sats_out as i64),
                cents_delta: if receipt.direction == SwapDirection::BtcToUsd {
                    swap_cents as i64
                } else {
                    -(swap_cents as i64)
                },
                job_id: None,
                payment_pointer: None,
                quote_id: Some(receipt.quote_id.clone()),
                transaction_id: receipt.transaction_id.clone(),
                note: None,
            });
        }
        if swap_sats_in > 0 {
            events.push(WalletLedgerEvent {
                event_id: format!("swap:{}:credit", receipt.receipt_id),
                occurred_at_epoch_seconds: receipt.finished_at_epoch_seconds,
                kind: WalletLedgerEventKind::SwapConversionCredit,
                sats_delta: swap_sats_in as i64,
                cents_delta: if receipt.direction == SwapDirection::UsdToBtc {
                    -(swap_cents as i64)
                } else {
                    swap_cents as i64
                },
                job_id: None,
                payment_pointer: None,
                quote_id: Some(receipt.quote_id.clone()),
                transaction_id: receipt.transaction_id.clone(),
                note: None,
            });
        }
        if receipt.fee_sats > 0 {
            events.push(WalletLedgerEvent {
                event_id: format!("swap:{}:fee", receipt.receipt_id),
                occurred_at_epoch_seconds: receipt.finished_at_epoch_seconds,
                kind: WalletLedgerEventKind::SwapFeeDebit,
                sats_delta: -(receipt.fee_sats as i64),
                cents_delta: 0,
                job_id: None,
                payment_pointer: None,
                quote_id: Some(receipt.quote_id.clone()),
                transaction_id: receipt.transaction_id.clone(),
                note: None,
            });
        }
    }

    let mut non_swap_spend_sats = 0u64;
    let mut unattributed_receive_sats = 0u64;
    for payment in wallet_payments {
        if matched_payment_ids.contains(payment.id.as_str()) {
            continue;
        }
        if payment.direction.eq_ignore_ascii_case("send") {
            let total_debit_sats = crate::spark_wallet::wallet_payment_total_debit_sats(payment);
            non_swap_spend_sats = non_swap_spend_sats.saturating_add(total_debit_sats);
            events.push(WalletLedgerEvent {
                event_id: format!("wallet:send:{}", payment.id),
                occurred_at_epoch_seconds: payment.timestamp,
                kind: WalletLedgerEventKind::WalletSpend,
                sats_delta: -(total_debit_sats as i64),
                cents_delta: 0,
                job_id: None,
                payment_pointer: Some(payment.id.clone()),
                quote_id: None,
                transaction_id: None,
                note: (payment.fees_sats > 0).then(|| {
                    format!(
                        "amount_sats={} fees_sats={} total_debit_sats={}",
                        payment.amount_sats, payment.fees_sats, total_debit_sats
                    )
                }),
            });
        } else if payment.direction.eq_ignore_ascii_case("receive") {
            unattributed_receive_sats =
                unattributed_receive_sats.saturating_add(payment.amount_sats);
            events.push(WalletLedgerEvent {
                event_id: format!("wallet:receive:{}", payment.id),
                occurred_at_epoch_seconds: payment.timestamp,
                kind: WalletLedgerEventKind::WalletReceiveUnattributed,
                sats_delta: payment.amount_sats as i64,
                cents_delta: 0,
                job_id: None,
                payment_pointer: Some(payment.id.clone()),
                quote_id: None,
                transaction_id: None,
                note: Some("receive event not attributed to job payout".to_string()),
            });
        }
    }

    events.sort_by(|left, right| {
        left.occurred_at_epoch_seconds
            .cmp(&right.occurred_at_epoch_seconds)
            .then_with(|| left.event_id.cmp(&right.event_id))
    });

    let swap_delta_sats =
        swap_converted_in_sats as i64 - swap_converted_out_sats as i64 - swap_fee_sats as i64;
    let wallet_delta_excluding_swaps_sats = wallet_delta_sats_raw.saturating_sub(swap_delta_sats);

    WalletReconciliationReport {
        wallet_delta_sats_raw,
        wallet_delta_excluding_swaps_sats,
        earned_wallet_delta_sats,
        swap_converted_out_sats,
        swap_converted_in_sats,
        swap_fee_sats,
        non_swap_spend_sats,
        unattributed_receive_sats,
        total_swap_cents,
        events,
    }
}

fn swap_sats_out(receipt: &GoalSwapExecutionReceipt) -> u64 {
    match receipt.direction {
        SwapDirection::BtcToUsd => {
            if receipt.amount_in.unit == SwapAmountUnit::Sats {
                receipt.amount_in.amount
            } else if receipt.amount_out.unit == SwapAmountUnit::Sats {
                receipt.amount_out.amount
            } else {
                0
            }
        }
        SwapDirection::UsdToBtc => 0,
    }
}

fn swap_sats_in(receipt: &GoalSwapExecutionReceipt) -> u64 {
    match receipt.direction {
        SwapDirection::BtcToUsd => 0,
        SwapDirection::UsdToBtc => {
            if receipt.amount_out.unit == SwapAmountUnit::Sats {
                receipt.amount_out.amount
            } else if receipt.amount_in.unit == SwapAmountUnit::Sats {
                receipt.amount_in.amount
            } else {
                0
            }
        }
    }
}

fn swap_cents_volume(receipt: &GoalSwapExecutionReceipt) -> u64 {
    match receipt.direction {
        SwapDirection::BtcToUsd => {
            if receipt.amount_out.unit == SwapAmountUnit::Cents {
                receipt.amount_out.amount
            } else if receipt.amount_in.unit == SwapAmountUnit::Cents {
                receipt.amount_in.amount
            } else {
                0
            }
        }
        SwapDirection::UsdToBtc => {
            if receipt.amount_in.unit == SwapAmountUnit::Cents {
                receipt.amount_in.amount
            } else if receipt.amount_out.unit == SwapAmountUnit::Cents {
                receipt.amount_out.amount
            } else {
                0
            }
        }
    }
}

fn is_synthetic_payment_pointer(pointer: &str) -> bool {
    let normalized = pointer.trim().to_ascii_lowercase();
    normalized.starts_with("pending:")
        || normalized.starts_with("pay:")
        || normalized.starts_with("pay-req-")
        || normalized.starts_with("inv-")
}

#[cfg(test)]
mod tests {
    use super::reconcile_wallet_events_for_goal;
    use crate::app_state::{
        JobHistoryReceiptRow, JobHistoryState, JobHistoryStatus, JobHistoryStatusFilter,
        JobHistoryTimeRange,
    };
    use crate::spark_wallet::SparkPaneState;
    use crate::state::swap_contract::{
        GoalSwapExecutionReceipt, SwapAmount, SwapAmountUnit, SwapDirection, SwapExecutionStatus,
    };
    use openagents_spark::PaymentSummary;

    fn sample_job_history(rows: Vec<JobHistoryReceiptRow>) -> JobHistoryState {
        JobHistoryState {
            load_state: crate::app_state::PaneLoadState::Ready,
            last_error: None,
            last_action: None,
            rows,
            status_filter: JobHistoryStatusFilter::All,
            time_range: JobHistoryTimeRange::All,
            page: 0,
            page_size: 10,
            search_job_id: String::new(),
            reference_epoch_seconds: 1_760_000_000,
        }
    }

    #[test]
    fn reconciliation_distinguishes_earn_vs_swap_and_fee() {
        let history = sample_job_history(vec![JobHistoryReceiptRow {
            job_id: "job-1".to_string(),
            status: JobHistoryStatus::Succeeded,
            demand_source: crate::app_state::JobDemandSource::OpenNetwork,
            completed_at_epoch_seconds: 1_700_000_100,
            requester_nostr_pubkey: Some("npub1buyer".to_string()),
            provider_nostr_pubkey: Some("npub1provider".to_string()),
            skill_scope_id: None,
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_result_event_id: None,
            sa_trajectory_session_id: None,
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
            payout_sats: 1_500,
            result_hash: "sha256:job-1".to_string(),
            payment_pointer: "wallet:pay:job-1".to_string(),
            failure_reason: None,
            execution_provenance: None,
        }]);

        let mut wallet = SparkPaneState::default();
        wallet.recent_payments = vec![
            PaymentSummary {
                id: "wallet:pay:job-1".to_string(),
                direction: "receive".to_string(),
                status: "succeeded".to_string(),
                amount_sats: 1_500,
                timestamp: 1_700_000_100,
                ..Default::default()
            },
            PaymentSummary {
                id: "wallet:spend:1".to_string(),
                direction: "send".to_string(),
                status: "succeeded".to_string(),
                amount_sats: 200,
                fees_sats: 7,
                timestamp: 1_700_000_120,
                ..Default::default()
            },
        ];

        let swaps = vec![
            GoalSwapExecutionReceipt {
                receipt_id: "swap-1".to_string(),
                goal_id: "goal-1".to_string(),
                quote_id: "quote-1".to_string(),
                direction: SwapDirection::BtcToUsd,
                amount_in: SwapAmount {
                    amount: 1_000,
                    unit: SwapAmountUnit::Sats,
                },
                amount_out: SwapAmount {
                    amount: 760,
                    unit: SwapAmountUnit::Cents,
                },
                fee_sats: 30,
                status: SwapExecutionStatus::Success,
                transaction_id: Some("swap-tx-1".to_string()),
                failure_reason: None,
                started_at_epoch_seconds: 1_700_000_110,
                finished_at_epoch_seconds: 1_700_000_111,
                command_provenance: None,
            },
            GoalSwapExecutionReceipt {
                receipt_id: "swap-2".to_string(),
                goal_id: "goal-1".to_string(),
                quote_id: "quote-2".to_string(),
                direction: SwapDirection::UsdToBtc,
                amount_in: SwapAmount {
                    amount: 500,
                    unit: SwapAmountUnit::Cents,
                },
                amount_out: SwapAmount {
                    amount: 640,
                    unit: SwapAmountUnit::Sats,
                },
                fee_sats: 10,
                status: SwapExecutionStatus::AlreadyPaid,
                transaction_id: None,
                failure_reason: None,
                started_at_epoch_seconds: 1_700_000_130,
                finished_at_epoch_seconds: 1_700_000_131,
                command_provenance: None,
            },
        ];

        let report = reconcile_wallet_events_for_goal(
            1_700_000_000,
            10_000,
            10_900,
            "goal-1",
            &history,
            &wallet,
            &swaps,
        );
        assert_eq!(report.earned_wallet_delta_sats, 1_500);
        assert_eq!(report.swap_converted_out_sats, 1_000);
        assert_eq!(report.swap_converted_in_sats, 640);
        assert_eq!(report.swap_fee_sats, 40);
        assert_eq!(report.non_swap_spend_sats, 207);
        assert_eq!(report.total_swap_cents, 1_260);
        assert!(
            report
                .events
                .iter()
                .any(|event| matches!(event.kind, super::WalletLedgerEventKind::EarnPayout))
        );
        assert!(
            report
                .events
                .iter()
                .any(|event| matches!(event.kind, super::WalletLedgerEventKind::SwapFeeDebit))
        );
        assert!(report.events.iter().any(|event| {
            matches!(event.kind, super::WalletLedgerEventKind::WalletSpend)
                && event.sats_delta == -207
                && event
                    .note
                    .as_deref()
                    .is_some_and(|note| note.contains("fees_sats=7"))
        }));
    }
}
