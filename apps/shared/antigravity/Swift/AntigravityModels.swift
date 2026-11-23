// Firm reason: 6 (cross-Apple Swift view models and JSON parsers over Coax-validated EaView & WsView payloads, strictly filtered to Antigravity models).
import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

enum ThinkingEffort: String, CaseIterable, Identifiable, Codable {
    case none
    case minimal
    case low
    case medium
    case high
    case xhigh
    case max

    var id: String { rawValue }

    var label: String {
        switch self {
        case .none: return "Off"
        case .minimal: return "Minimal"
        case .low: return "Low"
        case .medium: return "Medium"
        case .high: return "High"
        case .xhigh: return "X-High"
        case .max: return "Max"
        }
    }

    static let standardLevels: [ThinkingEffort] = [.low, .medium, .high, .xhigh]
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

    var supportsThinking: Bool { !supportedEfforts.isEmpty }
    var isLocalInkling: Bool { id.lowercased().hasPrefix("inkling") }

    var family: String {
        let lower = id.lowercased()
        if lower.hasPrefix("inkling") { return "Local · Psionic Metal" }
        if lower.contains("orchestrator") { return "Orchestrator" }
        if lower.hasPrefix("gemini-4") { return "Gemini 4" }
        if lower.hasPrefix("gemini-3") { return "Gemini 3.8" }
        if lower.hasPrefix("gemini-") { return "Gemini" }
        if lower.hasPrefix("claude-") { return "Claude (ACP)" }
        if lower.hasPrefix("gpt-") { return "GPT-OSS" }
        return "Custom / Gateway"
    }

    static let fallbackCatalog: [AntigravityModelOption] = [
        AntigravityModelOption(id: "gemini-4-neon", label: "Gemini 4 Neon", supportedEfforts: ThinkingEffort.standardLevels),
        AntigravityModelOption(id: "gemini-4-carbon", label: "Gemini 4 Carbon", supportedEfforts: ThinkingEffort.standardLevels),
        AntigravityModelOption(id: "gemini-4-hydrogen", label: "Gemini 4 Hydrogen", supportedEfforts: ThinkingEffort.standardLevels),
        AntigravityModelOption(id: "gemini-4-argon", label: "Gemini 4 Argon", supportedEfforts: ThinkingEffort.standardLevels),
        AntigravityModelOption(id: "gemini-3.8-flash", label: "Gemini 3.8 Flash", supportedEfforts: [.low, .medium, .high]),
    ]
}

struct HostRuntimeMeta: Equatable, Codable {
    let hostVersion: String
    let agyVersion: String
    let agyUpdateAvailable: Bool
    let agyAuthMethod: String
    let agySignedIn: Bool?
    let capabilities: Set<String>

    var supportsV2Wire: Bool { capabilities.contains("wire.v2") }
    var supportsRemoteUpdate: Bool { capabilities.contains("agy.update") }
    var supportsDynamicModels: Bool { capabilities.contains("agy.dynamic_models") }
    var supportsAttachments: Bool { capabilities.contains("agy.attachments") }
    var supportsModes: Bool { capabilities.contains("agy.modes") }
}

struct SessionTodoItem: Identifiable, Equatable, Codable {
    let id: String
    let content: String
    let status: String

    var isCompleted: Bool {
        let lower = status.lowercased()
        return lower == "completed" || lower == "done"
    }

    var isInProgress: Bool {
        let lower = status.lowercased()
        return lower == "in_progress" || lower == "running" || lower == "active"
    }
}

struct SessionUsageStats: Equatable, Codable {
    let inputTokens: Int
    let outputTokens: Int
    let cachedTokens: Int
    let thoughtTokens: Int

    var totalTokens: Int { inputTokens + outputTokens + cachedTokens + thoughtTokens }
    /// Estimated occupancy against standard 1M-token Gemini context window.
    var contextPercent: Double {
        let ctx = inputTokens + cachedTokens
        guard ctx > 0 else { return 0 }
        return min(100.0, (Double(ctx) / 1_000_000.0) * 100.0)
    }
    var summaryLabel: String {
        func fmt(_ n: Int) -> String {
            if n >= 1_000_000 { return String(format: "%.1fM", Double(n) / 1_000_000.0) }
            if n >= 1_000 { return String(format: "%.1fk", Double(n) / 1_000.0) }
            return "\(n)"
        }
        var parts = ["in \(fmt(inputTokens))", "out \(fmt(outputTokens))"]
        if cachedTokens > 0 { parts.append("cache \(fmt(cachedTokens))") }
        if thoughtTokens > 0 { parts.append("think \(fmt(thoughtTokens))") }
        return parts.joined(separator: " · ")
    }
}

struct SlashCommandItem: Identifiable, Equatable {
    let id: String
    let command: String
    let subtitle: String
    let template: String

    static let defaults: [SlashCommandItem] = [
        SlashCommandItem(id: "plan", command: "/plan", subtitle: "Design an implementation plan before editing", template: "/plan "),
        SlashCommandItem(id: "btw", command: "/btw", subtitle: "Ask a read-only side question without altering history", template: "/btw "),
        SlashCommandItem(id: "compact", command: "/compact", subtitle: "Compact conversation context on host", template: "/compact"),
        SlashCommandItem(id: "fork", command: "/fork", subtitle: "Fork conversation into a new session", template: "/fork"),
        SlashCommandItem(id: "rewind", command: "/rewind", subtitle: "Rewind conversation to an earlier checkpoint", template: "/rewind"),
        SlashCommandItem(id: "review", command: "/review", subtitle: "Audit current git diff for bugs and regressions", template: "Review the current git diff and call out any bugs, regressions, or missing tests."),
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

    private struct WireAttachmentDTO: Encodable {
        let name: String
        let mime_type: String
        let data: String
    }

    /// Builds a bounded `EaRequest.attachments` JSON array for v2 hosts (`[{"name":...,"mime_type":...,"data":...}]`).
    /// Returns `""` when no binary/image attachments need structured carriage so v1 compat is preserved.
    static func encodeWireAttachmentsJSON(_ attachments: [FileAttachment], maxTotalBytes: Int = 31_000) -> String {
        var entries: [WireAttachmentDTO] = []
        var usedBytes = 2 // "[]"
        for att in attachments {
            guard let b64 = att.highResBase64 ?? att.imageBase64 ?? att.binaryBase64, !b64.isEmpty else { continue }
            let est = b64.utf8.count + att.name.utf8.count + att.mimeType.utf8.count + 48
            if usedBytes + est <= maxTotalBytes {
                entries.append(WireAttachmentDTO(name: att.name, mime_type: att.mimeType, data: b64))
                usedBytes += est
            } else if let compact = att.imageBase64, !compact.isEmpty {
                let compactEst = compact.utf8.count + att.name.utf8.count + att.mimeType.utf8.count + 48
                if usedBytes + compactEst <= maxTotalBytes {
                    entries.append(WireAttachmentDTO(name: att.name, mime_type: att.mimeType, data: compact))
                    usedBytes += compactEst
                }
            }
        }
        guard !entries.isEmpty,
              let data = try? JSONEncoder().encode(entries),
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
        case "not_serving":
            return "Refused: OpenAgents host daemon is not serving on this computer"
        case "busy":
            return "Refused: host is temporarily busy; retrying shortly"
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

/// Resilient forward-compatible enums for wire values (#14006).
enum SessionControlKind: Equatable {
    case available
    case observeOnly
    case unknown(String)

    init(raw: String) {
        switch raw {
        case "available": self = .available
        case "observe-only": self = .observeOnly
        default: self = .unknown(raw)
        }
    }
}

enum NoticeKind: Equatable {
    case install
    case login
    case limit
    case note
    case none
    case unknown(String)

    init(raw: String) {
        switch raw {
        case "": self = .none
        case "install": self = .install
        case "login": self = .login
        case "limit": self = .limit
        case "note": self = .note
        default: self = .unknown(raw)
        }
    }
}

enum WorkspacePRState: Equatable {
    case open
    case closed
    case merged
    case draft
    case unknown(String)

    init(raw: String) {
        switch raw.uppercased() {
        case "OPEN": self = .open
        case "CLOSED": self = .closed
        case "MERGED": self = .merged
        case "DRAFT": self = .draft
        default: self = .unknown(raw)
        }
    }
}

enum TailnetPushKind: Equatable {
    case sessionsChanged
    case sessionUpdated
    case interactionPending
    case turnEnded
    case unknown(String)

    init(raw: String) {
        switch raw {
        case "sessions_changed": self = .sessionsChanged
        case "session_updated": self = .sessionUpdated
        case "interaction_pending": self = .interactionPending
        case "turn_ended": self = .turnEnded
        default: self = .unknown(raw)
        }
    }
}

/// Maps every raw wire/host/workspace/tailnet refusal token into plain, actionable English (#14006).
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
        case "missing_right":
            return "This pairing grant does not include permission for that operation. Re-pair from Computers to grant access."
        case "expired":
            return "This pairing grant or request expired. Open Computers and tap Pair over Tailscale to renew."
        case "revoked":
            return "This device's access was revoked on your Mac. Pair again from Computers."
        case "stale":
            return "Your device's grant epoch is out of date. Open Computers and pair again."
        case "wrong_code":
            return "The pairing code did not match. Request a fresh pairing over Tailscale."
        case "denied", "refused":
            return "The host denied this request."
        case "conflict":
            return "This chat is read-only or changed on another device. Start a new chat or refresh to continue."
        case "stale_cursor":
            return "The transcript changed while paginating. Refreshing from the latest turn."
        case "record_limit":
            return "The host reached its session history record limit for this view."
        case "discovery_incomplete":
            return "Some local session files could not be scanned on your Mac."
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
        case "stale_state":
            return "The repository changed since you last loaded it. Refresh Workspace and retry."
        case "dirty_worktree":
            return "Your workspace has uncommitted changes. Commit, stash, or discard them first."
        case "workspace_busy":
            return "An active Antigravity turn is modifying this workspace. Wait for it to finish before running Git changes."
        case "workspace_locked":
            return "This Git worktree is locked and cannot be removed."
        case "github_unavailable":
            return "GitHub CLI (`gh`) isn't installed or signed in on your Mac. Run `gh auth login` on the host."
        case "github_remote_required":
            return "This repository has no GitHub remote configured for pull requests."
        case "workspace_unavailable":
            return "The configured workspace directory isn't accessible on your Mac."
        case "repository_unavailable":
            return "This folder is not a Git repository on your Mac."
        case "diff_unavailable":
            return "Git diff could not be computed for this repository."
        case "branches_unavailable":
            return "Could not read Git branches from this repository."
        case "invalid_branch":
            return "That branch name is invalid or already exists."
        case "detached_head":
            return "The repository is in a detached HEAD state. Switch to or create a branch first."
        case "operation_failed":
            return "The Git operation failed on your Mac. Check repository status and retry."
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
        case "not_serving":
            return "The OpenAgents host daemon is not serving on your Mac."
        case "busy":
            return "The host is temporarily busy processing another request. Try again shortly."
        case "tagged":
            return "Tagged Tailscale devices cannot pair automatically."
        case "not_owner":
            return "Sign into the same Tailscale user account on both devices."
        case "not_tailnet":
            return "Connection did not arrive over your Tailscale network."
        case "unavailable":
            return "The host daemon on your Mac is temporarily unavailable."
        default:
            return "Host refused request (\(code))."
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

    var typedNoticeKind: NoticeKind { NoticeKind(raw: noticeKind) }
}

private struct WireEnvironmentViewDTO: Decodable {
    struct WireSession: Decodable {
        let session_id: String?
        let workspace_id: String?
        let title: String?
        let origin: String?
        let control: String?
        let generation: Int64?
        let revision: Int64?
        let busy: Bool?
        let interaction_id: String?
        let updated_at_ms: Int64?
        let model: String?
        let effort: String?
        let mode: String?
        let surface: String?
        let note: String?
    }
    struct WireItem: Decodable {
        let item_id: String?
        let role: String?
        let text: String?
        let tool_name: String?
        let complete: Bool?
        let started_at_ms: Int64?
        let subagent_calls: Int?
        let subagent_now: String?
        let path: String?
        let diff: String?
        let diff_cut: Bool?
        let notice_kind: String?
    }
    struct WireThought: Decodable {
        let item_id: String?
        let text: String?
        let complete: Bool?
        let at: Int?
    }
    struct WireModel: Decodable {
        let model_id: String?
        let display_name: String?
        let efforts: [String]?
    }
    struct WireInteractionOption: Decodable {
        let optionId: String?
        let option_id: String?
        let id: String?
        let name: String?
        let label: String?
        let kind: String?
    }

    struct WireTodo: Decodable {
        let id: String?
        let content: String?
        let status: String?
    }
    struct WireCommand: Decodable {
        let name: String?
        let description: String?
    }
    struct WireUsage: Decodable {
        let input_tokens: Int?
        let output_tokens: Int?
        let cached_tokens: Int?
        let thought_tokens: Int?
    }

    let host_pubkey: String
    let display_name: String?
    let method: String
    let error: String?
    let complete: Bool?
    let next_cursor: String?
    let workspace_name: String?
    let folders: [String]?
    let sessions: [WireSession]?
    let items: [WireItem]?
    let thoughts: [WireThought]?
    let models: [WireModel]?
    let default_effort: String?
    let interaction_prompt: String?
    let interaction_warning: String?
    let interaction_question: Bool?
    let interaction_options: [WireInteractionOption]?
    let host_version: String?
    let agy_version: String?
    let agy_update_available: Bool?
    let agy_auth_method: String?
    let agy_signed_in: Bool?
    let capabilities: [String]?
    let todos: [WireTodo]?
    let commands: [WireCommand]?
    let usage: WireUsage?
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
    let hostMeta: HostRuntimeMeta?
    let todos: [SessionTodoItem]
    let liveCommands: [SlashCommandItem]
    let usage: SessionUsageStats?

    /// Parses a Coax-validated `EaView` JSON payload via strict `Decodable` structs (#14006)
    /// and filters its model catalog exclusively through Coax `MobileCore.isAntigravityModel`.
    static func parse(_ data: Data) -> ParsedEnvironmentView? {
        guard let dto = try? JSONDecoder().decode(WireEnvironmentViewDTO.self, from: data) else {
            return nil
        }

        let folders = (dto.folders ?? []).filter { MobileCore.isValidFolder($0) }

        let sessions: [ChatSessionSummary] = (dto.sessions ?? []).compactMap { row in
            guard let sid = row.session_id, !sid.isEmpty else { return nil }
            return ChatSessionSummary(
                id: sid,
                workspaceID: row.workspace_id ?? "default",
                title: row.title ?? "",
                managed: row.origin == "managed",
                control: row.control ?? "",
                generation: row.generation ?? 0,
                revision: row.revision ?? 0,
                busy: row.busy ?? false,
                interactionID: row.interaction_id ?? "",
                updatedAtMs: row.updated_at_ms ?? 0,
                model: row.model ?? "",
                effort: row.effort ?? "",
                mode: row.mode ?? "",
                desktopSurface: row.surface == "coder-desktop",
                note: row.note ?? ""
            )
        }
        .sorted { $0.updatedAtMs > $1.updatedAtMs }

        var items: [TranscriptItem] = (dto.items ?? []).compactMap { row in
            guard let iid = row.item_id, !iid.isEmpty else { return nil }
            return TranscriptItem(
                id: iid,
                role: TranscriptItem.Role(rawValue: row.role ?? "") ?? .unknown,
                text: row.text ?? "",
                toolName: row.tool_name ?? "",
                complete: row.complete ?? true,
                startedAtMs: row.started_at_ms ?? 0,
                subagentCalls: row.subagent_calls ?? 0,
                subagentNow: row.subagent_now ?? "",
                path: row.path ?? "",
                diff: row.diff ?? "",
                diffCut: row.diff_cut ?? false,
                noticeKind: row.notice_kind ?? ""
            )
        }

        if let rawThoughts = dto.thoughts, !rawThoughts.isEmpty {
            var inserted = 0
            for row in rawThoughts {
                guard let iid = row.item_id, let at = row.at else { continue }
                let thought = TranscriptItem(
                    id: iid,
                    role: .thought,
                    text: row.text ?? "",
                    toolName: "",
                    complete: row.complete ?? true,
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

        let filteredModels: [AntigravityModelOption] = (dto.models ?? []).compactMap { row in
            guard let id = row.model_id, MobileCore.isAntigravityModel(id) else {
                return nil
            }
            let normalizedID = MobileCore.normalizeAntigravityModel(id)
            let label = row.display_name.flatMap { $0.isEmpty ? nil : $0 } ?? normalizedID
            let efforts = (row.efforts ?? []).compactMap { ThinkingEffort(rawValue: $0) }
            // Preserve empty effort array (e.g. gemini-pro-orchestrator disables --effort; #14005).
            return AntigravityModelOption(
                id: normalizedID,
                label: label,
                supportedEfforts: efforts
            )
        }

        var seen = Set<String>()
        let uniqueModels = filteredModels.filter { seen.insert($0.id).inserted }
        let defaultEffort = ThinkingEffort(rawValue: dto.default_effort ?? "high") ?? .high

        let interactionOptions: [InteractionOption] = (dto.interaction_options ?? []).compactMap { row in
            let oid = row.optionId ?? row.option_id ?? row.id ?? ""
            guard !oid.isEmpty else { return nil }
            let label = row.name ?? row.label ?? oid
            return InteractionOption(id: oid, label: label, kind: row.kind ?? "")
        }

        let hostVer = dto.host_version ?? ""
        let agyVer = dto.agy_version ?? ""
        let agyUpd = dto.agy_update_available ?? false
        let agyAuth = dto.agy_auth_method ?? ""
        let agySignedIn = dto.agy_signed_in
        let caps = Set(dto.capabilities ?? [])
        let hostMeta: HostRuntimeMeta? = (!hostVer.isEmpty || !agyVer.isEmpty || !caps.isEmpty || agySignedIn != nil)
            ? HostRuntimeMeta(
                hostVersion: hostVer,
                agyVersion: agyVer,
                agyUpdateAvailable: agyUpd,
                agyAuthMethod: agyAuth,
                agySignedIn: agySignedIn,
                capabilities: caps
            )
            : nil

        let todos: [SessionTodoItem] = (dto.todos ?? []).enumerated().compactMap { idx, row in
            guard let content = row.content, !content.isEmpty else { return nil }
            return SessionTodoItem(
                id: row.id.flatMap { $0.isEmpty ? nil : $0 } ?? "\(idx + 1)",
                content: content,
                status: row.status ?? "pending"
            )
        }

        let liveCommands: [SlashCommandItem] = (dto.commands ?? []).compactMap { row in
            guard let raw = row.name?.trimmingCharacters(in: .whitespacesAndNewlines), !raw.isEmpty else { return nil }
            let clean = raw.hasPrefix("/") ? String(raw.dropFirst()) : raw
            let cmd = "/\(clean)"
            return SlashCommandItem(
                id: "acp-\(clean)",
                command: cmd,
                subtitle: row.description.flatMap { $0.isEmpty ? nil : $0 } ?? "Antigravity command",
                template: "\(cmd) "
            )
        }

        let usage: SessionUsageStats? = dto.usage.flatMap { u in
            let inp = u.input_tokens ?? 0
            let out = u.output_tokens ?? 0
            let cac = u.cached_tokens ?? 0
            let thk = u.thought_tokens ?? 0
            guard inp > 0 || out > 0 || cac > 0 || thk > 0 else { return nil }
            return SessionUsageStats(inputTokens: inp, outputTokens: out, cachedTokens: cac, thoughtTokens: thk)
        }

        return ParsedEnvironmentView(
            hostPubkey: dto.host_pubkey,
            displayName: dto.display_name ?? "",
            method: dto.method,
            error: dto.error ?? "",
            complete: dto.complete ?? false,
            nextCursor: dto.next_cursor ?? "",
            workspaceName: dto.workspace_name ?? "",
            folders: folders,
            sessions: sessions,
            items: items,
            antigravityModels: uniqueModels,
            defaultEffort: defaultEffort,
            interactionPrompt: dto.interaction_prompt ?? "",
            interactionWarning: dto.interaction_warning ?? "",
            interactionQuestion: dto.interaction_question ?? false,
            interactionOptions: interactionOptions,
            hostMeta: hostMeta,
            todos: todos,
            liveCommands: liveCommands,
            usage: usage
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
    var nextCursor: Int = 0

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
            pullRequests: (newPRs.flatMap { $0.isEmpty ? nil : $0 }) ?? pullRequests,
            nextCursor: nextCursor
        )
    }

    private struct WireWorkspaceViewDTO: Decodable {
        struct WireWorktree: Decodable {
            let id: String?
            let path: String?
            let branch: String?
            let locked: Bool?
        }
        struct WirePR: Decodable {
            let number: Int?
            let title: String?
            let url: String?
            let head: String?
            let base: String?
            let body: String?
            let state: String?
        }

        let host: String
        let workspace: String?
        let revision: String?
        let path: String?
        let repository: String?
        let branch: String?
        let status: String?
        let diff: String?
        let diff_cut: Bool?
        let error: String?
        let github_error: String?
        let worktrees: [WireWorktree]?
        let branches: [String]?
        let pull_requests: [WirePR]?
        let next: Int?
    }

    static func parse(_ data: Data) -> WorkspaceSnapshot? {
        guard let dto = try? JSONDecoder().decode(WireWorkspaceViewDTO.self, from: data) else {
            return nil
        }
        let trees: [WorkspaceWorktree] = (dto.worktrees ?? []).compactMap { row in
            guard let id = row.id, let path = row.path else { return nil }
            return WorkspaceWorktree(
                id: id,
                path: path,
                branch: row.branch ?? "",
                locked: row.locked ?? false
            )
        }
        let prs: [WorkspacePR] = (dto.pull_requests ?? []).compactMap { row in
            guard let num = row.number else { return nil }
            return WorkspacePR(
                number: num,
                title: row.title ?? "",
                url: row.url ?? "",
                head: row.head ?? "",
                base: row.base ?? "",
                body: row.body ?? "",
                state: row.state ?? ""
            )
        }
        return WorkspaceSnapshot(
            host: dto.host,
            workspace: dto.workspace ?? "default",
            revision: dto.revision ?? "",
            path: dto.path ?? "",
            repository: dto.repository ?? "",
            branch: dto.branch ?? "",
            status: dto.status ?? "",
            diff: dto.diff ?? "",
            diffCut: dto.diff_cut ?? false,
            error: dto.error ?? "",
            githubError: dto.github_error ?? "",
            worktrees: trees,
            branches: dto.branches ?? [],
            pullRequests: prs,
            nextCursor: dto.next ?? 0
        )
    }
}

// MARK: - Daemon Session Meta, Inline Review Comments, File Viewer, Mentions & Side Questions (#14012, #14013)

struct SessionMetadataRecord: Identifiable, Equatable, Codable {
    let id: String
    var title: String
    var folder: String
    var pinned: Bool
    var archived: Bool
    var updatedAt: Int64
}

struct ParsedDaemonView {
    private struct WireDaemonDTO: Decodable {
        struct WireForeman: Decodable {
            let tier: Int?
            let workspace: String?
            let prompt: String?
            let updated: Int64?
        }
        struct WireSessionMeta: Decodable {
            let session_id: String?
            let id: String?
            let title: String?
            let folder: String?
            let pinned: Bool?
            let archived: Bool?
            let updated: Int64?
            let updated_at: Int64?
        }
        let ok: Bool?
        let host: String?
        let method: String?
        let error: String?
        let status: String?
        let foreman: WireForeman?
        let sessions_meta: [WireSessionMeta]?
        let sessions: [WireSessionMeta]?
        let meta: [WireSessionMeta]?
    }

    let ok: Bool
    let host: String
    let method: String
    let error: String
    let status: String
    let foremanPrompt: String
    let foremanWorkspace: String
    let sessions: [SessionMetadataRecord]

    static func parse(_ data: Data) -> ParsedDaemonView? {
        guard let dto = try? JSONDecoder().decode(WireDaemonDTO.self, from: data) else { return nil }
        let rows = dto.sessions_meta ?? dto.sessions ?? dto.meta ?? []
        let mapped: [SessionMetadataRecord] = rows.compactMap { row in
            let sid = row.session_id ?? row.id ?? ""
            guard !sid.isEmpty else { return nil }
            return SessionMetadataRecord(
                id: sid,
                title: row.title ?? "",
                folder: row.folder ?? "",
                pinned: row.pinned ?? false,
                archived: row.archived ?? false,
                updatedAt: row.updated ?? row.updated_at ?? 0
            )
        }
        return ParsedDaemonView(
            ok: dto.ok ?? false,
            host: dto.host ?? "",
            method: dto.method ?? "",
            error: dto.error ?? "",
            status: dto.status ?? "",
            foremanPrompt: dto.foreman?.prompt ?? "",
            foremanWorkspace: dto.foreman?.workspace ?? "",
            sessions: mapped
        )
    }
}

struct InlineReviewComment: Identifiable, Equatable, Codable {
    let id: UUID
    let filePath: String
    let lineRange: String
    let comment: String

    init(id: UUID = UUID(), filePath: String, lineRange: String, comment: String) {
        self.id = id
        self.filePath = filePath
        self.lineRange = lineRange
        self.comment = comment
    }

    var renderedMarkdown: String {
        let loc = lineRange.isEmpty ? filePath : "\(filePath) #\(lineRange)"
        return "- `\(loc)`: \(comment)"
    }
}

struct WorkspaceFileContent: Equatable {
    let path: String
    let content: String
    let startLine: Int
    let nextLine: Int
    let truncated: Bool
    let isBinary: Bool
}

struct MentionCandidate: Identifiable, Equatable {
    enum Kind: String {
        case file
        case folder
        case rule
        case conversation
    }

    let id: String
    let kind: Kind
    let label: String
    let insertion: String
    let subtitle: String

    var icon: String {
        switch kind {
        case .file: return "doc.text"
        case .folder: return "folder"
        case .rule: return "checklist"
        case .conversation: return "bubble.left.and.bubble.right"
        }
    }
}

struct SideQuestionEntry: Identifiable, Equatable {
    let id: UUID
    let question: String
    var answer: String
    var isLoading: Bool

    init(id: UUID = UUID(), question: String, answer: String = "", isLoading: Bool = true) {
        self.id = id
        self.question = question
        self.answer = answer
        self.isLoading = isLoading
    }
}

// MARK: - Wave 4 Models: Artifacts, Swarms, Automations, Extensibility, Voice/Browser/Quotas, Offline Outbox (#14014-#14019)

enum ArtifactReviewPolicy: String, CaseIterable, Identifiable, Codable {
    case asksForReview = "asks-for-review"
    case agentDecides = "agent-decides"
    case alwaysProceed = "always-proceed"

    var id: String { rawValue }
    var label: String {
        switch self {
        case .asksForReview: return "Ask for Review"
        case .agentDecides: return "Agent Decides"
        case .alwaysProceed: return "Always Proceed"
        }
    }
}

struct ArtifactSummary: Identifiable, Equatable {
    let id: String // relative filename inside <brain>/<sessionId>
    let kind: String // plan, walkthrough, task, html, image, video, learning, markdown
    let displayName: String
    let summary: String
    let requestFeedback: Bool
    let updatedAt: Int64

    var icon: String {
        switch kind {
        case "plan": return "list.bullet.clipboard"
        case "walkthrough": return "checkmark.seal"
        case "task": return "checklist"
        case "html": return "rectangle.3.group"
        case "image": return "photo"
        case "video": return "video"
        case "learning": return "brain"
        default: return "doc.richtext"
        }
    }

    static func fromDaemonMeta(_ row: SessionMetadataRecord) -> ArtifactSummary {
        let parts = row.title.split(separator: "|", maxSplits: 1)
        let kind = parts.first.map(String.init) ?? "markdown"
        let display = parts.count > 1 ? String(parts[1]) : row.id
        return ArtifactSummary(
            id: row.id,
            kind: kind,
            displayName: display,
            summary: row.folder,
            requestFeedback: row.pinned,
            updatedAt: row.updatedAt
        )
    }
}

struct ArtifactDocument: Equatable {
    let name: String
    let mimeType: String
    let localPath: String
    let isImage: Bool
    let content: String // text/markdown/html OR base64 for image
    let byteCount: Int64

    var imageData: Data? {
        guard isImage else { return nil }
        return Data(base64Encoded: content)
    }
}

struct SwarmMemberEntry: Identifiable, Equatable {
    let id: String
    let name: String
    let model: String
    let state: String // active, waiting, stopped, retired, quota_paused
    let cwd: String
    let isActive: Bool
    let startedAt: Int64
}

struct SwarmBoardPost: Identifiable, Equatable {
    let id: String
    let author: String
    let message: String
    let index: Int64
}

struct SwarmSnapshot: Equatable {
    let sessionID: String
    let activeSubagents: Int
    let members: [SwarmMemberEntry]
    let posts: [SwarmBoardPost]
}

struct AutomationRecord: Identifiable, Equatable {
    let id: String // sched:<id> or sidecar:<id>
    let name: String
    let cron: String
    let model: String
    let folder: String
    let prompt: String
    let lastChatID: String
    let enabled: Bool
    let isSidecar: Bool
    let lastRunMs: Int64

    static func fromDaemonMeta(_ row: SessionMetadataRecord) -> AutomationRecord {
        let tParts = row.title.split(separator: "|", omittingEmptySubsequences: false).map(String.init)
        let fParts = row.folder.split(separator: "|", omittingEmptySubsequences: false).map(String.init)
        return AutomationRecord(
            id: row.id,
            name: tParts.first.flatMap { $0.isEmpty ? nil : $0 } ?? row.id,
            cron: tParts.count > 1 ? tParts[1] : "",
            model: tParts.count > 2 ? tParts[2] : "antigravity",
            folder: fParts.first ?? "",
            prompt: fParts.count > 1 ? fParts[1] : "",
            lastChatID: fParts.count > 2 ? fParts[2] : "",
            enabled: row.pinned,
            isSidecar: row.archived,
            lastRunMs: row.updatedAt
        )
    }
}

struct PermissionRuleEntry: Identifiable, Equatable {
    let id: String
    let bucket: String // allow, ask, deny
    let rule: String
}

struct MCPServerRecord: Identifiable, Equatable {
    let id: String
    let name: String
    let endpointOrCommand: String
    let enabled: Bool
}

struct CustomizationItem: Identifiable, Equatable {
    let id: String
    let kind: String // skill, rule, agent, plugin, hook
    let name: String
    let sourcePath: String
}

struct AntigravityConfigSnapshot: Equatable {
    let toolPermission: String
    let artifactReviewPolicy: ArtifactReviewPolicy
    let enableTerminalSandbox: Bool
    let sandboxProxy: Bool
    let useG1Credits: Bool
    let adminSettingsPresent: Bool
    let agyVersion: String
    let authMode: String
    let permissionRules: [PermissionRuleEntry]
    let mcpServers: [MCPServerRecord]
    let customizations: [CustomizationItem]

    static func fromDaemonView(_ view: ParsedDaemonView) -> AntigravityConfigSnapshot {
        var toolPermission = "request-review"
        var policy = ArtifactReviewPolicy.asksForReview
        var sandbox = false
        var proxy = false
        var useG1 = false
        var admin = false
        var agyVer = ""
        var authMode = ""

        if let data = view.foremanPrompt.data(using: .utf8),
           let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            if let tp = obj["toolPermission"] as? String, !tp.isEmpty { toolPermission = tp }
            if let ap = obj["artifactReviewPolicy"] as? String, let parsed = ArtifactReviewPolicy(rawValue: ap) { policy = parsed }
            sandbox = obj["enableTerminalSandbox"] as? Bool ?? false
            proxy = obj["sandboxProxy"] as? Bool ?? false
            useG1 = obj["useG1Credits"] as? Bool ?? false
            admin = obj["adminSettingsPresent"] as? Bool ?? false
            agyVer = obj["agyVersion"] as? String ?? ""
            authMode = obj["authMode"] as? String ?? ""
        }

        var perms: [PermissionRuleEntry] = []
        var mcps: [MCPServerRecord] = []
        var customs: [CustomizationItem] = []

        for row in view.sessions {
            if row.id.hasPrefix("perm:") {
                perms.append(PermissionRuleEntry(id: row.id, bucket: row.title, rule: row.folder))
            } else if row.id.hasPrefix("mcp:") {
                mcps.append(MCPServerRecord(id: row.id, name: row.title, endpointOrCommand: row.folder, enabled: row.pinned))
            } else if let colon = row.id.firstIndex(of: ":") {
                let kind = String(row.id[..<colon])
                customs.append(CustomizationItem(id: row.id, kind: kind, name: row.title, sourcePath: row.folder))
            }
        }
        return AntigravityConfigSnapshot(
            toolPermission: toolPermission,
            artifactReviewPolicy: policy,
            enableTerminalSandbox: sandbox,
            sandboxProxy: proxy,
            useG1Credits: useG1,
            adminSettingsPresent: admin,
            agyVersion: agyVer,
            authMode: authMode,
            permissionRules: perms,
            mcpServers: mcps,
            customizations: customs
        )
    }
}

struct RateLimitBannerState: Equatable {
    let isHardExhaustion: Bool
    let message: String
    let retryUntil: Date?
}

struct PendingOutboxMessage: Identifiable, Equatable, Codable {
    let id: String // stable 32-hex command_id for duplicate-free replay
    let hostID: String
    let sessionID: String?
    let text: String
    let model: String
    let effort: String
    let reviewMode: String
    let folder: String
    let createdAt: Int64
}

struct ActiveTurnLiveStatus: Equatable {
    let hostName: String
    let sessionTitle: String
    let phaseLabel: String
    let startedAt: Date
    let needsApproval: Bool
}
