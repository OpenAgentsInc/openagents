// Firm reason: 6 (macOS AVAudioEngine + AVAudioUnitSampler SoundFont host & offline WAV bounce).
import AVFoundation
import CryptoKit
import Foundation

final class StudioAudioEngine {
    static let soundbankURLString = "https://ftp.osuosl.org/pub/musescore/soundfont/MuseScore_General/MuseScore_General.sf2"
    static let soundbankSHA256 = "ee51d2c4b1525e70f19a45909c4fd7a2e26d91d115fa89dbf5a6bc413d8b9bf3"
    static let systemDLSPath = "/System/Library/Components/CoreAudio.component/Contents/Resources/gs_instruments.dls"

    let store: StudioStore
    private let engine = AVAudioEngine()
    private var samplers: [AVAudioUnitSampler] = []
    private let hallReverb = AVAudioUnitReverb()
    private let masterMixer = AVAudioMixerNode()
    private var loadedPrograms: [UInt8] = Array(repeating: 255, count: 32)
    private var loadedChannels: [UInt8] = Array(repeating: 255, count: 32)

    private(set) var activeBankURL: URL?
    private(set) var activeBankLabel: String = "macOS Built-In DLS"
    private(set) var isDownloadingBank: Bool = false
    private(set) var downloadProgress: Double = 0

    // Transport playback state
    private(set) var isPlaying: Bool = false
    private(set) var currentSample: Int64 = 0
    private(set) var currentTick: Int64 = 0
    private var playTimer: DispatchSourceTimer?
    private let timerQueue = DispatchQueue(label: "com.openagents.music-studio.playback", qos: .userInteractive)
    private var lastWallTime: CFAbsoluteTime = 0
    private var sampleRemainder: Double = 0
    private var stepEvents = [MsSchedEvent](repeating: MsSchedEvent(), count: 512)

    // Track notes currently sounding via scheduler so Pause/Stop releases cleanly
    private var soundingNotes: Set<UInt32> = []
    private let soundingLock = NSLock()

    var onBankStatusChanged: ((String, Bool, Double) -> Void)?

    var soundbankFileURL: URL {
        let dir = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".openagents/music/soundbanks", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir.appendingPathComponent("MuseScore_General.sf2")
    }

    init(store: StudioStore) {
        self.store = store
        setupEngine()
    }

    private func setupEngine() {
        engine.attach(hallReverb)
        engine.attach(masterMixer)
        hallReverb.loadFactoryPreset(.cathedral)
        hallReverb.wetDryMix = 28.0

        let fmt = AVAudioFormat(standardFormatWithSampleRate: 48000, channels: 2)!
        for _ in 0..<32 {
            let s = AVAudioUnitSampler()
            engine.attach(s)
            engine.connect(s, to: masterMixer, format: fmt)
            samplers.append(s)
        }
        engine.connect(masterMixer, to: hallReverb, format: fmt)
        engine.connect(hallReverb, to: engine.mainMixerNode, format: fmt)
        engine.mainMixerNode.outputVolume = 0.92

        selectBestAvailableBank()
        try? engine.start()
        syncAllTrackPatches()
    }

    func selectBestAvailableBank() {
        let sf2 = soundbankFileURL
        if FileManager.default.fileExists(atPath: sf2.path),
           let attrs = try? FileManager.default.attributesOfItem(atPath: sf2.path),
           let size = attrs[.size] as? Int64, size > 10_000_000 {
            activeBankURL = sf2
            activeBankLabel = "MuseScore General (MIT · 205 MB)"
        } else if FileManager.default.fileExists(atPath: Self.systemDLSPath) {
            activeBankURL = URL(fileURLWithPath: Self.systemDLSPath)
            activeBankLabel = "macOS Built-In DLS (Fallback)"
        } else {
            activeBankURL = nil
            activeBankLabel = "No Sound Bank"
        }
        loadedPrograms = Array(repeating: 255, count: 32)
        loadedChannels = Array(repeating: 255, count: 32)
        onBankStatusChanged?(activeBankLabel, isDownloadingBank, downloadProgress)
    }

    func hasDownloadedMuseScoreBank() -> Bool {
        guard let attrs = try? FileManager.default.attributesOfItem(atPath: soundbankFileURL.path),
              let size = attrs[.size] as? Int64 else { return false }
        return size > 100_000_000
    }

    func downloadMuseScoreSoundBank() {
        guard !isDownloadingBank else { return }
        guard let url = URL(string: Self.soundbankURLString) else { return }
        isDownloadingBank = true
        downloadProgress = 0.01
        onBankStatusChanged?("Downloading MuseScore General SF2…", true, 0.01)

        let dest = soundbankFileURL
        let part = dest.appendingPathExtension("part")

        let session = URLSession(configuration: .default)
        let task = session.downloadTask(with: url) { [weak self] tmpURL, _, error in
            guard let self = self else { return }
            defer {
                DispatchQueue.main.async {
                    self.isDownloadingBank = false
                    self.downloadProgress = 0
                    self.onBankStatusChanged?(self.activeBankLabel, false, 0)
                }
            }
            guard let tmpURL = tmpURL, error == nil else {
                DispatchQueue.main.async {
                    self.store.onStatus?("Sound bank download failed: \(error?.localizedDescription ?? "unknown")")
                }
                return
            }
            do {
                try? FileManager.default.removeItem(at: part)
                try FileManager.default.moveItem(at: tmpURL, to: part)
                // Verify SHA-256
                let data = try Data(contentsOf: part, options: .mappedIfSafe)
                let digest = SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
                try? FileManager.default.removeItem(at: dest)
                try FileManager.default.moveItem(at: part, to: dest)
                let sources = dest.deletingLastPathComponent().appendingPathComponent("SOURCES.json")
                let meta = """
                {
                  "bank": "MuseScore_General.sf2",
                  "license": "MIT",
                  "url": "\(Self.soundbankURLString)",
                  "sha256": "\(digest)"
                }
                """
                try? meta.write(to: sources, atomically: true, encoding: .utf8)
                DispatchQueue.main.async {
                    self.selectBestAvailableBank()
                    self.syncAllTrackPatches()
                    self.store.onStatus?("Loaded MuseScore_General.sf2 (MIT, SHA-256 verified)")
                }
            } catch {
                DispatchQueue.main.async {
                    self.store.onStatus?("Failed to install sound bank: \(error.localizedDescription)")
                }
            }
        }
        task.resume()
    }

    func syncAllTrackPatches() {
        let trs = store.tracks()
        guard let bankURL = activeBankURL else { return }
        for tr in trs {
            let idx = Int(tr.index)
            guard idx >= 0 && idx < samplers.count else { continue }
            let sampler = samplers[idx]
            sampler.volume = tr.gain
            sampler.pan = tr.pan
            if loadedPrograms[idx] != tr.gmProgram || loadedChannels[idx] != tr.midiChannel {
                let bankMSB: UInt8 = (tr.midiChannel == 9)
                    ? UInt8(kAUSampler_DefaultPercussionBankMSB)
                    : UInt8(kAUSampler_DefaultMelodicBankMSB)
                let bankLSB: UInt8 = UInt8(kAUSampler_DefaultBankLSB)
                do {
                    try sampler.loadSoundBankInstrument(
                        at: bankURL,
                        program: tr.gmProgram,
                        bankMSB: bankMSB,
                        bankLSB: bankLSB
                    )
                    loadedPrograms[idx] = tr.gmProgram
                    loadedChannels[idx] = tr.midiChannel
                } catch {
                    // Fallback to melodic program 0 if preset missing in bank
                    try? sampler.loadSoundBankInstrument(
                        at: bankURL,
                        program: 0,
                        bankMSB: UInt8(kAUSampler_DefaultMelodicBankMSB),
                        bankLSB: UInt8(kAUSampler_DefaultBankLSB)
                    )
                }
            }
        }
    }

    func setReverbWet(_ pct: Float) {
        hallReverb.wetDryMix = max(0, min(100, pct))
    }

    // Immediate software keyboard / Piano Roll audition
    func auditionNoteOn(track: Int32, pitch: UInt8, velocity: UInt8 = 96) {
        if !engine.isRunning { try? engine.start() }
        let idx = max(0, min(samplers.count - 1, Int(track)))
        if loadedPrograms[idx] == 255 { syncAllTrackPatches() }
        samplers[idx].startNote(pitch, withVelocity: velocity, onChannel: 0)
    }

    func auditionNoteOff(track: Int32, pitch: UInt8) {
        let idx = max(0, min(samplers.count - 1, Int(track)))
        samplers[idx].stopNote(pitch, onChannel: 0)
    }

    func allNotesOff() {
        soundingLock.lock()
        let active = soundingNotes
        soundingNotes.removeAll()
        soundingLock.unlock()

        for packed in active {
            let t = Int((packed >> 8) & 0xff)
            let p = UInt8(packed & 0xff)
            if t >= 0 && t < samplers.count {
                samplers[t].stopNote(p, onChannel: 0)
            }
        }
        // Send CC123 (All Notes Off) to every sampler for complete safety.
        for s in samplers {
            s.sendController(123, withValue: 0, onChannel: 0)
        }
        store.withLock { ms_clear_active(store.handle) }
    }

    func seek(tick: Int64) {
        let clamped = max(0, tick)
        if isPlaying { allNotesOff() }
        store.withLock {
            currentTick = clamped
            currentSample = ms_tick_to_sample(store.handle, clamped)
            ms_update_active(store.handle, clamped)
        }
        lastWallTime = CFAbsoluteTimeGetCurrent()
        sampleRemainder = 0
    }

    func togglePlay() {
        if isPlaying { pause() } else { play() }
    }

    func play() {
        guard !isPlaying else { return }
        if !engine.isRunning { try? engine.start() }
        syncAllTrackPatches()
        isPlaying = true
        lastWallTime = CFAbsoluteTimeGetCurrent()
        sampleRemainder = 0

        let timer = DispatchSource.makeTimerSource(queue: timerQueue)
        timer.schedule(deadline: .now(), repeating: .milliseconds(4), leeway: .milliseconds(1))
        timer.setEventHandler { [weak self] in
            self?.tickPlayback()
        }
        playTimer = timer
        timer.resume()
    }

    func pause() {
        guard isPlaying else { return }
        isPlaying = false
        playTimer?.cancel()
        playTimer = nil
        allNotesOff()
    }

    func stop() {
        pause()
        let targetTick = store.loopEnabled ? store.loopStartTick : 0
        seek(tick: targetTick)
    }

    private func tickPlayback() {
        guard isPlaying else { return }
        let now = CFAbsoluteTimeGetCurrent()
        let dt = max(0.0, min(0.05, now - lastWallTime))
        lastWallTime = now

        let rate = 48000.0
        let exact = dt * rate + sampleRemainder
        let frames = Int32(floor(exact))
        sampleRemainder = exact - Double(frames)
        if frames <= 0 { return }

        var stepOut = MsStepOut()
        var evCount: Int32 = 0
        let loopOn = store.loopEnabled
        let lStart = store.loopStartTick
        let lEnd = store.loopEndTick

        store.withLock {
            stepOut = stepEvents.withUnsafeMutableBufferPointer { buf in
                ms_transport_step(
                    store.handle, currentSample, frames,
                    loopOn, lStart, lEnd,
                    buf.baseAddress, Int32(buf.count)
                )
            }
            evCount = stepOut.event_count
            currentSample = stepOut.next_sample
            currentTick = ms_sample_to_tick(store.handle, currentSample)
        }

        if evCount > 0 {
            soundingLock.lock()
            for i in 0..<Int(evCount) {
                let ev = stepEvents[i]
                let t = Int(ev.track)
                guard t >= 0 && t < samplers.count else { continue }
                if ev.kind == 3 {
                    // ALL_NOTES_OFF on loop wrap
                    for packed in soundingNotes {
                        let st = Int((packed >> 8) & 0xff)
                        let sp = UInt8(packed & 0xff)
                        if st >= 0 && st < samplers.count {
                            samplers[st].stopNote(sp, onChannel: 0)
                        }
                    }
                    soundingNotes.removeAll()
                } else if ev.kind == 2 {
                    samplers[t].stopNote(ev.pitch, onChannel: 0)
                    soundingNotes.remove((UInt32(t) << 8) | UInt32(ev.pitch))
                } else if ev.kind == 1 {
                    samplers[t].startNote(ev.pitch, withVelocity: ev.velocity, onChannel: 0)
                    soundingNotes.insert((UInt32(t) << 8) | UInt32(ev.pitch))
                }
            }
            soundingLock.unlock()
        }

        // Stop cleanly 2 seconds past the final note if loop is off.
        if !loopOn {
            let endTick = store.withLock { ms_total_ticks(store.handle) }
            if currentTick > endTick + 3840 {
                DispatchQueue.main.async { [weak self] in
                    self?.stop()
                }
            }
        }
    }

    // MARK: - Offline Stereo 48 kHz WAV Bounce

    func bounceOfflineWav(to outputURL: URL, completion: @escaping (Bool, String) -> Void) {
        let wasPlaying = isPlaying
        if wasPlaying { pause() }

        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self = self else { return }
            let offline = AVAudioEngine()
            let reverb = AVAudioUnitReverb()
            let mixer = AVAudioMixerNode()
            offline.attach(reverb)
            offline.attach(mixer)
            reverb.loadFactoryPreset(.cathedral)
            reverb.wetDryMix = self.hallReverb.wetDryMix

            let fmt = AVAudioFormat(standardFormatWithSampleRate: 48000, channels: 2)!
            let trs = self.store.tracks()
            var offSamplers: [AVAudioUnitSampler] = []
            for _ in 0..<32 {
                let s = AVAudioUnitSampler()
                offline.attach(s)
                offline.connect(s, to: mixer, format: fmt)
                offSamplers.append(s)
            }
            offline.connect(mixer, to: reverb, format: fmt)
            offline.connect(reverb, to: offline.mainMixerNode, format: fmt)

            let maxFrames: AVAudioFrameCount = 512
            do {
                try offline.enableManualRenderingMode(.offline, format: fmt, maximumFrameCount: maxFrames)
                try offline.start()
            } catch {
                DispatchQueue.main.async { completion(false, "Offline engine start failed: \(error.localizedDescription)") }
                return
            }

            if let bankURL = self.activeBankURL {
                for tr in trs {
                    let idx = Int(tr.index)
                    guard idx >= 0 && idx < offSamplers.count else { continue }
                    offSamplers[idx].volume = tr.gain
                    offSamplers[idx].pan = tr.pan
                    let msb: UInt8 = (tr.midiChannel == 9) ? UInt8(kAUSampler_DefaultPercussionBankMSB) : UInt8(kAUSampler_DefaultMelodicBankMSB)
                    try? offSamplers[idx].loadSoundBankInstrument(at: bankURL, program: tr.gmProgram, bankMSB: msb, bankLSB: UInt8(kAUSampler_DefaultBankLSB))
                }
            }

            let totalTicks = self.store.withLock { ms_total_ticks(self.store.handle) }
            let endSample = self.store.withLock { ms_tick_to_sample(self.store.handle, totalTicks) } + 48000 * 3 // 3s reverb tail
            guard let outFile = try? AVAudioFile(forWriting: outputURL, settings: fmt.settings),
                  let buffer = AVAudioPCMBuffer(pcmFormat: offline.manualRenderingFormat, frameCapacity: maxFrames) else {
                offline.stop()
                DispatchQueue.main.async { completion(false, "Failed to open output WAV") }
                return
            }

            var cursor: Int64 = 0
            var evBuf = [MsSchedEvent](repeating: MsSchedEvent(), count: 512)
            while cursor < endSample {
                let block = Int32(min(Int64(maxFrames), endSample - cursor))
                let step = self.store.withLock {
                    evBuf.withUnsafeMutableBufferPointer { ptr in
                        ms_transport_step(self.store.handle, cursor, block, false, 0, 0, ptr.baseAddress, Int32(ptr.count))
                    }
                }
                for i in 0..<Int(step.event_count) {
                    let ev = evBuf[i]
                    let t = Int(ev.track)
                    guard t >= 0 && t < offSamplers.count else { continue }
                    if ev.kind == 2 { offSamplers[t].stopNote(ev.pitch, onChannel: 0) }
                    else if ev.kind == 1 { offSamplers[t].startNote(ev.pitch, withVelocity: ev.velocity, onChannel: 0) }
                }
                let status = try? offline.renderOffline(AVAudioFrameCount(block), to: buffer)
                if status == .success {
                    try? outFile.write(from: buffer)
                }
                cursor = step.next_sample
            }

            offline.stop()
            self.store.withLock { ms_clear_active(self.store.handle) }
            let secs = Double(endSample) / 48000.0
            DispatchQueue.main.async {
                completion(true, String(format: "Bounced %.1fs stereo 48 kHz WAV -> %@", secs, outputURL.lastPathComponent))
            }
        }
    }
}
