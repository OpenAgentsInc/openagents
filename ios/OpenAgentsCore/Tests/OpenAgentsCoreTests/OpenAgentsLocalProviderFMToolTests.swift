import XCTest
@testable import OpenAgentsCore

final class OpenAgentsLocalProviderFMToolTests: XCTestCase {
    #if os(macOS)
    func testFMToolCodexRunEmitsToolCallAndInvokesServer() async throws {
        #if canImport(FoundationModels)
        if #available(iOS 26.0, macOS 26.0, *) {
            // Setup server with Tinyvex (to create updateHub)
            let server = DesktopWebSocketServer()
            let tmp = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent(UUID().uuidString).appendingPathExtension("sqlite")
            server.setTinyvexDb(path: tmp.path)

            // Prepare session
            let sid = ACPSessionId(UUID().uuidString)

            // Subscribe to notifications
            let expToolCall = expectation(description: "tool_call received")
            let expAgent = expectation(description: "agent response/error received")
            var sawToolCall = false
            var sawAgent = false
            let sub = server.notificationPublisher.sink { evt in
                guard evt.method == ACPRPC.sessionUpdate else { return }
                if let note = try? JSONDecoder().decode(ACP.Client.SessionNotificationWire.self, from: evt.payload) {
                    switch note.update {
                    case .toolCall(let w):
                        if w.name == "codex.run" { sawToolCall = true; expToolCall.fulfill() }
                    case .agentMessageChunk(let chunk):
                        if case .text(let t) = chunk.content, t.text.contains("Codex") || t.text.contains("❌") {
                            sawAgent = true; expAgent.fulfill()
                        }
                    default: break
                    }
                }
            }
            defer { sub.cancel() }

            // Invoke tool directly
            let hub = try XCTUnwrap(server.updateHub)
            let tool = OpenAgentsLocalProvider.FMTool_CodexRun(
                sessionId: sid,
                updateHub: hub,
                workspaceRoot: FileManager.default.currentDirectoryPath,
                server: server
            )
            let args = OpenAgentsLocalProvider.FMTool_CodexRun.Arguments(
                task: "delegate",
                description: "OpenAgents → Codex delegation",
                user_prompt: "list files",
                workspace_root: FileManager.default.currentDirectoryPath,
                files_include_glob: ["**/*"],
                summarize: true,
                max_files: 100
            )
            _ = try await tool.call(arguments: args)

            await fulfillment(of: [expToolCall], timeout: 2.0)
            // We may or may not have Codex installed in CI; if not, server emits an error agent message.
            await fulfillment(of: [expAgent], timeout: 3.0)

            XCTAssertTrue(sawToolCall)
            XCTAssertTrue(sawAgent)
            // Ensure working directory was applied
            XCTAssertEqual(server.workingDirectory?.path, FileManager.default.currentDirectoryPath)
        }
        #endif
    }
    #endif
}

