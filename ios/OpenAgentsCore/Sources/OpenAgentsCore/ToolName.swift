// ToolName.swift
// Type-safe tool name enum with support for namespaced suffixes

import Foundation

/// Type-safe representation of tool names used in ACP protocol
///
/// Tools can be:
/// - Core tools: bash, read, write, edit, glob, grep
/// - FM tools: delegate.run, session.list, code.grep, etc.
/// - Namespaced variants: provider.toolname (e.g., codex.bash, claude_code.read)
///
/// The enum provides type safety while remaining extensible via `.custom(String)`.
public enum ToolName: Codable, Equatable, Hashable, Sendable {
    // MARK: - Core Tools
    case bash
    case shell
    case read
    case write
    case edit
    case glob
    case grep

    // MARK: - Foundation Models Tools
    case delegate
    case sessionList
    case sessionSearch
    case sessionRead
    case sessionAnalyze
    case contentGetSpan
    case codeGrep
    case fsListDir

    // MARK: - Extensibility
    /// Custom tool name for provider-specific or unknown tools
    case custom(String)

    /// The raw string value of the tool name
    public var rawValue: String {
        switch self {
        case .bash: return "bash"
        case .shell: return "shell"
        case .read: return "read"
        case .write: return "write"
        case .edit: return "edit"
        case .glob: return "glob"
        case .grep: return "grep"
        case .delegate: return "delegate.run"
        case .sessionList: return "session.list"
        case .sessionSearch: return "session.search"
        case .sessionRead: return "session.read"
        case .sessionAnalyze: return "session.analyze"
        case .contentGetSpan: return "content.get_span"
        case .codeGrep: return "code.grep"
        case .fsListDir: return "fs.list_dir"
        case .custom(let s): return s
        }
    }

    /// Create a ToolName from a string
    public static func fromString(_ str: String) -> ToolName {
        switch str {
        case "bash": return .bash
        case "shell": return .shell
        case "read": return .read
        case "write": return .write
        case "edit": return .edit
        case "glob": return .glob
        case "grep": return .grep
        case "delegate.run": return .delegate
        case "session.list": return .sessionList
        case "session.search": return .sessionSearch
        case "session.read": return .sessionRead
        case "session.analyze": return .sessionAnalyze
        case "content.get_span": return .contentGetSpan
        case "code.grep": return .codeGrep
        case "fs.list_dir": return .fsListDir
        default: return .custom(str)
        }
    }

    // MARK: - Codable

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        let str = try container.decode(String.self)
        self = Self.fromString(str)
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(rawValue)
    }

    // MARK: - Helper Methods

    /// Check if this tool name has a specific suffix (e.g., ".shell", ".read")
    ///
    /// Example:
    /// ```
    /// ToolName.custom("codex.bash").hasSuffix(".bash") // true
    /// ToolName.bash.hasSuffix(".bash") // false (exact match "bash", not "*.bash")
    /// ```
    public func hasSuffix(_ suffix: String) -> Bool {
        return rawValue.hasSuffix(suffix)
    }

    /// Extract the base name from a namespaced tool
    ///
    /// Example:
    /// ```
    /// ToolName.custom("codex.bash").baseName // "bash"
    /// ToolName.bash.baseName // "bash"
    /// ToolName.delegate.baseName // "run"
    /// ```
    public var baseName: String {
        if let dotIndex = rawValue.lastIndex(of: ".") {
            return String(rawValue[rawValue.index(after: dotIndex)...])
        }
        return rawValue
    }

    /// Check if this tool matches a specific core tool name
    ///
    /// Useful for checking if a namespaced tool is a variant of a core tool.
    ///
    /// Example:
    /// ```
    /// ToolName.custom("codex.bash").matches(.bash) // true
    /// ToolName.custom("claude_code.read").matches(.read) // true
    /// ```
    public func matches(_ coreTool: ToolName) -> Bool {
        guard case .custom = self else {
            return self == coreTool
        }
        return baseName == coreTool.rawValue || rawValue == coreTool.rawValue
    }

    /// Check if this is a core tool (not namespaced)
    public var isCore: Bool {
        if case .custom(let str) = self {
            return !str.contains(".")
        }
        return true
    }
}

// MARK: - ExpressibleByStringLiteral

extension ToolName: ExpressibleByStringLiteral {
    public init(stringLiteral value: String) {
        self = Self.fromString(value)
    }
}

// MARK: - CustomStringConvertible

extension ToolName: CustomStringConvertible {
    public var description: String {
        return rawValue
    }
}
