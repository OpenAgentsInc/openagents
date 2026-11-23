// Firm reason: 6 (macOS AVAudioEngine: 16-layer Salamander Concert Grand SF2, MuseScore GM SF2,
// native AUv3/AUv2 instrument hosting, warm acoustic EQ, Abbey-Road filtered Hall Reverb, CC64/CC11).
import AppKit
import AudioToolbox
import AVFoundation
import CoreAudioKit
import CryptoKit
import Foundation

struct HostedAUInfo: Equatable {
    let id: String               // "type:subType:mfg"
    let name: String
    let manufacturer: String
    let description: AudioComponentDescription

    var displayName: String { "\(name) (\(manufacturer))" }

    static func == (lhs: HostedAUInfo, rhs: HostedAUInfo) -> Bool {
        lhs.id == rhs.id
    }
}

final class StudioAudioEngine {
    static let orchestraURLString = "https://ftp.osuosl.org/pub/musescore/soundfont/MuseScore_General/MuseScore_General.sf2"
    static let pianoTarXzURLString = "https://freepats.zenvoid.org/Piano/SalamanderGrandPiano/SalamanderGrandPiano-SF2-V3+20200602.tar.xz"
    static let systemDLSPath = "/System/Library/Components/CoreAudio.component/Contents/Resources/gs_instruments.dls"

    let store: StudioStore
    private let engine = AVAudioEngine()

    // Per-track audio strips: each track has a built-in AVAudioUnitSampler AND an optional hosted AUv3/AUv2.
    private var samplers: [AVAudioUnitSampler] = []
    private var hostedAUs: [Int: AVAudioUnitMIDIInstrument] = [:]
    private var hostedAUIds: [Int: String] = [:]
    private var hostedAUWindows: [Int: NSWindow] = [:]
    private var trackMixers: [AVAudioMixerNode] = []

    // Studio acoustic mastering chain:
    //   Track -> Dry Bus -> Warm 3-Band Studio EQ -> Main Mixer
    //         -> Wet Send Bus -> Hall Reverb (.largeHall2) -> Reverb Warmth Filter (HPF 180Hz + LPF 7.5kHz) -> Main Mixer
    private let dryBus = AVAudioMixerNode()
    private let wetSendBus = AVAudioMixerNode()
    private let warmEQ = AVAudioUnitEQ(numberOfBands: 3)
    private let hallReverb = AVAudioUnitReverb()
    private let reverbFilterEQ = AVAudioUnitEQ(numberOfBands: 2)

    private var loadedBankKinds: [UInt8] = Array(repeating: 255, count: 32)
    private var loadedPrograms: [UInt8] = Array(repeating: 255, count: 32)
    private var loadedChannels: [UInt8] = Array(repeating: 255, count: 32)

    private(set) var orchestraBankURL: URL?
    private(set) var concertGrandURL: URL?
    private(set) var activeBankLabel: String = "Loading Sound Banks…"
    private(set) var isDownloadingBank: Bool = false
    private(set) var availableAUs: [HostedAUInfo] = []

    // Live software keyboard sustain pedal state (Tab key or Sustain button)
    private(set) var liveSustainDown: Bool = false
    private var liveSustainTrack: Int = 0

    // Transport playback state
    private(set) var isPlaying: Bool = false
    private(set) var currentSample: Int64 = 0
    private(set) var currentTick: Int64 = 0
    private var playTimer: DispatchSourceTimer?
    private let timerQueue = DispatchQueue(label: "com.openagents.music-studio.playback", qos: .userInteractive)
    private var lastWallTime: CFAbsoluteTime = 0
    private var sampleRemainder: Double = 0
    private var stepEvents = [MsSchedEvent](repeating: MsSchedEvent(), count: 512)

    private var soundingNotes: Set<UInt32> = []
    private let soundingLock = NSLock()

    var onBankStatusChanged: ((String, Bool) -> Void)?
    var onAUsDiscovered: (([HostedAUInfo]) -> Void)?

    var soundbankDirURL: URL {
        let dir = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".openagents/music/soundbanks", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir
    }
    var orchestraFileURL: URL { soundbankDirURL.appendingPathComponent("MuseScore_General.sf2") }
    var concertGrandFileURL: URL { soundbankDirURL.appendingPathComponent("SalamanderGrandPiano.sf2") }

    init(store: StudioStore) {
        self.store = store
        setupAcousticGraph()
        scanInstalledAudioUnits()
    }

    private func setupAcousticGraph() {
        let fmt = AVAudioFormat(standardFormatWithSampleRate: 48000, channels: 2)!

        engine.attach(dryBus)
        engine.attach(wetSendBus)
        engine.attach(warmEQ)
        engine.attach(hallReverb)
        engine.attach(reverbFilterEQ)

        // Warm Acoustic Studio EQ (eliminates thin/harsh GM MIDI sheen)
        let low = warmEQ.bands[0]
        low.filterType = .lowShelf
        low.frequency = 135
        low.gain = 1.6
        low.bypass = false

        let mid = warmEQ.bands[1]
        mid.filterType = .parametric
        mid.frequency = 2750
        mid.bandwidth = 1.1
        mid.gain = -2.2
        mid.bypass = false

        let air = warmEQ.bands[2]
        air.filterType = .highShelf
        air.frequency = 9800
        air.gain = 1.1
        air.bypass = false

        // Abbey-Road filtered Opera Hall Reverb (100% wet on dedicated parallel send bus)
        hallReverb.loadFactoryPreset(.largeHall2)
        hallReverb.wetDryMix = 100.0

        let revHP = reverbFilterEQ.bands[0]
        revHP.filterType = .highPass
        revHP.frequency = 180
        revHP.bypass = false

        let revLP = reverbFilterEQ.bands[1]
        revLP.filterType = .lowPass
        revLP.frequency = 7200
        revLP.bypass = false

        // Wire dry + parallel wet hall return into master output
        engine.connect(dryBus, to: warmEQ, format: fmt)
        engine.connect(warmEQ, to: engine.mainMixerNode, format: fmt)

        engine.connect(wetSendBus, to: hallReverb, format: fmt)
        engine.connect(hallReverb, to: reverbFilterEQ, format: fmt)
        engine.connect(reverbFilterEQ, to: engine.mainMixerNode, format: fmt)
        wetSendBus.outputVolume = 0.34
        engine.mainMixerNode.outputVolume = 0.94

        // Create 32 per-track strips with independent sampler -> trackMixer -> [dryBus, wetSendBus]
        for i in 0..<32 {
            let s = AVAudioUnitSampler()
            let tm = AVAudioMixerNode()
            engine.attach(s)
            engine.attach(tm)
            engine.connect(s, to: tm, format: fmt)
            // Fan out trackMixer to BOTH dryBus (input i) and wetSendBus (input i)
            let dryPoint = AVAudioConnectionPoint(node: dryBus, bus: i)
            let wetPoint = AVAudioConnectionPoint(node: wetSendBus, bus: i)
            engine.connect(tm, to: [dryPoint, wetPoint], fromBus: 0, format: fmt)
            samplers.append(s)
            trackMixers.append(tm)
        }

        selectBestAvailableBanks()
        try? engine.start()
        syncAllTrackPatches()
    }

    // MARK: - Native macOS Audio Unit (AUv3 / AUv2) Discovery & Hosting

    private static func fourCC(_ v: OSType) -> String {
        let bytes: [UInt8] = [
            UInt8((v >> 24) & 0xff), UInt8((v >> 16) & 0xff),
            UInt8((v >> 8) & 0xff), UInt8(v & 0xff),
        ]
        return String(bytes: bytes, encoding: .macOSRoman) ?? String(format: "%08x", v)
    }

    func scanInstalledAudioUnits() {
        let desc = AudioComponentDescription(
            componentType: kAudioUnitType_MusicDevice,
            componentSubType: 0,
            componentManufacturer: 0,
            componentFlags: 0,
            componentFlagsMask: 0
        )
        let comps = AVAudioUnitComponentManager.shared().components(matching: desc)
        var list: [HostedAUInfo] = []
        for c in comps {
            let d = c.audioComponentDescription
            // Skip Apple's internal sampler/DLS since we already host those natively
            let sub = Self.fourCC(d.componentSubType)
            if c.manufacturerName == "Apple" && (sub == "samp" || sub == "dls ") { continue }
            let id = "\(Self.fourCC(d.componentType)):\(sub):\(Self.fourCC(d.componentManufacturer))"
            list.append(HostedAUInfo(id: id, name: c.name, manufacturer: c.manufacturerName, description: d))
        }
        list.sort {
            // Put third-party studio plugins (Spitfire, Decent Sampler, Modartt/Pianoteq, etc.) ahead of Apple stock
            let aApple = $0.manufacturer == "Apple"
            let bApple = $1.manufacturer == "Apple"
            if aApple != bApple { return !aApple }
            return $0.name.localizedCaseInsensitiveCompare($1.name) == .orderedAscending
        }
        availableAUs = list
        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            self.onAUsDiscovered?(self.availableAUs)
        }
    }

    private func instantiateAudioUnit(_ desc: AudioComponentDescription, completion: @escaping (AVAudioUnitMIDIInstrument?, Error?) -> Void) {
        AVAudioUnit.instantiate(with: desc, options: .loadOutOfProcess) { unit, error in
            if let midiUnit = unit as? AVAudioUnitMIDIInstrument, error == nil {
                completion(midiUnit, nil)
                return
            }
            // Classic AUv2 plugins may require in-process loading
            AVAudioUnit.instantiate(with: desc, options: []) { unit2, error2 in
                completion(unit2 as? AVAudioUnitMIDIInstrument, error2 ?? error)
            }
        }
    }

    func loadAudioUnit(_ au: HostedAUInfo, onTrack trackIndex: Int32, completion: ((Bool) -> Void)? = nil) {
        let idx = Int(trackIndex)
        guard idx >= 0 && idx < trackMixers.count else { completion?(false); return }
        store.withLock { ms_set_track_bank(store.handle, trackIndex, 2, au.id) }
        store.onStatus?("Loading Audio Unit \(au.displayName) on track \(idx)…")

        instantiateAudioUnit(au.description) { [weak self] midiUnit, error in
            DispatchQueue.main.async {
                guard let self = self else { return }
                guard let midiUnit = midiUnit, error == nil else {
                    // Revert track bank back to sampler on load failure
                    self.store.withLock { ms_set_track_bank(self.store.handle, trackIndex, 0, nil) }
                    self.store.onStatus?("Failed to load AU \(au.name): \(error?.localizedDescription ?? "not a MIDI instrument")")
                    self.store.onChange?()
                    completion?(false)
                    return
                }
                let fmt = AVAudioFormat(standardFormatWithSampleRate: 48000, channels: 2)!
                let wasRunning = self.engine.isRunning
                if wasRunning { self.engine.pause() }

                // Detach previous hosted AU on this track if any
                if let oldAU = self.hostedAUs.removeValue(forKey: idx) {
                    self.hostedAUWindows.removeValue(forKey: idx)?.close()
                    self.engine.disconnectNodeOutput(oldAU)
                    self.engine.detach(oldAU)
                }
                // Disconnect built-in sampler input on this track so AU owns the channel strip
                self.engine.disconnectNodeOutput(self.samplers[idx])
                self.engine.attach(midiUnit)
                self.engine.connect(midiUnit, to: self.trackMixers[idx], format: fmt)
                self.hostedAUs[idx] = midiUnit
                self.hostedAUIds[idx] = au.id
                self.loadedBankKinds[idx] = 2

                try? self.engine.start()
                self.store.onStatus?("Loaded AU: \(au.displayName) on Track \(idx) — click UI to pick preset!")
                self.store.onChange?()
                completion?(true)
            }
        }
    }

    func switchTrackBackToSampler(trackIndex: Int32, bankKind: UInt8) {
        let idx = Int(trackIndex)
        guard idx >= 0 && idx < samplers.count else { return }
        let fmt = AVAudioFormat(standardFormatWithSampleRate: 48000, channels: 2)!
        if let oldAU = hostedAUs.removeValue(forKey: idx) {
            hostedAUWindows.removeValue(forKey: idx)?.close()
            let wasRunning = engine.isRunning
            if wasRunning { engine.pause() }
            engine.disconnectNodeOutput(oldAU)
            engine.detach(oldAU)
            engine.connect(samplers[idx], to: trackMixers[idx], format: fmt)
            try? engine.start()
        }
        hostedAUIds.removeValue(forKey: idx)
        store.withLock { ms_set_track_bank(store.handle, trackIndex, bankKind, nil) }
        loadedBankKinds[idx] = 255
        loadedPrograms[idx] = 255
        syncAllTrackPatches()
        store.onChange?()
    }

    func openAudioUnitUI(forTrack trackIndex: Int32) {
        let idx = Int(trackIndex)
        guard let unit = hostedAUs[idx] else {
            store.onStatus?("Track \(idx) is using a SoundFont sampler (select an AU plugin to open its UI)")
            return
        }
        if let existing = hostedAUWindows[idx] {
            existing.makeKeyAndOrderFront(nil)
            return
        }
        unit.auAudioUnit.requestViewController { [weak self] vc in
            DispatchQueue.main.async {
                guard let self = self else { return }
                let contentVC: NSViewController
                if let customVC = vc {
                    contentVC = customVC
                } else {
                    let generic = AUGenericViewController()
                    generic.auAudioUnit = unit.auAudioUnit
                    contentVC = generic
                }
                let size = contentVC.preferredContentSize.width > 100 ? contentVC.preferredContentSize : NSSize(width: 720, height: 480)
                let win = NSWindow(
                    contentRect: NSRect(origin: .zero, size: size),
                    styleMask: [.titled, .closable, .resizable],
                    backing: .buffered,
                    defer: false
                )
                win.title = "Track \(idx) — \(unit.name)"
                win.contentViewController = contentVC
                win.center()
                win.isReleasedWhenClosed = false
                self.hostedAUWindows[idx] = win
                win.makeKeyAndOrderFront(nil)
            }
        }
    }

    // MARK: - Sound Bank Discovery & Auto-Downloader

    func hasConcertGrandBank() -> Bool {
        guard let attrs = try? FileManager.default.attributesOfItem(atPath: concertGrandFileURL.path),
              let size = attrs[.size] as? Int64 else { return false }
        return size > 200_000_000
    }

    func hasOrchestraBank() -> Bool {
        guard let attrs = try? FileManager.default.attributesOfItem(atPath: orchestraFileURL.path),
              let size = attrs[.size] as? Int64 else { return false }
        return size > 100_000_000
    }

    func selectBestAvailableBanks() {
        if hasConcertGrandBank() {
            concertGrandURL = concertGrandFileURL
        } else {
            concertGrandURL = nil
        }

        if hasOrchestraBank() {
            orchestraBankURL = orchestraFileURL
        } else if FileManager.default.fileExists(atPath: Self.systemDLSPath) {
            orchestraBankURL = URL(fileURLWithPath: Self.systemDLSPath)
        } else {
            orchestraBankURL = nil
        }

        if concertGrandURL != nil && hasOrchestraBank() {
            activeBankLabel = "Salamander Grand (1.2GB) + MuseScore"
        } else if concertGrandURL != nil {
            activeBankLabel = "Salamander Grand (1.2GB) + DLS"
        } else if hasOrchestraBank() {
            activeBankLabel = "MuseScore GM (Get Salamander Grand)"
        } else {
            activeBankLabel = "Download Piano + Orchestra Banks"
        }

        loadedBankKinds = Array(repeating: 255, count: 32)
        loadedPrograms = Array(repeating: 255, count: 32)
        loadedChannels = Array(repeating: 255, count: 32)
        onBankStatusChanged?(activeBankLabel, isDownloadingBank)
    }

    func downloadMissingSoundBanks() {
        guard !isDownloadingBank else { return }
        isDownloadingBank = true
        onBankStatusChanged?("Downloading Salamander Grand (310 MB)…", true)

        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self = self else { return }
            defer {
                DispatchQueue.main.async {
                    self.isDownloadingBank = false
                    self.selectBestAvailableBanks()
                    self.syncAllTrackPatches()
                }
            }
            if !self.hasConcertGrandBank() {
                let tmp = self.soundbankDirURL.appendingPathComponent("salamander-dl")
                try? FileManager.default.removeItem(at: tmp)
                try? FileManager.default.createDirectory(at: tmp, withIntermediateDirectories: true)
                let tarPath = tmp.appendingPathComponent("salamander.tar.xz").path
                let proc = Process()
                proc.executableURL = URL(fileURLWithPath: "/usr/bin/curl")
                proc.arguments = ["-fsSL", Self.pianoTarXzURLString, "-o", tarPath]
                try? proc.run()
                proc.waitUntilExit()
                if proc.terminationStatus == 0 {
                    let untar = Process()
                    untar.executableURL = URL(fileURLWithPath: "/usr/bin/tar")
                    untar.arguments = ["-xf", tarPath, "-C", tmp.path]
                    try? untar.run()
                    untar.waitUntilExit()
                    if let en = FileManager.default.enumerator(at: tmp, includingPropertiesForKeys: nil) {
                        for case let u as URL in en where u.pathExtension.lowercased() == "sf2" {
                            try? FileManager.default.removeItem(at: self.concertGrandFileURL)
                            try? FileManager.default.moveItem(at: u, to: self.concertGrandFileURL)
                            break
                        }
                    }
                }
                try? FileManager.default.removeItem(at: tmp)
            }
            if !self.hasOrchestraBank() {
                let part = self.orchestraFileURL.appendingPathExtension("part")
                let proc = Process()
                proc.executableURL = URL(fileURLWithPath: "/usr/bin/curl")
                proc.arguments = ["-fsSL", Self.orchestraURLString, "-o", part.path]
                try? proc.run()
                proc.waitUntilExit()
                if proc.terminationStatus == 0 {
                    try? FileManager.default.removeItem(at: self.orchestraFileURL)
                    try? FileManager.default.moveItem(at: part, to: self.orchestraFileURL)
                }
            }
            DispatchQueue.main.async {
                self.store.onStatus?("Sound banks ready: Salamander Grand Piano (16-layer Yamaha C5) + MuseScore Orchestra!")
            }
        }
    }

    func syncAllTrackPatches() {
        let trs = store.tracks()
        for tr in trs {
            let idx = Int(tr.index)
            guard idx >= 0 && idx < samplers.count else { continue }
            trackMixers[idx].outputVolume = tr.gain
            trackMixers[idx].pan = tr.pan

            // If this track hosts a macOS Audio Unit, keep AU active
            if tr.bankKind == 2 && hostedAUs[idx] != nil {
                continue
            }
            // If project was reset/imported and a stale AU is still attached on this strip, detach and restore sampler
            if tr.bankKind != 2, let oldAU = hostedAUs.removeValue(forKey: idx) {
                hostedAUWindows.removeValue(forKey: idx)?.close()
                hostedAUIds.removeValue(forKey: idx)
                let fmt = AVAudioFormat(standardFormatWithSampleRate: 48000, channels: 2)!
                let wasRunning = engine.isRunning
                if wasRunning { engine.pause() }
                engine.disconnectNodeOutput(oldAU)
                engine.detach(oldAU)
                engine.connect(samplers[idx], to: trackMixers[idx], format: fmt)
                if wasRunning { try? engine.start() }
                loadedBankKinds[idx] = 255
                loadedPrograms[idx] = 255
            }

            let wantConcertGrand = (tr.bankKind == 1 && concertGrandURL != nil)
            let bankURL = wantConcertGrand ? concertGrandURL : orchestraBankURL
            guard let url = bankURL else { continue }

            let effectiveKind: UInt8 = wantConcertGrand ? 1 : 0
            let prog: UInt8 = wantConcertGrand ? 0 : tr.gmProgram
            let ch: UInt8 = wantConcertGrand ? 0 : tr.midiChannel

            if loadedBankKinds[idx] != effectiveKind || loadedPrograms[idx] != prog || loadedChannels[idx] != ch {
                let bankMSB: UInt8 = (!wantConcertGrand && ch == 9)
                    ? UInt8(kAUSampler_DefaultPercussionBankMSB)
                    : UInt8(kAUSampler_DefaultMelodicBankMSB)
                let bankLSB: UInt8 = UInt8(kAUSampler_DefaultBankLSB)
                do {
                    try samplers[idx].loadSoundBankInstrument(at: url, program: prog, bankMSB: bankMSB, bankLSB: bankLSB)
                    loadedBankKinds[idx] = effectiveKind
                    loadedPrograms[idx] = prog
                    loadedChannels[idx] = ch
                } catch {
                    if let fallback = orchestraBankURL {
                        try? samplers[idx].loadSoundBankInstrument(at: fallback, program: 0, bankMSB: UInt8(kAUSampler_DefaultMelodicBankMSB), bankLSB: bankLSB)
                    }
                }
            }
        }
    }

    func setReverbWet(_ pct: Float) {
        // Controls the parallel Opera Hall return level smoothly (0..0.85)
        wetSendBus.outputVolume = max(0, min(0.85, pct / 85.0))
    }

    // MARK: - Unified Dispatch to Active Instrument (Sampler OR Hosted AU)

    private func sendNoteOn(track idx: Int, pitch: UInt8, velocity: UInt8) {
        if let au = hostedAUs[idx] {
            au.startNote(pitch, withVelocity: velocity, onChannel: 0)
        } else {
            samplers[idx].startNote(pitch, withVelocity: velocity, onChannel: 0)
        }
    }

    private func sendNoteOff(track idx: Int, pitch: UInt8) {
        if let au = hostedAUs[idx] {
            au.stopNote(pitch, onChannel: 0)
        } else {
            samplers[idx].stopNote(pitch, onChannel: 0)
        }
    }

    private func sendCC(track idx: Int, cc: UInt8, value: UInt8) {
        if let au = hostedAUs[idx] {
            au.sendController(cc, withValue: value, onChannel: 0)
        } else {
            samplers[idx].sendController(cc, withValue: value, onChannel: 0)
        }
    }

    func setLiveSustain(_ down: Bool) {
        liveSustainDown = down
        if down {
            let idx = max(0, min(samplers.count - 1, Int(store.selectedTrack)))
            liveSustainTrack = idx
            sendCC(track: idx, cc: 64, value: 127)
        } else {
            let curIdx = max(0, min(samplers.count - 1, Int(store.selectedTrack)))
            sendCC(track: liveSustainTrack, cc: 64, value: 0)
            if curIdx != liveSustainTrack {
                sendCC(track: curIdx, cc: 64, value: 0)
            }
        }
    }

    func auditionNoteOn(track: Int32, pitch: UInt8, velocity: UInt8 = 92) {
        if !engine.isRunning { try? engine.start() }
        let idx = max(0, min(samplers.count - 1, Int(track)))
        if hostedAUs[idx] == nil && loadedPrograms[idx] == 255 { syncAllTrackPatches() }
        sendNoteOn(track: idx, pitch: pitch, velocity: velocity)
    }

    func auditionNoteOff(track: Int32, pitch: UInt8) {
        let idx = max(0, min(samplers.count - 1, Int(track)))
        sendNoteOff(track: idx, pitch: pitch)
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
                sendNoteOff(track: t, pitch: p)
            }
        }
        // Release sustain pedal (CC64=0) + All Notes Off (CC123=0) on every track!
        for t in 0..<samplers.count {
            sendCC(track: t, cc: 64, value: 0)
            sendCC(track: t, cc: 123, value: 0)
        }
        store.withLock { ms_clear_active(store.handle) }
    }

    // MARK: - Transport Playback

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

        let exact = dt * 48000.0 + sampleRemainder
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
                    // ALL_NOTES_OFF + damper clear on loop wrap
                    for packed in soundingNotes {
                        let st = Int((packed >> 8) & 0xff)
                        let sp = UInt8(packed & 0xff)
                        if st >= 0 && st < samplers.count {
                            sendNoteOff(track: st, pitch: sp)
                        }
                    }
                    soundingNotes.removeAll()
                    for ti in 0..<samplers.count { sendCC(track: ti, cc: 64, value: 0) }
                } else if ev.kind == 4 {
                    // CC64 Sustain Pedal / CC11 Expression / CC1 Mod
                    sendCC(track: t, cc: ev.pitch, value: ev.velocity)
                } else if ev.kind == 2 {
                    sendNoteOff(track: t, pitch: ev.pitch)
                    soundingNotes.remove((UInt32(t) << 8) | UInt32(ev.pitch))
                } else if ev.kind == 1 {
                    sendNoteOn(track: t, pitch: ev.pitch, velocity: ev.velocity)
                    soundingNotes.insert((UInt32(t) << 8) | UInt32(ev.pitch))
                }
            }
            soundingLock.unlock()
        }

        if !loopOn {
            let endTick = store.withLock { ms_total_ticks(store.handle) }
            if currentTick > endTick + 3840 {
                DispatchQueue.main.async { [weak self] in
                    self?.stop()
                }
            }
        }
    }

    // MARK: - Offline Stereo 48 kHz WAV Bounce (includes Salamander Concert Grand + Warm EQ + Hall Reverb + CC64/CC11!)

    func bounceOfflineWav(to outputURL: URL, completion: @escaping (Bool, String) -> Void) {
        let wasPlaying = isPlaying
        if wasPlaying { pause() }

        // Snapshot hosted AU component descriptions + preset states on main thread before offline render
        var auSnapshots: [Int: (AudioComponentDescription, [String: Any]?)] = [:]
        for (idx, unit) in hostedAUs {
            auSnapshots[idx] = (unit.audioComponentDescription, unit.auAudioUnit.fullState)
        }
        let wetLevel = self.wetSendBus.outputVolume

        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self = self else { return }
            let offline = AVAudioEngine()
            let offDry = AVAudioMixerNode()
            let offWet = AVAudioMixerNode()
            let offWarmEQ = AVAudioUnitEQ(numberOfBands: 3)
            let offRev = AVAudioUnitReverb()
            let offRevEQ = AVAudioUnitEQ(numberOfBands: 2)
            let fmt = AVAudioFormat(standardFormatWithSampleRate: 48000, channels: 2)!

            offline.attach(offDry); offline.attach(offWet); offline.attach(offWarmEQ)
            offline.attach(offRev); offline.attach(offRevEQ)

            offWarmEQ.bands[0].filterType = .lowShelf; offWarmEQ.bands[0].frequency = 135; offWarmEQ.bands[0].gain = 1.6; offWarmEQ.bands[0].bypass = false
            offWarmEQ.bands[1].filterType = .parametric; offWarmEQ.bands[1].frequency = 2750; offWarmEQ.bands[1].bandwidth = 1.1; offWarmEQ.bands[1].gain = -2.2; offWarmEQ.bands[1].bypass = false
            offWarmEQ.bands[2].filterType = .highShelf; offWarmEQ.bands[2].frequency = 9800; offWarmEQ.bands[2].gain = 1.1; offWarmEQ.bands[2].bypass = false

            offRev.loadFactoryPreset(.largeHall2); offRev.wetDryMix = 100.0
            offRevEQ.bands[0].filterType = .highPass; offRevEQ.bands[0].frequency = 180; offRevEQ.bands[0].bypass = false
            offRevEQ.bands[1].filterType = .lowPass; offRevEQ.bands[1].frequency = 7200; offRevEQ.bands[1].bypass = false

            offline.connect(offDry, to: offWarmEQ, format: fmt)
            offline.connect(offWarmEQ, to: offline.mainMixerNode, format: fmt)
            offline.connect(offWet, to: offRev, format: fmt)
            offline.connect(offRev, to: offRevEQ, format: fmt)
            offline.connect(offRevEQ, to: offline.mainMixerNode, format: fmt)
            offWet.outputVolume = wetLevel

            let trs = self.store.tracks()
            var offInstruments: [AVAudioUnitMIDIInstrument] = []
            var offSamplers: [Int: AVAudioUnitSampler] = [:]
            var offMixers: [AVAudioMixerNode] = []

            for i in 0..<32 {
                let tm = AVAudioMixerNode()
                offline.attach(tm)
                offline.connect(tm, to: [AVAudioConnectionPoint(node: offDry, bus: i), AVAudioConnectionPoint(node: offWet, bus: i)], fromBus: 0, format: fmt)
                offMixers.append(tm)

                var clonedAU: AVAudioUnitMIDIInstrument? = nil
                if let snap = auSnapshots[i] {
                    let sem = DispatchSemaphore(value: 0)
                    self.instantiateAudioUnit(snap.0) { unit, _ in
                        clonedAU = unit
                        sem.signal()
                    }
                    _ = sem.wait(timeout: .now() + 5.0)
                    if let au = clonedAU {
                        if let st = snap.1 { au.auAudioUnit.fullState = st }
                        offline.attach(au)
                        offline.connect(au, to: tm, format: fmt)
                        offInstruments.append(au)
                        continue
                    }
                }
                let s = AVAudioUnitSampler()
                offline.attach(s)
                offline.connect(s, to: tm, format: fmt)
                offInstruments.append(s)
                offSamplers[i] = s
            }

            let maxFrames: AVAudioFrameCount = 512
            do {
                try offline.enableManualRenderingMode(.offline, format: fmt, maximumFrameCount: maxFrames)
                try offline.start()
            } catch {
                DispatchQueue.main.async { completion(false, "Offline engine start failed: \(error.localizedDescription)") }
                return
            }

            for tr in trs {
                let idx = Int(tr.index)
                guard idx >= 0 && idx < offMixers.count else { continue }
                offMixers[idx].outputVolume = tr.gain
                offMixers[idx].pan = tr.pan
                if let s = offSamplers[idx] {
                    let useGrand = ((tr.bankKind == 1 || (tr.bankKind == 2 && tr.section == 0)) && self.concertGrandURL != nil)
                    if let url = useGrand ? self.concertGrandURL : self.orchestraBankURL {
                        let prog: UInt8 = useGrand ? 0 : tr.gmProgram
                        let msb: UInt8 = (!useGrand && tr.midiChannel == 9) ? UInt8(kAUSampler_DefaultPercussionBankMSB) : UInt8(kAUSampler_DefaultMelodicBankMSB)
                        try? s.loadSoundBankInstrument(at: url, program: prog, bankMSB: msb, bankLSB: UInt8(kAUSampler_DefaultBankLSB))
                    }
                }
            }

            let totalTicks = self.store.withLock { ms_total_ticks(self.store.handle) }
            let endSample = self.store.withLock { ms_tick_to_sample(self.store.handle, totalTicks) } + 48000 * 4
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
                    guard t >= 0 && t < offInstruments.count else { continue }
                    if ev.kind == 4 { offInstruments[t].sendController(ev.pitch, withValue: ev.velocity, onChannel: 0) }
                    else if ev.kind == 2 { offInstruments[t].stopNote(ev.pitch, onChannel: 0) }
                    else if ev.kind == 1 { offInstruments[t].startNote(ev.pitch, withVelocity: ev.velocity, onChannel: 0) }
                }
                let status = try? offline.renderOffline(AVAudioFrameCount(block), to: buffer)
                if status == .success { try? outFile.write(from: buffer) }
                cursor = step.next_sample
            }

            offline.stop()
            self.store.withLock { ms_clear_active(self.store.handle) }
            let secs = Double(endSample) / 48000.0
            DispatchQueue.main.async {
                completion(true, String(format: "Bounced %.1fs 48 kHz Concert Grand + Orchestra WAV -> %@", secs, outputURL.lastPathComponent))
            }
        }
    }
}
