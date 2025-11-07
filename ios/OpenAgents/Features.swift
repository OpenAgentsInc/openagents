import Foundation

enum Features {
    /// Enable Claude Code history loading
    /// - Defaults to true.
    /// - Can be disabled by setting env `OPENAGENTS_ENABLE_CLAUDE=0` or `UserDefaults` key `enable_claude = false`.
    static var claudeEnabled: Bool {
        if ProcessInfo.processInfo.environment["OPENAGENTS_ENABLE_CLAUDE"] == "0" { return false }
        if UserDefaults.standard.object(forKey: "enable_claude") != nil {
            return UserDefaults.standard.bool(forKey: "enable_claude")
        }
        return true // Default to enabled
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

    /// On‑device Foundation Models usage (probe/summarizer). Default off.
    /// Enable with env `OPENAGENTS_ENABLE_FM=1` or UserDefaults `enable_foundation_models=true`.
    static var foundationModelsEnabled: Bool {
        if ProcessInfo.processInfo.environment["OPENAGENTS_ENABLE_FM"] == "1" { return true }
        return UserDefaults.standard.bool(forKey: "enable_foundation_models")
    }

    /// Autoload latest provider thread on connect (legacy behavior). Default: false.
    /// Enable with env `OPENAGENTS_AUTOLOAD_LATEST=1` or UserDefaults `autoload_latest=true`.
    static var autoloadLatestOnConnect: Bool {
        if ProcessInfo.processInfo.environment["OPENAGENTS_AUTOLOAD_LATEST"] == "1" { return true }
        return UserDefaults.standard.bool(forKey: "autoload_latest")
    }

    /// Show simplified macOS UI (bridge status + working directory only). Default: true.
    /// Disable with env `OPENAGENTS_SIMPLIFIED_MACOS=0` or UserDefaults `simplified_macos_ui=false`.
    static var simplifiedMacOSUI: Bool {
        if ProcessInfo.processInfo.environment["OPENAGENTS_SIMPLIFIED_MACOS"] == "0" { return false }
        if UserDefaults.standard.object(forKey: "simplified_macos_ui") != nil {
            return UserDefaults.standard.bool(forKey: "simplified_macos_ui")
        }
        return true // Default to simplified UI
    }

    /// Show simplified iOS UI (bridge status + working directory + agents). Default: true.
    /// Disable with env `OPENAGENTS_SIMPLIFIED_IOS=0` or UserDefaults `simplified_ios_ui=false`.
    static var simplifiedIOSUI: Bool {
        if ProcessInfo.processInfo.environment["OPENAGENTS_SIMPLIFIED_IOS"] == "0" { return false }
        if UserDefaults.standard.object(forKey: "simplified_ios_ui") != nil {
            return UserDefaults.standard.bool(forKey: "simplified_ios_ui")
        }
        return true // Default to simplified UI
    }

    /// Use Tinyvex server instead of DesktopWebSocketServer on macOS (development only)
    /// Enable with env `OPENAGENTS_USE_TINYVEX=1` or UserDefaults `use_tinyvex=true`.
    static var useTinyvexServer: Bool {
        #if os(macOS)
        if ProcessInfo.processInfo.environment["OPENAGENTS_USE_TINYVEX"] == "1" { return true }
        return UserDefaults.standard.bool(forKey: "use_tinyvex")
        #else
        return false
        #endif
    }
}
