// Firm reason: 6 (iOS OpenAgents Cloud models, API origin resolver, execution targets, Tier 1 tools, and Swift wrappers over #authority Coax C ABI in cloud-chat-bridge.h).
import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

enum CloudAPIBase {
    static let defaultProductionOrigin = "https://openagents.com"

    /// Resolves API origin cleanly: OA_API_BASE env -> UserDefaults OA_API_BASE / OAAPIOrigin -> production.
    static var origin: String {
        if let env = ProcessInfo.processInfo.environment["OA_API_BASE"]?.trimmingCharacters(in: .whitespacesAndNewlines),
           !env.isEmpty,
           URL(string: env) != nil {
            return stripTrailingSlash(env)
        }
        if let ud = UserDefaults.standard.string(forKey: "OA_API_BASE")?.trimmingCharacters(in: .whitespacesAndNewlines),
           !ud.isEmpty,
           URL(string: ud) != nil {
            return stripTrailingSlash(ud)
        }
        #if DEBUG
        if let o = UserDefaults.standard.string(forKey: "OAAPIOrigin")?.trimmingCharacters(in: .whitespacesAndNewlines),
           !o.isEmpty,
           URL(string: o) != nil {
            return stripTrailingSlash(o)
        }
        #endif
        return defaultProductionOrigin
    }

    private static func stripTrailingSlash(_ s: String) -> String {
        var out = s
        while out.hasSuffix("/") { out.removeLast() }
        return out
    }
}

enum CloudTranscriptRole: String, Codable, Equatable {
    case user
    case assistant
    case thought
    case tool
    case notice
}

struct CloudTranscriptItem: Identifiable, Equatable {
    let id: String
    var role: CloudTranscriptRole
    var text: String
    var toolName: String = ""
    var complete: Bool = true
    var failed: Bool = false
    var startedAtMs: Int64 = 0
    var path: String = ""
    var diff: String = ""
    var diffCut: Bool = false
}

struct CloudModelOption: Identifiable, Equatable {
    let id: String
    let wireModel: String
    let label: String
    let chipLabel: String
    let detail: String
    let supportsThinking: Bool

    static let catalog: [CloudModelOption] = [
        CloudModelOption(
            id: "openagents/auto",
            wireModel: "google/gemini-4-neon",
            label: "OpenAgents Auto",
            chipLabel: "OA Auto",
            detail: "Automatic cloud routing on OpenAgents Core",
            supportsThinking: true
        ),
        CloudModelOption(
            id: "google/gemini-4-neon",
            wireModel: "google/gemini-4-neon",
            label: "Gemini 4 Neon",
            chipLabel: "Gemini 4 Neon",
            detail: "Flagship OpenAgents Cloud model",
            supportsThinking: true
        ),
        CloudModelOption(
            id: "google/gemini-4-hydrogen",
            wireModel: "google/gemini-4-hydrogen",
            label: "Gemini 4 Hydrogen",
            chipLabel: "Gemini 4 Hydrogen",
            detail: "Fast OpenAgents Cloud model",
            supportsThinking: true
        ),
        CloudModelOption(
            id: "google/gemini-4-argon",
            wireModel: "google/gemini-4-argon",
            label: "Gemini 4 Argon",
            chipLabel: "Gemini 4 Argon",
            detail: "Reasoning OpenAgents Cloud model",
            supportsThinking: true
        ),
        CloudModelOption(
            id: "google/gemini-4-carbon",
            wireModel: "google/gemini-4-carbon",
            label: "Gemini 4 Carbon",
            chipLabel: "Gemini 4 Carbon",
            detail: "High-throughput OpenAgents Cloud model",
            supportsThinking: true
        ),
    ]

    static func option(for id: String) -> CloudModelOption {
        catalog.first { $0.id == id } ?? catalog[0]
    }
}

enum CloudThinkingEffort: String, CaseIterable, Equatable {
    case low
    case medium
    case high

    var label: String {
        switch self {
        case .low: return "Low"
        case .medium: return "Medium"
        case .high: return "High"
        }
    }

    var detail: String {
        switch self {
        case .low: return "Fastest replies"
        case .medium: return "Balanced speed and reasoning"
        case .high: return "Deeper multi-step reasoning"
        }
    }
}

enum CloudExecutionTarget: Equatable {
    case auto
    case cloud
    case sandbox(size: String)
    case computer(hostPubkey: String, name: String)

    static let autoMachineID = "target:auto"
    static let cloudMachineID = "target:cloud"
    static let sandboxMachineID = "target:sandbox"

    var machineID: String {
        switch self {
        case .auto: return Self.autoMachineID
        case .cloud: return Self.cloudMachineID
        case .sandbox: return Self.sandboxMachineID
        case .computer(let host, _): return "host:\(host)"
        }
    }

    var displayLabel: String {
        switch self {
        case .auto: return "OpenAgents Cloud"
        case .cloud: return "Cloud Instant"
        case .sandbox(let size): return "Cloud Sandbox (\(size))"
        case .computer(_, let name): return name.isEmpty ? "Enrolled Computer" : name
        }
    }

    var shortChipLabel: String {
        switch self {
        case .auto: return "Auto"
        case .cloud: return "Cloud"
        case .sandbox: return "Sandbox"
        case .computer(_, let name): return name.isEmpty ? "Computer" : name
        }
    }

    var persistedString: String {
        switch self {
        case .auto: return "auto"
        case .cloud: return "cloud"
        case .sandbox(let size): return "sandbox:\(size)"
        case .computer(let host, let name): return "computer:\(host)|\(name)"
        }
    }

    static func fromPersisted(_ raw: String?) -> CloudExecutionTarget {
        guard let raw, !raw.isEmpty else { return .auto }
        if raw == "cloud" { return .cloud }
        if raw.hasPrefix("sandbox:") {
            let sz = String(raw.dropFirst("sandbox:".count))
            return .sandbox(size: sz.isEmpty ? "small" : sz)
        }
        if raw.hasPrefix("computer:") {
            let body = String(raw.dropFirst("computer:".count))
            let parts = body.split(separator: "|", maxSplits: 1, omittingEmptySubsequences: false)
            let host = parts.first.map(String.init) ?? ""
            let name = parts.count > 1 ? String(parts[1]) : ""
            if host.count == 64 {
                return .computer(hostPubkey: host, name: name)
            }
        }
        return .auto
    }
}

struct CloudWorkspace: Identifiable, Equatable {
    let workspaceID: String
    let name: String
    let localPath: String
    let repoRemote: String
    let repoCanonical: String
    let branch: String

    var id: String { "\(workspaceID):\(localPath)" }
}

struct CloudEnvironment: Identifiable, Equatable {
    let hostPubkey: String
    let displayName: String
    let os: String
    let online: Bool
    let lastSeenMs: Int64
    let workspaces: [CloudWorkspace]

    var id: String { hostPubkey }

    var osLabel: String {
        switch os.lowercased() {
        case "macos", "darwin": return "macOS"
        case "linux": return "Linux"
        case "windows": return "Windows"
        default: return os.isEmpty ? "Computer" : os
        }
    }
}

struct CloudProject: Identifiable, Equatable {
    let id: String
    let revision: Int64
    let name: String
    let description: String
    let repoCanonical: String
    let updatedAtMs: Int64
}

struct CloudPlacementDecision: Equatable {
    let tier: Int
    let compute: String
    let hostPubkey: String
    let hostName: String
    let reason: String
    let sandboxRateUsdMicrosHr: Int64
    let creditMsat: Int64
}

struct CloudUsageSnapshot: Equatable {
    var todayUsdMicros: Int64 = 0
    var monthUsdMicros: Int64 = 0
    var totalUsdMicros: Int64 = 0
    var todayTokens: Int64 = 0
    var totalTokens: Int64 = 0
    var todayRequests: Int64 = 0
    var totalCalls: Int64 = 0
    var creditMsat: Int64 = 0
    var promoMicros: Int64 = 0
    var promoExpiresAt: String = ""
    var sandboxTurns: Int64 = 0
    var sandboxSeconds: Int64 = 0
    var sandboxMsat: Int64 = 0
    var localTurns: Int64 = 0

    static func formatUsd(micros: Int64) -> String {
        let dollars = Double(max(0, micros)) / 1_000_000.0
        if dollars < 0.01 && micros > 0 {
            return String(format: "$%.4f", dollars)
        }
        return String(format: "$%.2f", dollars)
    }

    static func formatTokens(_ count: Int64) -> String {
        if count >= 1_000_000 {
            return String(format: "%.1fM", Double(count) / 1_000_000.0)
        }
        if count >= 1_000 {
            return String(format: "%.1fk", Double(count) / 1_000.0)
        }
        return "\(max(0, count))"
    }
}

struct CloudAttachment: Identifiable, Equatable {
    let id = UUID()
    let name: String
    let mimeType: String
    let textPreview: String?
    let imageData: Data?
    let byteCount: Int

    var isImage: Bool { imageData != nil }

    static func prepareImage(data: Data, name: String) -> CloudAttachment {
        let safeName = name.replacingOccurrences(of: " ", with: "_")
        if let src = CGImageSourceCreateWithData(data as CFData, nil),
           let cg = CGImageSourceCreateImageAtIndex(src, 0, nil) {
            let maxSide: CGFloat = 1024
            let w = CGFloat(cg.width)
            let h = CGFloat(cg.height)
            let scale = min(1.0, maxSide / max(1, max(w, h)))
            let nw = max(1, Int((w * scale).rounded()))
            let nh = max(1, Int((h * scale).rounded()))
            if let ctx = CGContext(
                data: nil,
                width: nw,
                height: nh,
                bitsPerComponent: 8,
                bytesPerRow: 0,
                space: CGColorSpaceCreateDeviceRGB(),
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
            ) {
                ctx.interpolationQuality = .high
                ctx.draw(cg, in: CGRect(x: 0, y: 0, width: nw, height: nh))
                if let scaled = ctx.makeImage() {
                    let out = NSMutableData()
                    if let dest = CGImageDestinationCreateWithData(out, UTType.jpeg.identifier as CFString, 1, nil) {
                        CGImageDestinationAddImage(dest, scaled, [kCGImageDestinationLossyCompressionQuality: 0.72] as CFDictionary)
                        if CGImageDestinationFinalize(dest) {
                            let jpeg = out as Data
                            return CloudAttachment(
                                name: safeName,
                                mimeType: "image/jpeg",
                                textPreview: nil,
                                imageData: jpeg,
                                byteCount: jpeg.count
                            )
                        }
                    }
                }
            }
        }
        return CloudAttachment(name: safeName, mimeType: "image/jpeg", textPreview: nil, imageData: data, byteCount: data.count)
    }

    func promptNote() -> String {
        if let textPreview, !textPreview.isEmpty {
            return "\n\n[Attached file: \(name)]\n```\n\(textPreview)\n```"
        }
        return "\n\n[Attached file: \(name) (\(mimeType), \(byteCount) bytes)]"
    }
}

struct CloudSlashCommand: Identifiable, Equatable {
    let id: String
    let command: String
    let subtitle: String
    let replacement: String

    static let all: [CloudSlashCommand] = [
        CloudSlashCommand(id: "slash-new", command: "/new", subtitle: "Start a fresh Cloud conversation", replacement: "/new"),
        CloudSlashCommand(id: "slash-auto", command: "/auto", subtitle: "Use Auto execution placement", replacement: "/auto"),
        CloudSlashCommand(id: "slash-cloud", command: "/cloud", subtitle: "Target instant Cloud Tools", replacement: "/cloud"),
        CloudSlashCommand(id: "slash-sandbox", command: "/sandbox", subtitle: "Target Cloud Sandbox", replacement: "/sandbox"),
    ]
}

/// Pure Swift wrappers over the compiled `#authority` Coax C ABI (`cloud-chat-bridge.h`).
enum CloudCore {
    static let workReadOnly: Int64 = 0
    static let workCodeExec: Int64 = 1

    static func isValidCompute(_ compute: String, costMsat: Int64) -> Bool {
        oa_cc_compute_valid(compute, costMsat)
    }

    static func isCloudToolAllowed(_ tool: String) -> Bool {
        oa_cc_tool_kind_valid(tool)
    }

    static func webToolsJSON() -> String {
        var buf = [UInt8](repeating: 0, count: 8192)
        let n = oa_cc_web_tools_json(&buf, Int64(buf.count))
        guard n > 0 else { return "[]" }
        return String(decoding: buf[0..<Int(n)], as: UTF8.self)
    }

    static func webToolsObjects() -> [[String: Any]] {
        guard let data = webToolsJSON().data(using: .utf8),
              let arr = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]] else {
            return []
        }
        return arr
    }

    static func classifyPrompt(_ prompt: String) -> Int64 {
        oa_cc_classify_prompt(prompt)
    }

    static func encodeSubmit(
        operation: String,
        conversation: String,
        revision: Int64,
        project: String = "",
        projectRevision: Int64 = -1,
        prompt: String
    ) -> Data? {
        var buf = [UInt8](repeating: 0, count: 32768)
        let n = oa_cc_submit_write(operation, conversation, revision, project, projectRevision, prompt, &buf, Int64(buf.count))
        guard n > 0 else { return nil }
        return Data(buf[0..<Int(n)])
    }

    static func encodeStop(operation: String, turn: String) -> Data? {
        var buf = [UInt8](repeating: 0, count: 512)
        let n = oa_cc_stop_write(operation, turn, &buf, Int64(buf.count))
        guard n > 0 else { return nil }
        return Data(buf[0..<Int(n)])
    }

    static func encodeMeta(
        operation: String,
        title: String? = nil,
        pinned: Bool? = nil,
        archived: Bool? = nil
    ) -> Data? {
        var buf = [UInt8](repeating: 0, count: 1024)
        let n = oa_cc_meta_write(
            operation,
            title != nil, title ?? "",
            pinned != nil, pinned ?? false,
            archived != nil, archived ?? false,
            &buf, Int64(buf.count)
        )
        guard n > 0 else { return nil }
        return Data(buf[0..<Int(n)])
    }

    static func fullAutoShouldGate(fullAuto: Bool, approvalMode: Bool) -> Bool {
        oa_cc_fa_should_gate(fullAuto, approvalMode)
    }

    static func fullAutoDecidePrompt(fullAuto: Bool, autoAnswerQuestions: Bool, kind: Int64) -> Int64 {
        oa_cc_fa_decide_prompt(fullAuto, autoAnswerQuestions, kind)
    }

    static func voiceDuplexVadStep(state: inout [Int64], rmsBp: Int64, dtMs: Int64) -> Int64 {
        guard state.count >= 7 else { return 0 }
        return state.withUnsafeMutableBufferPointer { buf in
            oa_cc_vd_vad_step(buf.baseAddress, rmsBp, dtMs)
        }
    }

    static func voiceDuplexSanitizeSpeech(_ src: String) -> String {
        var buf = [UInt8](repeating: 0, count: max(256, src.utf8.count + 64))
        let n = oa_cc_vd_sanitize_speech(src, &buf, Int64(buf.count))
        guard n > 0 else { return "" }
        return String(decoding: buf[0..<Int(n)], as: UTF8.self)
    }

    static func voiceDuplexSentenceEnd(_ src: String) -> Int {
        Int(oa_cc_vd_sentence_end(src))
    }
}

extension String {
    func nilIfContainsBinaryNUL() -> String? {
        contains("\0") ? nil : self
    }

    func truncatedToUTF8Bytes(_ maxBytes: Int) -> String {
        guard utf8.count > maxBytes else { return self }
        var end = startIndex
        var used = 0
        for idx in indices {
            let next = index(after: idx)
            let charBytes = self[idx..<next].utf8.count
            if used + charBytes > maxBytes { break }
            used += charBytes
            end = next
        }
        return String(self[..<end])
    }
}
