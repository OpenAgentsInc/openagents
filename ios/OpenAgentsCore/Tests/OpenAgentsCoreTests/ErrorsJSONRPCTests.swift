import XCTest
@testable import OpenAgentsCore

final class ErrorsJSONRPCTests: XCTestCase {
    func testJSONRPCErrorResponseEncode() throws {
        let err = JSONRPC.ErrorObject(code: -32601, message: "Method not found")
        let env = JSONRPC.ErrorResponse(id: JSONRPC.ID("99"), error: err)
        let data = try JSONEncoder().encode(env)
        let s = String(data: data, encoding: .utf8)!
        XCTAssertTrue(s.contains("\"jsonrpc\":\"2.0\""))
        XCTAssertTrue(s.contains("\"error\""))
        XCTAssertTrue(s.contains("-32601"))
    }
}

