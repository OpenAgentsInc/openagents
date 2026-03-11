//! Authoritative earnings verification gate.
//!
//! Goal success is accepted only when Spark wallet payment evidence confirms funds.

use std::collections::HashSet;

use crate::app_state::{JobHistoryState, JobHistoryStatus};
use crate::spark_wallet::SparkPaneState;
use crate::state::autopilot_goals::GoalRecord;
use crate::state::goal_conditions::{
    ConditionEvaluation, GoalProgressSnapshot, evaluate_conditions,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EarningsVerificationReport {
    pub authoritative_evaluation: ConditionEvaluation,
    pub authoritative_goal_complete: bool,
    pub authoritative_wallet_delta_sats: i64,
    pub verified_receipt_count: u32,
    pub mismatches: Vec<String>,
}

pub fn verify_authoritative_earnings(
    goal: &GoalRecord,
    progress: &GoalProgressSnapshot,
    job_history: &JobHistoryState,
    spark_wallet: &SparkPaneState,
) -> EarningsVerificationReport {
    let mut mismatches = Vec::<String>::new();
    if let Some(error) = spark_wallet.last_error.as_deref() {
        mismatches.push(format!("wallet source error: {error}"));
    }
    if spark_wallet.balance.is_none() {
        mismatches.push("wallet balance unavailable".to_string());
    }

    let wallet_payments = settled_receive_payments(spark_wallet);
    let wallet_payment_ids = wallet_payments
        .iter()
        .map(|payment| payment.id.as_str())
        .collect::<HashSet<_>>();
    let wallet_receive_sats = wallet_payments
        .iter()
        .map(|payment| payment.amount_sats)
        .sum::<u64>();

    let mut verified_receipt_count = 0u32;
    for row in &job_history.rows {
        if row.status != JobHistoryStatus::Succeeded || row.payout_sats == 0 {
            continue;
        }
        if is_synthetic_payment_pointer(row.payment_pointer.as_str()) {
            mismatches.push(format!(
                "synthetic payout pointer for {}: {}",
                row.job_id, row.payment_pointer
            ));
            continue;
        }
        if !wallet_payment_ids.contains(row.payment_pointer.as_str()) {
            mismatches.push(format!(
                "job {} payout pointer missing from wallet payments: {}",
                row.job_id, row.payment_pointer
            ));
            continue;
        }
        verified_receipt_count = verified_receipt_count.saturating_add(1);
    }

    let mut authoritative_progress = progress.clone();
    authoritative_progress.wallet_delta_sats = wallet_receive_sats as i64;
    authoritative_progress.earned_wallet_delta_sats = wallet_receive_sats as i64;
    authoritative_progress.jobs_completed = verified_receipt_count;
    authoritative_progress.successes = verified_receipt_count;

    let authoritative_evaluation = evaluate_conditions(goal, &authoritative_progress);
    let authoritative_goal_complete =
        authoritative_evaluation.goal_complete && mismatches.is_empty();

    EarningsVerificationReport {
        authoritative_evaluation,
        authoritative_goal_complete,
        authoritative_wallet_delta_sats: wallet_receive_sats as i64,
        verified_receipt_count,
        mismatches,
    }
}

fn is_synthetic_payment_pointer(pointer: &str) -> bool {
    pointer.trim().starts_with("pay:")
}

fn settled_receive_payments(
    spark_wallet: &SparkPaneState,
) -> Vec<openagents_spark::PaymentSummary> {
    spark_wallet
        .recent_payments
        .iter()
        .filter(|payment| {
            payment.direction.eq_ignore_ascii_case("receive")
                && is_settled_status(payment.status.as_str())
        })
        .cloned()
        .collect()
}

fn is_settled_status(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "succeeded" | "success" | "settled" | "completed" | "confirmed"
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use openagents_spark::{Balance, PaymentSummary};

    use crate::app_state::{
        JobHistoryReceiptRow, JobHistoryState, JobHistoryStatus, JobHistoryStatusFilter,
        JobHistoryTimeRange,
    };
    use crate::spark_wallet::SparkPaneState;
    use crate::state::autopilot_goals::{
        GoalConstraints, GoalLifecycleStatus, GoalObjective, GoalRecord, GoalRetryPolicy,
        GoalScheduleConfig, GoalStopCondition,
    };
    use crate::state::goal_conditions::GoalProgressSnapshot;

    use super::verify_authoritative_earnings;

    fn sample_goal() -> GoalRecord {
        GoalRecord {
            goal_id: "goal-auth".to_string(),
            title: "Earn +1000 sats".to_string(),
            objective: GoalObjective::EarnBitcoin {
                min_wallet_delta_sats: 1_000,
                note: None,
            },
            constraints: GoalConstraints::default(),
            stop_conditions: vec![GoalStopCondition::WalletDeltaSatsAtLeast { sats: 1_000 }],
            retry_policy: GoalRetryPolicy::default(),
            schedule: GoalScheduleConfig::default(),
            lifecycle_status: GoalLifecycleStatus::Queued,
            created_at_epoch_seconds: 10,
            updated_at_epoch_seconds: 10,
            attempt_count: 0,
            last_failure_reason: None,
            terminal_reason: None,
            last_receipt_id: None,
            recovery_replay_pending: false,
        }
    }

    fn sample_progress() -> GoalProgressSnapshot {
        GoalProgressSnapshot {
            started_at_epoch_seconds: 10,
            now_epoch_seconds: 30,
            attempt_count: 1,
            wallet_delta_sats: 1_000,
            earned_wallet_delta_sats: 1_000,
            jobs_completed: 1,
            successes: 1,
            errors: 0,
            total_spend_sats: 0,
            total_swap_cents: 0,
            external_signals: BTreeMap::new(),
        }
    }

    fn sample_history_with_payout(pointer: &str, payout_sats: u64) -> JobHistoryState {
        JobHistoryState {
            load_state: crate::app_state::PaneLoadState::Ready,
            last_error: None,
            last_action: None,
            rows: vec![JobHistoryReceiptRow {
                job_id: "job-1".to_string(),
                status: JobHistoryStatus::Succeeded,
                demand_source: crate::app_state::JobDemandSource::OpenNetwork,
                completed_at_epoch_seconds: 20,
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
                payout_sats,
                result_hash: "hash".to_string(),
                payment_pointer: pointer.to_string(),
                failure_reason: None,
                execution_provenance: None,
            }],
            status_filter: JobHistoryStatusFilter::All,
            time_range: JobHistoryTimeRange::All,
            page: 0,
            page_size: 6,
            search_job_id: String::new(),
            reference_epoch_seconds: 30,
        }
    }

    fn sample_history(pointer: &str) -> JobHistoryState {
        sample_history_with_payout(pointer, 1_000)
    }

    fn sample_wallet_with_receive(
        payment_id: &str,
        status: &str,
        amount_sats: u64,
    ) -> SparkPaneState {
        let mut wallet = SparkPaneState::default();
        wallet.balance = Some(Balance {
            spark_sats: 10_000,
            lightning_sats: 0,
            onchain_sats: 0,
        });
        wallet.recent_payments.push(PaymentSummary {
            id: payment_id.to_string(),
            direction: "receive".to_string(),
            status: status.to_string(),
            amount_sats,
            timestamp: 25,
            ..Default::default()
        });
        wallet
    }

    fn sample_wallet(payment_id: &str, status: &str) -> SparkPaneState {
        sample_wallet_with_receive(payment_id, status, 1_000)
    }

    #[test]
    fn rejects_synthetic_payout_pointers() {
        let report = verify_authoritative_earnings(
            &sample_goal(),
            &sample_progress(),
            &sample_history("pay:starter-job-1"),
            &sample_wallet("real-wallet-payment", "succeeded"),
        );
        assert!(!report.authoritative_goal_complete);
        assert!(
            report
                .mismatches
                .iter()
                .any(|mismatch| mismatch.contains("synthetic payout pointer"))
        );
    }

    #[test]
    fn requires_wallet_payment_pointer_match() {
        let report = verify_authoritative_earnings(
            &sample_goal(),
            &sample_progress(),
            &sample_history("payment-not-found"),
            &sample_wallet("real-wallet-payment", "succeeded"),
        );
        assert!(!report.authoritative_goal_complete);
        assert!(
            report
                .mismatches
                .iter()
                .any(|mismatch| mismatch.contains("missing from wallet payments"))
        );
    }

    #[test]
    fn accepts_wallet_backed_earnings_evidence() {
        let report = verify_authoritative_earnings(
            &sample_goal(),
            &sample_progress(),
            &sample_history("real-wallet-payment"),
            &sample_wallet("real-wallet-payment", "succeeded"),
        );
        assert!(report.authoritative_goal_complete);
        assert!(report.mismatches.is_empty());
    }

    #[test]
    fn earn_bitcoin_until_target_sats_requires_wallet_confirmed_threshold() {
        let goal = sample_goal();
        let mut progress = sample_progress();
        progress.wallet_delta_sats = 500;
        progress.earned_wallet_delta_sats = 500;
        progress.jobs_completed = 0;
        progress.successes = 0;

        let pending_report = verify_authoritative_earnings(
            &goal,
            &progress,
            &sample_history_with_payout("wallet-payment-pending", 500),
            &sample_wallet_with_receive("wallet-payment-pending", "succeeded", 500),
        );
        assert!(!pending_report.authoritative_goal_complete);
        assert_eq!(pending_report.authoritative_wallet_delta_sats, 500);
        assert_eq!(pending_report.verified_receipt_count, 1);

        let mut complete_progress = progress.clone();
        complete_progress.wallet_delta_sats = 1_000;
        complete_progress.earned_wallet_delta_sats = 1_000;
        complete_progress.jobs_completed = 1;
        complete_progress.successes = 1;
        let complete_report = verify_authoritative_earnings(
            &goal,
            &complete_progress,
            &sample_history_with_payout("wallet-payment-complete", 1_000),
            &sample_wallet_with_receive("wallet-payment-complete", "succeeded", 1_000),
        );
        assert!(complete_report.authoritative_goal_complete);
        assert_eq!(complete_report.authoritative_wallet_delta_sats, 1_000);
        assert!(
            complete_report
                .authoritative_evaluation
                .completion_reasons
                .iter()
                .any(|reason| reason.contains("wallet delta reached 1000 sats"))
        );
    }
}
