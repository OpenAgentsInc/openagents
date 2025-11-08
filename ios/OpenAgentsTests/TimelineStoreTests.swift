import XCTest
@testable import OpenAgents
@testable import OpenAgentsCore

#if os(iOS)
@MainActor
final class TimelineStoreTests: XCTestCase {
    var store: TimelineStore!

    override func setUp() async throws {
        try await super.setUp()
        store = TimelineStore()
    }

    override func tearDown() async throws {
        store = nil
        try await super.tearDown()
    }

    func testRingBufferLimit200() throws {
        // Build 250 simple text updates
        for i in 1...250 {
            let update = TestHelpers.makeSessionUpdateNotification(update: TestHelpers.makeTextUpdate(text: "msg \(i)"))
            let data = try JSONEncoder().encode(update)
            store.applySessionUpdatePayload(data)
        }
        let exp = expectation(description: "publisher emits")
        var latest: [ACP.Client.SessionNotificationWire] = []
        let c = store.updatesPublisher.sink { arr in
            latest = arr
            exp.fulfill()
        }
        wait(for: [exp], timeout: 1.0)
        c.cancel()
        XCTAssertEqual(latest.count, 200)
    }

    func testAvailableCommandsUpdateReflects() throws {
        let command = ACP.Client.AvailableCommand(id: ACP.CommandId("cmd-1"), command_name: "Test Command", mode_id: .default_mode)
        let update = TestHelpers.makeAvailableCommandsUpdate(commands: [command])
        let wire = TestHelpers.makeSessionUpdateNotification(update: update)
        store.applySessionUpdatePayload(try JSONEncoder().encode(wire))
        let ac = try waitForValue(store.availableCommandsPublisher, timeout: 1.0)
        XCTAssertEqual(ac.count, 1)
        XCTAssertEqual(ac[0].command_name, "Test Command")
    }

    // Helper: await first emission
    private func waitForValue<T: Publisher>(_ publisher: T, timeout: TimeInterval) throws -> T.Output {
        let exp = expectation(description: "await publisher")
        var output: T.Output!
        let c = publisher.sink { _ in } receiveValue: { value in output = value; exp.fulfill() }
        wait(for: [exp], timeout: timeout)
        c.cancel()
        return output
    }
}
#endif
