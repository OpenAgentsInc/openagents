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
                let result = results
                    .iter()
                    .find(|r| r.success)
                    .cloned()
                    .ok_or_else(|| FrlmError::VerificationFailed {
                        reason: "no successful results".to_string(),
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
    fn verify_objective(
        results: &[SubQueryResult],
        schema: &Option<String>,
    ) -> Result<VerifyResult> {
        let successful: Vec<_> = results.iter().filter(|r| r.success).collect();

        if successful.is_empty() {
            return Ok(VerifyResult::failed("no successful results"));
        }

        // For now, just check that the result is valid JSON if schema is specified
        if let Some(_schema) = schema {
            for result in &successful {
                if serde_json::from_str::<serde_json::Value>(&result.content).is_ok() {
                    // TODO: Actual JSON schema validation
                    return Ok(VerifyResult::passed((*result).clone()));
                }
            }
            return Ok(VerifyResult::failed("no results match schema"));
        }

        // No schema - accept first successful
        Ok(VerifyResult::passed((*successful[0]).clone()))
    }

    /// Verify using validator attestation.
    fn verify_validated(
        results: &[SubQueryResult],
        _validator_pubkey: &str,
    ) -> Result<VerifyResult> {
        // TODO: Check for validator attestation in result metadata
        // For now, fall back to accepting first successful result
        let result = results
            .iter()
            .find(|r| r.success)
            .cloned()
            .ok_or_else(|| FrlmError::VerificationFailed {
                reason: "no successful results".to_string(),
            })?;
        Ok(VerifyResult::passed(result))
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
    fn test_similarity_exact() {
        assert_eq!(Verifier::calculate_similarity("hello", "hello"), 1.0);
    }

    #[test]
    fn test_similarity_partial() {
        let sim = Verifier::calculate_similarity("hello world", "hello there");
        assert!(sim > 0.0 && sim < 1.0);
    }
}
