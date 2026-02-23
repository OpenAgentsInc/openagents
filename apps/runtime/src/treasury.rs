use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

/// Phase-0 treasury/settlement model.
///
/// This is intentionally minimal: it provides idempotent reserve -> release/withhold
/// semantics for compute jobs. It is an in-memory ledger suitable for local harnesses
/// and early staging, and will be replaced by durable authority paths as the
/// Phase-2 "authority baseline" issues land.
#[derive(Default)]
pub struct Treasury {
    inner: Mutex<TreasuryInner>,
}

#[derive(Default)]
struct TreasuryInner {
    accounts: HashMap<String, BudgetAccount>,
    compute_jobs: HashMap<String, ComputeJobSettlement>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BudgetAccount {
    /// Total spend budget (msats).
    pub limit_msats: u64,
    /// Total reserved but not settled (msats).
    pub reserved_msats: u64,
    /// Total spent (msats).
    pub spent_msats: u64,
    pub updated_at: DateTime<Utc>,
}

impl BudgetAccount {
    fn ensure_default(now: DateTime<Utc>) -> Self {
        Self {
            // Phase-0 default budget is intentionally high; enforcement will be wired to
            // control-plane budgets in later phases.
            limit_msats: 10_000_000_000,
            reserved_msats: 0,
            spent_msats: 0,
            updated_at: now,
        }
    }

    fn remaining_msats(&self) -> u64 {
        self.limit_msats
            .saturating_sub(self.spent_msats)
            .saturating_sub(self.reserved_msats)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SettlementStatus {
    Reserved,
    Released,
    Withheld,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComputeJobSettlement {
    pub job_hash: String,
    pub owner_key: String,
    pub provider_id: String,
    pub provider_worker_id: String,
    pub reservation_id: String,
    pub amount_msats: u64,
    pub status: SettlementStatus,
    pub verification_passed: Option<bool>,
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub withheld_reason: Option<String>,
    pub reserved_at: DateTime<Utc>,
    pub settled_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderEarningsEntry {
    pub provider_id: String,
    pub earned_msats: u64,
    pub released_jobs: u64,
    pub withheld_jobs: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComputeTreasurySummary {
    pub schema: String,
    pub owner_key: String,
    pub account: BudgetAccount,
    pub released_msats_total: u64,
    pub released_count: u64,
    pub withheld_count: u64,
    pub provider_earnings: Vec<ProviderEarningsEntry>,
    pub recent_jobs: Vec<ComputeJobSettlement>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComputeTreasuryReconcileSummary {
    pub schema: String,
    pub reconciled_at: DateTime<Utc>,
    pub expired_reservations: u64,
    pub freed_msats_total: u64,
    pub updated_jobs: Vec<ComputeJobSettlement>,
}

#[derive(Debug, thiserror::Error)]
pub enum TreasuryError {
    #[error("job already reserved for a different owner")]
    OwnerMismatch,
    #[error("job already reserved with a different amount")]
    AmountMismatch,
    #[error("job is not reserved")]
    NotReserved,
    #[error("job already settled")]
    AlreadySettled,
    #[error("job already settled with conflicting outcome")]
    SettlementConflict,
    #[error("insufficient budget")]
    InsufficientBudget,
}

impl Treasury {
    pub async fn reserve_compute_job(
        &self,
        owner_key: &str,
        job_hash: &str,
        provider_id: &str,
        provider_worker_id: &str,
        amount_msats: u64,
    ) -> Result<(ComputeJobSettlement, bool), TreasuryError> {
        let now = Utc::now();
        let mut inner = self.inner.lock().await;

        if let Some(existing) = inner.compute_jobs.get(job_hash) {
            if existing.owner_key != owner_key {
                return Err(TreasuryError::OwnerMismatch);
            }
            if existing.amount_msats != amount_msats {
                return Err(TreasuryError::AmountMismatch);
            }
            return Ok((existing.clone(), false));
        }

        let account = inner
            .accounts
            .entry(owner_key.to_string())
            .or_insert_with(|| BudgetAccount::ensure_default(now));
        if account.remaining_msats() < amount_msats {
            return Err(TreasuryError::InsufficientBudget);
        }
        account.reserved_msats = account.reserved_msats.saturating_add(amount_msats);
        account.updated_at = now;

        let reservation_id = reservation_id_from_job_hash(job_hash);
        let record = ComputeJobSettlement {
            job_hash: job_hash.to_string(),
            owner_key: owner_key.to_string(),
            provider_id: provider_id.to_string(),
            provider_worker_id: provider_worker_id.to_string(),
            reservation_id,
            amount_msats,
            status: SettlementStatus::Reserved,
            verification_passed: None,
            exit_code: None,
            withheld_reason: None,
            reserved_at: now,
            settled_at: None,
            updated_at: now,
        };
        inner
            .compute_jobs
            .insert(job_hash.to_string(), record.clone());
        Ok((record, true))
    }

    pub async fn settle_compute_job(
        &self,
        job_hash: &str,
        verification_passed: bool,
        exit_code: i32,
        release_allowed: bool,
    ) -> Result<(ComputeJobSettlement, bool), TreasuryError> {
        let now = Utc::now();
        let mut inner = self.inner.lock().await;
        let existing = inner
            .compute_jobs
            .get(job_hash)
            .cloned()
            .ok_or(TreasuryError::NotReserved)?;

        let should_release = verification_passed && release_allowed;
        let desired_status = if should_release {
            SettlementStatus::Released
        } else {
            SettlementStatus::Withheld
        };
        let desired_withheld_reason = if should_release {
            None
        } else if !verification_passed {
            Some("verification_failed")
        } else if !release_allowed {
            Some("price_integrity_failed")
        } else {
            Some("withheld")
        }
        .map(|value| value.to_string());

        match existing.status {
            SettlementStatus::Released | SettlementStatus::Withheld => {
                if existing.verification_passed == Some(verification_passed)
                    && existing.exit_code == Some(exit_code)
                    && existing.status == desired_status
                    && existing.withheld_reason == desired_withheld_reason
                {
                    return Ok((existing, false));
                }
                return Err(TreasuryError::SettlementConflict);
            }
            SettlementStatus::Reserved => {}
        }

        let owner_key = existing.owner_key.clone();
        let amount_msats = existing.amount_msats;
        let account = inner
            .accounts
            .entry(owner_key.clone())
            .or_insert_with(|| BudgetAccount::ensure_default(now));

        let mut updated = existing.clone();
        updated.verification_passed = Some(verification_passed);
        updated.exit_code = Some(exit_code);
        updated.settled_at = Some(now);
        updated.updated_at = now;

        if should_release {
            // Consume reservation.
            account.reserved_msats = account.reserved_msats.saturating_sub(amount_msats);
            account.spent_msats = account.spent_msats.saturating_add(amount_msats);
            updated.status = SettlementStatus::Released;
            updated.withheld_reason = None;
        } else {
            // Release reservation without spending.
            account.reserved_msats = account.reserved_msats.saturating_sub(amount_msats);
            updated.status = SettlementStatus::Withheld;
            updated.withheld_reason = desired_withheld_reason;
        }
        account.updated_at = now;

        inner
            .compute_jobs
            .insert(job_hash.to_string(), updated.clone());
        Ok((updated, true))
    }

    pub async fn get_compute_job(&self, job_hash: &str) -> Option<ComputeJobSettlement> {
        let inner = self.inner.lock().await;
        inner.compute_jobs.get(job_hash).cloned()
    }

    pub async fn get_account(&self, owner_key: &str) -> BudgetAccount {
        let now = Utc::now();
        let mut inner = self.inner.lock().await;
        inner
            .accounts
            .entry(owner_key.to_string())
            .or_insert_with(|| BudgetAccount::ensure_default(now))
            .clone()
    }

    pub async fn summarize_compute_owner(
        &self,
        owner_key: &str,
        job_limit: usize,
    ) -> ComputeTreasurySummary {
        let mut inner = self.inner.lock().await;
        let account = inner
            .accounts
            .entry(owner_key.to_string())
            .or_insert_with(|| BudgetAccount::ensure_default(Utc::now()))
            .clone();

        let mut released_msats_total = 0u64;
        let mut released_count = 0u64;
        let mut withheld_count = 0u64;
        let mut provider_map: HashMap<String, ProviderEarningsEntry> = HashMap::new();

        let mut jobs = inner
            .compute_jobs
            .values()
            .filter(|job| job.owner_key == owner_key)
            .cloned()
            .collect::<Vec<_>>();
        jobs.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));

        for job in &jobs {
            match job.status {
                SettlementStatus::Released => {
                    released_count = released_count.saturating_add(1);
                    released_msats_total = released_msats_total.saturating_add(job.amount_msats);
                    let entry = provider_map
                        .entry(job.provider_id.clone())
                        .or_insert_with(|| ProviderEarningsEntry {
                            provider_id: job.provider_id.clone(),
                            earned_msats: 0,
                            released_jobs: 0,
                            withheld_jobs: 0,
                        });
                    entry.earned_msats = entry.earned_msats.saturating_add(job.amount_msats);
                    entry.released_jobs = entry.released_jobs.saturating_add(1);
                }
                SettlementStatus::Withheld => {
                    withheld_count = withheld_count.saturating_add(1);
                    let entry = provider_map
                        .entry(job.provider_id.clone())
                        .or_insert_with(|| ProviderEarningsEntry {
                            provider_id: job.provider_id.clone(),
                            earned_msats: 0,
                            released_jobs: 0,
                            withheld_jobs: 0,
                        });
                    entry.withheld_jobs = entry.withheld_jobs.saturating_add(1);
                }
                SettlementStatus::Reserved => {}
            }
        }

        let mut provider_earnings = provider_map.into_values().collect::<Vec<_>>();
        provider_earnings.sort_by(|left, right| right.earned_msats.cmp(&left.earned_msats));

        let limit = job_limit.clamp(1, 200);
        jobs.truncate(limit);

        ComputeTreasurySummary {
            schema: "openagents.treasury.compute_summary.v1".to_string(),
            owner_key: owner_key.to_string(),
            account,
            released_msats_total,
            released_count,
            withheld_count,
            provider_earnings,
            recent_jobs: jobs,
        }
    }

    /// Reconcile reserved jobs that were never settled (e.g. process crash mid-request).
    ///
    /// Phase-0 semantics are intentionally conservative: expired reservations are withheld and the
    /// reserved budget is released back to the account without spending.
    pub async fn reconcile_reserved_compute_jobs(
        &self,
        max_age_seconds: i64,
        max_jobs: usize,
    ) -> ComputeTreasuryReconcileSummary {
        let now = Utc::now();
        let max_age_seconds = max_age_seconds.max(0);
        let max_jobs = max_jobs.clamp(1, 2000);

        let mut inner = self.inner.lock().await;

        let mut candidates = inner
            .compute_jobs
            .values()
            .filter(|job| job.status == SettlementStatus::Reserved)
            .filter(|job| (now - job.reserved_at).num_seconds() >= max_age_seconds)
            .map(|job| job.job_hash.clone())
            .collect::<Vec<_>>();
        candidates.sort();
        candidates.truncate(max_jobs);

        let mut expired_reservations = 0u64;
        let mut freed_msats_total = 0u64;
        let mut updated_jobs = Vec::new();

        for job_hash in candidates {
            let Some(existing) = inner.compute_jobs.get(&job_hash).cloned() else {
                continue;
            };
            if existing.status != SettlementStatus::Reserved {
                continue;
            }

            let owner_key = existing.owner_key.clone();
            let amount_msats = existing.amount_msats;

            let account = inner
                .accounts
                .entry(owner_key.clone())
                .or_insert_with(|| BudgetAccount::ensure_default(now));
            account.reserved_msats = account.reserved_msats.saturating_sub(amount_msats);
            account.updated_at = now;

            let mut updated = existing.clone();
            updated.status = SettlementStatus::Withheld;
            updated.withheld_reason = Some("reservation_expired".to_string());
            updated.settled_at = Some(now);
            updated.updated_at = now;

            inner
                .compute_jobs
                .insert(job_hash.to_string(), updated.clone());
            expired_reservations = expired_reservations.saturating_add(1);
            freed_msats_total = freed_msats_total.saturating_add(amount_msats);
            updated_jobs.push(updated);
        }

        ComputeTreasuryReconcileSummary {
            schema: "openagents.treasury.compute_reconcile_summary.v1".to_string(),
            reconciled_at: now,
            expired_reservations,
            freed_msats_total,
            updated_jobs,
        }
    }
}

fn reservation_id_from_job_hash(job_hash: &str) -> String {
    let normalized = job_hash.trim();
    if normalized.len() >= 16 {
        format!("rsv_{}", &normalized[..16])
    } else if normalized.is_empty() {
        "rsv_invalid".to_string()
    } else {
        format!("rsv_{normalized}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reconcile_expires_reserved_job_and_releases_budget() {
        let treasury = Treasury::default();

        let (_job, created) = treasury
            .reserve_compute_job("owner-1", "jobhash-1", "provider-1", "worker-1", 1000)
            .await
            .expect("reserve should succeed");
        assert!(created);

        let account_before = treasury.get_account("owner-1").await;
        assert_eq!(account_before.reserved_msats, 1000);
        assert_eq!(account_before.spent_msats, 0);

        let summary = treasury.reconcile_reserved_compute_jobs(0, 50).await;
        assert_eq!(summary.expired_reservations, 1);
        assert_eq!(summary.freed_msats_total, 1000);

        let job_after = treasury
            .get_compute_job("jobhash-1")
            .await
            .expect("job should exist");
        assert_eq!(job_after.status, SettlementStatus::Withheld);
        assert_eq!(
            job_after.withheld_reason.as_deref(),
            Some("reservation_expired")
        );

        let account_after = treasury.get_account("owner-1").await;
        assert_eq!(account_after.reserved_msats, 0);
        assert_eq!(account_after.spent_msats, 0);
    }

    #[tokio::test]
    async fn reconcile_is_idempotent() {
        let treasury = Treasury::default();
        let (_job, _created) = treasury
            .reserve_compute_job("owner-2", "jobhash-2", "provider-2", "worker-2", 1000)
            .await
            .expect("reserve should succeed");

        let first = treasury.reconcile_reserved_compute_jobs(0, 50).await;
        assert_eq!(first.expired_reservations, 1);

        let second = treasury.reconcile_reserved_compute_jobs(0, 50).await;
        assert_eq!(second.expired_reservations, 0);

        let job_after = treasury
            .get_compute_job("jobhash-2")
            .await
            .expect("job should exist");
        assert_eq!(job_after.status, SettlementStatus::Withheld);
    }
}
