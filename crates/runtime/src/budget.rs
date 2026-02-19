//! Budget policy and tracking.

use crate::identity::PublicKey;
use crate::types::Timestamp;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const DAY_MILLIS: u64 = 86_400_000;

/// Static budget policy (micro-USD).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BudgetPolicy {
    /// Maximum spend per tick (micro-USD).
    pub per_tick_usd: u64,
    /// Maximum spend per day (micro-USD).
    pub per_day_usd: u64,
    /// Spend above this requires approval (micro-USD).
    pub approval_threshold_usd: u64,
    /// Approvers allowed to authorize.
    pub approvers: Vec<PublicKey>,
}

impl BudgetPolicy {
    /// Create a policy with zeroed limits.
    pub fn zero() -> Self {
        Self {
            per_tick_usd: 0,
            per_day_usd: 0,
            approval_threshold_usd: 0,
            approvers: Vec::new(),
        }
    }
}

/// Dynamic budget counters (micro-USD).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BudgetState {
    /// Amount reserved this tick.
    pub reserved_tick_usd: u64,
    /// Amount reserved today.
    pub reserved_day_usd: u64,
    /// Amount spent this tick.
    pub spent_tick_usd: u64,
    /// Amount spent today.
    pub spent_day_usd: u64,
    /// Day boundary for resetting daily counters.
    pub day_start: Timestamp,
}

impl BudgetState {
    /// Create a fresh state anchored at the provided timestamp.
    pub fn new(now: Timestamp) -> Self {
        Self {
            reserved_tick_usd: 0,
            reserved_day_usd: 0,
            spent_tick_usd: 0,
            spent_day_usd: 0,
            day_start: now,
        }
    }

    /// Remaining tick budget.
    pub fn remaining_tick(&self, policy: &BudgetPolicy) -> u64 {
        policy
            .per_tick_usd
            .saturating_sub(self.reserved_tick_usd + self.spent_tick_usd)
    }

    /// Remaining day budget.
    pub fn remaining_day(&self, policy: &BudgetPolicy) -> u64 {
        policy
            .per_day_usd
            .saturating_sub(self.reserved_day_usd + self.spent_day_usd)
    }
}

/// Budget reservation returned by a reserve operation.
#[derive(Clone, Copy, Debug)]
pub struct BudgetReservation {
    /// Reserved amount (micro-USD).
    pub amount_usd: u64,
}

/// Budget errors.
#[derive(Debug, thiserror::Error)]
pub enum BudgetError {
    /// Reservation exceeds remaining budget.
    #[error("budget exceeded")]
    Exceeded,
    /// Actual cost exceeds reserved amount.
    #[error("actual cost exceeds reserved amount")]
    ActualExceedsReservation,
}

/// Budget tracker with reserve/reconcile primitives.
#[derive(Clone, Debug)]
pub struct BudgetTracker {
    policy: BudgetPolicy,
    state: BudgetState,
}

impl BudgetTracker {
    /// Create a tracker with the provided policy.
    pub fn new(policy: BudgetPolicy) -> Self {
        let now = Timestamp::now();
        Self {
            policy,
            state: BudgetState::new(now),
        }
    }

    /// Get the current policy.
    pub fn policy(&self) -> &BudgetPolicy {
        &self.policy
    }

    /// Get the current state snapshot.
    pub fn state(&self) -> &BudgetState {
        &self.state
    }

    /// Reset tick counters (call at tick boundaries).
    pub fn reset_tick(&mut self) {
        self.state.reserved_tick_usd = 0;
        self.state.spent_tick_usd = 0;
    }

    /// Reserve budget for a max cost.
    pub fn reserve(&mut self, amount_usd: u64) -> Result<BudgetReservation, BudgetError> {
        self.rollover_day();
        if amount_usd > self.state.remaining_tick(&self.policy)
            || amount_usd > self.state.remaining_day(&self.policy)
        {
            return Err(BudgetError::Exceeded);
        }
        self.state.reserved_tick_usd += amount_usd;
        self.state.reserved_day_usd += amount_usd;
        Ok(BudgetReservation { amount_usd })
    }

    /// Reconcile a reservation with the actual cost.
    pub fn reconcile(
        &mut self,
        reservation: BudgetReservation,
        actual_usd: u64,
    ) -> Result<(), BudgetError> {
        if actual_usd > reservation.amount_usd {
            return Err(BudgetError::ActualExceedsReservation);
        }

        self.state.reserved_tick_usd = self
            .state
            .reserved_tick_usd
            .saturating_sub(reservation.amount_usd);
        self.state.reserved_day_usd = self
            .state
            .reserved_day_usd
            .saturating_sub(reservation.amount_usd);

        self.state.spent_tick_usd += actual_usd;
        self.state.spent_day_usd += actual_usd;
        Ok(())
    }

    /// Release a reservation without spending.
    pub fn release(&mut self, reservation: BudgetReservation) {
        self.state.reserved_tick_usd = self
            .state
            .reserved_tick_usd
            .saturating_sub(reservation.amount_usd);
        self.state.reserved_day_usd = self
            .state
            .reserved_day_usd
            .saturating_sub(reservation.amount_usd);
    }

    fn rollover_day(&mut self) {
        let now = Timestamp::now();
        if now
            .as_millis()
            .saturating_sub(self.state.day_start.as_millis())
            >= DAY_MILLIS
        {
            self.state.day_start = now;
            self.state.reserved_day_usd = 0;
            self.state.spent_day_usd = 0;
        }
    }

    /// Convenience helper to reserve and immediately reconcile a charge.
    pub fn charge(&mut self, amount_usd: u64) -> Result<(), BudgetError> {
        let reservation = self.reserve(amount_usd)?;
        self.reconcile(reservation, amount_usd)
    }
}

/// Helper to convert a duration into milliseconds (micro-USD budgets use millis timestamps).
pub fn duration_to_millis(duration: Duration) -> u64 {
    duration.as_millis() as u64
}
