// Firm reason: 6 (AVFoundation and CoreAudio: a page's sound through the Apple audio host).
// A page's sound (aethyr/host/README.md, Audio), for the page host on Apple (page-host.h). It is
// the Apple audio host the Metaverse Engine desktop app plays its realm with (apps/aethyrforge-desktop/Sources/
// MetalAudio.swift, ForgeMetalAudio): the shared Coax interpreter (aethyr/host/native-audio.coax:
// host-pcm, the session, the bank, the memory tickets, the decoders) behind audio-host.h, and an
// AVAudioEngine source node that copies its 1024-frame blocks to the device. This file only hands
// it the page: the bank the realm renders, the packets it says, the gesture and the cover.
//
// `offline` is a shot's: the device is never opened (tests never use the owner's speakers), the
// engine starts as the host contract's explicit offline test permission allows, and every block the
// interpreter renders in the page's time is kept, so a shot can write what it heard.
import AVFoundation
import Foundation

@MainActor
final class AEPageAudio {
  let audio: ForgeMetalAudio
  let offline: Bool
  private var owed = 0.0              // frames of the page's time not yet rendered (offline)
  private(set) var heard: [Float] = []   // offline: the blocks rendered, interleaved stereo at 48 kHz
  private(set) var names: [String: Int] = [:]   // each sound a packet named that the interpreter took
  private var gestured = false, shown = true

  // The page's library is the bank's and the packets' owner; `resources` holds web/ (the site's
  // sound files, as the page names them).
  init(page: AEPageHost, resources: URL, offline: Bool) {
    self.offline = offline
    audio = ForgeMetalAudio(host: Unmanaged.passUnretained(page).toOpaque(), resources: resources, muted: offline,
                            bankDirectory: FileManager.default.temporaryDirectory.appendingPathComponent("everglade-audio-bank-\(getpid())"))
    if offline { gesture() }
  }
  // A key or a press in the page: the platform's gesture. Audio starts at the first one.
  func gesture() {
    if gestured { return }
    gestured = true
    if offline { audio.bank(); audio.source.sessionEvent(1) } else { audio.start() }
  }
  // The page is on screen or covered. A covered page's audio clock stops; its packets still drain.
  func show(_ on: Bool) {
    if on == shown { return }
    shown = on
    if offline { audio.source.sessionEvent(on ? 3 : 2) } else { audio.setPaused(!on) }
  }
  // After each handoff to the page (a frame, or its messages while covered): the packet it said,
  // once. Offline, the interpreter then renders the blocks that cover `dt` of the page's time.
  func step(_ dt: Float) {
    drainCounting()
    guard offline, shown, audio.source.sessionOffer().active else { return }
    owed += Double(dt) * 48000
    while owed >= 1024 {
      guard let block = audio.source.offlineBlock() else { owed = 0; break }
      heard.append(contentsOf: block); owed -= 1024
    }
  }
  private func drainCounting() {
    audio.bank()
    var n = 0
    guard let p = mf_sound_commands(audio.host, &n), n > 0 else { return }
    for line in Data(bytes: p, count: n).split(separator: 10) {
      let result = audio.execute(Data(line))
      if result >= 0, let name = line.split(separator: 32).first.map({ String(decoding: $0, as: UTF8.self) }), !name.hasPrefix("!"), !name.hasPrefix("-") {
        names[name.hasPrefix("~") ? String(name.dropFirst()) : name, default: 0] += 1
      }
    }
  }
  // What the page's sound did: packets taken, voices scheduled, decodes, the interpreter's counts,
  // the names it played, and (offline) the loudest sample rendered.
  func summary() -> String {
    let info = audio.source.observations()
    let peak = heard.reduce(Float(0)) { max($0, abs($1)) }
    let played = names.keys.sorted().map { "\($0)x\(names[$0]!)" }.joined(separator: ",")
    return "audio: packets \(audio.packets) scheduled \(audio.scheduled) bank \(audio.bankWav.count) decodes \(audio.sdkDecodes)+\(audio.companionDecodes) accepted \(info["accepted"] ?? 0) refused \(info["refused"] ?? 0) frames \(info["frames"] ?? 0) seconds \(String(format: "%.2f", Double(heard.count) / 96000)) peak \(String(format: "%.4f", peak)) played \(played.isEmpty ? "-" : played)"
  }
  // Offline: what was heard, as a 48 kHz stereo float WAV.
  func write(_ path: String) -> Bool {
    guard offline, let format = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: 48000, channels: 2, interleaved: false),
          let file = try? AVAudioFile(forWriting: URL(fileURLWithPath: path), settings: format.settings, commonFormat: .pcmFormatFloat32, interleaved: false) else { return false }
    let frames = heard.count / 2
    guard frames > 0, let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: AVAudioFrameCount(frames)), let planes = buffer.floatChannelData else { return frames == 0 }
    buffer.frameLength = AVAudioFrameCount(frames)
    for i in 0..<frames { planes[0][i] = heard[i * 2]; planes[1][i] = heard[i * 2 + 1] }
    do { try file.write(from: buffer) } catch { return false }
    return true
  }
  func close() { audio.close() }
}
