//! Agent Lifecycle Management
//!
//! Manages agent state transitions based on wallet balance and activity.

use crate::config::LifecycleState;
use thiserror::Error;

/// Errors that can occur during lifecycle operations
#[derive(Debug, Error)]
pub enum LifecycleError {
    #[error("invalid state transition from {from:?} to {to:?}")]
    InvalidTransition {
        from: LifecycleState,
        to: LifecycleState,
    },

    #[error("agent is dead and cannot be resurrected")]
    AgentDead,
}

/// Configuration for lifecycle thresholds
#[derive(Debug, Clone)]
pub struct LifecycleConfig {
    /// Minimum days of runway before entering LowBalance
    pub low_balance_days: f64,
    /// Minimum sats before hibernating
    pub hibernate_threshold_sats: u64,
    /// Estimated cost per tick in sats
    pub cost_per_tick_sats: u64,
    /// Ticks per day (based on heartbeat)
    pub ticks_per_day: f64,
}

impl Default for LifecycleConfig {
    fn default() -> Self {
        Self {
            low_balance_days: 7.0,
            hibernate_threshold_sats: 1000,
            cost_per_tick_sats: 100,
            ticks_per_day: 96.0, // 15-minute heartbeat = 96 ticks/day
        }
    }
}

impl LifecycleConfig {
    /// Create config from heartbeat interval
    pub fn with_heartbeat_seconds(heartbeat: u64) -> Self {
        let ticks_per_day = if heartbeat > 0 {
            86400.0 / heartbeat as f64
        } else {
            0.0
        };

        Self {
            ticks_per_day,
            ..Default::default()
        }
    }

    /// Estimate daily burn rate
    pub fn daily_burn_sats(&self) -> u64 {
        (self.ticks_per_day * self.cost_per_tick_sats as f64) as u64
    }
}

/// Analysis of agent's financial runway
#[derive(Debug, Clone)]
pub struct RunwayAnalysis {
    /// Current wallet balance in sats
    pub balance_sats: u64,
    /// Estimated daily burn rate in sats
    pub daily_burn_sats: u64,
    /// Days of runway remaining
    pub days_remaining: f64,
    /// Recommended lifecycle state
    pub recommended_state: LifecycleState,
    /// Whether agent can afford another tick
    pub can_tick: bool,
}

/// Lifecycle manager for a sovereign agent
pub struct LifecycleManager {
    current_state: LifecycleState,
    config: LifecycleConfig,
}

impl LifecycleManager {
    /// Create a new lifecycle manager
    pub fn new(initial_state: LifecycleState, config: LifecycleConfig) -> Self {
        Self {
            current_state: initial_state,
            config,
        }
    }

    /// Create with default config
    pub fn with_state(initial_state: LifecycleState) -> Self {
        Self::new(initial_state, LifecycleConfig::default())
    }

    /// Get current state
    pub fn current_state(&self) -> &LifecycleState {
        &self.current_state
    }

    /// Analyze runway and determine recommended state
    pub fn analyze_runway(&self, balance_sats: u64) -> RunwayAnalysis {
        let daily_burn = self.config.daily_burn_sats();

        let days_remaining = if daily_burn > 0 {
            balance_sats as f64 / daily_burn as f64
        } else {
            f64::INFINITY
        };

        let can_tick = balance_sats >= self.config.cost_per_tick_sats;

        let recommended_state = if balance_sats == 0 {
            LifecycleState::Dead
        } else if balance_sats < self.config.hibernate_threshold_sats {
            LifecycleState::Hibernating
        } else if days_remaining < self.config.low_balance_days {
            LifecycleState::LowBalance
        } else {
            LifecycleState::Active
        };

        RunwayAnalysis {
            balance_sats,
            daily_burn_sats: daily_burn,
            days_remaining,
            recommended_state,
            can_tick,
        }
    }

    /// Check if a transition is valid
    pub fn is_valid_transition(&self, to: &LifecycleState) -> bool {
        match (&self.current_state, to) {
            // Spawning can go to Active (funded) or Dead (failed)
            (LifecycleState::Spawning, LifecycleState::Active) => true,
            (LifecycleState::Spawning, LifecycleState::Dead) => true,

            // Active can go to LowBalance, Hibernating, or Dead
            (LifecycleState::Active, LifecycleState::LowBalance) => true,
            (LifecycleState::Active, LifecycleState::Hibernating) => true,
            (LifecycleState::Active, LifecycleState::Dead) => true,

            // LowBalance can go back to Active (funded) or forward
            (LifecycleState::LowBalance, LifecycleState::Active) => true,
            (LifecycleState::LowBalance, LifecycleState::Hibernating) => true,
            (LifecycleState::LowBalance, LifecycleState::Dead) => true,

            // Hibernating can go back to Active (funded) or to Dead
            (LifecycleState::Hibernating, LifecycleState::Active) => true,
            (LifecycleState::Hibernating, LifecycleState::LowBalance) => true,
            (LifecycleState::Hibernating, LifecycleState::Dead) => true,

            // Dead is terminal - no transitions allowed
            (LifecycleState::Dead, _) => false,

            // Same state is always valid (no-op)
            (a, b) if a == b => true,

            _ => false,
        }
    }

    /// Attempt to transition to a new state
    pub fn transition(&mut self, to: LifecycleState) -> Result<(), LifecycleError> {
        if self.current_state == LifecycleState::Dead {
            return Err(LifecycleError::AgentDead);
        }

        if !self.is_valid_transition(&to) {
            return Err(LifecycleError::InvalidTransition {
                from: self.current_state.clone(),
                to,
            });
        }

        self.current_state = to;
        Ok(())
    }

    /// Update state based on current balance
    pub fn update_from_balance(&mut self, balance_sats: u64) -> Result<&LifecycleState, LifecycleError> {
        let analysis = self.analyze_runway(balance_sats);

        if analysis.recommended_state != self.current_state {
            self.transition(analysis.recommended_state)?;
        }

        Ok(&self.current_state)
    }

    /// Check if agent should run a tick
    pub fn should_tick(&self, balance_sats: u64) -> bool {
        match self.current_state {
            LifecycleState::Active | LifecycleState::LowBalance => {
                balance_sats >= self.config.cost_per_tick_sats
            }
            LifecycleState::Hibernating => {
                // Only tick on zaps (external trigger)
                false
            }
            _ => false,
        }
    }

    /// Check if agent should only respond to zaps (hibernating behavior)
    pub fn zaps_only(&self) -> bool {
        matches!(self.current_state, LifecycleState::Hibernating)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lifecycle_transitions() {
        let mut manager = LifecycleManager::with_state(LifecycleState::Spawning);

        // Spawning -> Active
        assert!(manager.transition(LifecycleState::Active).is_ok());
        assert_eq!(manager.current_state, LifecycleState::Active);

        // Active -> LowBalance
        assert!(manager.transition(LifecycleState::LowBalance).is_ok());

        // LowBalance -> Active (funded)
        assert!(manager.transition(LifecycleState::Active).is_ok());

        // Active -> Dead
        assert!(manager.transition(LifecycleState::Dead).is_ok());

        // Dead -> anything should fail
        assert!(manager.transition(LifecycleState::Active).is_err());
    }

    #[test]
    fn test_runway_analysis() {
        let manager = LifecycleManager::new(
            LifecycleState::Active,
            LifecycleConfig {
                low_balance_days: 7.0,
                hibernate_threshold_sats: 1000,
                cost_per_tick_sats: 100,
                ticks_per_day: 10.0, // 10 ticks/day for easy math
            },
        );

        // 7000 sats = 7 days at 1000/day = exactly at threshold
        let analysis = manager.analyze_runway(7000);
        assert_eq!(analysis.days_remaining, 7.0);
        assert_eq!(analysis.recommended_state, LifecycleState::Active);

        // 6000 sats = 6 days = LowBalance
        let analysis = manager.analyze_runway(6000);
        assert!(analysis.days_remaining < 7.0);
        assert_eq!(analysis.recommended_state, LifecycleState::LowBalance);

        // 500 sats = hibernating
        let analysis = manager.analyze_runway(500);
        assert_eq!(analysis.recommended_state, LifecycleState::Hibernating);

        // 0 sats = dead
        let analysis = manager.analyze_runway(0);
        assert_eq!(analysis.recommended_state, LifecycleState::Dead);
    }

    #[test]
    fn test_should_tick() {
        let mut manager = LifecycleManager::new(
            LifecycleState::Active,
            LifecycleConfig {
                cost_per_tick_sats: 100,
                ..Default::default()
            },
        );

        // Active with enough balance
        assert!(manager.should_tick(100));
        assert!(manager.should_tick(1000));
        assert!(!manager.should_tick(50));

        // Hibernating - only zaps
        manager.transition(LifecycleState::Hibernating).unwrap();
        assert!(!manager.should_tick(10000)); // Won't tick even with balance
        assert!(manager.zaps_only());
    }

    #[test]
    fn test_update_from_balance() {
        let mut manager = LifecycleManager::with_state(LifecycleState::Spawning);

        // Fund the agent
        manager.update_from_balance(100_000).unwrap();
        assert_eq!(manager.current_state, LifecycleState::Active);

        // Drain to low balance
        manager.update_from_balance(500).unwrap();
        assert_eq!(manager.current_state, LifecycleState::Hibernating);

        // Completely drain
        manager.update_from_balance(0).unwrap();
        assert_eq!(manager.current_state, LifecycleState::Dead);
    }
}
