// Thin shim that centralizes access to NostrSDK so the app can
// call simple helpers via OpenAgentsCore without importing NostrSDK directly.

import Foundation
import NostrSDK

public enum NostrShim {
    public static func available() -> Bool { true }
}

public enum NostrBridge {
    /// Generate a fresh Nostr keypair via NostrSDK.
    /// Returns nil if generation fails.
    public static func generateKeypair() -> (privHex: String, nsec: String, pubHex: String, npub: String)? {
        guard let kp = NostrSDK.Keypair() else { return nil }
        let priv = kp.privateKey
        let pub = kp.publicKey
        return (priv.hex, priv.nsec, pub.hex, pub.npub)
    }
}
