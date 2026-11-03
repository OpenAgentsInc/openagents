// Native effects for Coax dictation: microphone permission/capture, scheduling,
// and Psionic's resident recognizer. Timeline, admission, audio conversion,
// segment boundaries, text joining and result eligibility belong to Coax.
import Foundation
import AVFoundation
import CoreAudio
import AudioToolbox

enum DictationPermission { case unknown, granted, denied }
enum DictationEffectError: Error, Equatable { case model, capture, format, recognition }

struct DictationInputDevice: Equatable { let uid: String; let name: String }

protocol DictationAudioCapture: AnyObject {
  var inputDevices: [DictationInputDevice] { get }
  var selectedInputUID: String { get }
  func selectInput(_ uid: String) throws
  var permission: DictationPermission { get }
  func requestPermission(_ completion: @escaping (Bool) -> Void)
  func start(_ receive: @escaping ([Float], Int32, Int64) -> Void,
             failure: @escaping () -> Void) throws
  func stop()
}

extension DictationAudioCapture {
  var inputDevices: [DictationInputDevice] { [] }
  var selectedInputUID: String { "" }
  func selectInput(_ uid: String) throws { if !uid.isEmpty { throw DictationEffectError.capture } }
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
  private let preferences: UserDefaults
  private static let selectionKey = "OpenAgentsLocalDictationInputUID"
  private var hardwareListeners: [(AudioObjectPropertyAddress, AudioObjectPropertyListenerBlock)] = []
  init(preferences: UserDefaults = .standard) { self.preferences = preferences }
  var selectedInputUID: String { preferences.string(forKey: Self.selectionKey) ?? "" }
  private func text(_ device: AudioDeviceID, _ selector: AudioObjectPropertySelector) -> String {
    var address = AudioObjectPropertyAddress(mSelector: selector, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
    var value: Unmanaged<CFString>?; var size = UInt32(MemoryLayout<Unmanaged<CFString>?>.size)
    let status = withUnsafeMutablePointer(to: &value) { AudioObjectGetPropertyData(device, &address, 0, nil, &size, $0) }
    // CoreAudio's property contract transfers a retained CFObject to caller.
    guard status == noErr, let value else { return "" }
    return value.takeRetainedValue() as String
  }
  private func channels(_ device: AudioDeviceID) -> Int {
    var address = AudioObjectPropertyAddress(mSelector: kAudioDevicePropertyStreamConfiguration, mScope: kAudioDevicePropertyScopeInput, mElement: kAudioObjectPropertyElementMain)
    var size: UInt32 = 0
    guard AudioObjectGetPropertyDataSize(device, &address, 0, nil, &size) == noErr, size >= MemoryLayout<AudioBufferList>.size else { return 0 }
    let storage = UnsafeMutableRawPointer.allocate(byteCount: Int(size), alignment: MemoryLayout<AudioBufferList>.alignment)
    defer { storage.deallocate() }
    guard AudioObjectGetPropertyData(device, &address, 0, nil, &size, storage) == noErr else { return 0 }
    return UnsafeMutableAudioBufferListPointer(storage.assumingMemoryBound(to: AudioBufferList.self)).reduce(0) { $0 + Int($1.mNumberChannels) }
  }
  private func devices() -> [(AudioDeviceID, DictationInputDevice)] {
    var address = AudioObjectPropertyAddress(mSelector: kAudioHardwarePropertyDevices, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
    var size: UInt32 = 0
    guard AudioObjectGetPropertyDataSize(AudioObjectID(kAudioObjectSystemObject), &address, 0, nil, &size) == noErr,
          size >= MemoryLayout<AudioDeviceID>.size, size <= 4096, size % UInt32(MemoryLayout<AudioDeviceID>.size) == 0 else { return [] }
    var ids = [AudioDeviceID](repeating: 0, count: Int(size) / MemoryLayout<AudioDeviceID>.size)
    let status = ids.withUnsafeMutableBytes { AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject), &address, 0, nil, &size, $0.baseAddress!) }
    guard status == noErr else { return [] }
    return ids.filter { channels($0) > 0 }.compactMap { id in
      let uid = text(id, kAudioDevicePropertyDeviceUID)
      guard deviceMatches(uid, uid, false) else { return nil }
      return (id, DictationInputDevice(uid: uid, name: text(id, kAudioObjectPropertyName)))
    }
  }
  var inputDevices: [DictationInputDevice] { devices().map { $0.1 } }
  func selectInput(_ uid: String) throws {
    // An empty selection explicitly follows the OS default. An unavailable
    // selected UID never silently changes the user's persisted selection.
    if !uid.isEmpty && !inputDevices.contains(where: { deviceMatches(uid, $0.uid, false) }) { throw DictationEffectError.capture }
    preferences.set(uid, forKey: Self.selectionKey)
  }
  private func deviceMatches(_ wanted: String, _ observed: String, _ isDefault: Bool) -> Bool {
    let a = Array(wanted.utf8), b = Array(observed.utf8)
    return a.withUnsafeBufferPointer { x in b.withUnsafeBufferPointer { y in
      dct_device_matches(x.baseAddress, Int32(x.count), y.baseAddress, Int32(y.count), isDefault ? 1 : 0) != 0
    } }
  }
  private func selectedDevice() -> AudioDeviceID? {
    var address = AudioObjectPropertyAddress(mSelector: kAudioHardwarePropertyDefaultInputDevice, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
    var value = AudioDeviceID(0), size = UInt32(MemoryLayout<AudioDeviceID>.size)
    if AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject), &address, 0, nil, &size, &value) != noErr { value = 0 }
    return devices().first { deviceMatches(selectedInputUID, $0.1.uid, $0.0 == value) }?.0
  }
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
    guard let device = selectedDevice() else { throw DictationEffectError.capture }
    let input = engine.inputNode
    guard let unit = input.audioUnit else { throw DictationEffectError.capture }
    var selected = device
    guard AudioUnitSetProperty(unit, kAudioOutputUnitProperty_CurrentDevice, kAudioUnitScope_Global, 0, &selected, UInt32(MemoryLayout<AudioDeviceID>.size)) == noErr else { throw DictationEffectError.capture }
    let format = input.outputFormat(forBus: 0)
    guard format.sampleRate >= 1, format.sampleRate <= 192000,
          format.commonFormat == .pcmFormatFloat32,
          dct_device_format(format.sampleRate, Int64(format.channelCount), format.isInterleaved ? Int64(format.channelCount) : 1, 1) != 0 else { throw DictationEffectError.format }
    input.installTap(onBus: 0, bufferSize: 1024, format: format) { buffer, time in
      guard let mono = Self.channelZero(buffer, hostTimeValid: time.isHostTimeValid) else { failure(); return }
      let first = Int64(AVAudioTime.seconds(forHostTime: time.hostTime) * 1000)
      receive(mono, Int32(buffer.format.sampleRate), first)
    }
    tapped = true
    do {
      for selector in [kAudioHardwarePropertyDevices, kAudioHardwarePropertyDefaultInputDevice] {
        var address = AudioObjectPropertyAddress(mSelector: selector, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
        let listener: AudioObjectPropertyListenerBlock = { _, _ in failure() }
        guard AudioObjectAddPropertyListenerBlock(AudioObjectID(kAudioObjectSystemObject), &address, nil, listener) == noErr else { throw DictationEffectError.capture }
        hardwareListeners.append((address, listener))
      }
      changed = NotificationCenter.default.addObserver(forName: .AVAudioEngineConfigurationChange,
                                                       object: engine, queue: nil) { _ in failure() }
      // Subscribe before starting IO and revalidate the choice so a route
      // change between enumeration and listener installation cannot be missed.
      guard selectedDevice() == device else { throw DictationEffectError.capture }
      engine.prepare(); try engine.start()
    }
    catch { stop(); throw error }
  }
  static func channelZero(_ buffer: AVAudioPCMBuffer, hostTimeValid: Bool) -> [Float]? {
    guard buffer.format.sampleRate >= 1, buffer.format.sampleRate <= 192000,
          buffer.format.commonFormat == .pcmFormatFloat32,
          dct_device_format(buffer.format.sampleRate, Int64(buffer.format.channelCount), Int64(buffer.stride), hostTimeValid ? 1 : 0) != 0,
          let channel = buffer.floatChannelData?[0] else { return nil }
    let n = Int(buffer.frameLength), stride = Int(buffer.stride)
    var mono = [Float](repeating: 0, count: n)
    for i in 0..<n { mono[i] = channel[i * stride] }
    return mono
  }
  func stop() {
    for (var address, listener) in hardwareListeners { AudioObjectRemovePropertyListenerBlock(AudioObjectID(kAudioObjectSystemObject), &address, nil, listener) }
    hardwareListeners.removeAll()
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
    var segments: [(range: Range<Int>, leading: Int, trailing: Int)] = []; var offset: Int32 = 0
    while let n = Optional(dct_segment(controller, offset)), n > 0 {
      segments.append((Int(offset)..<Int(offset + n),
                       Int(dct_segment_leading_silence(controller, offset)),
                       Int(dct_segment_trailing_silence(controller, offset))))
      offset += n
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
          // The Coax plan owns cut positions and silence counts. Padding is
          // only model input; it never advances the captured audio offset.
          var input = [Float](repeating: 0, count: segment.leading)
          input.append(contentsOf: audio[segment.range])
          input.append(contentsOf: repeatElement(Float(0), count: segment.trailing))
          let text = try recognizer.recognize(input[...])
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
  var inputDevices: [DictationInputDevice] { sync { capture.inputDevices } }
  var selectedInputUID: String { sync { capture.selectedInputUID } }
  @discardableResult func selectInput(_ uid: String) -> Bool {
    sync {
      guard !disposed else { return false }
      stopCapture(); dct_cancel(controller); dct_ready(controller, 0)
      do { try capture.selectInput(uid); reconcile(); publish(); return true }
      catch { captureFault(3); return false }
    }
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
