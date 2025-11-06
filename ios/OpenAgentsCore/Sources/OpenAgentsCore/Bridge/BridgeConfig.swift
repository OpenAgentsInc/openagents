import Foundation

public enum BridgeConfig {
    public static let serviceType = "_openagents._tcp"
    public static let defaultPort: UInt16 = 9099
    // Default LAN host for manual connect flows on iOS.
    // This mirrors the ManualConnectSheet default.
    public static let defaultHost: String = "192.168.1.11"
}
