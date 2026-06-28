import Foundation

struct LocalDiagnosticsSnapshot: Equatable {
    struct Row: Identifiable, Equatable {
        let id: String
        let label: String
        let value: String
        let systemImage: String
    }

    let generatedAt: Date
    let publicRows: [Row]
    let rawRows: [Row]

    static func make(
        generatedAt: Date = Date(),
        hasAPIKey: Bool,
        channelName: String,
        isStreaming: Bool,
        activeConversation: Conversation?,
        conversationCount: Int,
        isUsingEphemeralFallback: Bool,
        processInfo: ProcessInfo = .processInfo,
        bundle: Bundle = .main
    ) -> LocalDiagnosticsSnapshot {
        let messages = activeConversation?.sortedMessages ?? []
        let lastAssignmentRef = messages
            .lazy
            .compactMap { Self.firstAssignmentRef(in: $0.content) }
            .first

        let publicRows = [
            Row(
                id: "apple-runtime",
                label: "Apple runtime",
                value: "Native SwiftUI lifecycle active",
                systemImage: "apple.logo"
            ),
            Row(
                id: "pylon",
                label: "Pylon",
                value: lastAssignmentRef == nil ? "No local assignment linked" : "Assignment \(lastAssignmentRef!)",
                systemImage: "point.3.connected.trianglepath.dotted"
            ),
            Row(
                id: "assignment",
                label: "Assignment status",
                value: isStreaming ? "Streaming owner request" : "Idle",
                systemImage: isStreaming ? "dot.radiowaves.left.and.right" : "checkmark.circle"
            ),
            Row(
                id: "privacy",
                label: "Default privacy",
                value: "Public-safe summary only",
                systemImage: "lock.shield"
            ),
            Row(
                id: "storage",
                label: "Local history",
                value: isUsingEphemeralFallback ? "Ephemeral for this session" : "\(conversationCount) local chats",
                systemImage: "internaldrive"
            ),
        ]

        let bundleID = bundle.bundleIdentifier ?? "unknown"
        let version = [
            bundle.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String,
            bundle.object(forInfoDictionaryKey: "CFBundleVersion") as? String,
        ]
            .compactMap { $0?.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }
            .joined(separator: " ")

        let rawRows = [
            Row(
                id: "raw-channel",
                label: "Channel",
                value: channelName,
                systemImage: "bubble.left.and.bubble.right"
            ),
            Row(
                id: "raw-bundle",
                label: "Bundle",
                value: [bundleID, version].filter { !$0.isEmpty }.joined(separator: " "),
                systemImage: "app"
            ),
            Row(
                id: "raw-key",
                label: "Keychain",
                value: hasAPIKey ? "API key present: \(redactSecret("oa_agent_local_key_present"))" : "No API key stored",
                systemImage: "key"
            ),
            Row(
                id: "raw-store",
                label: "Store",
                value: redactSensitiveText(localSupportPath(processInfo: processInfo)),
                systemImage: "folder"
            ),
            Row(
                id: "raw-transcript",
                label: "Active transcript",
                value: "\(messages.count) messages; prompts/content hidden in default view",
                systemImage: "text.bubble"
            ),
        ]

        return LocalDiagnosticsSnapshot(
            generatedAt: generatedAt,
            publicRows: publicRows,
            rawRows: rawRows
        )
    }

    static func redactSensitiveText(_ value: String) -> String {
        var redacted = value

        let patterns = [
            #"oa_agent_[A-Za-z0-9._:-]+"#,
            #"(?i)bearer\s+[A-Za-z0-9._~+/=-]+"#,
            #"(?i)(api[_ -]?key|authorization|password|private[_ -]?key|mnemonic|secret)\s*[:=]\s*[^\s,;]+"#,
            #"spark1[A-Za-z0-9]+"#,
            #"/Users/[^/\s]+(?:/[^\s,;:]+)*"#,
            #"~/.codex(?:/[^\s,;:]+)*"#,
        ]

        for pattern in patterns {
            redacted = redacted.replacingOccurrences(
                of: pattern,
                with: replacement(for: pattern),
                options: .regularExpression
            )
        }
        return redacted
    }

    private static func replacement(for pattern: String) -> String {
        if pattern.contains("/Users/") || pattern.contains("~/.codex") {
            return "<local-path>"
        }
        return "<redacted>"
    }

    private static func redactSecret(_ value: String) -> String {
        redactSensitiveText(value)
    }

    private static func localSupportPath(processInfo: ProcessInfo) -> String {
        if let override = processInfo.environment["KHALA_DIAGNOSTICS_STORE_PATH"], !override.isEmpty {
            return override
        }

        return FileManager.default
            .urls(for: .applicationSupportDirectory, in: .userDomainMask)
            .first?
            .appendingPathComponent("Khala", isDirectory: true)
            .path ?? "Application Support/Khala"
    }

    private static func firstAssignmentRef(in text: String) -> String? {
        guard let match = text.range(
            of: #"Assignment:\s*([A-Za-z0-9_.:-]+)"#,
            options: .regularExpression
        ) else {
            return nil
        }

        let matched = String(text[match])
        return matched
            .replacingOccurrences(of: "Assignment:", with: "")
            .trimmingCharacters(in: .whitespacesAndNewlines)
    }
}
