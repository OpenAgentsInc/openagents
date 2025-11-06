import XCTest
@testable import OpenAgentsCore

final class MobileWebSocketClientTimingTests: XCTestCase {

    func testHandshakeTimeoutInitialAttemptIsShort() {
        let t = MobileWebSocketClient.handshakeTimeoutForAttempt(
            retryCount: 0,
            initial: 3.0,
            retry: 10.0
        )
        XCTAssertEqual(t, 3.0, accuracy: 0.0001)
    }

    func testHandshakeTimeoutRetryAttemptIsLonger() {
        let t1 = MobileWebSocketClient.handshakeTimeoutForAttempt(
            retryCount: 1,
            initial: 3.0,
            retry: 10.0
        )
        let t2 = MobileWebSocketClient.handshakeTimeoutForAttempt(
            retryCount: 5,
            initial: 3.0,
            retry: 10.0
        )
        XCTAssertEqual(t1, 10.0, accuracy: 0.0001)
        XCTAssertEqual(t2, 10.0, accuracy: 0.0001)
    }
}

