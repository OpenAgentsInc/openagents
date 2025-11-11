import XCTest
import Combine
@testable import OpenAgents
import OpenAgentsCore

@MainActor
final class ChatSendWiringTests: XCTestCase {
    var cancellables: Set<AnyCancellable> = []

    #if os(macOS)
    /// Verify BridgeManager.sendPrompt always routes to OpenAgents orchestrator
    func testSendPromptAlwaysRoutesToOrchestrator() async throws {
        let server = DesktopWebSocketServer()
        let dbPath = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent(UUID().uuidString).appendingPathExtension("sqlite")
        server.setTinyvexDb(path: dbPath.path)

        // Register OpenAgents provider (default mode)
        final class MockOrchestratorProvider: AgentProvider {
            let id: ACPSessionModeId = .default_mode
            let displayName: String = "OpenAgents"
            let capabilities = AgentCapabilities(executionMode: .native, streamingMode: .acp)
            func isAvailable() async -> Bool { true }
            func start(sessionId: ACPSessionId, prompt: String, context: AgentContext, updateHub: SessionUpdateHub) async throws -> AgentHandle {
                await updateHub.sendSessionUpdate(sessionId: sessionId, update: .agentMessageChunk(.init(content: .text(.init(text: "OpenAgents: \(prompt)")))))
                return AgentHandle(sessionId: sessionId, mode: id, isStarted: true)
            }
            func resume(sessionId: ACPSessionId, prompt: String, handle: AgentHandle, context: AgentContext, updateHub: SessionUpdateHub) async throws {
                await updateHub.sendSessionUpdate(sessionId: sessionId, update: .agentMessageChunk(.init(content: .text(.init(text: "OpenAgents resume: \(prompt)")))))
            }
            func cancel(sessionId: ACPSessionId, handle: AgentHandle) async { }
        }

        await server.registerProvider(MockOrchestratorProvider())

        let rpc = LocalJsonRpcClient(server: server)
        let bm = BridgeManager()
        let store = TimelineStore()
        bm.dispatcher = PromptDispatcher(rpc: rpc, timeline: store)
        bm.timeline = store

        // Subscribe to server notifications to capture agent message
        let gotAgent = expectation(description: "got agent message")
        var text: String?
        let sub = server.notificationPublisher.sink { evt in
            guard evt.method == ACPRPC.sessionUpdate else { return }
            if let note = try? JSONDecoder().decode(ACP.Client.SessionNotificationWire.self, from: evt.payload) {
                if case .agentMessageChunk(let chunk) = note.update,
                   case .text(let t) = chunk.content {
                    text = t.text
                    gotAgent.fulfill()
                }
            }
        }
        defer { sub.cancel() }

        // Send prompt - should always route to OpenAgents orchestrator
        bm.sendPrompt(text: "Hello from Composer")

        await fulfillment(of: [gotAgent], timeout: 2.0)
        XCTAssertEqual(text, "OpenAgents: Hello from Composer")
    }
    #else
    func testSkipOnIOS() throws { throw XCTSkip("macOS-only") }
    #endif
}
