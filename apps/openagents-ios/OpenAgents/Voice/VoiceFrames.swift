import Foundation

/// The voice wire in Swift: builds `oa_voice_msg` values and encodes them
/// with `oa_voice_encode` (verse/platform/voice.h, voice-wire.coax), and
/// turns decoded frames into Swift values. No protobuf is written by hand
/// here. Not thread-safe: the voice worker owns one.
final class VoiceWire {
    private let out = UnsafeMutablePointer<UInt8>.allocate(capacity: Int(OA_VOICE_MAX_FRAME_BYTES))
    // Strings for one message are copied here, then the arena is reset.
    private let arena = UnsafeMutablePointer<UInt8>.allocate(capacity: 4096)
    private var used = 0
    deinit { out.deallocate(); arena.deallocate() }

    func text(_ s: String) -> oa_voice_bytes {
        let utf8 = Array(s.utf8.prefix(4096 - used))
        guard !utf8.isEmpty else { return oa_voice_bytes(ptr: nil, len: 0) }
        let at = arena + used
        utf8.withUnsafeBufferPointer { at.update(from: $0.baseAddress!, count: $0.count) }
        used += utf8.count
        return oa_voice_bytes(ptr: UnsafePointer(at), len: UInt32(utf8.count))
    }
    /// Forget the strings copied for the last message.
    func reset() { used = 0 }
    /// Encodes and resets the arena. Nil when the engine refuses the frame.
    func encode(_ m: inout oa_voice_msg) -> Data? {
        defer { used = 0 }
        let n = oa_voice_encode(out, Int(OA_VOICE_MAX_FRAME_BYTES), &m)
        return n == 0 ? nil : Data(bytes: out, count: n)
    }

    static func string(_ b: oa_voice_bytes) -> String {
        guard let p = b.ptr, b.len > 0 else { return "" }
        return String(decoding: UnsafeBufferPointer(start: p, count: Int(b.len)), as: UTF8.self)
    }
}

/// Every frame the host can send, copied out of the decoder's borrowed views.
enum VoiceInbound {
    case welcome(session: String, provider: String, outputRate: UInt32, maxTurnMs: UInt32, meter: oa_voice_meter_report?)
    case audio(sequence: UInt32, rate: UInt32, epoch: UInt64, utterance: String, samples: [Int16])
    case control(kind: UInt32, turn: String, epoch: UInt64, utterance: String)
    case turn(turn: String, phase: UInt32, text: String, epoch: UInt64)
    case speech(text: String, utterance: String, rate: UInt32, epoch: UInt64, turn: String)
    case route(turn: String, target: String, reason: String)
    case meter(oa_voice_meter_report)
    case error(code: UInt32, message: String, turn: String)
    case features(oa_voice_features)
    case decision(oa_voice_decision, decider: String)

    static func decode(_ data: Data) -> VoiceInbound? {
        guard data.count <= Int(OA_VOICE_MAX_FRAME_BYTES) else { return nil }
        return data.withUnsafeBytes { raw -> VoiceInbound? in
            guard let base = raw.bindMemory(to: UInt8.self).baseAddress else { return nil }
            var m = oa_voice_msg()
            guard oa_voice_decode(base, raw.count, &m) else { return nil }
            let s = VoiceWire.string
            switch m.kind {
            case UInt32(OA_MSG_WELCOME.rawValue):
                let w = m.u.welcome
                return .welcome(session: s(w.session), provider: s(w.provider), outputRate: w.output_sample_rate,
                                maxTurnMs: w.max_turn_ms, meter: w.has_meter ? w.meter : nil)
            case UInt32(OA_MSG_AUDIO.rawValue):
                let a = m.u.audio
                let count = Int(a.data.len) / 2
                var samples = [Int16](repeating: 0, count: count)
                if let p = a.data.ptr, count > 0 {
                    samples.withUnsafeMutableBytes { $0.copyMemory(from: UnsafeRawBufferPointer(start: p, count: count * 2)) }
                    for i in samples.indices { samples[i] = Int16(littleEndian: samples[i]) }
                }
                return .audio(sequence: a.sequence, rate: a.sample_rate, epoch: a.media_epoch, utterance: s(a.utterance), samples: samples)
            case UInt32(OA_MSG_CONTROL.rawValue):
                let c = m.u.control
                return .control(kind: c.kind, turn: s(c.turn), epoch: c.media_epoch, utterance: s(c.utterance))
            case UInt32(OA_MSG_TURN_EVENT.rawValue):
                let t = m.u.turn_event
                return .turn(turn: s(t.turn), phase: t.phase, text: s(t.text), epoch: t.media_epoch)
            case UInt32(OA_MSG_SPEECH.rawValue):
                let sp = m.u.speech
                return .speech(text: sp.has_utterance ? s(sp.utterance.text) : "", utterance: sp.has_utterance ? s(sp.utterance.id) : "",
                               rate: sp.sample_rate, epoch: sp.media_epoch, turn: s(sp.turn))
            case UInt32(OA_MSG_ROUTE.rawValue):
                let r = m.u.route
                return .route(turn: s(r.turn), target: s(r.target), reason: s(r.reason))
            case UInt32(OA_MSG_METER.rawValue):
                return .meter(m.u.meter)
            case UInt32(OA_MSG_ERROR.rawValue):
                let e = m.u.error
                return .error(code: e.code, message: s(e.message), turn: s(e.turn))
            case UInt32(OA_MSG_FEATURES.rawValue):
                return .features(m.u.features)
            case UInt32(OA_MSG_DECISION.rawValue):
                var d = m.u.decision
                let decider = s(d.decider)
                d.decider = oa_voice_bytes(ptr: nil, len: 0) // the borrowed view dies with `data`
                return .decision(d, decider: decider)
            default:
                return nil
            }
        }
    }
}
