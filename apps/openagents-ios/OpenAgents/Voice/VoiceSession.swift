import AVFoundation
import Foundation
import Synchronization

/// What the voice UI shows. Published from the voice worker; read on main.
struct VoiceSnapshot {
    enum Phase: String { case off, connecting, listening, hearing, thinking, replying }
    var phase = Phase.off
    /// The caption line: partial, then final transcript, then the reply as it's spoken.
    var caption = ""
    enum CaptionKind: String { case none, partial, final, reply, notice }
    var captionKind = CaptionKind.none
    /// The last final transcript, kept while the reply plays.
    var heard = ""
    var reply = ""
    /// The latest uplink and downlink FrameFeatures.
    var uplink = oa_voice_features()
    var downlink = oa_voice_features()
    /// Loudest uplink level this session, 0..1 (the level indicator's high-water mark).
    var peakLevel: Float = 0
    var frames: UInt64 = 0
    /// Turns voice.coax has opened this session.
    var turns = 0
    /// Replies the host has finished sending and timing (CONTROL_KIND_SPEECH_END).
    var replies = 0
    var lastDecision: String = ""
    var provider = ""
    var meter: oa_voice_meter_report?
    var hostFeatures: oa_voice_features?
}

/// The phone's voice host: audio session, capture with Apple's voice
/// processing (echo cancellation), 20 ms frames through `oa_voice_uplink`,
/// the WebSocket to the voice host, reply audio through the jitter buffer and
/// `oa_voice_downlink`, barge-in and frame decisions. voice.coax decides turns,
/// barge-in, gain and policy; this file only moves audio and bytes.
///
/// Threads: the microphone tap only copies into a preallocated ring. One
/// serial worker queue owns the `oa_voice` engine, the wire and the socket's
/// inbound frames; nothing else touches them.
final class VoiceSession: NSObject, URLSessionWebSocketDelegate, @unchecked Sendable {
    // MARK: configuration
    struct Options {
        var host: URL?
        var fixture: String?
        var observe = false
        var route = "foreman"
        var speaker = "ios"
    }
    let options: Options
    /// Called on the worker for every frame's features (uplink and downlink).
    var onFeatures: ((oa_voice_features) -> Void)?
    /// Called on main whenever the snapshot changes in a way captions care about.
    var onChange: (() -> Void)?

    // MARK: engine state (worker only)
    private let queue = DispatchQueue(label: "com.openagents.voice", qos: .userInteractive)
    private var engineMemory: UnsafeMutableRawPointer?
    private var v: OpaquePointer?
    private let wire = VoiceWire()
    private var inputRate: UInt32 = 16000
    private var frameSamples = 320
    private let frameIn = UnsafeMutablePointer<Int16>.allocate(capacity: Int(OA_VOICE_MAX_INPUT_SAMPLES))
    private let frame16k = UnsafeMutablePointer<Int16>.allocate(capacity: Int(OA_VOICE_FRAME_SAMPLES))
    private let downScratch = UnsafeMutablePointer<Int16>.allocate(capacity: Int(OA_VOICE_MAX_DOWNLINK_SAMPLES))
    private let preroll = UnsafeMutablePointer<Int16>.allocate(capacity: Int(OA_VOICE_PREROLL_SAMPLES))
    private let ring = VoiceRing(capacity: 48000 * 2) // two seconds at 48 kHz
    private var timer: DispatchSourceTimer?
    private var turnCount = 0
    private var currentTurn: String?
    private var uplinkSequence: UInt32 = 0
    private var lastUplinkTurn: UInt32 = 0
    private var lastDownlink = oa_voice_features()

    // MARK: audio
    private let audio = AVAudioEngine()
    private let player = AVAudioPlayerNode()
    private let pitch = AVAudioUnitTimePitch()
    private var playerRate: Double = 24000
    private var tapInstalled = false
    private var bridge: RateBridge?
    private var fixture: VoiceFixture?
    private var fixtureStart: Double = 0
    private var fixtureFed = 0
    private var audioRunning = false

    // MARK: reply playback (worker only, except the played counter)
    private var speaking = false          // a Speech is current
    private var speechEnded = false       // its SPEECH_END has arrived
    private var speechStarted = false     // its first real sample was scheduled
    private var speechTurn = ""
    private var speechUtterance = ""
    private var speechText = ""
    private var receivedSamples: UInt64 = 0
    private var scheduledSamples: UInt64 = 0
    private let played = Atomic<UInt64>(0)  // samples the player has played this utterance
    private var playbackGeneration: UInt64 = 0
    private let playGeneration = Atomic<UInt64>(0)
    private var lastProgress: Double = 0
    private var buffers: [AVAudioPCMBuffer] = []
    private var nextBuffer = 0
    private var appliedRate: Float = 1

    // MARK: socket
    private var session: URLSession?
    private var socket: URLSessionWebSocketTask?
    private var connected = false
    private var welcomed = false
    private var backoff: Double = 0.5
    private var running = false

    // MARK: published
    private let state = Mutex(VoiceSnapshot())
    var snapshot: VoiceSnapshot { state.withLock { $0 } }

    init(options: Options) {
        self.options = options
        super.init()
    }
    deinit {
        frameIn.deallocate(); frame16k.deallocate(); downScratch.deallocate(); preroll.deallocate()
        engineMemory?.deallocate()
    }

    // MARK: start / stop (main)

    func start() {
        queue.async { [self] in
            guard !running else { return }
            running = true
            publish { $0 = VoiceSnapshot(); $0.phase = .connecting }
            do {
                try configureSession()
                if let name = options.fixture {
                    let f = try VoiceFixture(named: name)
                    fixture = f
                    setUpEngine(rate: f.rate)
                    fixtureStart = CACurrentMediaTime(); fixtureFed = 0
                    try startPlaybackOnly()
                } else {
                    try startCapture()
                }
            } catch {
                NSLog("OpenAgents voice: audio unavailable: %@", String(describing: error))
                notice("Voice can't use the microphone right now.")
                stopOnQueue(); return
            }
            startTimer()
            connect()
            observeSession()
        }
    }

    func stop() { queue.async { [self] in stopOnQueue() } }

    private func stopOnQueue() {
        guard running else { return }
        running = false
        if let turn = currentTurn { sendControl(OA_CONTROL_CANCEL, turn: turn) }
        currentTurn = nil
        cutPlayback(report: false)
        timer?.cancel(); timer = nil
        if tapInstalled { audio.inputNode.removeTap(onBus: 0); tapInstalled = false }
        audio.stop(); audioRunning = false
        socket?.cancel(with: .normalClosure, reason: nil); socket = nil
        session?.invalidateAndCancel(); session = nil
        connected = false; welcomed = false
        fixture = nil
        NotificationCenter.default.removeObserver(self)
        try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
        publish { $0.phase = .off; $0.caption = ""; $0.captionKind = .none }
    }

    // MARK: audio session and engine

    private func configureSession() throws {
        let s = AVAudioSession.sharedInstance()
        // Hands-free talk: echo-cancelled voice chat that follows Bluetooth
        // headsets, AirPlay and the car, and the loudspeaker otherwise.
        try s.setCategory(.playAndRecord, mode: .voiceChat,
                          options: [.allowBluetoothHFP, .allowBluetoothA2DP, .allowAirPlay, .defaultToSpeaker])
        try s.setPreferredIOBufferDuration(0.01)
        try s.setActive(true)
    }

    private func observeSession() {
        let center = NotificationCenter.default, s = AVAudioSession.sharedInstance()
        center.addObserver(self, selector: #selector(interrupted(_:)), name: AVAudioSession.interruptionNotification, object: s)
        center.addObserver(self, selector: #selector(routeChanged(_:)), name: AVAudioSession.routeChangeNotification, object: s)
        center.addObserver(self, selector: #selector(mediaReset(_:)), name: AVAudioSession.mediaServicesWereResetNotification, object: s)
        center.addObserver(self, selector: #selector(engineChanged(_:)), name: .AVAudioEngineConfigurationChange, object: audio)
    }

    /// (Re)make the engine for an input rate, keeping the media epoch moving forward.
    private func setUpEngine(rate: UInt32) {
        let previous = v.map { oa_voice_epoch($0) } ?? 0
        if engineMemory == nil {
            engineMemory = .allocate(byteCount: oa_voice_size(), alignment: 16)
        }
        engineMemory!.initializeMemory(as: UInt8.self, repeating: 0, count: oa_voice_size())
        v = OpaquePointer(engineMemory!)
        let supported: Set<UInt32> = [16000, 24000, 44100, 48000]
        inputRate = supported.contains(rate) ? rate : 16000
        oa_voice_init(v, inputRate)
        if previous > 0 { _ = oa_voice_set_epoch(v, previous + 1) }
        frameSamples = Int(oa_voice_input_frame_samples(v))
        ring.drain()
    }

    private func attachPlayer(rate: Double) {
        if audio.attachedNodes.contains(player) == false {
            audio.attach(player); audio.attach(pitch)
        }
        let format = AVAudioFormat(standardFormatWithSampleRate: rate, channels: 1)!
        audio.connect(player, to: pitch, format: format)
        audio.connect(pitch, to: audio.mainMixerNode, format: format)
        playerRate = rate
        buffers = (0..<16).map { _ in AVAudioPCMBuffer(pcmFormat: format, frameCapacity: AVAudioFrameCount(OA_VOICE_MAX_DOWNLINK_SAMPLES))! }
        nextBuffer = 0
    }

    private func startPlaybackOnly() throws {
        attachPlayer(rate: playerRate)
        audio.prepare()
        try audio.start(); audioRunning = true
        player.play()
    }

    private func startCapture() throws {
        let input = audio.inputNode
        // Apple's voice processing: echo cancellation against our own
        // playback, automatic gain and noise suppression, on the device.
        if !input.isVoiceProcessingEnabled {
            do { try input.setVoiceProcessingEnabled(true) } catch {
                // Some simulators have no voice processing unit; capture still works.
                NSLog("OpenAgents voice: no voice processing (%@); capturing without echo cancellation", error.localizedDescription)
            }
        }
        attachPlayer(rate: playerRate)
        try installTap()
        audio.prepare()
        try audio.start(); audioRunning = true
        player.play()
    }

    private func installTap() throws {
        let input = audio.inputNode
        if tapInstalled { input.removeTap(onBus: 0); tapInstalled = false }
        let format = input.outputFormat(forBus: 0)
        guard format.sampleRate > 0, format.channelCount > 0 else { throw VoiceFailure.noInput }
        let hw = UInt32(format.sampleRate.rounded())
        let supported: Set<UInt32> = [16000, 24000, 44100, 48000]
        if supported.contains(hw) {
            bridge = nil; setUpEngine(rate: hw)
        } else {
            // A rate the engine doesn't take (8 kHz headsets): Apple's
            // converter brings it to 16 kHz on the worker, never in the tap.
            bridge = RateBridge(from: format.sampleRate); setUpEngine(rate: 16000)
        }
        let ring = self.ring
        input.installTap(onBus: 0, bufferSize: AVAudioFrameCount(format.sampleRate / 50), format: format) { buffer, _ in
            // The tap copies into the preallocated ring and nothing else.
            guard let channel = buffer.floatChannelData?[0] else { return }
            ring.write(floats: channel, count: Int(buffer.frameLength))
        }
        tapInstalled = true
    }

    @objc private func interrupted(_ note: Notification) {
        guard let raw = note.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt,
              let kind = AVAudioSession.InterruptionType(rawValue: raw) else { return }
        queue.async { [self] in
            guard running else { return }
            switch kind {
            case .began:
                // A call or Siri took the audio: drop the turn and the reply.
                if let turn = currentTurn { oa_voice_cancel_turn(v); sendControl(OA_CONTROL_CANCEL, turn: turn); currentTurn = nil }
                cutPlayback(report: true)
                audio.pause(); audioRunning = false
                notice("Voice paused.")
            case .ended:
                let opts = (note.userInfo?[AVAudioSessionInterruptionOptionKey] as? UInt).map(AVAudioSession.InterruptionOptions.init) ?? []
                if opts.contains(.shouldResume) { restartAudio() }
            @unknown default: break
            }
        }
    }

    @objc private func routeChanged(_ note: Notification) {
        guard let raw = note.userInfo?[AVAudioSessionRouteChangeReasonKey] as? UInt,
              let reason = AVAudioSession.RouteChangeReason(rawValue: raw) else { return }
        switch reason {
        case .newDeviceAvailable, .oldDeviceUnavailable, .routeConfigurationChange:
            // AirPods in or out, the car, the speaker: the hardware rate may change.
            queue.async { [self] in
                guard running else { return }
                NSLog("OpenAgents voice: route changed (%lu)", raw)
                restartAudio()
            }
        default: break
        }
    }

    @objc private func engineChanged(_ note: Notification) {
        queue.async { [self] in if running { restartAudio() } }
    }

    @objc private func mediaReset(_ note: Notification) {
        queue.async { [self] in
            guard running else { return }
            try? configureSession()
            restartAudio()
        }
    }

    private func restartAudio() {
        if let turn = currentTurn { oa_voice_cancel_turn(v); sendControl(OA_CONTROL_CANCEL, turn: turn); currentTurn = nil }
        cutPlayback(report: true)
        audio.stop(); audioRunning = false
        do {
            if fixture == nil { try installTap() }
            audio.prepare()
            try audio.start(); audioRunning = true
            player.play()
            refreshPhase()
        } catch {
            NSLog("OpenAgents voice: audio restart failed: %@", String(describing: error))
            notice("Voice lost the microphone. Turn it off and on again.")
        }
    }

    // MARK: the worker tick

    private func startTimer() {
        let t = DispatchSource.makeTimerSource(flags: .strict, queue: queue)
        t.schedule(deadline: .now(), repeating: .milliseconds(10), leeway: .milliseconds(2))
        t.setEventHandler { [weak self] in self?.tick() }
        t.resume(); timer = t
    }

    private func tick() {
        guard running, v != nil else { return }
        feedFixture()
        if let bridge {
            while let n = bridge.pull(from: ring, into: frameIn, frame: frameSamples) { _ = n; uplink() }
        } else {
            while ring.read(into: frameIn, count: frameSamples) { uplink() }
        }
        feedPlayer()
    }

    /// The fixture plays into the ring in real time, after a short lead-in
    /// for the noise floor, then silence: exactly what the tap would write.
    private func feedFixture() {
        guard let fixture else { return }
        let lead = Int(fixture.rate) * 3 / 10
        let due = Int((CACurrentMediaTime() - fixtureStart) * Double(fixture.rate))
        var chunk = [Int16](repeating: 0, count: 0)
        while fixtureFed < due {
            let n = min(due - fixtureFed, frameSamples)
            chunk.removeAll(keepingCapacity: true)
            for i in 0..<n {
                let at = fixtureFed + i - lead
                chunk.append(at >= 0 && at < fixture.samples.count ? fixture.samples[at] : 0)
            }
            chunk.withUnsafeBufferPointer { _ = ring.write($0.baseAddress!, count: n) }
            fixtureFed += n
        }
    }

    private func uplink() {
        var f = oa_voice_features()
        let ev = oa_voice_uplink(v, frameIn, frame16k, &f)
        lastUplinkTurn = f.turn_state
        onFeatures?(f)
        let level = Self.level(f.rms_db)
        publish { s in
            s.uplink = f; s.frames &+= 1
            if level > s.peakLevel { s.peakLevel = level }
        }
        if ev & UInt32(OA_EV_BARGE_IN) != 0 { bargeIn() }
        if ev & UInt32(OA_EV_TURN_START) != 0 { beginTurn() }
        if ev & UInt32(OA_EV_FORWARD | OA_EV_TURN_START) != 0, ev & UInt32(OA_EV_DROPPED) == 0, let turn = currentTurn, welcomed {
            sendAudio(turn: turn, samples: frame16k, count: Int(OA_VOICE_FRAME_SAMPLES))
        }
        if ev & UInt32(OA_EV_DECISION) != 0 { appliedPolicyDecision() }
        if ev & UInt32(OA_EV_TURN_END) != 0 { endTurn(tooLong: ev & UInt32(OA_EV_TOO_LONG) != 0) }
        if options.observe, welcomed, f.sequence % 5 == 0 {
            // The dev console's trace: every fifth uplink frame's measurements.
            var m = oa_voice_msg(); m.kind = UInt32(OA_MSG_FEATURES.rawValue); m.u.features = f
            send(&m)
        }
    }

    static func level(_ db: Float) -> Float { min(max((db + 60) / 60, 0), 1) }

    private func beginTurn() {
        // Speech before the host has answered isn't a turn anyone can hear.
        guard welcomed else { return }
        turnCount += 1
        let turn = "\(options.speaker)-\(turnCount)"
        currentTurn = turn
        sendControl(OA_CONTROL_TURN_START, turn: turn)
        // The onset voice.coax heard before it decided, so recognition gets the first syllable.
        let n = Int(oa_voice_preroll(v, preroll, UInt32(OA_VOICE_PREROLL_SAMPLES)))
        var off = 0
        while off < n {
            let k = min(n - off, Int(OA_VOICE_MAX_AUDIO_BYTES) / 2)
            sendAudio(turn: turn, samples: preroll + off, count: k)
            off += k
        }
        publish { s in s.caption = ""; s.captionKind = .partial; s.turns += 1 }
        refreshPhase()
    }

    private func endTurn(tooLong: Bool) {
        guard let turn = currentTurn else { return }
        currentTurn = nil
        sendControl(OA_CONTROL_TURN_END, turn: turn)
        publish { s in s.phase = .thinking }
        main()
        if tooLong { NSLog("OpenAgents voice: turn %@ hit the longest turn", turn) }
    }

    private func appliedPolicyDecision() {
        var d = oa_voice_decision()
        guard oa_voice_last_decision(v, &d) else { return }
        apply(d, decider: VoiceWire.string(d.decider))
    }

    /// The engine has already taken the decision (gain, duck, pause and rate
    /// are its state, applied in oa_voice_downlink). The host acts on the
    /// parts that are platform effects: cutting the player and its rate.
    private func apply(_ d: oa_voice_decision, decider: String) {
        if d.action == UInt32(OA_FRAME_ACTION_BARGE_IN.rawValue), speaking { bargeIn() }
        let name = Self.actionName(d.action)
        publish { $0.lastDecision = "\(name) from \(decider) at \(d.apply_from_sequence)" }
    }

    static func actionName(_ a: UInt32) -> String {
        ["keep", "hold turn", "end turn", "barge in", "duck", "pause", "resume", "drop"][Int(min(a, 7))]
    }

    // MARK: barge-in and playback

    private func bargeIn() {
        guard speaking else { return }
        let reportTurn = speechTurn, utterance = speechUtterance
        let heard = played.load(ordering: .acquiring)
        stopPlayer()
        oa_voice_jitter_flush(v)
        let epoch = oa_voice_epoch(v)
        var m = oa_voice_msg(); m.kind = UInt32(OA_MSG_CONTROL.rawValue)
        m.u.control.kind = UInt32(OA_CONTROL_BARGE_IN.rawValue)
        m.u.control.turn = wire.text(reportTurn)
        m.u.control.played_samples = heard
        m.u.control.media_epoch = epoch
        m.u.control.utterance = wire.text(utterance)
        m.u.control.at_ms = UInt64(CACurrentMediaTime() * 1000)
        send(&m)
        speaking = false
        NSLog("OpenAgents voice: barge-in after %llu samples (epoch %llu)", heard, epoch)
        refreshPhase()
    }

    /// Stops the player at once: everything scheduled is dropped.
    private func stopPlayer() {
        playbackGeneration &+= 1
        playGeneration.store(playbackGeneration, ordering: .releasing)
        player.stop()
        if audioRunning { player.play() }
        oa_voice_playing(v, false)
    }

    private func cutPlayback(report: Bool) {
        guard speaking else { stopPlayer(); return }
        if report { bargeIn() } else {
            stopPlayer(); _ = oa_voice_cut(v); speaking = false
        }
    }

    /// Keeps about 60 ms of reply scheduled: small enough that duck, pause
    /// and rate decisions are heard at once, large enough not to starve.
    private func feedPlayer() {
        guard speaking, audioRunning else { return }
        let chunk = Int(playerRate / 50)
        let ahead = Int(playerRate * 0.06)
        let heard = played.load(ordering: .acquiring)
        oa_voice_playing(v, speechStarted && scheduledSamples > heard)
        let rate = oa_voice_playback_rate(v)
        if rate != appliedRate, rate > 0 { pitch.rate = rate; appliedRate = rate }
        while Int(scheduledSamples) - Int(heard) < ahead {
            let ev = oa_voice_jitter_pop(v, downScratch, UInt32(chunk))
            let underrun = ev & UInt32(OA_EV_UNDERRUN) != 0
            if underrun && (!speechStarted || (speechEnded && oa_voice_jitter_depth(v) == 0)) { break }
            var f = oa_voice_features()
            _ = oa_voice_downlink(v, downScratch, UInt32(chunk), UInt32(playerRate), &f)
            onFeatures?(f); lastDownlink = f
            schedule(chunk)
            speechStarted = true
        }
        let done = speechEnded && speechStarted && oa_voice_jitter_depth(v) == 0 && heard >= scheduledSamples
        let now = CACurrentMediaTime()
        if now - lastProgress > 0.25 || done {
            lastProgress = now
            var m = oa_voice_msg(); m.kind = UInt32(OA_MSG_CONTROL.rawValue)
            m.u.control.kind = UInt32(OA_CONTROL_PLAYBACK.rawValue)
            m.u.control.turn = wire.text(speechTurn)
            m.u.control.played_samples = heard
            m.u.control.media_epoch = oa_voice_epoch(v)
            m.u.control.utterance = wire.text(speechUtterance)
            m.u.control.at_ms = UInt64(now * 1000)
            send(&m)
        }
        // The reply caption follows playback, word by word.
        let total = max(receivedSamples, 1)
        let fraction = done ? 1 : min(Double(heard) / Double(total), 1)
        let words = speechText.split(separator: " ", omittingEmptySubsequences: true)
        let shown = done ? speechText : words.prefix(Int((Double(words.count) * fraction).rounded(.up))).joined(separator: " ")
        let downlink = lastDownlink
        publish { s in
            if s.captionKind == .reply, s.caption != shown { s.caption = shown }
            s.downlink = downlink
        }
        if done {
            speaking = false
            oa_voice_playing(v, false)
            refreshPhase()
        }
    }

    private func schedule(_ n: Int) {
        let buffer = buffers[nextBuffer]; nextBuffer = (nextBuffer + 1) % buffers.count
        buffer.frameLength = AVAudioFrameCount(n)
        let out = buffer.floatChannelData![0]
        for i in 0..<n { out[i] = Float(downScratch[i]) / 32768 }
        scheduledSamples += UInt64(n)
        let generation = playbackGeneration
        player.scheduleBuffer(buffer, completionCallbackType: .dataPlayedBack) { [weak self] _ in
            guard let self, self.playGeneration.load(ordering: .acquiring) == generation else { return }
            self.played.wrappingAdd(UInt64(n), ordering: .acquiringAndReleasing)
        }
    }

    // MARK: socket

    private func connect() {
        guard running, socket == nil else { return }
        guard let url = options.host else {
            notice("Voice needs a voice server. Hold the voice button to set one.")
            return
        }
        publish { $0.phase = .connecting }
        main()
        let session = URLSession(configuration: .ephemeral, delegate: self, delegateQueue: nil)
        let task = session.webSocketTask(with: url)
        task.maximumMessageSize = Int(OA_VOICE_MAX_FRAME_BYTES)
        self.session = session; socket = task
        task.resume()
        receive(task)
    }

    func urlSession(_ session: URLSession, webSocketTask: URLSessionWebSocketTask, didOpenWithProtocol protocol: String?) {
        queue.async { [self] in
            guard webSocketTask === socket else { return }
            connected = true; backoff = 0.5
            sendHello()
        }
    }

    func urlSession(_ session: URLSession, webSocketTask: URLSessionWebSocketTask, didCloseWith closeCode: URLSessionWebSocketTask.CloseCode, reason: Data?) {
        queue.async { [self] in if webSocketTask === socket { lost() } }
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        queue.async { [self] in if task === socket { lost() } }
    }

    private func receive(_ task: URLSessionWebSocketTask) {
        task.receive { [weak self] result in
            guard let self else { return }
            switch result {
            case .success(.data(let data)):
                self.queue.async { if task === self.socket { self.handle(data) } }
                self.receive(task)
            case .success:
                self.receive(task) // text frames aren't part of the wire
            case .failure:
                self.queue.async { if task === self.socket { self.lost() } }
            }
        }
    }

    /// The connection dropped: drop the turn and the reply, then retry with backoff.
    private func lost() {
        guard running else { return }
        socket?.cancel(); socket = nil
        session?.invalidateAndCancel(); session = nil
        connected = false; welcomed = false
        if currentTurn != nil { oa_voice_cancel_turn(v); currentTurn = nil }
        cutPlayback(report: false)
        let wait = backoff; backoff = min(backoff * 2, 10)
        notice("Voice needs a connection. Trying again…")
        queue.asyncAfter(deadline: .now() + wait) { [weak self] in self?.connect() }
    }

    private func sendHello() {
        var m = oa_voice_msg(); m.kind = UInt32(OA_MSG_HELLO.rawValue)
        m.u.hello.`protocol` = UInt32(OA_VOICE_PROTOCOL)
        m.u.hello.client = UInt32(OA_CLIENT_IOS.rawValue)
        m.u.hello.has_speaker = true
        m.u.hello.speaker.id = wire.text(options.speaker)
        m.u.hello.speaker.role = UInt32(OA_SPEAKER_ROLE_PLAYER.rawValue)
        m.u.hello.route = wire.text(options.route)
        m.u.hello.input_sample_rate = UInt32(OA_VOICE_RATE)
        m.u.hello.output_sample_rate = 24000
        m.u.hello.codec = UInt32(OA_CODEC_PCM_S16LE.rawValue)
        m.u.hello.observe = options.observe
        send(&m)
    }

    private func send(_ m: inout oa_voice_msg) {
        guard connected, let socket else { return }
        guard let data = wire.encode(&m) else { NSLog("OpenAgents voice: frame kind %u refused by the encoder", m.kind); return }
        socket.send(.data(data)) { error in if let error { NSLog("OpenAgents voice: send failed: %@", error.localizedDescription) } }
    }

    private func sendControl(_ kind: oa_voice_control_kind, turn: String) {
        var m = oa_voice_msg(); m.kind = UInt32(OA_MSG_CONTROL.rawValue)
        m.u.control.kind = UInt32(kind.rawValue)
        m.u.control.turn = wire.text(turn)
        m.u.control.media_epoch = v.map { oa_voice_epoch($0) } ?? 0
        m.u.control.at_ms = UInt64(CACurrentMediaTime() * 1000)
        send(&m)
    }

    private func sendAudio(turn: String, samples: UnsafePointer<Int16>, count: Int) {
        var m = oa_voice_msg(); m.kind = UInt32(OA_MSG_AUDIO.rawValue)
        m.u.audio.turn = wire.text(turn)
        m.u.audio.sequence = uplinkSequence; uplinkSequence &+= 1
        m.u.audio.sample_rate = UInt32(OA_VOICE_RATE)
        m.u.audio.codec = UInt32(OA_CODEC_PCM_S16LE.rawValue)
        m.u.audio.data = oa_voice_bytes(ptr: UnsafeRawPointer(samples).assumingMemoryBound(to: UInt8.self), len: UInt32(count * 2))
        m.u.audio.media_epoch = oa_voice_epoch(v)
        send(&m)
    }

    private func handle(_ data: Data) {
        guard let inbound = VoiceInbound.decode(data) else {
            NSLog("OpenAgents voice: dropped a malformed frame (%ld bytes)", data.count); return
        }
        switch inbound {
        case let .welcome(sessionID, provider, outputRate, _, meter):
            welcomed = true
            NSLog("OpenAgents voice: welcome %@ (%@, %u Hz)", sessionID, provider, outputRate)
            publish { s in s.provider = provider; s.meter = meter; if s.captionKind == .notice { s.caption = ""; s.captionKind = .none } }
            refreshPhase()
        case let .turn(_, phase, text, _):
            switch phase {
            case UInt32(OA_TURN_PHASE_PARTIAL.rawValue):
                let replying = speaking
                publish { s in if !(s.captionKind == .reply && replying) { s.caption = text; s.captionKind = .partial } }
            case UInt32(OA_TURN_PHASE_FINAL.rawValue):
                publish { s in s.caption = text; s.captionKind = .final; s.heard = text }
                NSLog("OpenAgents voice: heard \"%@\"", text)
            case UInt32(OA_TURN_PHASE_ENDED.rawValue), UInt32(OA_TURN_PHASE_INTERRUPTED.rawValue):
                // A turn with no reply (nothing heard, or cut): back to listening.
                if !speaking { publish { s in if s.phase == .thinking { s.phase = .listening } } }
            default: break
            }
            main()
        case let .speech(text, utterance, rate, epoch, turn):
            guard epoch == 0 || epoch >= oa_voice_epoch(v) else { return } // an old reply
            if speaking, utterance == speechUtterance, !utterance.isEmpty {
                // The same utterance again: more of its text is known (a reply
                // streamed sentence by sentence). Playback carries on.
                speechText = text
                publish { s in s.reply = text }
                return
            }
            if epoch > 0 { _ = oa_voice_set_epoch(v, epoch) }
            if speaking { stopPlayer() }
            if rate > 0, Double(rate) != playerRate {
                player.stop(); attachPlayer(rate: Double(rate)); if audioRunning { player.play() }
            }
            played.store(0, ordering: .releasing)
            speaking = true; speechEnded = false; speechStarted = false
            speechTurn = turn; speechUtterance = utterance; speechText = text
            receivedSamples = 0; scheduledSamples = 0
            publish { s in s.reply = text; s.caption = ""; s.captionKind = .reply; s.phase = .replying }
            main()
        case let .audio(sequence, _, epoch, _, samples):
            guard speaking, !samples.isEmpty, samples.count <= Int(OA_VOICE_MAX_DOWNLINK_SAMPLES) else { return }
            let kept = samples.withUnsafeBufferPointer { oa_voice_jitter_push(v, sequence, epoch, $0.baseAddress, UInt32($0.count)) }
            if kept { receivedSamples += UInt64(samples.count) }
        case let .control(kind, _, _, _):
            if kind == UInt32(OA_CONTROL_SPEECH_END.rawValue) { speechEnded = true; publish { $0.replies += 1 } }
            if kind == UInt32(OA_CONTROL_CANCEL.rawValue) { cutPlayback(report: false); refreshPhase() }
            if kind == UInt32(OA_CONTROL_PING.rawValue) { sendControl(OA_CONTROL_PONG, turn: "") }
        case .route:
            break
        case let .meter(report):
            publish { $0.meter = report }
        case let .error(code, message, _):
            NSLog("OpenAgents voice: host error %u: %@", code, message)
            notice(Self.words(for: code))
            publish { s in if s.phase == .thinking { s.phase = .listening } }
        case let .features(f):
            publish { $0.hostFeatures = f }
        case let .decision(d, decider):
            var kept = d
            kept.decider = wire.text(decider)
            if oa_voice_decide(v, &kept) { apply(d, decider: decider) }
            wire.reset()
        }
    }

    static func words(for code: UInt32) -> String {
        switch code {
        case UInt32(OA_ERROR_LIMIT.rawValue): return "Today's free voice time is used up."
        case UInt32(OA_ERROR_PROVIDER.rawValue): return "Speech isn't working right now. Try again in a moment."
        case UInt32(OA_ERROR_TOO_LONG.rawValue): return "That was too long for one turn. Try a shorter one."
        case UInt32(OA_ERROR_UNAUTHORIZED.rawValue): return "This phone isn't allowed to use that voice server."
        default: return "Voice hit a problem. Try again."
        }
    }

    // MARK: publishing

    private func refreshPhase() {
        let t = lastUplinkTurn
        publish { s in
            if !self.running { s.phase = .off }
            else if !self.welcomed { s.phase = .connecting }
            else if self.speaking { s.phase = .replying }
            else if t == UInt32(OA_TURN_STATE_SPEAKING.rawValue) || t == UInt32(OA_TURN_STATE_TRAILING.rawValue) { s.phase = .hearing }
            else if s.phase == .thinking { }
            else { s.phase = .listening }
        }
        main()
    }

    private func notice(_ text: String) {
        publish { s in s.caption = text; s.captionKind = .notice }
        main()
    }

    private func publish(_ change: (inout VoiceSnapshot) -> Void) { state.withLock { change(&$0) } }
    private func main() { DispatchQueue.main.async { [weak self] in self?.onChange?() } }
}

enum VoiceFailure: Error { case noInput }

/// Brings an input rate the engine doesn't take (8 kHz Bluetooth headsets,
/// for one) to 16 kHz with Apple's converter, on the worker. Buffers are
/// made once.
final class RateBridge {
    private let converter: AVAudioConverter
    private let inBuffer: AVAudioPCMBuffer
    private let outBuffer: AVAudioPCMBuffer
    private var pending: [Int16] = []
    private let inFrame: Int

    init?(from rate: Double) {
        guard let src = AVAudioFormat(commonFormat: .pcmFormatInt16, sampleRate: rate, channels: 1, interleaved: true),
              let dst = AVAudioFormat(commonFormat: .pcmFormatInt16, sampleRate: 16000, channels: 1, interleaved: true),
              let c = AVAudioConverter(from: src, to: dst),
              let i = AVAudioPCMBuffer(pcmFormat: src, frameCapacity: AVAudioFrameCount(rate / 50)),
              let o = AVAudioPCMBuffer(pcmFormat: dst, frameCapacity: 1024) else { return nil }
        converter = c; inBuffer = i; outBuffer = o; inFrame = Int(rate / 50)
        pending.reserveCapacity(4096)
    }

    /// One 16 kHz frame into `out`, or nil when not enough input is buffered yet.
    func pull(from ring: VoiceRing, into out: UnsafeMutablePointer<Int16>, frame: Int) -> Int? {
        while pending.count < frame {
            guard ring.read(into: inBuffer.int16ChannelData![0], count: inFrame) else { return nil }
            inBuffer.frameLength = AVAudioFrameCount(inFrame)
            var given = false
            outBuffer.frameLength = 0
            _ = converter.convert(to: outBuffer, error: nil) { _, status in
                if given { status.pointee = .noDataNow; return nil }
                given = true; status.pointee = .haveData; return self.inBuffer
            }
            let o = outBuffer.int16ChannelData![0]
            pending.append(contentsOf: UnsafeBufferPointer(start: o, count: Int(outBuffer.frameLength)))
        }
        pending.withUnsafeBufferPointer { out.update(from: $0.baseAddress!, count: frame) }
        pending.removeFirst(frame)
        return frame
    }
}
