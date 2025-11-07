import Foundation

public enum BridgeConfig {
    public static let serviceType = "_openagents._tcp"
    public static let defaultPort: UInt16 = 9099
    // Default host value used to prefill manual connect UI.
    // Leave blank to avoid shipping developer-specific LAN IPs.
    public static let defaultHost: String = ""
}
