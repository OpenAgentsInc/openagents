// Firm reason: 6 (macOS host transport, durable offline outbox, and SSE sync over the shared #authority Coax codec account-chat-cloud-wire.coax; #14093).
import CryptoKit
import Foundation

/// Materialized conversation received from Web, Mobile, or another Desktop via GET /api/chat/v2/conversations/:id.
struct RemoteCloudConversation: Equatable {
    let conversationID: String
    let localChatID: String
    let title: String
    let pinned: Bool
    let archived: Bool
    let workspace: String
    let chatJSON: String
}

/// One queued write-through operation stored in cloud-outbox.json for offline resilience.
struct CloudOutboxItem: Codable, Equatable {
    let operationID: String
    let path: String
    let body: String
    var attempts: Int
    var nextRetryMs: Int64
}

/// Write-through cloud chat sync client connecting Desktop oa-worker <-> oa-api Postgres <-> Web & Mobile (#14093).
/// All wire JSON payloads are produced exclusively by the #authority Coax codec (cc_event_write, cc_sync_write, cc_meta_write).
final class CloudChatSync: @unchecked Sendable {
    typealias Transport = @Sendable (URLRequest) async throws -> (Data, Int)
    typealias Authorizer = @Sendable (URLRequest) async throws -> URLRequest

    // Event kind constants matching account-chat-cloud-wire.coax.
    static let evStarted: Int64 = 1
    static let evUserMessage: Int64 = 2
    static let evTextDelta: Int64 = 3
    static let evThinkingDelta: Int64 = 4
    static let evToolCall: Int64 = 5
    static let evToolProgress: Int64 = 6
    static let evToolResult: Int64 = 7
    static let evApproval: Int64 = 8
    static let evUsage: Int64 = 9
    static let evCompleted: Int64 = 10
    static let evFailed: Int64 = 11
    static let evCancelled: Int64 = 12
    static let evUnknown: Int64 = 13

    // Turn state constants matching account-chat-turn-policy.coax.
    static let stateQueued: Int64 = 1
    static let stateRunning: Int64 = 2
    static let stateCompleted: Int64 = 3
    static let stateFailed: Int64 = 4
    static let stateCancelled: Int64 = 5
    static let stateUnknown: Int64 = 6

    private struct ActiveTurnState {
        var ordinal: Int = 1
        var turnID: String = ""
        var sequence: Int64 = 0
        var output: String = ""
        var userText: String = ""
    }

    private let stateDir: URL
    private let originProvider: () -> String
    private let isSignedIn: () -> Bool
    private let authorizer: Authorizer
    private let transport: Transport
    private let sourceDevice: String
    private let lock = NSLock()

    private var outbox: [CloudOutboxItem] = []
    private var convToLocal: [String: String] = [:]
    private var lastSyncedDigest: [String: String] = [:]
    private var activeTurns: [String: ActiveTurnState] = [:]
    private var flushing = false
    private var sseTask: Task<Void, Never>?
    private var sseCursor: Int64 = 0

    /// Invoked when an incoming Web/Mobile/cross-device event materializes a conversation update.
    var onRemoteConversation: (@Sendable (RemoteCloudConversation) -> Void)?

    private var outboxURL: URL { stateDir.appendingPathComponent("cloud-outbox.json") }
    private var mapURL: URL { stateDir.appendingPathComponent("cloud-sync-map.json") }
    private var cursorURL: URL { stateDir.appendingPathComponent("cloud-cursor.txt") }

    init(
        stateDir: URL,
        originProvider: @escaping () -> String,
        isSignedIn: @escaping () -> Bool,
        authorizer: @escaping Authorizer,
        transport: @escaping Transport = { req in
            let (data, resp) = try await URLSession.shared.data(for: req)
            return (data, (resp as? HTTPURLResponse)?.statusCode ?? 0)
        },
        sourceDevice: String = "mac-desktop"
    ) {
        self.stateDir = stateDir
        self.originProvider = originProvider
        self.isSignedIn = isSignedIn
        self.authorizer = authorizer
        self.transport = transport
        self.sourceDevice = sourceDevice
        try? FileManager.default.createDirectory(at: stateDir, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        loadState()
    }

    // MARK: - Deterministic Identity & Provenance Mapping

    static func isCanonicalConvID(_ id: String) -> Bool {
        guard id.count == 36, id.hasPrefix("cnv_") else { return false }
        for b in id.dropFirst(4).utf8 {
            let ok = (b >= 48 && b <= 57) || (b >= 97 && b <= 102)
            if !ok { return false }
        }
        return true
    }

    static func conversationID(for localID: String) -> String {
        if isCanonicalConvID(localID) { return localID }
        let hash = SHA256.hash(data: Data(("openagents.desktop.chat:" + localID).utf8))
        let hex = hash.prefix(16).map { String(format: "%02x", $0) }.joined()
        return "cnv_" + hex
    }

    static func turnID(for conversationID: String, ordinal: Int) -> String {
        let hash = SHA256.hash(data: Data(("openagents.desktop.turn:\(conversationID):\(ordinal)").utf8))
        let hex = hash.prefix(16).map { String(format: "%02x", $0) }.joined()
        return "trn_" + hex
    }

    static func classifyModel(_ model: String, isLocalHint: Bool = false, observedCostMsat: Int64 = 0) -> (compute: String, costMsat: Int64) {
        let low = model.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        let isLocal = isLocalHint
            || low.contains("inkling")
            || low.hasPrefix("local:")
            || low == "local"
            || low.hasPrefix("psionic:")
        if isLocal {
            return ("local", 0)
        }
        return ("cloud", max(0, observedCostMsat))
    }

    static func validateCompute(compute: String, costMsat: Int64) -> Bool {
        withCStr(compute) { cs in cc_compute_valid(cs, costMsat) }
    }

    // MARK: - #authority C Codec Bridges

    private static func withCStr<R>(_ s: String, _ body: (coax_string) -> R) -> R {
        let bytes = Array(s.utf8)
        if bytes.isEmpty {
            var z: CChar = 0
            return withUnsafePointer(to: &z) { p in body(coax_string(data: p, count: 0)) }
        }
        return bytes.withUnsafeBytes { raw in
            let ptr = raw.bindMemory(to: CChar.self).baseAddress
            return body(coax_string(data: ptr, count: Int64(bytes.count)))
        }
    }

    private static func clampUTF8(_ s: String, maxBytes: Int) -> String {
        let bytes = Array(s.utf8)
        if bytes.count <= maxBytes { return s }
        var end = maxBytes
        while end > 0 && (bytes[end] & 0xC0) == 0x80 { end -= 1 }
        return String(decoding: bytes[0..<end], as: UTF8.self)
    }

    private static func sanitize(_ s: String, maxBytes: Int) -> String {
        let noNul = s.replacingOccurrences(of: "\0", with: "")
        return clampUTF8(noNul, maxBytes: maxBytes)
    }

    static func encodeEvent(
        conversationID: String,
        turnID: String,
        sequence: Int64,
        kind: Int64,
        state: Int64,
        text: String = "",
        toolID: String = "",
        toolName: String = "",
        toolArgs: String = "",
        diff: String = "",
        failed: Bool = false,
        approvalStatus: String = "",
        attachmentsJSON: String = "",
        compute: String = "",
        model: String = "",
        costMsat: Int64 = 0,
        sourceDevice: String = "mac-desktop",
        workspace: String = "",
        usage: (input: Int64, output: Int64, cacheRead: Int64, cacheWrite: Int64, reasoning: Int64)? = nil,
        stopReason: String = "",
        createdAtMs: Int64 = Int64(Date().timeIntervalSince1970 * 1000)
    ) -> String? {
        let sText = sanitize(text, maxBytes: 16384)
        let sToolID = sanitize(toolID, maxBytes: 128)
        let sToolName = sanitize(toolName, maxBytes: 128)
        let sToolArgs = sanitize(toolArgs, maxBytes: 16384)
        let sDiff = sanitize(diff, maxBytes: 32768)
        let sModel = sanitize(model, maxBytes: 160)
        let sDev = sanitize(sourceDevice, maxBytes: 80)
        let sWS = sanitize(workspace, maxBytes: 512)
        let sStop = sanitize(stopReason, maxBytes: 80)
        let effCost: Int64 = (compute == "local" || compute.isEmpty) ? 0 : max(0, costMsat)
        let u = coax_t_CtUsage(
            input: usage?.input ?? -1,
            output: usage?.output ?? -1,
            cache_read: usage?.cacheRead ?? -1,
            cache_write: usage?.cacheWrite ?? -1,
            reasoning: usage?.reasoning ?? -1
        )
        var out = [UInt8](repeating: 0, count: 131072)
        let n: Int64 = withCStr(conversationID) { cConv in
            withCStr(turnID) { cTurn in
                withCStr(sText) { cText in
                    withCStr(sToolID) { cTid in
                        withCStr(sToolName) { cTname in
                            withCStr(sToolArgs) { cTargs in
                                withCStr(sDiff) { cDiff in
                                    withCStr(approvalStatus) { cApp in
                                        withCStr(attachmentsJSON) { cAtt in
                                            withCStr(compute) { cComp in
                                                withCStr(sModel) { cMod in
                                                    withCStr(sDev) { cDev in
                                                        withCStr(sWS) { cWs in
                                                            withCStr(sStop) { cStop in
                                                                var ev = coax_t_CcEvent(
                                                                    conversation: cConv, turn: cTurn, sequence: sequence,
                                                                    kind: kind, state: state, text: cText,
                                                                    tool_id: cTid, tool_name: cTname, tool_args: cTargs,
                                                                    diff: cDiff, failed: failed, approval_status: cApp,
                                                                    attachments: cAtt, compute: cComp, model: cMod,
                                                                    cost_msat: effCost, source_device: cDev, workspace: cWs,
                                                                    has_usage: usage != nil, usage: u, stop_reason: cStop,
                                                                    created_at_ms: max(0, createdAtMs)
                                                                )
                                                                return withUnsafeMutablePointer(to: &ev) { ep in
                                                                    out.withUnsafeMutableBufferPointer { bp in
                                                                        cc_event_write(
                                                                            coax_v_t_CcEvent(data: ep, count: 1),
                                                                            coax_v_u8(data: bp.baseAddress, count: Int64(bp.count))
                                                                        )
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        guard n > 0 else { return nil }
        return String(decoding: out[0..<Int(n)], as: UTF8.self)
    }

    static func encodeSync(
        operationID: String,
        conversationID: String,
        turnID: String,
        title: String,
        pinned: Bool? = nil,
        archived: Bool? = nil,
        compute: String,
        model: String,
        costMsat: Int64,
        sourceDevice: String = "mac-desktop",
        workspace: String = "",
        userText: String = "",
        attachmentsJSON: String = "",
        state: Int64 = 0,
        output: String = "",
        stopReason: String = "",
        error: String = "",
        usage: (input: Int64, output: Int64, cacheRead: Int64, cacheWrite: Int64, reasoning: Int64)? = nil,
        eventsJSON: [String] = []
    ) -> String? {
        let sOp = sanitize(operationID, maxBytes: 80)
        let sTitle = sanitize(title, maxBytes: 160)
        let sModel = sanitize(model.isEmpty ? "google/gemini-4-neon" : model, maxBytes: 160)
        let sDev = sanitize(sourceDevice, maxBytes: 80)
        let sWS = sanitize(workspace, maxBytes: 512)
        let sUser = sanitize(userText, maxBytes: 32768)
        let sOut = sanitize(output, maxBytes: 131072)
        let sStop = sanitize(stopReason, maxBytes: 80)
        let sErr = sanitize(error, maxBytes: 512)
        let effCost: Int64 = compute == "local" ? 0 : max(0, costMsat)
        let u = coax_t_CtUsage(
            input: usage?.input ?? -1,
            output: usage?.output ?? -1,
            cache_read: usage?.cacheRead ?? -1,
            cache_write: usage?.cacheWrite ?? -1,
            reasoning: usage?.reasoning ?? -1
        )
        let clampedEvents = Array(eventsJSON.prefix(32))
        let eventBuffers: [[CChar]] = clampedEvents.map { Array($0.utf8).map { CChar(bitPattern: $0) } }
        var out = [UInt8](repeating: 0, count: 524288)
        let zeros = coax_a32_s64(e: (0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0))

        let n: Int64 = withCStr(sOp) { cOp in
            withCStr(conversationID) { cConv in
                withCStr(turnID) { cTurn in
                    withCStr(sTitle) { cTitle in
                        withCStr(compute) { cComp in
                            withCStr(sModel) { cMod in
                                withCStr(sDev) { cDev in
                                    withCStr(sWS) { cWs in
                                        withCStr(sUser) { cUser in
                                            withCStr(attachmentsJSON) { cAtt in
                                                withCStr(sOut) { cOut in
                                                    withCStr(sStop) { cStop in
                                                        withCStr(sErr) { cErr in
                                                            var req = coax_t_CcSyncReq(
                                                                operation: cOp, conversation: cConv, turn: cTurn, title: cTitle,
                                                                has_pinned: pinned != nil, pinned: pinned ?? false,
                                                                has_archived: archived != nil, archived: archived ?? false,
                                                                compute: cComp, model: cMod, cost_msat: effCost,
                                                                source_device: cDev, workspace: cWs, user_text: cUser,
                                                                attachments: cAtt, state: state, output: cOut,
                                                                stop_reason: cStop, error: cErr,
                                                                has_usage: usage != nil, usage: u,
                                                                event_count: 0, event_starts: zeros, event_ends: zeros
                                                            )
                                                            var cEvs: [coax_string] = []
                                                            cEvs.reserveCapacity(eventBuffers.count)
                                                            var pinnedPtrs: [UnsafeMutablePointer<CChar>] = []
                                                            defer { for p in pinnedPtrs { p.deallocate() } }
                                                            for buf in eventBuffers {
                                                                let p = UnsafeMutablePointer<CChar>.allocate(capacity: max(1, buf.count))
                                                                if !buf.isEmpty { buf.withUnsafeBufferPointer { p.initialize(from: $0.baseAddress!, count: buf.count) } }
                                                                pinnedPtrs.append(p)
                                                                cEvs.append(coax_string(data: p, count: Int64(buf.count)))
                                                            }
                                                            return withUnsafeMutablePointer(to: &req) { rp in
                                                                cEvs.withUnsafeMutableBufferPointer { evbp in
                                                                    out.withUnsafeMutableBufferPointer { bp in
                                                                        cc_sync_write(
                                                                            coax_v_t_CcSyncReq(data: rp, count: 1),
                                                                            coax_v_string(data: evbp.baseAddress, count: Int64(evbp.count)),
                                                                            coax_v_u8(data: bp.baseAddress, count: Int64(bp.count))
                                                                        )
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        guard n > 0 else { return nil }
        return String(decoding: out[0..<Int(n)], as: UTF8.self)
    }

    static func encodeMeta(operationID: String, title: String? = nil, pinned: Bool? = nil, archived: Bool? = nil) -> String? {
        guard title != nil || pinned != nil || archived != nil else { return nil }
        let sOp = sanitize(operationID, maxBytes: 80)
        let sTitle = sanitize(title ?? "", maxBytes: 160)
        var out = [UInt8](repeating: 0, count: 4096)
        let n: Int64 = withCStr(sOp) { cOp in
            withCStr(sTitle) { cTitle in
                var req = coax_t_CcMetaReq(
                    operation: cOp,
                    has_title: title != nil, title: cTitle,
                    has_pinned: pinned != nil, pinned: pinned ?? false,
                    has_archived: archived != nil, archived: archived ?? false
                )
                return withUnsafeMutablePointer(to: &req) { rp in
                    out.withUnsafeMutableBufferPointer { bp in
                        cc_meta_write(
                            coax_v_t_CcMetaReq(data: rp, count: 1),
                            coax_v_u8(data: bp.baseAddress, count: Int64(bp.count))
                        )
                    }
                }
            }
        }
        guard n > 0 else { return nil }
        return String(decoding: out[0..<Int(n)], as: UTF8.self)
    }

    // MARK: - Outbox Persistence & Flush

    var pendingOutboxCount: Int {
        lock.lock(); defer { lock.unlock() }
        return outbox.count
    }

    var currentCursor: Int64 {
        lock.lock(); defer { lock.unlock() }
        return sseCursor
    }

    func localChatID(for conversationID: String) -> String {
        lock.lock(); defer { lock.unlock() }
        return convToLocal[conversationID] ?? conversationID
    }

    private func rememberMapping(convID: String, localID: String) {
        lock.lock()
        let changed = convToLocal[convID] != localID
        if changed { convToLocal[convID] = localID }
        let snapshot = convToLocal
        lock.unlock()
        if changed, let data = try? JSONEncoder().encode(snapshot) {
            try? data.write(to: mapURL, options: .atomic)
        }
    }

    private func loadState() {
        if let data = try? Data(contentsOf: outboxURL),
           let items = try? JSONDecoder().decode([CloudOutboxItem].self, from: data) {
            outbox = items
        }
        if let data = try? Data(contentsOf: mapURL),
           let map = try? JSONDecoder().decode([String: String].self, from: data) {
            convToLocal = map
        }
        if let s = try? String(contentsOf: cursorURL, encoding: .utf8),
           let c = Int64(s.trimmingCharacters(in: .whitespacesAndNewlines)), c >= 0 {
            sseCursor = c
        }
    }

    private func saveOutboxLocked() {
        if let data = try? JSONEncoder().encode(outbox) {
            try? data.write(to: outboxURL, options: .atomic)
        }
    }

    private func saveCursorLocked() {
        try? Data("\(sseCursor)\n".utf8).write(to: cursorURL, options: .atomic)
    }

    func enqueue(operationID: String, path: String, body: String) {
        lock.lock()
        if !outbox.contains(where: { $0.operationID == operationID }) {
            outbox.append(CloudOutboxItem(operationID: operationID, path: path, body: body, attempts: 0, nextRetryMs: 0))
            saveOutboxLocked()
        }
        lock.unlock()
        Task.detached { [weak self] in
            await self?.flushOutbox(ignoreBackoff: false)
        }
    }

    @discardableResult
    func flushOutbox(ignoreBackoff: Bool = true) async -> Int {
        guard isSignedIn() else { return 0 }
        let acquired: Bool = lock.withLock {
            if flushing { return false }
            flushing = true
            return true
        }
        guard acquired else { return 0 }
        defer { lock.withLock { flushing = false } }

        var delivered = 0
        while true {
            let nowMs = Int64(Date().timeIntervalSince1970 * 1000)
            let nextItem: CloudOutboxItem? = lock.withLock { outbox.first }
            guard let item = nextItem else { break }
            if !ignoreBackoff && item.nextRetryMs > nowMs { break }

            guard let url = URL(string: originProvider() + item.path) else {
                dequeueOutbox(operationID: item.operationID)
                continue
            }
            var req = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 20)
            req.httpMethod = "POST"
            req.setValue("application/json", forHTTPHeaderField: "Content-Type")
            req.httpBody = Data(item.body.utf8)

            do {
                let authed = try await authorizer(req)
                let (_, status) = try await transport(authed)
                if status == 200 || status == 202 {
                    dequeueOutbox(operationID: item.operationID)
                    delivered += 1
                } else if status == 400 || status == 404 || status == 409 || status == 422 {
                    // Permanent client error: drop so later ops are never head-of-line blocked.
                    dequeueOutbox(operationID: item.operationID)
                } else {
                    bumpRetry(operationID: item.operationID, nowMs: nowMs)
                    break
                }
            } catch {
                bumpRetry(operationID: item.operationID, nowMs: nowMs)
                break
            }
        }
        return delivered
    }

    private func dequeueOutbox(operationID: String) {
        lock.lock()
        outbox.removeAll { $0.operationID == operationID }
        saveOutboxLocked()
        lock.unlock()
    }

    private func bumpRetry(operationID: String, nowMs: Int64) {
        lock.lock()
        if let idx = outbox.firstIndex(where: { $0.operationID == operationID }) {
            outbox[idx].attempts += 1
            let shift = min(outbox[idx].attempts, 6)
            let delayMs = min(Int64(30_000), Int64(500) << shift)
            outbox[idx].nextRetryMs = nowMs + delayMs
            saveOutboxLocked()
        }
        lock.unlock()
    }

    // MARK: - Live oa-worker Line Observer & Snapshot Sync

    /// Translates one oa-worker JSON line into an #authority openagents.chat.event.v2 + sync envelope and queues it.
    func observeWorkerLine(
        chatID: String,
        title: String,
        folder: String,
        model: String,
        isLocal: Bool,
        line: String
    ) {
        guard !chatID.isEmpty,
              let data = line.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let kindStr = obj["kind"] as? String else { return }

        let convID = Self.conversationID(for: chatID)
        rememberMapping(convID: convID, localID: chatID)
        let effModel = (obj["model"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? (model.isEmpty ? "google/gemini-4-neon" : model)
        let obsCost = (obj["cost_msat"] as? NSNumber)?.int64Value ?? 0
        let prov = Self.classifyModel(effModel, isLocalHint: isLocal, observedCostMsat: obsCost)
        let nowMs = Int64(Date().timeIntervalSince1970 * 1000)

        lock.lock()
        var st = activeTurns[convID] ?? ActiveTurnState()
        if kindStr == "turn" || st.turnID.isEmpty {
            if kindStr == "turn" && !st.turnID.isEmpty {
                st.ordinal += 1
                st.sequence = 0
                st.output = ""
                st.userText = ""
            }
            st.turnID = Self.turnID(for: convID, ordinal: st.ordinal)
        }
        st.sequence += 1
        let seq = st.sequence
        let tid = st.turnID
        var evKind: Int64 = Self.evTextDelta
        var turnState: Int64 = Self.stateRunning
        let text = (obj["text"] as? String) ?? ""
        let toolID = (obj["id"] as? String) ?? ""
        var toolName = (obj["tool"] as? String) ?? (obj["name"] as? String) ?? ""
        var toolArgs = (obj["args"] as? String) ?? ""
        let diff = (obj["diff"] as? String) ?? ""
        let failed = (obj["failed"] as? Bool) ?? false
        var approvalStatus = ""
        var stopReason = ""
        var errText = ""
        var usage: (Int64, Int64, Int64, Int64, Int64)? = nil

        switch kindStr {
        case "turn":
            evKind = Self.evStarted
            if let ask = obj["ask"] as? String, !ask.isEmpty { st.userText = ask }
        case "thought":
            evKind = Self.evThinkingDelta
        case "delta", "reply":
            evKind = Self.evTextDelta
            if !text.isEmpty { st.output += text }
        case "command":
            if toolName.isEmpty { toolName = "command" }
            if (obj["done"] as? Bool) == true {
                evKind = Self.evToolResult
            } else if (obj["progress"] as? Bool) == true {
                evKind = Self.evToolProgress
            } else {
                evKind = Self.evToolCall
            }
            if toolArgs.isEmpty, let cmd = obj["command"] as? String { toolArgs = cmd }
        case "ask":
            evKind = Self.evApproval
            approvalStatus = "requested"
        case "steered":
            evKind = Self.evApproval
            approvalStatus = "steered"
        case "ended":
            let ending = (obj["ending"] as? String) ?? "done"
            if ending == "stopped" || ending == "cancelled" {
                evKind = Self.evCancelled
                turnState = Self.stateCancelled
                stopReason = "cancelled"
            } else if ending == "failed" || ending == "error" {
                evKind = Self.evFailed
                turnState = Self.stateFailed
                stopReason = "error"
                errText = text
            } else {
                evKind = Self.evCompleted
                turnState = Self.stateCompleted
                stopReason = "end_turn"
            }
            if let u = obj["usage"] as? [String: Any] {
                usage = (
                    (u["input_tokens"] as? NSNumber)?.int64Value ?? -1,
                    (u["output_tokens"] as? NSNumber)?.int64Value ?? -1,
                    (u["cache_read_tokens"] as? NSNumber)?.int64Value ?? -1,
                    (u["cache_write_tokens"] as? NSNumber)?.int64Value ?? -1,
                    (u["reasoning_tokens"] as? NSNumber)?.int64Value ?? -1
                )
            }
        default:
            lock.unlock()
            return
        }

        let userText = st.userText
        let outText = st.output
        activeTurns[convID] = st
        lock.unlock()

        guard let evJSON = Self.encodeEvent(
            conversationID: convID, turnID: tid, sequence: seq,
            kind: evKind, state: turnState, text: text,
            toolID: toolID, toolName: toolName, toolArgs: toolArgs,
            diff: diff, failed: failed, approvalStatus: approvalStatus,
            compute: prov.compute, model: effModel, costMsat: prov.costMsat,
            sourceDevice: sourceDevice, workspace: folder,
            usage: usage, stopReason: stopReason, createdAtMs: nowMs
        ) else { return }

        let opID = "ev:\(tid):\(seq)"
        guard let syncBody = Self.encodeSync(
            operationID: opID, conversationID: convID, turnID: tid, title: title,
            compute: prov.compute, model: effModel, costMsat: prov.costMsat,
            sourceDevice: sourceDevice, workspace: folder,
            userText: userText, state: turnState, output: outText,
            stopReason: stopReason, error: errText, usage: usage,
            eventsJSON: [evJSON]
        ) else { return }

        enqueue(operationID: opID, path: "/api/chat/v2/sync", body: syncBody)
    }

    /// Write-through sync of a saved Desktop chat snapshot (called from keep()).
    func syncSavedChat(
        chatID: String,
        title: String,
        folder: String,
        chatJSON: String,
        pinned: Bool,
        archived: Bool
    ) {
        guard !chatID.isEmpty,
              let data = chatJSON.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return }

        let convID = Self.conversationID(for: chatID)
        rememberMapping(convID: convID, localID: chatID)

        let model = (obj["model"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? "google/gemini-4-neon"
        let prov = Self.classifyModel(model, isLocalHint: false, observedCostMsat: 0)
        let turns = (obj["turns"] as? [[String: Any]]) ?? []
        let ordinal = max(1, turns.count)
        let tid = Self.turnID(for: convID, ordinal: ordinal)
        let lastTurn = turns.last
        let ask = (lastTurn?["ask"] as? String) ?? ""
        let reply = (lastTurn?["reply"] as? String) ?? ""

        let digestInput = "\(convID)|\(tid)|\(title)|\(pinned)|\(archived)|\(model)|\(ask)|\(reply)"
        let digest = SHA256.hash(data: Data(digestInput.utf8)).prefix(12).map { String(format: "%02x", $0) }.joined()

        lock.lock()
        if lastSyncedDigest[convID] == digest {
            lock.unlock()
            return
        }
        lastSyncedDigest[convID] = digest
        lock.unlock()

        let opID = "snap:\(convID.dropFirst(4).prefix(12)):\(ordinal):\(digest)"
        let turnState: Int64 = reply.isEmpty ? Self.stateRunning : Self.stateCompleted
        guard let syncBody = Self.encodeSync(
            operationID: opID, conversationID: convID, turnID: tid, title: title,
            pinned: pinned, archived: archived,
            compute: prov.compute, model: model, costMsat: prov.costMsat,
            sourceDevice: sourceDevice, workspace: folder,
            userText: ask, state: turnState, output: reply,
            stopReason: reply.isEmpty ? "" : "end_turn"
        ) else { return }

        enqueue(operationID: opID, path: "/api/chat/v2/sync", body: syncBody)
    }

    /// Write-through sync of rename / pin / archive sidebar actions.
    func syncMeta(chatID: String, title: String? = nil, pinned: Bool? = nil, archived: Bool? = nil) {
        guard !chatID.isEmpty, title != nil || pinned != nil || archived != nil else { return }
        let convID = Self.conversationID(for: chatID)
        rememberMapping(convID: convID, localID: chatID)
        let opID = "meta:\(UUID().uuidString.lowercased().replacingOccurrences(of: "-", with: "").prefix(24))"
        guard let body = Self.encodeMeta(operationID: opID, title: title, pinned: pinned, archived: archived) else { return }
        enqueue(operationID: opID, path: "/api/chat/v2/conversations/\(convID)/meta", body: body)
    }

    // MARK: - Live Account SSE Stream (Web/Mobile -> Desktop)

    func startLiveStream() {
        lock.lock()
        if sseTask != nil { lock.unlock(); return }
        sseTask = Task.detached { [weak self] in
            await self?.runSSELoop()
        }
        lock.unlock()
    }

    func stopLiveStream() {
        lock.lock()
        sseTask?.cancel()
        sseTask = nil
        lock.unlock()
    }

    private func runSSELoop() async {
        var backoffMs: UInt64 = 500
        while !Task.isCancelled {
            guard isSignedIn() else {
                try? await Task.sleep(nanoseconds: 2_000_000_000)
                continue
            }
            _ = await flushOutbox(ignoreBackoff: false)
            let cursor = currentCursor
            guard let url = URL(string: originProvider() + "/api/chat/v2/stream?after=\(cursor)") else {
                try? await Task.sleep(nanoseconds: 2_000_000_000)
                continue
            }
            var req = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 1860)
            req.httpMethod = "GET"
            req.setValue("text/event-stream", forHTTPHeaderField: "Accept")
            if cursor > 0 { req.setValue("\(cursor)", forHTTPHeaderField: "Last-Event-ID") }

            do {
                let authed = try await authorizer(req)
                let (bytes, resp) = try await URLSession.shared.bytes(for: authed)
                guard let http = resp as? HTTPURLResponse, http.statusCode == 200 else {
                    try? await Task.sleep(nanoseconds: backoffMs * 1_000_000)
                    backoffMs = min(backoffMs * 2, 15_000)
                    continue
                }
                backoffMs = 500
                var evID: Int64 = -1
                var evName = ""
                var dataLines: [String] = []
                for try await line in bytes.lines {
                    if Task.isCancelled { break }
                    if line.isEmpty {
                        if !dataLines.isEmpty {
                            let payload = dataLines.joined(separator: "\n")
                            await handleSSEEvent(id: evID, event: evName, data: payload)
                        }
                        evID = -1; evName = ""; dataLines = []
                    } else if line.hasPrefix(":") {
                        continue
                    } else if line.hasPrefix("id:") {
                        let raw = line.dropFirst(3).trimmingCharacters(in: .whitespaces)
                        if let parsed = Int64(raw) { evID = parsed }
                    } else if line.hasPrefix("event:") {
                        evName = line.dropFirst(6).trimmingCharacters(in: .whitespaces)
                    } else if line.hasPrefix("data:") {
                        let d = line.hasPrefix("data: ") ? String(line.dropFirst(6)) : String(line.dropFirst(5))
                        dataLines.append(d)
                    }
                }
            } catch {
                if Task.isCancelled { break }
                try? await Task.sleep(nanoseconds: backoffMs * 1_000_000)
                backoffMs = min(backoffMs * 2, 15_000)
            }
        }
    }

    /// Processes a single parsed SSE frame from GET /api/chat/v2/stream (also callable directly from tests).
    func handleSSEEvent(id: Int64, event: String, data: String) async {
        if id > 0 {
            lock.withLock {
                if id > sseCursor {
                    sseCursor = id
                    saveCursorLocked()
                }
            }
        }
        guard let raw = data.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: raw) as? [String: Any] else { return }
        let convID = (obj["conversation_id"] as? String) ?? ""
        let src = (obj["source_device"] as? String) ?? ""
        guard Self.isCanonicalConvID(convID) else { return }
        // Ignore our own echoes if we already have this device's write locally, unless remote metadata changed.
        if src == sourceDevice && event == "conversation_created" { return }
        await pullConversation(convID)
    }

    /// Fetches GET /api/chat/v2/conversations/:id and notifies onRemoteConversation to update Desktop's CoderStore.
    @discardableResult
    func pullConversation(_ convID: String) async -> RemoteCloudConversation? {
        guard Self.isCanonicalConvID(convID),
              let url = URL(string: originProvider() + "/api/chat/v2/conversations/\(convID)") else { return nil }
        var req = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 20)
        req.httpMethod = "GET"
        do {
            let authed = try await authorizer(req)
            let (data, status) = try await transport(authed)
            guard status == 200,
                  let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let conv = root["conversation"] as? [String: Any] else { return nil }
            let title = (conv["title"] as? String) ?? "New chat"
            let pinned = (conv["pinned"] as? Bool) ?? false
            let archived = (conv["archived"] as? Bool) ?? false
            let workspace = (conv["workspace"] as? String) ?? ""
            let model = (conv["model"] as? String) ?? "google/gemini-4-neon"
            let rawTurns = (root["turns"] as? [[String: Any]]) ?? []
            var desktopTurns: [[String: Any]] = []
            for t in rawTurns {
                let ask = (t["user_text"] as? String) ?? ""
                let reply = (t["output"] as? String) ?? ""
                desktopTurns.append(["ask": ask, "reply": reply, "cmds": []])
            }
            let chatObj: [String: Any] = ["model": model, "turns": desktopTurns]
            guard let chatBytes = try? JSONSerialization.data(withJSONObject: chatObj),
                  let chatJSON = String(data: chatBytes, encoding: .utf8) else { return nil }
            let localID = localChatID(for: convID)
            let remote = RemoteCloudConversation(
                conversationID: convID,
                localChatID: localID,
                title: title,
                pinned: pinned,
                archived: archived,
                workspace: workspace,
                chatJSON: chatJSON
            )
            onRemoteConversation?(remote)
            return remote
        } catch {
            return nil
        }
    }
}
