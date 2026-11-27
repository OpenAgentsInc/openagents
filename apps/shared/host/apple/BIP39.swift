import Foundation
import CryptoKit
import Security

/// BIP-39 recovery words from entropy using the pinned English wordlist.
/// Words only pass from here to the Spark SDK (or once behind an explicit
/// backup warning); nothing ever logs them.
public enum OABIP39 {
    /// SHA-256 of `bip-0039/english.txt` in bitcoin/bips.
    public static let englishSHA256 = "2f5eed53a4727b4bf8880d8f3f199efc90e58503646d9ff8eff3a2ed3b24dbda"

    /// Loads and verifies `bip39-english.txt` from a bundle (default `.main`).
    public static func englishWords(bundle: Bundle = .main) -> [String]? {
        guard let url = bundle.url(forResource: "bip39-english", withExtension: "txt"),
              let data = try? Data(contentsOf: url) else { return nil }
        return englishWords(fileContents: data)
    }

    /// Loads and verifies `bip39-english.txt` from an explicit file URL.
    public static func englishWords(url: URL) -> [String]? {
        guard let data = try? Data(contentsOf: url) else { return nil }
        return englishWords(fileContents: data)
    }

    public static func englishWords(fileContents data: Data) -> [String]? {
        let digest = SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
        guard digest == englishSHA256, let text = String(data: data, encoding: .utf8) else { return nil }
        let words = text.split(separator: "\n").map(String.init)
        return words.count == 2048 ? words : nil
    }

    /// Derives 12 words (16 bytes) or 24 words (32 bytes) from BIP-39 entropy.
    public static func mnemonic(entropy: Data, words: [String]) -> String? {
        guard entropy.count == 16 || entropy.count == 32, words.count == 2048 else { return nil }
        let checksumBits = entropy.count * 8 / 32
        let checksum = Array(SHA256.hash(data: entropy))[0]
        var bits: [Bool] = []
        bits.reserveCapacity(entropy.count * 8 + checksumBits)
        for byte in entropy {
            for i in (0..<8).reversed() {
                bits.append((byte >> i) & 1 == 1)
            }
        }
        for i in 0..<checksumBits {
            bits.append((checksum >> (7 - i)) & 1 == 1)
        }
        var out: [String] = []
        out.reserveCapacity(bits.count / 11)
        for start in stride(from: 0, to: bits.count, by: 11) {
            var index = 0
            for bit in bits[start..<(start + 11)] {
                index = (index << 1) | (bit ? 1 : 0)
            }
            out.append(words[index])
        }
        return out.joined(separator: " ")
    }

    /// Decodes a 12-word (16-byte) or 24-word (32-byte) BIP-39 mnemonic and verifies its SHA-256 checksum.
    public static func entropy(mnemonic: String, words: [String]) -> Data? {
        guard words.count == 2048 else { return nil }
        let parts = mnemonic
            .lowercased()
            .split { $0.isWhitespace }
            .map(String.init)
        guard parts.count == 12 || parts.count == 24 else { return nil }
        var indexByWord: [String: Int] = [:]
        indexByWord.reserveCapacity(2048)
        for (i, w) in words.enumerated() { indexByWord[w] = i }
        var bits: [Bool] = []
        bits.reserveCapacity(parts.count * 11)
        for w in parts {
            guard let idx = indexByWord[w] else { return nil }
            for i in (0..<11).reversed() {
                bits.append((idx >> i) & 1 == 1)
            }
        }
        let entropyBytes = parts.count == 12 ? 16 : 32
        let checksumBits = entropyBytes * 8 / 32
        var entropy = Data(count: entropyBytes)
        for b in 0..<entropyBytes {
            var val: UInt8 = 0
            for i in 0..<8 {
                val = (val << 1) | (bits[b * 8 + i] ? 1 : 0)
            }
            entropy[b] = val
        }
        let hashByte = Array(SHA256.hash(data: entropy))[0]
        for i in 0..<checksumBits {
            let expected = ((hashByte >> (7 - i)) & 1) == 1
            if bits[entropyBytes * 8 + i] != expected { return nil }
        }
        return entropy
    }

    /// Generates 16 bytes (128 bits / 12 words) of OS-random entropy.
    public static func generateEntropy16() -> Data? {
        var bytes = [UInt8](repeating: 0, count: 16)
        guard SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes) == errSecSuccess else {
            return nil
        }
        return Data(bytes)
    }
}
