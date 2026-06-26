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
    @Published private(set) var state: VoiceState = .idle
    @Published private(set) var transcript: String = ""
    @Published private(set) var response: String = ""
    /// Smoothed 0...1 microphone level for the animated background.
    @Published private(set) var level: Double = 0
    @Published var speakResponses: Bool = true

    private let engine = AVAudioEngine()
    private let recognizer = SpeechRecognizer()
    private let synthesizer = SpeechSynthesizer()

    /// Onyx default: ignore presses shorter than 200ms.
    private let minHoldSeconds: TimeInterval = 0.2
    private var pressStart: Date?
    private var capturing = false

    // MARK: - Permissions

    /// Request mic + speech permissions up front. Returns true if both granted.
    func requestPermissions() async -> Bool {
        let speechOK = await SpeechRecognizer.requestAuthorization()
        let micOK = await withCheckedContinuation { continuation in
            AVAudioApplication.requestRecordPermission { granted in
                continuation.resume(returning: granted)
            }
        }
        return speechOK && micOK
    }

    // MARK: - Push-to-talk

    func pressDown() {
        guard !state.isBusy else { return }
        pressStart = Date()
        response = ""
        transcript = ""
        startCapture()
    }

    func pressUp() {
        let held = pressStart.map { Date().timeIntervalSince($0) } ?? 0
        pressStart = nil
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
            transcript = text
            state = .success
            await send(text)
        } catch {
            state = .error((error as? LocalizedError)?.errorDescription ?? "Transcription failed.")
        }
    }

    // MARK: - Text input (voice-free handshake path)

    /// Send a typed message through the same Khala round-trip the voice path
    /// uses. This is the minimal, voice-free way to exercise the end-to-end
    /// handshake (mint/paste key -> type -> POST /chat/completions -> response),
    /// e.g. in the simulator or before microphone permission is granted.
    func sendText(_ text: String) {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        guard !state.isBusy else { return }
        response = ""
        transcript = trimmed
        Task { await send(trimmed) }
    }

    // MARK: - Khala API

    private func send(_ prompt: String) async {
        guard let key = KeychainStore.loadAPIKey() else {
            state = .error("No API key. Open Settings to add one.")
            return
        }
        state = .thinking
        do {
            let reply = try await KhalaClient.complete(prompt: prompt, apiKey: key)
            response = reply
            state = .idle
            if speakResponses { synthesizer.speak(reply) }
        } catch {
            state = .error((error as? LocalizedError)?.errorDescription ?? "Request failed.")
        }
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
