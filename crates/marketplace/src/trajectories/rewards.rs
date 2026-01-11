//! Reward calculation for trajectory contributions

use super::{TrajectorySession, validate::QualityScore};
use serde::{Deserialize, Serialize};

/// Reward information for a trajectory contribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardInfo {
    /// Base reward in sats
    pub base_sats: u64,

    /// Quality bonus in sats
    pub quality_bonus_sats: u64,

    /// CI signal bonus in sats (if CI/CD result available)
    pub ci_bonus_sats: u64,

    /// Complexity bonus in sats
    pub complexity_bonus_sats: u64,

    /// Total reward in sats
    pub total_sats: u64,
}

/// Calculator for trajectory contribution rewards
pub struct RewardCalculator {
    /// Base reward per trajectory (sats)
    base_reward: u64,

    /// Bonus per quality point above minimum (sats)
    quality_bonus_per_point: u64,

    /// Bonus for CI/CD signal presence (sats)
    ci_signal_bonus: u64,

    /// Bonus per 1000 tokens (sats)
    token_bonus_rate: u64,

    /// Bonus per tool call (sats)
    tool_call_bonus: u64,
}

impl Default for RewardCalculator {
    fn default() -> Self {
        Self {
            base_reward: 100,            // 100 sats base
            quality_bonus_per_point: 50, // Up to 50 sats for quality above minimum
            ci_signal_bonus: 200,        // 200 sats if CI/CD result present
            token_bonus_rate: 10,        // 10 sats per 1000 tokens
            tool_call_bonus: 5,          // 5 sats per tool call
        }
    }
}

impl RewardCalculator {
    /// Create a new reward calculator with custom rates
    pub fn new(
        base_reward: u64,
        quality_bonus_per_point: u64,
        ci_signal_bonus: u64,
        token_bonus_rate: u64,
        tool_call_bonus: u64,
    ) -> Self {
        Self {
            base_reward,
            quality_bonus_per_point,
            ci_signal_bonus,
            token_bonus_rate,
            tool_call_bonus,
        }
    }

    /// Calculate reward for a trajectory
    pub fn calculate_reward(
        &self,
        session: &TrajectorySession,
        quality: QualityScore,
        min_quality: f64,
    ) -> RewardInfo {
        // Base reward
        let base_sats = self.base_reward;

        // Quality bonus (for quality above minimum threshold)
        let quality_above_min = (quality.value() - min_quality).max(0.0);
        let quality_bonus_sats = (quality_above_min * self.quality_bonus_per_point as f64) as u64;

        // CI signal bonus
        let ci_bonus_sats = if session.ci_passed.is_some() {
            self.ci_signal_bonus
        } else {
            0
        };

        // Complexity bonus (tokens + tool calls)
        let token_bonus = (session.token_count / 1000) as u64 * self.token_bonus_rate;
        let tool_bonus = session.tool_calls as u64 * self.tool_call_bonus;
        let complexity_bonus_sats = token_bonus + tool_bonus;

        // Total
        let total_sats = base_sats + quality_bonus_sats + ci_bonus_sats + complexity_bonus_sats;

        RewardInfo {
            base_sats,
            quality_bonus_sats,
            ci_bonus_sats,
            complexity_bonus_sats,
            total_sats,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_basic_reward_calculation() {
        let calculator = RewardCalculator::default();

        let session = TrajectorySession {
            session_id: "test".to_string(),
            source: "codex".to_string(),
            path: "/tmp/test.rlog".into(),
            initial_commit: Some("abc".to_string()),
            final_commit: Some("def".to_string()),
            ci_passed: Some(true),
            started_at: Utc::now(),
            ended_at: Some(Utc::now()),
            token_count: 2000,
            tool_calls: 15,
            quality_score: 0.8,
        };

        let reward = calculator.calculate_reward(&session, QualityScore::new(0.8), 0.5);

        assert_eq!(reward.base_sats, 100);
        assert!(reward.ci_bonus_sats > 0);
        assert!(reward.complexity_bonus_sats > 0);
        assert!(reward.total_sats > reward.base_sats);
    }

    #[test]
    fn test_reward_without_ci_signal() {
        let calculator = RewardCalculator::default();

        let session = TrajectorySession {
            session_id: "test".to_string(),
            source: "codex".to_string(),
            path: "/tmp/test.rlog".into(),
            initial_commit: None,
            final_commit: None,
            ci_passed: None,
            started_at: Utc::now(),
            ended_at: None,
            token_count: 1000,
            tool_calls: 5,
            quality_score: 0.6,
        };

        let reward = calculator.calculate_reward(&session, QualityScore::new(0.6), 0.5);

        assert_eq!(reward.ci_bonus_sats, 0);
        assert!(reward.total_sats >= reward.base_sats);
    }
}
