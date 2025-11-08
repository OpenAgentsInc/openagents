import Foundation

/// Encapsulates reconnect/backoff settings and attempt tracking for MobileWebSocketClient.
/// Maintains the same defaults as the previous inline implementation.
struct ReconnectPolicy {
    // MARK: - Configuration
    var maxRetryAttempts: Int = 5
    var initialRetryDelay: TimeInterval = 1.0
    var maxRetryDelay: TimeInterval = 30.0
    var initialHandshakeTimeout: TimeInterval = 3.0
    var retryHandshakeTimeout: TimeInterval = 10.0

    // MARK: - State
    var retryCount: Int = 0

    // MARK: - Handshake
    /// Compute the handshake timeout based on the current attempt.
    func handshakeTimeoutForCurrentAttempt() -> TimeInterval {
        return ReconnectPolicy.handshakeTimeoutForAttempt(
            retryCount: retryCount,
            initial: initialHandshakeTimeout,
            retry: retryHandshakeTimeout
        )
    }

    static func handshakeTimeoutForAttempt(retryCount: Int, initial: TimeInterval, retry: TimeInterval) -> TimeInterval {
        return (retryCount == 0) ? initial : retry
    }

    // MARK: - Backoff
    /// Increment the attempt counter and return the next backoff delay.
    mutating func registerFailureAndGetDelay() -> TimeInterval {
        retryCount += 1
        return calculateBackoff(for: retryCount)
    }

    /// Calculate exponential backoff delay capped at `maxRetryDelay`.
    func calculateBackoff(for attempt: Int) -> TimeInterval {
        guard attempt > 0 else { return 0 }
        let exponential = initialRetryDelay * pow(2.0, Double(attempt - 1))
        return min(exponential, maxRetryDelay)
    }

    // MARK: - Control
    func canRetry() -> Bool { retryCount < maxRetryAttempts }
    mutating func reset() { retryCount = 0 }
}

