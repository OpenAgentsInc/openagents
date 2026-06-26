import AVFoundation
import Foundation
import Speech

/// On-device speech-to-text via Apple `Speech` (`SFSpeechRecognizer`).
///
/// "Swift speech-to-text for now" (owner): we transcribe locally and send only
/// the resulting text to the Khala API — the audio never leaves the device.
final class SpeechRecognizer {
    enum RecognizerError: Error, LocalizedError {
        case unauthorized
        case unavailable
        case noSpeech

        var errorDescription: String? {
            switch self {
            case .unauthorized: return "Speech recognition not authorized."
            case .unavailable: return "Speech recognition is unavailable."
            case .noSpeech: return "Didn't catch that. Try again."
            }
        }
    }

    private let recognizer = SFSpeechRecognizer(locale: Locale(identifier: "en-US"))
    private var request: SFSpeechAudioBufferRecognitionRequest?
    private var task: SFSpeechRecognitionTask?
    private var latestTranscript: String = ""

    /// Request the Speech permission. Returns true if authorized.
    static func requestAuthorization() async -> Bool {
        await withCheckedContinuation { continuation in
            SFSpeechRecognizer.requestAuthorization { status in
                continuation.resume(returning: status == .authorized)
            }
        }
    }

    /// Begin a streaming recognition request. Caller feeds buffers via
    /// `append(_:)` and ends with `finish()`.
    func start() throws {
        guard SFSpeechRecognizer.authorizationStatus() == .authorized else {
            throw RecognizerError.unauthorized
        }
        guard let recognizer, recognizer.isAvailable else {
            throw RecognizerError.unavailable
        }

        latestTranscript = ""
        let request = SFSpeechAudioBufferRecognitionRequest()
        request.shouldReportPartialResults = true
        // Prefer on-device recognition when the device supports it.
        if recognizer.supportsOnDeviceRecognition {
            request.requiresOnDeviceRecognition = true
        }
        self.request = request

        task = recognizer.recognitionTask(with: request) { [weak self] result, _ in
            guard let self, let result else { return }
            self.latestTranscript = result.bestTranscription.formattedString
        }
    }

    /// Feed an audio buffer captured from `AVAudioEngine`.
    func append(_ buffer: AVAudioPCMBuffer) {
        request?.append(buffer)
    }

    /// Stop capture and return the final transcript. Gives the recognizer a
    /// brief window to finalize partial results.
    func finish() async throws -> String {
        request?.endAudio()
        // Allow the recognizer a moment to settle the final transcript.
        try? await Task.sleep(nanoseconds: 400_000_000)
        task?.cancel()
        request = nil
        task = nil
        let text = latestTranscript.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { throw RecognizerError.noSpeech }
        return text
    }
}
