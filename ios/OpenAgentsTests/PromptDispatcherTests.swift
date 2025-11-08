import XCTest
@testable import OpenAgents
@testable import OpenAgentsCore

#if os(iOS)
final class MockRPCClient: JSONRPCSending {
    struct Call { let method: String }
    var calls: [Call] = []

    func sendJSONRPC<P, R>(method: String, params: P, id: String, completion: @escaping (R?) -> Void) where P : Decodable, P : Encodable, R : Decodable, R : Encodable {
        calls.append(.init(method: method))
        if method == ACPRPC.sessionNew {
            let resp = ACP.Agent.SessionNewResponse(session_id: ACPSessionId("new-session"))
            completion(resp as? R)
            return
        }
        completion(nil)
    }

    func sendJSONRPCNotification<P>(method: String, params: P) where P : Decodable, P : Encodable {
        calls.append(.init(method: method))
    }
}

@MainActor
final class PromptDispatcherTests: XCTestCase {
    func testSendPrompt_NoSession_CreatesSessionAndSendsPrompt() {
        let rpc = MockRPCClient()
        let store = TimelineStore()
        let dispatcher = PromptDispatcher(rpc: rpc, timeline: store)

        var sessionId: ACPSessionId?
        dispatcher.sendPrompt(text: "Hello", desiredMode: nil, getSessionId: { sessionId }, setSessionId: { sessionId = $0 })

        // Expect session/new followed by session/prompt
        let methods = rpc.calls.map { $0.method }
        XCTAssertTrue(methods.contains(ACPRPC.sessionNew))
        XCTAssertTrue(methods.contains(ACPRPC.sessionPrompt))
    }

    func testCancelCurrentSession_SendsNotification() {
        let rpc = MockRPCClient()
        let store = TimelineStore()
        let dispatcher = PromptDispatcher(rpc: rpc, timeline: store)

        let sid = ACPSessionId("s1")
        dispatcher.cancelCurrentSession(getSessionId: { sid })
        XCTAssertTrue(rpc.calls.contains { $0.method == ACPRPC.sessionCancel })
    }
}
#endif

