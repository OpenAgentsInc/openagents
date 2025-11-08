import Foundation

/// Placeholder error recovery orchestrator. Can be extended for retries/fallbacks.
struct ErrorRecoveryOrchestrator {
    func shouldRetry(after error: Error, attempt: Int) -> Bool { return false }
}

