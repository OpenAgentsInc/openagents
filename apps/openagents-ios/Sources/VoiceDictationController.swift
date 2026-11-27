// Firm reason: 6 (iOS AVFoundation/Speech bi-directional voice controller: streaming sentence-chunked AVSpeechSynthesizer TTS, .playAndRecord/.voiceChat hardware AEC + AirPods/Bluetooth, Coax #authority VAD auto-send & barge-in interruption, and one-way dictation; #14068).
import AVFoundation
import Foundation
import Speech

enum VoiceDuplexState: Int32, Equatable {
    case off = 0
    case listening = 1
    case thinking = 2
    case speaking = 3

    var label: String {
        switch self {
        case .off: return "Off"
        case .listening: return "Listening"
        case .thinking: return "Thinking…"
        case .speaking: return "Speaking"
        }
    }
}

@MainActor
final class VoiceDictationController: ObservableObject {
    @Published var isListening: Bool = false
    @Published var isDuplexActive: Bool = false
    @Published var duplexState: VoiceDuplexState = .off
    @Published var isSpeaking: Bool = false
    @Published var audioLevel: Float = 0
    @Published var errorMessage: String?

    var speechRate: Float = 0.52
    var vadSilenceCommitMs: Int64 = 1200

    private let audioEngine = AVAudioEngine()
    private let synthesizer = AVSpeechSynthesizer()
    private var synthProxy: SynthDelegateProxy?

    private var recognitionRequest: SFSpeechAudioBufferRecognitionRequest?
    private var recognitionTask: SFSpeechRecognitionTask?
    private var baseText: String = ""
    private var liveSpokenText: String = ""
    private var lastLevelTick: TimeInterval = 0

    // Coax #authority voice-duplex.coax VdState vector:
    // [0:state, 1:speech_seen, 2:silence_ms, 3:barge_frames, 4:silence_commit_ms, 5:vad_floor_bp, 6:barge_floor_bp]
    private var vadState: [Int64] = [0, 0, 0, 0, 1200, 800, 2200]

    // Streaming TTS incremental tracking
    private var activeSpeechItemID: String = ""
    private var consumedCharCount: Int = 0
    private var pendingSpeechBuffer: String = ""
    private var queuedUtterances: Int = 0

    private var onDuplexPartial: ((String) -> Void)?
    private var onDuplexAutoSend: ((String) -> Void)?
    private var onDuplexBargeIn: (() -> Void)?

    init() {
        let proxy = SynthDelegateProxy()
        self.synthProxy = proxy
        self.synthesizer.delegate = proxy
        proxy.onStart = { [weak self] in
            Task { @MainActor [weak self] in
                guard let self else { return }
                self.isSpeaking = true
                if self.isDuplexActive {
                    self.setDuplexState(.speaking)
                }
            }
        }
        proxy.onFinishOrCancel = { [weak self] in
            Task { @MainActor [weak self] in
                guard let self else { return }
                self.queuedUtterances = max(0, self.queuedUtterances - 1)
                if self.queuedUtterances == 0 && !self.synthesizer.isSpeaking {
                    self.isSpeaking = false
                    if self.isDuplexActive && self.duplexState == .speaking {
                        self.setDuplexState(.listening)
                        self.restartRecognitionWindowForDuplex()
                    }
                }
            }
        }
    }

    // MARK: - One-Way Voice Dictation (Composer Mic Button)

    func toggle(existingText: String, onUpdate: @escaping (String) -> Void) {
        if isDuplexActive {
            stopLiveVoice()
        }
        if isListening {
            stop()
            return
        }
        errorMessage = nil
        Task { await startOneWay(existingText: existingText, onUpdate: onUpdate) }
    }

    func stop() {
        teardownAudioAndRecognition()
        isListening = false
        audioLevel = 0
        if !isDuplexActive {
            try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
        }
    }

    // MARK: - Bi-Directional Live Voice (Duplex AEC + AirPods + Streaming TTS + VAD + Barge-In)

    func toggleLiveVoice(
        onPartial: @escaping (String) -> Void,
        onAutoSend: @escaping (String) -> Void,
        onBargeIn: @escaping () -> Void
    ) {
        if isDuplexActive {
            stopLiveVoice()
            return
        }
        stop()
        errorMessage = nil
        onDuplexPartial = onPartial
        onDuplexAutoSend = onAutoSend
        onDuplexBargeIn = onBargeIn
        Task { await startLiveVoice() }
    }

    func stopLiveVoice() {
        stopSpeakingImmediate()
        teardownAudioAndRecognition()
        isDuplexActive = false
        isListening = false
        isSpeaking = false
        audioLevel = 0
        setDuplexState(.off)
        liveSpokenText = ""
        activeSpeechItemID = ""
        consumedCharCount = 0
        pendingSpeechBuffer = ""
        try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
    }

    /// Feeds live streaming assistant text from `MobileCloudChat`; immediately speaks completed sentences via `AVSpeechSynthesizer`.
    func feedAssistantStream(itemID: String, fullText: String, complete: Bool) {
        guard isDuplexActive else { return }
        if itemID != activeSpeechItemID {
            activeSpeechItemID = itemID
            consumedCharCount = 0
            pendingSpeechBuffer = ""
        }
        if fullText.count > consumedCharCount {
            let startIdx = fullText.index(fullText.startIndex, offsetBy: consumedCharCount)
            let delta = String(fullText[startIdx...])
            consumedCharCount = fullText.count
            pendingSpeechBuffer.append(delta)
        }
        let sentences = Self.extractCompleteSentences(from: &pendingSpeechBuffer, flushAll: complete)
        for sentence in sentences {
            enqueueUtterance(sentence)
        }
        if complete && queuedUtterances == 0 && !synthesizer.isSpeaking && duplexState == .thinking {
            // Turn finished with nothing left to speak -> return smoothly to hands-free listening.
            setDuplexState(.listening)
            restartRecognitionWindowForDuplex()
        }
    }

    /// Immediate barge-in stop: halts active speech, clears queued sentences, and returns to `.listening`.
    func stopSpeakingImmediate() {
        pendingSpeechBuffer = ""
        queuedUtterances = 0
        if synthesizer.isSpeaking {
            synthesizer.stopSpeaking(at: .immediate)
        }
        isSpeaking = false
    }

    // MARK: - Pure Streaming Sentence Chunker & Markdown Sanitizer (Coax voice-duplex.coax backed)

    /// Strips fenced code blocks and runs Coax `#authority` `vd_sanitize_speech` to produce clean speakable prose.
    static func sanitizeForSpeech(_ raw: String) -> String {
        var text = raw
        while let start = text.range(of: "```") {
            let afterStart = text[start.upperBound...]
            if let end = afterStart.range(of: "```") {
                text.replaceSubrange(start.lowerBound..<end.upperBound, with: " Code block omitted. ")
            } else {
                // Incomplete open code fence while streaming: omit until closed.
                text.removeSubrange(start.lowerBound..<text.endIndex)
                break
            }
        }
        // Strip raw URLs so TTS doesn't spell out https://...
        if let urlRx = try? NSRegularExpression(pattern: "https?://\\S+", options: []) {
            text = urlRx.stringByReplacingMatches(in: text, range: NSRange(text.startIndex..., in: text), withTemplate: "link")
        }
        return CloudCore.voiceDuplexSanitizeSpeech(text)
    }

    /// Extracts speakable complete sentences incrementally using Coax `vd_sentence_end`, preserving incomplete code fences.
    static func extractCompleteSentences(from buffer: inout String, flushAll: Bool) -> [String] {
        var out: [String] = []
        // Wait for closing ``` if a fenced code block is currently open mid-stream.
        let fenceCount = buffer.components(separatedBy: "```").count - 1
        if !flushAll && fenceCount % 2 == 1 {
            return out
        }

        while !buffer.isEmpty {
            let byteEnd = CloudCore.voiceDuplexSentenceEnd(buffer)
            if byteEnd > 0, byteEnd <= buffer.utf8.count {
                let idx = buffer.utf8.index(buffer.utf8.startIndex, offsetBy: byteEnd)
                let rawChunk = String(buffer[..<idx])
                buffer.removeSubrange(..<idx)
                while let first = buffer.first, first.isWhitespace {
                    buffer.removeFirst()
                }
                let clean = sanitizeForSpeech(rawChunk).trimmingCharacters(in: .whitespacesAndNewlines)
                if !clean.isEmpty {
                    out.append(clean)
                }
            } else if flushAll {
                let rawChunk = buffer
                buffer = ""
                let clean = sanitizeForSpeech(rawChunk).trimmingCharacters(in: .whitespacesAndNewlines)
                if !clean.isEmpty {
                    out.append(clean)
                }
            } else {
                break
            }
        }
        return out
    }

    // MARK: - Private Audio & Speech Internals

    private func setDuplexState(_ next: VoiceDuplexState) {
        duplexState = next
        vadState[0] = Int64(next.rawValue)
        vadState[1] = 0
        vadState[2] = 0
        vadState[3] = 0
        vadState[4] = vadSilenceCommitMs
    }

    private func enqueueUtterance(_ sentence: String) {
        let trimmed = sentence.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        let utt = AVSpeechUtterance(string: trimmed)
        utt.rate = max(AVSpeechUtteranceMinimumSpeechRate, min(AVSpeechUtteranceMaximumSpeechRate, speechRate))
        utt.voice = AVSpeechSynthesisVoice(language: Locale.current.identifier) ?? AVSpeechSynthesisVoice(language: "en-US")
        queuedUtterances += 1
        setDuplexState(.speaking)
        synthesizer.speak(utt)
    }

    private func requestPermissions() async -> SFSpeechRecognizer? {
        let speechStatus = await withCheckedContinuation { cont in
            SFSpeechRecognizer.requestAuthorization { status in
                cont.resume(returning: status)
            }
        }
        guard speechStatus == .authorized else {
            errorMessage = "Enable Speech Recognition in Settings to use Voice."
            return nil
        }
        let micAllowed = await AVAudioApplication.requestRecordPermission()
        guard micAllowed else {
            errorMessage = "Enable Microphone access in Settings to use Voice."
            return nil
        }
        guard let recognizer = SFSpeechRecognizer(), recognizer.isAvailable else {
            errorMessage = "Speech recognition is temporarily unavailable on this device."
            return nil
        }
        return recognizer
    }

    private func teardownAudioAndRecognition() {
        if audioEngine.isRunning {
            audioEngine.stop()
            audioEngine.inputNode.removeTap(onBus: 0)
        }
        recognitionRequest?.endAudio()
        recognitionRequest = nil
        recognitionTask?.cancel()
        recognitionTask = nil
    }

    private func startOneWay(existingText: String, onUpdate: @escaping (String) -> Void) async {
        guard let recognizer = await requestPermissions() else { return }
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
            inputNode.installTap(onBus: 0, bufferSize: 1024, format: format) { [weak self, weak request] buffer, _ in
                request?.append(buffer)
                let rms = Self.computeRMS(buffer)
                Task { @MainActor [weak self] in
                    self?.audioLevel = min(1.0, rms * 4.0)
                }
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

    private func startLiveVoice() async {
        guard let recognizer = await requestPermissions() else { return }
        teardownAudioAndRecognition()

        do {
            // Hardware Acoustic Echo Cancellation (AEC) + AirPods / Bluetooth HFP & A2DP duplex routing (#14068).
            let session = AVAudioSession.sharedInstance()
            try session.setCategory(
                .playAndRecord,
                mode: .voiceChat,
                options: [.defaultToSpeaker, .allowBluetoothHFP, .allowBluetoothA2DP]
            )
            try session.setActive(true, options: .notifyOthersOnDeactivation)

            let inputNode = audioEngine.inputNode
            let format = inputNode.outputFormat(forBus: 0)
            guard format.sampleRate > 0, format.channelCount > 0 else {
                errorMessage = "No active microphone input was found."
                stopLiveVoice()
                return
            }

            isDuplexActive = true
            isListening = true
            liveSpokenText = ""
            lastLevelTick = Date().timeIntervalSinceReferenceDate
            setDuplexState(.listening)

            beginDuplexRecognitionTask(recognizer: recognizer)

            inputNode.removeTap(onBus: 0)
            inputNode.installTap(onBus: 0, bufferSize: 1024, format: format) { [weak self] buffer, _ in
                let rms = Self.computeRMS(buffer)
                Task { @MainActor [weak self] in
                    guard let self, self.isDuplexActive else { return }
                    if self.duplexState == .listening {
                        self.recognitionRequest?.append(buffer)
                    }
                    self.processDuplexAudioFrame(rms: rms)
                }
            }

            audioEngine.prepare()
            try audioEngine.start()
        } catch {
            errorMessage = "Couldn't start Live Voice duplex audio session."
            stopLiveVoice()
        }
    }

    private func beginDuplexRecognitionTask(recognizer: SFSpeechRecognizer) {
        recognitionTask?.cancel()
        recognitionTask = nil
        recognitionRequest?.endAudio()

        let request = SFSpeechAudioBufferRecognitionRequest()
        request.shouldReportPartialResults = true
        self.recognitionRequest = request
        self.liveSpokenText = ""

        recognitionTask = recognizer.recognitionTask(with: request) { [weak self] result, _ in
            Task { @MainActor [weak self] in
                guard let self, self.isDuplexActive else { return }
                if let result {
                    let spoken = result.bestTranscription.formattedString.trimmingCharacters(in: .whitespacesAndNewlines)
                    if !spoken.isEmpty {
                        self.liveSpokenText = spoken
                        self.onDuplexPartial?(spoken)
                    }
                }
            }
        }
    }

    private func restartRecognitionWindowForDuplex() {
        guard isDuplexActive, let recognizer = SFSpeechRecognizer(), recognizer.isAvailable else { return }
        beginDuplexRecognitionTask(recognizer: recognizer)
    }

    private func processDuplexAudioFrame(rms: Float) {
        let now = Date().timeIntervalSinceReferenceDate
        let dtSec = now - lastLevelTick
        guard dtSec >= 0.045 else { return }
        lastLevelTick = now
        let dtMs = max(1, min(500, Int64((dtSec * 1000.0).rounded())))
        audioLevel = min(1.0, rms * 4.0)
        let rmsBp = max(0, min(10_000, Int64((rms * 10_000.0).rounded())))

        // Drive Coax #authority voice-duplex.coax VAD & barge-in state machine!
        let ev = CloudCore.voiceDuplexVadStep(state: &vadState, rmsBp: rmsBp, dtMs: dtMs)
        if ev == 1 { // VD_EV_AUTO_SEND
            let spoken = liveSpokenText.trimmingCharacters(in: .whitespacesAndNewlines)
            if !spoken.isEmpty {
                duplexState = .thinking
                liveSpokenText = ""
                onDuplexAutoSend?(spoken)
            } else {
                // Silence after noise spike with no transcribed speech -> remain listening.
                setDuplexState(.listening)
            }
        } else if ev == 2 { // VD_EV_BARGE_IN
            stopSpeakingImmediate()
            duplexState = .listening
            restartRecognitionWindowForDuplex()
            onDuplexBargeIn?()
        }
    }

    private nonisolated static func computeRMS(_ buffer: AVAudioPCMBuffer) -> Float {
        guard let ch = buffer.floatChannelData?[0] else { return 0 }
        let n = Int(buffer.frameLength)
        guard n > 0 else { return 0 }
        var sumSq: Float = 0
        for i in 0..<n {
            let s = ch[i]
            sumSq += s * s
        }
        return sqrt(sumSq / Float(n))
    }
}

private final class SynthDelegateProxy: NSObject, AVSpeechSynthesizerDelegate {
    var onStart: (() -> Void)?
    var onFinishOrCancel: (() -> Void)?

    func speechSynthesizer(_ synthesizer: AVSpeechSynthesizer, didStart utterance: AVSpeechUtterance) {
        onStart?()
    }

    func speechSynthesizer(_ synthesizer: AVSpeechSynthesizer, didFinish utterance: AVSpeechUtterance) {
        onFinishOrCancel?()
    }

    func speechSynthesizer(_ synthesizer: AVSpeechSynthesizer, didCancel utterance: AVSpeechUtterance) {
        onFinishOrCancel?()
    }
}
