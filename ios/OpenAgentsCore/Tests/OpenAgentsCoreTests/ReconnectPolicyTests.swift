import XCTest
@testable import OpenAgentsCore

final class ReconnectPolicyTests: XCTestCase {
    func testHandshakeTimeout_FirstAttempt_UsesInitial() {
        var p = ReconnectPolicy()
        p.initialHandshakeTimeout = 2.5
        p.retryHandshakeTimeout = 9.0
        XCTAssertEqual(p.handshakeTimeoutForCurrentAttempt(), 2.5)
    }

    func testHandshakeTimeout_RetryAttempt_UsesRetry() {
        var p = ReconnectPolicy()
        p.retryCount = 1
        p.initialHandshakeTimeout = 2.5
        p.retryHandshakeTimeout = 9.0
        XCTAssertEqual(p.handshakeTimeoutForCurrentAttempt(), 9.0)
    }

    func testBackoff_GrowsExponentially_AndCapsAtMax() {
        var p = ReconnectPolicy()
        p.initialRetryDelay = 1.0
        p.maxRetryDelay = 8.0
        // attempts: 1->1, 2->2, 3->4, 4->8, 5->cap at 8
        XCTAssertEqual(p.calculateBackoff(for: 1), 1.0)
        XCTAssertEqual(p.calculateBackoff(for: 2), 2.0)
        XCTAssertEqual(p.calculateBackoff(for: 3), 4.0)
        XCTAssertEqual(p.calculateBackoff(for: 4), 8.0)
        XCTAssertEqual(p.calculateBackoff(for: 5), 8.0)
    }

    func testRegisterFailureIncrementsAndReturnsDelay() {
        var p = ReconnectPolicy()
        p.initialRetryDelay = 0.5
        XCTAssertEqual(p.retryCount, 0)
        let d1 = p.registerFailureAndGetDelay()
        XCTAssertEqual(p.retryCount, 1)
        XCTAssertEqual(d1, 0.5)
        let d2 = p.registerFailureAndGetDelay()
        XCTAssertEqual(p.retryCount, 2)
        XCTAssertEqual(d2, 1.0)
    }

    func testCanRetry_AndReset() {
        var p = ReconnectPolicy()
        p.maxRetryAttempts = 2
        XCTAssertTrue(p.canRetry())
        _ = p.registerFailureAndGetDelay()
        XCTAssertTrue(p.canRetry())
        _ = p.registerFailureAndGetDelay()
        XCTAssertFalse(p.canRetry())
        p.reset()
        XCTAssertEqual(p.retryCount, 0)
        XCTAssertTrue(p.canRetry())
    }
}

