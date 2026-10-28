import Foundation

/// A bundled 16 kHz mono 16-bit WAV, streamed through the same ring and
/// uplink path as the microphone so simulator runs are deterministic
/// (launch argument `-OAVoiceFixture NAME`, NAME without `.wav`).
struct VoiceFixture {
    let rate: UInt32
    let samples: [Int16]

    enum Failure: LocalizedError {
        case missing(String), unreadable(String)
        var errorDescription: String? {
            switch self {
            case .missing(let n): return "No voice fixture named \(n)"
            case .unreadable(let n): return "The voice fixture \(n) isn't 16-bit mono PCM"
            }
        }
    }

    init(named name: String) throws {
        guard let url = Bundle.main.url(forResource: name, withExtension: "wav") else { throw Failure.missing(name) }
        let data = try Data(contentsOf: url)
        func u32(_ o: Int) -> UInt32 { data[o..<o+4].enumerated().reduce(0) { $0 | UInt32($1.element) << (8 * $1.offset) } }
        func u16(_ o: Int) -> UInt16 { UInt16(data[o]) | UInt16(data[o+1]) << 8 }
        guard data.count >= 12, data.prefix(4) == Data("RIFF".utf8), data[8..<12] == Data("WAVE".utf8) else { throw Failure.unreadable(name) }
        var at = 12, rate: UInt32 = 0, format: UInt16 = 0, channels: UInt16 = 0, bits: UInt16 = 0
        var pcm: Range<Int>?
        // Walk the chunks: fmt and data, skipping anything else (LIST, FLLR).
        while at + 8 <= data.count {
            let id = data[at..<at+4], size = Int(u32(at + 4)), body = at + 8
            guard body + size <= data.count else { break }
            if id == Data("fmt ".utf8), size >= 16 {
                format = u16(body); channels = u16(body + 2); rate = u32(body + 4); bits = u16(body + 14)
            } else if id == Data("data".utf8) {
                pcm = body..<(body + size)
            }
            at = body + size + (size & 1)
        }
        guard format == 1, channels == 1, bits == 16, rate > 0, let pcm else { throw Failure.unreadable(name) }
        let count = pcm.count / 2
        var samples = [Int16](repeating: 0, count: count)
        _ = samples.withUnsafeMutableBytes { out in data.copyBytes(to: out, from: pcm.lowerBound..<(pcm.lowerBound + count * 2)) }
        self.rate = rate
        self.samples = samples.map { Int16(littleEndian: $0) }
    }
}
