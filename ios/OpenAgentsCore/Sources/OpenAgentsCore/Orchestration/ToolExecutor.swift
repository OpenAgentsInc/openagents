// ToolExecutor.swift — Executes AgentOp operations with bounded output
// Part of Phase 2: orchestrate.explore.start implementation

import Foundation

/// Executes agent operations and returns typed results
public struct ToolExecutor: Sendable {
    /// Workspace root path (all operations scoped within)
    private let workspaceRoot: String
    /// Optional progress sink for long-running tools
    private let progressSink: (@Sendable (_ op: AgentOp, _ fraction: Double, _ note: String?) async -> Void)?

    public init(workspaceRoot: String, progress: (@Sendable (_ op: AgentOp, _ fraction: Double, _ note: String?) async -> Void)? = nil) {
        self.workspaceRoot = workspaceRoot
        self.progressSink = progress
    }

    /// Execute an operation and return typed result
    public func execute(_ op: AgentOp) async throws -> any Encodable {
        switch op.kind {
        case .readSpan(let params):
            return try await executeReadSpan(params)

        case .grep(let params):
            return try await executeGrep(params)

        case .listDir(let params):
            return try await executeListDir(params)

        case .indexRepo:
            throw ToolExecutionError.notImplemented("indexRepo is Phase 3+")

        case .semantic:
            throw ToolExecutionError.notImplemented("semantic search is Phase 3+")

        // MARK: - Session History Tools (Phase 2.5)

        case .sessionList(let params):
            return try await executeSessionList(params)

        case .sessionSearch(let params):
            return try await executeSessionSearch(params)

        case .sessionRead(let params):
            return try await executeSessionRead(params)

        case .sessionAnalyze(let params):
            return try await executeSessionAnalyze(params, op: op)
        }
    }

    // MARK: - ReadSpan Execution

    private func executeReadSpan(_ params: ReadSpanParams) async throws -> ContentSpanResult {
        let tool = ContentSpanTool(workspaceRoot: workspaceRoot)
        return try await tool.readSpan(
            path: params.path,
            startLine: params.startLine,
            endLine: params.endLine,
            context: params.context
        )
    }

    // MARK: - Grep Execution

    private func executeGrep(_ params: GrepParams) async throws -> GrepResult {
        let tool = GrepTool(workspaceRoot: workspaceRoot)
        return try await tool.grep(
            pattern: params.pattern,
            pathPrefix: params.pathPrefix,
            caseInsensitive: params.caseInsensitive,
            maxResults: params.maxResults
        )
    }

    // MARK: - ListDir Execution

    private func executeListDir(_ params: ListDirParams) async throws -> [String] {
        let scanner = WorkspaceScanner(workspaceRoot: workspaceRoot)
        return try await scanner.listDirectory(path: params.path, depth: params.depth)
    }

    // MARK: - Session History Execution (Phase 2.5)

    private func executeSessionList(_ params: SessionListParams) async throws -> SessionListResult {
        let tool = SessionListTool()
        return try await tool.list(
            provider: params.provider,
            topK: params.topK,
            since: params.since
        )
    }

    private func executeSessionSearch(_ params: SessionSearchParams) async throws -> SessionSearchResult {
        let tool = SessionSearchTool()
        return try await tool.search(
            pattern: params.pattern,
            provider: params.provider,
            sessionIds: params.sessionIds,
            maxResults: params.maxResults,
            contextLines: params.contextLines
        )
    }

    private func executeSessionRead(_ params: SessionReadParams) async throws -> SessionReadResult {
        let tool = SessionReadTool()
        return try await tool.read(
            sessionId: params.sessionId,
            provider: params.provider,
            startLine: params.startLine,
            endLine: params.endLine,
            maxEvents: params.maxEvents
        )
    }

    private func executeSessionAnalyze(_ params: SessionAnalyzeParams, op: AgentOp) async throws -> SessionAnalyzeResult {
        let tool = SessionAnalyzeTool()
        return try await tool.analyze(
            sessionIds: params.sessionIds,
            provider: params.provider,
            metrics: params.metrics,
            progress: { processed, total in
                let fraction = total > 0 ? Double(processed) / Double(total) : 0.0
                if let sink = progressSink {
                    Task { await sink(op, fraction, "processed \(processed)/\(total)") }
                }
            }
        )
    }
}

/// Tool execution errors
public enum ToolExecutionError: Error, LocalizedError {
    case pathOutsideWorkspace(String)
    case fileNotFound(String)
    case permissionDenied(String)
    case invalidParameters(String)
    case executionFailed(String)
    case notImplemented(String)

    public var errorDescription: String? {
        switch self {
        case .pathOutsideWorkspace(let path):
            return "Path outside workspace: \(path)"
        case .fileNotFound(let path):
            return "File not found: \(path)"
        case .permissionDenied(let path):
            return "Permission denied: \(path)"
        case .invalidParameters(let msg):
            return "Invalid parameters: \(msg)"
        case .executionFailed(let msg):
            return "Execution failed: \(msg)"
        case .notImplemented(let feature):
            return "Not implemented: \(feature)"
        }
    }
}

// MARK: - Output Bounds Constants

/// Maximum inline bytes (8KB as per spec)
public let MAX_INLINE_BYTES = 8192

/// Maximum inline lines (120 as per spec)
public let MAX_INLINE_LINES = 120

/// Default context lines
public let DEFAULT_CONTEXT_LINES = 2
