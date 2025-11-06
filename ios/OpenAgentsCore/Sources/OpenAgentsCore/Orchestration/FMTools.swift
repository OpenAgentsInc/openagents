// FMTools.swift — Foundation Models Tool wrappers for OpenAgents operations
// Exposes our existing operations as model-callable tools so FM can drive traversal.

import Foundation

#if canImport(FoundationModels)
import FoundationModels

@available(iOS 26.0, macOS 26.0, *)
public enum FMToolsRegistry {
    public static func defaultTools(workspaceRoot: String) -> [any Tool] {
        var tools: [any Tool] = []
        tools.append(FMTool_SessionList())
        tools.append(FMTool_SessionSearch())
        tools.append(FMTool_SessionRead())
        tools.append(FMTool_SessionAnalyze())
        tools.append(FMTool_ReadSpan(workspaceRoot: workspaceRoot))
        tools.append(FMTool_Grep(workspaceRoot: workspaceRoot))
        tools.append(FMTool_ListDir(workspaceRoot: workspaceRoot))
        return tools
    }
}

// MARK: - Session Tools

@available(iOS 26.0, macOS 26.0, *)
struct FMTool_SessionList: Tool {
    let name = "session.list"
    let description = "List recent conversation sessions from Claude Code and/or Codex (bounded)."
    typealias Output = String

    @Generable
    struct Arguments {
        @Guide(description: "Provider filter: claude-code, codex, or omit for both") var provider: String?
        @Guide(description: "Most recent N sessions") var topK: Int?
        @Guide(description: "Only sessions updated after this unix ms timestamp") var since: Int?
    }

    func call(arguments a: Arguments) async throws -> Output {
        let tool = SessionListTool()
        let since64 = a.since.map { Int64($0) }
        let res = try await tool.list(provider: a.provider, topK: a.topK, since: since64)
        let sample = res.sessions.prefix(5).map { $0.id }.joined(separator: ", ")
        return "session.list total=\(res.total_count) truncated=\(res.truncated) sample=[\(sample)]"
    }
}

@available(iOS 26.0, macOS 26.0, *)
struct FMTool_SessionSearch: Tool {
    let name = "session.search"
    let description = "Search session history for a regex pattern with limited context (bounded)."
    typealias Output = String

    @Generable
    struct Arguments { @Guide(description: "Regex pattern (case-insensitive)") var pattern: String; @Guide(description: "Provider filter") var provider: String?; @Guide(description: "Session IDs (optional)") var sessionIds: [String]?; @Guide(description: "Max results") var maxResults: Int?; @Guide(description: "Context lines") var contextLines: Int? }

    func call(arguments a: Arguments) async throws -> Output {
        let tool = SessionSearchTool()
        let res = try await tool.search(pattern: a.pattern, provider: a.provider, sessionIds: a.sessionIds, maxResults: a.maxResults, contextLines: a.contextLines)
        let sample = res.matches.prefix(3).map { "\($0.sessionId)#\($0.lineNumber)" }.joined(separator: ", ")
        return "session.search matches=\(res.totalMatches) truncated=\(res.truncated) sample=[\(sample)]"
    }
}

@available(iOS 26.0, macOS 26.0, *)
struct FMTool_SessionRead: Tool {
    let name = "session.read"
    let description = "Read a bounded slice of a session and extract event summaries + file references."
    typealias Output = String

    @Generable
    struct Arguments { @Guide(description: "Session ID") var sessionId: String; @Guide(description: "Provider") var provider: String; @Guide(description: "Start line") var startLine: Int?; @Guide(description: "End line") var endLine: Int?; @Guide(description: "Max events") var maxEvents: Int? }

    func call(arguments a: Arguments) async throws -> Output {
        let tool = SessionReadTool()
        let res = try await tool.read(sessionId: a.sessionId, provider: a.provider, startLine: a.startLine, endLine: a.endLine, maxEvents: a.maxEvents)
        let files = res.fileReferences.prefix(5).joined(separator: ", ")
        return "session.read id=\(res.sessionId) events=\(res.events.count) files=[\(files)] truncated=\(res.truncated)"
    }
}

@available(iOS 26.0, macOS 26.0, *)
struct FMTool_SessionAnalyze: Tool {
    let name = "session.analyze"
    let description = "Aggregate insights across sessions: file frequency, tool usage, goals, avg length (bounded)."
    typealias Output = String

    @Generable
    struct Arguments { @Guide(description: "Session IDs") var sessionIds: [String]?; @Guide(description: "Provider") var provider: String?; @Guide(description: "Metrics") var metrics: [String]? }

    func call(arguments a: Arguments) async throws -> Output {
        let tool = SessionAnalyzeTool()
        let res = try await tool.analyze(sessionIds: a.sessionIds ?? [], provider: a.provider, metrics: a.metrics)
        let topFiles = (res.fileFrequency ?? [:]).sorted { $0.value > $1.value }.prefix(3).map { $0.key }.joined(separator: ", ")
        return "session.analyze avgLen=\(Int(res.avgConversationLength ?? 0)) files=\(res.fileFrequency?.count ?? 0) tools=\(res.toolFrequency?.count ?? 0) top=[\(topFiles)]"
    }
}

// MARK: - Filesystem / Code Tools (wrappers)

@available(iOS 26.0, macOS 26.0, *)
struct FMTool_ReadSpan: Tool {
    let name = "content.get_span"
    let description = "Read a small span from a file in the workspace (bounded)."
    private let workspaceRoot: String
    init(workspaceRoot: String) { self.workspaceRoot = workspaceRoot }
    typealias Output = String

    @Generable
    struct Arguments { @Guide(description: "Path (relative or absolute)") var path: String; @Guide(description: "Start line") var startLine: Int; @Guide(description: "End line") var endLine: Int }

    func call(arguments a: Arguments) async throws -> Output {
        let tool = ContentSpanTool(workspaceRoot: workspaceRoot)
        let res = try await tool.readSpan(path: a.path, startLine: a.startLine, endLine: a.endLine)
        let text = res.lines.joined(separator: "\n")
        let preview = text.count > 2000 ? String(text.prefix(2000)) + "…" : text
        return "content.get_span \(res.path):\(res.start_line)-\(res.end_line) (\(text.count) chars)\n\n\(preview)"
    }
}

@available(iOS 26.0, macOS 26.0, *)
struct FMTool_Grep: Tool {
    let name = "code.grep"
    let description = "Search files in the workspace for a pattern (bounded)."
    private let workspaceRoot: String
    init(workspaceRoot: String) { self.workspaceRoot = workspaceRoot }
    typealias Output = String

    @Generable
    struct Arguments { @Guide(description: "Regex pattern") var pattern: String; @Guide(description: "Optional path prefix relative to root") var pathPrefix: String? }

    func call(arguments a: Arguments) async throws -> Output {
        let tool = GrepTool(workspaceRoot: workspaceRoot)
        let res = try await tool.grep(pattern: a.pattern, pathPrefix: a.pathPrefix)
        let files = Set(res.matches.map { $0.path })
        return "code.grep matches=\(res.matches.count) files=\(files.count) pattern=\(res.pattern)"
    }
}

@available(iOS 26.0, macOS 26.0, *)
struct FMTool_ListDir: Tool {
    let name = "fs.list_dir"
    let description = "List directory contents (non-recursive)."
    private let workspaceRoot: String
    init(workspaceRoot: String) { self.workspaceRoot = workspaceRoot }
    typealias Output = String

    @Generable
    struct Arguments { @Guide(description: "Directory path (relative or absolute)") var path: String }

    func call(arguments a: Arguments) async throws -> String {
        // Normalize path to workspace-relative to handle aliases like "/workspace" or "/<name>"
        let rel = PathUtils.normalizeToWorkspaceRelative(workspaceRoot: workspaceRoot, inputPath: a.path)
        let fullPath = (rel == ".") ? workspaceRoot : (workspaceRoot as NSString).appendingPathComponent(rel)
        let stdPath = (fullPath as NSString).standardizingPath

        // Ensure within workspace and is a directory
        let stdRoot = (workspaceRoot as NSString).standardizingPath
        guard stdPath.hasPrefix(stdRoot) else {
            throw ToolExecutionError.pathOutsideWorkspace(a.path)
        }
        var isDir: ObjCBool = false
        guard FileManager.default.fileExists(atPath: stdPath, isDirectory: &isDir), isDir.boolValue else {
            throw ToolExecutionError.invalidParameters("Not a directory: \(a.path)")
        }
        let items = try FileManager.default.contentsOfDirectory(atPath: stdPath)
        return items.sorted().joined(separator: "\n")
    }
}

#endif
