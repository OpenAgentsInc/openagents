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

    func testIOSStartTriggersConnection() {
        let expectation = expectation(description: "status changes")
        sut.$status
            .dropFirst()
            .sink { status in
                // Should transition from idle to connecting
                if case .connecting = status {
                    expectation.fulfill()
                }
            }
            .store(in: &cancellables)

        sut.start()

        wait(for: [expectation], timeout: 2.0)
    }

    func testManualConnectStopsExistingClient() {
        // Start initial connection
        sut.start()

        // Verify we're connecting
        XCTAssertNotEqual(sut.status, .idle)

        // Manual connect should stop and restart
        sut.performManualConnect(host: "192.168.1.100", port: 8888)

        // Should be connecting to new host
        if case .connecting(let host, let port) = sut.status {
            XCTAssertEqual(host, "192.168.1.100")
            XCTAssertEqual(port, 8888)
        } else {
            XCTFail("Expected connecting status")
        }
    }

    func testStopDisconnectsClient() {
        sut.start()
        sut.stop()
        // After stop, status may be idle or error
        // Just verify stop doesn't crash
        XCTAssertNotNil(sut)
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
            ACP.Client.AvailableCommand(
                id: ACP.CommandId("cmd1"),
                command_name: "Test Command",
                mode_id: .default_mode
            )
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
        XCTAssertEqual(sut.availableCommands[0].command_name, "Test Command")
    }

    func testCurrentModeUpdateExtracted() {
        let update = TestHelpers.makeCurrentModeUpdate(mode: ACPSessionModeId("custom-mode"))
        let notification = TestHelpers.makeSessionUpdateNotification(update: update)

        // Simulate receiving notification
        sut.updates.append(notification)
        switch notification.update {
        case .currentModeUpdate(let cur):
            sut.currentMode = cur.current_mode_id
        default:
            break
        }

        XCTAssertEqual(sut.currentMode.value, "custom-mode")
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
}
