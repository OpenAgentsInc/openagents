import Foundation

enum Features {
    /// Enable Claude Code history loading
    /// - Defaults to false.
    /// - Can be enabled by setting env `OPENAGENTS_ENABLE_CLAUDE=1` or `UserDefaults` key `enable_claude = true`.
    static var claudeEnabled: Bool {
        if ProcessInfo.processInfo.environment["OPENAGENTS_ENABLE_CLAUDE"] == "1" { return true }
        return UserDefaults.standard.bool(forKey: "enable_claude")
    }

}
