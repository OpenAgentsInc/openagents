// Firm reason: 6 (cross-Apple Swift view models and JSON parsers over Coax-validated EaView & WsView payloads, strictly filtered to Antigravity models).
import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

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

extension String {
    /// Truncates a string cleanly on a UTF-8 scalar boundary so its UTF-8 encoding never exceeds `maxBytes` (#14002).
    func truncatedToUTF8Bytes(_ maxBytes: Int) -> String {
        guard maxBytes > 0 else { return "" }
        if utf8.count <= maxBytes { return self }
        var bytes = 0
        var end = startIndex
        for idx in indices {
            let next = index(after: idx)
            let len = self[idx..<next].utf8.count
            if bytes + len > maxBytes { break }
            bytes += len
            end = next
        }
        return String(self[startIndex..<end])
    }

    /// Returns `nil` when the string contains NUL bytes or excessive replacement characters (binary file heuristic).
    func nilIfContainsBinaryNUL() -> String? {
        guard !utf8.contains(0) else { return nil }
        let bad = unicodeScalars.filter { $0 == "\u{FFFD}" }.count
        if bad > 4 { return nil }
        return self
    }
}

enum ReviewMode: String, CaseIterable, Identifiable, Codable {
    case auto
    case ask
    case acceptEdits
    case plan
    case turbo

    var id: String { rawValue }

    /// Wire value expected by `EaRequest.mode` and `CODER_AGY_MODE`.
    var wireValue: String {
        switch self {
        case .auto: return "auto"
        case .ask: return "default"
        case .acceptEdits: return "accept-edits"
        case .plan: return "plan"
        case .turbo: return "turbo"
        }
    }

    static func fromWire(_ wire: String) -> ReviewMode? {
        switch wire.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() {
        case "auto": return .auto
        case "default", "ask": return .ask
        case "accept-edits", "acceptedits": return .acceptEdits
        case "plan": return .plan
        case "turbo": return .turbo
        default: return nil
        }
    }

    var label: String {
        switch self {
        case .auto: return "Auto"
        case .ask: return "Ask"
        case .acceptEdits: return "Accept Edits"
        case .plan: return "Plan"
        case .turbo: return "Turbo"
        }
    }

    var icon: String {
        switch self {
        case .auto: return "bolt.fill"
        case .ask: return "hand.raised.fill"
        case .acceptEdits: return "pencil.and.outline"
        case .plan: return "list.bullet.rectangle"
        case .turbo: return "flame.fill"
        }
    }

    var description: String {
        switch self {
        case .auto: return "Approve workspace tools automatically; guard external paths"
        case .ask: return "Ask before editing files or running commands"
        case .acceptEdits: return "Auto-approve file edits; ask before running commands"
        case .plan: return "Plan first before editing files"
        case .turbo: return "Unrestricted — skip all permission checks"
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
    let imageData: Data?
    let imageBase64: String?
    let highResBase64: String?
    let binaryBase64: String?
    let byteCount: Int

    init(
        id: UUID = UUID(),
        name: String,
        mimeType: String,
        textPreview: String?,
        byteCount: Int,
        imageData: Data? = nil,
        imageBase64: String? = nil,
        highResBase64: String? = nil,
        binaryBase64: String? = nil
    ) {
        self.id = id
        self.name = name
        self.mimeType = mimeType
        self.textPreview = textPreview
        self.imageData = imageData
        self.imageBase64 = imageBase64
        self.highResBase64 = highResBase64
        self.binaryBase64 = binaryBase64
        self.byteCount = byteCount
    }

    var isImage: Bool {
        imageData != nil || imageBase64 != nil || highResBase64 != nil || mimeType.hasPrefix("image/")
    }

    var hasStructuredWireData: Bool {
        if let hr = highResBase64, !hr.isEmpty { return true }
        if let img = imageBase64, !img.isEmpty { return true }
        if let bin = binaryBase64, !bin.isEmpty { return true }
        return false
    }

    var formattedSize: String {
        if byteCount < 1024 { return "\(byteCount) B" }
        return String(format: "%.1f KB", Double(byteCount) / 1024.0)
    }

    /// Downscales and JPEG-compresses an image cleanly via ImageIO:
    /// - `imageBase64`: compact <= `maxRawBytes` (3.4 KB) JPEG for strict v1 legacy hosts and single-chunk transcripts.
    /// - `highResBase64`: crisp <= `maxHighResBytes` (22 KB) JPEG for v2 `EaRequest.attachments` Vision analysis (#14002).
    static func prepareImage(data: Data, name: String, maxRawBytes: Int = 3_400, maxHighResBytes: Int = 22_000) -> FileAttachment {
        let safeName = name
            .replacingOccurrences(of: " ", with: "_")
            .replacingOccurrences(of: "]", with: "")
            .replacingOccurrences(of: "[", with: "")
        let finalName = safeName.isEmpty ? "photo.jpg" : safeName

        guard let source = CGImageSourceCreateWithData(data as CFData, nil) else {
            let trimmed = data.count <= maxRawBytes ? data : nil
            let highRes = data.count <= maxHighResBytes ? data : trimmed
            return FileAttachment(
                name: finalName,
                mimeType: "image/jpeg",
                textPreview: nil,
                byteCount: data.count,
                imageData: data,
                imageBase64: trimmed?.base64EncodedString(),
                highResBase64: highRes?.base64EncodedString()
            )
        }

        func encodeJPEG(maxPixel: Int, quality: CGFloat) -> Data? {
            let opts: [CFString: Any] = [
                kCGImageSourceCreateThumbnailFromImageAlways: true,
                kCGImageSourceCreateThumbnailWithTransform: true,
                kCGImageSourceThumbnailMaxPixelSize: maxPixel,
            ]
            guard let cg = CGImageSourceCreateThumbnailAtIndex(source, 0, opts as CFDictionary) else { return nil }
            let out = NSMutableData()
            guard let dest = CGImageDestinationCreateWithData(out, UTType.jpeg.identifier as CFString, 1, nil) else { return nil }
            let destOpts: [CFString: Any] = [kCGImageDestinationLossyCompressionQuality: quality]
            CGImageDestinationAddImage(dest, cg, destOpts as CFDictionary)
            guard CGImageDestinationFinalize(dest) else { return nil }
            return out as Data
        }

        var chosen: Data?
        let ladder: [(Int, CGFloat)] = [
            (512, 0.70), (512, 0.52), (448, 0.42), (384, 0.35), (320, 0.28), (256, 0.22),
        ]
        for (px, q) in ladder {
            if let jpeg = encodeJPEG(maxPixel: px, quality: q) {
                chosen = jpeg
                if jpeg.count <= maxRawBytes { break }
            }
        }
        var chosenHighRes: Data?
        let highResLadder: [(Int, CGFloat)] = [
            (1280, 0.78), (1024, 0.72), (896, 0.65), (768, 0.58), (640, 0.50), (512, 0.42),
        ]
        for (px, q) in highResLadder {
            if let jpeg = encodeJPEG(maxPixel: px, quality: q) {
                chosenHighRes = jpeg
                if jpeg.count <= maxHighResBytes { break }
            }
        }
        let wireData = chosen ?? (data.count <= maxRawBytes ? data : nil)
        let highResData = (chosenHighRes.flatMap { $0.count <= maxHighResBytes ? $0 : nil }) ?? wireData
        let previewThumb = encodeJPEG(maxPixel: 480, quality: 0.78) ?? wireData ?? data
        return FileAttachment(
            name: finalName,
            mimeType: "image/jpeg",
            textPreview: nil,
            byteCount: wireData?.count ?? data.count,
            imageData: previewThumb,
            imageBase64: wireData?.base64EncodedString(),
            highResBase64: highResData?.base64EncodedString()
        )
    }

    /// Builds a bounded `EaRequest.attachments` JSON array for v2 hosts (`[{"name":...,"mime_type":...,"data":...}]`).
    /// Returns `""` when no binary/image attachments need structured carriage so v1 compat is preserved.
    static func encodeWireAttachmentsJSON(_ attachments: [FileAttachment], maxTotalBytes: Int = 31_000) -> String {
        var entries: [[String: String]] = []
        var usedBytes = 2 // "[]"
        for att in attachments {
            guard let b64 = att.highResBase64 ?? att.imageBase64 ?? att.binaryBase64, !b64.isEmpty else { continue }
            let est = b64.utf8.count + att.name.utf8.count + att.mimeType.utf8.count + 48
            if usedBytes + est <= maxTotalBytes {
                entries.append(["name": att.name, "mime_type": att.mimeType, "data": b64])
                usedBytes += est
            } else if let compact = att.imageBase64, !compact.isEmpty {
                let compactEst = compact.utf8.count + att.name.utf8.count + att.mimeType.utf8.count + 48
                if usedBytes + compactEst <= maxTotalBytes {
                    entries.append(["name": att.name, "mime_type": att.mimeType, "data": compact])
                    usedBytes += compactEst
                }
            }
        }
        guard !entries.isEmpty,
              let data = try? JSONSerialization.data(withJSONObject: entries, options: []),
              data.count <= maxTotalBytes,
              let json = String(data: data, encoding: .utf8) else {
            return ""
        }
        return json
    }

    /// Formats the attachment for v2 prompts when structured `EaRequest.attachments` carries the media bytes.
    func renderedV2Placeholder() -> String {
        if hasStructuredWireData && isImage {
            let safeName = name.replacingOccurrences(of: " ", with: "_").replacingOccurrences(of: "]", with: "")
            return "\n\n[oa-image name=\(safeName) mime=\(mimeType)]"
        }
        if let text = textPreview, !text.isEmpty {
            let clipped = text.truncatedToUTF8Bytes(4_000)
            return "\n\n[Attached file: \(name) (\(formattedSize))]\n```\n\(clipped)\n```"
        }
        return "\n\n[Attached file: \(name) (\(mimeType), \(formattedSize))]"
    }

    /// Formats the attachment cleanly for legacy v1 prompt payloads within wire bounds.
    /// Image attachments emit real `[oa-image name=... mime=... base64=...]` blocks decoded into ACP
    /// `{"type":"image","mimeType":...,"data":...}` content blocks by `agyworker`.
    func renderedPromptBlock() -> String {
        if let b64 = imageBase64, !b64.isEmpty {
            let safeName = name.replacingOccurrences(of: " ", with: "_").replacingOccurrences(of: "]", with: "")
            return "\n\n[oa-image name=\(safeName) mime=\(mimeType) base64=\(b64)]"
        }
        if let text = textPreview, !text.isEmpty {
            let clipped = text.truncatedToUTF8Bytes(4_000)
            return "\n\n[Attached file: \(name) (\(formattedSize))]\n```\n\(clipped)\n```"
        }
        return "\n\n[Attached file: \(name) (\(mimeType), \(formattedSize))]"
    }
}

/// Parses `[oa-image name=... mime=... base64=...]` markers inside `.user` transcript bubbles
/// so the UI renders actual image thumbnails instead of raw base64 text.
struct ParsedUserMessage {
    struct InlineImage: Identifiable, Equatable {
        let id: String
        let name: String
        let mime: String
        let data: Data?
    }

    let text: String
    let images: [InlineImage]

    static func parse(_ raw: String) -> ParsedUserMessage {
        guard raw.contains("[oa-image ") else {
            return ParsedUserMessage(text: raw, images: [])
        }
        var textParts: [String] = []
        var images: [InlineImage] = []
        var idx = raw.startIndex
        var seq = 0

        while idx < raw.endIndex {
            guard let start = raw[idx...].range(of: "[oa-image ") else {
                textParts.append(String(raw[idx...]))
                break
            }
            if start.lowerBound > idx {
                textParts.append(String(raw[idx..<start.lowerBound]))
            }
            guard let close = raw[start.upperBound...].firstIndex(of: "]") else {
                // Truncated/split tag across chunk boundary: show clean badge instead of raw base64.
                images.append(InlineImage(id: "img-\(seq)", name: "photo.jpg", mime: "image/jpeg", data: nil))
                break
            }
            let inner = String(raw[start.upperBound..<close])
            var name = "photo.jpg"
            var mime = "image/jpeg"
            var b64 = ""
            for token in inner.split(separator: " ") {
                if token.hasPrefix("name=") { name = String(token.dropFirst(5)) }
                else if token.hasPrefix("mime=") { mime = String(token.dropFirst(5)) }
                else if token.hasPrefix("base64=") { b64 = String(token.dropFirst(7)) }
            }
            let decoded = b64.isEmpty ? nil : Data(base64Encoded: b64)
            images.append(InlineImage(id: "img-\(seq)-\(name)", name: name, mime: mime, data: decoded))
            seq += 1
            idx = raw.index(after: close)
        }

        let cleaned = textParts
            .joined()
            .trimmingCharacters(in: .whitespacesAndNewlines)
        return ParsedUserMessage(text: cleaned, images: images)
    }
}

/// Extracts Desktop-style title headings from Gemini thought summaries (e.g. `**Confirming System Functionality**`).
enum ThoughtSummaryParser {
    static func extractHeading(_ raw: String) -> String {
        for rawLine in raw.split(separator: "\n", omittingEmptySubsequences: true) {
            var line = String(rawLine).trimmingCharacters(in: .whitespacesAndNewlines)
            guard !line.isEmpty else { continue }
            while line.hasPrefix("#") || line.hasPrefix("*") || line.hasPrefix("-") || line.hasPrefix(" ") {
                line.removeFirst()
            }
            while line.hasSuffix("*") || line.hasSuffix(":") || line.hasSuffix(" ") {
                line.removeLast()
            }
            if !line.isEmpty {
                return String(line.prefix(64))
            }
        }
        return ""
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
    let mode: String
    let desktopSurface: Bool
    let note: String

    init(
        id: String,
        workspaceID: String,
        title: String,
        managed: Bool,
        control: String,
        generation: Int64,
        revision: Int64,
        busy: Bool,
        interactionID: String,
        updatedAtMs: Int64,
        model: String,
        effort: String,
        mode: String = "",
        desktopSurface: Bool,
        note: String
    ) {
        self.id = id
        self.workspaceID = workspaceID
        self.title = title
        self.managed = managed
        self.control = control
        self.generation = generation
        self.revision = revision
        self.busy = busy
        self.interactionID = interactionID
        self.updatedAtMs = updatedAtMs
        self.model = model
        self.effort = effort
        self.mode = mode
        self.desktopSurface = desktopSurface
        self.note = note
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = try c.decode(String.self, forKey: .id)
        workspaceID = try c.decode(String.self, forKey: .workspaceID)
        title = try c.decode(String.self, forKey: .title)
        managed = try c.decode(Bool.self, forKey: .managed)
        control = try c.decode(String.self, forKey: .control)
        generation = try c.decode(Int64.self, forKey: .generation)
        revision = try c.decode(Int64.self, forKey: .revision)
        busy = try c.decode(Bool.self, forKey: .busy)
        interactionID = try c.decode(String.self, forKey: .interactionID)
        updatedAtMs = try c.decode(Int64.self, forKey: .updatedAtMs)
        model = try c.decode(String.self, forKey: .model)
        effort = try c.decode(String.self, forKey: .effort)
        mode = try c.decodeIfPresent(String.self, forKey: .mode) ?? ""
        desktopSurface = try c.decode(Bool.self, forKey: .desktopSurface)
        note = try c.decode(String.self, forKey: .note)
    }

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

struct InteractionOption: Identifiable, Equatable {
    let id: String
    let label: String
    let kind: String

    var isAllow: Bool {
        let lower = kind.lowercased()
        if lower.contains("reject") || lower.contains("deny") || lower.contains("cancel") { return false }
        if lower.contains("allow") || lower.contains("accept") { return true }
        let lid = id.lowercased()
        return !(lid.contains("reject") || lid.contains("deny"))
    }
}

struct PendingInteraction: Equatable, Identifiable {
    let id: String
    let sessionID: String
    let prompt: String
    let warning: String
    let isQuestion: Bool
    let options: [InteractionOption]

    init(
        id: String,
        sessionID: String,
        prompt: String,
        warning: String = "",
        isQuestion: Bool = false,
        options: [InteractionOption] = []
    ) {
        self.id = id
        self.sessionID = sessionID
        self.prompt = prompt
        self.warning = warning
        self.isQuestion = isQuestion
        self.options = options
    }
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
    let interactionPrompt: String
    let interactionWarning: String
    let interactionQuestion: Bool
    let interactionOptions: [InteractionOption]

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
                mode: row["mode"] as? String ?? "",
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

        let interactionPrompt = obj["interaction_prompt"] as? String ?? ""
        let interactionWarning = obj["interaction_warning"] as? String ?? ""
        let interactionQuestion = obj["interaction_question"] as? Bool ?? false
        let rawOptions = obj["interaction_options"] as? [[String: Any]] ?? []
        let interactionOptions: [InteractionOption] = rawOptions.compactMap { row in
            let oid = (row["optionId"] as? String)
                ?? (row["option_id"] as? String)
                ?? (row["id"] as? String)
                ?? ""
            guard !oid.isEmpty else { return nil }
            let label = (row["name"] as? String)
                ?? (row["label"] as? String)
                ?? oid
            let kind = row["kind"] as? String ?? ""
            return InteractionOption(id: oid, label: label, kind: kind)
        }

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
            defaultEffort: defaultEffort,
            interactionPrompt: interactionPrompt,
            interactionWarning: interactionWarning,
            interactionQuestion: interactionQuestion,
            interactionOptions: interactionOptions
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

    /// Unquotes C-escaped git status/diff paths (e.g. `"dir/file with space.swift"` -> `dir/file with space.swift`).
    private static func unquoteGitPath(_ raw: String) -> String {
        let trimmed = raw.trimmingCharacters(in: .whitespaces)
        guard trimmed.count >= 2, trimmed.first == "\"", trimmed.last == "\"" else { return trimmed }
        let inner = trimmed.dropFirst().dropLast()
        var out = ""
        out.reserveCapacity(inner.count)
        var i = inner.startIndex
        while i < inner.endIndex {
            let ch = inner[i]
            if ch == "\\" {
                let next = inner.index(after: i)
                if next < inner.endIndex {
                    let esc = inner[next]
                    switch esc {
                    case "\"": out.append("\"")
                    case "\\": out.append("\\")
                    case "t": out.append("\t")
                    case "n": out.append("\n")
                    default: out.append(esc)
                    }
                    i = inner.index(after: next)
                    continue
                }
            }
            out.append(ch)
            i = inner.index(after: i)
        }
        return out
    }

    /// Parses `diff --git a/... b/...` safely even when filenames contain spaces or quotes (#13999).
    private static func parseDiffGitPath(_ line: String) -> String? {
        guard line.hasPrefix("diff --git ") else { return nil }
        let rest = String(line.dropFirst("diff --git ".count))
        if let r = rest.range(of: " \"b/") {
            let bPart = "\"" + String(rest[r.upperBound...])
            return unquoteGitPath(bPart)
        }
        if let r = rest.range(of: " b/") {
            return unquoteGitPath(String(rest[r.upperBound...]))
        }
        if let last = rest.split(separator: " ").last {
            let s = unquoteGitPath(String(last))
            return s.hasPrefix("b/") ? String(s.dropFirst(2)) : s
        }
        return nil
    }

    /// Parses `status` lines and splits the unified `diff` into per-file sections for the Files inspector tab.
    /// Handles filenames with spaces, git-quoted paths, and renames (`old -> new`) (#13999).
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
                currentPath = Self.parseDiffGitPath(line)
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
            var rawPath = String(line.dropFirst(3)).trimmingCharacters(in: .whitespaces)
            if (code.contains("R") || code.contains("C")), let arrow = rawPath.range(of: " -> ") {
                rawPath = String(rawPath[arrow.upperBound...])
            }
            let path = Self.unquoteGitPath(rawPath)
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

    /// Merges paginated `ReadWorkspace` pages (`page=0` diff/status + `page=1` trees + `page=2` branches + `page=3` PRs) (#13999).
    func merging(
        worktrees newTrees: [WorkspaceWorktree]? = nil,
        branches newBranches: [String]? = nil,
        pullRequests newPRs: [WorkspacePR]? = nil,
        githubError newGithubError: String? = nil
    ) -> WorkspaceSnapshot {
        WorkspaceSnapshot(
            host: host,
            workspace: workspace,
            revision: revision,
            path: path,
            repository: repository,
            branch: branch,
            status: status,
            diff: diff,
            diffCut: diffCut,
            error: error,
            githubError: (newGithubError.flatMap { $0.isEmpty ? nil : $0 }) ?? githubError,
            worktrees: (newTrees.flatMap { $0.isEmpty ? nil : $0 }) ?? worktrees,
            branches: (newBranches.flatMap { $0.isEmpty ? nil : $0 }) ?? branches,
            pullRequests: (newPRs.flatMap { $0.isEmpty ? nil : $0 }) ?? pullRequests
        )
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
