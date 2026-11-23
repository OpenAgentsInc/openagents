// Firm reason: 6 (cross-Apple Swift view models and JSON parsers over Coax-validated EaView & WsView payloads, strictly filtered to Antigravity models).
import Foundation

enum ThinkingEffort: String, CaseIterable, Identifiable, Codable {
    case low
    case medium
    case high
    case xhigh

    var id: String { rawValue }

    var label: String {
        switch self {
        case .low: return "Low"
        case .medium: return "Medium"
        case .high: return "High"
        case .xhigh: return "X-High"
        }
    }
}

enum ReviewMode: String, CaseIterable, Identifiable, Codable {
    case auto
    case ask
    case plan

    var id: String { rawValue }

    /// Wire value expected by `EaRequest.mode` and `CODER_AGY_MODE` (`auto` | `default` | `plan`).
    var wireValue: String {
        switch self {
        case .auto: return "auto"
        case .ask: return "default"
        case .plan: return "plan"
        }
    }

    var label: String {
        switch self {
        case .auto: return "Auto"
        case .ask: return "Ask"
        case .plan: return "Plan"
        }
    }

    var icon: String {
        switch self {
        case .auto: return "bolt.fill"
        case .ask: return "hand.raised.fill"
        case .plan: return "list.bullet.rectangle"
        }
    }

    var description: String {
        switch self {
        case .auto: return "Full access — run tools automatically"
        case .ask: return "Ask before running risky commands"
        case .plan: return "Plan first before editing files"
        }
    }
}

struct AntigravityModelOption: Identifiable, Equatable, Codable {
    let id: String
    let label: String
    let supportedEfforts: [ThinkingEffort]

    static let fallbackCatalog: [AntigravityModelOption] = [
        AntigravityModelOption(id: "gemini-4-neon", label: "Gemini 4 Neon", supportedEfforts: ThinkingEffort.allCases),
        AntigravityModelOption(id: "gemini-4-carbon", label: "Gemini 4 Carbon", supportedEfforts: ThinkingEffort.allCases),
        AntigravityModelOption(id: "gemini-4-hydrogen", label: "Gemini 4 Hydrogen", supportedEfforts: ThinkingEffort.allCases),
        AntigravityModelOption(id: "gemini-4-argon", label: "Gemini 4 Argon", supportedEfforts: ThinkingEffort.allCases),
        AntigravityModelOption(id: "gemini-3.8-flash", label: "Gemini 3.8 Flash", supportedEfforts: [.low, .medium, .high]),
    ]
}

struct SlashCommandItem: Identifiable, Equatable {
    let id: String
    let command: String
    let subtitle: String
    let template: String

    static let defaults: [SlashCommandItem] = [
        SlashCommandItem(id: "plan", command: "/plan", subtitle: "Design an implementation plan before editing", template: "/plan "),
        SlashCommandItem(id: "review", command: "/review", subtitle: "Audit current git diff for bugs and regressions", template: "Review the current git diff and call out any bugs, regressions, or missing tests."),
        SlashCommandItem(id: "compact", command: "/compact", subtitle: "Compact conversation context on host", template: "/compact"),
        SlashCommandItem(id: "test", command: "/test", subtitle: "Run the project test suite and fix failures", template: "Run the relevant tests for our recent changes and fix any failures."),
        SlashCommandItem(id: "explain", command: "/explain", subtitle: "Walk through architecture and key files", template: "Explain how this part of the codebase works: "),
    ]
}

struct FileAttachment: Identifiable, Equatable {
    let id: UUID
    let name: String
    let mimeType: String
    let textPreview: String?
    let byteCount: Int

    init(id: UUID = UUID(), name: String, mimeType: String, textPreview: String?, byteCount: Int) {
        self.id = id
        self.name = name
        self.mimeType = mimeType
        self.textPreview = textPreview
        self.byteCount = byteCount
    }

    var formattedSize: String {
        if byteCount < 1024 { return "\(byteCount) B" }
        return String(format: "%.1f KB", Double(byteCount) / 1024.0)
    }

    /// Formats the attachment cleanly for the Antigravity prompt payload within wire bounds.
    func renderedPromptBlock() -> String {
        if let text = textPreview, !text.isEmpty {
            let clipped = String(text.prefix(4000))
            return "\n\n[Attached file: \(name) (\(formattedSize))]\n```\n\(clipped)\n```"
        }
        return "\n\n[Attached media: \(name) (\(mimeType), \(formattedSize))]"
    }
}

enum ReachabilityState: Equatable {
    case unknown
    case checking
    case reachable
    case refused(String)
    case unreachable(String)

    var label: String {
        switch self {
        case .unknown: return "Not checked"
        case .checking: return "Checking…"
        case .reachable: return "Connected"
        case .refused(let code): return ReachabilityState.humanRefusal(code)
        case .unreachable(let reason): return reason.isEmpty ? "Unreachable on Tailscale" : reason
        }
    }

    static func humanRefusal(_ code: String) -> String {
        switch code {
        case "tagged":
            return "Refused: tagged Tailscale devices cannot pair automatically"
        case "not_owner":
            return "Refused: sign into the same Tailscale user account on both devices"
        case "not_tailnet":
            return "Refused: connection did not arrive over Tailscale"
        case "unavailable":
            return "Refused: OpenAgents host daemon is not serving yet"
        case "malformed":
            return "Refused: protocol version mismatch"
        default:
            return "Refused (\(code))"
        }
    }
}

struct ComputerEntry: Identifiable, Equatable {
    static let localMacID = "local:this-mac"

    let id: String
    var hostKey: String?
    var name: String
    var ip: String
    var dns: String
    var port: UInt16
    var paired: PairedComputer?
    var reachability: ReachabilityState = .unknown
    var isLocalMac: Bool = false

    var displayAddress: String {
        if isLocalMac { return "Local Mac (Unix socket)" }
        if !dns.isEmpty { return "\(dns):\(port)" }
        if !ip.isEmpty { return "\(ip):\(port)" }
        return "Tailscale"
    }

    var canSendRPC: Bool {
        if isLocalMac { return true }
        guard let paired, !paired.isExpired else { return false }
        return !ip.isEmpty || !dns.isEmpty
    }

    var canRequestDirectGrant: Bool {
        !isLocalMac && (!ip.isEmpty || !dns.isEmpty)
    }
}

struct ChatSessionSummary: Identifiable, Equatable, Codable {
    let id: String
    let workspaceID: String
    let title: String
    let managed: Bool
    let control: String
    let generation: Int64
    let revision: Int64
    let busy: Bool
    let interactionID: String
    let updatedAtMs: Int64
    let model: String
    let effort: String
    let desktopSurface: Bool
    let note: String

    var displayTitle: String {
        let trimmed = title.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? "Untitled chat" : trimmed
    }

    var isNativeAntigravity: Bool {
        id.hasPrefix("agy-")
    }

    var isObserveOnly: Bool {
        control == "observe-only" && !isNativeAntigravity
    }

    var canSteerOrSend: Bool {
        (managed && !isObserveOnly) || isNativeAntigravity
    }
}

/// Maps every raw wire/host refusal token into plain, actionable English so users never see naked wire codes.
enum HostErrorFormatter {
    static func describe(_ rawCode: String, method: String = "") -> String {
        let code = rawCode.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !code.isEmpty else { return "" }
        switch code {
        case "unsupported":
            return "Your Mac is running an older OpenAgents host daemon that doesn't support this option yet. Update OpenAgents on your Mac to enable all features."
        case "forbidden":
            if method.hasPrefix("ReadWorkspace") || method.hasSuffix("Branch") || method.hasSuffix("Worktree") || method.hasSuffix("PullRequest") {
                return "Workspace Git controls aren't enabled for this pairing yet. Re-pair over Tailscale or run `coder host workspace-enable` on your Mac."
            }
            return "This device's grant was not accepted by your Mac. Open Computers and tap Pair over Tailscale to refresh it."
        case "expired":
            return "This pairing grant or request expired. Open Computers and tap Pair over Tailscale to renew."
        case "revoked":
            return "This device's access was revoked on your Mac. Pair again from Computers."
        case "conflict":
            return "This chat is read-only or changed on another device. Start a new chat or refresh to continue."
        case "no_agent":
            return "Antigravity (`agy`) isn't available or signed in on your Mac. Open OpenAgents macOS to sign in."
        case "model_unavailable":
            return "The selected Antigravity model isn't enabled on your Mac. Pick another model from the composer."
        case "capacity":
            return "Your Mac has reached its active chat limit. Stop an active turn or wait for a task to finish."
        case "not_found":
            return "That chat or subagent is no longer active on your Mac."
        case "unsupported_profile":
            return "This action isn't supported for this chat type on your Mac."
        case "folder_refused":
            return "That workspace subfolder isn't allowed on your Mac."
        case "workspace_refused":
            return "Workspace access isn't configured on your Mac."
        case "storage_unavailable", "source_unavailable":
            return "Your Mac host couldn't read or write session storage. Check disk space and host daemon logs."
        case "interaction_expired":
            return "That approval prompt has already been answered or expired."
        case "malformed":
            return "The request couldn't be validated by the host. Try sending again."
        case "rate_limited":
            return "Too many requests right now. Wait a moment and try again."
        case "too_large", "bounds", "output_limit":
            return "The message or response exceeded the Tailscale message size limit."
        case "idempotency_conflict":
            return "A duplicate request conflicted with an earlier command. Try sending again."
        case "unavailable":
            return "The host daemon on your Mac is temporarily unavailable."
        default:
            return code
        }
    }
}

struct PendingInteraction: Equatable, Identifiable {
    let id: String
    let sessionID: String
    let prompt: String
}

struct TranscriptItem: Identifiable, Equatable, Codable {
    enum Role: String, Equatable, Codable {
        case user
        case assistant
        case thought
        case tool
        case notice
        case unknown
    }

    let id: String
    let role: Role
    let text: String
    let toolName: String
    let complete: Bool
    let startedAtMs: Int64
    let subagentCalls: Int
    let subagentNow: String
    let path: String
    let diff: String
    let diffCut: Bool
    let noticeKind: String

    var isSubagent: Bool {
        role == .tool && (subagentCalls > 0 || toolName.lowercased() == "task" || toolName.lowercased().contains("subagent"))
    }
}

struct ParsedEnvironmentView {
    let hostPubkey: String
    let displayName: String
    let method: String
    let error: String
    let complete: Bool
    let nextCursor: String
    let workspaceName: String
    let folders: [String]
    let sessions: [ChatSessionSummary]
    let items: [TranscriptItem]
    let antigravityModels: [AntigravityModelOption]
    let defaultEffort: ThinkingEffort

    /// Parses a Coax-validated `EaView` JSON payload and filters its model catalog
    /// exclusively through Coax `MobileCore.isAntigravityModel` (never Claude or Inkling).
    static func parse(_ data: Data) -> ParsedEnvironmentView? {
        guard let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let host = obj["host_pubkey"] as? String,
              let method = obj["method"] as? String else {
            return nil
        }

        let displayName = obj["display_name"] as? String ?? ""
        let error = obj["error"] as? String ?? ""
        let complete = obj["complete"] as? Bool ?? false
        let nextCursor = obj["next_cursor"] as? String ?? ""
        let workspaceName = obj["workspace_name"] as? String ?? ""
        let folders = (obj["folders"] as? [String] ?? []).filter { MobileCore.isValidFolder($0) }

        let rawSessions = obj["sessions"] as? [[String: Any]] ?? []
        let sessions: [ChatSessionSummary] = rawSessions.compactMap { row in
            guard let sid = row["session_id"] as? String, !sid.isEmpty else { return nil }
            return ChatSessionSummary(
                id: sid,
                workspaceID: row["workspace_id"] as? String ?? "default",
                title: row["title"] as? String ?? "",
                managed: (row["origin"] as? String) == "managed",
                control: row["control"] as? String ?? "",
                generation: (row["generation"] as? NSNumber)?.int64Value ?? 0,
                revision: (row["revision"] as? NSNumber)?.int64Value ?? 0,
                busy: row["busy"] as? Bool ?? false,
                interactionID: row["interaction_id"] as? String ?? "",
                updatedAtMs: (row["updated_at_ms"] as? NSNumber)?.int64Value ?? 0,
                model: row["model"] as? String ?? "",
                effort: row["effort"] as? String ?? "",
                desktopSurface: (row["surface"] as? String) == "coder-desktop",
                note: row["note"] as? String ?? ""
            )
        }
        .sorted { $0.updatedAtMs > $1.updatedAtMs }

        let rawItems = obj["items"] as? [[String: Any]] ?? []
        var items: [TranscriptItem] = rawItems.compactMap { row in
            guard let iid = row["item_id"] as? String, !iid.isEmpty else { return nil }
            let roleStr = row["role"] as? String ?? ""
            return TranscriptItem(
                id: iid,
                role: TranscriptItem.Role(rawValue: roleStr) ?? .unknown,
                text: row["text"] as? String ?? "",
                toolName: row["tool_name"] as? String ?? "",
                complete: row["complete"] as? Bool ?? true,
                startedAtMs: (row["started_at_ms"] as? NSNumber)?.int64Value ?? 0,
                subagentCalls: (row["subagent_calls"] as? NSNumber)?.intValue ?? 0,
                subagentNow: row["subagent_now"] as? String ?? "",
                path: row["path"] as? String ?? "",
                diff: row["diff"] as? String ?? "",
                diffCut: row["diff_cut"] as? Bool ?? false,
                noticeKind: row["notice_kind"] as? String ?? ""
            )
        }

        if let rawThoughts = obj["thoughts"] as? [[String: Any]], !rawThoughts.isEmpty {
            var inserted = 0
            for row in rawThoughts {
                guard let iid = row["item_id"] as? String,
                      let at = (row["at"] as? NSNumber)?.intValue else { continue }
                let thought = TranscriptItem(
                    id: iid,
                    role: .thought,
                    text: row["text"] as? String ?? "",
                    toolName: "",
                    complete: row["complete"] as? Bool ?? true,
                    startedAtMs: 0,
                    subagentCalls: 0,
                    subagentNow: "",
                    path: "",
                    diff: "",
                    diffCut: false,
                    noticeKind: ""
                )
                let idx = min(max(0, at + inserted), items.count)
                items.insert(thought, at: idx)
                inserted += 1
            }
        }

        let rawModels = obj["models"] as? [[String: Any]] ?? []
        let filteredModels: [AntigravityModelOption] = rawModels.compactMap { row in
            guard let id = row["model_id"] as? String, MobileCore.isAntigravityModel(id) else {
                return nil
            }
            let normalizedID = MobileCore.normalizeAntigravityModel(id)
            let label = (row["display_name"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? normalizedID
            let rawEfforts = row["efforts"] as? [String] ?? []
            let efforts = rawEfforts.compactMap { ThinkingEffort(rawValue: $0) }
            return AntigravityModelOption(
                id: normalizedID,
                label: label,
                supportedEfforts: efforts.isEmpty ? ThinkingEffort.allCases : efforts
            )
        }

        var seen = Set<String>()
        let uniqueModels = filteredModels.filter { seen.insert($0.id).inserted }
        let defaultEffortStr = obj["default_effort"] as? String ?? "medium"
        let defaultEffort = ThinkingEffort(rawValue: defaultEffortStr) ?? .medium

        return ParsedEnvironmentView(
            hostPubkey: host,
            displayName: displayName,
            method: method,
            error: error,
            complete: complete,
            nextCursor: nextCursor,
            workspaceName: workspaceName,
            folders: folders,
            sessions: sessions,
            items: items,
            antigravityModels: uniqueModels,
            defaultEffort: defaultEffort
        )
    }
}

// MARK: - Workspace Git / Diff / Branch / Worktree / PR models (`coax/lib/agent/workspace/wire.coax`)

struct WorkspaceWorktree: Identifiable, Equatable {
    let id: String
    let path: String
    let branch: String
    let locked: Bool
}

struct WorkspacePR: Identifiable, Equatable {
    let number: Int
    let title: String
    let url: String
    let head: String
    let base: String
    let body: String
    let state: String

    var id: Int { number }
}

struct WorkspaceFileChange: Identifiable, Equatable {
    let id: String
    let code: String
    let path: String
    let diff: String
}

struct WorkspaceSnapshot: Equatable {
    let host: String
    let workspace: String
    let revision: String
    let path: String
    let repository: String
    let branch: String
    let status: String
    let diff: String
    let diffCut: Bool
    let error: String
    let githubError: String
    let worktrees: [WorkspaceWorktree]
    let branches: [String]
    let pullRequests: [WorkspacePR]

    /// Parses `status` lines and splits the unified `diff` into per-file sections for the Files inspector tab.
    var changedFiles: [WorkspaceFileChange] {
        var diffsByPath: [String: String] = [:]
        var currentPath: String?
        var currentLines: [String] = []

        func flush() {
            if let p = currentPath, !currentLines.isEmpty {
                diffsByPath[p] = currentLines.joined(separator: "\n")
            }
            currentLines.removeAll()
        }

        for rawLine in diff.split(separator: "\n", omittingEmptySubsequences: false) {
            let line = String(rawLine)
            if line.hasPrefix("diff --git ") {
                flush()
                let parts = line.split(separator: " ")
                if let last = parts.last {
                    let s = String(last)
                    currentPath = s.hasPrefix("b/") ? String(s.dropFirst(2)) : s
                } else {
                    currentPath = nil
                }
                currentLines = [line]
            } else if currentPath != nil {
                currentLines.append(line)
            }
        }
        flush()

        var out: [WorkspaceFileChange] = []
        var seen = Set<String>()
        for rawLine in status.split(separator: "\n") {
            let line = String(rawLine)
            guard line.count >= 4 else { continue }
            let code = String(line.prefix(2)).trimmingCharacters(in: .whitespaces)
            let path = String(line.dropFirst(3)).trimmingCharacters(in: .whitespaces)
            guard !path.isEmpty, seen.insert(path).inserted else { continue }
            out.append(WorkspaceFileChange(
                id: path,
                code: code.isEmpty ? "M" : code,
                path: path,
                diff: diffsByPath[path] ?? ""
            ))
        }
        for (path, fileDiff) in diffsByPath.sorted(by: { $0.key < $1.key }) where seen.insert(path).inserted {
            out.append(WorkspaceFileChange(id: path, code: "M", path: path, diff: fileDiff))
        }
        return out
    }

    static func parse(_ data: Data) -> WorkspaceSnapshot? {
        guard let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let host = obj["host"] as? String else { return nil }
        let rawTrees = obj["worktrees"] as? [[String: Any]] ?? []
        let trees: [WorkspaceWorktree] = rawTrees.compactMap { row in
            guard let id = row["id"] as? String, let path = row["path"] as? String else { return nil }
            return WorkspaceWorktree(
                id: id,
                path: path,
                branch: row["branch"] as? String ?? "",
                locked: row["locked"] as? Bool ?? false
            )
        }
        let rawPRs = obj["pull_requests"] as? [[String: Any]] ?? []
        let prs: [WorkspacePR] = rawPRs.compactMap { row in
            guard let num = (row["number"] as? NSNumber)?.intValue else { return nil }
            return WorkspacePR(
                number: num,
                title: row["title"] as? String ?? "",
                url: row["url"] as? String ?? "",
                head: row["head"] as? String ?? "",
                base: row["base"] as? String ?? "",
                body: row["body"] as? String ?? "",
                state: row["state"] as? String ?? ""
            )
        }
        return WorkspaceSnapshot(
            host: host,
            workspace: obj["workspace"] as? String ?? "default",
            revision: obj["revision"] as? String ?? "",
            path: obj["path"] as? String ?? "",
            repository: obj["repository"] as? String ?? "",
            branch: obj["branch"] as? String ?? "",
            status: obj["status"] as? String ?? "",
            diff: obj["diff"] as? String ?? "",
            diffCut: obj["diff_cut"] as? Bool ?? false,
            error: obj["error"] as? String ?? "",
            githubError: obj["github_error"] as? String ?? "",
            worktrees: trees,
            branches: obj["branches"] as? [String] ?? [],
            pullRequests: prs
        )
    }
}
