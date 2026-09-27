//! Retry and timeout policy.
use std::fmt;
use std::time::Duration;

/// The longest backoff ladder a policy accepts.
const MAX_LADDER: usize = 32;

/// Timeouts and the retry ladder one supervisor follows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Policy {
    /// How long an attempt to open or replace a connection may take.
    pub establish_timeout: Duration,
    /// How long a probe of an existing connection may take.
    pub probe_timeout: Duration,
    /// Waits after consecutive failures. The last step repeats, so it is the
    /// cap.
    pub ladder: Vec<Duration>,
    /// How long a connection must stay up before the ladder starts over.
    pub stable_after: Duration,
    /// A background at least this long replaces the connection on return
    /// instead of probing it.
    pub long_background: Duration,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            establish_timeout: Duration::from_secs(10),
            probe_timeout: Duration::from_secs(3),
            ladder: [1, 2, 4, 8, 16, 30]
                .into_iter()
                .map(Duration::from_secs)
                .collect(),
            stable_after: Duration::from_secs(30),
            long_background: Duration::from_secs(300),
        }
    }
}

/// Why a policy was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyError {
    /// A timeout is zero, so no attempt could complete.
    ZeroTimeout,
    /// The ladder is empty or longer than 32 steps.
    Ladder,
    /// A ladder step is shorter than the step before it.
    Decreasing,
}

impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ZeroTimeout => "establish and probe timeouts must be greater than zero",
            Self::Ladder => "the backoff ladder must have 1 to 32 steps",
            Self::Decreasing => "each backoff step must be at least as long as the step before it",
        })
    }
}

impl std::error::Error for PolicyError {}

impl Policy {
    /// Checks that the policy can drive a supervisor.
    pub fn validate(&self) -> Result<(), PolicyError> {
        if self.establish_timeout.is_zero() || self.probe_timeout.is_zero() {
            return Err(PolicyError::ZeroTimeout);
        }
        if self.ladder.is_empty() || self.ladder.len() > MAX_LADDER {
            return Err(PolicyError::Ladder);
        }
        if self.ladder.windows(2).any(|pair| pair[1] < pair[0]) {
            return Err(PolicyError::Decreasing);
        }
        Ok(())
    }

    /// Returns the wait for ladder position `step`, capped at the last step.
    pub(crate) fn delay(&self, step: usize) -> Duration {
        self.ladder[step.min(self.ladder.len() - 1)]
    }
}
