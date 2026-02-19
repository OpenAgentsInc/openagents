//! Budget constraints enforcement for agent state
//!
//! This module provides budget enforcement to prevent agents from overspending.
//! Budget limits can be set at multiple levels:
//! - Daily limit: Maximum spending per UTC day
//! - Per-tick limit: Maximum spending per execution tick
//! - Reserved balance: Minimum balance that must be maintained
//!
//! Budget violations are tracked and can be published as anomalies for monitoring.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Budget constraint errors
#[derive(Debug, Error)]
pub enum BudgetError {
    #[error("daily budget exceeded: spent {spent} sats, limit {limit} sats")]
    DailyLimitExceeded { spent: u64, limit: u64 },

    #[error("per-tick budget exceeded: spent {spent} sats, limit {limit} sats")]
    PerTickLimitExceeded { spent: u64, limit: u64 },

    #[error("reserved balance violated: balance {balance} sats, reserved {reserved} sats")]
    ReservedBalanceViolated { balance: u64, reserved: u64 },

    #[error("insufficient balance: need {needed} sats, have {available} sats")]
    InsufficientBalance { needed: u64, available: u64 },
}

/// Budget limits configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetLimits {
    /// Maximum spending per UTC day in satoshis
    pub daily_limit_sats: u64,
    /// Maximum spending per tick in satoshis
    pub per_tick_limit_sats: u64,
    /// Reserved balance that cannot be spent (emergency funds)
    pub reserved_sats: u64,
}

impl Default for BudgetLimits {
    fn default() -> Self {
        Self {
            daily_limit_sats: 10_000,   // 10k sats per day (~$10 at $100k/BTC)
            per_tick_limit_sats: 1_000, // 1k sats per tick
            reserved_sats: 5_000,       // 5k sats reserved
        }
    }
}

/// Budget spending tracker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetTracker {
    /// Budget limits
    pub limits: BudgetLimits,
    /// Current UTC date (YYYY-MM-DD format)
    pub current_date: String,
    /// Total spending today in satoshis
    pub daily_spent_sats: u64,
    /// Spending in current tick in satoshis
    pub tick_spent_sats: u64,
    /// Number of budget violations today
    pub violations_today: u32,
}

impl BudgetTracker {
    /// Create a new budget tracker with default limits
    pub fn new() -> Self {
        Self::with_limits(BudgetLimits::default())
    }

    /// Create a budget tracker with custom limits
    pub fn with_limits(limits: BudgetLimits) -> Self {
        let current_date = Self::get_current_utc_date();
        Self {
            limits,
            current_date,
            daily_spent_sats: 0,
            tick_spent_sats: 0,
            violations_today: 0,
        }
    }

    /// Get current UTC date in YYYY-MM-DD format
    fn get_current_utc_date() -> String {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Convert to UTC date
        let days = now / 86400;
        let epoch_days = 719_163; // Days from year 0 to Unix epoch (1970-01-01)
        let _total_days = epoch_days + days;

        // Calculate year (approximation)
        let year = 1970 + (days / 365);
        let day_of_year = days % 365;

        // Simple month/day calculation (approximation, good enough for daily reset)
        let month = (day_of_year / 30) + 1;
        let day = (day_of_year % 30) + 1;

        format!("{:04}-{:02}-{:02}", year, month.min(12), day.min(31))
    }

    /// Check if it's a new UTC day and reset counters if needed
    pub fn check_and_reset_daily(&mut self) {
        let current_date = Self::get_current_utc_date();
        if current_date != self.current_date {
            // New day - reset daily counters
            self.current_date = current_date;
            self.daily_spent_sats = 0;
            self.violations_today = 0;
        }
    }

    /// Reset tick spending counter (call at start of each tick)
    pub fn reset_tick(&mut self) {
        self.tick_spent_sats = 0;
    }

    /// Check if a spend operation is allowed
    ///
    /// # Arguments
    /// * `amount_sats` - Amount to spend in satoshis
    /// * `current_balance_sats` - Current wallet balance in satoshis
    ///
    /// # Returns
    /// Ok(()) if spend is allowed, Err(BudgetError) if budget would be violated
    pub fn check_spend(
        &self,
        amount_sats: u64,
        current_balance_sats: u64,
    ) -> Result<(), BudgetError> {
        // Check sufficient balance
        if amount_sats > current_balance_sats {
            return Err(BudgetError::InsufficientBalance {
                needed: amount_sats,
                available: current_balance_sats,
            });
        }

        // Check reserved balance
        let available_to_spend = current_balance_sats.saturating_sub(self.limits.reserved_sats);
        if amount_sats > available_to_spend {
            return Err(BudgetError::ReservedBalanceViolated {
                balance: current_balance_sats,
                reserved: self.limits.reserved_sats,
            });
        }

        // Check daily limit
        let new_daily_spent = self.daily_spent_sats.saturating_add(amount_sats);
        if new_daily_spent > self.limits.daily_limit_sats {
            return Err(BudgetError::DailyLimitExceeded {
                spent: new_daily_spent,
                limit: self.limits.daily_limit_sats,
            });
        }

        // Check per-tick limit
        let new_tick_spent = self.tick_spent_sats.saturating_add(amount_sats);
        if new_tick_spent > self.limits.per_tick_limit_sats {
            return Err(BudgetError::PerTickLimitExceeded {
                spent: new_tick_spent,
                limit: self.limits.per_tick_limit_sats,
            });
        }

        Ok(())
    }

    /// Record a spend operation
    ///
    /// Should be called after a spend succeeds to update counters.
    ///
    /// # Arguments
    /// * `amount_sats` - Amount spent in satoshis
    pub fn record_spend(&mut self, amount_sats: u64) {
        self.daily_spent_sats = self.daily_spent_sats.saturating_add(amount_sats);
        self.tick_spent_sats = self.tick_spent_sats.saturating_add(amount_sats);
    }

    /// Record a budget violation
    pub fn record_violation(&mut self) {
        self.violations_today = self.violations_today.saturating_add(1);
    }

    /// Get remaining budget for today
    pub fn remaining_daily_budget(&self) -> u64 {
        self.limits
            .daily_limit_sats
            .saturating_sub(self.daily_spent_sats)
    }

    /// Get remaining budget for current tick
    pub fn remaining_tick_budget(&self) -> u64 {
        self.limits
            .per_tick_limit_sats
            .saturating_sub(self.tick_spent_sats)
    }

    /// Get available spendable balance
    ///
    /// Returns the minimum of:
    /// - Current balance minus reserved
    /// - Remaining daily budget
    /// - Remaining tick budget
    pub fn available_to_spend(&self, current_balance_sats: u64) -> u64 {
        let balance_available = current_balance_sats.saturating_sub(self.limits.reserved_sats);
        let daily_available = self.remaining_daily_budget();
        let tick_available = self.remaining_tick_budget();

        balance_available.min(daily_available).min(tick_available)
    }
}

impl Default for BudgetTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_budget_limits_default() {
        let limits = BudgetLimits::default();
        assert_eq!(limits.daily_limit_sats, 10_000);
        assert_eq!(limits.per_tick_limit_sats, 1_000);
        assert_eq!(limits.reserved_sats, 5_000);
    }

    #[test]
    fn test_budget_tracker_creation() {
        let tracker = BudgetTracker::new();
        assert_eq!(tracker.daily_spent_sats, 0);
        assert_eq!(tracker.tick_spent_sats, 0);
        assert_eq!(tracker.violations_today, 0);
        assert!(!tracker.current_date.is_empty());
    }

    #[test]
    fn test_check_spend_insufficient_balance() {
        let tracker = BudgetTracker::new();
        let result = tracker.check_spend(1000, 500);

        assert!(matches!(
            result,
            Err(BudgetError::InsufficientBalance { .. })
        ));
    }

    #[test]
    fn test_check_spend_reserved_balance() {
        let mut tracker = BudgetTracker::new();
        tracker.limits.reserved_sats = 5000;

        // Try to spend 6000 sats with 10000 balance
        // Available = 10000 - 5000 = 5000, so 6000 should fail
        let result = tracker.check_spend(6000, 10000);

        assert!(matches!(
            result,
            Err(BudgetError::ReservedBalanceViolated { .. })
        ));
    }

    #[test]
    fn test_check_spend_daily_limit() {
        let mut tracker = BudgetTracker::new();
        tracker.daily_spent_sats = 9500;
        tracker.limits.daily_limit_sats = 10000;

        // Try to spend 600 sats (would exceed daily limit)
        let result = tracker.check_spend(600, 50000);

        assert!(matches!(
            result,
            Err(BudgetError::DailyLimitExceeded { .. })
        ));
    }

    #[test]
    fn test_check_spend_per_tick_limit() {
        let mut tracker = BudgetTracker::new();
        tracker.tick_spent_sats = 800;
        tracker.limits.per_tick_limit_sats = 1000;

        // Try to spend 300 sats (would exceed tick limit)
        let result = tracker.check_spend(300, 50000);

        assert!(matches!(
            result,
            Err(BudgetError::PerTickLimitExceeded { .. })
        ));
    }

    #[test]
    fn test_check_spend_success() {
        let tracker = BudgetTracker::new();

        // Spend 500 sats with 50000 balance - should succeed
        let result = tracker.check_spend(500, 50000);
        assert!(result.is_ok());
    }

    #[test]
    fn test_record_spend() {
        let mut tracker = BudgetTracker::new();
        assert_eq!(tracker.daily_spent_sats, 0);
        assert_eq!(tracker.tick_spent_sats, 0);

        tracker.record_spend(500);
        assert_eq!(tracker.daily_spent_sats, 500);
        assert_eq!(tracker.tick_spent_sats, 500);

        tracker.record_spend(300);
        assert_eq!(tracker.daily_spent_sats, 800);
        assert_eq!(tracker.tick_spent_sats, 800);
    }

    #[test]
    fn test_reset_tick() {
        let mut tracker = BudgetTracker::new();
        tracker.record_spend(500);
        assert_eq!(tracker.tick_spent_sats, 500);

        tracker.reset_tick();
        assert_eq!(tracker.tick_spent_sats, 0);
        assert_eq!(tracker.daily_spent_sats, 500); // Daily should NOT reset
    }

    #[test]
    fn test_record_violation() {
        let mut tracker = BudgetTracker::new();
        assert_eq!(tracker.violations_today, 0);

        tracker.record_violation();
        assert_eq!(tracker.violations_today, 1);

        tracker.record_violation();
        assert_eq!(tracker.violations_today, 2);
    }

    #[test]
    fn test_remaining_budgets() {
        let mut tracker = BudgetTracker::new();
        tracker.limits.daily_limit_sats = 10000;
        tracker.limits.per_tick_limit_sats = 1000;

        tracker.daily_spent_sats = 3000;
        tracker.tick_spent_sats = 400;

        assert_eq!(tracker.remaining_daily_budget(), 7000);
        assert_eq!(tracker.remaining_tick_budget(), 600);
    }

    #[test]
    fn test_available_to_spend() {
        let mut tracker = BudgetTracker::new();
        tracker.limits.daily_limit_sats = 10000;
        tracker.limits.per_tick_limit_sats = 1000;
        tracker.limits.reserved_sats = 5000;

        tracker.daily_spent_sats = 8000; // 2000 daily budget left
        tracker.tick_spent_sats = 200; // 800 tick budget left

        // Balance: 50000
        // Available from balance: 50000 - 5000 = 45000
        // Daily available: 2000
        // Tick available: 800
        // Minimum: 800
        let available = tracker.available_to_spend(50000);
        assert_eq!(available, 800);
    }

    #[test]
    fn test_spend_workflow() {
        let mut tracker = BudgetTracker::new();
        let balance = 50000;

        // Check if we can spend 500 sats
        assert!(tracker.check_spend(500, balance).is_ok());

        // Record the spend
        tracker.record_spend(500);

        // Check that counters updated
        assert_eq!(tracker.daily_spent_sats, 500);
        assert_eq!(tracker.tick_spent_sats, 500);

        // Next tick - reset tick counter
        tracker.reset_tick();
        assert_eq!(tracker.tick_spent_sats, 0);
        assert_eq!(tracker.daily_spent_sats, 500); // Daily persists
    }

    #[test]
    fn test_overflow_protection() {
        let mut tracker = BudgetTracker::new();
        tracker.daily_spent_sats = u64::MAX;

        // Recording spend should saturate, not overflow
        tracker.record_spend(100);
        assert_eq!(tracker.daily_spent_sats, u64::MAX);
    }
}
