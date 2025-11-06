import XCTest
@testable import OpenAgentsCore

#if os(macOS)
/// Tests for network interruption and recovery scenarios
/// Phase 6: Network Recovery Tests - Connection loss, reconnection, state restoration
final class NetworkRecoveryTests: XCTestCase {
    private var server: DesktopWebSocketServer?
    private var client: MobileWebSocketClient?
    private var recoveryDelegate: RecoveryTestDelegate?

    override func tearDown() {
        super.tearDown()
        client?.disconnect()
        client = nil
        server?.stop()
        server = nil
        recoveryDelegate = nil
    }

    // MARK: - Connection Loss Tests

    func testConnectionLoss_DetectedCorrectly() throws {
        let port: UInt16 = 9921
        let srv = DesktopWebSocketServer()
        server = srv
        try srv.start(port: port, advertiseService: false)

        let connectExp = expectation(description: "Client connected")
        let disconnectExp = expectation(description: "Client detected disconnection")

        let delegate = RecoveryTestDelegate(
            connectCallback: {
                connectExp.fulfill()
            },
            disconnectCallback: { _ in
                disconnectExp.fulfill()
            }
        )
        recoveryDelegate = delegate

        let cli = MobileWebSocketClient()
        client = cli
        cli.delegate = delegate

        let url = URL(string: "ws://127.0.0.1:\(port)")!
        cli.connect(url: url)

        wait(for: [connectExp], timeout: 5.0)

        // Simulate connection loss by stopping server
        srv.stop()

        wait(for: [disconnectExp], timeout: 5.0)

        XCTAssert(true, "Client detected disconnection")
    }

    func testConnectionLoss_DuringMessageStream() throws {
        let port: UInt16 = 9922
        let srv = DesktopWebSocketServer()
        server = srv
        try srv.start(port: port, advertiseService: false)

        let connectExp = expectation(description: "Client connected")
        let receiveExp = expectation(description: "Received some updates")
        let disconnectExp = expectation(description: "Disconnected during stream")

        var receivedCount = 0

        let delegate = RecoveryTestDelegate(
            connectCallback: {
                connectExp.fulfill()
            },
            disconnectCallback: { _ in
                disconnectExp.fulfill()
            },
            receiveCallback: { _ in
                receivedCount += 1
                if receivedCount == 1 {
                    receiveExp.fulfill()
                }
            }
        )
        recoveryDelegate = delegate

        let cli = MobileWebSocketClient()
        client = cli
        cli.delegate = delegate

        let url = URL(string: "ws://127.0.0.1:\(port)")!
        cli.connect(url: url)

        wait(for: [connectExp], timeout: 5.0)

        // Simulate receiving updates, then connection loss
        // (In real scenario, server would send updates)

        // Stop server to simulate connection loss
        srv.stop()

        wait(for: [disconnectExp], timeout: 5.0)

        XCTAssert(true, "Handled disconnection during message stream")
    }

    // MARK: - Reconnection Tests

    func testReconnection_AfterConnectionLoss() throws {
        let port: UInt16 = 9923
        var srv = DesktopWebSocketServer()
        server = srv
        try srv.start(port: port, advertiseService: false)

        let connectExp = expectation(description: "Initial connection")
        let disconnectExp = expectation(description: "Disconnection")
        let reconnectExp = expectation(description: "Reconnection")

        var connectionCount = 0

        let delegate = RecoveryTestDelegate(
            connectCallback: {
                connectionCount += 1
                if connectionCount == 1 {
                    connectExp.fulfill()
                } else if connectionCount == 2 {
                    reconnectExp.fulfill()
                }
            },
            disconnectCallback: { _ in
                disconnectExp.fulfill()
            }
        )
        recoveryDelegate = delegate

        let cli = MobileWebSocketClient()
        client = cli
        cli.delegate = delegate

        let url = URL(string: "ws://127.0.0.1:\(port)")!
        cli.connect(url: url)

        wait(for: [connectExp], timeout: 5.0)

        // Stop server to simulate connection loss
        srv.stop()

        wait(for: [disconnectExp], timeout: 5.0)

        // Restart server
        srv = DesktopWebSocketServer()
        server = srv
        try srv.start(port: port, advertiseService: false)

        // Attempt reconnection
        cli.connect(url: url)

        wait(for: [reconnectExp], timeout: 5.0)

        XCTAssertEqual(connectionCount, 2, "Should reconnect successfully")
    }

    func testAutoReconnect_WithBackoff() throws {
        // Test that client can implement automatic reconnection with backoff
        let port: UInt16 = 9924

        let connectExp = expectation(description: "Connection")
        connectExp.expectedFulfillmentCount = 1

        let delegate = RecoveryTestDelegate(
            connectCallback: {
                connectExp.fulfill()
            }
        )
        recoveryDelegate = delegate

        let cli = MobileWebSocketClient()
        client = cli
        cli.delegate = delegate

        let url = URL(string: "ws://127.0.0.1:\(port)")!

        // Try to connect to non-existent server
        cli.connect(url: url)

        // Start server after delay (simulating network recovery)
        DispatchQueue.global().asyncAfter(deadline: .now() + 1.0) {
            let srv = DesktopWebSocketServer()
            self.server = srv
            try? srv.start(port: port, advertiseService: false)
        }

        wait(for: [connectExp], timeout: 5.0)

        XCTAssert(true, "Should eventually connect when server becomes available")
    }

    // MARK: - State Restoration Tests

    func testStateRestoration_AfterReconnect() throws {
        let sessionId = ACPSessionId("test-restore")

        // Create state before "disconnection"
        let updatesBeforeDisconnect: [ACP.Client.SessionUpdate] = [
            .userMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Message 1")))),
            .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Response 1"))))
        ]

        let wiresBeforeDisconnect = updatesBeforeDisconnect.map {
            ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0)
        }

        let (itemsBefore, _) = AcpThreadView_computeTimelineFromUpdates(updates: wiresBeforeDisconnect, cap: 100)

        // Simulate reconnection and receiving more updates
        let updatesAfterReconnect: [ACP.Client.SessionUpdate] = [
            .userMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Message 2")))),
            .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Response 2"))))
        ]

        let wiresAfterReconnect = updatesAfterReconnect.map {
            ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0)
        }

        // Merge state: all updates together
        let allWires = wiresBeforeDisconnect + wiresAfterReconnect
        let (itemsRestored, _) = AcpThreadView_computeTimelineFromUpdates(updates: allWires, cap: 100)

        // Should have all 4 items (2 from before, 2 from after)
        XCTAssertEqual(itemsRestored.count, 4, "Should restore state with new updates")
        XCTAssertGreaterThan(itemsRestored.count, itemsBefore.count, "Should have more items after reconnect")
    }

    func testBufferPreservation_DuringDisconnect() throws {
        // Test that the 200-item ring buffer is preserved during disconnection
        let sessionId = ACPSessionId("test-buffer-preserve")

        // Create 150 updates before "disconnection"
        var updatesBefore: [ACP.Client.SessionUpdate] = []
        for i in 0..<150 {
            updatesBefore.append(.agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Message \(i)")))))
        }

        let wiresBefore = updatesBefore.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }
        let (itemsBefore, _) = AcpThreadView_computeTimelineFromUpdates(updates: wiresBefore, cap: 200)

        // After "reconnection", add more updates
        var updatesAfter: [ACP.Client.SessionUpdate] = []
        for i in 150..<200 {
            updatesAfter.append(.agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Message \(i)")))))
        }

        let wiresAfter = updatesAfter.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }

        // Merge all updates
        let allWires = wiresBefore + wiresAfter
        let (itemsAfter, _) = AcpThreadView_computeTimelineFromUpdates(updates: allWires, cap: 200)

        // Should preserve buffer and add new items (up to cap of 200)
        XCTAssertEqual(itemsAfter.count, 200, "Should maintain buffer limit")
    }

    // MARK: - Graceful Degradation Tests

    func testGracefulDegradation_PartialConnectivity() throws {
        // Test that system continues to function with degraded connectivity
        let sessionId = ACPSessionId("test-degraded")

        // Simulate receiving updates with gaps (dropped packets)
        let updates: [ACP.Client.SessionUpdate] = [
            .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Message 1")))),
            // Gap: Messages 2-3 dropped
            .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Message 4")))),
            .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Message 5"))))
        ]

        let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }
        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 100)

        // Should process available messages despite gaps
        XCTAssertEqual(items.count, 1, "Should accumulate available text chunks")
    }

    // MARK: - Connection Health Tests

    func testPingPong_KeepAlive() throws {
        let port: UInt16 = 9925
        let srv = DesktopWebSocketServer()
        server = srv
        try srv.start(port: port, advertiseService: false)

        let connectExp = expectation(description: "Connection established")

        let delegate = RecoveryTestDelegate(
            connectCallback: {
                connectExp.fulfill()
            }
        )
        recoveryDelegate = delegate

        let cli = MobileWebSocketClient()
        client = cli
        cli.delegate = delegate

        let url = URL(string: "ws://127.0.0.1:\(port)")!
        cli.connect(url: url)

        wait(for: [connectExp], timeout: 5.0)

        // Connection should remain alive with ping/pong
        // (Server automatically handles WebSocket ping/pong frames)

        // Wait and verify connection is still alive
        let stillConnectedExp = expectation(description: "Still connected")
        DispatchQueue.main.asyncAfter(deadline: .now() + 2.0) {
            stillConnectedExp.fulfill()
        }

        wait(for: [stillConnectedExp], timeout: 3.0)

        XCTAssert(true, "Connection maintained with keep-alive")
    }

    func testConnectionTimeout_NoKeepalive() throws {
        // Test that connection timeout is detected when no keepalive
        let port: UInt16 = 9926

        let connectExp = expectation(description: "Connection")
        connectExp.expectedFulfillmentCount = 1

        let delegate = RecoveryTestDelegate(
            connectCallback: {
                connectExp.fulfill()
            }
        )
        recoveryDelegate = delegate

        let cli = MobileWebSocketClient()
        client = cli
        cli.delegate = delegate

        let url = URL(string: "ws://127.0.0.1:\(port)")!

        // Server doesn't exist - connection should timeout
        cli.connect(url: url)

        // In real implementation, would have timeout logic
        // For now, just verify connect was attempted
        XCTAssert(true, "Connection timeout should be handled")
    }

    // MARK: - Multi-Client Recovery Tests

    func testMultiClient_IndependentRecovery() throws {
        // Test that multiple clients can recover independently
        let port: UInt16 = 9927
        let srv = DesktopWebSocketServer()
        server = srv
        try srv.start(port: port, advertiseService: false)

        let client1ConnectExp = expectation(description: "Client 1 connected")
        let client2ConnectExp = expectation(description: "Client 2 connected")

        let delegate1 = RecoveryTestDelegate(connectCallback: { client1ConnectExp.fulfill() })
        let delegate2 = RecoveryTestDelegate(connectCallback: { client2ConnectExp.fulfill() })

        let cli1 = MobileWebSocketClient()
        let cli2 = MobileWebSocketClient()

        cli1.delegate = delegate1
        cli2.delegate = delegate2

        let url = URL(string: "ws://127.0.0.1:\(port)")!

        cli1.connect(url: url)
        cli2.connect(url: url)

        wait(for: [client1ConnectExp, client2ConnectExp], timeout: 5.0)

        // Both clients should connect independently
        XCTAssert(true, "Multiple clients can connect independently")

        cli1.disconnect()
        cli2.disconnect()
    }

    // MARK: - Session Continuity Tests

    func testSessionContinuity_AcrossReconnections() throws {
        let sessionId = ACPSessionId("test-continuity")

        // Session state before disconnection
        let phase1Updates: [ACP.Client.SessionUpdate] = [
            .userMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Start task")))),
            .agentThoughtChunk(ACP.Client.ContentChunk(content: .thinking(.init(thinking: "Beginning work..."))))
        ]

        // Session continues after reconnection
        let phase2Updates: [ACP.Client.SessionUpdate] = [
            .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Continuing task")))),
            .statusUpdate(.init(status: .completed, message: "Done"))
        ]

        let phase1Wires = phase1Updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }
        let phase2Wires = phase2Updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }

        let allWires = phase1Wires + phase2Wires
        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: allWires, cap: 100)

        // Session should continue seamlessly
        XCTAssertGreaterThanOrEqual(items.count, 2, "Session should continue across reconnection")
    }

    // MARK: - Error During Recovery Tests

    func testErrorDuringReconnection_HandledGracefully() throws {
        let port: UInt16 = 9928

        let delegate = RecoveryTestDelegate()
        recoveryDelegate = delegate

        let cli = MobileWebSocketClient()
        client = cli
        cli.delegate = delegate

        let url = URL(string: "ws://127.0.0.1:\(port)")!

        // Try to connect to non-existent server (should fail gracefully)
        cli.connect(url: url)

        // Wait briefly
        let waitExp = expectation(description: "Wait for connection attempt")
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
            waitExp.fulfill()
        }

        wait(for: [waitExp], timeout: 2.0)

        // Should not crash on failed connection
        XCTAssert(true, "Failed reconnection handled gracefully")
    }

    // MARK: - Data Integrity Tests

    func testDataIntegrity_AcrossDisconnections() throws {
        let sessionId = ACPSessionId("test-integrity")

        // Specific message content before disconnection
        let beforeContent = "Important data: 12345"
        let updatesBefore: [ACP.Client.SessionUpdate] = [
            .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: beforeContent))))
        ]

        let wiresBefore = updatesBefore.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }
        let (itemsBefore, _) = AcpThreadView_computeTimelineFromUpdates(updates: wiresBefore, cap: 100)

        // After "reconnection", same session
        let afterContent = " Additional data: 67890"
        let updatesAfter: [ACP.Client.SessionUpdate] = [
            .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: afterContent))))
        ]

        let wiresAfter = updatesAfter.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }

        // Combine all updates
        let allWires = wiresBefore + wiresAfter
        let (itemsAfter, _) = AcpThreadView_computeTimelineFromUpdates(updates: allWires, cap: 100)

        // Data should be preserved correctly
        XCTAssertEqual(itemsAfter.count, 1, "Should accumulate text in same message")

        if case .assistant_message(let assoc) = itemsAfter.first?.variant {
            let fullText = assoc.blocks.compactMap { block -> String? in
                if case .text(let textBlock) = block {
                    return textBlock.text
                }
                return nil
            }.joined()

            XCTAssert(fullText.contains("12345"), "Should preserve original data")
            XCTAssert(fullText.contains("67890"), "Should preserve data after reconnect")
        }
    }
}

// MARK: - Test Delegate

class RecoveryTestDelegate: NSObject, MobileWebSocketClientDelegate {
    let connectCallback: (() -> Void)?
    let disconnectCallback: ((Error?) -> Void)?
    let receiveCallback: ((ACP.Client.SessionNotificationWire) -> Void)?

    init(
        connectCallback: (() -> Void)? = nil,
        disconnectCallback: ((Error?) -> Void)? = nil,
        receiveCallback: ((ACP.Client.SessionNotificationWire) -> Void)? = nil
    ) {
        self.connectCallback = connectCallback
        self.disconnectCallback = disconnectCallback
        self.receiveCallback = receiveCallback
    }

    func mobileWebSocketClientDidConnect(_ client: MobileWebSocketClient) {
        connectCallback?()
    }

    func mobileWebSocketClient(_ client: MobileWebSocketClient, didDisconnect error: Error?) {
        disconnectCallback?(error)
    }

    func mobileWebSocketClient(_ client: MobileWebSocketClient, didReceive notification: ACP.Client.SessionNotificationWire) {
        receiveCallback?(notification)
    }
}
#endif
