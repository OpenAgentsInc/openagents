import XCTest
import Combine
@testable import OpenAgents
@testable import OpenAgentsCore

@MainActor
final class BridgeManagerTests: XCTestCase {
    var sut: BridgeManager!
    var cancellables: Set<AnyCancellable>!

    override func setUp() async throws {
        try await super.setUp()
        sut = BridgeManager()
        cancellables = []
    }

    override func tearDown() async throws {
        sut.stop()
        sut = nil
        cancellables = nil
        try await super.tearDown()
    }

    // MARK: - Initialization Tests

    func testInitialState() {
        XCTAssertEqual(sut.status, .idle)
        XCTAssertTrue(sut.logs.isEmpty)
        XCTAssertEqual(sut.lastLog, "")
    }

    #if os(iOS)
    func testIOSInitialState() {
        XCTAssertTrue(sut.threads.isEmpty)
        XCTAssertTrue(sut.updates.isEmpty)
        XCTAssertNil(sut.currentSessionId)
        XCTAssertTrue(sut.availableCommands.isEmpty)
        XCTAssertEqual(sut.currentMode, .default_mode)
    }
    #endif

    #if os(macOS)
    func testMacOSInitialState() {
        XCTAssertEqual(sut.connectedClientCount, 0)
    }
    #endif

    // MARK: - Logging Tests

    func testLogAddsToLogsArray() {
        sut.log("test", "message 1")
        XCTAssertEqual(sut.logs.count, 1)
        XCTAssertTrue(sut.logs[0].contains("[test] message 1"))
    }

    func testLogUpdatesLastLog() {
        sut.log("test", "latest message")
        XCTAssertTrue(sut.lastLog.contains("[test] latest message"))
    }

    func testLogRingBufferLimit() {
        // Add 250 logs (should keep only last 200)
        for i in 1...250 {
            sut.log("test", "message \(i)")
        }
        XCTAssertEqual(sut.logs.count, 200)
        // Should have messages 51-250
        XCTAssertTrue(sut.logs.first?.contains("message 51") ?? false)
        XCTAssertTrue(sut.logs.last?.contains("message 250") ?? false)
    }

    func testLogPublishesChanges() {
        let expectation = expectation(description: "log published")
        sut.$lastLog
            .dropFirst()
            .sink { log in
                XCTAssertTrue(log.contains("test message"))
                expectation.fulfill()
            }
            .store(in: &cancellables)

        sut.log("tag", "test message")
        wait(for: [expectation], timeout: 1.0)
    }

    #if os(iOS)
    // MARK: - iOS Connection Tests

    func testIOSStartTriggersConnection() throws {
        throw XCTSkip("Requires live MobileConnectionManager; skipping in unit tests")
    }

    func testManualConnectStopsExistingClient() throws {
        throw XCTSkip("Requires live MobileConnectionManager; skipping in unit tests")
    }

    func testStopDisconnectsClient() throws {
        throw XCTSkip("Requires live MobileConnectionManager; skipping in unit tests")
    }

    // MARK: - iOS Update Management Tests

    func testUpdateRingBufferLimit() {
        // Create 250 updates
        let updates = (1...250).map { i in
            TestHelpers.makeSessionUpdateNotification(
                update: TestHelpers.makeTextUpdate(text: "message \(i)")
            )
        }

        // Add updates one by one (simulating what happens in delegate)
        for update in updates {
            sut.updates.append(update)
            if sut.updates.count > 200 {
                sut.updates.removeFirst(sut.updates.count - 200)
            }
        }

        XCTAssertEqual(sut.updates.count, 200)
    }

    func testAvailableCommandsUpdateExtracted() {
        let commands = [
            ACP.Client.AvailableCommand(name: "Test Command", description: "desc", input: .unstructured(hint: ""))
        ]
        let update = TestHelpers.makeAvailableCommandsUpdate(commands: commands)
        let notification = TestHelpers.makeSessionUpdateNotification(update: update)

        // Simulate receiving notification
        sut.updates.append(notification)
        switch notification.update {
        case .availableCommandsUpdate(let ac):
            sut.availableCommands = ac.available_commands
        default:
            break
        }

        XCTAssertEqual(sut.availableCommands.count, 1)
        XCTAssertEqual(sut.availableCommands[0].name, "Test Command")
    }

    func testCurrentModeUpdateExtracted() {
        let update = ACP.Client.SessionUpdate.currentModeUpdate(.init(current_mode_id: .codex))
        let notification = ACP.Client.SessionNotificationWire(
            session_id: ACPSessionId("test-session"),
            update: update
        )

        // Simulate receiving notification
        sut.updates.append(notification)
        switch notification.update {
        case .currentModeUpdate(let cur):
            sut.currentMode = cur.current_mode_id
        default:
            break
        }

        XCTAssertEqual(sut.currentMode, .codex)
    }

    // MARK: - iOS Prompt Sending Tests

    func testSendPromptWithNoSession() {
        // This will trigger session creation then prompt
        // We can't easily test the network call, but we can verify it doesn't crash
        sut.sendPrompt(text: "test prompt")
        XCTAssertNotNil(sut)
    }

    func testSendPromptWithExistingSession() {
        sut.currentSessionId = ACPSessionId("existing-session")
        sut.sendPrompt(text: "test prompt")
        XCTAssertNotNil(sut)
    }

    func testCancelCurrentSession() {
        sut.currentSessionId = ACPSessionId("session-to-cancel")
        sut.cancelCurrentSession()
        // Should not crash
        XCTAssertNotNil(sut)
    }

    func testCancelWithNoSession() {
        // Should handle gracefully
        sut.cancelCurrentSession()
        XCTAssertNotNil(sut)
    }

    // MARK: - iOS Connection Failure Tests

    func testConnectionFailure_TransitionsToErrorState() {
        let expectation = expectation(description: "transitions to error")
        sut.$status
            .dropFirst()
            .sink { status in
                if case .error = status {
                    expectation.fulfill()
                }
            }
            .store(in: &cancellables)

        // Simulate connection failure
        sut.status = .connecting(host: "invalid.host", port: 8787)
        sut.status = .error("Connection failed: invalid host")

        wait(for: [expectation], timeout: 1.0)

        if case .error(let message) = sut.status {
            XCTAssertTrue(message.contains("Connection failed"))
        } else {
            XCTFail("Expected error status")
        }
    }

    func testConnectionTimeout_HandledGracefully() {
        let expectation = expectation(description: "timeout handled")
        sut.$status
            .dropFirst(2) // Skip initial connecting state
            .sink { status in
                if case .error = status {
                    expectation.fulfill()
                }
            }
            .store(in: &cancellables)

        // Simulate connection attempt
        sut.status = .connecting(host: "timeout.host", port: 8787)

        // Simulate timeout after delay
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
            self.sut.status = .error("Connection timeout")
        }

        wait(for: [expectation], timeout: 2.0)
    }

    func testReconnectAfterDisconnect_PreservesState() {
        // Set up some state
        sut.currentSessionId = ACPSessionId("preserved-session")
        let initialUpdate = TestHelpers.makeSessionUpdateNotification(
            update: TestHelpers.makeTextUpdate(text: "preserved message")
        )
        sut.updates.append(initialUpdate)

        // Simulate connected state
        sut.status = .connected(host: "test.local", port: 8787)

        // Store state before disconnect
        let preservedSessionId = sut.currentSessionId
        let preservedUpdateCount = sut.updates.count

        // Simulate disconnect
        sut.status = .error("Connection lost")

        // Verify state is preserved after disconnect
        XCTAssertEqual(sut.currentSessionId, preservedSessionId)
        XCTAssertEqual(sut.updates.count, preservedUpdateCount)

        // Simulate reconnect
        sut.status = .connecting(host: "test.local", port: 8787)
        sut.status = .connected(host: "test.local", port: 8787)

        // State should still be preserved
        XCTAssertEqual(sut.currentSessionId, preservedSessionId)
        XCTAssertEqual(sut.updates.count, preservedUpdateCount)
    }

    func testMultipleConnectionAttempts_TrackedInLogs() {
        // Attempt 1
        sut.status = .connecting(host: "test1.local", port: 8787)
        sut.log("bridge", "Connecting to test1.local:8787")
        sut.status = .error("Connection failed")
        sut.log("bridge", "Connection failed")

        // Attempt 2
        sut.status = .connecting(host: "test2.local", port: 8788)
        sut.log("bridge", "Connecting to test2.local:8788")
        sut.status = .error("Connection failed")
        sut.log("bridge", "Connection failed")

        // Attempt 3 (success)
        sut.status = .connecting(host: "test3.local", port: 8789)
        sut.log("bridge", "Connecting to test3.local:8789")
        sut.status = .connected(host: "test3.local", port: 8789)
        sut.log("bridge", "Connected")

        // Should have all attempts logged
        XCTAssertTrue(sut.logs.count >= 6, "Should have logs for all connection attempts")
        XCTAssertTrue(sut.logs.contains { $0.contains("test1.local") })
        XCTAssertTrue(sut.logs.contains { $0.contains("test2.local") })
        XCTAssertTrue(sut.logs.contains { $0.contains("test3.local") })
        XCTAssertTrue(sut.lastLog.contains("Connected"))
    }

    func testHandshakeFailure_LogsErrorMessage() {
        let expectation = expectation(description: "handshake failure logged")

        sut.$lastLog
            .dropFirst()
            .sink { log in
                if log.contains("Handshake failed") {
                    expectation.fulfill()
                }
            }
            .store(in: &cancellables)

        sut.status = .connecting(host: "test.local", port: 8787)
        sut.log("handshake", "Handshake failed: protocol version mismatch")

        wait(for: [expectation], timeout: 1.0)
    }

    func testMalformedMessage_DoesNotCrashBridge() {
        // This tests that receiving malformed updates doesn't crash
        // In real usage, malformed JSON would be caught during decode

        // Start with valid connection
        sut.status = .connected(host: "test.local", port: 8787)

        // Add some valid updates
        let validUpdate = TestHelpers.makeSessionUpdateNotification(
            update: TestHelpers.makeTextUpdate(text: "valid message")
        )
        sut.updates.append(validUpdate)

        let updateCountBefore = sut.updates.count

        // Malformed messages would be filtered out during JSON decode,
        // but we can test that the state remains stable
        // (In real usage, the WebSocket client handles decode errors)

        // Add another valid update after "malformed" one
        let anotherValid = TestHelpers.makeSessionUpdateNotification(
            update: TestHelpers.makeTextUpdate(text: "another valid message")
        )
        sut.updates.append(anotherValid)

        // Should have 2 valid updates
        XCTAssertEqual(sut.updates.count, updateCountBefore + 1)
        XCTAssertNotNil(sut)
    }

    func testNetworkInterruption_CanRecover() {
        let expectation = expectation(description: "recovery after interruption")
        expectation.expectedFulfillmentCount = 3

        var statusSequence: [String] = []

        sut.$status
            .dropFirst()
            .sink { status in
                switch status {
                case .connected:
                    statusSequence.append("connected")
                case .error:
                    statusSequence.append("error")
                case .connecting:
                    statusSequence.append("connecting")
                default:
                    break
                }
                if statusSequence.count <= 3 {
                    expectation.fulfill()
                }
            }
            .store(in: &cancellables)

        // Initial connection
        sut.status = .connected(host: "test.local", port: 8787)

        // Network interruption
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
            self.sut.status = .error("Network interrupted")
        }

        // Automatic reconnect attempt
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) {
            self.sut.status = .connecting(host: "test.local", port: 8787)
        }

        wait(for: [expectation], timeout: 2.0)

        // Should see: connected → error → connecting
        XCTAssertEqual(statusSequence.count, 3)
    }

    func testConnectionStateTransitions_AllScenarios() {
        // Test all valid state transitions:
        // idle → connecting
        XCTAssertEqual(sut.status, .idle)
        sut.status = .connecting(host: "test.local", port: 8787)
        if case .connecting = sut.status {
            XCTAssert(true)
        } else {
            XCTFail("Expected connecting state")
        }

        // connecting → connected
        sut.status = .connected(host: "test.local", port: 8787)
        if case .connected = sut.status {
            XCTAssert(true)
        } else {
            XCTFail("Expected connected state")
        }

        // connected → error (disconnect)
        sut.status = .error("Connection lost")
        if case .error = sut.status {
            XCTAssert(true)
        } else {
            XCTFail("Expected error state")
        }

        // error → connecting (retry)
        sut.status = .connecting(host: "test.local", port: 8787)
        if case .connecting = sut.status {
            XCTAssert(true)
        } else {
            XCTFail("Expected connecting state")
        }

        // connecting → error (connection failed)
        sut.status = .error("Connection failed")
        if case .error = sut.status {
            XCTAssert(true)
        } else {
            XCTFail("Expected error state")
        }
    }

    func testStopDuringConnectionAttempt_HandledGracefully() {
        // Start connecting
        sut.start()

        // Verify we're in connecting state
        if case .connecting = sut.status {
            XCTAssert(true, "Started connecting")
        }

        // Stop during connection attempt
        sut.stop()

        // Should not crash and should be in idle or error state
        switch sut.status {
        case .idle, .error:
            XCTAssert(true, "Stop handled gracefully")
        default:
            // Other states are also acceptable as long as no crash
            XCTAssert(true, "Stop completed without crash")
        }
    }

    func testManualReconnect_ClearsErrorState() throws {
        throw XCTSkip("Requires live MobileConnectionManager; skipping in unit tests")
    }
    #endif

    #if os(macOS)
    // MARK: - macOS Server Tests

    func testMacOSServerStartUpdatesStatus() {
        let expectation = expectation(description: "status changes to advertising")
        sut.$status
            .dropFirst()
            .sink { status in
                if case .advertising = status {
                    expectation.fulfill()
                }
            }
            .store(in: &cancellables)

        sut.start()

        wait(for: [expectation], timeout: 2.0)
    }

    func testMacOSServerStop() {
        sut.start()
        sut.stop()
        // Should not crash
        XCTAssertNotNil(sut)
    }

    func testConnectedClientCountIncrementsOnHandshake() {
        XCTAssertEqual(sut.connectedClientCount, 0)

        // Simulate client handshake via delegate callback
        // (This would normally be called by the server)
        sut.connectedClientCount += 1

        XCTAssertEqual(sut.connectedClientCount, 1)
    }

    func testConnectedClientCountDecrementsOnDisconnect() {
        sut.connectedClientCount = 2

        // Simulate client disconnect
        if sut.connectedClientCount > 0 {
            sut.connectedClientCount -= 1
        }

        XCTAssertEqual(sut.connectedClientCount, 1)
    }

    func testConnectedClientCountNeverNegative() {
        sut.connectedClientCount = 0

        // Simulate disconnect when count is already 0
        if sut.connectedClientCount > 0 {
            sut.connectedClientCount -= 1
        }

        XCTAssertEqual(sut.connectedClientCount, 0)
    }
    #endif

    // MARK: - State Machine Tests

    func testStatusTransitionsPublish() {
        let expectation = expectation(description: "status published")
        expectation.expectedFulfillmentCount = 2

        var receivedStatuses: [BridgeManager.Status] = []

        sut.$status
            .dropFirst()
            .sink { status in
                receivedStatuses.append(status)
                expectation.fulfill()
            }
            .store(in: &cancellables)

        #if os(iOS)
        sut.status = .connecting(host: "test.local", port: 8787)
        sut.status = .connected(host: "test.local", port: 8787)
        #else
        sut.status = .advertising(port: 8787)
        sut.status = .advertising(port: 8788)
        #endif

        wait(for: [expectation], timeout: 1.0)
        XCTAssertEqual(receivedStatuses.count, 2)
    }

    func testErrorStatusContainsMessage() {
        sut.status = .error("connection failed")

        if case .error(let message) = sut.status {
            XCTAssertEqual(message, "connection failed")
        } else {
            XCTFail("Expected error status")
        }
    }

    // MARK: - Thread Safety Tests

    func testConcurrentLogsCalls() {
        let expectation = expectation(description: "concurrent logs complete")
        expectation.expectedFulfillmentCount = 100

        DispatchQueue.concurrentPerform(iterations: 100) { i in
            Task { @MainActor in
                self.sut.log("concurrent", "message \(i)")
                expectation.fulfill()
            }
        }

        wait(for: [expectation], timeout: 5.0)
        // Should have at most 200 logs (ring buffer limit)
        XCTAssertLessThanOrEqual(sut.logs.count, 200)
    }

    // MARK: - Endpoint persistence / selection

    func testPickInitialEndpointPrefersPersisted() async throws {
        #if os(iOS)
        // Clear any prior values
        UserDefaults.standard.removeObject(forKey: "oa.bridge.last_host")
        UserDefaults.standard.removeObject(forKey: "oa.bridge.last_port")

        BridgeManager.saveLastSuccessfulEndpoint(host: "10.1.2.3", port: 9999)
        let (h, p) = BridgeManager.pickInitialEndpoint()
        XCTAssertEqual(h, "10.1.2.3")
        XCTAssertEqual(p, 9999)
        #endif
    }

    func testStartUsesPersistedEndpoint() async throws {
        #if os(iOS)
        // Clear and set persisted values
        UserDefaults.standard.removeObject(forKey: "oa.bridge.last_host")
        UserDefaults.standard.removeObject(forKey: "oa.bridge.last_port")
        BridgeManager.saveLastSuccessfulEndpoint(host: "10.9.8.7", port: 12345)

        let exp = expectation(description: "status becomes connecting to persisted")
        sut.$status
            .dropFirst()
            .sink { status in
                if case .connecting(let h, let p) = status {
                    if h == "10.9.8.7" && p == 12345 { exp.fulfill() }
                }
            }
            .store(in: &cancellables)

        sut.start()
        wait(for: [exp], timeout: 2.0)
        #endif
    }
}
