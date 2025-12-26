//! Bid Selection Strategies
//!
//! Strategies for selecting the best provider bid.

use super::ProviderBid;

/// Bid selection strategy
#[derive(Debug, Clone, Default)]
pub enum BidStrategy {
    /// Select the lowest price bid
    #[default]
    LowestPrice,
    /// Select based on best value (price + reputation)
    BestValue,
    /// Select the fastest provider
    Fastest,
    /// Select based on reputation only
    HighestReputation,
    /// Custom weighted scoring
    Custom {
        price_weight: f32,
        time_weight: f32,
        reputation_weight: f32,
    },
}

/// Selected bid with reasoning
#[derive(Debug)]
pub struct BidSelection<'a> {
    pub bid: &'a ProviderBid,
    pub score: f64,
    pub reason: String,
}

impl BidStrategy {
    /// Select the best bid according to this strategy
    pub fn select<'a>(&self, bids: &'a [ProviderBid]) -> Option<&'a ProviderBid> {
        if bids.is_empty() {
            return None;
        }

        match self {
            BidStrategy::LowestPrice => self.select_lowest_price(bids),
            BidStrategy::BestValue => self.select_best_value(bids),
            BidStrategy::Fastest => self.select_fastest(bids),
            BidStrategy::HighestReputation => self.select_highest_reputation(bids),
            BidStrategy::Custom {
                price_weight,
                time_weight,
                reputation_weight,
            } => self.select_custom(bids, *price_weight, *time_weight, *reputation_weight),
        }
    }

    /// Select with full reasoning
    pub fn select_with_reasoning<'a>(&self, bids: &'a [ProviderBid]) -> Option<BidSelection<'a>> {
        let selected = self.select(bids)?;

        let (score, reason) = match self {
            BidStrategy::LowestPrice => {
                let min_price = bids.iter().map(|b| b.amount_sats).min().unwrap_or(0);
                (
                    1.0 / (selected.amount_sats as f64 / 1000.0),
                    format!("Lowest price at {} sats", selected.amount_sats),
                )
            }
            BidStrategy::BestValue => {
                let score = self.calculate_value_score(selected);
                (score, format!("Best value score: {:.2}", score))
            }
            BidStrategy::Fastest => {
                let time = selected.estimated_time_secs.unwrap_or(u32::MAX);
                (
                    1000.0 / time as f64,
                    format!("Fastest at {} seconds", time),
                )
            }
            BidStrategy::HighestReputation => {
                let rep = selected.reputation.unwrap_or(0);
                (rep as f64, format!("Highest reputation: {}", rep))
            }
            BidStrategy::Custom { .. } => {
                let score = self.calculate_custom_score(selected, 0.4, 0.3, 0.3);
                (score, format!("Custom weighted score: {:.2}", score))
            }
        };

        Some(BidSelection {
            bid: selected,
            score,
            reason,
        })
    }

    fn select_lowest_price<'a>(&self, bids: &'a [ProviderBid]) -> Option<&'a ProviderBid> {
        bids.iter().min_by_key(|b| b.amount_sats)
    }

    fn select_best_value<'a>(&self, bids: &'a [ProviderBid]) -> Option<&'a ProviderBid> {
        bids.iter().max_by(|a, b| {
            let score_a = self.calculate_value_score(a);
            let score_b = self.calculate_value_score(b);
            score_a
                .partial_cmp(&score_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    fn calculate_value_score(&self, bid: &ProviderBid) -> f64 {
        // Lower price is better (invert)
        let price_score = 10000.0 / (bid.amount_sats as f64 + 1.0);

        // Higher reputation is better
        let reputation_score = bid.reputation.unwrap_or(50) as f64;

        // Lower time is better (invert)
        let time_score = if let Some(time) = bid.estimated_time_secs {
            1000.0 / (time as f64 + 1.0)
        } else {
            10.0 // Default score when no time estimate
        };

        // Weighted combination
        price_score * 0.5 + reputation_score * 0.35 + time_score * 0.15
    }

    fn select_fastest<'a>(&self, bids: &'a [ProviderBid]) -> Option<&'a ProviderBid> {
        bids.iter()
            .filter(|b| b.estimated_time_secs.is_some())
            .min_by_key(|b| b.estimated_time_secs.unwrap())
            .or_else(|| bids.first())
    }

    fn select_highest_reputation<'a>(&self, bids: &'a [ProviderBid]) -> Option<&'a ProviderBid> {
        bids.iter()
            .filter(|b| b.reputation.is_some())
            .max_by_key(|b| b.reputation.unwrap())
            .or_else(|| self.select_lowest_price(bids))
    }

    fn select_custom<'a>(
        &self,
        bids: &'a [ProviderBid],
        price_weight: f32,
        time_weight: f32,
        reputation_weight: f32,
    ) -> Option<&'a ProviderBid> {
        bids.iter().max_by(|a, b| {
            let score_a =
                self.calculate_custom_score(a, price_weight, time_weight, reputation_weight);
            let score_b =
                self.calculate_custom_score(b, price_weight, time_weight, reputation_weight);
            score_a
                .partial_cmp(&score_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    fn calculate_custom_score(
        &self,
        bid: &ProviderBid,
        price_weight: f32,
        time_weight: f32,
        reputation_weight: f32,
    ) -> f64 {
        let price_score = 10000.0 / (bid.amount_sats as f64 + 1.0);
        let time_score = bid
            .estimated_time_secs
            .map(|t| 1000.0 / (t as f64 + 1.0))
            .unwrap_or(10.0);
        let reputation_score = bid.reputation.unwrap_or(50) as f64;

        price_score * price_weight as f64
            + time_score * time_weight as f64
            + reputation_score * reputation_weight as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_bids() -> Vec<ProviderBid> {
        vec![
            ProviderBid::new("cheap", 500)
                .with_estimated_time(120)
                .with_reputation(60),
            ProviderBid::new("fast", 1000)
                .with_estimated_time(30)
                .with_reputation(70),
            ProviderBid::new("trusted", 800)
                .with_estimated_time(60)
                .with_reputation(95),
        ]
    }

    #[test]
    fn test_lowest_price_strategy() {
        let bids = create_test_bids();
        let strategy = BidStrategy::LowestPrice;
        let selected = strategy.select(&bids).unwrap();

        assert_eq!(selected.provider_pubkey, "cheap");
        assert_eq!(selected.amount_sats, 500);
    }

    #[test]
    fn test_fastest_strategy() {
        let bids = create_test_bids();
        let strategy = BidStrategy::Fastest;
        let selected = strategy.select(&bids).unwrap();

        assert_eq!(selected.provider_pubkey, "fast");
        assert_eq!(selected.estimated_time_secs, Some(30));
    }

    #[test]
    fn test_highest_reputation_strategy() {
        let bids = create_test_bids();
        let strategy = BidStrategy::HighestReputation;
        let selected = strategy.select(&bids).unwrap();

        assert_eq!(selected.provider_pubkey, "trusted");
        assert_eq!(selected.reputation, Some(95));
    }

    #[test]
    fn test_best_value_strategy() {
        let bids = create_test_bids();
        let strategy = BidStrategy::BestValue;
        let selected = strategy.select(&bids).unwrap();

        // "trusted" should win - good balance of price, time, reputation
        assert_eq!(selected.provider_pubkey, "trusted");
    }

    #[test]
    fn test_custom_strategy_price_focused() {
        let bids = create_test_bids();
        let strategy = BidStrategy::Custom {
            price_weight: 0.8,
            time_weight: 0.1,
            reputation_weight: 0.1,
        };
        let selected = strategy.select(&bids).unwrap();

        // Should favor cheap
        assert_eq!(selected.provider_pubkey, "cheap");
    }

    #[test]
    fn test_custom_strategy_reputation_focused() {
        let bids = create_test_bids();
        let strategy = BidStrategy::Custom {
            price_weight: 0.1,
            time_weight: 0.1,
            reputation_weight: 0.8,
        };
        let selected = strategy.select(&bids).unwrap();

        // Should favor trusted
        assert_eq!(selected.provider_pubkey, "trusted");
    }

    #[test]
    fn test_select_with_reasoning() {
        let bids = create_test_bids();
        let strategy = BidStrategy::LowestPrice;
        let selection = strategy.select_with_reasoning(&bids).unwrap();

        assert_eq!(selection.bid.provider_pubkey, "cheap");
        assert!(selection.reason.contains("Lowest price"));
    }

    #[test]
    fn test_empty_bids() {
        let bids: Vec<ProviderBid> = vec![];
        let strategy = BidStrategy::LowestPrice;
        assert!(strategy.select(&bids).is_none());
    }

    #[test]
    fn test_fallback_when_no_time_estimates() {
        let bids = vec![
            ProviderBid::new("a", 500),
            ProviderBid::new("b", 600),
        ];
        let strategy = BidStrategy::Fastest;
        let selected = strategy.select(&bids).unwrap();

        // Falls back to first when no time estimates
        assert_eq!(selected.provider_pubkey, "a");
    }

    #[test]
    fn test_fallback_when_no_reputation() {
        let bids = vec![
            ProviderBid::new("a", 500),
            ProviderBid::new("b", 600),
        ];
        let strategy = BidStrategy::HighestReputation;
        let selected = strategy.select(&bids).unwrap();

        // Falls back to lowest price
        assert_eq!(selected.provider_pubkey, "a");
    }
}
