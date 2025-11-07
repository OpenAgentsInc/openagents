import XCTest
@testable import OpenAgentsCore

final class TinyvexJSONRPCTests: XCTestCase {
    func testJSONRPCResponseEncode() throws {
        struct Result: Codable { let ok: Bool }
        let resp = JSONRPCResponse(id: .int(1), result: Result(ok: true))
        let data = try JSONEncoder().encode(resp)
        let obj = try JSONSerialization.jsonObject(with: data) as? [String: Any]
        XCTAssertEqual(obj?["jsonrpc"] as? String, "2.0")
        XCTAssertEqual((obj?["id"] as? NSNumber)?.intValue, 1)
        let result = obj?["result"] as? [String: Any]
        XCTAssertEqual(result?["ok"] as? Bool, true)
    }
}

