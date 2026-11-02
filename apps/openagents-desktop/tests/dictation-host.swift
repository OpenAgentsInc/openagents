// Firm reason: 2 (independent native scheduling/permission/cancellation oracle).
// Injected devices never request microphone permission, open hardware or write
// audio. --real uses a public recorded PCM clip and the genuine Metal model.
import Foundation

enum DictationOracleError: Error { case failed(String), injected }
func require(_ test: @autoclosure () -> Bool, _ message: String) throws {
  if !test() { throw DictationOracleError.failed(message) }
}
func eventually(_ message: String, timeout: Double = 5, _ condition: () -> Bool) throws {
  let end = Date().addingTimeInterval(timeout)
  while !condition(), Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.005)) }
  try require(condition(), message)
}
func drain() { RunLoop.main.run(until: Date().addingTimeInterval(0.05)) }

final class OracleCapture: DictationAudioCapture {
  private let lock = NSLock()
  private var value: DictationPermission
  private var requests = 0, starts = 0, stops = 0
  private var pending: ((Bool) -> Void)?
  private var failed: (() -> Void)?
  init(_ permission: DictationPermission = .granted) { value = permission }
  var permission: DictationPermission { lock.lock(); defer { lock.unlock() }; return value }
  var counts: (Int, Int, Int) { lock.lock(); defer { lock.unlock() }; return (requests, starts, stops) }
  func requestPermission(_ completion: @escaping (Bool) -> Void) { lock.lock(); requests += 1; pending = completion; lock.unlock() }
  func resolve(_ allowed: Bool) {
    lock.lock(); value = allowed ? .granted : .denied; let callback = pending; pending = nil; lock.unlock(); callback?(allowed)
  }
  func start(_ receive: @escaping ([Float], Int32, Int64) -> Void, failure: @escaping () -> Void) throws {
    lock.lock(); starts += 1; failed = failure; lock.unlock()
  }
  func stop() { lock.lock(); stops += 1; failed = nil; lock.unlock() }
  func fail() { lock.lock(); let callback = failed; lock.unlock(); callback?() }
}

final class OracleRecognizer: DictationRecognizer {
  private let lock = NSLock()
  private var opened = false, loads = 0, calls = 0, closed = 0, completed = 0
  private var saved: [[Float]] = []
  var blockLoad: DispatchSemaphore?
  var blockCall: DispatchSemaphore?
  var failCall = false
  var answers = ["dictated words"]
  var counts: (Int, Int, Int, Int) { lock.lock(); defer { lock.unlock() }; return (loads, calls, closed, completed) }
  var clips: [[Float]] { lock.lock(); defer { lock.unlock() }; return saved }
  func open() throws {
    lock.lock(); loads += 1; let wait = blockLoad; lock.unlock()
    if let wait, wait.wait(timeout: .now() + 5) != .success { throw DictationOracleError.injected }
    lock.lock(); opened = true; lock.unlock()
  }
  func recognize(_ samples: ArraySlice<Float>) throws -> [UInt8] {
    lock.lock(); let index = calls; calls += 1; saved.append(Array(samples)); let wait = blockCall
    let text = answers[min(index, answers.count - 1)], fail = failCall; lock.unlock()
    defer { lock.lock(); completed += 1; lock.unlock() }
    if let wait, wait.wait(timeout: .now() + 5) != .success { throw DictationOracleError.injected }
    if fail { throw DictationOracleError.injected }
    return Array(text.utf8)
  }
  func close() { lock.lock(); if opened { closed += 1; opened = false }; lock.unlock() }
}

func square(_ n: Int) -> [Float] { (0..<n).map { $0 % 16 < 8 ? 0.25 : -0.25 } }
func frames(_ host: CoderDictation, _ from: Int, _ to: Int, silence: Bool = false) {
  let audio = silence ? [Float](repeating: 0, count: 320) : square(320)
  for frame in from..<to { host.fixtureAudio(audio, rate: 16000, firstSampleMS: Int64(frame * 20)) }
}
func prepared(_ capture: OracleCapture, _ recognizer: OracleRecognizer,
              delivered: @escaping (UInt64, String) -> Void = { _, _ in }) throws -> CoderDictation {
  let host = CoderDictation(fixture: true, capture: capture, recognizer: recognizer,
                           onState: { _, _, _, _ in }, onTranscript: delivered)
  host.activate(); host.setEnabled(true)
  try eventually("fixture capture did not become ready") { capture.counts.1 == 1 }
  return host
}
func utterance(_ host: CoderDictation, silence: Bool = false) throws -> UInt64 {
  frames(host, 0, 20, silence: silence); host.fixtureTime(400)
  let ticket = host.begin(); try require(ticket > 0, "held request refused after readiness")
  frames(host, 20, 35, silence: silence); host.fixtureTime(700); host.end()
  frames(host, 35, 45, silence: silence); host.fixtureTime(900)
  return ticket
}

@main struct DictationHostOracle {
  static func main() throws {
    if CommandLine.arguments.count == 4, CommandLine.arguments[1] == "--real" {
      try real(model: CommandLine.arguments[2], pcm: CommandLine.arguments[3]); return
    }
    var checked = 0
    do {
      let capture = OracleCapture(.unknown), recognizer = OracleRecognizer()
      let host = CoderDictation(fixture: true, capture: capture, recognizer: recognizer,
                               onState: { _, _, _, _ in }, onTranscript: { _, _ in fatalError("unexpected transcript") })
      try require(host.begin() == 0 && capture.counts.0 == 0 && recognizer.counts.0 == 0, "disabled opens an effect")
      host.activate(); host.setEnabled(true)
      try require(host.begin() == 0 && capture.counts.0 == 1 && capture.counts.1 == 0, "permission warm-up accepted hold")
      host.end(); capture.resolve(true)
      try eventually("permission completion did not warm") { capture.counts.1 == 1 }
      try require(recognizer.counts.1 == 0 && host.inspect().0 == 1, "permission completion resumed released hold")
      host.shutdown(); checked += 1
    }
    do {
      let capture = OracleCapture(.denied), recognizer = OracleRecognizer(); var fault: Int32 = 0
      let host = CoderDictation(fixture: true, capture: capture, recognizer: recognizer,
                               onState: { _, _, reason, _ in fault = reason }, onTranscript: { _, _ in fatalError() })
      host.activate(); host.setEnabled(true); try eventually("permission refusal not reported") { fault == 1 }
      try require(host.begin() == 0 && capture.counts.1 == 0 && recognizer.counts.0 == 0, "denied permission opened resources")
      host.shutdown(); checked += 1
    }
    do {
      let capture = OracleCapture(), recognizer = OracleRecognizer(); recognizer.blockLoad = DispatchSemaphore(value: 0)
      let host = CoderDictation(fixture: true, capture: capture, recognizer: recognizer,
                               onState: { _, _, _, _ in }, onTranscript: { _, _ in fatalError() })
      host.activate(); host.setEnabled(true); try eventually("model open not dispatched") { recognizer.counts.0 == 1 }
      try require(host.begin() == 0, "loading model accepted hold"); host.setEnabled(false); recognizer.blockLoad?.signal()
      try eventually("disabled warm-up model not closed") { recognizer.counts.2 == 1 }
      try require(capture.counts.1 == 0 && host.begin() == 0, "late model completion opened microphone")
      host.shutdown(); checked += 1
    }
    do {
      let capture = OracleCapture(), recognizer = OracleRecognizer(); var results: [String] = []
      let host = try prepared(capture, recognizer) { _, text in results.append(text) }
      frames(host, 0, 20); host.fixtureTime(400); try require(host.begin() > 0, "tap begin")
      host.fixtureTime(699); host.end(); drain()
      try require(host.inspect().1 == 1 && recognizer.counts.1 == 0 && results.isEmpty, "tap reached recognizer")
      host.shutdown(); checked += 1
    }
    do {
      let capture = OracleCapture(), recognizer = OracleRecognizer(); var results: [String] = []
      let host = try prepared(capture, recognizer) { _, text in results.append(text) }
      _ = try utterance(host, silence: true); drain()
      try require(host.inspect().1 == 2 && recognizer.counts.1 == 0 && results.isEmpty, "silence reached recognizer")
      host.shutdown(); checked += 1
    }
    do {
      let capture = OracleCapture(), recognizer = OracleRecognizer(); var result: (UInt64, String)?
      let host = try prepared(capture, recognizer) { result = ($0, $1) }
      let ticket = try utterance(host); try eventually("success not delivered") { result != nil }
      try require(result!.0 == ticket && result!.1 == "dictated words", "wrong final result")
      try require(recognizer.clips.count == 1 && recognizer.clips[0] == square(14400), "pre-roll/tail audio differs")
      try require(host.inspect().3 == 0 && recognizer.counts.0 == 1, "audio not cleared or model reopened")
      host.shutdown(); checked += 1
    }
    for deactivate in [false, true] {
      let capture = OracleCapture(), recognizer = OracleRecognizer(); recognizer.blockCall = DispatchSemaphore(value: 0)
      var results: [String] = []; let host = try prepared(capture, recognizer) { _, text in results.append(text) }
      _ = try utterance(host); try eventually("recognition not dispatched") { recognizer.counts.1 == 1 }
      if deactivate { host.deactivate() } else { host.cancel() }
      recognizer.blockCall?.signal(); try eventually("recognizer did not finish") { recognizer.counts.3 == 1 }; drain()
      try require(results.isEmpty && host.inspect().3 == 0, "cancelled recognition delivered")
      if deactivate {
        try require(capture.counts.2 > 0 && host.begin() == 0, "background retained active microphone")
        try require(recognizer.counts.0 == 1 && recognizer.counts.2 == 0, "background discarded resident model")
        recognizer.blockCall = nil; host.activate()
        try eventually("reactivation did not rearm") { capture.counts.1 == 2 }
        try require(recognizer.counts.0 == 1, "reactivation reloaded the resident model")
      }
      host.shutdown(); checked += 1
    }
    do {
      let capture = OracleCapture(), recognizer = OracleRecognizer(); var fault: Int32 = 0
      let host = CoderDictation(fixture: true, capture: capture, recognizer: recognizer,
                               onState: { _, _, reason, _ in fault = reason }, onTranscript: { _, _ in fatalError() })
      host.activate(); host.setEnabled(true); try eventually("capture not ready") { capture.counts.1 == 1 }
      capture.fail(); try eventually("capture failure not published") { fault == 3 }
      try require(host.begin() == 0 && capture.counts.2 > 0, "failed capture accepted a new hold")
      host.setEnabled(true); try eventually("explicit rearm failed") { capture.counts.1 == 2 }
      host.shutdown(); checked += 1
    }
    do {
      let capture = OracleCapture(), recognizer = OracleRecognizer(); recognizer.failCall = true
      var results: [String] = []; let host = try prepared(capture, recognizer) { _, text in results.append(text) }
      _ = try utterance(host); try eventually("recognition failure missing") { host.inspect().1 == 4 }; drain()
      try require(results.isEmpty && host.inspect().3 == 0, "failed recognizer published text")
      frames(host, 45, 50); host.fixtureTime(1000)
      try require(host.isReady() && capture.counts.2 == 0, "old ASR error stopped healthy capture on next audio")
      recognizer.failCall = false; try require(host.begin() > 0, "retry after ASR error refused")
      frames(host, 50, 65); host.fixtureTime(1300); host.end(); frames(host, 65, 75); host.fixtureTime(1500)
      try eventually("recognizer retry did not deliver") { results.count == 1 }
      try require(results[0] == "dictated words" && recognizer.counts.0 == 1 && recognizer.counts.1 == 2,
                  "recognizer retry reopened or used wrong result")
      host.shutdown(); checked += 1
    }
    do {
      let capture = OracleCapture(), recognizer = OracleRecognizer(); recognizer.answers = [""]
      var results: [String] = []; let host = try prepared(capture, recognizer) { _, text in results.append(text) }
      _ = try utterance(host); try eventually("empty ASR did not become no speech") { host.inspect().1 == 2 }; drain()
      try require(recognizer.counts.1 == 1 && results.isEmpty && host.inspect().3 == 0, "empty ASR published a draft")
      host.shutdown(); checked += 1
    }
    do {
      let capture = OracleCapture(), recognizer = OracleRecognizer(); let host = try prepared(capture, recognizer)
      host.fixtureAudio([Float.nan], rate: 16000, firstSampleMS: 0)
      try require(host.inspect().1 == 4 && !host.isReady() && host.begin() == 0 && capture.counts.2 > 0,
                  "nonfinite capture did not stop and refuse holds")
      host.shutdown(); checked += 1
    }
    do {
      let capture = OracleCapture(), recognizer = OracleRecognizer(); let host = try prepared(capture, recognizer)
      frames(host, 0, 20); host.fixtureTime(400); try require(host.begin() > 0, "stall begin")
      host.fixtureTime(700); host.end(); host.fixtureTime(1901)
      try require(host.inspect().1 == 4 && !host.isReady() && capture.counts.2 > 0 && recognizer.counts.1 == 0,
                  "stalled release tail did not stop capture")
      host.shutdown(); checked += 1
    }
    do {
      let capture = OracleCapture(), recognizer = OracleRecognizer(); recognizer.answers = ["one", "two", "three"]
      var result: String?; let host = try prepared(capture, recognizer) { _, text in result = text }
      frames(host, 0, 20); host.fixtureTime(400); try require(host.begin() > 0, "long begin")
      frames(host, 20, 3000); host.fixtureTime(60000)
      try eventually("long clip not joined") { result != nil }
      let lengths = recognizer.clips.map { $0.count }
      try require(result == "one two three" && lengths.count == 3 && lengths.reduce(0, +) == 959200,
                  "60s model segments did not cover bounded clip")
      try require(lengths.allSatisfy { $0 >= 320 && $0 <= 327520 }, "invalid model segment length")
      host.shutdown(); checked += 1
    }
    print("Dictation native effects: \(checked) cases passed; no microphone, network or audio files")
  }
  static func real(model: String, pcm: String) throws {
    let data = try Data(contentsOf: URL(fileURLWithPath: pcm)); try require(data.count % 2 == 0, "PCM alignment")
    var audio: [Float] = []; audio.reserveCapacity(data.count / 2)
    for i in stride(from: 0, to: data.count, by: 2) {
      let bits = UInt16(data[i]) | (UInt16(data[i + 1]) << 8)
      audio.append(Float(Int16(bitPattern: bits)) / 32768)
    }
    var result: String?; let host = CoderDictation(fixture: true,
      recognizer: ResidentParakeetDictation(directory: model), onState: { _, _, _, _ in },
      onTranscript: { _, text in result = text })
    host.activate(); host.setEnabled(true)
    // No device exists in fixture mode. Readiness comes from the real pinned
    // engine open and the fixture capture transport, never a mic permission.
    try eventually("actual model did not warm", timeout: 180) { host.isReady() }
    host.cancel(); host.fixtureTime(0)
    func feed(_ from: Int, _ to: Int) {
      var at = from
      while at < to { let end = min(at + 320, to); host.fixtureAudio(Array(audio[at..<end]), rate: 16000, firstSampleMS: Int64(at / 16)); at = end }
    }
    feed(0, 6400); host.fixtureTime(400); try require(host.begin() > 0, "real fixture begin")
    let release = audio.count - 3200; feed(6400, release); host.fixtureTime(Int64(release / 16)); host.end()
    feed(release, audio.count); host.fixtureTime(Int64(audio.count / 16))
    try eventually("actual recorded recognition did not complete", timeout: 180) { result != nil }
    try require(result == "I am convinced of what I say, said the Count.", "actual fixture transcript differs")
    try require(host.inspect().3 == 0, "actual fixture audio not cleared")
    host.shutdown(); print("Dictation recorded Metal model: exact expected transcript, composer candidate only")
  }
}
