import XCTest
@testable import OpenAgents
import OpenAgentsCore

#if os(macOS)
@MainActor
final class TinyvexTitleTests: XCTestCase {
    func testSetGetClearTitle() async throws {
        let server = DesktopWebSocketServer()
        let dbURL = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("TinyvexTitle-").appendingPathExtension("sqlite")
        server.setTinyvexDb(path: dbURL.path)
        let rpc = LocalJsonRpcClient(server: server)

        // Create a session id to use for title ops
        let sessionId = ACPSessionId(UUID().uuidString).value

        struct SetParams: Codable { let session_id: String; let title: String }
        struct GetParams: Codable { let session_id: String }
        struct GetResp: Codable { let title: String? }

        // Set
        let setExp = expectation(description: "set")
        rpc.sendJSONRPC(method: "tinyvex/history.setSessionTitle", params: SetParams(session_id: sessionId, title: "Hello World"), id: "set") { (_: [String: Bool]?) in
            setExp.fulfill()
        }
        await fulfillment(of: [setExp], timeout: 2.0)

        // Get and verify
        let getExp = expectation(description: "get")
        var got: String?
        rpc.sendJSONRPC(method: "tinyvex/history.getSessionTitle", params: GetParams(session_id: sessionId), id: "get") { (resp: GetResp?) in
            got = resp?.title
            getExp.fulfill()
        }
        await fulfillment(of: [getExp], timeout: 2.0)
        XCTAssertEqual(got, "Hello World")

        // Clear
        struct ClearParams: Codable { let session_id: String }
        let clearExp = expectation(description: "clear")
        rpc.sendJSONRPC(method: "tinyvex/history.clearSessionTitle", params: ClearParams(session_id: sessionId), id: "clear") { (_: [String: Bool]?) in
            clearExp.fulfill()
        }
        await fulfillment(of: [clearExp], timeout: 2.0)

        // Get should be nil after clear
        let get2 = expectation(description: "get2")
        var got2: String?
        rpc.sendJSONRPC(method: "tinyvex/history.getSessionTitle", params: GetParams(session_id: sessionId), id: "get2") { (resp: GetResp?) in
            got2 = resp?.title
            get2.fulfill()
        }
        await fulfillment(of: [get2], timeout: 2.0)
        XCTAssertNil(got2)
    }
}
#endif

