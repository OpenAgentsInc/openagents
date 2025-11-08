import Foundation
import OpenAgentsNostr

public struct NostrKeys: Equatable {
    public let privateHex: String
    public let nsec: String
    public let publicHex: String
    public let npub: String
}

public enum NostrSupport {
    /// Generate a Nostr keypair using the NostrSDK via the shim.
    /// Returns nil if the SDK is unavailable or generation fails.
    public static func generate() -> NostrKeys? {
        guard NostrShim.available(), let keys = NostrBridge.generateKeypair() else { return nil }
        return NostrKeys(privateHex: keys.privHex, nsec: keys.nsec, publicHex: keys.pubHex, npub: keys.npub)
    }
}

