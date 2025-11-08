// Minimal macOS-only shim to allow linking the local Nostr SDK
// The iOS build will compile this file but exclude macOS-specific imports.

#if os(macOS)
import Foundation
import NostrSDK

public enum NostrShim {
    public static func available() -> Bool { true }
}
#else
// iOS (and other platforms) receive an empty shim so the module builds cleanly
public enum NostrShim {
    public static func available() -> Bool { false }
}
#endif

