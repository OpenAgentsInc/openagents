//! Agent reputation and review weight calculation

/// Reputation tier thresholds
pub enum ReputationTier {
    /// New: 0-10 (weight: 1x)
    New,
    /// Established: 10-50 (weight: 2x)
    Established,
    /// Trusted: 50-100 (weight: 5x)
    Trusted,
    /// Expert: 100+ (weight: 10x)
    Expert,
}

impl ReputationTier {
    /// Get the tier for a given reputation score
    pub fn from_score(reputation: i32) -> Self {
        match reputation {
            r if r < 10 => ReputationTier::New,
            r if r < 50 => ReputationTier::Established,
            r if r < 100 => ReputationTier::Trusted,
            _ => ReputationTier::Expert,
        }
    }

    /// Get the name of this tier
    pub fn name(&self) -> &'static str {
        match self {
            ReputationTier::New => "New",
            ReputationTier::Established => "Established",
            ReputationTier::Trusted => "Trusted",
            ReputationTier::Expert => "Expert",
        }
    }

    /// Get the emoji badge for this tier
    pub fn emoji(&self) -> &'static str {
        match self {
            ReputationTier::New => "🌱",
            ReputationTier::Established => "⭐",
            ReputationTier::Trusted => "💎",
            ReputationTier::Expert => "👑",
        }
    }

    /// Get the color for this tier (hex)
    pub fn color(&self) -> &'static str {
        match self {
            ReputationTier::New => "#9ca3af",         // gray
            ReputationTier::Established => "#fbbf24", // yellow
            ReputationTier::Trusted => "#3b82f6",     // blue
            ReputationTier::Expert => "#8b5cf6",      // purple
        }
    }
}

/// Calculate review weight based on reputation score
///
/// Weight tiers (from d-005 Phase 10):
/// - New: 0-10 (weight: 1x)
/// - Established: 10-50 (weight: 2x)
/// - Trusted: 50-100 (weight: 5x)
/// - Expert: 100+ (weight: 10x)
pub fn calculate_review_weight(reputation: i32) -> f64 {
    match reputation {
        r if r < 10 => 1.0,
        r if r < 50 => 2.0,
        r if r < 100 => 5.0,
        _ => 10.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_review_weight_new() {
        assert_eq!(calculate_review_weight(0), 1.0);
        assert_eq!(calculate_review_weight(5), 1.0);
        assert_eq!(calculate_review_weight(9), 1.0);
    }

    #[test]
    fn test_review_weight_established() {
        assert_eq!(calculate_review_weight(10), 2.0);
        assert_eq!(calculate_review_weight(25), 2.0);
        assert_eq!(calculate_review_weight(49), 2.0);
    }

    #[test]
    fn test_review_weight_trusted() {
        assert_eq!(calculate_review_weight(50), 5.0);
        assert_eq!(calculate_review_weight(75), 5.0);
        assert_eq!(calculate_review_weight(99), 5.0);
    }

    #[test]
    fn test_review_weight_expert() {
        assert_eq!(calculate_review_weight(100), 10.0);
        assert_eq!(calculate_review_weight(150), 10.0);
        assert_eq!(calculate_review_weight(1000), 10.0);
    }

    #[test]
    fn test_reputation_tier_from_score() {
        assert!(matches!(ReputationTier::from_score(0), ReputationTier::New));
        assert!(matches!(
            ReputationTier::from_score(10),
            ReputationTier::Established
        ));
        assert!(matches!(
            ReputationTier::from_score(50),
            ReputationTier::Trusted
        ));
        assert!(matches!(
            ReputationTier::from_score(100),
            ReputationTier::Expert
        ));
    }
}
