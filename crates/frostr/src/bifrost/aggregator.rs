//! Share aggregation for threshold ECDH operations
//!
//! This module implements the aggregation logic for combining partial
//! ECDH shares from threshold peers.
//!
//! Note: Signing aggregation is handled inline in the two-phase protocol
//! (see `BifrostNode::sign()`). This module only handles ECDH aggregation.

use crate::ecdh::EcdhShare;
use crate::{Error, Result};
use std::collections::BTreeMap;

/// Aggregator for threshold ECDH operations
///
/// Collects partial ECDH points from threshold participants and combines
/// them to derive a shared secret. Uses the Lagrange interpolation approach
/// implemented in `crate::ecdh::combine_ecdh_shares`.
///
/// # Examples
///
/// ```
/// use frostr::bifrost::EcdhAggregator;
///
/// // Create aggregator for 2-of-3 threshold
/// let mut aggregator = EcdhAggregator::new(2, "ecdh-session-456".to_string());
///
/// // Check if ready (need 2 responses)
/// assert!(!aggregator.is_ready());
/// assert_eq!(aggregator.response_count(), 0);
///
/// // Add partial ECDH results from peers (33-byte compressed points)
/// let partial1 = [0x02; 33]; // Example compressed point
/// let partial2 = [0x02; 33];
/// aggregator.add_response(1, partial1).unwrap();
/// aggregator.add_response(2, partial2).unwrap();
///
/// // Now ready to aggregate
/// assert!(aggregator.is_ready());
/// assert_eq!(aggregator.response_count(), 2);
///
/// // Aggregate to get shared secret
/// // let shared_secret = aggregator.aggregate()?;
/// ```
pub struct EcdhAggregator {
    /// Required number of ECDH shares (threshold k)
    threshold: usize,
    /// Session ID for this ECDH round (reserved for future validation)
    #[allow(dead_code)]
    session_id: String,
    /// Collected partial ECDH points by participant ID (33-byte compressed points)
    partial_ecdh: BTreeMap<u8, [u8; 33]>,
}

impl EcdhAggregator {
    /// Create a new ECDH aggregator
    pub fn new(threshold: usize, session_id: String) -> Self {
        Self {
            threshold,
            session_id,
            partial_ecdh: BTreeMap::new(),
        }
    }

    /// Add a partial ECDH response (33-byte compressed point)
    pub fn add_response(&mut self, participant_id: u8, partial: [u8; 33]) -> Result<()> {
        self.partial_ecdh.insert(participant_id, partial);
        Ok(())
    }

    /// Check if we have enough responses to aggregate
    pub fn is_ready(&self) -> bool {
        self.partial_ecdh.len() >= self.threshold
    }

    /// Get the number of responses collected
    pub fn response_count(&self) -> usize {
        self.partial_ecdh.len()
    }

    /// Aggregate partial ECDH results into a shared secret
    ///
    /// Combines the partial ECDH points from threshold participants using
    /// elliptic curve point addition. The resulting point's x-coordinate
    /// is the shared secret.
    ///
    /// # Returns
    /// 32-byte shared secret (x-coordinate of combined point)
    pub fn aggregate(&self) -> Result<[u8; 32]> {
        if !self.is_ready() {
            return Err(Error::Protocol(format!(
                "Not enough responses: need {}, have {}",
                self.threshold,
                self.partial_ecdh.len()
            )));
        }

        // Convert collected partial points to EcdhShare format
        let ecdh_shares: Vec<EcdhShare> = self
            .partial_ecdh
            .iter()
            .map(|(&participant_id, &partial_point)| EcdhShare {
                index: participant_id as u16,
                partial_point,
            })
            .collect();

        // Use the existing combine_ecdh_shares implementation
        crate::ecdh::combine_ecdh_shares(&ecdh_shares)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ecdh_aggregator_new() {
        let agg = EcdhAggregator::new(2, "ecdh-session".to_string());
        assert_eq!(agg.threshold, 2);
        assert_eq!(agg.session_id, "ecdh-session");
        assert!(!agg.is_ready());
        assert_eq!(agg.response_count(), 0);
    }

    #[test]
    fn test_ecdh_aggregator_add_response() {
        let mut agg = EcdhAggregator::new(2, "ecdh-session".to_string());

        agg.add_response(1, [0x02; 33]).unwrap();
        assert_eq!(agg.response_count(), 1);
        assert!(!agg.is_ready());

        agg.add_response(2, [0x02; 33]).unwrap();
        assert_eq!(agg.response_count(), 2);
        assert!(agg.is_ready());
    }

    #[test]
    fn test_ecdh_aggregator_threshold() {
        let mut agg = EcdhAggregator::new(3, "ecdh-session".to_string());

        // Add 2 responses - not enough (use 0x02 prefix for compressed points)
        for i in 1..=2u8 {
            let mut point = [0x02; 33];
            point[1] = i;
            agg.add_response(i, point).unwrap();
        }
        assert!(!agg.is_ready());

        // Add 3rd response - now ready
        let mut point = [0x02; 33];
        point[1] = 3;
        agg.add_response(3, point).unwrap();
        assert!(agg.is_ready());
    }

    #[test]
    fn test_ecdh_aggregator_aggregate() {
        let mut agg = EcdhAggregator::new(2, "ecdh-session".to_string());

        // Use compressed points (0x02 prefix with some x-coordinate)
        // [0x02; 33] happens to decode as a valid curve point
        agg.add_response(1, [0x02; 33]).unwrap();
        agg.add_response(2, [0x02; 33]).unwrap();

        assert!(agg.is_ready());
        // Aggregation should succeed now that it's wired to combine_ecdh_shares
        let result = agg.aggregate();
        assert!(result.is_ok());
        // Result is a 32-byte shared secret
        assert_eq!(result.unwrap().len(), 32);
    }

    #[test]
    fn test_ecdh_aggregator_not_ready() {
        let agg = EcdhAggregator::new(3, "ecdh-session".to_string());

        // Try to aggregate without any responses
        let result = agg.aggregate();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Not enough responses")
        );
    }
}
