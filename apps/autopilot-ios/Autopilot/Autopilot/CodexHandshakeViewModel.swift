import Foundation
import Combine
import SwiftUI

@MainActor
final class CodexHandshakeViewModel: ObservableObject {
    @Published var email: String
    @Published var verificationCode: String = ""
    @Published private(set) var authState: AuthState
    @Published private(set) var isAuthenticated: Bool

    @Published var workers: [RuntimeCodexWorkerSummary] = []
    @Published var selectedWorkerID: String? {
        didSet {
            storeSelection()
            restartStreamIfReady(resetCursor: true)
        }
    }

    @Published var streamState: StreamState = .idle
    @Published var handshakeState: HandshakeState = .idle
    @Published var statusMessage: String?
    @Published var errorMessage: String?
    @Published var latestSnapshot: RuntimeCodexWorkerSnapshot?
    @Published var recentEvents: [RuntimeCodexStreamEvent] = []
    @Published var chatMessages: [CodexChatMessage] = []

    private var streamTask: Task<Void, Never>?
    private var handshakeTimeoutTask: Task<Void, Never>?
    private var streamCursor: Int = 0
    private var assistantMessageIndexByItemKey: [String: Int] = [:]
    private var reasoningMessageIndexByItemKey: [String: Int] = [:]
    private var toolMessageIndexByItemKey: [String: Int] = [:]
    private var seenUserMessageKeys: Set<String> = []
    private var agentDeltaAliasSources: [String: AgentDeltaSource] = [:]
    private var processedCodexEventSeqs: Set<Int> = []
    private var processedCodexEventSeqOrder: [Int] = []
    private var hasAttemptedAutoConnect = false

    private let defaults: UserDefaults
    private let now: () -> Date

    private let tokenKey = "autopilot.ios.codex.authToken"
    private let emailKey = "autopilot.ios.codex.email"
    private let selectedWorkerIDKey = "autopilot.ios.codex.selectedWorkerID"
    private let deviceIDKey = "autopilot.ios.codex.deviceID"

    private static let defaultBaseURL = URL(string: "https://openagents.com")!
    private static let streamTailMS = 4_000
    private static let streamIdleSleepNS: UInt64 = 250_000_000
    private static let streamReconnectSleepNS: UInt64 = 1_500_000_000
    private static let aliasCacheLimit = 2_048
    private static let seqCacheLimit = 8_192
    private static let iso8601Parsers: [ISO8601DateFormatter] = {
        let withFractional = ISO8601DateFormatter()
        withFractional.formatOptions = [.withInternetDateTime, .withFractionalSeconds]

        let internetDateTime = ISO8601DateFormatter()
        internetDateTime.formatOptions = [.withInternetDateTime]

        return [withFractional, internetDateTime]
    }()

    private var authToken: String {
        didSet {
            isAuthenticated = !authToken.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        }
    }

    private enum AgentDeltaSource {
        case modern
        case legacy
    }

    let deviceID: String

    var environmentHost: String {
        Self.defaultBaseURL.host ?? "openagents.com"
    }

    init(defaults: UserDefaults = .standard, now: @escaping () -> Date = Date.init) {
        self.defaults = defaults
        self.now = now

        let savedEmail = defaults.string(forKey: emailKey)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        let savedToken = defaults.string(forKey: tokenKey)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""

        self.email = savedEmail
        self.authToken = savedToken
        self.isAuthenticated = !savedToken.isEmpty
        self.authState = savedToken.isEmpty
            ? .signedOut
            : .authenticated(email: savedEmail.isEmpty ? nil : savedEmail)

        self.selectedWorkerID = defaults.string(forKey: selectedWorkerIDKey)

        if let existingDeviceID = defaults.string(forKey: deviceIDKey), !existingDeviceID.isEmpty {
            self.deviceID = existingDeviceID
        } else {
            let newDeviceID = UUID().uuidString.lowercased()
            defaults.set(newDeviceID, forKey: deviceIDKey)
            self.deviceID = newDeviceID
        }
    }

    deinit {
        streamTask?.cancel()
        handshakeTimeoutTask?.cancel()
    }

    func autoConnectOnLaunch() async {
        guard !hasAttemptedAutoConnect else {
            return
        }
        hasAttemptedAutoConnect = true

        guard isAuthenticated else {
            return
        }

        await refreshWorkers()
    }

    func sendEmailCode() async {
        let normalizedEmail = normalizeEmail(email)
        guard isValidEmail(normalizedEmail) else {
            errorMessage = "Enter a valid email address first."
            return
        }

        authState = .sendingCode
        errorMessage = nil
        statusMessage = "Sending sign-in code..."

        do {
            try await makeAuthClient().sendEmailCode(email: normalizedEmail)
            email = normalizedEmail
            defaults.set(normalizedEmail, forKey: emailKey)
            verificationCode = ""
            authState = .codeSent(email: normalizedEmail)
            statusMessage = "Code sent to \(normalizedEmail)."
        } catch {
            authState = .signedOut
            errorMessage = formatError(error)
            statusMessage = nil
        }
    }

    func verifyEmailCode() async {
        let normalizedCode = verificationCode.replacingOccurrences(of: " ", with: "")
        guard !normalizedCode.isEmpty else {
            errorMessage = "Enter your verification code first."
            return
        }

        authState = .verifying
        errorMessage = nil
        statusMessage = "Verifying code..."

        do {
            let session = try await makeAuthClient().verifyEmailCode(code: normalizedCode)
            authToken = session.token
            defaults.set(session.token, forKey: tokenKey)

            if let sessionEmail = session.email?.trimmingCharacters(in: .whitespacesAndNewlines), !sessionEmail.isEmpty {
                email = sessionEmail
            }
            defaults.set(email, forKey: emailKey)

            verificationCode = ""
            authState = .authenticated(email: email.isEmpty ? nil : email)
            statusMessage = "Signed in to OpenAgents."
            errorMessage = nil

            await refreshWorkers()
        } catch {
            authState = .codeSent(email: email)
            errorMessage = formatError(error)
            statusMessage = nil
        }
    }

    func signOut() {
        streamTask?.cancel()
        streamTask = nil
        handshakeTimeoutTask?.cancel()
        handshakeTimeoutTask = nil

        authToken = ""
        defaults.removeObject(forKey: tokenKey)

        workers = []
        selectedWorkerID = nil
        latestSnapshot = nil
        recentEvents = []
        resetChatTimeline()
        streamState = .idle
        handshakeState = .idle

        authState = .signedOut
        statusMessage = "Signed out."
        errorMessage = nil
    }

    func refreshWorkers() async {
        guard let client = makeClient() else {
            errorMessage = "Sign in first to load workers."
            return
        }

        errorMessage = nil
        statusMessage = "Loading workers..."

        do {
            let result = try await client.listWorkers(status: nil, limit: 100)
            workers = result
            guard let worker = preferredWorker(from: result) else {
                selectedWorkerID = nil
                latestSnapshot = nil
                streamState = .idle
                statusMessage = "No running desktop workers found."
                return
            }

            selectedWorkerID = worker.workerID
            streamCursor = max(0, worker.latestSeq - 1)
            latestSnapshot = try? await client.workerSnapshot(workerID: worker.workerID)
            statusMessage = "Loaded \(result.count) worker(s). Auto-selected \(shortWorkerID(worker.workerID))."
            restartStreamIfReady(resetCursor: true)

            // Keep the iOS flow simple: selecting workers also kicks off a handshake.
            Task { [weak self] in
                await self?.sendHandshake()
            }
        } catch {
            errorMessage = formatError(error)
            statusMessage = nil
        }
    }

    func sendHandshake() async {
        guard isAuthenticated else {
            errorMessage = "Sign in first to send handshake."
            return
        }

        guard let client = makeClient() else {
            errorMessage = "Sign in first to send handshake."
            return
        }

        if workers.isEmpty {
            await refreshWorkers()
        }

        guard let worker = preferredWorker(from: workers) else {
            errorMessage = "No active desktop worker found."
            return
        }

        if selectedWorkerID != worker.workerID {
            selectedWorkerID = worker.workerID
        }

        let workerID = worker.workerID
        if streamState == .idle {
            restartStreamIfReady(resetCursor: false)
        }

        let handshakeID = UUID().uuidString.lowercased()
        handshakeState = .sending
        errorMessage = nil
        statusMessage = "Sending handshake to \(shortWorkerID(workerID))..."

        let payload: [String: JSONValue] = [
            "source": .string("autopilot-ios"),
            "method": .string("ios/handshake"),
            "handshake_id": .string(handshakeID),
            "device_id": .string(deviceID),
            "occurred_at": .string(iso8601(now())),
        ]

        do {
            try await client.ingestWorkerEvent(workerID: workerID, eventType: "worker.event", payload: payload)
            handshakeState = .waitingAck(handshakeID: handshakeID)
            statusMessage = "Handshake sent to \(shortWorkerID(workerID)). Waiting for desktop ack..."
            scheduleHandshakeTimeout(handshakeID: handshakeID, seconds: 30)
        } catch {
            handshakeState = .failed(message: formatError(error))
            errorMessage = formatError(error)
            statusMessage = nil
        }
    }

    func clearMessages() {
        errorMessage = nil
        statusMessage = nil
    }

    func connectStream() {
        if selectedWorkerID == nil, let worker = preferredWorker(from: workers) {
            selectedWorkerID = worker.workerID
        }

        restartStreamIfReady(resetCursor: false)
    }

    func disconnectStream() {
        streamTask?.cancel()
        streamTask = nil
        streamState = .idle
    }

    private func restartStreamIfReady(resetCursor: Bool) {
        streamTask?.cancel()
        streamTask = nil

        guard makeClient() != nil,
              let workerID = selectedWorkerID,
              !workerID.isEmpty else {
            streamState = .idle
            return
        }

        if resetCursor {
            streamCursor = max(0, selectedWorkerLatestSeq(workerID: workerID) - 1)
            recentEvents = []
            resetChatTimeline()
        }

        streamTask = Task { [weak self] in
            await self?.runStreamLoop(workerID: workerID)
        }
    }

    private func runStreamLoop(workerID: String) async {
        guard let client = makeClient() else {
            streamState = .idle
            return
        }

        streamState = .connecting
        statusMessage = "Connecting stream for \(shortWorkerID(workerID)) (long-poll can take a few seconds)..."

        while !Task.isCancelled {
            do {
                let batch = try await client.streamWorker(
                    workerID: workerID,
                    cursor: streamCursor,
                    tailMS: Self.streamTailMS
                )
                if Task.isCancelled {
                    return
                }

                streamState = .live
                streamCursor = batch.nextCursor
                handleIncoming(events: batch.events)

                if batch.events.isEmpty {
                    statusMessage = "Stream live for \(shortWorkerID(workerID)). Waiting for new events..."
                } else if case .waitingAck = handshakeState {
                    // Keep waiting-ack status as-is while the matcher looks for desktop ack.
                } else {
                    statusMessage = "Stream live for \(shortWorkerID(workerID)). Received \(batch.events.count) event(s)."
                }

                if batch.events.isEmpty {
                    try? await Task.sleep(nanoseconds: Self.streamIdleSleepNS)
                }
            } catch {
                if Task.isCancelled {
                    return
                }

                streamState = .reconnecting
                errorMessage = formatError(error)
                statusMessage = "Stream reconnecting for \(shortWorkerID(workerID))..."
                try? await Task.sleep(nanoseconds: Self.streamReconnectSleepNS)
            }
        }
    }

    private func handleIncoming(events: [RuntimeCodexStreamEvent]) {
        guard !events.isEmpty else {
            return
        }

        recentEvents = (events.reversed() + recentEvents).prefix(100).map { $0 }
        for event in events {
            guard let codexEvent = RuntimeCodexProto.decodeCodexEventEnvelope(from: event.payload),
                  codexEvent.source == RuntimeCodexProto.desktopSource,
                  shouldProcessCodexEvent(codexEvent) else {
                continue
            }
            applyCodexEvent(codexEvent)
        }

        let ackHandshakeIDs = Set(events.compactMap { CodexHandshakeMatcher.ackHandshakeID(from: $0) })
        guard !ackHandshakeIDs.isEmpty else {
            return
        }

        switch handshakeState {
        case .waitingAck(let expectedHandshakeID) where ackHandshakeIDs.contains(expectedHandshakeID):
            handshakeTimeoutTask?.cancel()
            handshakeTimeoutTask = nil
            handshakeState = .success(handshakeID: expectedHandshakeID)
            statusMessage = "Handshake succeeded. Desktop ack received."
            errorMessage = nil

        case .timedOut(let expectedHandshakeID) where ackHandshakeIDs.contains(expectedHandshakeID):
            handshakeTimeoutTask?.cancel()
            handshakeTimeoutTask = nil
            handshakeState = .success(handshakeID: expectedHandshakeID)
            statusMessage = "Handshake ack arrived after timeout. Connected."
            errorMessage = nil

        case .success(let handshakeID):
            if errorMessage?.contains("Handshake timed out") == true,
               ackHandshakeIDs.contains(handshakeID) {
                errorMessage = nil
            }

        default:
            break
        }
    }

    private func shouldProcessCodexEvent(_ event: RuntimeCodexProto.CodexEventEnvelope) -> Bool {
        guard let seq = event.seq else {
            return true
        }

        if processedCodexEventSeqs.contains(seq) {
            return false
        }

        processedCodexEventSeqs.insert(seq)
        processedCodexEventSeqOrder.append(seq)
        if processedCodexEventSeqOrder.count > Self.seqCacheLimit {
            let overflow = processedCodexEventSeqOrder.count - Self.seqCacheLimit
            for _ in 0..<overflow {
                let dropped = processedCodexEventSeqOrder.removeFirst()
                processedCodexEventSeqs.remove(dropped)
            }
        }

        return true
    }

    private func applyCodexEvent(_ event: RuntimeCodexProto.CodexEventEnvelope) {
        switch event.method {
        case "thread/started":
            appendSystemMessage(
                "Thread started.",
                threadID: event.threadID,
                turnID: event.turnID,
                occurredAt: event.occurredAt
            )

        case "turn/started":
            appendSystemMessage(
                "Turn started.",
                threadID: event.threadID,
                turnID: event.turnID,
                occurredAt: event.occurredAt
            )

        case "turn/completed", "turn/failed", "turn/aborted", "turn/interrupted":
            let status = extractTurnStatus(from: event.params) ?? "completed"
            appendSystemMessage(
                "Turn \(status).",
                threadID: event.threadID,
                turnID: event.turnID,
                occurredAt: event.occurredAt
            )

        case "error", "codex/error":
            let message = extractErrorMessage(from: event.params) ?? "Codex error."
            appendErrorMessage(
                message,
                threadID: event.threadID,
                turnID: event.turnID,
                occurredAt: event.occurredAt
            )

        case "item/started", "codex/event/item_started":
            handleItemStarted(event)

        case "item/completed", "codex/event/item_completed":
            handleItemCompleted(event)

        case "item/agentMessage/started", "item/assistantMessage/started":
            handleStandaloneAgentMessageStarted(event)

        case "item/agentMessage/completed", "item/assistantMessage/completed":
            handleStandaloneAgentMessageCompleted(event)

        case "item/agentMessage/delta":
            guard let itemID = event.params["itemId"]?.stringValue ?? event.params["item_id"]?.stringValue ?? event.itemID,
                  let delta = event.params["delta"]?.stringValue else {
                return
            }
            appendAssistantDelta(
                itemID: itemID,
                delta: delta,
                source: .modern,
                threadID: event.threadID,
                turnID: event.turnID,
                occurredAt: event.occurredAt
            )

        case "codex/event/agent_message_content_delta", "codex/event/agent_message_delta":
            guard let msg = event.params["msg"]?.objectValue,
                  let itemID = msg["item_id"]?.stringValue ?? msg["itemId"]?.stringValue ?? event.itemID,
                  let delta = msg["delta"]?.stringValue else {
                return
            }
            appendAssistantDelta(
                itemID: itemID,
                delta: delta,
                source: .legacy,
                threadID: event.threadID,
                turnID: event.turnID,
                occurredAt: event.occurredAt
            )

        case "item/reasoning/summaryTextDelta", "item/reasoning/textDelta", "item/reasoning/contentDelta":
            guard let itemID = event.params["itemId"]?.stringValue ?? event.params["item_id"]?.stringValue ?? event.itemID,
                  let delta = event.params["delta"]?.stringValue else {
                return
            }
            appendReasoningDelta(
                itemID: itemID,
                delta: delta,
                threadID: event.threadID,
                turnID: event.turnID,
                occurredAt: event.occurredAt
            )

        case "item/commandExecution/outputDelta", "item/fileChange/outputDelta":
            guard let itemID = event.params["itemId"]?.stringValue ?? event.params["item_id"]?.stringValue ?? event.itemID,
                  let delta = event.params["delta"]?.stringValue else {
                return
            }
            appendToolDelta(
                itemID: itemID,
                delta: delta,
                threadID: event.threadID,
                turnID: event.turnID,
                occurredAt: event.occurredAt
            )

        case "codex/event/user_message":
            guard let msg = event.params["msg"]?.objectValue,
                  let text = msg["message"]?.stringValue else {
                return
            }
            let dedupeKey = userMessageDedupeKey(text: text, threadID: event.threadID, turnID: event.turnID)
            appendUserMessage(
                text,
                dedupeKey: dedupeKey,
                threadID: event.threadID,
                turnID: event.turnID,
                itemID: nil,
                occurredAt: event.occurredAt
            )

        default:
            break
        }
    }

    private func handleItemStarted(_ event: RuntimeCodexProto.CodexEventEnvelope) {
        guard let item = extractItem(from: event.params),
              let itemType = item["type"]?.stringValue else {
            return
        }

        let normalizedType = normalizedItemType(itemType)
        let threadID = event.threadID ?? item["threadId"]?.stringValue ?? item["thread_id"]?.stringValue
        let turnID = event.turnID ?? item["turnId"]?.stringValue ?? item["turn_id"]?.stringValue
        let itemID = item["id"]?.stringValue ?? event.itemID

        switch normalizedType {
        case "usermessage":
            if let text = extractUserMessageText(from: item) {
                let dedupeKey = userMessageDedupeKey(text: text, threadID: threadID, turnID: turnID)
                appendUserMessage(
                    text,
                    dedupeKey: dedupeKey,
                    threadID: threadID,
                    turnID: turnID,
                    itemID: itemID,
                    occurredAt: event.occurredAt
                )
            }

        case "agentmessage", "assistantmessage":
            guard let itemID else {
                return
            }
            ensureAssistantEntry(
                itemID: itemID,
                threadID: threadID,
                turnID: turnID,
                occurredAt: event.occurredAt
            )

        case "reasoning":
            guard let itemID else {
                return
            }
            ensureReasoningEntry(
                itemID: itemID,
                threadID: threadID,
                turnID: turnID,
                occurredAt: event.occurredAt
            )

        case "commandexecution", "filechange", "mcptoolcall", "websearch":
            guard let itemID else {
                return
            }
            let summary = summarizeToolItem(item: item) ?? "Tool started (\(itemType))."
            ensureToolEntry(
                itemID: itemID,
                initialText: summary,
                threadID: threadID,
                turnID: turnID,
                occurredAt: event.occurredAt
            )

        default:
            break
        }
    }

    private func handleItemCompleted(_ event: RuntimeCodexProto.CodexEventEnvelope) {
        guard let item = extractItem(from: event.params),
              let itemType = item["type"]?.stringValue else {
            return
        }

        let normalizedType = normalizedItemType(itemType)
        let threadID = event.threadID ?? item["threadId"]?.stringValue ?? item["thread_id"]?.stringValue
        let turnID = event.turnID ?? item["turnId"]?.stringValue ?? item["turn_id"]?.stringValue
        let itemID = item["id"]?.stringValue ?? event.itemID

        switch normalizedType {
        case "agentmessage", "assistantmessage":
            guard let itemID else {
                return
            }
            finishAssistantMessage(
                itemID: itemID,
                text: extractAgentMessageText(from: item),
                threadID: threadID,
                turnID: turnID,
                occurredAt: event.occurredAt
            )

        case "reasoning":
            guard let itemID else {
                return
            }
            if let summary = extractReasoningText(from: item) {
                finishReasoningMessage(
                    itemID: itemID,
                    text: summary,
                    threadID: threadID,
                    turnID: turnID,
                    occurredAt: event.occurredAt
                )
            }

        case "commandexecution", "filechange", "mcptoolcall", "websearch":
            guard let itemID else {
                return
            }
            let summary = summarizeToolItem(item: item) ?? "Tool completed (\(itemType))."
            finishToolMessage(
                itemID: itemID,
                text: summary,
                threadID: threadID,
                turnID: turnID,
                occurredAt: event.occurredAt
            )

        default:
            break
        }
    }

    private func handleStandaloneAgentMessageStarted(_ event: RuntimeCodexProto.CodexEventEnvelope) {
        guard let itemID = event.params["itemId"]?.stringValue
                ?? event.params["item_id"]?.stringValue
                ?? event.itemID else {
            return
        }
        ensureAssistantEntry(
            itemID: itemID,
            threadID: event.threadID,
            turnID: event.turnID,
            occurredAt: event.occurredAt
        )
    }

    private func handleStandaloneAgentMessageCompleted(_ event: RuntimeCodexProto.CodexEventEnvelope) {
        guard let itemID = event.params["itemId"]?.stringValue
                ?? event.params["item_id"]?.stringValue
                ?? event.params["message"]?.objectValue?["id"]?.stringValue
                ?? event.params["item"]?.objectValue?["id"]?.stringValue
                ?? event.itemID else {
            return
        }

        let text = extractStandaloneAgentMessageText(from: event.params)
        finishAssistantMessage(
            itemID: itemID,
            text: text,
            threadID: event.threadID,
            turnID: event.turnID,
            occurredAt: event.occurredAt
        )
    }

    private func extractItem(from params: [String: JSONValue]) -> [String: JSONValue]? {
        if let item = params["item"]?.objectValue {
            return item
        }

        if let msg = params["msg"]?.objectValue,
           let item = msg["item"]?.objectValue {
            return item
        }

        return nil
    }

    private func normalizedItemType(_ raw: String) -> String {
        raw
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .lowercased()
            .replacingOccurrences(of: "_", with: "")
            .replacingOccurrences(of: "-", with: "")
            .replacingOccurrences(of: " ", with: "")
    }

    private func userMessageDedupeKey(text: String, threadID: String?, turnID: String?) -> String {
        let normalizedText = text
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .replacingOccurrences(of: "\\s+", with: " ", options: .regularExpression)
        return "user:\(threadID ?? "_"):\(turnID ?? "_"):\(normalizedText)"
    }

    private func extractUserMessageText(from item: [String: JSONValue]) -> String? {
        if let text = item["text"]?.stringValue, !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            return text
        }

        let parts = extractTextFragments(from: item["content"])
        guard !parts.isEmpty else {
            return nil
        }
        return parts.joined(separator: "\n")
    }

    private func extractAgentMessageText(from item: [String: JSONValue]) -> String? {
        if let content = item["content"]?.stringValue,
           !content.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            return content
        }

        if let text = item["text"]?.stringValue,
           !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            return text
        }

        let contentParts = extractTextFragments(from: item["content"])
        let merged = contentParts.joined()
        return merged.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? nil : merged
    }

    private func extractReasoningText(from item: [String: JSONValue]) -> String? {
        let summary = extractTextFragments(from: item["summary"]).joined(separator: "\n")
        let content = extractTextFragments(from: item["content"]).joined(separator: "\n")
        let merged = [summary, content]
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }
            .joined(separator: "\n")
        return merged.isEmpty ? nil : merged
    }

    private func extractStandaloneAgentMessageText(from params: [String: JSONValue]) -> String? {
        if let message = params["message"]?.objectValue,
           let text = extractAgentMessageText(from: message) {
            return text
        }

        if let item = params["item"]?.objectValue,
           let text = extractAgentMessageText(from: item) {
            return text
        }

        if let text = params["text"]?.stringValue,
           !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            return text
        }

        return nil
    }

    private func extractTextFragments(from value: JSONValue?) -> [String] {
        guard let value else {
            return []
        }

        if let text = value.stringValue {
            let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
            return trimmed.isEmpty ? [] : [text]
        }

        guard let array = value.arrayValue else {
            return []
        }

        var parts: [String] = []
        for entry in array {
            if let text = entry.stringValue, !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                parts.append(text)
                continue
            }

            if let object = entry.objectValue,
               let text = object["text"]?.stringValue,
               !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                parts.append(text)
            }
        }

        return parts
    }

    private func summarizeToolItem(item: [String: JSONValue]) -> String? {
        guard let type = item["type"]?.stringValue?.lowercased() else {
            return nil
        }

        switch type {
        case "commandexecution":
            let command = item["command"]?.stringValue ?? "command"
            let status = item["status"]?.stringValue ?? "unknown"
            let output = item["aggregatedOutput"]?.stringValue?.trimmingCharacters(in: .whitespacesAndNewlines)
            if let output, !output.isEmpty {
                return "$ \(command)\n[\(status)]\n\(output)"
            }
            return "$ \(command)\n[\(status)]"

        case "filechange":
            let status = item["status"]?.stringValue ?? "unknown"
            let paths = (item["changes"]?.arrayValue ?? [])
                .compactMap { $0.objectValue?["path"]?.stringValue }
            let pathSummary = paths.prefix(3).joined(separator: ", ")
            if pathSummary.isEmpty {
                return "File change [\(status)]"
            }
            return "File change [\(status)]\n\(pathSummary)"

        case "mcptoolcall":
            let server = item["server"]?.stringValue ?? "mcp"
            let tool = item["tool"]?.stringValue ?? "tool"
            let status = item["status"]?.stringValue ?? "unknown"
            return "\(server).\(tool) [\(status)]"

        case "websearch":
            let query = item["query"]?.stringValue ?? "search"
            return "Web search: \(query)"

        default:
            return nil
        }
    }

    private func extractTurnStatus(from params: [String: JSONValue]) -> String? {
        if let turn = params["turn"]?.objectValue,
           let status = turn["status"]?.stringValue {
            return status
        }
        return nil
    }

    private func extractErrorMessage(from params: [String: JSONValue]) -> String? {
        if let error = params["error"]?.objectValue,
           let message = error["message"]?.stringValue,
           !message.isEmpty {
            return message
        }

        if let turn = params["turn"]?.objectValue,
           let error = turn["error"]?.objectValue,
           let message = error["message"]?.stringValue,
           !message.isEmpty {
            return message
        }

        if let message = params["message"]?.stringValue,
           !message.isEmpty {
            return message
        }

        return nil
    }

    private func appendUserMessage(
        _ text: String,
        dedupeKey: String,
        threadID: String?,
        turnID: String?,
        itemID: String?,
        occurredAt: String?
    ) {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            return
        }
        if seenUserMessageKeys.contains(dedupeKey) {
            return
        }
        seenUserMessageKeys.insert(dedupeKey)

        chatMessages.append(
            CodexChatMessage(
                role: .user,
                text: trimmed,
                threadID: threadID,
                turnID: turnID,
                itemID: itemID,
                occurredAt: occurredAt
            )
        )
    }

    private func appendSystemMessage(
        _ text: String,
        threadID: String?,
        turnID: String?,
        occurredAt: String?
    ) {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            return
        }
        chatMessages.append(
            CodexChatMessage(
                role: .system,
                text: trimmed,
                threadID: threadID,
                turnID: turnID,
                itemID: nil,
                occurredAt: occurredAt
            )
        )
    }

    private func appendErrorMessage(
        _ text: String,
        threadID: String?,
        turnID: String?,
        occurredAt: String?
    ) {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            return
        }
        chatMessages.append(
            CodexChatMessage(
                role: .error,
                text: trimmed,
                threadID: threadID,
                turnID: turnID,
                itemID: nil,
                occurredAt: occurredAt
            )
        )
    }

    private func ensureAssistantEntry(
        itemID: String,
        threadID: String?,
        turnID: String?,
        occurredAt: String?
    ) {
        let key = itemKey(threadID: threadID, turnID: turnID, itemID: itemID)
        guard assistantMessageIndexByItemKey[key] == nil else {
            return
        }

        chatMessages.append(
            CodexChatMessage(
                id: "assistant:\(key)",
                role: .assistant,
                text: "",
                isStreaming: true,
                threadID: threadID,
                turnID: turnID,
                itemID: itemID,
                occurredAt: occurredAt
            )
        )
        assistantMessageIndexByItemKey[key] = chatMessages.count - 1
    }

    private func appendAssistantDelta(
        itemID: String,
        delta: String,
        source: AgentDeltaSource,
        threadID: String?,
        turnID: String?,
        occurredAt: String?
    ) {
        guard !delta.isEmpty else {
            return
        }

        let key = itemKey(threadID: threadID, turnID: turnID, itemID: itemID)
        let aliasKey = "\(key)\u{1f}\(delta)"
        if let existing = agentDeltaAliasSources[aliasKey] {
            if existing != source {
                return
            }
        } else {
            if agentDeltaAliasSources.count >= Self.aliasCacheLimit {
                agentDeltaAliasSources.removeAll(keepingCapacity: true)
            }
            agentDeltaAliasSources[aliasKey] = source
        }

        ensureAssistantEntry(itemID: itemID, threadID: threadID, turnID: turnID, occurredAt: occurredAt)
        guard let index = assistantMessageIndexByItemKey[key], chatMessages.indices.contains(index) else {
            return
        }
        chatMessages[index].text.append(delta)
        chatMessages[index].isStreaming = true
    }

    private func finishAssistantMessage(
        itemID: String,
        text: String?,
        threadID: String?,
        turnID: String?,
        occurredAt: String?
    ) {
        let key = itemKey(threadID: threadID, turnID: turnID, itemID: itemID)
        ensureAssistantEntry(itemID: itemID, threadID: threadID, turnID: turnID, occurredAt: occurredAt)

        guard let index = assistantMessageIndexByItemKey[key], chatMessages.indices.contains(index) else {
            return
        }

        if let text {
            let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
            if !trimmed.isEmpty {
                chatMessages[index].text = trimmed
            }
        }
        chatMessages[index].isStreaming = false
    }

    private func ensureReasoningEntry(
        itemID: String,
        threadID: String?,
        turnID: String?,
        occurredAt: String?
    ) {
        let key = itemKey(threadID: threadID, turnID: turnID, itemID: itemID)
        guard reasoningMessageIndexByItemKey[key] == nil else {
            return
        }

        chatMessages.append(
            CodexChatMessage(
                id: "reasoning:\(key)",
                role: .reasoning,
                text: "",
                isStreaming: true,
                threadID: threadID,
                turnID: turnID,
                itemID: itemID,
                occurredAt: occurredAt
            )
        )
        reasoningMessageIndexByItemKey[key] = chatMessages.count - 1
    }

    private func appendReasoningDelta(
        itemID: String,
        delta: String,
        threadID: String?,
        turnID: String?,
        occurredAt: String?
    ) {
        guard !delta.isEmpty else {
            return
        }

        let key = itemKey(threadID: threadID, turnID: turnID, itemID: itemID)
        ensureReasoningEntry(itemID: itemID, threadID: threadID, turnID: turnID, occurredAt: occurredAt)

        guard let index = reasoningMessageIndexByItemKey[key], chatMessages.indices.contains(index) else {
            return
        }
        chatMessages[index].text.append(delta)
        chatMessages[index].isStreaming = true
    }

    private func finishReasoningMessage(
        itemID: String,
        text: String,
        threadID: String?,
        turnID: String?,
        occurredAt: String?
    ) {
        let key = itemKey(threadID: threadID, turnID: turnID, itemID: itemID)
        ensureReasoningEntry(itemID: itemID, threadID: threadID, turnID: turnID, occurredAt: occurredAt)

        guard let index = reasoningMessageIndexByItemKey[key], chatMessages.indices.contains(index) else {
            return
        }

        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        if !trimmed.isEmpty {
            chatMessages[index].text = trimmed
        }
        chatMessages[index].isStreaming = false
    }

    private func ensureToolEntry(
        itemID: String,
        initialText: String,
        threadID: String?,
        turnID: String?,
        occurredAt: String?
    ) {
        let key = itemKey(threadID: threadID, turnID: turnID, itemID: itemID)
        guard toolMessageIndexByItemKey[key] == nil else {
            return
        }

        chatMessages.append(
            CodexChatMessage(
                id: "tool:\(key)",
                role: .tool,
                text: initialText,
                isStreaming: true,
                threadID: threadID,
                turnID: turnID,
                itemID: itemID,
                occurredAt: occurredAt
            )
        )
        toolMessageIndexByItemKey[key] = chatMessages.count - 1
    }

    private func appendToolDelta(
        itemID: String,
        delta: String,
        threadID: String?,
        turnID: String?,
        occurredAt: String?
    ) {
        guard !delta.isEmpty else {
            return
        }

        let key = itemKey(threadID: threadID, turnID: turnID, itemID: itemID)
        ensureToolEntry(
            itemID: itemID,
            initialText: "",
            threadID: threadID,
            turnID: turnID,
            occurredAt: occurredAt
        )

        guard let index = toolMessageIndexByItemKey[key], chatMessages.indices.contains(index) else {
            return
        }

        if chatMessages[index].text.isEmpty {
            chatMessages[index].text = delta
        } else {
            chatMessages[index].text.append(delta)
        }
        chatMessages[index].isStreaming = true
    }

    private func finishToolMessage(
        itemID: String,
        text: String,
        threadID: String?,
        turnID: String?,
        occurredAt: String?
    ) {
        let key = itemKey(threadID: threadID, turnID: turnID, itemID: itemID)
        ensureToolEntry(
            itemID: itemID,
            initialText: text,
            threadID: threadID,
            turnID: turnID,
            occurredAt: occurredAt
        )

        guard let index = toolMessageIndexByItemKey[key], chatMessages.indices.contains(index) else {
            return
        }

        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        if !trimmed.isEmpty {
            chatMessages[index].text = trimmed
        }
        chatMessages[index].isStreaming = false
    }

    private func itemKey(threadID: String?, turnID: String?, itemID: String) -> String {
        _ = threadID
        _ = turnID
        return itemID
    }

    private func resetChatTimeline() {
        chatMessages = []
        assistantMessageIndexByItemKey = [:]
        reasoningMessageIndexByItemKey = [:]
        toolMessageIndexByItemKey = [:]
        seenUserMessageKeys = []
        agentDeltaAliasSources = [:]
        processedCodexEventSeqs = []
        processedCodexEventSeqOrder = []
    }

    private func scheduleHandshakeTimeout(handshakeID: String, seconds: Int) {
        handshakeTimeoutTask?.cancel()

        handshakeTimeoutTask = Task { [weak self] in
            try? await Task.sleep(nanoseconds: UInt64(seconds) * 1_000_000_000)

            guard !Task.isCancelled else {
                return
            }

            await MainActor.run {
                guard let self else {
                    return
                }

                if case .waitingAck(let pendingID) = self.handshakeState, pendingID == handshakeID {
                    self.handshakeState = .timedOut(handshakeID: handshakeID)
                    self.errorMessage = "Handshake timed out after \(seconds)s (no desktop ack)."
                    self.statusMessage = nil
                }
            }
        }
    }

    private func storeSelection() {
        if let selectedWorkerID {
            defaults.set(selectedWorkerID, forKey: selectedWorkerIDKey)
        } else {
            defaults.removeObject(forKey: selectedWorkerIDKey)
        }
    }

    private func selectedWorkerLatestSeq(workerID: String) -> Int {
        workers.first(where: { $0.workerID == workerID })?.latestSeq ?? 0
    }

    private func preferredWorker(from workers: [RuntimeCodexWorkerSummary]) -> RuntimeCodexWorkerSummary? {
        guard !workers.isEmpty else {
            return nil
        }

        let running = workers.filter { $0.status == "running" }
        let desktopRunning = running.filter { isDesktopWorker($0) }
        let pool = desktopRunning.isEmpty ? (running.isEmpty ? workers : running) : desktopRunning

        return pool.sorted { lhs, rhs in
            let lhsShared = sharedWorkerRank(lhs)
            let rhsShared = sharedWorkerRank(rhs)
            if lhsShared != rhsShared {
                return lhsShared > rhsShared
            }

            let lhsFresh = freshnessRank(lhs)
            let rhsFresh = freshnessRank(rhs)
            if lhsFresh != rhsFresh {
                return lhsFresh > rhsFresh
            }

            let lhsHeartbeat = timestampFromISO8601(lhs.lastHeartbeatAt)
            let rhsHeartbeat = timestampFromISO8601(rhs.lastHeartbeatAt)
            if lhsHeartbeat != rhsHeartbeat {
                if let lhsHeartbeat, let rhsHeartbeat {
                    return lhsHeartbeat > rhsHeartbeat
                }

                return lhsHeartbeat != nil
            }

            let lhsStarted = timestampFromISO8601(lhs.startedAt)
            let rhsStarted = timestampFromISO8601(rhs.startedAt)
            if lhsStarted != rhsStarted {
                if let lhsStarted, let rhsStarted {
                    return lhsStarted > rhsStarted
                }

                return lhsStarted != nil
            }

            if lhs.latestSeq != rhs.latestSeq {
                return lhs.latestSeq > rhs.latestSeq
            }

            return lhs.workerID > rhs.workerID
        }.first
    }

    private func isDesktopWorker(_ worker: RuntimeCodexWorkerSummary) -> Bool {
        if worker.adapter == "desktop_bridge" {
            return true
        }

        if worker.workerID.hasPrefix("desktopw:") {
            return true
        }

        return worker.metadata?["source"]?.stringValue == "autopilot-desktop"
    }

    private func freshnessRank(_ worker: RuntimeCodexWorkerSummary) -> Int {
        switch worker.heartbeatState?.lowercased() {
        case "fresh":
            return 2
        case "stale":
            return 1
        default:
            return 0
        }
    }

    private func sharedWorkerRank(_ worker: RuntimeCodexWorkerSummary) -> Int {
        worker.workerID.contains(":shared") ? 1 : 0
    }

    private func timestampFromISO8601(_ raw: String?) -> TimeInterval? {
        guard let raw = raw?.trimmingCharacters(in: .whitespacesAndNewlines), !raw.isEmpty else {
            return nil
        }

        for parser in Self.iso8601Parsers {
            if let date = parser.date(from: raw) {
                return date.timeIntervalSince1970
            }
        }

        return nil
    }

    private func shortWorkerID(_ workerID: String) -> String {
        if workerID.count <= 20 {
            return workerID
        }

        return String(workerID.prefix(20)) + "..."
    }

    private func makeAuthClient() -> RuntimeCodexClient {
        RuntimeCodexClient(baseURL: Self.defaultBaseURL)
    }

    private func makeClient() -> RuntimeCodexClient? {
        let token = authToken.trimmingCharacters(in: .whitespacesAndNewlines)

        guard !token.isEmpty else {
            return nil
        }

        return RuntimeCodexClient(baseURL: Self.defaultBaseURL, authToken: token)
    }

    private func normalizeEmail(_ value: String) -> String {
        value.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
    }

    private func isValidEmail(_ value: String) -> Bool {
        let regex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/
        return value.wholeMatch(of: regex) != nil
    }

    private func iso8601(_ date: Date) -> String {
        ISO8601DateFormatter().string(from: date)
    }

    private func formatError(_ error: Error) -> String {
        if let runtimeError = error as? RuntimeCodexApiError {
            return runtimeError.message
        }

        return error.localizedDescription
    }
}
