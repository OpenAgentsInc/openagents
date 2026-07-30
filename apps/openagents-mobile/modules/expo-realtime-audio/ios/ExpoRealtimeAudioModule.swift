import AVFoundation
import ExpoModulesCore

private let maximumQueuedSeconds: Int64 = 120

public final class ExpoRealtimeAudioModule: Module {
  private let engine = AVAudioEngine()
  private let player = AVAudioPlayerNode()
  private let stateLock = NSLock()
  private var format: AVAudioFormat?
  private var queuedFrames: Int64 = 0
  private var sampleRate: Double = 24_000
  private var started = false
  private var interrupted = false
  private var playbackGeneration: Int64 = 0
  private var interruptionObserver: NSObjectProtocol?
  private var mediaResetObserver: NSObjectProtocol?
  private var microphoneEngine: AVAudioEngine?
  private var microphoneConverter: AVAudioConverter?
  private var microphoneStarted = false

  public func definition() -> ModuleDefinition {
    Name("ExpoRealtimeAudio")
    Events("onMicrophoneBuffer")

    Function("start") { (requestedSampleRate: Int) throws in
      try self.start(sampleRate: requestedSampleRate)
    }

    Function("enqueue") { (pcm16Base64: String) throws -> Int in
      try self.enqueue(pcm16Base64: pcm16Base64)
    }

    Function("flush") {
      self.flush()
    }

    Function("stop") {
      self.stop()
    }

    Function("getStatus") {
      self.status()
    }

    Function("startMicrophone") { (requestedSampleRate: Int) throws in
      try self.startMicrophone(sampleRate: requestedSampleRate)
    }

    Function("stopMicrophone") {
      self.stopMicrophone()
    }

    Function("isMicrophoneStarted") {
      self.stateLock.lock()
      let currentStarted = self.microphoneStarted
      self.stateLock.unlock()
      return currentStarted
    }

    OnDestroy {
      self.stopMicrophone()
      self.stop()
    }
  }

  private func startMicrophone(sampleRate requestedSampleRate: Int) throws {
    guard requestedSampleRate == 24_000 else {
      throw Exception(name: "unsupported_sample_rate", description: "Sarah voice requires 24 kHz PCM.")
    }
    stateLock.lock()
    let alreadyStarted = microphoneStarted
    stateLock.unlock()
    if alreadyStarted { return }

    let session = AVAudioSession.sharedInstance()
    let microphoneEngine = AVAudioEngine()
    do {
      try session.setCategory(
        .playAndRecord,
        mode: .voiceChat,
        options: [.defaultToSpeaker, .allowBluetoothHFP]
      )
      try session.setPreferredSampleRate(Double(requestedSampleRate))
      try session.setActive(true)

      let inputNode = microphoneEngine.inputNode
      let hardwareFormat = inputNode.outputFormat(forBus: 0)
      guard hardwareFormat.sampleRate > 0, hardwareFormat.channelCount > 0 else {
        throw Exception(name: "microphone_unavailable", description: "The microphone input is unavailable.")
      }
      guard let targetFormat = AVAudioFormat(
        commonFormat: .pcmFormatInt16,
        sampleRate: Double(requestedSampleRate),
        channels: 1,
        interleaved: true
      ), let converter = AVAudioConverter(from: hardwareFormat, to: targetFormat) else {
        throw Exception(name: "unsupported_microphone_format", description: "The microphone format is unavailable.")
      }

      let bufferSize = AVAudioFrameCount(hardwareFormat.sampleRate / 10)
      microphoneConverter = converter
      inputNode.installTap(onBus: 0, bufferSize: bufferSize, format: hardwareFormat) {
        [weak self] inputBuffer, _ in
        self?.convertAndEmitMicrophoneBuffer(
          inputBuffer: inputBuffer,
          converter: converter,
          targetFormat: targetFormat
        )
      }
      microphoneEngine.prepare()
      try microphoneEngine.start()
      self.microphoneEngine = microphoneEngine
      stateLock.lock()
      microphoneStarted = true
      stateLock.unlock()
    } catch {
      microphoneEngine.inputNode.removeTap(onBus: 0)
      microphoneEngine.stop()
      self.microphoneEngine = nil
      microphoneConverter = nil
      stateLock.lock()
      microphoneStarted = false
      stateLock.unlock()
      try? session.setActive(false, options: .notifyOthersOnDeactivation)
      throw error
    }
  }

  private func stopMicrophone() {
    stateLock.lock()
    let wasStarted = microphoneStarted
    microphoneStarted = false
    stateLock.unlock()
    guard wasStarted || microphoneEngine != nil else { return }
    microphoneEngine?.inputNode.removeTap(onBus: 0)
    microphoneEngine?.stop()
    microphoneEngine = nil
    microphoneConverter = nil
    try? AVAudioSession.sharedInstance().setActive(
      false,
      options: .notifyOthersOnDeactivation
    )
  }

  private func convertAndEmitMicrophoneBuffer(
    inputBuffer: AVAudioPCMBuffer,
    converter: AVAudioConverter,
    targetFormat: AVAudioFormat
  ) {
    let frameCapacity = AVAudioFrameCount(
      Double(inputBuffer.frameLength) * targetFormat.sampleRate / inputBuffer.format.sampleRate
    ) + 1
    guard
      let outputBuffer = AVAudioPCMBuffer(pcmFormat: targetFormat, frameCapacity: frameCapacity)
    else { return }

    var conversionError: NSError?
    var inputConsumed = false
    converter.convert(to: outputBuffer, error: &conversionError) { _, status in
      if inputConsumed {
        status.pointee = .noDataNow
        return nil
      }
      inputConsumed = true
      status.pointee = .haveData
      return inputBuffer
    }
    guard
      conversionError == nil,
      outputBuffer.frameLength > 0,
      let samples = outputBuffer.int16ChannelData?[0]
    else { return }
    let byteCount = Int(outputBuffer.frameLength) * MemoryLayout<Int16>.size
    let payload = Data(bytes: samples, count: byteCount).base64EncodedString()
    sendEvent("onMicrophoneBuffer", [
      "pcm16Base64": payload,
      "sampleRate": 24_000,
      "channels": 1,
    ])
  }

  private func start(sampleRate requestedSampleRate: Int) throws {
    guard requestedSampleRate == 24_000 else {
      throw Exception(name: "unsupported_sample_rate", description: "Sarah voice requires 24 kHz PCM.")
    }
    if started { return }

    let session = AVAudioSession.sharedInstance()
    do {
      try session.setCategory(
        .playAndRecord,
        mode: .voiceChat,
        options: [.defaultToSpeaker, .allowBluetoothHFP]
      )
      try session.setActive(true)

      guard let pcmFormat = AVAudioFormat(
        commonFormat: .pcmFormatInt16,
        sampleRate: Double(requestedSampleRate),
        channels: 1,
        interleaved: false
      ) else {
        throw Exception(name: "audio_format_unavailable", description: "The PCM playback format is unavailable.")
      }
      if player.engine == nil {
        engine.attach(player)
      }
      engine.connect(player, to: engine.mainMixerNode, format: pcmFormat)
      engine.prepare()
      try engine.start()
      player.play()

      stateLock.lock()
      sampleRate = Double(requestedSampleRate)
      format = pcmFormat
      queuedFrames = 0
      interrupted = false
      playbackGeneration += 1
      started = true
      stateLock.unlock()
      installAudioSessionObservers(session: session)
    } catch {
      player.stop()
      engine.stop()
      engine.reset()
      stateLock.lock()
      queuedFrames = 0
      format = nil
      playbackGeneration += 1
      started = false
      stateLock.unlock()
      try? session.setActive(false, options: .notifyOthersOnDeactivation)
      throw error
    }
  }

  private func enqueue(pcm16Base64: String) throws -> Int {
    guard
      started,
      let pcmFormat = format,
      let bytes = Data(base64Encoded: pcm16Base64),
      !bytes.isEmpty,
      bytes.count <= 24_000,
      bytes.count.isMultiple(of: 2)
    else {
      throw Exception(name: "invalid_pcm_frame", description: "The PCM playback frame is invalid.")
    }

    let frameCount = AVAudioFrameCount(bytes.count / 2)
    stateLock.lock()
    let nextQueuedFrames = queuedFrames + Int64(frameCount)
    if nextQueuedFrames > Int64(sampleRate) * maximumQueuedSeconds {
      stateLock.unlock()
      throw Exception(name: "playback_backpressure", description: "The PCM playback queue is full.")
    }
    queuedFrames = nextQueuedFrames
    let generation = playbackGeneration
    stateLock.unlock()

    guard
      let buffer = AVAudioPCMBuffer(pcmFormat: pcmFormat, frameCapacity: frameCount),
      let destination = buffer.int16ChannelData?[0]
    else {
      stateLock.lock()
      queuedFrames -= Int64(frameCount)
      stateLock.unlock()
      throw Exception(name: "audio_buffer_unavailable", description: "The PCM playback buffer is unavailable.")
    }
    buffer.frameLength = frameCount
    bytes.copyBytes(to: UnsafeMutableRawBufferPointer(
      start: destination,
      count: bytes.count
    ))
    player.scheduleBuffer(
      buffer,
      completionCallbackType: .dataPlayedBack
    ) { [weak self] _ in
      guard let self else { return }
      self.stateLock.lock()
      if self.playbackGeneration == generation {
        self.queuedFrames = max(0, self.queuedFrames - Int64(frameCount))
      }
      self.stateLock.unlock()
    }
    if !player.isPlaying {
      player.play()
    }
    return Int(frameCount)
  }

  private func flush() {
    player.stop()
    player.reset()
    stateLock.lock()
    queuedFrames = 0
    playbackGeneration += 1
    stateLock.unlock()
    if started {
      player.play()
    }
  }

  private func stop(interruptedBySystem: Bool = false) {
    player.stop()
    engine.stop()
    engine.reset()
    stateLock.lock()
    queuedFrames = 0
    format = nil
    playbackGeneration += 1
    started = false
    interrupted = interruptedBySystem
    stateLock.unlock()
    removeAudioSessionObservers()
    try? AVAudioSession.sharedInstance().setActive(
      false,
      options: .notifyOthersOnDeactivation
    )
  }

  private func status() -> [String: Any] {
    stateLock.lock()
    let currentStarted = started
    let currentQueuedFrames = queuedFrames
    let currentSampleRate = sampleRate
    let currentInterrupted = interrupted
    stateLock.unlock()

    var playedMilliseconds = 0.0
    if
      currentStarted,
      player.engine != nil,
      let renderTime = player.lastRenderTime,
      let playerTime = player.playerTime(forNodeTime: renderTime)
    {
      playedMilliseconds =
        Double(playerTime.sampleTime) * 1_000.0 / currentSampleRate
    }
    return [
      "started": currentStarted,
      "queuedFrames": currentQueuedFrames,
      "playedMilliseconds": max(0, Int(playedMilliseconds)),
      "interrupted": currentInterrupted,
    ]
  }

  private func installAudioSessionObservers(session: AVAudioSession) {
    removeAudioSessionObservers()
    interruptionObserver = NotificationCenter.default.addObserver(
      forName: AVAudioSession.interruptionNotification,
      object: session,
      queue: .main
    ) { [weak self] notification in
      guard
        let rawType = notification.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt,
        AVAudioSession.InterruptionType(rawValue: rawType) == .began
      else { return }
      self?.stop(interruptedBySystem: true)
    }
    mediaResetObserver = NotificationCenter.default.addObserver(
      forName: AVAudioSession.mediaServicesWereResetNotification,
      object: session,
      queue: .main
    ) { [weak self] _ in
      self?.stop(interruptedBySystem: true)
    }
  }

  private func removeAudioSessionObservers() {
    if let observer = interruptionObserver {
      NotificationCenter.default.removeObserver(observer)
      interruptionObserver = nil
    }
    if let observer = mediaResetObserver {
      NotificationCenter.default.removeObserver(observer)
      mediaResetObserver = nil
    }
  }
}
