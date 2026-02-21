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
            guard selectedWorkerID != oldValue else {
                return
            }
            storeSelection()
            restartStreamIfReady(resetCursor: true)
        }
    }

    @Published var streamState: StreamState = .idle
    @Published private(set) var streamLifecycle = KhalaLifecycleSnapshot()
    @Published var handshakeState: HandshakeState = .idle
    @Published var statusMessage: String?
    @Published var errorMessage: String?
    @Published private(set) var isSendingCode = false
    @Published private(set) var isVerifyingCode = false
    @Published var messageDraft: String = ""
    @Published private(set) var isSendingMessage = false
    @Published var latestSnapshot: RuntimeCodexWorkerSnapshot?
    @Published var recentEvents: [RuntimeCodexStreamEvent] = []
    @Published var chatMessages: [CodexChatMessage] = []

    private var streamTask: Task<Void, Never>?
    private var activeKhalaSocket: URLSessionWebSocketTask?
    private var activeStreamWorkerID: String?
    private var handshakeTimeoutTask: Task<Void, Never>?
    private var khalaWorkerEventsWatermark: Int
    private var nextCodeSendAllowedAt: Date?
    private var assistantMessageIndexByItemKey: [String: Int] = [:]
    private var reasoningMessageIndexByItemKey: [String: Int] = [:]
    private var toolMessageIndexByItemKey: [String: Int] = [:]
    private var assistantDeltaSourceByItemKey: [String: CodexAssistantDeltaSource] = [:]
    private var seenUserMessageKeys: Set<String> = []
    private var seenUserTurnIDs: Set<String> = []
    private var seenSystemMessageKeys: Set<String> = []
    private var processedCodexEventSeqs: Set<Int> = []
    private var processedCodexEventSeqOrder: [Int] = []
    private var hasAttemptedAutoConnect = false
    private var khalaRefCounter: Int = 0
    private var reconnectAttempt: Int = 0
    private var reconnectWindowStartedAt: Date?

    private let defaults: UserDefaults
    private let now: () -> Date
    private let randomUnit: () -> Double

    private let tokenKey = "autopilot.ios.codex.authToken"
    private let emailKey = "autopilot.ios.codex.email"
    private let selectedWorkerIDKey = "autopilot.ios.codex.selectedWorkerID"
    private let deviceIDKey = "autopilot.ios.codex.deviceID"
    private let khalaWorkerEventsWatermarkKey = "autopilot.ios.codex.khala.workerEventsWatermark"

    private static let defaultBaseURL = URL(string: "https://openagents.com")!
    private static let khalaWorkerEventsTopic = "runtime.codex_worker_events"
    private static let khalaChannelTopic = "sync:v1"
    private static let khalaHeartbeatIntervalNS: UInt64 = 20_000_000_000
    private static let reconnectPolicy = KhalaReconnectPolicy.default
    private static let maxReconnectEventHistory = 24
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

    private struct KhalaFrame {
        let joinRef: String?
        let ref: String?
        let topic: String
        let event: String
        let payload: JSONValue
    }

    let deviceID: String

    var environmentHost: String {
        Self.defaultBaseURL.host ?? "openagents.com"
    }

    var canSendMessage: Bool {
        let trimmed = messageDraft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            return false
        }
        guard isAuthenticated else {
            return false
        }
        guard !isSendingMessage else {
            return false
        }
        return selectedWorkerID != nil || preferredWorker(from: workers) != nil
    }

    var canSendAuthCode: Bool {
        let normalizedEmail = normalizeEmail(email)
        guard isValidEmail(normalizedEmail) else {
            return false
        }
        if let nextAllowed = nextCodeSendAllowedAt, now() < nextAllowed {
            return false
        }
        return !isSendingCode && !isVerifyingCode
    }

    var canVerifyAuthCode: Bool {
        let normalizedCode = verificationCode.replacingOccurrences(of: " ", with: "")
        guard !normalizedCode.isEmpty else {
            return false
        }
        return !isSendingCode && !isVerifyingCode
    }

    @Published private(set) var streamLifecycleEvents: [String] = []

    init(
        defaults: UserDefaults = .standard,
        now: @escaping () -> Date = Date.init,
        randomUnit: @escaping () -> Double = { Double.random(in: 0.0...1.0) }
    ) {
        self.defaults = defaults
        self.now = now
        self.randomUnit = randomUnit

        let savedEmail = defaults.string(forKey: emailKey)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        let savedToken = defaults.string(forKey: tokenKey)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        let savedWorkerEventsWatermark = max(0, defaults.integer(forKey: khalaWorkerEventsWatermarkKey))

        self.email = savedEmail
        self.authToken = savedToken
        self.khalaWorkerEventsWatermark = savedWorkerEventsWatermark
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
        guard !isSendingCode, !isVerifyingCode else {
            return
        }

        if let nextAllowed = nextCodeSendAllowedAt, now() < nextAllowed {
            errorMessage = "Code already sent. Check your email or wait before requesting another."
            return
        }

        let normalizedEmail = normalizeEmail(email)
        guard isValidEmail(normalizedEmail) else {
            errorMessage = "Enter a valid email address first."
            return
        }

        // Always start a fresh email-code session so server-side guest routes are not blocked by stale cookies.
        RuntimeCodexClient.clearSessionCookies(baseURL: Self.defaultBaseURL)

        authState = .sendingCode
        isSendingCode = true
        errorMessage = nil
        statusMessage = "Sending sign-in code..."
        defer {
            isSendingCode = false
        }

        do {
            try await makeAuthClient().sendEmailCode(email: normalizedEmail)
            email = normalizedEmail
            defaults.set(normalizedEmail, forKey: emailKey)
            verificationCode = ""
            authState = .codeSent(email: normalizedEmail)
            nextCodeSendAllowedAt = now().addingTimeInterval(30)
            statusMessage = "Code sent to \(normalizedEmail)."
        } catch {
            authState = .signedOut
            errorMessage = formatError(error)
            statusMessage = nil
        }
    }

    func verifyEmailCode() async {
        guard !isSendingCode, !isVerifyingCode else {
            return
        }

        let normalizedCode = verificationCode.replacingOccurrences(of: " ", with: "")
        guard !normalizedCode.isEmpty else {
            errorMessage = "Enter your verification code first."
            return
        }

        authState = .verifying
        isVerifyingCode = true
        errorMessage = nil
        statusMessage = "Verifying code..."
        defer {
            isVerifyingCode = false
        }

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
            // If another verify request already succeeded, ignore late/stale failures.
            if isAuthenticated {
                return
            }
            authState = .codeSent(email: email)
            errorMessage = formatError(error)
            statusMessage = nil
        }
    }

    func signOut() {
        streamTask?.cancel()
        streamTask = nil
        activeKhalaSocket?.cancel(with: .normalClosure, reason: nil)
        activeKhalaSocket = nil
        activeStreamWorkerID = nil
        handshakeTimeoutTask?.cancel()
        handshakeTimeoutTask = nil
        resetReconnectTracking()

        authToken = ""
        defaults.removeObject(forKey: tokenKey)
        khalaWorkerEventsWatermark = 0
        defaults.removeObject(forKey: khalaWorkerEventsWatermarkKey)
        RuntimeCodexClient.clearSessionCookies(baseURL: Self.defaultBaseURL)
        isSendingCode = false
        isVerifyingCode = false
        isSendingMessage = false
        nextCodeSendAllowedAt = nil

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
                activeStreamWorkerID = nil
                statusMessage = "No running desktop workers found."
                return
            }

            let previousWorkerID = selectedWorkerID
            selectedWorkerID = worker.workerID
            latestSnapshot = try? await client.workerSnapshot(workerID: worker.workerID)
            statusMessage = "Loaded \(result.count) worker(s). Auto-selected \(shortWorkerID(worker.workerID))."

            // If selection did not change, ensure stream still starts once without forcing restart churn.
            if previousWorkerID == worker.workerID,
               streamTask == nil || streamState == .idle {
                restartStreamIfReady(resetCursor: true)
            }

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

    func sendUserMessage() async {
        let trimmed = messageDraft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            return
        }

        guard isAuthenticated else {
            errorMessage = "Sign in first to send a message."
            return
        }

        guard let client = makeClient() else {
            errorMessage = "Sign in first to send a message."
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
        let messageID = "iosmsg-\(UUID().uuidString.lowercased())"
        let occurredAt = iso8601(now())

        messageDraft = ""
        isSendingMessage = true
        errorMessage = nil
        defer {
            isSendingMessage = false
        }

        // Optimistic local echo so mobile feels instant while runtime stream catches up.
        appendUserMessage(
            trimmed,
            dedupeKey: "local:\(messageID)",
            threadID: nil,
            turnID: nil,
            itemID: nil,
            occurredAt: occurredAt
        )

        let payload: [String: JSONValue] = [
            "source": .string("autopilot-ios"),
            "method": .string("ios/user_message"),
            "message_id": .string(messageID),
            "occurred_at": .string(occurredAt),
            "params": .object([
                "message_id": .string(messageID),
                "text": .string(trimmed),
                "sent_from": .string("autopilot-ios"),
            ]),
        ]

        do {
            try await client.ingestWorkerEvent(workerID: workerID, eventType: "worker.event", payload: payload)
            statusMessage = "Sent to \(shortWorkerID(workerID))."
        } catch {
            messageDraft = trimmed
            errorMessage = formatError(error)
            statusMessage = nil
        }
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
        activeKhalaSocket?.cancel(with: .normalClosure, reason: nil)
        activeKhalaSocket = nil
        activeStreamWorkerID = nil
        streamState = .idle
        resetReconnectTracking()
        recordLifecycleEvent("stream_disconnect_manual")
    }

    private func restartStreamIfReady(resetCursor: Bool) {
        guard makeClient() != nil,
              let workerID = selectedWorkerID,
              !workerID.isEmpty else {
            streamTask?.cancel()
            streamTask = nil
            activeKhalaSocket?.cancel(with: .normalClosure, reason: nil)
            activeKhalaSocket = nil
            activeStreamWorkerID = nil
            streamState = .idle
            resetReconnectTracking()
            return
        }

        let isSameWorkerStream =
            activeStreamWorkerID == workerID
            && streamTask != nil
            && (streamState == .connecting || streamState == .live || streamState == .reconnecting)

        if isSameWorkerStream && !resetCursor {
            return
        }

        streamTask?.cancel()
        streamTask = nil
        activeKhalaSocket?.cancel(with: .normalClosure, reason: nil)
        activeKhalaSocket = nil
        activeStreamWorkerID = workerID
        resetReconnectTracking()

        if resetCursor {
            recentEvents = []
            resetChatTimeline()
            fastForwardKhalaWatermarkIfPossible(for: workerID)
        }

        streamTask = Task { [weak self] in
            await self?.runStreamLoop(workerID: workerID)
        }
    }

    private func runStreamLoop(workerID: String) async {
        guard let client = makeClient() else {
            streamState = .idle
            activeStreamWorkerID = nil
            return
        }

        streamState = .connecting
        statusMessage = "Connecting stream for \(shortWorkerID(workerID)) over Khala WS..."
        recordLifecycleEvent("stream_connect_start worker=\(shortWorkerID(workerID))")

        while !Task.isCancelled {
            streamLifecycle.connectAttempts += 1
            let inReconnect = reconnectAttempt > 0
            if inReconnect {
                streamLifecycle.reconnectAttempts += 1
                streamState = .reconnecting
            } else {
                streamState = .connecting
            }

            do {
                let syncToken = try await client.mintSyncToken(scopes: [Self.khalaWorkerEventsTopic])
                let socketURL = try client.syncWebSocketURL(token: syncToken.token)
                let session = URLSession(configuration: .default)
                let socket = session.webSocketTask(with: socketURL)
                socket.resume()
                activeKhalaSocket = socket

                let joinRef = try await khalaJoin(socket: socket, workerID: workerID)
                try await khalaSubscribe(
                    socket: socket,
                    joinRef: joinRef,
                    workerID: workerID,
                    resumeAfterWatermark: khalaWorkerEventsWatermark
                )

                streamState = .live
                errorMessage = nil
                statusMessage = "Stream live for \(shortWorkerID(workerID))."
                streamLifecycle.successfulSessions += 1
                if reconnectAttempt > 0 {
                    streamLifecycle.recoveredSessions += 1
                    if let reconnectStartedAt = reconnectWindowStartedAt {
                        streamLifecycle.lastRecoveryLatencyMs = max(
                            0,
                            Int(now().timeIntervalSince(reconnectStartedAt) * 1000.0)
                        )
                    }
                }
                recordLifecycleEvent(
                    "stream_live worker=\(shortWorkerID(workerID)) reconnect_attempts=\(reconnectAttempt)"
                )
                resetReconnectTracking()

                let heartbeatTask = Task { [weak self] in
                    await self?.runKhalaHeartbeatLoop(socket: socket, joinRef: joinRef)
                }

                defer {
                    heartbeatTask.cancel()
                    socket.cancel(with: .normalClosure, reason: nil)
                    session.invalidateAndCancel()
                    if activeKhalaSocket === socket {
                        activeKhalaSocket = nil
                    }
                }

                while !Task.isCancelled {
                    guard let frame = try await receiveKhalaFrame(socket: socket) else {
                        throw RuntimeCodexApiError(
                            message: "khala_stream_closed",
                            code: .network,
                            status: nil
                        )
                    }

                    try await handleKhalaFrame(frame, workerID: workerID)
                }

                return
            } catch {
                if Task.isCancelled {
                    return
                }

                if let runtimeError = error as? RuntimeCodexApiError,
                   runtimeError.code == .auth || runtimeError.status == 401 {
                    streamLifecycle.lastDisconnectReason = .unauthorized
                    recordLifecycleEvent("stream_unauthorized worker=\(shortWorkerID(workerID))")
                    streamState = .idle
                    activeStreamWorkerID = nil
                    errorMessage = "Khala stream unauthorized. Stay signed in, then reload workers."
                    statusMessage = nil
                    resetReconnectTracking()
                    return
                } else {
                    if reconnectAttempt == 0 {
                        reconnectWindowStartedAt = now()
                    }
                    reconnectAttempt += 1
                    let reason = KhalaReconnectClassifier.classify(error)
                    streamLifecycle.lastDisconnectReason = reason
                    let backoffMs = Self.reconnectPolicy.delayMs(
                        attempt: reconnectAttempt,
                        jitterUnit: randomUnit()
                    )
                    streamLifecycle.lastBackoffMs = backoffMs
                    recordLifecycleEvent(
                        "stream_reconnect_scheduled worker=\(shortWorkerID(workerID)) reason=\(reason.rawValue) attempt=\(reconnectAttempt) delay_ms=\(backoffMs)"
                    )
                    streamState = .reconnecting
                    errorMessage = formatError(error)
                    statusMessage =
                        "Khala stream reconnecting for \(shortWorkerID(workerID)) (attempt \(reconnectAttempt), \(backoffMs)ms)..."
                    try? await Task.sleep(nanoseconds: UInt64(backoffMs) * 1_000_000)
                }
            }
        }
    }

    private func resetReconnectTracking() {
        reconnectAttempt = 0
        reconnectWindowStartedAt = nil
        streamLifecycle.lastBackoffMs = 0
        streamLifecycle.lastRecoveryLatencyMs = 0
    }

    private func recordLifecycleEvent(_ message: String) {
        let stamped = "\(iso8601(now())) \(message)"
        if streamLifecycleEvents.count >= Self.maxReconnectEventHistory {
            streamLifecycleEvents.removeFirst(streamLifecycleEvents.count - Self.maxReconnectEventHistory + 1)
        }
        streamLifecycleEvents.append(stamped)
    }

    private func resetKhalaWorkerEventsWatermarkForReplayBootstrap(reason: String) {
        khalaWorkerEventsWatermark = 0
        defaults.removeObject(forKey: khalaWorkerEventsWatermarkKey)
        recordLifecycleEvent("stale_cursor_reset reason=\(reason)")
    }

    private func khalaJoin(
        socket: URLSessionWebSocketTask,
        workerID: String
    ) async throws -> String {
        let joinRef = nextKhalaRef()

        try await sendKhalaFrame(
            socket: socket,
            joinRef: nil,
            ref: joinRef,
            event: "phx_join",
            payload: [:]
        )

        _ = try await awaitKhalaReply(
            socket: socket,
            expectedRef: joinRef,
            workerID: workerID
        )

        return joinRef
    }

    private func khalaSubscribe(
        socket: URLSessionWebSocketTask,
        joinRef: String,
        workerID: String,
        resumeAfterWatermark: Int
    ) async throws {
        let subscribeRef = nextKhalaRef()
        let payload: [String: Any] = [
            "topics": [Self.khalaWorkerEventsTopic],
            "resume_after": [
                Self.khalaWorkerEventsTopic: max(0, resumeAfterWatermark),
            ],
            "replay_batch_size": 200,
        ]

        try await sendKhalaFrame(
            socket: socket,
            joinRef: joinRef,
            ref: subscribeRef,
            event: "sync:subscribe",
            payload: payload
        )

        _ = try await awaitKhalaReply(
            socket: socket,
            expectedRef: subscribeRef,
            workerID: workerID
        )
    }

    private func runKhalaHeartbeatLoop(
        socket: URLSessionWebSocketTask,
        joinRef: String
    ) async {
        while !Task.isCancelled {
            try? await Task.sleep(nanoseconds: Self.khalaHeartbeatIntervalNS)

            if Task.isCancelled {
                return
            }

            let heartbeatRef = nextKhalaRef()
            try? await sendKhalaFrame(
                socket: socket,
                joinRef: joinRef,
                ref: heartbeatRef,
                event: "sync:heartbeat",
                payload: [:]
            )
        }
    }

    private func awaitKhalaReply(
        socket: URLSessionWebSocketTask,
        expectedRef: String,
        workerID: String
    ) async throws -> JSONValue {
        while !Task.isCancelled {
            guard let frame = try await receiveKhalaFrame(socket: socket) else {
                continue
            }

            if frame.topic != Self.khalaChannelTopic {
                continue
            }

            switch frame.event {
            case "sync:update_batch":
                processKhalaUpdateBatch(frame.payload, workerID: workerID)

            case "sync:error":
                throw khalaSyncError(from: frame.payload)

            case "phx_reply":
                guard frame.ref == expectedRef else {
                    continue
                }

                guard let payloadObject = frame.payload.objectValue else {
                    throw RuntimeCodexApiError(
                        message: "khala_invalid_reply",
                        code: .unknown,
                        status: nil
                    )
                }

                let status = payloadObject["status"]?.stringValue ?? "error"
                if status == "ok" {
                    return payloadObject["response"] ?? .object([:])
                }

                let response = payloadObject["response"]?.objectValue
                let code = response?["code"]?.stringValue
                let message =
                    response?["message"]?.stringValue
                    ?? payloadObject["status"]?.stringValue
                    ?? "khala_request_failed"

                switch code {
                case "unauthorized":
                    throw RuntimeCodexApiError(message: message, code: .auth, status: 401)
                case "forbidden_topic":
                    throw RuntimeCodexApiError(message: message, code: .forbidden, status: 403)
                case "stale_cursor":
                    resetKhalaWorkerEventsWatermarkForReplayBootstrap(reason: "server_reply")
                    throw RuntimeCodexApiError(message: message, code: .conflict, status: 409)
                default:
                    throw RuntimeCodexApiError(message: message, code: .unknown, status: nil)
                }

            default:
                continue
            }
        }

        throw RuntimeCodexApiError(message: "khala_reply_cancelled", code: .network, status: nil)
    }

    private func handleKhalaFrame(
        _ frame: KhalaFrame,
        workerID: String
    ) async throws {
        guard frame.topic == Self.khalaChannelTopic else {
            return
        }

        switch frame.event {
        case "sync:update_batch":
            processKhalaUpdateBatch(frame.payload, workerID: workerID)

        case "sync:error":
            throw khalaSyncError(from: frame.payload)

        case "phx_error":
            throw RuntimeCodexApiError(
                message: "khala_channel_error",
                code: .network,
                status: nil
            )

        default:
            break
        }
    }

    private func processKhalaUpdateBatch(_ payload: JSONValue, workerID: String) {
        guard let payloadObject = payload.objectValue else {
            return
        }

        let updates = payloadObject["updates"]?.arrayValue ?? []
        var matchedEvents: [RuntimeCodexStreamEvent] = []

        for update in updates {
            guard let updateObject = update.objectValue,
                  updateObject["topic"]?.stringValue == Self.khalaWorkerEventsTopic else {
                continue
            }

            if let watermark = updateObject["watermark"]?.intValue, watermark > khalaWorkerEventsWatermark {
                khalaWorkerEventsWatermark = watermark
                defaults.set(watermark, forKey: khalaWorkerEventsWatermarkKey)
            }

            guard let streamPayload = updateObject["payload"]?.objectValue else {
                continue
            }

            let eventWorkerID =
                streamPayload["workerId"]?.stringValue
                ?? streamPayload["worker_id"]?.stringValue

            guard eventWorkerID == workerID else {
                continue
            }

            let eventValue = JSONValue.object(streamPayload)
            let rawData = jsonString(from: eventValue) ?? "{}"
            let seq = streamPayload["seq"]?.intValue

            matchedEvents.append(
                RuntimeCodexStreamEvent(
                    id: seq,
                    event: "codex.worker.event",
                    payload: eventValue,
                    rawData: rawData
                )
            )
        }

        guard !matchedEvents.isEmpty else {
            return
        }

        let orderedEvents = matchedEvents.sorted { lhs, rhs in
            (lhs.cursorHint ?? Int.min) < (rhs.cursorHint ?? Int.min)
        }

        handleIncoming(events: orderedEvents)

        if case .waitingAck = handshakeState {
            return
        }
    }

    private func khalaSyncError(from payload: JSONValue) -> RuntimeCodexApiError {
        guard let object = payload.objectValue else {
            return RuntimeCodexApiError(
                message: "khala_sync_error",
                code: .unknown,
                status: nil
            )
        }

        let code = object["code"]?.stringValue?.trimmingCharacters(in: .whitespacesAndNewlines) ?? "sync_error"
        let message =
            object["message"]?.stringValue?.trimmingCharacters(in: .whitespacesAndNewlines)
            ?? "khala_sync_error"

        if code == "unauthorized" {
            return RuntimeCodexApiError(message: message, code: .auth, status: 401)
        }

        if code == "forbidden_topic" {
            return RuntimeCodexApiError(message: message, code: .forbidden, status: 403)
        }

        if code == "stale_cursor" {
            resetKhalaWorkerEventsWatermarkForReplayBootstrap(reason: "sync_error_payload")
            return RuntimeCodexApiError(message: message, code: .conflict, status: 409)
        }

        return RuntimeCodexApiError(message: message, code: .unknown, status: nil)
    }

    private func sendKhalaFrame(
        socket: URLSessionWebSocketTask,
        joinRef: String?,
        ref: String?,
        event: String,
        payload: [String: Any]
    ) async throws {
        let frame: [Any] = [
            joinRef ?? NSNull(),
            ref ?? NSNull(),
            Self.khalaChannelTopic,
            event,
            payload,
        ]

        let data = try JSONSerialization.data(withJSONObject: frame, options: [])
        let text = String(data: data, encoding: .utf8) ?? "[]"
        try await socket.send(.string(text))
    }

    private func receiveKhalaFrame(socket: URLSessionWebSocketTask) async throws -> KhalaFrame? {
        let message = try await socket.receive()
        let raw: String

        switch message {
        case .string(let text):
            raw = text
        case .data(let data):
            raw = String(data: data, encoding: .utf8) ?? ""
        @unknown default:
            return nil
        }

        guard !raw.isEmpty else {
            return nil
        }

        return parseKhalaFrame(raw: raw)
    }

    private func parseKhalaFrame(raw: String) -> KhalaFrame? {
        guard let data = raw.data(using: .utf8),
              let parsed = try? JSONSerialization.jsonObject(with: data, options: []),
              let frameArray = parsed as? [Any],
              frameArray.count == 5 else {
            return nil
        }

        let joinRef = frameArray[0] as? String
        let ref = frameArray[1] as? String

        guard let topic = frameArray[2] as? String,
              let event = frameArray[3] as? String,
              let payload = jsonValue(from: frameArray[4]) else {
            return nil
        }

        return KhalaFrame(joinRef: joinRef, ref: ref, topic: topic, event: event, payload: payload)
    }

    private func jsonValue(from any: Any) -> JSONValue? {
        switch any {
        case let string as String:
            return .string(string)

        case let number as NSNumber:
            if CFGetTypeID(number) == CFBooleanGetTypeID() {
                return .bool(number.boolValue)
            }

            let doubleValue = number.doubleValue
            if floor(doubleValue) == doubleValue {
                return .int(number.intValue)
            }

            return .double(doubleValue)

        case let dictionary as [String: Any]:
            var mapped: [String: JSONValue] = [:]
            mapped.reserveCapacity(dictionary.count)

            for (key, value) in dictionary {
                guard let converted = jsonValue(from: value) else {
                    return nil
                }
                mapped[key] = converted
            }

            return .object(mapped)

        case let array as [Any]:
            var mapped: [JSONValue] = []
            mapped.reserveCapacity(array.count)

            for value in array {
                guard let converted = jsonValue(from: value) else {
                    return nil
                }
                mapped.append(converted)
            }

            return .array(mapped)

        case _ as NSNull:
            return .null

        default:
            return nil
        }
    }

    private func jsonString(from value: JSONValue) -> String? {
        guard let data = try? JSONEncoder().encode(value) else {
            return nil
        }
        return String(data: data, encoding: .utf8)
    }

    private func nextKhalaRef() -> String {
        khalaRefCounter += 1
        return String(khalaRefCounter)
    }

    private func handleIncoming(events: [RuntimeCodexStreamEvent]) {
        guard !events.isEmpty else {
            return
        }

        recentEvents = (events.reversed() + recentEvents).prefix(100).map { $0 }

        let codexEvents = events.compactMap { event -> RuntimeCodexProto.CodexEventEnvelope? in
            guard let codexEvent = RuntimeCodexProto.decodeCodexEventEnvelope(from: event.payload),
                  codexEvent.source == RuntimeCodexProto.desktopSource,
                  shouldProcessCodexEvent(codexEvent) else {
                return nil
            }
            return codexEvent
        }

        for codexEvent in codexEvents {
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
            if CodexChatEventDisplayPolicy.shouldDisplaySystemMethod(event.method) {
                appendSystemMessage(
                    "Thread started.",
                    threadID: event.threadID,
                    turnID: event.turnID,
                    occurredAt: event.occurredAt
                )
            }

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
                source: .legacyContent,
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
            // Don't render an empty reasoning placeholder bubble on start.
            // A bubble is created only when we get reasoning text.
            _ = itemID

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
            finishReasoningMessage(
                itemID: itemID,
                text: extractReasoningText(from: item),
                threadID: threadID,
                turnID: turnID,
                occurredAt: event.occurredAt
            )

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
        let merged = stitchTextFragments(contentParts)
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
        if let turnID {
            if seenUserTurnIDs.contains(turnID) {
                return
            }
        }
        if seenUserMessageKeys.contains(dedupeKey) {
            return
        }
        seenUserMessageKeys.insert(dedupeKey)
        if let turnID {
            seenUserTurnIDs.insert(turnID)
        }

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

        if let last = chatMessages.last,
           last.role == .system,
           last.text == trimmed {
            return
        }

        if let dedupeKey = systemMessageDedupeKey(text: trimmed, threadID: threadID, turnID: turnID) {
            if seenSystemMessageKeys.contains(dedupeKey) {
                return
            }
            seenSystemMessageKeys.insert(dedupeKey)
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

    private func systemMessageDedupeKey(text: String, threadID: String?, turnID: String?) -> String? {
        guard threadID != nil || turnID != nil else {
            return nil
        }

        return "system:\(threadID ?? "_"):\(turnID ?? "_"):\(text)"
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
        source: CodexAssistantDeltaSource,
        threadID: String?,
        turnID: String?,
        occurredAt: String?
    ) {
        guard !delta.isEmpty else {
            return
        }

        let key = itemKey(threadID: threadID, turnID: turnID, itemID: itemID)
        ensureAssistantEntry(itemID: itemID, threadID: threadID, turnID: turnID, occurredAt: occurredAt)

        let decision = CodexAssistantDeltaPolicy.decide(
            current: assistantDeltaSourceByItemKey[key],
            incoming: source
        )
        assistantDeltaSourceByItemKey[key] = decision.selectedSource

        guard decision.shouldAccept else {
            return
        }

        if decision.shouldReset,
           let index = assistantMessageIndexByItemKey[key],
           chatMessages.indices.contains(index) {
            chatMessages[index].text = ""
        }

        guard let index = assistantMessageIndexByItemKey[key], chatMessages.indices.contains(index) else {
            return
        }
        chatMessages[index].text = appendAssistantChunk(
            chatMessages[index].text,
            delta: delta
        )
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
        chatMessages[index].text = appendAssistantChunk(
            chatMessages[index].text,
            delta: delta
        )
        chatMessages[index].isStreaming = true
    }

    private func finishReasoningMessage(
        itemID: String,
        text: String?,
        threadID: String?,
        turnID: String?,
        occurredAt: String?
    ) {
        let key = itemKey(threadID: threadID, turnID: turnID, itemID: itemID)
        ensureReasoningEntry(itemID: itemID, threadID: threadID, turnID: turnID, occurredAt: occurredAt)

        guard let index = reasoningMessageIndexByItemKey[key], chatMessages.indices.contains(index) else {
            return
        }

        if let text {
            let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
            if !trimmed.isEmpty {
                chatMessages[index].text = trimmed
            }
        }

        let normalized = chatMessages[index].text.trimmingCharacters(in: .whitespacesAndNewlines)
        if normalized.isEmpty || normalized == "..." || normalized == "…" {
            chatMessages.remove(at: index)
            reasoningMessageIndexByItemKey[key] = nil
            reindexMessageDictionaries()
            return
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

    private func appendAssistantChunk(_ existing: String, delta: String) -> String {
        CodexStreamingTextAssembler.append(existing: existing, delta: delta)
    }

    private func stitchTextFragments(_ fragments: [String]) -> String {
        var assembled = ""
        for fragment in fragments {
            assembled = appendAssistantChunk(assembled, delta: fragment)
        }
        return assembled
    }

    private func reindexMessageDictionaries() {
        assistantMessageIndexByItemKey = [:]
        reasoningMessageIndexByItemKey = [:]
        toolMessageIndexByItemKey = [:]

        for (index, message) in chatMessages.enumerated() {
            guard let itemID = message.itemID else {
                continue
            }

            let key = itemKey(threadID: message.threadID, turnID: message.turnID, itemID: itemID)
            switch message.role {
            case .assistant:
                assistantMessageIndexByItemKey[key] = index
            case .reasoning:
                reasoningMessageIndexByItemKey[key] = index
            case .tool:
                toolMessageIndexByItemKey[key] = index
            default:
                continue
            }
        }
    }

    private func resetChatTimeline() {
        chatMessages = []
        assistantMessageIndexByItemKey = [:]
        reasoningMessageIndexByItemKey = [:]
        toolMessageIndexByItemKey = [:]
        assistantDeltaSourceByItemKey = [:]
        seenUserMessageKeys = []
        seenUserTurnIDs = []
        seenSystemMessageKeys = []
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

    private func fastForwardKhalaWatermarkIfPossible(for workerID: String) {
        guard let worker = workers.first(where: { $0.workerID == workerID }) else {
            return
        }

        let projectionSeq = worker.khalaProjection?.lastRuntimeSeq ?? 0
        guard projectionSeq > khalaWorkerEventsWatermark else {
            return
        }

        khalaWorkerEventsWatermark = projectionSeq
        defaults.set(projectionSeq, forKey: khalaWorkerEventsWatermarkKey)
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
