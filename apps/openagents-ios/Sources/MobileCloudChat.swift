// Firm reason: 6 (iOS Cloud Chat client + cross-device sync over #authority account-chat-cloud-wire.coax, /api/chat/complete/v1, and /api/chat/v2/*).
import CryptoKit
import Foundation

struct MobileCloudConversation: Identifiable, Equatable {
    let id: String // cnv_<32 hex>
    var title: String
    var model: String
    var workspace: String
    var updatedAtMs: Int64
    var pinned: Bool
    var archived: Bool
    var turns: [MobileCloudTurn]
}

struct MobileCloudTurn: Equatable {
    let id: String
    let userText: String
    var output: String
    var complete: Bool
}

@MainActor
final class MobileCloudChat: ObservableObject {
    static let shared = MobileCloudChat()
    static let cloudMachineID = "openagents-cloud"

    // Event kind & state constants matching account-chat-cloud-wire.coax.
    private static let evCompleted: Int64 = 10
    private static let stateCompleted: Int64 = 3

    private let auth: AppAuth
    private var sseTask: Task<Void, Never>?
    private var sseCursor: Int64 = 0

    @Published var conversations: [MobileCloudConversation] = []
    @Published var activeConversationID: String?
    @Published var isStreamConnected: Bool = false
    @Published var isSending: Bool = false
    @Published var lastError: String?

    var onConversationsUpdated: (() -> Void)?

    init(auth: AppAuth = AppAuth.shared) {
        self.auth = auth
    }

    var isSignedIn: Bool { auth.loggedIn }

    var activeConversation: MobileCloudConversation? {
        guard let id = activeConversationID else { return nil }
        return conversations.first { $0.id == id }
    }

    static func newConversationID() -> String {
        var bytes = [UInt8](repeating: 0, count: 16)
        _ = SecRandomCopyBytes(kSecRandomDefault, 16, &bytes)
        return "cnv_" + bytes.map { String(format: "%02x", $0) }.joined()
    }

    static func turnID(conversationID: String, ordinal: Int) -> String {
        let hash = SHA256.hash(data: Data(("openagents.ios.turn:\(conversationID):\(ordinal)").utf8))
        return "trn_" + hash.prefix(16).map { String(format: "%02x", $0) }.joined()
    }

    func start() {
        guard auth.loggedIn else {
            stop()
            return
        }
        Task { await refreshConversations() }
        guard sseTask == nil else { return }
        sseTask = Task { [weak self] in
            await self?.runSSELoop()
        }
    }

    func stop() {
        sseTask?.cancel()
        sseTask = nil
        isStreamConnected = false
    }

    func startNewCloudChat() {
        activeConversationID = nil
        lastError = nil
        onConversationsUpdated?()
    }

    func selectConversation(_ id: String) {
        activeConversationID = id
        Task { await pullConversation(id) }
        onConversationsUpdated?()
    }

    func refreshConversations() async {
        guard auth.loggedIn,
              let url = URL(string: auth.origin + "/api/chat/v2/conversations") else { return }
        var req = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 20)
        req.httpMethod = "GET"
        do {
            let authed = try await auth.authorize(req)
            let (data, resp) = try await URLSession.shared.data(for: authed)
            guard (resp as? HTTPURLResponse)?.statusCode == 200,
                  let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let list = root["conversations"] as? [[String: Any]] else { return }
            let existingByID = Dictionary(uniqueKeysWithValues: conversations.map { ($0.id, $0) })
            var next: [MobileCloudConversation] = []
            for row in list {
                guard let id = row["id"] as? String, !id.isEmpty else { continue }
                let prev = existingByID[id]
                let title = (row["title"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? prev?.title ?? "Cloud chat"
                let model = (row["model"] as? String) ?? prev?.model ?? "google/gemini-4-hydrogen"
                let workspace = (row["workspace"] as? String) ?? prev?.workspace ?? ""
                let updatedMs = (row["updated_at_ms"] as? NSNumber)?.int64Value ?? prev?.updatedAtMs ?? Int64(Date().timeIntervalSince1970 * 1000)
                let pinned = (row["pinned"] as? Bool) ?? prev?.pinned ?? false
                let archived = (row["archived"] as? Bool) ?? prev?.archived ?? false
                next.append(MobileCloudConversation(
                    id: id,
                    title: title,
                    model: model,
                    workspace: workspace,
                    updatedAtMs: updatedMs,
                    pinned: pinned,
                    archived: archived,
                    turns: prev?.turns ?? []
                ))
            }
            conversations = next.sorted { $0.updatedAtMs > $1.updatedAtMs }
            onConversationsUpdated?()
        } catch {
            // Keep cached conversations on transient network errors.
        }
    }

    @discardableResult
    func pullConversation(_ convID: String) async -> MobileCloudConversation? {
        guard auth.loggedIn,
              let url = URL(string: auth.origin + "/api/chat/v2/conversations/\(convID)") else { return nil }
        var req = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 20)
        req.httpMethod = "GET"
        do {
            let authed = try await auth.authorize(req)
            let (data, resp) = try await URLSession.shared.data(for: authed)
            guard (resp as? HTTPURLResponse)?.statusCode == 200,
                  let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let conv = root["conversation"] as? [String: Any] else { return nil }
            let title = (conv["title"] as? String) ?? "Cloud chat"
            let model = (conv["model"] as? String) ?? "google/gemini-4-hydrogen"
            let workspace = (conv["workspace"] as? String) ?? ""
            let pinned = (conv["pinned"] as? Bool) ?? false
            let archived = (conv["archived"] as? Bool) ?? false
            let updatedMs = (conv["updated_at_ms"] as? NSNumber)?.int64Value ?? Int64(Date().timeIntervalSince1970 * 1000)
            let rawTurns = (root["turns"] as? [[String: Any]]) ?? []
            let turns: [MobileCloudTurn] = rawTurns.enumerated().map { idx, t in
                MobileCloudTurn(
                    id: (t["id"] as? String) ?? Self.turnID(conversationID: convID, ordinal: idx + 1),
                    userText: (t["user_text"] as? String) ?? "",
                    output: (t["output"] as? String) ?? "",
                    complete: true
                )
            }
            let updated = MobileCloudConversation(
                id: convID,
                title: title,
                model: model,
                workspace: workspace,
                updatedAtMs: updatedMs,
                pinned: pinned,
                archived: archived,
                turns: turns
            )
            if let idx = conversations.firstIndex(where: { $0.id == convID }) {
                conversations[idx] = updated
            } else {
                conversations.insert(updated, at: 0)
            }
            onConversationsUpdated?()
            return updated
        } catch {
            return nil
        }
    }

    /// Sends a standalone Cloud Chat turn via POST /api/chat/complete/v1 and syncs it to /api/chat/v2/sync
    /// so it appears immediately on Desktop and Web.
    func sendCloudMessage(_ text: String, model: String = "google/gemini-4-hydrogen") async {
        let prompt = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !prompt.isEmpty else { return }
        guard auth.loggedIn else {
            lastError = "Sign in to your OpenAgents account to use Cloud Chat."
            return
        }

        let convID = activeConversationID ?? Self.newConversationID()
        activeConversationID = convID
        let nowMs = Int64(Date().timeIntervalSince1970 * 1000)

        var conv: MobileCloudConversation
        if let existing = conversations.first(where: { $0.id == convID }) {
            conv = existing
        } else {
            let shortTitle = String(prompt.prefix(64))
            conv = MobileCloudConversation(
                id: convID,
                title: shortTitle,
                model: model,
                workspace: "",
                updatedAtMs: nowMs,
                pinned: false,
                archived: false,
                turns: []
            )
            conversations.insert(conv, at: 0)
        }

        let ordinal = conv.turns.count + 1
        let tid = Self.turnID(conversationID: convID, ordinal: ordinal)
        conv.turns.append(MobileCloudTurn(id: tid, userText: prompt, output: "", complete: false))
        conv.updatedAtMs = nowMs
        upsertConversation(conv)
        isSending = true
        lastError = nil
        onConversationsUpdated?()

        defer {
            isSending = false
            onConversationsUpdated?()
        }

        // Build OpenAI-compatible inference payload for POST /api/v1/inference.
        var messages: [[String: String]] = []
        for t in conv.turns {
            if !t.userText.isEmpty {
                messages.append(["role": "user", "content": t.userText])
            }
            if !t.output.isEmpty {
                messages.append(["role": "assistant", "content": t.output])
            }
        }
        let bodyObj: [String: Any] = [
            "model": model,
            "messages": messages,
            "stream": false,
        ]
        guard let url = URL(string: auth.origin + "/api/v1/inference"),
              let bodyData = try? JSONSerialization.data(withJSONObject: bodyObj) else {
            lastError = "Could not encode Cloud Chat request."
            return
        }

        var req = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 60)
        req.httpMethod = "POST"
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = bodyData

        do {
            let authed = try await auth.authorize(req)
            let (data, resp) = try await URLSession.shared.data(for: authed)
            let status = (resp as? HTTPURLResponse)?.statusCode ?? 0
            if status != 200 {
                let errMsg = (try? JSONSerialization.jsonObject(with: data) as? [String: Any])?["error"] as? String
                lastError = errMsg ?? "Cloud Chat failed (HTTP \(status))."
                markLastTurnComplete(convID: convID, output: lastError ?? "Cloud request failed.")
                return
            }
            let reply = extractCompletionReply(data) ?? ""
            markLastTurnComplete(convID: convID, output: reply)
            await syncTurnToCloudV2(
                conversationID: convID,
                turnID: tid,
                title: conv.title,
                model: model,
                userText: prompt,
                output: reply
            )
        } catch {
            lastError = AppAccount.words(error)
            markLastTurnComplete(convID: convID, output: lastError ?? "Cloud request failed.")
        }
    }

    private func extractCompletionReply(_ data: Data) -> String? {
        guard let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
        if let choices = obj["choices"] as? [[String: Any]],
           let first = choices.first,
           let msg = first["message"] as? [String: Any],
           let content = msg["content"] as? String {
            return content
        }
        if let text = obj["content"] as? String { return text }
        if let text = obj["reply"] as? String { return text }
        return nil
    }

    private func markLastTurnComplete(convID: String, output: String) {
        guard let idx = conversations.firstIndex(where: { $0.id == convID }),
              !conversations[idx].turns.isEmpty else { return }
        let lastIdx = conversations[idx].turns.count - 1
        conversations[idx].turns[lastIdx].output = output
        conversations[idx].turns[lastIdx].complete = true
        conversations[idx].updatedAtMs = Int64(Date().timeIntervalSince1970 * 1000)
        onConversationsUpdated?()
    }

    private func upsertConversation(_ conv: MobileCloudConversation) {
        if let idx = conversations.firstIndex(where: { $0.id == conv.id }) {
            conversations[idx] = conv
        } else {
            conversations.insert(conv, at: 0)
        }
    }

    private func syncTurnToCloudV2(
        conversationID: String,
        turnID: String,
        title: String,
        model: String,
        userText: String,
        output: String
    ) async {
        guard auth.loggedIn,
              let url = URL(string: auth.origin + "/api/chat/v2/sync") else { return }
        let nowMs = Int64(Date().timeIntervalSince1970 * 1000)
        var evBuf = [UInt8](repeating: 0, count: 65536)
        let evLen = oa_cc_event_write(
            conversationID, turnID, 1,
            Self.evCompleted, Self.stateCompleted, output,
            "cloud", model, 0,
            "ios-mobile", "",
            "end_turn", nowMs,
            &evBuf, Int64(evBuf.count)
        )
        let evJSON = evLen > 0 ? String(decoding: evBuf[0..<Int(evLen)], as: UTF8.self) : ""
        let opID = "ios:\(turnID):1"
        var syncBuf = [UInt8](repeating: 0, count: 131072)
        let syncLen = oa_cc_sync_write(
            opID, conversationID, turnID,
            title, false, false, false, false,
            "cloud", model, 0,
            "ios-mobile", "",
            userText, Self.stateCompleted, output,
            "end_turn", "",
            evJSON,
            &syncBuf, Int64(syncBuf.count)
        )
        guard syncLen > 0 else { return }
        var req = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 20)
        req.httpMethod = "POST"
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = Data(syncBuf[0..<Int(syncLen)])
        if let authed = try? await auth.authorize(req) {
            _ = try? await URLSession.shared.data(for: authed)
        }
    }

    private func runSSELoop() async {
        var backoffMs: UInt64 = 1000
        while !Task.isCancelled {
            guard auth.loggedIn,
                  let url = URL(string: auth.origin + "/api/chat/v2/stream?after=\(sseCursor)") else {
                isStreamConnected = false
                try? await Task.sleep(nanoseconds: 2_000_000_000)
                continue
            }
            var req = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 1860)
            req.httpMethod = "GET"
            req.setValue("text/event-stream", forHTTPHeaderField: "Accept")
            if sseCursor > 0 { req.setValue("\(sseCursor)", forHTTPHeaderField: "Last-Event-ID") }

            do {
                let authed = try await auth.authorize(req)
                let (bytes, resp) = try await URLSession.shared.bytes(for: authed)
                guard (resp as? HTTPURLResponse)?.statusCode == 200 else {
                    isStreamConnected = false
                    try? await Task.sleep(nanoseconds: backoffMs * 1_000_000)
                    backoffMs = min(backoffMs * 2, 15_000)
                    continue
                }
                isStreamConnected = true
                backoffMs = 1000
                var evID: Int64 = -1
                var dataLines: [String] = []
                for try await line in bytes.lines {
                    if Task.isCancelled { break }
                    if line.isEmpty {
                        if !dataLines.isEmpty {
                            let payload = dataLines.joined(separator: "\n")
                            await handleSSEPayload(id: evID, payload: payload)
                        }
                        evID = -1
                        dataLines = []
                    } else if line.hasPrefix("id:") {
                        if let parsed = Int64(line.dropFirst(3).trimmingCharacters(in: .whitespaces)) {
                            evID = parsed
                        }
                    } else if line.hasPrefix("data:") {
                        let d = line.hasPrefix("data: ") ? String(line.dropFirst(6)) : String(line.dropFirst(5))
                        dataLines.append(d)
                    }
                }
            } catch {
                if Task.isCancelled { break }
                isStreamConnected = false
                try? await Task.sleep(nanoseconds: backoffMs * 1_000_000)
                backoffMs = min(backoffMs * 2, 15_000)
            }
        }
        isStreamConnected = false
    }

    private func handleSSEPayload(id: Int64, payload: String) async {
        if id > sseCursor { sseCursor = id }
        guard let raw = payload.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: raw) as? [String: Any],
              let convID = obj["conversation_id"] as? String, !convID.isEmpty else { return }
        await pullConversation(convID)
    }

    func transcriptItems(for conv: MobileCloudConversation) -> [TranscriptItem] {
        var out: [TranscriptItem] = []
        for turn in conv.turns {
            if !turn.userText.isEmpty {
                out.append(TranscriptItem(
                    id: "\(turn.id)-u",
                    role: .user,
                    text: turn.userText,
                    toolName: "",
                    complete: true,
                    startedAtMs: conv.updatedAtMs,
                    subagentCalls: 0,
                    subagentNow: "",
                    path: "",
                    diff: "",
                    diffCut: false,
                    noticeKind: ""
                ))
            }
            if !turn.output.isEmpty || !turn.complete {
                out.append(TranscriptItem(
                    id: "\(turn.id)-a",
                    role: .assistant,
                    text: turn.output.isEmpty ? "Thinking…" : turn.output,
                    toolName: "",
                    complete: turn.complete,
                    startedAtMs: conv.updatedAtMs,
                    subagentCalls: 0,
                    subagentNow: "",
                    path: "",
                    diff: "",
                    diffCut: false,
                    noticeKind: ""
                ))
            }
        }
        return out
    }
}
