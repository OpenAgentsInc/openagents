import Foundation
import OpenAgentsCore

#if os(iOS)
extension BridgeManager {
    private static let lastHostKey = "oa.bridge.last_host"
    private static let lastPortKey = "oa.bridge.last_port"

    static func saveLastSuccessfulEndpoint(host: String, port: Int) {
        UserDefaults.standard.set(host, forKey: lastHostKey)
        UserDefaults.standard.set(port, forKey: lastPortKey)
        OpenAgentsLog.bridge.debug("saved last endpoint host=\(host, privacy: .private) port=\(port)")
    }
    static func readLastSuccessfulEndpoint() -> (String, Int)? {
        if let h = UserDefaults.standard.string(forKey: lastHostKey) {
            let p = UserDefaults.standard.integer(forKey: lastPortKey)
            if p > 0 { return (h, p) }
        }
        return nil
    }
    /// Decide the initial endpoint for iOS startup.
    /// Order: persisted last‑successful → simulator loopback → configured default.
    static func pickInitialEndpoint() -> (String, Int) {
        // 1) Use persisted last‑successful endpoint if available
        if let last = readLastSuccessfulEndpoint() {
            OpenAgentsLog.bridge.debug("pickInitialEndpoint using persisted host=\(last.0, privacy: .private) port=\(last.1)")
            return last
        }
        #if targetEnvironment(simulator)
        // 2) Simulator: prefer loopback for out‑of‑box dev experience
        OpenAgentsLog.bridge.debug("pickInitialEndpoint simulator loopback")
        return ("127.0.0.1", Int(BridgeConfig.defaultPort))
        #else
        // 3) Device: fall back to configured neutral default host
        OpenAgentsLog.bridge.debug("pickInitialEndpoint fallback default host=\(BridgeConfig.defaultHost, privacy: .private) port=\(BridgeConfig.defaultPort)")
        return (BridgeConfig.defaultHost, Int(BridgeConfig.defaultPort))
        #endif
    }
}
#endif
