import XCTest
import Combine
import NostrSDK
@testable import OpenAgentsCore

final class NostrEventFeedManagerTests: XCTestCase {
    private var cancellables: Set<AnyCancellable> = []

    func testKindFilterRanges() {
        XCTAssertTrue(NostrEventFeedManager.KindFilter.all.kinds.contains(5000))
        XCTAssertTrue(NostrEventFeedManager.KindFilter.all.kinds.contains(7000))
        XCTAssertTrue(NostrEventFeedManager.KindFilter.results.kinds.contains(6001))
        XCTAssertEqual(NostrEventFeedManager.KindFilter.feedback.kinds, [7000])
    }

    func testEventTransformAndDeduplication() async {
        let mock = MockRelayManager()
        let mgr = NostrEventFeedManager(relayManager: mock)

        await mgr.startFeed()

        let e1 = makeEvent(kind: 5000, content: "hello")
        let e2 = makeEvent(kind: 6000, content: "world")
        let e3 = makeEvent(kind: 7000, content: "feedback")

        mock.emit(e1)
        mock.emit(e2)
        mock.emit(e3)

        // Allow batching time to pass
        let expect = expectation(description: "batched")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) { expect.fulfill() }
        await fulfillment(of: [expect], timeout: 1.0)

        XCTAssertEqual(mgr.events.count, 3)
        // newest first
        XCTAssertEqual(mgr.events[0].event.id, e3.id)
        XCTAssertEqual(mgr.events[1].event.id, e2.id)
        XCTAssertEqual(mgr.events[2].event.id, e1.id)

        // Dedup: emit e2 again
        mock.emit(e2)
        let expect2 = expectation(description: "batched2")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) { expect2.fulfill() }
        await fulfillment(of: [expect2], timeout: 1.0)
        XCTAssertEqual(mgr.events.count, 3)
    }

    // MARK: - Helpers
    private func makeEvent(kind: Int, content: String) -> NostrEvent {
        let pubkey = String(repeating: "a", count: 64)
        return NostrEvent(kind: .unknown(kind), content: content, tags: [], pubkey: pubkey)
    }
}

private final class MockRelayManager: NostrEventFeedManager.NostrRelayManaging {
    let subject = PassthroughSubject<NostrEvent, Never>()

    func connect() async throws { /* no-op */ }
    func disconnect() { /* no-op */ }
    func subscribe(kinds: [Int], limit: Int, since: Date?, until: Date?) -> AnyPublisher<NostrEvent, Never> {
        subject.eraseToAnyPublisher()
    }

    func emit(_ event: NostrEvent) { subject.send(event) }
}

