import XCTest
@testable import OpenAgentsCore

final class ErrorRecoveryOrchestratorTests: XCTestCase {
    func testShouldRetry_defaultFalse() {
        struct Dummy: Error {}
        let ero = ErrorRecoveryOrchestrator()
        XCTAssertFalse(ero.shouldRetry(after: Dummy(), attempt: 1))
    }
}

