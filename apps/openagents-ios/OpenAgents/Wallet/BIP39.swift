import Foundation
import CryptoKit

/// BIP-39 recovery words from entropy, with the standard English list.
/// The words only ever pass from here to the Spark SDK; nothing logs them.
enum BIP39 {
    /// SHA-256 of `bip-0039/english.txt` in bitcoin/bips.
    static let englishSHA256 = "2f5eed53a4727b4bf8880d8f3f199efc90e58503646d9ff8eff3a2ed3b24dbda"

    /// The English list from the app bundle, or nil if it isn't the exact list.
    static func englishWords(bundle: Bundle = .main) -> [String]? {
        guard let url = bundle.url(forResource: "bip39-english", withExtension: "txt"),
              let data = try? Data(contentsOf: url) else { return nil }
        return englishWords(fileContents: data)
    }

    static func englishWords(fileContents data: Data) -> [String]? {
        let digest = SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
        guard digest == englishSHA256, let text = String(data: data, encoding: .utf8) else { return nil }
        let words = text.split(separator: "\n").map(String.init)
        return words.count == 2048 ? words : nil
    }

    /// The words for 16 bytes (12 words) or 32 bytes (24 words) of entropy.
    static func mnemonic(entropy: Data, words: [String]) -> String? {
        guard entropy.count == 16 || entropy.count == 32, words.count == 2048 else { return nil }
        let checksumBits = entropy.count * 8 / 32
        let checksum = Array(SHA256.hash(data: entropy))[0]
        var bits: [Bool] = []
        for byte in entropy { for i in (0..<8).reversed() { bits.append((byte >> i) & 1 == 1) } }
        for i in 0..<checksumBits { bits.append((checksum >> (7 - i)) & 1 == 1) }
        var out: [String] = []
        for start in stride(from: 0, to: bits.count, by: 11) {
            var index = 0
            for bit in bits[start..<start + 11] { index = index << 1 | (bit ? 1 : 0) }
            out.append(words[index])
        }
        return out.joined(separator: " ")
    }
}
