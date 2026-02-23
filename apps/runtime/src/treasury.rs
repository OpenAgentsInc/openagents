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
    ) -> Result<(ComputeJobSettlement, bool), TreasuryError> {
        let now = Utc::now();
        let mut inner = self.inner.lock().await;
        let existing = inner
            .compute_jobs
            .get(job_hash)
            .cloned()
            .ok_or(TreasuryError::NotReserved)?;

        match existing.status {
            SettlementStatus::Released | SettlementStatus::Withheld => {
                if existing.verification_passed == Some(verification_passed)
                    && existing.exit_code == Some(exit_code)
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

        if verification_passed {
            // Consume reservation.
            account.reserved_msats = account.reserved_msats.saturating_sub(amount_msats);
            account.spent_msats = account.spent_msats.saturating_add(amount_msats);
            updated.status = SettlementStatus::Released;
        } else {
            // Release reservation without spending.
            account.reserved_msats = account.reserved_msats.saturating_sub(amount_msats);
            updated.status = SettlementStatus::Withheld;
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
