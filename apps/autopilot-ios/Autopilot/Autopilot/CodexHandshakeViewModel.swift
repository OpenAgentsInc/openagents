import Foundation
import Combine
import SwiftUI

@MainActor
final class CodexHandshakeViewModel: ObservableObject {
    @Published var apiBaseURL: String
    @Published var authToken: String

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

    private let baseURLKey = "autopilot.ios.codex.baseURL"
    private let tokenKey = "autopilot.ios.codex.authToken"
    private let selectedWorkerIDKey = "autopilot.ios.codex.selectedWorkerID"
    private let deviceIDKey = "autopilot.ios.codex.deviceID"

    let deviceID: String

    init(defaults: UserDefaults = .standard, now: @escaping () -> Date = Date.init) {
        self.defaults = defaults
        self.now = now

        self.apiBaseURL = defaults.string(forKey: baseURLKey) ?? "https://openagents.com"
        self.authToken = defaults.string(forKey: tokenKey) ?? ""
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

    func saveConfiguration() {
        defaults.set(apiBaseURL.trimmingCharacters(in: .whitespacesAndNewlines), forKey: baseURLKey)
        defaults.set(authToken.trimmingCharacters(in: .whitespacesAndNewlines), forKey: tokenKey)
    }

    func refreshWorkers() async {
        guard let client = makeClient() else {
            errorMessage = "Set API base URL and auth token first."
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
            errorMessage = "Set API base URL and auth token first."
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

    private func makeClient() -> RuntimeCodexClient? {
        let base = apiBaseURL.trimmingCharacters(in: .whitespacesAndNewlines)
        let token = authToken.trimmingCharacters(in: .whitespacesAndNewlines)

        guard !base.isEmpty, !token.isEmpty,
              let url = URL(string: base) else {
            return nil
        }

        return RuntimeCodexClient(baseURL: url, authToken: token)
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
