//! Budget management for SwarmCompiler
//!
//! Tracks and allocates costs across optimization phases to prevent runaway spending.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// A budget allocation for a specific phase.
#[derive(Debug, Clone)]
pub struct BudgetAllocation {
    /// Name of the phase.
    pub phase: String,
    /// Amount allocated to this phase in millisatoshis.
    pub allocated: u64,
    /// Amount spent so far.
    spent: Arc<AtomicU64>,
}

impl BudgetAllocation {
    /// Try to spend from this allocation.
    pub fn try_spend(&self, msats: u64) -> Result<()> {
        let current = self.spent.load(Ordering::SeqCst);
        if current + msats > self.allocated {
            return Err(anyhow!(
                "Phase '{}' budget exceeded: would spend {} + {} = {} > {} allocated",
                self.phase,
                current,
                msats,
                current + msats,
                self.allocated
            ));
        }
        self.spent.fetch_add(msats, Ordering::SeqCst);
        Ok(())
    }

    /// Get amount spent so far.
    pub fn spent(&self) -> u64 {
        self.spent.load(Ordering::SeqCst)
    }

    /// Get remaining budget.
    pub fn remaining(&self) -> u64 {
        self.allocated.saturating_sub(self.spent())
    }
}

/// Report of budget usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetReport {
    /// Total budget allocated.
    pub total: u64,
    /// Total spent across all phases.
    pub spent: u64,
    /// Spending by phase.
    pub by_phase: HashMap<String, PhaseSpend>,
}

/// Spending details for a single phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseSpend {
    pub allocated: u64,
    pub spent: u64,
}

impl BudgetReport {
    /// Get remaining budget.
    pub fn remaining(&self) -> u64 {
        self.total.saturating_sub(self.spent)
    }

    /// Get percentage used.
    pub fn percent_used(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.spent as f64 / self.total as f64) * 100.0
        }
    }
}

/// Budget manager for tracking and allocating costs across optimization phases.
///
/// # Example
///
/// ```
/// use dsrs::compiler::BudgetManager;
///
/// let mut budget = BudgetManager::new(10000); // 10,000 msats total
///
/// // Allocate to phases
/// let bootstrap = budget.allocate("bootstrap", 2000).unwrap();
/// let validate = budget.allocate("validate", 8000).unwrap();
///
/// // Spend from allocations
/// bootstrap.try_spend(100).unwrap();
/// validate.try_spend(500).unwrap();
///
/// // Get report
/// let report = budget.get_report();
/// assert_eq!(report.spent, 600);
/// ```
#[derive(Debug)]
pub struct BudgetManager {
    /// Total budget in millisatoshis.
    total_budget_msats: u64,
    /// Allocations by phase.
    allocations: HashMap<String, BudgetAllocation>,
    /// Total allocated so far.
    total_allocated: u64,
}

impl BudgetManager {
    /// Create a new budget manager with a total budget.
    pub fn new(total_budget_msats: u64) -> Self {
        Self {
            total_budget_msats,
            allocations: HashMap::new(),
            total_allocated: 0,
        }
    }

    /// Allocate budget to a phase.
    ///
    /// Returns an allocation handle for tracking spending in that phase.
    pub fn allocate(&mut self, phase: &str, msats: u64) -> Result<BudgetAllocation> {
        if self.total_allocated + msats > self.total_budget_msats {
            return Err(anyhow!(
                "Cannot allocate {} msats to '{}': only {} remaining of {} total",
                msats,
                phase,
                self.total_budget_msats - self.total_allocated,
                self.total_budget_msats
            ));
        }

        let allocation = BudgetAllocation {
            phase: phase.to_string(),
            allocated: msats,
            spent: Arc::new(AtomicU64::new(0)),
        };

        self.allocations.insert(phase.to_string(), allocation.clone());
        self.total_allocated += msats;

        Ok(allocation)
    }

    /// Get total spent across all phases.
    pub fn total_spent(&self) -> u64 {
        self.allocations.values().map(|a| a.spent()).sum()
    }

    /// Get remaining unallocated budget.
    pub fn unallocated(&self) -> u64 {
        self.total_budget_msats - self.total_allocated
    }

    /// Get remaining unspent budget (across all allocations).
    pub fn remaining(&self) -> u64 {
        self.total_budget_msats.saturating_sub(self.total_spent())
    }

    /// Get a budget report.
    pub fn get_report(&self) -> BudgetReport {
        let by_phase: HashMap<String, PhaseSpend> = self
            .allocations
            .iter()
            .map(|(name, alloc)| {
                (
                    name.clone(),
                    PhaseSpend {
                        allocated: alloc.allocated,
                        spent: alloc.spent(),
                    },
                )
            })
            .collect();

        BudgetReport {
            total: self.total_budget_msats,
            spent: self.total_spent(),
            by_phase,
        }
    }

    /// Check if we can afford to spend a given amount.
    pub fn can_afford(&self, msats: u64) -> bool {
        self.remaining() >= msats
    }
}

impl Default for BudgetManager {
    fn default() -> Self {
        // Default to 10,000 msats (10 sats)
        Self::new(10000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_budget_allocation() {
        let mut budget = BudgetManager::new(1000);

        let alloc = budget.allocate("test", 500).unwrap();
        assert_eq!(alloc.allocated, 500);
        assert_eq!(alloc.spent(), 0);
        assert_eq!(alloc.remaining(), 500);
    }

    #[test]
    fn test_budget_spending() {
        let mut budget = BudgetManager::new(1000);
        let alloc = budget.allocate("test", 500).unwrap();

        alloc.try_spend(100).unwrap();
        assert_eq!(alloc.spent(), 100);
        assert_eq!(alloc.remaining(), 400);

        alloc.try_spend(400).unwrap();
        assert_eq!(alloc.spent(), 500);
        assert_eq!(alloc.remaining(), 0);
    }

    #[test]
    fn test_budget_overspend() {
        let mut budget = BudgetManager::new(1000);
        let alloc = budget.allocate("test", 500).unwrap();

        let result = alloc.try_spend(600);
        assert!(result.is_err());
    }

    #[test]
    fn test_budget_over_allocation() {
        let mut budget = BudgetManager::new(1000);

        budget.allocate("phase1", 600).unwrap();
        let result = budget.allocate("phase2", 600);
        assert!(result.is_err());
    }

    #[test]
    fn test_budget_report() {
        let mut budget = BudgetManager::new(1000);

        let bootstrap = budget.allocate("bootstrap", 200).unwrap();
        let validate = budget.allocate("validate", 800).unwrap();

        bootstrap.try_spend(50).unwrap();
        validate.try_spend(300).unwrap();

        let report = budget.get_report();
        assert_eq!(report.total, 1000);
        assert_eq!(report.spent, 350);
        assert_eq!(report.remaining(), 650);
        assert!((report.percent_used() - 35.0).abs() < 0.01);

        assert_eq!(report.by_phase["bootstrap"].spent, 50);
        assert_eq!(report.by_phase["validate"].spent, 300);
    }

    #[test]
    fn test_budget_can_afford() {
        let mut budget = BudgetManager::new(1000);
        let alloc = budget.allocate("test", 500).unwrap();

        assert!(budget.can_afford(1000));

        alloc.try_spend(400).unwrap();
        assert!(budget.can_afford(600));
        assert!(!budget.can_afford(700));
    }

    #[test]
    fn test_budget_unallocated() {
        let mut budget = BudgetManager::new(1000);
        assert_eq!(budget.unallocated(), 1000);

        budget.allocate("phase1", 300).unwrap();
        assert_eq!(budget.unallocated(), 700);

        budget.allocate("phase2", 500).unwrap();
        assert_eq!(budget.unallocated(), 200);
    }
}
