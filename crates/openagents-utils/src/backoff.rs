//! Shared exponential backoff utilities with optional jitter.

use rand::Rng;
use std::time::Duration;

/// Jitter strategy to apply to backoff delays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Jitter {
    /// No jitter; always return the capped exponential delay.
    None,
    /// Full jitter; return a random delay in `[0, capped_delay]`.
    Full,
}

/// Exponential backoff calculator.
#[derive(Clone, Debug)]
pub struct ExponentialBackoff {
    base_delay: Duration,
    max_delay: Duration,
    max_attempts: Option<u32>,
    jitter: Jitter,
    attempt: u32,
}

impl ExponentialBackoff {
    /// Create a new backoff sequence.
    ///
    /// `max_attempts` of 0 means unlimited attempts.
    pub fn new(base_delay: Duration, max_delay: Duration, max_attempts: u32) -> Self {
        Self {
            base_delay,
            max_delay,
            max_attempts: if max_attempts == 0 {
                None
            } else {
                Some(max_attempts)
            },
            jitter: Jitter::Full,
            attempt: 0,
        }
    }

    /// Override the jitter strategy (defaults to `Full`).
    pub fn with_jitter(mut self, jitter: Jitter) -> Self {
        self.jitter = jitter;
        self
    }

    /// Override the maximum attempts (0 = unlimited).
    pub fn with_max_attempts(mut self, max_attempts: u32) -> Self {
        self.max_attempts = if max_attempts == 0 {
            None
        } else {
            Some(max_attempts)
        };
        self
    }

    /// Get the next delay in the sequence.
    ///
    /// Returns `None` if the backoff is exhausted.
    pub fn next_delay(&mut self) -> Option<Duration> {
        if self.max_attempts.map_or(false, |max| self.attempt >= max) {
            return None;
        }

        // capped_delay_ms = min(base * 2^attempt, max_delay)
        let base_ms = self.base_delay.as_millis() as u128;
        let max_ms = self.max_delay.as_millis() as u128;
        let shift = self.attempt.min(63);
        let multiplier = 1u128
            .checked_shl(shift)
            .unwrap_or(u128::MAX);
        let exp_ms = base_ms.saturating_mul(multiplier);
        let capped_ms = std::cmp::min(exp_ms, max_ms);

        let jitter_ms = match self.jitter {
            Jitter::None => capped_ms,
            Jitter::Full => {
                let capped_u64 = capped_ms.min(u64::MAX as u128) as u64;
                rand::thread_rng().gen_range(0..=capped_u64) as u128
            }
        };

        self.attempt = self.attempt.saturating_add(1);
        Some(Duration::from_millis(
            jitter_ms.min(u64::MAX as u128) as u64,
        ))
    }

    /// Reset the attempt counter.
    pub fn reset(&mut self) {
        self.attempt = 0;
    }

    /// Current attempt count (number of delays generated so far).
    pub fn attempt(&self) -> u32 {
        self.attempt
    }

    /// Whether the backoff has reached its maximum attempts.
    pub fn is_exhausted(&self) -> bool {
        self.max_attempts
            .map_or(false, |max| self.attempt >= max)
    }
}

impl Default for ExponentialBackoff {
    fn default() -> Self {
        Self::new(Duration::from_secs(1), Duration::from_secs(60), 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_jitter_is_deterministic() {
        let mut backoff = ExponentialBackoff::new(
            Duration::from_millis(100),
            Duration::from_millis(1000),
            3,
        )
        .with_jitter(Jitter::None);

        assert_eq!(backoff.next_delay().unwrap(), Duration::from_millis(100));
        assert_eq!(backoff.next_delay().unwrap(), Duration::from_millis(200));
        assert_eq!(backoff.next_delay().unwrap(), Duration::from_millis(400));
        assert!(backoff.next_delay().is_none());
    }

    #[test]
    fn jittered_delay_caps_to_max() {
        let mut backoff = ExponentialBackoff::new(
            Duration::from_millis(500),
            Duration::from_millis(600),
            2,
        );
        let first = backoff.next_delay().unwrap();
        assert!(first <= Duration::from_millis(600));
        let second = backoff.next_delay().unwrap();
        assert!(second <= Duration::from_millis(600));
        assert!(backoff.next_delay().is_none());
    }

    #[test]
    fn reset_clears_attempts() {
        let mut backoff = ExponentialBackoff::default().with_jitter(Jitter::None);
        assert!(backoff.next_delay().is_some());
        assert_eq!(backoff.attempt(), 1);
        backoff.reset();
        assert_eq!(backoff.attempt(), 0);
    }
}
