// Firm reason: 6 (macOS AVFoundation + Speech framework host boundary over page/voice.coax).
// Native & local voice dictation in Desktop composer (GitHub #13882):
// - On-device macOS Apple Speech (SFSpeechRecognizer, requiresOnDeviceRecognition)
// - Local Whisper CLI + GGML model under ~/.openagents/models/
// - AVAudioEngine microphone tap computing RMS levels for the live 8-bar waveform pill
// - Configurable ⌥Space hotkey (Toggle or Push-to-talk) and Settings -> Voice dialogs.
import AppKit
import AVFoundation
import Speech

private enum VoiceDefaults {
    static let backend = "voice.backend"
    static let mode = "voice.mode"
    static let stripFillers = "voice.stripFillers"
    static let dictionary = "voice.dictionary"
    static let micName = "voice.micName"
}

private enum VoiceStateCode {
    static let idle: Int32 = 0
    static let listening: Int32 = 1
    static let transcribing: Int32 = 2
    static let error: Int32 = 3
}

private enum VoiceBackendCode {
    static let apple: Int32 = 0
    static let whisper: Int32 = 1
}

private enum VoiceModeCode {
    static let toggle: Int32 = 0
    static let hold: Int32 = 1
}

final class CoderVoiceController {
    var onState: ((Int32, String) -> Void)?
    var onLevel: ((Float) -> Void)?
    var onPartial: ((String) -> Void)?
    var onCommit: ((String) -> Void)?

    private var audioEngine: AVAudioEngine?
    private var speechRecognizer: SFSpeechRecognizer?
    private var recognitionRequest: SFSpeechAudioBufferRecognitionRequest?
    private var recognitionTask: SFSpeechRecognitionTask?
    private var latestTranscript = ""
    private var activeBackend: Int32 = VoiceBackendCode.apple
    private var isStopping = false
    private var isCancelled = false
    private var lastLevelAt: CFTimeInterval = 0

    // 16 kHz mono PCM accumulator for Local Whisper backend.
    private let pcmLock = NSLock()
    private var pcm16k: [Int16] = []

    var isListening: Bool { audioEngine?.isRunning == true }

    static func discoverMicrophones() -> [String] {
        let session = AVCaptureDevice.DiscoverySession(
            deviceTypes: [.microphone, .external],
            mediaType: .audio,
            position: .unspecified
        )
        let names = session.devices.map { $0.localizedName }
        return names.isEmpty ? ["System Default"] : names
    }

    static func discoverWhisperBinary() -> String? {
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        let candidates = [
            home + "/.openagents/bin/whisper-cli",
            home + "/.openagents/bin/whisper-cpp",
            home + "/.openagents/bin/whisper",
            "/opt/homebrew/bin/whisper-cli",
            "/opt/homebrew/bin/whisper-cpp",
            "/usr/local/bin/whisper-cli",
            "/usr/local/bin/whisper-cpp"
        ]
        for p in candidates where FileManager.default.isExecutableFile(atPath: p) {
            return p
        }
        return nil
    }

    static func discoverWhisperModel() -> String? {
        let fm = FileManager.default
        let home = fm.homeDirectoryForCurrentUser
        let dirs = [
            home.appendingPathComponent(".openagents/models"),
            home.appendingPathComponent(".openagents/models/whisper")
        ]
        for dir in dirs {
            guard let items = try? fm.contentsOfDirectory(at: dir, includingPropertiesForKeys: nil) else { continue }
            for url in items.sorted(by: { $0.lastPathComponent < $1.lastPathComponent }) {
                let name = url.lastPathComponent.lowercased()
                if name.hasSuffix(".bin") && (name.contains("ggml") || name.contains("whisper")) {
                    return url.path
                }
            }
        }
        return nil
    }

    static func whisperStatusSummary() -> (String, Bool) {
        let bin = discoverWhisperBinary()
        let model = discoverWhisperModel()
        if let m = model, bin != nil {
            let base = URL(fileURLWithPath: m).lastPathComponent
            return ("\(base) (ready)", true)
        }
        if let m = model {
            let base = URL(fileURLWithPath: m).lastPathComponent
            return ("\(base) (missing whisper-cli in ~/.openagents/bin/)", false)
        }
        if bin != nil {
            return ("No ggml-*.bin model in ~/.openagents/models/", false)
        }
        return ("Not installed (~/.openagents/models/)", false)
    }

    static func appleSpeechReady() -> Bool {
        guard let rec = SFSpeechRecognizer(locale: Locale.current) else { return false }
        return rec.isAvailable
    }

    func start(backend: Int32) {
        stop(cancel: true)
        activeBackend = backend
        isStopping = false
        isCancelled = false
        latestTranscript = ""
        pcmLock.lock(); pcm16k.removeAll(keepingCapacity: true); pcmLock.unlock()

        AVCaptureDevice.requestAccess(for: .audio) { [weak self] micGranted in
            DispatchQueue.main.async {
                guard let self else { return }
                if !micGranted {
                    self.onState?(VoiceStateCode.error, "Microphone access denied. Enable in System Settings → Privacy.")
                    return
                }
                if backend == VoiceBackendCode.whisper {
                    self.startWhisperCapture()
                } else {
                    self.requestSpeechAndStartApple()
                }
            }
        }
    }

    func stop(cancel: Bool) {
        isCancelled = cancel
        let wasRunning = audioEngine?.isRunning == true
        if let engine = audioEngine {
            engine.inputNode.removeTap(onBus: 0)
            engine.stop()
        }
        audioEngine = nil

        if activeBackend == VoiceBackendCode.apple {
            recognitionRequest?.endAudio()
            if cancel {
                recognitionTask?.cancel()
                recognitionTask = nil
                recognitionRequest = nil
                onState?(VoiceStateCode.idle, "")
            } else if wasRunning {
                isStopping = true
                onState?(VoiceStateCode.transcribing, "")
                let fallback = latestTranscript
                // Safety net in case SFSpeechRecognizer doesn't deliver isFinal promptly.
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.9) { [weak self] in
                    guard let self, self.isStopping else { return }
                    self.finishAppleCommit(fallback)
                }
            } else {
                onState?(VoiceStateCode.idle, "")
            }
            return
        }

        // Local Whisper backend
        if cancel || !wasRunning {
            pcmLock.lock(); pcm16k.removeAll(keepingCapacity: false); pcmLock.unlock()
            onState?(VoiceStateCode.idle, "")
            return
        }
        onState?(VoiceStateCode.transcribing, "")
        runLocalWhisperTranscription()
    }

    // MARK: - Apple Speech (On-Device)

    private func requestSpeechAndStartApple() {
        SFSpeechRecognizer.requestAuthorization { [weak self] status in
            DispatchQueue.main.async {
                guard let self else { return }
                guard status == .authorized else {
                    self.onState?(VoiceStateCode.error, "Speech recognition permission denied.")
                    return
                }
                self.startAppleSpeechEngine()
            }
        }
    }

    private func startAppleSpeechEngine() {
        guard let recognizer = SFSpeechRecognizer(locale: Locale.current), recognizer.isAvailable else {
            onState?(VoiceStateCode.error, "Apple Speech recognizer is unavailable.")
            return
        }
        let request = SFSpeechAudioBufferRecognitionRequest()
        request.shouldReportPartialResults = true
        if recognizer.supportsOnDeviceRecognition {
            request.requiresOnDeviceRecognition = true
        }
        let engine = AVAudioEngine()
        let input = engine.inputNode
        let fmt = input.outputFormat(forBus: 0)
        guard fmt.sampleRate > 0, fmt.channelCount > 0 else {
            onState?(VoiceStateCode.error, "No active microphone input format.")
            return
        }
        input.removeTap(onBus: 0)
        input.installTap(onBus: 0, bufferSize: 1024, format: fmt) { [weak self] buffer, _ in
            request.append(buffer)
            self?.emitLevel(from: buffer)
        }
        engine.prepare()
        do {
            try engine.start()
        } catch {
            input.removeTap(onBus: 0)
            onState?(VoiceStateCode.error, "Audio engine failed: \(error.localizedDescription)")
            return
        }
        speechRecognizer = recognizer
        recognitionRequest = request
        audioEngine = engine
        onState?(VoiceStateCode.listening, "")

        recognitionTask = recognizer.recognitionTask(with: request) { [weak self] result, error in
            DispatchQueue.main.async {
                guard let self else { return }
                if self.isCancelled { return }
                if let result {
                    let text = result.bestTranscription.formattedString
                    self.latestTranscript = text
                    if result.isFinal {
                        self.finishAppleCommit(text)
                        return
                    } else if !self.isStopping {
                        self.onPartial?(text)
                    }
                }
                if let error {
                    if self.isStopping && !self.latestTranscript.isEmpty {
                        self.finishAppleCommit(self.latestTranscript)
                    } else if !self.isStopping {
                        self.stop(cancel: true)
                        self.onState?(VoiceStateCode.error, error.localizedDescription)
                    } else {
                        self.finishAppleCommit("")
                    }
                }
            }
        }
    }

    private func finishAppleCommit(_ text: String) {
        isStopping = false
        recognitionTask?.cancel()
        recognitionTask = nil
        recognitionRequest = nil
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        if !trimmed.isEmpty {
            onCommit?(trimmed)
        } else {
            onState?(VoiceStateCode.idle, "")
        }
    }

    // MARK: - Local Whisper (~/.openagents/models/)

    private func startWhisperCapture() {
        guard Self.discoverWhisperBinary() != nil, Self.discoverWhisperModel() != nil else {
            let (msg, _) = Self.whisperStatusSummary()
            onState?(VoiceStateCode.error, "Local Whisper unavailable: \(msg)")
            return
        }
        let engine = AVAudioEngine()
        let input = engine.inputNode
        let inFmt = input.outputFormat(forBus: 0)
        guard inFmt.sampleRate > 0, inFmt.channelCount > 0 else {
            onState?(VoiceStateCode.error, "No active microphone input format.")
            return
        }
        guard let outFmt = AVAudioFormat(commonFormat: .pcmFormatInt16, sampleRate: 16000, channels: 1, interleaved: true),
              let converter = AVAudioConverter(from: inFmt, to: outFmt) else {
            onState?(VoiceStateCode.error, "Unable to create 16 kHz audio converter.")
            return
        }
        input.removeTap(onBus: 0)
        input.installTap(onBus: 0, bufferSize: 1024, format: inFmt) { [weak self] buffer, _ in
            guard let self else { return }
            self.emitLevel(from: buffer)
            let ratio = 16000.0 / inFmt.sampleRate
            let outFrames = AVAudioFrameCount(max(1, Int(Double(buffer.frameLength) * ratio) + 16))
            guard let convBuf = AVAudioPCMBuffer(pcmFormat: outFmt, frameCapacity: outFrames) else { return }
            var supplied = false
            var convErr: NSError?
            converter.convert(to: convBuf, error: &convErr) { _, outStatus in
                if supplied {
                    outStatus.pointee = .noDataNow
                    return nil
                }
                supplied = true
                outStatus.pointee = .haveData
                return buffer
            }
            if convErr == nil, let ptr = convBuf.int16ChannelData?[0] {
                let count = Int(convBuf.frameLength)
                self.pcmLock.lock()
                // Cap at 120 seconds of 16 kHz mono audio (~3.8 MB).
                if self.pcm16k.count + count <= 16000 * 120 {
                    self.pcm16k.append(contentsOf: UnsafeBufferPointer(start: ptr, count: count))
                }
                self.pcmLock.unlock()
            }
        }
        engine.prepare()
        do {
            try engine.start()
        } catch {
            input.removeTap(onBus: 0)
            onState?(VoiceStateCode.error, "Audio engine failed: \(error.localizedDescription)")
            return
        }
        audioEngine = engine
        onState?(VoiceStateCode.listening, "")
    }

    private func runLocalWhisperTranscription() {
        guard let bin = Self.discoverWhisperBinary(), let model = Self.discoverWhisperModel() else {
            onState?(VoiceStateCode.error, "Local Whisper model or binary missing.")
            return
        }
        pcmLock.lock()
        let rawSamples = pcm16k
        pcm16k.removeAll(keepingCapacity: false)
        pcmLock.unlock()

        // Trim leading/trailing silence via 20 ms energy VAD before invoking Whisper.
        let samples = Self.trimSilenceVAD(rawSamples, sampleRate: 16000)
        guard samples.count >= 1600 else {
            onState?(VoiceStateCode.idle, "")
            return
        }
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            let wavURL = URL(fileURLWithPath: NSTemporaryDirectory())
                .appendingPathComponent("openagents-voice-\(UUID().uuidString).wav")
            defer { try? FileManager.default.removeItem(at: wavURL) }
            guard Self.writeWav16kMono(samples, to: wavURL) else {
                DispatchQueue.main.async { self?.onState?(VoiceStateCode.error, "Failed to write temporary WAV.") }
                return
            }
            let proc = Process()
            proc.executableURL = URL(fileURLWithPath: bin)
            proc.arguments = ["-m", model, "-f", wavURL.path, "-nt", "--no-prints"]
            let pipe = Pipe()
            proc.standardOutput = pipe
            proc.standardError = Pipe()
            do {
                try proc.run()
                proc.waitUntilExit()
                let data = pipe.fileHandleForReading.readDataToEndOfFile()
                let text = (String(data: data, encoding: .utf8) ?? "")
                    .replacingOccurrences(of: "\n", with: " ")
                    .trimmingCharacters(in: .whitespacesAndNewlines)
                DispatchQueue.main.async {
                    guard let self else { return }
                    if proc.terminationStatus == 0 && !text.isEmpty {
                        self.onCommit?(text)
                    } else if proc.terminationStatus == 0 {
                        self.onState?(VoiceStateCode.idle, "")
                    } else {
                        self.onState?(VoiceStateCode.error, "Local Whisper exited with status \(proc.terminationStatus).")
                    }
                }
            } catch {
                DispatchQueue.main.async {
                    self?.onState?(VoiceStateCode.error, "Failed to launch Local Whisper: \(error.localizedDescription)")
                }
            }
        }
    }

    private static func trimSilenceVAD(_ samples: [Int16], sampleRate: Int) -> [Int16] {
        let frame = sampleRate / 50   // 20 ms = 320 samples (matches metaverse/voice.coax FRAME)
        guard samples.count > frame * 2 else { return samples }
        let threshold: Float = 0.008
        let nFrames = samples.count / frame
        var first = 0
        var last = nFrames - 1
        var found = false
        for fi in 0..<nFrames {
            var sumSq: Float = 0
            let base = fi * frame
            for j in 0..<frame {
                let s = Float(samples[base + j]) / 32768.0
                sumSq += s * s
            }
            let rms = sqrt(sumSq / Float(frame))
            if rms >= threshold {
                if !found { first = max(0, fi - 5); found = true }
                last = min(nFrames - 1, fi + 8)
            }
        }
        guard found else { return [] }
        return Array(samples[(first * frame)..<min(samples.count, (last + 1) * frame)])
    }

    private static func writeWav16kMono(_ samples: [Int16], to url: URL) -> Bool {
        let dataSize = UInt32(samples.count * 2)
        var data = Data(capacity: 44 + Int(dataSize))
        func put(_ s: String) { data.append(contentsOf: s.utf8) }
        func put32(_ v: UInt32) { var le = v.littleEndian; withUnsafeBytes(of: &le) { data.append(contentsOf: $0) } }
        func put16(_ v: UInt16) { var le = v.littleEndian; withUnsafeBytes(of: &le) { data.append(contentsOf: $0) } }
        put("RIFF"); put32(36 + dataSize); put("WAVE")
        put("fmt "); put32(16); put16(1); put16(1); put32(16000); put32(32000); put16(2); put16(16)
        put("data"); put32(dataSize)
        samples.withUnsafeBufferPointer { buf in
            data.append(UnsafeBufferPointer(start: UnsafeRawPointer(buf.baseAddress!).assumingMemoryBound(to: UInt8.self), count: Int(dataSize)))
        }
        return (try? data.write(to: url)) != nil
    }

    private func emitLevel(from buffer: AVAudioPCMBuffer) {
        let now = CACurrentMediaTime()
        if now - lastLevelAt < 0.05 { return }
        lastLevelAt = now
        guard let ch = buffer.floatChannelData?[0] else { return }
        let n = Int(buffer.frameLength)
        guard n > 0 else { return }
        var sumSq: Float = 0
        for i in 0..<n {
            let v = ch[i]
            sumSq += v * v
        }
        let rms = sqrt(sumSq / Float(n))
        DispatchQueue.main.async { [weak self] in
            self?.onLevel?(rms)
        }
    }
}

extension CoderWindow {
    func setupVoiceDictation() {
        let ud = UserDefaults.standard
        let backend = Int32(ud.integer(forKey: VoiceDefaults.backend))
        let mode = Int32(ud.integer(forKey: VoiceDefaults.mode))
        let fillers: Int32 = ud.object(forKey: VoiceDefaults.stripFillers) == nil ? 1 : (ud.bool(forKey: VoiceDefaults.stripFillers) ? 1 : 0)
        let dict = ud.string(forKey: VoiceDefaults.dictionary) ?? ""

        cd_voice_set_backend(page, backend)
        cd_voice_set_mode(page, mode)
        cd_voice_set_fillers(page, fillers)
        _ = cd_voice_dict_set(page, dict)
        refreshVoiceStatus()

        voice.onState = { [weak self] st, msg in
            guard let self else { return }
            cd_voice_set_state(self.page, st, msg)
            self.wake()
        }
        voice.onLevel = { [weak self] rms in
            guard let self else { return }
            cd_voice_level(self.page, rms)
            self.wake()
        }
        voice.onPartial = { [weak self] text in
            guard let self else { return }
            cd_voice_partial(self.page, text)
            self.wake()
        }
        voice.onCommit = { [weak self] text in
            guard let self else { return }
            _ = cd_voice_commit(self.page, text)
            self.focusComposer()
            self.wake()
        }
    }

    func refreshVoiceStatus() {
        let ud = UserDefaults.standard
        let mics = CoderVoiceController.discoverMicrophones()
        let savedMic = ud.string(forKey: VoiceDefaults.micName)
        let activeMic = (savedMic != nil && mics.contains(savedMic!)) ? savedMic! : (mics.first ?? "System Default")
        let (whisperLabel, whisperReady) = CoderVoiceController.whisperStatusSummary()
        let appleReady = CoderVoiceController.appleSpeechReady()
        cd_voice_status_set(page, activeMic, whisperLabel, appleReady ? 1 : 0, whisperReady ? 1 : 0)
    }

    func toggleVoiceDictation() {
        let st = cd_voice_state(page)
        if st == VoiceStateCode.listening || st == VoiceStateCode.transcribing {
            voice.stop(cancel: false)
        } else {
            refreshVoiceStatus()
            voice.start(backend: cd_voice_backend(page))
        }
        wake()
    }

    func cancelVoiceDictation() {
        voice.stop(cancel: true)
        wake()
    }

    func voiceBackendChanged() {
        let b = cd_voice_backend(page)
        UserDefaults.standard.set(Int(b), forKey: VoiceDefaults.backend)
        refreshVoiceStatus()
        wake()
    }

    func voicePrefsChanged() {
        let ud = UserDefaults.standard
        ud.set(Int(cd_voice_mode(page)), forKey: VoiceDefaults.mode)
        ud.set(cd_voice_fillers(page) != 0, forKey: VoiceDefaults.stripFillers)
        wake()
    }

    func chooseVoiceMicrophone() {
        let mics = CoderVoiceController.discoverMicrophones()
        let current = UserDefaults.standard.string(forKey: VoiceDefaults.micName) ?? mics.first ?? "System Default"
        var d = CoderDialog(size: .sm, title: "Microphone", body: "Available macOS audio input devices. OpenAgents uses your system input device for dictation; select a preferred label or open Sound Settings.")
        d.fields = [.init(label: "Active device", value: current)]
        d.buttons = [.init(title: "Sound Settings", aside: true), .init(title: "Cancel"), .init(title: "Save", kind: .primary)]
        d.cancel = 1; d.confirm = 2
        present(d) { [weak self] answer, values in
            guard let self else { return }
            if answer == 0 {
                if let url = URL(string: "x-apple.systempreferences:com.apple.Sound-Settings.extension") {
                    NSWorkspace.shared.open(url)
                }
            } else if answer == 2 {
                let chosen = (values.first ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
                if !chosen.isEmpty { UserDefaults.standard.set(chosen, forKey: VoiceDefaults.micName) }
                self.refreshVoiceStatus()
            }
            self.refocus()
        }
    }

    func editVoiceDictionary() {
        var buf = [CChar](repeating: 0, count: 4096)
        _ = cd_voice_dict_get(page, &buf, Int32(buf.count))
        let current = String(cString: buf)
        var d = CoderDialog(size: .md, title: "Custom Voice Dictionary", body: "One rule per line: spoken = Written (or a single canonical word like OpenAgents). Applied automatically when dictation inserts text.")
        d.fields = [.init(value: current, lines: 10)]
        d.buttons = [.init(title: "Cancel"), .init(title: "Save", kind: .primary)]
        d.cancel = 0; d.confirm = 1
        present(d) { [weak self] answer, values in
            guard let self else { return }
            if answer == 1 {
                let raw = values.first ?? ""
                UserDefaults.standard.set(raw, forKey: VoiceDefaults.dictionary)
                _ = cd_voice_dict_set(self.page, raw)
                self.wake()
            }
            self.refocus()
        }
    }

    /// ⌥Space shortcut for voice dictation (Toggle or Push-to-talk).
    func handleVoiceKeyDown(_ event: NSEvent) -> Bool {
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        guard event.keyCode == 49,
              flags.contains(.option),
              !flags.contains(.command),
              !flags.contains(.control),
              !flags.contains(.shift) else { return false }
        if event.isARepeat { return true }
        if cd_voice_mode(page) == VoiceModeCode.hold {
            if cd_voice_state(page) != VoiceStateCode.listening {
                refreshVoiceStatus()
                voice.start(backend: cd_voice_backend(page))
                wake()
            }
        } else {
            toggleVoiceDictation()
        }
        return true
    }

    func handleVoiceKeyUp(_ event: NSEvent) -> Bool {
        guard event.keyCode == 49,
              cd_voice_mode(page) == VoiceModeCode.hold,
              cd_voice_state(page) == VoiceStateCode.listening else { return false }
        voice.stop(cancel: false)
        wake()
        return true
    }
}
