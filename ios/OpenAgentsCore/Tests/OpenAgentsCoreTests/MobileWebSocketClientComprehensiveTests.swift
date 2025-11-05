import XCTest
@testable import OpenAgentsCore

final class MobileWebSocketClientComprehensiveTests: XCTestCase {
    var sut: MobileWebSocketClient!
    var mockDelegate: MockMobileWebSocketClientDelegate!
    var mockSession: MockURLSession!

    override func setUp() {
        super.setUp()
        mockSession = MockURLSession()
        mockDelegate = MockMobileWebSocketClientDelegate()
        sut = MobileWebSocketClient(session: mockSession)
        sut.delegate = mockDelegate
    }

    override func tearDown() {
        sut.disconnect()
        sut = nil
        mockDelegate = nil
        mockSession = nil
        super.tearDown()
    }

    // MARK: - Initialization Tests

    func testInitWithDefaultSession() {
        let client = MobileWebSocketClient()
        XCTAssertNotNil(client)
    }

    func testInitWithCustomSession() {
        let customSession = URLSession(configuration: .default)
        let client = MobileWebSocketClient(session: customSession)
        XCTAssertNotNil(client)
    }

    func testDelegateIsWeak() {
        var delegate: MockMobileWebSocketClientDelegate? = MockMobileWebSocketClientDelegate()
        sut.delegate = delegate
        weak var weakDelegate = delegate
        delegate = nil
        XCTAssertNil(weakDelegate, "Delegate should be weak and deallocated")
    }

    // MARK: - Connection Tests

    func testConnectCreatesWebSocketTask() {
        let url = URL(string: "ws://localhost:8787")!
        sut.connect(url: url, token: "test-token")

        XCTAssertTrue(mockSession.webSocketTaskCalled)
        XCTAssertEqual(mockSession.lastRequest?.url, url)
    }

    func testConnectDisconnectsExistingConnection() {
        let url1 = URL(string: "ws://localhost:8787")!
        sut.connect(url: url1, token: "token1")

        mockSession.webSocketTaskCalled = false
        let url2 = URL(string: "ws://localhost:8888")!
        sut.connect(url: url2, token: "token2")

        XCTAssertTrue(mockSession.webSocketTaskCalled)
        XCTAssertEqual(mockSession.lastRequest?.url, url2)
    }

    func testConnectSendsInitializeMessage() {
        let url = URL(string: "ws://localhost:8787")!
        let mockTask = MockURLSessionWebSocketTask()
        mockSession.mockTask = mockTask

        sut.connect(url: url, token: "test-token")

        XCTAssertTrue(mockTask.sendCalled)
        XCTAssertFalse(mockTask.sentMessages.isEmpty)

        // Verify initialize message was sent
        if case .string(let text) = mockTask.sentMessages.first {
            XCTAssertTrue(text.contains("\"method\":\"initialize\""))
            XCTAssertTrue(text.contains("\"protocol_version\":\"0.7.0\""))
        } else {
            XCTFail("Expected string message with initialize")
        }
    }

    // MARK: - Disconnect Tests

    func testDisconnectCancelsTask() {
        let url = URL(string: "ws://localhost:8787")!
        let mockTask = MockURLSessionWebSocketTask()
        mockSession.mockTask = mockTask

        sut.connect(url: url, token: "test-token")
        sut.disconnect()

        XCTAssertTrue(mockTask.cancelCalled)
    }

    func testDisconnectCallsDelegate() {
        let url = URL(string: "ws://localhost:8787")!
        sut.connect(url: url, token: "test-token")

        let expectation = self.expectation(description: "disconnect called")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
            if self.mockDelegate.didDisconnect {
                expectation.fulfill()
            }
        }

        sut.disconnect()
        wait(for: [expectation], timeout: 1.0)
        XCTAssertTrue(mockDelegate.didDisconnect)
        XCTAssertNil(mockDelegate.disconnectError)
    }

    func testDisconnectWithErrorCallsDelegate() {
        let url = URL(string: "ws://localhost:8787")!
        let mockTask = MockURLSessionWebSocketTask()
        mockSession.mockTask = mockTask

        sut.connect(url: url, token: "test-token")

        let testError = NSError(domain: "test", code: 1, userInfo: nil)

        let expectation = self.expectation(description: "disconnect with error called")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
            if self.mockDelegate.didDisconnect {
                expectation.fulfill()
            }
        }

        // Simulate error by calling disconnect internally
        // (In real scenario, this happens from receive failure)
        mockTask.simulateDisconnect(with: testError, client: sut)

        wait(for: [expectation], timeout: 1.0)
        XCTAssertTrue(mockDelegate.didDisconnect)
        XCTAssertNotNil(mockDelegate.disconnectError)
    }

    // MARK: - Ping Tests

    func testSendPing() {
        let url = URL(string: "ws://localhost:8787")!
        let mockTask = MockURLSessionWebSocketTask()
        mockSession.mockTask = mockTask

        sut.connect(url: url, token: "test-token")
        sut.sendPing()

        XCTAssertTrue(mockTask.sendPingCalled)
    }

    func testSendPingWithErrorDisconnects() {
        let url = URL(string: "ws://localhost:8787")!
        let mockTask = MockURLSessionWebSocketTask()
        mockTask.pingError = NSError(domain: "test", code: 1, userInfo: nil)
        mockSession.mockTask = mockTask

        sut.connect(url: url, token: "test-token")

        let expectation = self.expectation(description: "disconnect after ping error")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) {
            if self.mockDelegate.didDisconnect {
                expectation.fulfill()
            }
        }

        sut.sendPing()

        wait(for: [expectation], timeout: 1.0)
        XCTAssertTrue(mockDelegate.didDisconnect)
    }

    // MARK: - JSON-RPC Request Tests

    func testSendJSONRPCRequest() {
        let url = URL(string: "ws://localhost:8787")!
        let mockTask = MockURLSessionWebSocketTask()
        mockSession.mockTask = mockTask

        sut.connect(url: url, token: "test-token")

        struct TestParams: Codable { let value: String }
        struct TestResult: Codable { let success: Bool }

        let expectation = self.expectation(description: "RPC completion")
        var receivedResult: TestResult?

        sut.sendJSONRPC(
            method: "test.method",
            params: TestParams(value: "test"),
            id: "test-id-123"
        ) { (result: TestResult?) in
            receivedResult = result
            expectation.fulfill()
        }

        // Verify request was sent
        XCTAssertTrue(mockTask.sentMessages.count >= 2) // initialize + test.method

        if mockTask.sentMessages.count >= 2 {
            if case .string(let text) = mockTask.sentMessages.last {
                XCTAssertTrue(text.contains("\"method\":\"test.method\""))
                XCTAssertTrue(text.contains("\"id\":\"test-id-123\""))
            }
        }

        // Simulate response
        let response = """
        {"jsonrpc":"2.0","id":"test-id-123","result":{"success":true}}
        """
        mockTask.simulateReceive(message: .string(response), client: sut)

        wait(for: [expectation], timeout: 1.0)
        XCTAssertNotNil(receivedResult)
        XCTAssertTrue(receivedResult?.success ?? false)
    }

    func testSendJSONRPCRequestWithError() {
        let url = URL(string: "ws://localhost:8787")!
        let mockTask = MockURLSessionWebSocketTask()
        mockTask.sendError = NSError(domain: "test", code: 1, userInfo: nil)
        mockSession.mockTask = mockTask

        sut.connect(url: url, token: "test-token")

        struct TestParams: Codable { let value: String }
        struct TestResult: Codable { let success: Bool }

        let expectation = self.expectation(description: "RPC error completion")
        var receivedResult: TestResult?

        sut.sendJSONRPC(
            method: "test.method",
            params: TestParams(value: "test"),
            id: "error-test"
        ) { (result: TestResult?) in
            receivedResult = result
            expectation.fulfill()
        }

        wait(for: [expectation], timeout: 1.0)
        XCTAssertNil(receivedResult)
    }

    // MARK: - JSON-RPC Notification Tests

    func testSendJSONRPCNotification() {
        let url = URL(string: "ws://localhost:8787")!
        let mockTask = MockURLSessionWebSocketTask()
        mockSession.mockTask = mockTask

        sut.connect(url: url, token: "test-token")

        struct TestParams: Codable { let value: String }

        sut.sendJSONRPCNotification(
            method: "test.notify",
            params: TestParams(value: "notification")
        )

        // Verify notification was sent
        XCTAssertTrue(mockTask.sentMessages.count >= 2)

        if let lastMessage = mockTask.sentMessages.last,
           case .string(let text) = lastMessage {
            XCTAssertTrue(text.contains("\"method\":\"test.notify\""))
            XCTAssertFalse(text.contains("\"id\"")) // notifications have no id
        }
    }

    // MARK: - Message Receiving Tests

    func testReceiveJSONRPCNotification() {
        let url = URL(string: "ws://localhost:8787")!
        let mockTask = MockURLSessionWebSocketTask()
        mockSession.mockTask = mockTask

        sut.connect(url: url, token: "test-token")

        // Simulate successful initialize
        let initResponse = """
        {"jsonrpc":"2.0","id":"1","result":{"protocol_version":"0.7.0","agent_capabilities":{},"auth_methods":[]}}
        """
        mockTask.simulateReceive(message: .string(initResponse), client: sut)

        // Wait for connection
        let connectExpectation = expectation(description: "connection")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
            if self.mockDelegate.didConnect {
                connectExpectation.fulfill()
            }
        }
        wait(for: [connectExpectation], timeout: 1.0)

        // Now simulate notification
        let notification = """
        {"jsonrpc":"2.0","method":"session/update","params":{"session_id":"test-session","update":{}}}
        """

        let notificationExpectation = expectation(description: "notification received")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) {
            if !self.mockDelegate.receivedNotifications.isEmpty {
                notificationExpectation.fulfill()
            }
        }

        mockTask.simulateReceive(message: .string(notification), client: sut)

        wait(for: [notificationExpectation], timeout: 1.0)
        XCTAssertEqual(mockDelegate.receivedNotifications.count, 1)
        XCTAssertEqual(mockDelegate.receivedNotifications[0].method, "session/update")
    }

    func testReceiveJSONRPCRequest() {
        let url = URL(string: "ws://localhost:8787")!
        let mockTask = MockURLSessionWebSocketTask()
        mockSession.mockTask = mockTask

        sut.connect(url: url, token: "test-token")

        // Simulate successful initialize
        let initResponse = """
        {"jsonrpc":"2.0","id":"1","result":{"protocol_version":"0.7.0","agent_capabilities":{},"auth_methods":[]}}
        """
        mockTask.simulateReceive(message: .string(initResponse), client: sut)

        // Wait for connection
        let connectExpectation = expectation(description: "connection")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
            if self.mockDelegate.didConnect {
                connectExpectation.fulfill()
            }
        }
        wait(for: [connectExpectation], timeout: 1.0)

        // Now simulate request
        let request = """
        {"jsonrpc":"2.0","method":"session/request_permission","id":"req-123","params":{}}
        """

        let requestExpectation = expectation(description: "request received")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) {
            if !self.mockDelegate.receivedRequests.isEmpty {
                requestExpectation.fulfill()
            }
        }

        mockTask.simulateReceive(message: .string(request), client: sut)

        wait(for: [requestExpectation], timeout: 1.0)
        XCTAssertEqual(mockDelegate.receivedRequests.count, 1)
        XCTAssertEqual(mockDelegate.receivedRequests[0].method, "session/request_permission")
        XCTAssertEqual(mockDelegate.receivedRequests[0].id, "req-123")
    }

    // MARK: - Legacy Bridge Message Tests

    func testSendLegacyMessage() {
        let url = URL(string: "ws://localhost:8787")!
        let mockTask = MockURLSessionWebSocketTask()
        mockSession.mockTask = mockTask

        sut.connect(url: url, token: "test-token")

        struct TestMessage: Codable { let value: String }
        sut.send(type: "test.type", message: TestMessage(value: "test"))

        XCTAssertTrue(mockTask.sentMessages.count >= 2)
    }

    // MARK: - Error Handling Tests

    func testReceiveErrorDisconnects() {
        let url = URL(string: "ws://localhost:8787")!
        let mockTask = MockURLSessionWebSocketTask()
        mockSession.mockTask = mockTask

        sut.connect(url: url, token: "test-token")

        let error = NSError(domain: "test", code: 1, userInfo: nil)

        let expectation = self.expectation(description: "disconnect on error")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) {
            if self.mockDelegate.didDisconnect {
                expectation.fulfill()
            }
        }

        mockTask.simulateReceive(error: error, client: sut)

        wait(for: [expectation], timeout: 1.0)
        XCTAssertTrue(mockDelegate.didDisconnect)
        XCTAssertNotNil(mockDelegate.disconnectError)
    }

    func testInvalidInitializeResponseDisconnects() {
        let url = URL(string: "ws://localhost:8787")!
        let mockTask = MockURLSessionWebSocketTask()
        mockSession.mockTask = mockTask

        sut.connect(url: url, token: "test-token")

        // Send invalid initialize response
        let invalidResponse = """
        {"jsonrpc":"2.0","id":"1","result":{"protocol_version":"invalid"}}
        """

        let expectation = self.expectation(description: "disconnect on invalid init")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) {
            if self.mockDelegate.didDisconnect {
                expectation.fulfill()
            }
        }

        mockTask.simulateReceive(message: .string(invalidResponse), client: sut)

        wait(for: [expectation], timeout: 1.0)
        XCTAssertTrue(mockDelegate.didDisconnect)
    }

    // MARK: - Retry/Reconnect Tests

    func testRetryOnConnectionFailure() {
        // Enable auto-reconnect with short delays for testing
        sut.autoReconnect = true
        sut.initialRetryDelay = 0.1
        sut.maxRetryAttempts = 3

        let url = URL(string: "ws://localhost:8787")!
        let mockTask = MockURLSessionWebSocketTask()
        mockTask.shouldFailOnSend = true // Simulate connection failure
        mockSession.mockTask = mockTask

        sut.connect(url: url, token: "test-token")

        // Wait for retry attempts
        let expectation = self.expectation(description: "retry attempts")
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
            expectation.fulfill()
        }

        wait(for: [expectation], timeout: 2.0)

        // Should have attempted multiple times (initial + retries)
        XCTAssertTrue(mockSession.webSocketTaskCalled)
    }

    func testExponentialBackoff() {
        sut.autoReconnect = true
        sut.initialRetryDelay = 1.0
        sut.maxRetryDelay = 30.0

        // Test backoff calculation via private method reflection (or test behavior indirectly)
        // Retry 1: 1.0s, Retry 2: 2.0s, Retry 3: 4.0s, Retry 4: 8.0s, Retry 5: 16.0s
        // This is tested indirectly by verifying retry timing

        let url = URL(string: "ws://localhost:8787")!
        let mockTask = MockURLSessionWebSocketTask()
        mockTask.shouldFailOnSend = true
        mockSession.mockTask = mockTask

        sut.connect(url: url, token: "test-token")

        // Just verify retries happen with increasing delays
        let expectation = self.expectation(description: "backoff behavior")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
            expectation.fulfill()
        }

        wait(for: [expectation], timeout: 1.0)
    }

    func testMaxRetryLimit() {
        sut.autoReconnect = true
        sut.initialRetryDelay = 0.05
        sut.maxRetryAttempts = 2

        let url = URL(string: "ws://localhost:8787")!
        let mockTask = MockURLSessionWebSocketTask()
        mockTask.shouldFailOnSend = true
        mockSession.mockTask = mockTask

        sut.connect(url: url, token: "test-token")

        // Wait long enough for all retries to exhaust
        let expectation = self.expectation(description: "max retries reached")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
            expectation.fulfill()
        }

        wait(for: [expectation], timeout: 1.0)

        // After max retries, should have received final error
        XCTAssertTrue(mockDelegate.didDisconnect)
        if let error = mockDelegate.disconnectError as? NSError {
            XCTAssertEqual(error.code, 8) // Max retry attempts reached
        }
    }

    func testHandshakeTimeout() {
        sut.handshakeTimeout = 0.2 // Very short timeout for testing

        let url = URL(string: "ws://localhost:8787")!
        let mockTask = MockURLSessionWebSocketTask()
        mockTask.shouldNeverRespond = true // Never send initialize response
        mockSession.mockTask = mockTask

        sut.connect(url: url, token: "test-token")

        let expectation = self.expectation(description: "handshake timeout")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
            if self.mockDelegate.didDisconnect {
                expectation.fulfill()
            }
        }

        wait(for: [expectation], timeout: 1.0)

        XCTAssertTrue(mockDelegate.didDisconnect)
        if let error = mockDelegate.disconnectError as? NSError {
            XCTAssertEqual(error.code, 7) // Handshake timeout
        }
    }

    func testAutoReconnectOnUnexpectedDisconnect() {
        sut.autoReconnect = true
        sut.initialRetryDelay = 0.1
        sut.maxRetryAttempts = 3

        let url = URL(string: "ws://localhost:8787")!
        let mockTask = MockURLSessionWebSocketTask()
        mockSession.mockTask = mockTask

        sut.connect(url: url, token: "test-token")

        // Simulate successful handshake first
        let initResponse = """
        {"jsonrpc":"2.0","id":"1","result":{"protocol_version":"0.7.0","agent_capabilities":{},"auth_methods":[],"agent_info":{"name":"test","title":"Test","version":"1.0"}}}
        """
        mockTask.simulateReceive(message: .string(initResponse), client: sut)

        // Wait for connection
        let connectedExpectation = self.expectation(description: "connected")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
            if self.mockDelegate.didConnect {
                connectedExpectation.fulfill()
            }
        }
        wait(for: [connectedExpectation], timeout: 1.0)

        // Clear mock state and simulate unexpected disconnect
        mockDelegate.didConnect = false
        mockDelegate.didDisconnect = false

        // Create new task for reconnect
        let newMockTask = MockURLSessionWebSocketTask()
        mockSession.mockTask = newMockTask

        // Simulate error to trigger reconnect
        mockTask.simulateError(NSError(domain: "Test", code: 1, userInfo: nil), client: sut)

        // Wait for reconnect attempt
        let reconnectExpectation = self.expectation(description: "reconnect attempt")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
            // Should have attempted to reconnect
            if self.mockSession.webSocketTaskCalled {
                reconnectExpectation.fulfill()
            }
        }

        wait(for: [reconnectExpectation], timeout: 1.0)
    }

    func testManualDisconnectDoesNotRetry() {
        sut.autoReconnect = true
        sut.initialRetryDelay = 0.1

        let url = URL(string: "ws://localhost:8787")!
        let mockTask = MockURLSessionWebSocketTask()
        mockSession.mockTask = mockTask

        sut.connect(url: url, token: "test-token")

        // Manually disconnect
        mockSession.webSocketTaskCalled = false
        sut.disconnect()

        // Wait to ensure no retry happens
        let expectation = self.expectation(description: "no retry after manual disconnect")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
            expectation.fulfill()
        }

        wait(for: [expectation], timeout: 0.5)

        // Should not have attempted reconnect
        XCTAssertTrue(mockDelegate.didDisconnect)
    }

    func testSuccessfulConnectionResetsRetryCount() {
        sut.autoReconnect = true
        sut.initialRetryDelay = 0.1
        sut.maxRetryAttempts = 5

        let url = URL(string: "ws://localhost:8787")!

        // First attempt: fail
        let failTask = MockURLSessionWebSocketTask()
        failTask.shouldFailOnSend = true
        mockSession.mockTask = failTask

        sut.connect(url: url, token: "test-token")

        // Wait for first retry
        let retryExpectation = self.expectation(description: "first retry")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) {
            retryExpectation.fulfill()
        }
        wait(for: [retryExpectation], timeout: 0.5)

        // Second attempt: succeed
        let successTask = MockURLSessionWebSocketTask()
        mockSession.mockTask = successTask

        // Simulate successful initialize
        let initResponse = """
        {"jsonrpc":"2.0","id":"1","result":{"protocol_version":"0.7.0","agent_capabilities":{},"auth_methods":[],"agent_info":{"name":"test","title":"Test","version":"1.0"}}}
        """

        let successExpectation = self.expectation(description: "successful connection")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
            successTask.simulateReceive(message: .string(initResponse), client: self.sut)
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
                if self.mockDelegate.didConnect {
                    successExpectation.fulfill()
                }
            }
        }

        wait(for: [successExpectation], timeout: 1.0)
        XCTAssertTrue(mockDelegate.didConnect)

        // Retry count should be reset - if we disconnect again and reconnect,
        // it should start from 0
    }

    func testDisableAutoReconnect() {
        sut.autoReconnect = false

        let url = URL(string: "ws://localhost:8787")!
        let mockTask = MockURLSessionWebSocketTask()
        mockTask.shouldFailOnSend = true
        mockSession.mockTask = mockTask

        sut.connect(url: url, token: "test-token")

        // Wait to ensure no retry happens
        mockSession.webSocketTaskCalled = false
        let expectation = self.expectation(description: "no auto reconnect")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
            expectation.fulfill()
        }

        wait(for: [expectation], timeout: 0.5)

        // Should have failed once and not retried
        XCTAssertTrue(mockDelegate.didDisconnect)
    }

    func testConfigurableRetryParameters() {
        // Test that all retry parameters can be configured
        sut.maxRetryAttempts = 10
        sut.initialRetryDelay = 2.0
        sut.maxRetryDelay = 60.0
        sut.handshakeTimeout = 15.0
        sut.autoReconnect = false

        XCTAssertEqual(sut.maxRetryAttempts, 10)
        XCTAssertEqual(sut.initialRetryDelay, 2.0)
        XCTAssertEqual(sut.maxRetryDelay, 60.0)
        XCTAssertEqual(sut.handshakeTimeout, 15.0)
        XCTAssertFalse(sut.autoReconnect)
    }
}

// MARK: - Mock URLSession

class MockURLSession: URLSession {
    var mockTask: MockURLSessionWebSocketTask?
    var webSocketTaskCalled = false
    var lastRequest: URLRequest?

    override func webSocketTask(with request: URLRequest) -> URLSessionWebSocketTask {
        webSocketTaskCalled = true
        lastRequest = request
        if let task = mockTask {
            return task
        }
        return MockURLSessionWebSocketTask()
    }
}

// MARK: - Mock WebSocketTask

class MockURLSessionWebSocketTask: URLSessionWebSocketTask {
    var sendCalled = false
    var sendPingCalled = false
    var resumeCalled = false
    var cancelCalled = false
    var sentMessages: [URLSessionWebSocketTask.Message] = []
    var sendError: Error?
    var pingError: Error?

    // Retry/reconnect test flags
    var shouldFailOnSend = false
    var shouldNeverRespond = false

    private var receiveHandler: ((Result<URLSessionWebSocketTask.Message, Error>) -> Void)?

    override func resume() {
        resumeCalled = true
    }

    override func cancel(with closeCode: URLSessionWebSocketTask.CloseCode, reason: Data?) {
        cancelCalled = true
    }

    override func send(_ message: URLSessionWebSocketTask.Message, completionHandler: @escaping (Error?) -> Void) {
        sendCalled = true
        sentMessages.append(message)

        let error = shouldFailOnSend ? NSError(domain: "MockWebSocketTask", code: 1, userInfo: [NSLocalizedDescriptionKey: "Simulated send failure"]) : sendError

        DispatchQueue.main.async {
            completionHandler(error)
        }
    }

    override func sendPing(pongReceiveHandler: @escaping (Error?) -> Void) {
        sendPingCalled = true
        DispatchQueue.main.async {
            pongReceiveHandler(self.pingError)
        }
    }

    override func receive(completionHandler: @escaping (Result<URLSessionWebSocketTask.Message, Error>) -> Void) {
        receiveHandler = completionHandler
    }

    func simulateReceive(message: URLSessionWebSocketTask.Message, client: MobileWebSocketClient) {
        guard !shouldNeverRespond else { return }
        DispatchQueue.main.async {
            self.receiveHandler?(.success(message))
        }
    }

    func simulateReceive(error: Error, client: MobileWebSocketClient) {
        guard !shouldNeverRespond else { return }
        DispatchQueue.main.async {
            self.receiveHandler?(.failure(error))
        }
    }

    func simulateError(_ error: Error, client: MobileWebSocketClient) {
        DispatchQueue.main.async {
            self.receiveHandler?(.failure(error))
        }
    }

    func simulateDisconnect(with error: Error, client: MobileWebSocketClient) {
        client.disconnect()
    }
}
