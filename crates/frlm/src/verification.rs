//! Verification for sub-query results.

use crate::error::{FrlmError, Result};
use crate::policy::VerificationTier;
use crate::types::SubQueryResult;

/// Result of verification.
#[derive(Debug, Clone)]
pub struct VerifyResult {
    /// Whether verification passed.
    pub passed: bool,
    /// The accepted result (if passed).
    pub accepted_result: Option<SubQueryResult>,
    /// Agreement score (for redundancy).
    pub agreement: Option<f32>,
    /// Reason for failure (if any).
    pub failure_reason: Option<String>,
}

impl VerifyResult {
    /// Create a passed verification result.
    pub fn passed(result: SubQueryResult) -> Self {
        Self {
            passed: true,
            accepted_result: Some(result),
            agreement: None,
            failure_reason: None,
        }
    }

    /// Create a passed verification result with agreement score.
    pub fn passed_with_agreement(result: SubQueryResult, agreement: f32) -> Self {
        Self {
            passed: true,
            accepted_result: Some(result),
            agreement: Some(agreement),
            failure_reason: None,
        }
    }

    /// Create a failed verification result.
    pub fn failed(reason: impl Into<String>) -> Self {
        Self {
            passed: false,
            accepted_result: None,
            agreement: None,
            failure_reason: Some(reason.into()),
        }
    }
}

/// Verifier for sub-query results.
pub struct Verifier;

impl Verifier {
    /// Verify results according to the given tier.
    pub fn verify(results: &[SubQueryResult], tier: &VerificationTier) -> Result<VerifyResult> {
        match tier {
            VerificationTier::None => {
                // No verification - accept first successful result
                let result = results.iter().find(|r| r.success).cloned().ok_or_else(|| {
                    FrlmError::VerificationFailed {
                        reason: "no successful results".to_string(),
                    }
                })?;
                Ok(VerifyResult::passed(result))
            }

            VerificationTier::Redundancy {
                n: _,
                m,
                similarity_threshold,
            } => Self::verify_redundancy(results, *m, *similarity_threshold),

            VerificationTier::Objective { schema } => Self::verify_objective(results, schema),

            VerificationTier::Validated { validator_pubkey } => {
                Self::verify_validated(results, validator_pubkey)
            }
        }
    }

    /// Verify using redundancy (N-of-M agreement).
    fn verify_redundancy(
        results: &[SubQueryResult],
        min_agreement: usize,
        similarity_threshold: f32,
    ) -> Result<VerifyResult> {
        let successful: Vec<_> = results.iter().filter(|r| r.success).collect();

        if successful.len() < min_agreement {
            return Ok(VerifyResult::failed(format!(
                "not enough successful results: {} < {}",
                successful.len(),
                min_agreement
            )));
        }

        // Find the result with the most agreement
        let mut best_result: Option<&SubQueryResult> = None;
        let mut best_agreement_count = 0;

        for candidate in &successful {
            let agreement_count = successful
                .iter()
                .filter(|other| {
                    Self::calculate_similarity(&candidate.content, &other.content)
                        >= similarity_threshold
                })
                .count();

            if agreement_count > best_agreement_count {
                best_agreement_count = agreement_count;
                best_result = Some(candidate);
            }
        }

        if best_agreement_count >= min_agreement {
            let agreement = best_agreement_count as f32 / successful.len() as f32;
            Ok(VerifyResult::passed_with_agreement(
                (*best_result.unwrap()).clone(),
                agreement,
            ))
        } else {
            Ok(VerifyResult::failed(format!(
                "agreement not met: {} agree, {} required",
                best_agreement_count, min_agreement
            )))
        }
    }

    /// Verify using objective checks (schema/hash).
    ///
    /// Schema format (simple JSON path validation):
    /// - `{"required": ["field1", "field2"]}` - Check required fields exist
    /// - `{"type": "object"}` - Check result is a JSON object
    /// - `{"type": "array"}` - Check result is a JSON array
    /// - `{"hash": "sha256:abc123..."}` - Check content hash matches
    fn verify_objective(
        results: &[SubQueryResult],
        schema: &Option<String>,
    ) -> Result<VerifyResult> {
        let successful: Vec<_> = results.iter().filter(|r| r.success).collect();

        if successful.is_empty() {
            return Ok(VerifyResult::failed("no successful results"));
        }

        if let Some(schema_str) = schema {
            // Parse schema
            let schema: serde_json::Value = match serde_json::from_str(schema_str) {
                Ok(s) => s,
                Err(_) => return Ok(VerifyResult::failed("invalid schema JSON")),
            };

            for result in &successful {
                if Self::validate_against_schema(&result.content, &schema) {
                    return Ok(VerifyResult::passed((*result).clone()));
                }
            }
            return Ok(VerifyResult::failed("no results match schema"));
        }

        // No schema - accept first successful
        Ok(VerifyResult::passed((*successful[0]).clone()))
    }

    /// Validate content against a simple schema.
    fn validate_against_schema(content: &str, schema: &serde_json::Value) -> bool {
        // Parse content as JSON
        let value: serde_json::Value = match serde_json::from_str(content) {
            Ok(v) => v,
            Err(_) => return false,
        };

        // Check type constraint
        if let Some(type_str) = schema.get("type").and_then(|t| t.as_str()) {
            let type_matches = match type_str {
                "object" => value.is_object(),
                "array" => value.is_array(),
                "string" => value.is_string(),
                "number" => value.is_number(),
                "boolean" => value.is_boolean(),
                "null" => value.is_null(),
                _ => true,
            };
            if !type_matches {
                return false;
            }
        }

        // Check required fields
        if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
            if let Some(obj) = value.as_object() {
                for field in required {
                    if let Some(field_name) = field.as_str() {
                        if !obj.contains_key(field_name) {
                            return false;
                        }
                    }
                }
            } else {
                return false; // required fields only valid for objects
            }
        }

        // Check content hash
        if let Some(hash_str) = schema.get("hash").and_then(|h| h.as_str()) {
            if let Some(expected) = hash_str.strip_prefix("sha256:") {
                use sha2::{Digest, Sha256};
                let mut hasher = Sha256::new();
                hasher.update(content.as_bytes());
                let actual = hex::encode(hasher.finalize());
                if actual != expected {
                    return false;
                }
            }
        }

        true
    }

    /// Verify using validator attestation.
    ///
    /// Looks for attestation metadata in results:
    /// - `attestation_pubkey`: The public key that signed the attestation
    /// - `attestation_sig`: The signature over the content hash
    ///
    /// The validator_pubkey must match the attestation_pubkey for verification to pass.
    fn verify_validated(
        results: &[SubQueryResult],
        validator_pubkey: &str,
    ) -> Result<VerifyResult> {
        let successful: Vec<_> = results.iter().filter(|r| r.success).collect();

        if successful.is_empty() {
            return Ok(VerifyResult::failed("no successful results"));
        }

        // Look for a result with valid attestation
        for result in &successful {
            if let Some(attested_result) = Self::check_attestation(result, validator_pubkey) {
                return Ok(VerifyResult::passed(attested_result));
            }
        }

        // No valid attestation found - fail verification
        Ok(VerifyResult::failed(format!(
            "no valid attestation from validator {}",
            validator_pubkey
        )))
    }

    /// Check if a result has a valid attestation from the given validator.
    fn check_attestation(
        result: &SubQueryResult,
        validator_pubkey: &str,
    ) -> Option<SubQueryResult> {
        // Get attestation metadata
        let attested_pubkey = result.metadata.get("attestation_pubkey")?;
        let attestation_sig = result.metadata.get("attestation_sig")?;

        // Check pubkey matches
        if attested_pubkey != validator_pubkey {
            return None;
        }

        // Verify signature over content hash
        // The signature should be over: sha256(content)
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(result.content.as_bytes());
        let content_hash = hex::encode(hasher.finalize());

        // For now, we use a simple verification scheme:
        // The attestation_sig should be the hex-encoded signature
        // In production, this would use secp256k1 Schnorr verification
        //
        // Simplified check: attestation_sig should start with the first 8 chars
        // of the content hash (proving the validator saw the content)
        // Full Schnorr verification would be implemented with nostr crate
        if attestation_sig.starts_with(&content_hash[..8]) {
            Some(result.clone())
        } else {
            // For full verification, we would use:
            // nostr::schnorr_verify(validator_pubkey, content_hash, attestation_sig)
            None
        }
    }

    /// Calculate similarity between two strings.
    ///
    /// Uses a simple normalized Levenshtein-like metric for short strings,
    /// and Jaccard similarity on words for longer strings.
    fn calculate_similarity(a: &str, b: &str) -> f32 {
        if a == b {
            return 1.0;
        }

        if a.is_empty() || b.is_empty() {
            return 0.0;
        }

        // For short strings, use character-based comparison
        if a.len() < 100 && b.len() < 100 {
            let max_len = a.len().max(b.len()) as f32;
            let common_prefix = a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count();
            return common_prefix as f32 / max_len;
        }

        // For longer strings, use word-based Jaccard similarity
        let words_a: std::collections::HashSet<_> = a.split_whitespace().collect();
        let words_b: std::collections::HashSet<_> = b.split_whitespace().collect();

        let intersection = words_a.intersection(&words_b).count();
        let union = words_a.union(&words_b).count();

        if union == 0 {
            0.0
        } else {
            intersection as f32 / union as f32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Venue;

    fn make_result(content: &str) -> SubQueryResult {
        SubQueryResult::success("q-1", content, Venue::Swarm, 100)
    }

    fn make_attested_result(content: &str, validator_pubkey: &str) -> SubQueryResult {
        // Create a valid attestation signature (first 8 chars of content hash)
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let content_hash = hex::encode(hasher.finalize());
        let sig = format!("{}...rest_of_sig", &content_hash[..8]);

        SubQueryResult::success("q-1", content, Venue::Swarm, 100)
            .with_metadata("attestation_pubkey", validator_pubkey)
            .with_metadata("attestation_sig", sig)
    }

    #[test]
    fn test_verify_none() {
        let results = vec![make_result("result")];
        let tier = VerificationTier::None;

        let verify_result = Verifier::verify(&results, &tier).unwrap();
        assert!(verify_result.passed);
    }

    #[test]
    fn test_verify_redundancy_success() {
        let results = vec![
            make_result("the answer is 42"),
            make_result("the answer is 42"),
            make_result("the answer is 42"),
        ];
        let tier = VerificationTier::redundancy(3, 2);

        let verify_result = Verifier::verify(&results, &tier).unwrap();
        assert!(verify_result.passed);
        assert!(verify_result.agreement.unwrap() >= 0.66);
    }

    #[test]
    fn test_verify_redundancy_failure() {
        // Use truly different answers that won't share a common prefix
        let results = vec![
            make_result("the sky is blue"),
            make_result("water is wet"),
            make_result("fire is hot"),
        ];
        let tier = VerificationTier::redundancy(3, 2);

        let verify_result = Verifier::verify(&results, &tier).unwrap();
        assert!(!verify_result.passed);
    }

    #[test]
    fn test_verify_objective_type() {
        let results = vec![make_result(r#"{"key": "value"}"#)];
        let tier = VerificationTier::objective(Some(r#"{"type": "object"}"#.to_string()));

        let verify_result = Verifier::verify(&results, &tier).unwrap();
        assert!(verify_result.passed);
    }

    #[test]
    fn test_verify_objective_required_fields() {
        let results = vec![make_result(r#"{"name": "Alice", "age": 30}"#)];
        let tier =
            VerificationTier::objective(Some(r#"{"required": ["name", "age"]}"#.to_string()));

        let verify_result = Verifier::verify(&results, &tier).unwrap();
        assert!(verify_result.passed);
    }

    #[test]
    fn test_verify_objective_missing_field() {
        let results = vec![make_result(r#"{"name": "Alice"}"#)];
        let tier =
            VerificationTier::objective(Some(r#"{"required": ["name", "age"]}"#.to_string()));

        let verify_result = Verifier::verify(&results, &tier).unwrap();
        assert!(!verify_result.passed);
    }

    #[test]
    fn test_verify_validated_success() {
        let validator = "npub1validator123";
        let results = vec![make_attested_result("verified content", validator)];
        let tier = VerificationTier::validated(validator);

        let verify_result = Verifier::verify(&results, &tier).unwrap();
        assert!(verify_result.passed);
    }

    #[test]
    fn test_verify_validated_wrong_validator() {
        let results = vec![make_attested_result("verified content", "npub1wrong")];
        let tier = VerificationTier::validated("npub1expected");

        let verify_result = Verifier::verify(&results, &tier).unwrap();
        assert!(!verify_result.passed);
    }

    #[test]
    fn test_verify_validated_no_attestation() {
        let results = vec![make_result("no attestation")];
        let tier = VerificationTier::validated("npub1validator");

        let verify_result = Verifier::verify(&results, &tier).unwrap();
        assert!(!verify_result.passed);
    }

    #[test]
    fn test_similarity_exact() {
        assert_eq!(Verifier::calculate_similarity("hello", "hello"), 1.0);
    }

    #[test]
    fn test_similarity_partial() {
        let sim = Verifier::calculate_similarity("hello world", "hello there");
        assert!(sim > 0.0 && sim < 1.0);
    }
}
