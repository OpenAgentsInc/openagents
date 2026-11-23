// Firm reason: 6 (SwiftUI view models and JSON decoding over Coax-validated EaView payloads, strictly filtered to Antigravity models via MobileCore).
import Foundation

enum ThinkingEffort: String, CaseIterable, Identifiable {
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

enum ReviewMode: String, CaseIterable, Identifiable {
    case auto
    case ask
    case plan

    var id: String { rawValue }

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

struct AntigravityModelOption: Identifiable, Equatable {
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
        case .reachable: return "Connected over Tailscale"
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
    let id: String // host pubkey (64 hex) or manual endpoint id
    var hostKey: String? // 64 lowercase hex when known
    var name: String
    var ip: String
    var dns: String
    var port: UInt16
    var paired: PairedComputer?
    var reachability: ReachabilityState = .unknown

    var displayAddress: String {
        if !dns.isEmpty { return "\(dns):\(port)" }
        if !ip.isEmpty { return "\(ip):\(port)" }
        return "Tailscale"
    }

    var canSendRPC: Bool {
        guard let paired, !paired.isExpired else { return false }
        return !ip.isEmpty || !dns.isEmpty
    }
}

struct ChatSessionSummary: Identifiable, Equatable {
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

    var canSteerOrSend: Bool {
        managed || desktopSurface
    }
}

struct TranscriptItem: Identifiable, Equatable {
    enum Role: String, Equatable {
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

        // Re-interleave optional `thoughts` at their wire `at` indices (matching wire.coax ea_thoughts_read).
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

        // Strictly filter the host's model offer through Coax #authority `mc_is_agy_model`.
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

        // Deduplicate by normalized id while preserving order.
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
