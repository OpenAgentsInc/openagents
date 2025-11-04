import Foundation

enum Features {
    /// Enable Claude Code history loading
    /// - Defaults to false.
    /// - Can be enabled by setting env `OPENAGENTS_ENABLE_CLAUDE=1` or `UserDefaults` key `enable_claude = true`.
    static var claudeEnabled: Bool {
        if ProcessInfo.processInfo.environment["OPENAGENTS_ENABLE_CLAUDE"] == "1" { return true }
        return UserDefaults.standard.bool(forKey: "enable_claude")
    }

    /// Show raw provider JSON/event blobs (including tool args/results pretty JSON).
    /// Default: false. Enable with env `OPENAGENTS_SHOW_RAWJSON=1` or UserDefaults `show_raw_json=true`.
    static var showRawJSON: Bool {
        if ProcessInfo.processInfo.environment["OPENAGENTS_SHOW_RAWJSON"] == "1" { return true }
        return UserDefaults.standard.bool(forKey: "show_raw_json")
    }

    /// Multicast/Bonjour discovery for the iOS bridge.
    /// Default: false (to avoid special entitlements); enable with env `OPENAGENTS_ENABLE_MULTICAST=1`
    /// or UserDefaults `enable_multicast=true`.
    static var multicastEnabled: Bool {
        if ProcessInfo.processInfo.environment["OPENAGENTS_ENABLE_MULTICAST"] == "1" { return true }
        return UserDefaults.standard.bool(forKey: "enable_multicast")
    }

}
