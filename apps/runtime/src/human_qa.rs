use sha2::{Digest, Sha256};

use crate::inference_tiering::InferenceRisk;

#[derive(Debug, Clone)]
pub struct HumanQaSamplingPolicy {
    pub seed: String,
    /// Sample rate in basis points (0-10_000) for low-risk work.
    pub low_risk_bps: u16,
    /// Sample rate in basis points (0-10_000) for medium-risk work.
    pub medium_risk_bps: u16,
    /// Sample rate in basis points (0-10_000) for high-risk work.
    pub high_risk_bps: u16,
}

impl HumanQaSamplingPolicy {
    pub fn should_sample(&self, key: &str, risk: InferenceRisk) -> bool {
        let key = key.trim();
        if key.is_empty() {
            return false;
        }

        let bps = match risk {
            InferenceRisk::Low => self.low_risk_bps,
            InferenceRisk::Medium => self.medium_risk_bps,
            InferenceRisk::High => self.high_risk_bps,
        };

        should_sample_bps(self.seed.as_str(), key, bps)
    }
}

fn should_sample_bps(seed: &str, key: &str, bps: u16) -> bool {
    if bps == 0 {
        return false;
    }
    if bps >= 10_000 {
        return true;
    }

    sample_bucket(seed, key) < bps
}

fn sample_bucket(seed: &str, key: &str) -> u16 {
    // Deterministic bucket in [0, 9999] using SHA-256(seed || ":" || key).
    let mut hasher = Sha256::new();
    hasher.update(seed.as_bytes());
    hasher.update(b":");
    hasher.update(key.as_bytes());
    let digest = hasher.finalize();

    u16::from_be_bytes([digest[0], digest[1]]) % 10_000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bps_sampling_zero_and_full_are_hard_bounds() {
        assert!(!should_sample_bps("seed", "k", 0));
        assert!(should_sample_bps("seed", "k", 10_000));
    }

    #[test]
    fn sampling_is_deterministic_for_same_seed_and_key() {
        let a = should_sample_bps("seed", "key-123", 1234);
        let b = should_sample_bps("seed", "key-123", 1234);
        assert_eq!(a, b);
    }

    #[test]
    fn sampling_bucket_varies_across_seeds_for_some_keys() {
        let keys = (0..64).map(|idx| format!("key-{idx}")).collect::<Vec<_>>();
        let mut any_diff = false;
        for key in &keys {
            if sample_bucket("seed-a", key.as_str()) != sample_bucket("seed-b", key.as_str()) {
                any_diff = true;
                break;
            }
        }
        assert!(any_diff);
    }

    #[test]
    fn policy_uses_risk_specific_rates() {
        let policy = HumanQaSamplingPolicy {
            seed: "seed".to_string(),
            low_risk_bps: 0,
            medium_risk_bps: 10_000,
            high_risk_bps: 0,
        };

        assert!(!policy.should_sample("k", InferenceRisk::Low));
        assert!(policy.should_sample("k", InferenceRisk::Medium));
        assert!(!policy.should_sample("k", InferenceRisk::High));
    }
}
