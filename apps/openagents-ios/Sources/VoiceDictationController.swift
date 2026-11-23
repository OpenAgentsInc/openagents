// Firm reason: 6 (iOS Speech/AVFoundation live voice dictation streaming directly into the Antigravity composer).
import AVFoundation
import Foundation
import Speech

@MainActor
final class VoiceDictationController: ObservableObject {
    @Published var isListening: Bool = false
    @Published var errorMessage: String?

    private let audioEngine = AVAudioEngine()
    private var recognitionRequest: SFSpeechAudioBufferRecognitionRequest?
    private var recognitionTask: SFSpeechRecognitionTask?
    private var baseText: String = ""

    func toggle(existingText: String, onUpdate: @escaping (String) -> Void) {
        if isListening {
            stop()
            return
        }
        errorMessage = nil
        Task { await start(existingText: existingText, onUpdate: onUpdate) }
    }

    func stop() {
        if audioEngine.isRunning {
            audioEngine.stop()
            audioEngine.inputNode.removeTap(onBus: 0)
        }
        recognitionRequest?.endAudio()
        recognitionRequest = nil
        recognitionTask?.cancel()
        recognitionTask = nil
        isListening = false
        try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
    }

    private func start(existingText: String, onUpdate: @escaping (String) -> Void) async {
        let speechStatus = await withCheckedContinuation { cont in
            SFSpeechRecognizer.requestAuthorization { status in
                cont.resume(returning: status)
            }
        }
        guard speechStatus == .authorized else {
            errorMessage = "Enable Speech Recognition in Settings to dictate prompts."
            return
        }

        let micAllowed = await AVAudioApplication.requestRecordPermission()
        guard micAllowed else {
            errorMessage = "Enable Microphone access in Settings to dictate prompts."
            return
        }

        guard let recognizer = SFSpeechRecognizer(), recognizer.isAvailable else {
            errorMessage = "Speech recognition is temporarily unavailable on this device."
            return
        }

        stop()
        baseText = existingText.trimmingCharacters(in: .whitespacesAndNewlines)

        do {
            let session = AVAudioSession.sharedInstance()
            try session.setCategory(.record, mode: .measurement, options: .duckOthers)
            try session.setActive(true, options: .notifyOthersOnDeactivation)

            let request = SFSpeechAudioBufferRecognitionRequest()
            request.shouldReportPartialResults = true
            self.recognitionRequest = request

            let inputNode = audioEngine.inputNode
            let format = inputNode.outputFormat(forBus: 0)
            guard format.sampleRate > 0, format.channelCount > 0 else {
                errorMessage = "No active microphone input was found."
                stop()
                return
            }
            inputNode.removeTap(onBus: 0)
            inputNode.installTap(onBus: 0, bufferSize: 1024, format: format) { [weak request] buffer, _ in
                request?.append(buffer)
            }

            audioEngine.prepare()
            try audioEngine.start()
            isListening = true

            recognitionTask = recognizer.recognitionTask(with: request) { [weak self] result, error in
                Task { @MainActor in
                    guard let self else { return }
                    if let result {
                        let spoken = result.bestTranscription.formattedString
                        if !spoken.isEmpty {
                            let combined = self.baseText.isEmpty ? spoken : "\(self.baseText) \(spoken)"
                            onUpdate(combined)
                        }
                        if result.isFinal {
                            self.stop()
                        }
                    } else if error != nil {
                        self.stop()
                    }
                }
            }
        } catch {
            errorMessage = "Couldn't start voice dictation."
            stop()
        }
    }
}
