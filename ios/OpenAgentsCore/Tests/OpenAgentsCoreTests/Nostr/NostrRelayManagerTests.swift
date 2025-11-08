import XCTest
@testable import OpenAgentsCore

final class NostrRelayManagerTests: XCTestCase {
    func testAddRemoveRelays() throws {
        let start = [URL(string: "wss://relay.example.com")!]
        let mgr = NostrRelayManager(relayURLs: start)
        XCTAssertEqual(mgr.relays.count, 1)
        XCTAssertEqual(mgr.relays.first?.url.absoluteString, "wss://relay.example.com")

        let url2 = URL(string: "wss://relay2.example.com")!
        try mgr.addRelay(url: url2)
        XCTAssertEqual(mgr.relays.count, 2)

        // Duplicate is ignored
        try mgr.addRelay(url: url2)
        XCTAssertEqual(mgr.relays.count, 2)

        mgr.removeRelay(url: url2)
        XCTAssertEqual(mgr.relays.count, 1)
    }
}

