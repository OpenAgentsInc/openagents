import XCTest
@testable import OpenAgents

final class VoiceHostLabelTests: XCTestCase {
    func testRenameUpdatesTheSameCallHost() {
        var names = ["mac": "MacBook Pro"]
        var label = VoiceHostLabel(deviceId: "mac", name: "MacBook Pro")
        XCTAssertEqual(label.resolve { names[$0] }, "MacBook Pro")

        names["mac"] = "Studio Mac"
        XCTAssertEqual(label.resolve { names[$0] }, "Studio Mac")
        XCTAssertEqual(label.deviceId, "mac")
    }

    func testNextCallHostDoesNotReplaceTheActiveHost() {
        var names = ["active": "Call host", "next": "Next call host"]
        var label = VoiceHostLabel(deviceId: "active", name: "Call host")
        var nextHost = "active"
        nextHost = "next"
        names[nextHost] = "Renamed next host"
        XCTAssertEqual(label.resolve { names[$0] }, "Call host")

        names["active"] = "Renamed call host"
        XCTAssertEqual(label.resolve { names[$0] }, "Renamed call host")
        XCTAssertEqual(label.deviceId, "active")
    }

    func testMissingMetadataKeepsTheLastAvailableName() {
        var label = VoiceHostLabel(deviceId: "mac", name: "MacBook Pro")
        XCTAssertEqual(label.resolve { _ in nil }, "MacBook Pro")
        XCTAssertEqual(label.resolve { _ in "Studio Mac" }, "Studio Mac")
        XCTAssertEqual(label.resolve { _ in nil }, "Studio Mac")
        XCTAssertEqual(label.resolve { _ in "" }, "Studio Mac")
    }
}
