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

    private var streamTask: Task<Void, Never>?
    private var handshakeTimeoutTask: Task<Void, Never>?
    private var streamCursor: Int = 0

    private let defaults: UserDefaults
    private let now: () -> Date

    private let tokenKey = "autopilot.ios.codex.authToken"
    private let emailKey = "autopilot.ios.codex.email"
    private let selectedWorkerIDKey = "autopilot.ios.codex.selectedWorkerID"
    private let deviceIDKey = "autopilot.ios.codex.deviceID"

    private static let defaultBaseURL = URL(string: "https://openagents.com")!

    private var authToken: String {
        didSet {
            isAuthenticated = !authToken.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        }
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
            statusMessage = "Loaded \(result.count) worker(s)."

            if let selectedWorkerID, result.contains(where: { $0.workerID == selectedWorkerID }) {
                // keep selection
            } else {
                selectedWorkerID = result.first?.workerID
            }

            if let selectedWorkerID {
                streamCursor = max(0, selectedWorkerLatestSeq(workerID: selectedWorkerID) - 1)
                latestSnapshot = try? await client.workerSnapshot(workerID: selectedWorkerID)
            }

            restartStreamIfReady(resetCursor: true)
        } catch {
            errorMessage = formatError(error)
            statusMessage = nil
        }
    }

    func sendHandshake() async {
        guard let workerID = selectedWorkerID else {
            errorMessage = "Select a worker first."
            return
        }

        guard let client = makeClient() else {
            errorMessage = "Sign in first to send handshake."
            return
        }

        let handshakeID = UUID().uuidString.lowercased()
        handshakeState = .sending
        errorMessage = nil
        statusMessage = "Sending handshake..."

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
            statusMessage = "Handshake \(handshakeID) sent. Waiting for desktop ack..."
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

        while !Task.isCancelled {
            do {
                let batch = try await client.streamWorker(workerID: workerID, cursor: streamCursor, tailMS: 15_000)
                if Task.isCancelled {
                    return
                }

                streamState = .live
                streamCursor = batch.nextCursor
                handleIncoming(events: batch.events)

                if batch.events.isEmpty {
                    try? await Task.sleep(nanoseconds: 250_000_000)
                }
            } catch {
                if Task.isCancelled {
                    return
                }

                streamState = .reconnecting
                errorMessage = formatError(error)
                try? await Task.sleep(nanoseconds: 2_000_000_000)
            }
        }
    }

    private func handleIncoming(events: [RuntimeCodexStreamEvent]) {
        guard !events.isEmpty else {
            return
        }

        recentEvents = (events.reversed() + recentEvents).prefix(100).map { $0 }

        if case .waitingAck(let expectedHandshakeID) = handshakeState {
            if events.contains(where: { CodexHandshakeMatcher.isMatchingAck(event: $0, handshakeID: expectedHandshakeID) }) {
                handshakeTimeoutTask?.cancel()
                handshakeTimeoutTask = nil
                handshakeState = .success(handshakeID: expectedHandshakeID)
                statusMessage = "Handshake succeeded. Desktop ack received."
                errorMessage = nil
            }
        }
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
