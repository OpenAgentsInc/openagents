// orchestrate.swift — ACP types for orchestration operations
// Part of Phase 2: On-device Foundation Models orchestrator

import Foundation

// MARK: - Orchestrate Explore Start

/// Request to start workspace exploration using on-device Foundation Models
public struct OrchestrateExploreStartRequest: Codable, Sendable {
    /// Root path of workspace to explore
    public var root: String

    /// Optional remote URL for Git clone
    public var remote_url: String?

    /// Optional Git branch
    public var branch: String?

    /// Policy governing exploration behavior
    public var policy: ExplorationPolicy?

    /// High-level goals for exploration
    public var goals: [String]?

    public init(
        root: String,
        remote_url: String? = nil,
        branch: String? = nil,
        policy: ExplorationPolicy? = nil,
        goals: [String]? = nil
    ) {
        self.root = root
        self.remote_url = remote_url
        self.branch = branch
        self.policy = policy
        self.goals = goals
    }
}

/// Policy governing exploration behavior
public struct ExplorationPolicy: Codable, Sendable {
    /// Allow external LLM calls (Phase 2: always false, on-device only)
    public var allow_external_llms: Bool

    /// Allow network access
    public var allow_network: Bool

    public init(allow_external_llms: Bool = false, allow_network: Bool = false) {
        self.allow_external_llms = allow_external_llms
        self.allow_network = allow_network
    }
}

/// Response from orchestrate.explore.start
public struct OrchestrateExploreStartResponse: Codable, Sendable {
    /// Session ID for this exploration
    public var session_id: String

    /// Plan ID for tracking
    public var plan_id: String

    /// Initial status message
    public var status: String?

    public init(session_id: String, plan_id: String, status: String? = nil) {
        self.session_id = session_id
        self.plan_id = plan_id
        self.status = status
    }
}

// MARK: - Tool Result Types

/// Result from content.get_span tool
public struct ContentSpanResult: Codable, Sendable {
    /// File path
    public var path: String

    /// Start line (1-indexed)
    public var start_line: Int

    /// End line (1-indexed)
    public var end_line: Int

    /// Actual content lines
    public var lines: [String]

    /// Whether content was truncated due to size limits
    public var truncated: Bool

    /// Original size before truncation (bytes)
    public var original_size: Int?

    /// Encoding used (should be "utf-8")
    public var encoding: String

    public init(
        path: String,
        start_line: Int,
        end_line: Int,
        lines: [String],
        truncated: Bool,
        original_size: Int? = nil,
        encoding: String = "utf-8"
    ) {
        self.path = path
        self.start_line = start_line
        self.end_line = end_line
        self.lines = lines
        self.truncated = truncated
        self.original_size = original_size
        self.encoding = encoding
    }
}

/// Result from grep tool
public struct GrepResult: Codable, Sendable {
    /// Search pattern used
    public var pattern: String

    /// Matches found
    public var matches: [GrepMatch]

    /// Whether results were truncated due to limits
    public var truncated: Bool

    /// Total matches before truncation
    public var total_matches: Int

    public init(
        pattern: String,
        matches: [GrepMatch],
        truncated: Bool,
        total_matches: Int
    ) {
        self.pattern = pattern
        self.matches = matches
        self.truncated = truncated
        self.total_matches = total_matches
    }
}

/// Single grep match
public struct GrepMatch: Codable, Sendable {
    /// File path
    public var path: String

    /// Line number (1-indexed)
    public var line_number: Int

    /// Matching line content
    public var line: String

    /// Optional context lines before
    public var context_before: [String]?

    /// Optional context lines after
    public var context_after: [String]?

    public init(
        path: String,
        line_number: Int,
        line: String,
        context_before: [String]? = nil,
        context_after: [String]? = nil
    ) {
        self.path = path
        self.line_number = line_number
        self.line = line
        self.context_before = context_before
        self.context_after = context_after
    }
}

// MARK: - Explore Summary (final result)

/// Final summary of workspace exploration
public struct ExploreSummary: Codable, Sendable {
    /// Repository/workspace name
    public var repo_name: String

    /// Languages detected with line counts
    public var languages: [String: Int]

    /// Identified entry points
    public var entrypoints: [String]

    /// Top files of interest
    public var top_files: [String]

    /// Follow-up suggestions
    public var followups: [String]

    public init(
        repo_name: String,
        languages: [String: Int],
        entrypoints: [String],
        top_files: [String],
        followups: [String]
    ) {
        self.repo_name = repo_name
        self.languages = languages
        self.entrypoints = entrypoints
        self.top_files = top_files
        self.followups = followups
    }
}

// MARK: - Session History Tools (Phase 2.5)

/// Parameters for session.list tool
public struct SessionListParams: Codable, Sendable, Equatable {
    /// Filter by provider ("claude-code", "codex", or nil for both)
    public var provider: String?

    /// Most recent N sessions (default: 20, max: 200)
    public var topK: Int?

    /// Only sessions after this timestamp (ms since epoch)
    public var since: Int64?

    public init(provider: String? = nil, topK: Int? = nil, since: Int64? = nil) {
        self.provider = provider
        self.topK = topK
        self.since = since
    }
}

/// Session metadata entry
public struct SessionMetadata: Codable, Sendable {
    /// Session ID
    public var id: String

    /// Session title (first user message preview)
    public var title: String?

    /// Provider ("claude-code" or "codex")
    public var provider: String

    /// Last modified timestamp (ms since epoch)
    public var updated_at: Int64

    /// File path to session .jsonl
    public var file_path: String

    public init(id: String, title: String?, provider: String, updated_at: Int64, file_path: String) {
        self.id = id
        self.title = title
        self.provider = provider
        self.updated_at = updated_at
        self.file_path = file_path
    }
}

/// Result from session.list tool
public struct SessionListResult: Codable, Sendable {
    /// Sessions found
    public var sessions: [SessionMetadata]

    /// Whether results were truncated
    public var truncated: Bool

    /// Total sessions available before filtering
    public var total_count: Int

    public init(sessions: [SessionMetadata], truncated: Bool, total_count: Int) {
        self.sessions = sessions
        self.truncated = truncated
        self.total_count = total_count
    }
}

/// Parameters for session.search tool
public struct SessionSearchParams: Codable, Sendable, Equatable {
    /// Regex pattern to search
    public var pattern: String

    /// Filter by provider
    public var provider: String?

    /// Search specific session IDs, or nil for all
    public var sessionIds: [String]?

    /// Max results (default: 100)
    public var maxResults: Int?

    /// Context lines before/after match (default: 2)
    public var contextLines: Int?

    public init(pattern: String, provider: String? = nil, sessionIds: [String]? = nil, maxResults: Int? = nil, contextLines: Int? = nil) {
        self.pattern = pattern
        self.provider = provider
        self.sessionIds = sessionIds
        self.maxResults = maxResults
        self.contextLines = contextLines
    }
}

/// Single session search match
public struct SessionSearchMatch: Codable, Sendable {
    /// Session ID
    public var sessionId: String

    /// Provider
    public var provider: String

    /// Line number in JSONL file
    public var lineNumber: Int

    /// Matching line content
    public var line: String

    /// Context before (if requested)
    public var contextBefore: [String]?

    /// Context after (if requested)
    public var contextAfter: [String]?

    public init(sessionId: String, provider: String, lineNumber: Int, line: String, contextBefore: [String]? = nil, contextAfter: [String]? = nil) {
        self.sessionId = sessionId
        self.provider = provider
        self.lineNumber = lineNumber
        self.line = line
        self.contextBefore = contextBefore
        self.contextAfter = contextAfter
    }
}

/// Result from session.search tool
public struct SessionSearchResult: Codable, Sendable {
    /// Pattern used
    public var pattern: String

    /// Matches found
    public var matches: [SessionSearchMatch]

    /// Whether results were truncated
    public var truncated: Bool

    /// Total matches before truncation
    public var totalMatches: Int

    public init(pattern: String, matches: [SessionSearchMatch], truncated: Bool, totalMatches: Int) {
        self.pattern = pattern
        self.matches = matches
        self.truncated = truncated
        self.totalMatches = totalMatches
    }
}

/// Parameters for session.read tool
public struct SessionReadParams: Codable, Sendable, Equatable {
    /// Session ID to read
    public var sessionId: String

    /// Provider
    public var provider: String

    /// Start line (1-indexed, optional)
    public var startLine: Int?

    /// End line (1-indexed, optional)
    public var endLine: Int?

    /// Max events to return (default: 100)
    public var maxEvents: Int?

    public init(sessionId: String, provider: String, startLine: Int? = nil, endLine: Int? = nil, maxEvents: Int? = nil) {
        self.sessionId = sessionId
        self.provider = provider
        self.startLine = startLine
        self.endLine = endLine
        self.maxEvents = maxEvents
    }
}

/// Simplified event from session
public struct SessionEvent: Codable, Sendable {
    /// Event type (user, assistant, tool_call, etc.)
    public var type: String

    /// Line number in JSONL
    public var lineNumber: Int

    /// Event content (text extract or summary)
    public var content: String

    /// Timestamp if available (ms since epoch)
    public var timestamp: Int64?

    public init(type: String, lineNumber: Int, content: String, timestamp: Int64? = nil) {
        self.type = type
        self.lineNumber = lineNumber
        self.content = content
        self.timestamp = timestamp
    }
}

/// Result from session.read tool
public struct SessionReadResult: Codable, Sendable {
    /// Session ID
    public var sessionId: String

    /// Events extracted
    public var events: [SessionEvent]

    /// Whether truncated
    public var truncated: Bool

    /// Total events in session
    public var totalEvents: Int

    /// File references found (tool calls to Read/Edit/etc)
    public var fileReferences: [String]

    public init(sessionId: String, events: [SessionEvent], truncated: Bool, totalEvents: Int, fileReferences: [String]) {
        self.sessionId = sessionId
        self.events = events
        self.truncated = truncated
        self.totalEvents = totalEvents
        self.fileReferences = fileReferences
    }
}

/// Parameters for session.analyze tool
public struct SessionAnalyzeParams: Codable, Sendable, Equatable {
    /// Session IDs to analyze
    public var sessionIds: [String]

    /// Provider filter
    public var provider: String?

    /// Metrics to compute (["files", "tools", "goals"] or nil for all)
    public var metrics: [String]?

    public init(sessionIds: [String], provider: String? = nil, metrics: [String]? = nil) {
        self.sessionIds = sessionIds
        self.provider = provider
        self.metrics = metrics
    }
}

/// Result from session.analyze tool
public struct SessionAnalyzeResult: Codable, Sendable {
    /// File access frequency (path → count)
    public var fileFrequency: [String: Int]?

    /// Tool usage frequency (tool name → count)
    public var toolFrequency: [String: Int]?

    /// User goal patterns (extracted from first messages)
    public var goalPatterns: [String]?

    /// Average conversation length (events per session)
    public var avgConversationLength: Double?

    public init(fileFrequency: [String: Int]? = nil, toolFrequency: [String: Int]? = nil, goalPatterns: [String]? = nil, avgConversationLength: Double? = nil) {
        self.fileFrequency = fileFrequency
        self.toolFrequency = toolFrequency
        self.goalPatterns = goalPatterns
        self.avgConversationLength = avgConversationLength
    }
}
