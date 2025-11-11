import XCTest
import Combine
@testable import OpenAgents
import OpenAgentsCore

@MainActor
final class ChatSendWiringTests: XCTestCase {
    var cancellables: Set<AnyCancellable> = []

    #if os(macOS)
    /// Verify BridgeManager.preferredModeForSend maps selection and currentMode correctly.
    func testPreferredModeForSendMapping() {
        let bm = BridgeManager()
        // No selection, default mode → default_mode (persisted default applies)
        bm.selectedAgent = nil
        bm.currentMode = .default_mode
        XCTAssertEqual(bm.preferredModeForSend(), .default_mode)

        // No selection, explicit current mode → that mode
        bm.currentMode = .claude_code
        XCTAssertEqual(bm.preferredModeForSend(), .claude_code)

        // Selected agent with "Codex" in name → .codex
        let cmd = ACP.Client.AvailableCommand(name: "Codex", description: "", input: .unstructured(hint: ""))
        bm.selectedAgent = cmd
        XCTAssertEqual(bm.preferredModeForSend(), .codex)

        // Selected agent with "Claude" in name → .claude_code
        let cmd2 = ACP.Client.AvailableCommand(name: "Claude Code", description: "", input: .unstructured(hint: ""))
        bm.selectedAgent = cmd2
        XCTAssertEqual(bm.preferredModeForSend(), .claude_code)
    }

    /// End-to-end: BridgeManager.sendPrompt uses preferred mode and triggers provider via LocalJsonRpcClient.
    func testSendPromptRespectsPreferredModeAndEmitsAgentMessage() async throws {
        let server = DesktopWebSocketServer()
        // Use a temp Tinyvex DB to back SessionUpdateHub
        let dbPath = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent(UUID().uuidString).appendingPathExtension("sqlite")
        server.setTinyvexDb(path: dbPath.path)

        // Register a mock provider under codex mode
        final class MockProvider: AgentProvider {
            let id: ACPSessionModeId = .codex
            let displayName: String = "Mock Codex"
            let capabilities = AgentCapabilities(executionMode: .native, streamingMode: .acp)
            func isAvailable() async -> Bool { true }
            func start(sessionId: ACPSessionId, prompt: String, context: AgentContext, updateHub: SessionUpdateHub) async throws -> AgentHandle {
                await updateHub.sendSessionUpdate(sessionId: sessionId, update: .agentMessageChunk(.init(content: .text(.init(text: "Echo: \(prompt)")))))
                return AgentHandle(sessionId: sessionId, mode: id, isStarted: true)
            }
            func resume(sessionId: ACPSessionId, prompt: String, handle: AgentHandle, context: AgentContext, updateHub: SessionUpdateHub) async throws {
                await updateHub.sendSessionUpdate(sessionId: sessionId, update: .agentMessageChunk(.init(content: .text(.init(text: "Resume: \(prompt)")))))
            }
            func cancel(sessionId: ACPSessionId, handle: AgentHandle) async { }
        }
        await server.registerProvider(MockProvider())

        let rpc = LocalJsonRpcClient(server: server)

        // Wire a BridgeManager with our dispatcher and timeline
        let bm = BridgeManager()
        let store = TimelineStore()
        bm.dispatcher = PromptDispatcher(rpc: rpc, timeline: store)
        bm.timeline = store
        bm.selectedAgent = ACP.Client.AvailableCommand(name: "Codex", description: "", input: .unstructured(hint: ""))

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

        // Send through BridgeManager using preferredMode
        let desired = bm.preferredModeForSend()
        bm.sendPrompt(text: "Hello from Composer", desiredMode: desired)

        await fulfillment(of: [gotAgent], timeout: 2.0)
        XCTAssertEqual(text, "Echo: Hello from Composer")
    }
    /// Verify conversational questions auto-route to OpenAgents even when Codex mode is selected
    func testConversationalQuestionAutoRoutesToOrchestrator() async throws {
        let server = DesktopWebSocketServer()
        let dbPath = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent(UUID().uuidString).appendingPathExtension("sqlite")
        server.setTinyvexDb(path: dbPath.path)

        // Register OpenAgents provider (default mode) that responds to conversational questions
        final class MockOrchestratorProvider: AgentProvider {
            let id: ACPSessionModeId = .default_mode
            let displayName: String = "OpenAgents"
            let capabilities = AgentCapabilities(executionMode: .native, streamingMode: .acp)
            func isAvailable() async -> Bool { true }
            func start(sessionId: ACPSessionId, prompt: String, context: AgentContext, updateHub: SessionUpdateHub) async throws -> AgentHandle {
                await updateHub.sendSessionUpdate(sessionId: sessionId, update: .agentMessageChunk(.init(content: .text(.init(text: "I am OpenAgents, your coding assistant!")))))
                return AgentHandle(sessionId: sessionId, mode: id, isStarted: true)
            }
            func resume(sessionId: ACPSessionId, prompt: String, handle: AgentHandle, context: AgentContext, updateHub: SessionUpdateHub) async throws {
                await updateHub.sendSessionUpdate(sessionId: sessionId, update: .agentMessageChunk(.init(content: .text(.init(text: "I am OpenAgents (resumed)")))))
            }
            func cancel(sessionId: ACPSessionId, handle: AgentHandle) async { }
        }

        // Register Codex provider that echoes the prompt
        final class MockCodexProvider: AgentProvider {
            let id: ACPSessionModeId = .codex
            let displayName: String = "Codex"
            let capabilities = AgentCapabilities(executionMode: .native, streamingMode: .acp)
            func isAvailable() async -> Bool { true }
            func start(sessionId: ACPSessionId, prompt: String, context: AgentContext, updateHub: SessionUpdateHub) async throws -> AgentHandle {
                await updateHub.sendSessionUpdate(sessionId: sessionId, update: .agentMessageChunk(.init(content: .text(.init(text: "Codex executing: \(prompt)")))))
                return AgentHandle(sessionId: sessionId, mode: id, isStarted: true)
            }
            func resume(sessionId: ACPSessionId, prompt: String, handle: AgentHandle, context: AgentContext, updateHub: SessionUpdateHub) async throws {
                await updateHub.sendSessionUpdate(sessionId: sessionId, update: .agentMessageChunk(.init(content: .text(.init(text: "Codex resume: \(prompt)")))))
            }
            func cancel(sessionId: ACPSessionId, handle: AgentHandle) async { }
        }

        await server.registerProvider(MockOrchestratorProvider())
        await server.registerProvider(MockCodexProvider())

        let rpc = LocalJsonRpcClient(server: server)
        let bm = BridgeManager()
        let store = TimelineStore()
        bm.dispatcher = PromptDispatcher(rpc: rpc, timeline: store)
        bm.timeline = store

        // Select Codex as the preferred mode
        bm.selectedAgent = ACP.Client.AvailableCommand(name: "Codex", description: "", input: .unstructured(hint: ""))

        // Subscribe to server notifications
        let gotResponse = expectation(description: "got response")
        var responseText: String?
        let sub = server.notificationPublisher.sink { evt in
            guard evt.method == ACPRPC.sessionUpdate else { return }
            if let note = try? JSONDecoder().decode(ACP.Client.SessionNotificationWire.self, from: evt.payload) {
                if case .agentMessageChunk(let chunk) = note.update,
                   case .text(let t) = chunk.content {
                    responseText = t.text
                    gotResponse.fulfill()
                }
            }
        }
        defer { sub.cancel() }

        // Send a conversational question (should route to OpenAgents, not Codex)
        bm.sendPrompt(text: "who are you", desiredMode: .codex)

        await fulfillment(of: [gotResponse], timeout: 2.0)

        // Should have been answered by OpenAgents orchestrator, not Codex
        XCTAssertEqual(responseText, "I am OpenAgents, your coding assistant!")
    }

    /// Verify coding tasks still route to selected agent (Codex) even with conversational detection
    func testCodingTaskRoutesToSelectedAgent() async throws {
        let server = DesktopWebSocketServer()
        let dbPath = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent(UUID().uuidString).appendingPathExtension("sqlite")
        server.setTinyvexDb(path: dbPath.path)

        // Register OpenAgents provider
        final class MockOrchestratorProvider: AgentProvider {
            let id: ACPSessionModeId = .default_mode
            let displayName: String = "OpenAgents"
            let capabilities = AgentCapabilities(executionMode: .native, streamingMode: .acp)
            func isAvailable() async -> Bool { true }
            func start(sessionId: ACPSessionId, prompt: String, context: AgentContext, updateHub: SessionUpdateHub) async throws -> AgentHandle {
                await updateHub.sendSessionUpdate(sessionId: sessionId, update: .agentMessageChunk(.init(content: .text(.init(text: "OpenAgents: \(prompt)")))))
                return AgentHandle(sessionId: sessionId, mode: id, isStarted: true)
            }
            func resume(sessionId: ACPSessionId, prompt: String, handle: AgentHandle, context: AgentContext, updateHub: SessionUpdateHub) async throws { }
            func cancel(sessionId: ACPSessionId, handle: AgentHandle) async { }
        }

        // Register Codex provider
        final class MockCodexProvider: AgentProvider {
            let id: ACPSessionModeId = .codex
            let displayName: String = "Codex"
            let capabilities = AgentCapabilities(executionMode: .native, streamingMode: .acp)
            func isAvailable() async -> Bool { true }
            func start(sessionId: ACPSessionId, prompt: String, context: AgentContext, updateHub: SessionUpdateHub) async throws -> AgentHandle {
                await updateHub.sendSessionUpdate(sessionId: sessionId, update: .agentMessageChunk(.init(content: .text(.init(text: "Codex: \(prompt)")))))
                return AgentHandle(sessionId: sessionId, mode: id, isStarted: true)
            }
            func resume(sessionId: ACPSessionId, prompt: String, handle: AgentHandle, context: AgentContext, updateHub: SessionUpdateHub) async throws { }
            func cancel(sessionId: ACPSessionId, handle: AgentHandle) async { }
        }

        await server.registerProvider(MockOrchestratorProvider())
        await server.registerProvider(MockCodexProvider())

        let rpc = LocalJsonRpcClient(server: server)
        let bm = BridgeManager()
        let store = TimelineStore()
        bm.dispatcher = PromptDispatcher(rpc: rpc, timeline: store)
        bm.timeline = store

        // Select Codex
        bm.selectedAgent = ACP.Client.AvailableCommand(name: "Codex", description: "", input: .unstructured(hint: ""))

        let gotResponse = expectation(description: "got response")
        var responseText: String?
        let sub = server.notificationPublisher.sink { evt in
            guard evt.method == ACPRPC.sessionUpdate else { return }
            if let note = try? JSONDecoder().decode(ACP.Client.SessionNotificationWire.self, from: evt.payload) {
                if case .agentMessageChunk(let chunk) = note.update,
                   case .text(let t) = chunk.content {
                    responseText = t.text
                    gotResponse.fulfill()
                }
            }
        }
        defer { sub.cancel() }

        // Send a coding task (should route to Codex as selected)
        bm.sendPrompt(text: "fix the bug in main.swift", desiredMode: .codex)

        await fulfillment(of: [gotResponse], timeout: 2.0)

        // Should have been handled by Codex, not OpenAgents
        XCTAssertEqual(responseText, "Codex: fix the bug in main.swift")
    }
    #else
    func testSkipOnIOS() throws { throw XCTSkip("macOS-only") }
    #endif
}
