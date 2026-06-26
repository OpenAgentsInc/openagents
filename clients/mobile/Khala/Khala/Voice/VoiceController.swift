import AVFoundation
import Foundation
import SwiftUI

/// Orchestrates the push-to-talk loop:
///   press-and-hold -> record (AVAudioEngine) -> stop -> on-device STT
///   -> send transcript to Khala API -> show (and optionally speak) response.
///
/// Mirrors Onyx's `VoiceSession`: a short press below `minHoldSeconds` is
/// discarded as an accidental tap. Live mic amplitude drives the animated
/// background.
@MainActor
final class VoiceController: ObservableObject {
    struct RequestError: Equatable {
        let title: String
        let message: String
        let isRetryable: Bool
    }

    private enum Submission: Equatable {
        case codexTask(prompt: String, pylonRef: String)
    }

    @Published private(set) var state: VoiceState = .idle
    @Published private(set) var transcript: String = ""
    @Published private(set) var response: String = ""
    @Published private(set) var requestError: RequestError?
    /// Smoothed 0...1 microphone level for the animated background.
    @Published private(set) var level: Double = 0

    /// Called with the final transcript when a push-to-talk capture finishes.
    /// `ChatView` wires this to `ChatViewModel.send(_:)` so a spoken turn becomes
    /// a normal user turn that streams a reply into the shared transcript. The
    /// voice controller keeps the orb/level visualization but no longer owns the
    /// chat round-trip for the streaming path.
    var onTranscript: ((String) -> Void)?

    private let engine = AVAudioEngine()
    private let recognizer = SpeechRecognizer()

    /// Onyx default: ignore presses shorter than 200ms.
    private let minHoldSeconds: TimeInterval = 0.2
    private var pressStart: Date?
    private var capturing = false
    private var lastSubmission: Submission?

    /// When set (sim/CI), the system permission prompt is suppressed entirely so
    /// cold-launch screenshots and smoke runs never show a dialog. Voice capture
    /// simply no-ops gracefully if it isn't already authorized.
    private var skipPermissionPrompt: Bool {
        ProcessInfo.processInfo.environment["KHALA_SKIP_PERMISSIONS"] != nil
    }

    // MARK: - Permissions

    /// Request mic + speech permissions. Returns true if both granted. The
    /// prompt is DEFERRED to the first push-to-talk press (not launch), so a
    /// cold launch shows no permission dialog.
    @discardableResult
    func requestPermissions() async -> Bool {
        let speechOK = await SpeechRecognizer.requestAuthorization()
        let micOK = await withCheckedContinuation { continuation in
            AVAudioApplication.requestRecordPermission { granted in
                continuation.resume(returning: granted)
            }
        }
        return speechOK && micOK
    }

    /// Current combined authorization without prompting.
    private var permissionsAlreadyAuthorized: Bool {
        SpeechRecognizer.isAuthorized
            && AVAudioApplication.shared.recordPermission == .granted
    }

    /// True once the user has explicitly denied either permission, so we stop
    /// re-prompting and point them at Settings instead.
    private var permissionsExplicitlyDenied: Bool {
        SpeechRecognizer.isDenied
            || AVAudioApplication.shared.recordPermission == .denied
    }

    // MARK: - Push-to-talk

    func pressDown() {
        guard !state.isBusy else { return }

        // Fast path: already authorized -> start immediately, no async hop, no
        // dialog. This is the common case after the first grant.
        if permissionsAlreadyAuthorized {
            beginPress()
            startCapture()
            return
        }

        // Already denied -> graceful, no re-prompt.
        if permissionsExplicitlyDenied {
            state = .error("Enable mic & speech access in Settings to talk.")
            return
        }

        // In sim/CI we never raise the system dialog.
        if skipPermissionPrompt {
            state = .error("Voice is off in this build.")
            return
        }

        // Not yet determined and the user is actively asking to talk: this is the
        // RIGHT moment to request. Request lazily, then start capture if granted.
        Task { @MainActor in
            let granted = await requestPermissions()
            if granted {
                beginPress()
                startCapture()
            } else {
                state = .error("Enable mic & speech access in Settings to talk.")
            }
        }
    }

    /// Reset per-capture state at the start of a press.
    private func beginPress() {
        pressStart = Date()
        response = ""
        transcript = ""
        requestError = nil
    }

    func pressUp() {
        let held = pressStart.map { Date().timeIntervalSince($0) } ?? 0
        pressStart = nil
        // If capture never started (permission denied / still resolving), there
        // is nothing to process; leave any error state intact.
        guard capturing else { return }
        let tooShort = held < minHoldSeconds
        Task { await stopCaptureAndProcess(discard: tooShort) }
    }

    // MARK: - Capture

    private func startCapture() {
        do {
            let session = AVAudioSession.sharedInstance()
            try session.setCategory(.playAndRecord, mode: .default, options: [.duckOthers, .defaultToSpeaker])
            try session.setActive(true, options: .notifyOthersOnDeactivation)

            try recognizer.start()

            let input = engine.inputNode
            let format = input.outputFormat(forBus: 0)
            input.installTap(onBus: 0, bufferSize: 1024, format: format) { [weak self] buffer, _ in
                guard let self else { return }
                self.recognizer.append(buffer)
                let rms = Self.rms(of: buffer)
                Task { @MainActor in self.updateLevel(rms) }
            }
            engine.prepare()
            try engine.start()
            capturing = true
            state = .recording
        } catch {
            capturing = false
            state = .error("Couldn't start the mic.")
        }
    }

    private func stopCaptureAndProcess(discard: Bool) async {
        guard capturing else {
            if discard { state = .idle }
            return
        }
        capturing = false
        engine.inputNode.removeTap(onBus: 0)
        engine.stop()
        level = 0

        if discard {
            _ = try? await recognizer.finish()
            state = .idle
            return
        }

        state = .transcribing
        do {
            let text = try await recognizer.finish()
            let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
            transcript = trimmed
            // The streaming chat round-trip is owned by `ChatViewModel`; hand the
            // transcript off as a normal user turn. The orb returns to idle.
            state = .idle
            if !trimmed.isEmpty {
                onTranscript?(trimmed)
            }
        } catch {
            state = .error((error as? LocalizedError)?.errorDescription ?? "Transcription failed.")
        }
    }

    // MARK: - Codex delegation (typed, non-streaming, separate from chat)

    /// Submit a typed coding delegation request through Khala to the caller's
    /// own linked Pylon. Normal chat remains separate so coding work is always
    /// an explicit app action with an explicit target Pylon ref.
    func sendCodexTask(_ text: String, pylonRef: String) {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        guard !state.isBusy else { return }
        response = ""
        transcript = trimmed
        requestError = nil
        lastSubmission = .codexTask(prompt: trimmed, pylonRef: pylonRef)
        Task { await sendCodexTask(trimmed, pylonRef: pylonRef) }
    }

    func retryLastSubmission() {
        guard let lastSubmission, requestError?.isRetryable == true, !state.isBusy else { return }
        response = ""
        requestError = nil

        switch lastSubmission {
        case .codexTask(let prompt, let pylonRef):
            transcript = prompt
            Task { await sendCodexTask(prompt, pylonRef: pylonRef) }
        }
    }

    // MARK: - Khala API (Codex delegation)

    private func sendCodexTask(_ prompt: String, pylonRef: String) async {
        guard let key = KeychainStore.loadAPIKey() else {
            handle(KhalaClient.KhalaError.missingKey)
            return
        }
        state = .thinking
        do {
            let result = try await KhalaClient.requestCodexTask(
                prompt: prompt,
                pylonRef: pylonRef,
                apiKey: key
            )
            response = result.displayText
            requestError = nil
            state = .idle
        } catch {
            handle(error)
        }
    }

    private func handle(_ error: Error) {
        if let error = error as? KhalaClient.KhalaError {
            requestError = RequestError(
                title: error.recoveryTitle,
                message: error.recoveryMessage,
                isRetryable: error.isRetryable
            )
            state = .error(error.recoveryTitle)
            return
        }

        let message = (error as? LocalizedError)?.errorDescription ?? "Request failed."
        requestError = RequestError(
            title: "Request failed",
            message: message,
            isRetryable: true
        )
        state = .error("Request failed")
    }

    // MARK: - Level metering

    private func updateLevel(_ rms: Float) {
        // Map RMS to a 0...1 range with light smoothing for the background.
        let normalized = min(1.0, Double(rms) * 12.0)
        level = level * 0.7 + normalized * 0.3
    }

    private static func rms(of buffer: AVAudioPCMBuffer) -> Float {
        guard let channel = buffer.floatChannelData?[0] else { return 0 }
        let count = Int(buffer.frameLength)
        guard count > 0 else { return 0 }
        var sum: Float = 0
        for i in 0..<count {
            let sample = channel[i]
            sum += sample * sample
        }
        return (sum / Float(count)).squareRoot()
    }
}
