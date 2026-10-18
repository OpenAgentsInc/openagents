//! Retry policy shared by control-plane and S3 requests.

use std::time::Duration;

use rand::Rng;

/// How a request may be repeated after a failure.
///
/// * `Transient` repeats on transport errors and 429/5xx, so it fits reads and
///   idempotent writes only.
/// * `Connect` repeats only when no connection was established, so the request
///   never reached the server; creations, execs and deletes use it, since
///   replaying them could duplicate work or report a lost success as a 404.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RetryMode {
    Connect,
    Transient,
}

/// Maximum number of repeats after the first attempt.
pub const MAX_RETRIES: u32 = 3;

const RETRY_BASE_MS: u64 = 100;
const RETRY_CAP_MS: u64 = 2_000;

pub(crate) fn is_transient_status(status: u16) -> bool {
    matches!(status, 429 | 500 | 502 | 503 | 504)
}

/// Full-jitter exponential backoff: uniform in `[0, min(2s, 100ms * 2^attempt))`.
pub(crate) fn retry_delay(attempt: u32) -> Duration {
    let ceiling = RETRY_CAP_MS.min(RETRY_BASE_MS.saturating_mul(1u64 << attempt.min(16)));
    let ms = rand::rng().random_range(0..=ceiling);
    Duration::from_millis(ms)
}

pub(crate) async fn retry_sleep(attempt: u32) {
    tokio::time::sleep(retry_delay(attempt)).await;
}

/// Whether a reqwest error means no connection was established (DNS failure,
/// refused/unreachable, TLS handshake failure, connect timeout).
pub(crate) fn is_connection_establishment_error(error: &reqwest::Error) -> bool {
    error.is_connect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delay_never_exceeds_the_attempt_ceiling() {
        for attempt in 0..8 {
            let ceiling = Duration::from_millis(RETRY_CAP_MS.min(RETRY_BASE_MS << attempt));
            for _ in 0..200 {
                assert!(retry_delay(attempt) <= ceiling, "attempt {attempt}");
            }
        }
        assert_eq!(RETRY_CAP_MS, 2_000);
        assert!(retry_delay(1000) <= Duration::from_millis(RETRY_CAP_MS));
    }

    #[test]
    fn transient_statuses_match_the_ts_sdk() {
        for s in [429, 500, 502, 503, 504] {
            assert!(is_transient_status(s));
        }
        for s in [200, 400, 404, 409, 501, 505] {
            assert!(!is_transient_status(s));
        }
    }
}
