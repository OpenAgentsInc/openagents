use std::collections::HashMap;
use std::sync::Mutex;

use chrono::{DateTime, Utc};

#[derive(Debug, thiserror::Error)]
pub enum BudgetError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("budget exceeded: {0}")]
    Exceeded(String),
    #[error("internal: {0}")]
    Internal(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BudgetFinalizeDisposition {
    Commit,
    Release,
}

#[derive(Debug, Clone)]
pub struct BudgetReservation {
    pub reservation_id: String,
    pub idempotency_key: String,
    pub max_amount_msats: u64,
    pub policy_context_sha256: String,
    pub created_at: DateTime<Utc>,
}

pub trait BudgetHooks: Send + Sync {
    fn reserve(
        &self,
        idempotency_key: &str,
        max_amount_msats: u64,
        policy_context_sha256: &str,
    ) -> Result<BudgetReservation, BudgetError>;

    fn finalize(
        &self,
        reservation_id: &str,
        disposition: BudgetFinalizeDisposition,
    ) -> Result<(), BudgetError>;
}

#[derive(Debug, Clone)]
struct ReservationState {
    reservation: BudgetReservation,
    disposition: Option<BudgetFinalizeDisposition>,
}

pub struct InMemoryBudgetHooks {
    max_single_reservation_msats: u64,
    by_idempotency: Mutex<HashMap<String, String>>,
    by_reservation: Mutex<HashMap<String, ReservationState>>,
}

impl InMemoryBudgetHooks {
    #[must_use]
    pub fn new(max_single_reservation_msats: u64) -> Self {
        Self {
            max_single_reservation_msats,
            by_idempotency: Mutex::new(HashMap::new()),
            by_reservation: Mutex::new(HashMap::new()),
        }
    }

    fn lock_error(label: &str) -> BudgetError {
        BudgetError::Internal(format!("mutex poisoned: {label}"))
    }
}

impl Default for InMemoryBudgetHooks {
    fn default() -> Self {
        Self::new(500_000_000)
    }
}

impl BudgetHooks for InMemoryBudgetHooks {
    fn reserve(
        &self,
        idempotency_key: &str,
        max_amount_msats: u64,
        policy_context_sha256: &str,
    ) -> Result<BudgetReservation, BudgetError> {
        let key = idempotency_key.trim();
        if key.is_empty() {
            return Err(BudgetError::InvalidRequest(
                "idempotency_key is required".to_string(),
            ));
        }
        if max_amount_msats == 0 {
            return Err(BudgetError::InvalidRequest(
                "max_amount_msats must be > 0".to_string(),
            ));
        }
        if max_amount_msats > self.max_single_reservation_msats {
            return Err(BudgetError::Exceeded(format!(
                "max_amount_msats {max_amount_msats} exceeds limit {}",
                self.max_single_reservation_msats
            )));
        }

        let mut by_reservation = self
            .by_reservation
            .lock()
            .map_err(|_| Self::lock_error("by_reservation"))?;
        let mut by_idempotency = self
            .by_idempotency
            .lock()
            .map_err(|_| Self::lock_error("by_idempotency"))?;

        if let Some(existing_id) = by_idempotency.get(key) {
            let Some(existing) = by_reservation.get(existing_id) else {
                return Err(BudgetError::Internal(
                    "idempotency pointer missing reservation".to_string(),
                ));
            };
            if existing.reservation.max_amount_msats != max_amount_msats
                || existing.reservation.policy_context_sha256 != policy_context_sha256
            {
                return Err(BudgetError::Conflict(
                    "idempotency_key reused with different reservation parameters".to_string(),
                ));
            }
            return Ok(existing.reservation.clone());
        }

        let created_at = Utc::now();
        let reservation_id = format!(
            "nbrsv_{}",
            uuid::Uuid::new_v5(
                &uuid::Uuid::NAMESPACE_OID,
                format!(
                    "{}|{}|{}",
                    key,
                    max_amount_msats,
                    policy_context_sha256.trim()
                )
                .as_bytes(),
            )
            .simple()
        );
        let reservation = BudgetReservation {
            reservation_id: reservation_id.clone(),
            idempotency_key: key.to_string(),
            max_amount_msats,
            policy_context_sha256: policy_context_sha256.trim().to_string(),
            created_at,
        };
        by_idempotency.insert(key.to_string(), reservation_id.clone());
        by_reservation.insert(
            reservation_id,
            ReservationState {
                reservation: reservation.clone(),
                disposition: None,
            },
        );
        Ok(reservation)
    }

    fn finalize(
        &self,
        reservation_id: &str,
        disposition: BudgetFinalizeDisposition,
    ) -> Result<(), BudgetError> {
        let mut by_reservation = self
            .by_reservation
            .lock()
            .map_err(|_| Self::lock_error("by_reservation"))?;
        let Some(existing) = by_reservation.get_mut(reservation_id.trim()) else {
            return Err(BudgetError::InvalidRequest(
                "unknown reservation_id".to_string(),
            ));
        };
        if let Some(current) = &existing.disposition {
            if *current == disposition {
                return Ok(());
            }
            return Err(BudgetError::Conflict(
                "reservation already finalized with different disposition".to_string(),
            ));
        }
        existing.disposition = Some(disposition);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{BudgetFinalizeDisposition, BudgetHooks, InMemoryBudgetHooks};

    #[test]
    fn reserve_is_idempotent_for_matching_fingerprint() -> Result<(), Box<dyn std::error::Error>> {
        let hooks = InMemoryBudgetHooks::new(10_000);
        let first = hooks.reserve("idem-1", 5_000, "policy-sha")?;
        let second = hooks.reserve("idem-1", 5_000, "policy-sha")?;
        assert_eq!(first.reservation_id, second.reservation_id);
        hooks.finalize(
            first.reservation_id.as_str(),
            BudgetFinalizeDisposition::Commit,
        )?;
        hooks.finalize(
            second.reservation_id.as_str(),
            BudgetFinalizeDisposition::Commit,
        )?;
        Ok(())
    }
}
