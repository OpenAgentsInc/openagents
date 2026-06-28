import Foundation
import XCTest
@testable import Khala

final class KhalaClientTests: XCTestCase {
    func testParseOpenAICompatibleContent() throws {
        let data = Data("""
        {"choices":[{"message":{"role":"assistant","content":"Ready."}}]}
        """.utf8)
        XCTAssertEqual(try KhalaClient.parseContent(from: data), "Ready.")
    }

    func testParseRejectsUnexpectedShape() {
        let data = Data(#"{"choices":[]}"#.utf8)
        XCTAssertThrowsError(try KhalaClient.parseContent(from: data))
    }
}
