use super::DomainError;

/// Admission policy for NIP-01 `created_at` timestamps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimestampPolicy {
    max_future_seconds: u64,
}

impl TimestampPolicy {
    pub const fn new(max_future_seconds: u64) -> Self {
        Self { max_future_seconds }
    }

    pub const fn max_future_seconds(self) -> u64 {
        self.max_future_seconds
    }

    pub fn validate(self, created_at: u64, now: u64) -> Result<(), DomainError> {
        let latest_allowed = now.saturating_add(self.max_future_seconds);
        if created_at > latest_allowed {
            Err(DomainError::FutureTimestamp {
                created_at,
                latest_allowed,
            })
        } else {
            Ok(())
        }
    }
}
