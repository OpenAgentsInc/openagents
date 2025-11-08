import Foundation

#if canImport(FoundationModels)
import FoundationModels

/// Native FM tool-calling orchestrator
@available(iOS 26.0, macOS 26.0, *)
actor NativeFMOrchestrator {
    private let workspaceRoot: String
    private let goals: [String]
    private let stream: ACPUpdateStreamHandler
    private var fmSession: LanguageModelSession?
    private var sessionTurnCount: Int = 0

    init(workspaceRoot: String, goals: [String], stream: @escaping ACPUpdateStreamHandler) {
        self.workspaceRoot = workspaceRoot
        self.goals = goals
        self.stream = stream
    }

    private func getOrCreateSession() async throws -> LanguageModelSession {
        if let existing = fmSession { return existing }
        let tools = FMToolsRegistry.defaultTools(workspaceRoot: workspaceRoot)
        let instructions = Instructions("""
        You are a workspace exploration assistant. Use the available tools to explore the workspace and achieve the user's goals.

        Available tools:
        - session.list: List recent conversation sessions
        - session.search: Search sessions for patterns
        - session.read: Read session content
        - session.analyze: Analyze sessions for insights
        - content.get_span: Read file content
        - code.grep: Search code
        - fs.list_dir: List directory contents

        After using tools, summarize your findings and suggest next steps.
        """)
        let session = LanguageModelSession(model: SystemLanguageModel.default, tools: tools, instructions: instructions)
        session.prewarm(promptPrefix: nil)
        fmSession = session
        sessionTurnCount = 0
        OpenAgentsLog.orchestration.info("FM session created with \(tools.count) tools")
        return session
    }

    func executeNativeToolCallingLoop() async throws -> ExploreSummary {
        let session = try await getOrCreateSession()
        sessionTurnCount += 1
        let workspaceName = (workspaceRoot as NSString).lastPathComponent
        let goalsStr = goals.isEmpty ? "(explore the workspace)" : goals.joined(separator: "\n- ")
        let prompt = """
        Workspace: \(workspaceName)
        Goals:
        - \(goalsStr)

        Use the available tools to explore the workspace and achieve these goals. Start by analyzing recent sessions to understand the user's work patterns, then use other tools as needed.

        After using tools, provide a summary of your findings.
        """
        OpenAgentsLog.orchestration.info("Starting native tool calling (turn \(self.sessionTurnCount))")
        let t0 = Date()
        let response = try await session.respond(to: prompt)
        OpenAgentsLog.orchestration.debug("FM response received in \(String(format: "%.2f", Date().timeIntervalSince(t0)))s")
        OpenAgentsLog.orchestration.debug("Response: \(response.content.prefix(200))...")
        let chunk = ACP.Client.ContentChunk(content: .text(.init(text: response.content)))
        await stream(.agentMessageChunk(chunk))
        return try await generateSummaryFromResponse(response.content)
    }

    private func generateSummaryFromResponse(_ content: String) async throws -> ExploreSummary {
        let workspaceName = (workspaceRoot as NSString).lastPathComponent
        return ExploreSummary(repo_name: workspaceName, languages: [:], entrypoints: [], top_files: [], followups: [content])
    }
}
#endif
