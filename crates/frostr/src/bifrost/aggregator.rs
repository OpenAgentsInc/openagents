//! Share aggregation for threshold operations
//!
//! This module implements the aggregation logic for combining partial
//! signatures and ECDH shares from threshold peers.

use crate::bifrost::serialization::{deserialize_commitments, deserialize_sig_share};
use crate::bifrost::{SignRequest, SignResponse};
use crate::ecdh::EcdhShare;
use crate::keygen::FrostShare;
use crate::signing::aggregate_signatures;
use crate::{Error, Result};
use frost_secp256k1::{Identifier, SigningPackage};
use std::collections::BTreeMap;

/// Aggregator for threshold signing operations
///
/// # Examples
///
/// ```
/// use frostr::bifrost::SigningAggregator;
///
/// // Create aggregator for 2-of-3 threshold
/// let mut aggregator = SigningAggregator::new(2, "session-123".to_string());
///
/// // Check if ready (need 2 responses)
/// assert!(!aggregator.is_ready());
/// assert_eq!(aggregator.response_count(), 0);
///
/// // After adding responses (see SignResponse for details)
/// // aggregator.add_response(response1)?;
/// // aggregator.add_response(response2)?;
/// // assert!(aggregator.is_ready());
/// // let signature = aggregator.aggregate(&request, &frost_share)?;
/// ```
pub struct SigningAggregator {
    /// Required number of signature shares (threshold k)
    threshold: usize,
    /// Session ID for this signing round
    session_id: String,
    /// Collected partial signatures by participant ID
    partial_sigs: BTreeMap<u8, SignResponse>,
}

impl SigningAggregator {
    /// Create a new signing aggregator
    pub fn new(threshold: usize, session_id: String) -> Self {
        Self {
            threshold,
            session_id,
            partial_sigs: BTreeMap::new(),
        }
    }

    /// Add a partial signature response
    pub fn add_response(&mut self, response: SignResponse) -> Result<()> {
        if response.session_id != self.session_id {
            return Err(Error::Protocol(format!(
                "Session ID mismatch: expected {}, got {}",
                self.session_id, response.session_id
            )));
        }

        self.partial_sigs.insert(response.participant_id, response);
        Ok(())
    }

    /// Check if we have enough responses to aggregate
    pub fn is_ready(&self) -> bool {
        // We need (threshold - 1) peer responses since the coordinator
        // contributes their own partial signature separately
        self.partial_sigs.len() >= self.threshold.saturating_sub(1)
    }

    /// Get the number of responses collected
    pub fn response_count(&self) -> usize {
        self.partial_sigs.len()
    }

    /// Aggregate partial signatures into a final signature
    ///
    /// This method:
    /// 1. Converts partial signatures to frost-secp256k1 SignatureShare format
    /// 2. Reconstructs the SigningPackage from the original request
    /// 3. Calls the frost-secp256k1 aggregate function
    /// 4. Returns the final 64-byte Schnorr signature
    ///
    /// # Arguments
    ///
    /// * `request` - The original SignRequest containing initiator's commitment
    /// * `frost_share` - The coordinator's FROST share for verification
    /// * `our_commitment_bytes` - The coordinator's own commitment (already included in request)
    /// * `our_partial_sig_bytes` - The coordinator's own partial signature
    pub fn aggregate(
        &self,
        request: &SignRequest,
        frost_share: &FrostShare,
        our_commitment_bytes: &[u8; 66],
        our_partial_sig_bytes: &[u8; 32],
    ) -> Result<[u8; 64]> {
        if !self.is_ready() {
            return Err(Error::Protocol(format!(
                "Not enough responses: need {}, have {}",
                self.threshold,
                self.partial_sigs.len()
            )));
        }

        // Build map of commitments from all participants
        let mut signing_commitments = BTreeMap::new();
        let mut signature_shares = BTreeMap::new();

        // Add coordinator's own commitment and signature
        let our_id = Identifier::try_from(request.initiator_id as u16)
            .map_err(|e| Error::Protocol(format!("Invalid initiator ID: {:?}", e)))?;
        let our_commitment = deserialize_commitments(our_commitment_bytes)?;
        let our_sig_share = deserialize_sig_share(our_partial_sig_bytes)?;
        signing_commitments.insert(our_id, our_commitment);
        signature_shares.insert(our_id, our_sig_share);

        // Add responses from peers
        for (participant_id, response) in &self.partial_sigs {
            let id = Identifier::try_from(*participant_id as u16)
                .map_err(|e| Error::Protocol(format!("Invalid participant ID {}: {:?}", participant_id, e)))?;

            // Deserialize commitment from response
            let commitment = deserialize_commitments(&response.nonce_commitment)?;
            signing_commitments.insert(id, commitment);

            // Deserialize signature share from response
            let sig_share = deserialize_sig_share(&response.partial_sig)?;
            signature_shares.insert(id, sig_share);
        }

        // Build SigningPackage with all commitments
        let signing_package = SigningPackage::new(signing_commitments, &request.event_hash);

        // Aggregate signatures using the frost_share for the public key package
        let signature = aggregate_signatures(&signing_package, &signature_shares, frost_share)?;

        // Convert to 64-byte BIP-340 format (R.x || s)
        // FROST serializes as: R (33 bytes compressed point) || z (32 bytes scalar)
        // BIP-340 needs: R.x (32 bytes x-coordinate) || s (32 bytes scalar)
        let sig_bytes = signature.serialize()
            .map_err(|e| Error::Encoding(format!("Failed to serialize signature: {:?}", e)))?;

        if sig_bytes.len() != 65 {
            return Err(Error::Encoding(format!(
                "Unexpected signature length: expected 65, got {}",
                sig_bytes.len()
            )));
        }

        let mut result = [0u8; 64];
        // Skip the first byte (compression prefix 0x02 or 0x03) and take x-coordinate (32 bytes)
        result[..32].copy_from_slice(&sig_bytes[1..33]);
        // Copy the scalar z (32 bytes)
        result[32..].copy_from_slice(&sig_bytes[33..65]);

        Ok(result)
    }

    /// Get the collected partial signatures for inspection
    pub fn partial_signatures(&self) -> &BTreeMap<u8, SignResponse> {
        &self.partial_sigs
    }

    /// Get the threshold requirement
    pub fn threshold(&self) -> usize {
        self.threshold
    }
}

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
    use crate::bifrost::SignResponse;

    #[test]
    fn test_signing_aggregator_new() {
        let agg = SigningAggregator::new(2, "test-session".to_string());
        assert_eq!(agg.threshold, 2);
        assert_eq!(agg.session_id, "test-session");
        assert!(!agg.is_ready());
        assert_eq!(agg.response_count(), 0);
    }

    #[test]
    fn test_signing_aggregator_add_response() {
        // For threshold=3, we need 2 peer responses (coordinator contributes 1 separately)
        let mut agg = SigningAggregator::new(3, "test-session".to_string());

        let response1 = SignResponse {
            session_id: "test-session".to_string(),
            participant_id: 1,
            partial_sig: [0x01; 32],
            nonce_commitment: [0x02; 66],
        };

        agg.add_response(response1).unwrap();
        assert_eq!(agg.response_count(), 1);
        assert!(!agg.is_ready()); // 1 < 2 required peer responses

        let response2 = SignResponse {
            session_id: "test-session".to_string(),
            participant_id: 2,
            partial_sig: [0x03; 32],
            nonce_commitment: [0x04; 66],
        };

        agg.add_response(response2).unwrap();
        assert_eq!(agg.response_count(), 2);
        assert!(agg.is_ready()); // 2 >= 2 required peer responses
    }

    #[test]
    fn test_signing_aggregator_session_mismatch() {
        let mut agg = SigningAggregator::new(2, "test-session".to_string());

        let response = SignResponse {
            session_id: "wrong-session".to_string(),
            participant_id: 1,
            partial_sig: [0x01; 32],
            nonce_commitment: [0x02; 66],
        };

        let result = agg.add_response(response);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Session ID mismatch"));
    }

    #[test]
    fn test_signing_aggregator_threshold() {
        // For threshold=4, we need 3 peer responses (coordinator contributes 1 separately)
        let mut agg = SigningAggregator::new(4, "test-session".to_string());

        // Add 2 responses - not enough (need 3)
        for i in 1..=2 {
            let response = SignResponse {
                session_id: "test-session".to_string(),
                participant_id: i,
                partial_sig: [i; 32],
                nonce_commitment: [i; 66],
            };
            agg.add_response(response).unwrap();
        }
        assert!(!agg.is_ready()); // 2 < 3 required peer responses

        // Add 3rd response - now ready
        let response = SignResponse {
            session_id: "test-session".to_string(),
            participant_id: 3,
            partial_sig: [3; 32],
            nonce_commitment: [3; 66],
        };
        agg.add_response(response).unwrap();
        assert!(agg.is_ready()); // 3 >= 3 required peer responses
    }

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
    fn test_signing_aggregator_not_ready() {
        let mut agg = SigningAggregator::new(3, "test-session".to_string());

        // Add only 1 response when threshold is 3
        let response = SignResponse {
            session_id: "test-session".to_string(),
            participant_id: 1,
            partial_sig: [0x01; 32],
            nonce_commitment: [0x02; 66],
        };
        agg.add_response(response).unwrap();

        let request = SignRequest {
            event_hash: [0x42; 32],
            nonce_commitment: [0x99; 66],
            session_id: "test-session".to_string(),
            participants: vec![1, 2, 3],
            initiator_id: 1,
        };

        // Use dummy FrostShare (we just need the structure)
        let shares = crate::keygen::generate_key_shares(2, 3).unwrap();
        // Provide dummy commitment and partial sig (test will fail before using them)
        let dummy_commitment = [0x99; 66];
        let dummy_partial_sig = [0xAB; 32];
        let result = agg.aggregate(&request, &shares[0], &dummy_commitment, &dummy_partial_sig);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Not enough responses"));
    }

    #[test]
    fn test_ecdh_aggregator_not_ready() {
        let agg = EcdhAggregator::new(3, "ecdh-session".to_string());

        // Try to aggregate without any responses
        let result = agg.aggregate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Not enough responses"));
    }
}
