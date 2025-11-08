import XCTest
import Combine
@testable import OpenAgents

#if os(iOS)
@MainActor
final class ConnectionManagerTests: XCTestCase {
    var manager: MobileConnectionManager!
    var cancellables: Set<AnyCancellable>!

    override func setUp() async throws {
        try await super.setUp()
        manager = MobileConnectionManager()
        cancellables = []
    }

    override func tearDown() async throws {
        manager.stop()
        manager = nil
        cancellables = nil
        try await super.tearDown()
    }

    func testStartEmitsConnecting() {
        let exp = expectation(description: "connecting emitted")
        manager.statusPublisher
            .sink { st in
                if case .connecting = st { exp.fulfill() }
            }
            .store(in: &cancellables)
        manager.start()
        wait(for: [exp], timeout: 2.0)
    }
}
#endif

