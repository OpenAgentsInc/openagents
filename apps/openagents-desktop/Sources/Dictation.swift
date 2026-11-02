// Native effects for Coax dictation: microphone permission/capture, scheduling,
// and Psionic's resident recognizer. Timeline, admission, audio conversion,
// segment boundaries, text joining and result eligibility belong to Coax.
import Foundation
import AVFoundation

enum DictationPermission { case unknown, granted, denied }
enum DictationEffectError: Error, Equatable { case model, capture, format, recognition }

protocol DictationAudioCapture: AnyObject {
  var permission: DictationPermission { get }
  func requestPermission(_ completion: @escaping (Bool) -> Void)
  func start(_ receive: @escaping ([Float], Int32, Int64) -> Void,
             failure: @escaping () -> Void) throws
  func stop()
}

protocol DictationRecognizer: AnyObject {
  func open() throws
  func recognize(_ samples: ArraySlice<Float>) throws -> [UInt8]
  func close()
}

final class MicrophoneDictationCapture: DictationAudioCapture {
  private let engine = AVAudioEngine()
  private var tapped = false
  private var changed: NSObjectProtocol?
  var permission: DictationPermission {
    switch AVCaptureDevice.authorizationStatus(for: .audio) {
    case .authorized: return .granted
    case .notDetermined: return .unknown
    default: return .denied
    }
  }
  func requestPermission(_ completion: @escaping (Bool) -> Void) {
    AVCaptureDevice.requestAccess(for: .audio, completionHandler: completion)
  }
  func start(_ receive: @escaping ([Float], Int32, Int64) -> Void,
             failure: @escaping () -> Void) throws {
    stop()
    let input = engine.inputNode
    let format = input.outputFormat(forBus: 0)
    guard format.channelCount > 0, format.sampleRate > 0,
          format.commonFormat == .pcmFormatFloat32 else { throw DictationEffectError.format }
    input.installTap(onBus: 0, bufferSize: 1024, format: format) { buffer, time in
      guard time.isHostTimeValid, let channel = buffer.floatChannelData?[0] else { failure(); return }
      // Transport channel zero, including interleaved hardware layouts. All
      // conversion, rate admission and filtering occur in the Coax controller.
      let n = Int(buffer.frameLength), stride = Int(buffer.stride)
      var mono = [Float](repeating: 0, count: n)
      for i in 0..<n { mono[i] = channel[i * stride] }
      let first = Int64(AVAudioTime.seconds(forHostTime: time.hostTime) * 1000)
      receive(mono, Int32(buffer.format.sampleRate), first)
    }
    tapped = true
    do {
      engine.prepare(); try engine.start()
      changed = NotificationCenter.default.addObserver(forName: .AVAudioEngineConfigurationChange,
                                                       object: engine, queue: nil) { _ in failure() }
    }
    catch { stop(); throw error }
  }
  func stop() {
    if let changed { NotificationCenter.default.removeObserver(changed); self.changed = nil }
    if tapped { engine.inputNode.removeTap(onBus: 0); tapped = false }
    engine.stop()
  }
  deinit { stop() }
}

// This capture transport exists only for --shot fixtures and native effect
// checks. It never requests permission, opens a device or synthesizes sound.
final class FixtureDictationCapture: DictationAudioCapture {
  var permission: DictationPermission { .granted }
  func requestPermission(_ completion: @escaping (Bool) -> Void) { completion(true) }
  func start(_ receive: @escaping ([Float], Int32, Int64) -> Void,
             failure: @escaping () -> Void) throws {}
  func stop() {}
}

final class ResidentParakeetDictation: DictationRecognizer {
  private var model: OpaquePointer?
  private let directory: String
  init(directory: String? = nil) {
    self.directory = directory ?? ProcessInfo.processInfo.environment["OPENAGENTS_PARAKEET_MODEL"]
      ?? FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(
        ".cache/openagents/models/parakeet/parakeet-tdt-0.6b-v3-541d1f99").path
  }
  func open() throws {
    if model != nil { return }
    var error = [CChar](repeating: 0, count: 1024)
    model = directory.withCString { psionic_parakeet_open($0, 1, &error, error.count) }
    if model == nil { throw DictationEffectError.model }
  }
  func recognize(_ samples: ArraySlice<Float>) throws -> [UInt8] {
    guard let model else { throw DictationEffectError.model }
    // The library's resident work and snapshots are optional for inspection;
    // dictation keeps only its copied text and drops that per-turn audio state.
    defer { _ = psionic_parakeet_clear(model) }
    var error = [CChar](repeating: 0, count: 1024)
    var text = [CChar](repeating: 0, count: 8193)
    let ok = samples.withUnsafeBufferPointer {
      psionic_parakeet_text(model, $0.baseAddress, Int64($0.count), &text, text.count, &error, error.count)
    }
    guard ok != 0 else { throw DictationEffectError.recognition }
    return text.withUnsafeBufferPointer { p in
      let n = strlen(p.baseAddress!)
      return Array(UnsafeRawBufferPointer(start: p.baseAddress, count: n))
    }
  }
  func close() { if let model { psionic_parakeet_close(model); self.model = nil } }
  deinit { close() }
}

final class CoderDictation {
  typealias StateHandler = (Int32, Int32, Int32, Int32) -> Void
  private let queue = DispatchQueue(label: "openagents.dictation.controller")
  private let worker = DispatchQueue(label: "openagents.dictation.parakeet", qos: .userInitiated)
  private let queueKey = DispatchSpecificKey<UInt8>()
  private let capture: DictationAudioCapture
  private let recognizer: DictationRecognizer
  private let fixture: Bool
  private let onState: StateHandler
  private let onTranscript: (UInt64, String) -> Void
  private let controller: UnsafeMutableRawPointer
  private var timer: DispatchSourceTimer?
  private var disposed = false
  private var permissionPending = false
  private var permissionGranted = false
  private var captureRunning = false
  private var captureGeneration: UInt64 = 0
  private var modelEpoch: UInt64 = 0
  private var loadingEpoch: UInt64?
  private var modelReady = false
  private var fixtureTime: Int64 = 0

  init(fixture: Bool = false, capture: DictationAudioCapture? = nil,
       recognizer: DictationRecognizer? = nil,
       onState: @escaping StateHandler, onTranscript: @escaping (UInt64, String) -> Void) {
    self.fixture = fixture
    self.capture = capture ?? (fixture ? FixtureDictationCapture() : MicrophoneDictationCapture())
    self.recognizer = recognizer ?? ResidentParakeetDictation()
    self.onState = onState; self.onTranscript = onTranscript
    guard let state = dct_new() else { fatalError("Cannot allocate dictation controller") }
    controller = state
    queue.setSpecific(key: queueKey, value: 1)
    let timer = DispatchSource.makeTimerSource(queue: queue)
    timer.schedule(deadline: .now() + .milliseconds(20), repeating: .milliseconds(20))
    timer.setEventHandler { [weak self] in self?.poll() }
    self.timer = timer; timer.resume()
  }
  private func now() -> Int64 {
    fixture ? fixtureTime : Int64(DispatchTime.now().uptimeNanoseconds / 1_000_000)
  }
  private func sync<T>(_ body: () -> T) -> T {
    if DispatchQueue.getSpecific(key: queueKey) != nil { return body() }
    return queue.sync(execute: body)
  }
  private func publish() {
    DispatchQueue.main.async { [weak self] in
      guard let self else { return }
      let value = self.sync { () -> (Int32, Int32, Int32, Int32)? in
        if self.disposed { return nil }
        return (Int32(dct_phase(self.controller)), Int32(dct_outcome(self.controller)),
                Int32(dct_fault_reason(self.controller)), Int32(dct_preparing_reason(self.controller)))
      }
      if let value { self.onState(value.0, value.1, value.2, value.3) }
    }
  }
  func setEnabled(_ enabled: Bool) {
    sync { guard !disposed else { return }; dct_enable(controller, enabled ? 1 : 0, now()); reconcile(); publish() }
  }
  func activate() {
    sync { guard !disposed else { return }; dct_active(controller, 1); reconcile(); publish() }
  }
  func deactivate() {
    sync { guard !disposed else { return }; dct_active(controller, 0); stopCapture(); dct_ready(controller, 0); publish() }
  }
  @discardableResult func begin() -> UInt64 {
    sync { guard !disposed else { return 0 }; let ticket = dct_begin(controller, now()); publish(); return ticket }
  }
  func end() { sync { guard !disposed else { return }; dct_release(controller, now()); poll() } }
  func cancel() { sync { guard !disposed else { return }; dct_cancel(controller); publish() } }

  private func stopCapture() {
    captureGeneration &+= 1
    if captureRunning { capture.stop(); captureRunning = false }
  }
  private func closeModel() {
    modelEpoch &+= 1; loadingEpoch = nil; modelReady = false; dct_ready(controller, 0)
    let recognizer = self.recognizer
    worker.async { recognizer.close() }
  }
  private func captureFault(_ reason: Int32) {
    stopCapture(); dct_ready(controller, 0); dct_fault(controller, reason); publish()
  }
  private func reconcile() {
    guard !disposed else { return }
    // Enabled/active admission is queried from Coax; Swift transports only
    // permission and resource-completion facts and executes their effects.
    guard dct_enabled(controller) != 0 else {
      stopCapture(); closeModel(); return
    }
    guard dct_is_active(controller) != 0 else { stopCapture(); dct_ready(controller, 0); return }
    switch capture.permission {
    case .denied:
      permissionGranted = false; stopCapture(); closeModel(); dct_fault(controller, 1); return
    case .unknown:
      guard !permissionPending else { return }
      permissionPending = true; dct_preparing(controller, 1); publish()
      capture.requestPermission { [weak self] granted in
        guard let self else { return }
        self.queue.async { [weak self] in
          guard let self, !self.disposed else { return }
          self.permissionPending = false; self.permissionGranted = granted
          if granted { self.reconcile() } else { self.stopCapture(); self.closeModel(); dct_fault(self.controller, 1) }
          self.publish()
        }
      }
      return
    case .granted: permissionGranted = true
    }
    if !modelReady {
      guard loadingEpoch == nil else { return }
      modelEpoch &+= 1; let epoch = modelEpoch; loadingEpoch = epoch
      dct_preparing(controller, 2); publish(); let recognizer = self.recognizer
      worker.async { [weak self] in
        let loaded: Bool
        do { try recognizer.open(); loaded = true } catch { loaded = false }
        guard let self else { recognizer.close(); return }
        self.queue.async { [weak self] in
          guard let self, !self.disposed, self.modelEpoch == epoch else { return }
          self.loadingEpoch = nil; self.modelReady = loaded
          if loaded { self.reconcile() } else { dct_fault(self.controller, 2); self.stopCapture() }
          self.publish()
        }
      }
      return
    }
    dct_ready(controller, 1)
    if !captureRunning, dct_capture_allowed(controller) != 0 {
      dct_preparing(controller, 3); captureGeneration &+= 1; let generation = captureGeneration
      do {
        try capture.start({ [weak self] samples, rate, first in
          guard let self else { return }
          self.queue.async { [weak self] in
            guard let self, !self.disposed, self.captureGeneration == generation,
                  dct_capture_allowed(self.controller) != 0 else { return }
            self.audio(samples, rate: rate, first: first)
          }
        }, failure: { [weak self] in
          guard let self else { return }
          self.queue.async { [weak self] in
            guard let self, !self.disposed, self.captureGeneration == generation else { return }
            self.captureFault(3)
          }
        })
        captureRunning = true; dct_ready(controller, 1)
      } catch {
        captureFault((error as? DictationEffectError) == .format ? 5 : 3)
      }
    }
  }
  private func audio(_ samples: [Float], rate: Int32, first: Int64) {
    let phase = dct_phase(controller), outcome = dct_outcome(controller), fault = dct_fault_reason(controller)
    samples.withUnsafeBufferPointer { dct_audio(controller, $0.baseAddress, Int32($0.count), rate, first) }
    if dct_outcome(controller) == 4,
       outcome != 4 || fault != dct_fault_reason(controller) { captureFault(Int32(dct_fault_reason(controller))) }
    considerRecognition()
    if phase != dct_phase(controller) || outcome != dct_outcome(controller) { publish() }
  }
  private func poll() {
    guard !disposed else { return }
    let phase = dct_phase(controller), outcome = dct_outcome(controller)
    dct_tick(controller, now())
    if outcome != 4, dct_outcome(controller) == 4 { captureFault(Int32(dct_fault_reason(controller))) }
    considerRecognition()
    if phase != dct_phase(controller) || outcome != dct_outcome(controller) { publish() }
  }
  private func considerRecognition() {
    guard dct_phase(controller) == 4 else { return }
    let ticket = dct_ticket(controller), count = Int(dct_samples(controller))
    var audio = [Float](repeating: 0, count: count)
    let copied = audio.withUnsafeMutableBufferPointer { dct_copy_audio(controller, 0, $0.baseAddress, Int32($0.count)) }
    guard copied == count else { dct_fault(controller, 4); publish(); return }
    var segments: [Range<Int>] = []; var offset: Int32 = 0
    while let n = Optional(dct_segment(controller, offset)), n > 0 {
      segments.append(Int(offset)..<Int(offset + n)); offset += n
    }
    guard offset == count, dct_decode_begin(controller, ticket) != 0 else { dct_fault(controller, 4); publish(); return }
    publish(); let recognizer = self.recognizer
    worker.async { [weak self] in
      var succeeded = true
      for segment in segments {
        guard let self else { return }
        let current = self.sync { !self.disposed && dct_ticket(self.controller) == ticket && dct_phase(self.controller) == 5 }
        if !current { return }
        do {
          let text = try recognizer.recognize(audio[segment])
          let appended = self.sync {
            guard !self.disposed, dct_ticket(self.controller) == ticket, dct_phase(self.controller) == 5 else { return false }
            return text.withUnsafeBufferPointer { dct_append_text(self.controller, ticket, $0.baseAddress, Int32($0.count)) != 0 }
          }
          if !appended { succeeded = false; break }
        } catch { succeeded = false; break }
      }
      guard let self else { return }
      self.queue.async { [weak self] in
        guard let self, !self.disposed, dct_ticket(self.controller) == ticket else { return }
        let accepted = dct_result(self.controller, ticket, succeeded ? 1 : 0) != 0
        self.publish()
        if accepted {
          DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            let text: String? = self.sync {
              guard !self.disposed, dct_ticket(self.controller) == ticket,
                    dct_outcome(self.controller) == 5, dct_capture_allowed(self.controller) != 0 else { return nil }
              var bytes = [UInt8](repeating: 0, count: Int(dct_text_count(self.controller)))
              let n = bytes.withUnsafeMutableBufferPointer { dct_copy_text(self.controller, $0.baseAddress, Int32($0.count)) }
              return n == bytes.count ? String(decoding: bytes, as: UTF8.self) : nil
            }
            if let text { self.onTranscript(ticket, text) }
          }
        }
      }
    }
  }
  // Fixture-only transport exercises the same controller, recognizer and
  // delivery callbacks while leaving permission and microphone effects unused.
  func fixtureTime(_ milliseconds: Int64) {
    sync { guard fixture, !disposed else { return }; fixtureTime = milliseconds; poll() }
  }
  func fixtureAudio(_ samples: [Float], rate: Int32, firstSampleMS: Int64) {
    sync { guard fixture, !disposed, dct_capture_allowed(controller) != 0 else { return }; audio(samples, rate: rate, first: firstSampleMS) }
  }
  func inspect() -> (Int32, Int32, UInt64, Int32) {
    sync { (Int32(dct_phase(controller)), Int32(dct_outcome(controller)), dct_ticket(controller), dct_samples(controller)) }
  }
  func isReady() -> Bool { sync { !disposed && dct_capture_allowed(controller) != 0 } }
  func shutdown() {
    sync {
      guard !disposed else { return }
      dct_enable(controller, 0, now()); stopCapture(); closeModel(); disposed = true
      timer?.cancel(); timer = nil
    }
  }
  deinit { shutdown(); dct_free(controller) }
}
