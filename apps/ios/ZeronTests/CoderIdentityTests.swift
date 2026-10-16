import XCTest
@testable import Zeron

final class CoderIdentityTests: XCTestCase {
    func testAppIdentityAndServer() {
        XCTAssertEqual(Bundle.main.bundleIdentifier, "com.openagents.coder")
        XCTAssertEqual(Bundle.main.object(forInfoDictionaryKey: "CFBundleDisplayName") as? String, "OpenAgents Coder")
        XCTAssertEqual(productionEdgeUrl(), "https://sync.openagents.com")
        let types = Bundle.main.object(forInfoDictionaryKey: "CFBundleURLTypes") as? [[String: Any]]
        XCTAssertEqual(types?.first?["CFBundleURLSchemes"] as? [String], ["openagents-coder"])
    }

    func testAttributionIsBundled() throws {
        let url = try XCTUnwrap(Bundle.main.url(forResource: "Zeron-MIT", withExtension: "txt"))
        let notice = try String(contentsOf: url, encoding: .utf8)
        XCTAssertTrue(notice.contains("MIT License"))
        XCTAssertTrue(notice.contains("Wing"))
    }
}
