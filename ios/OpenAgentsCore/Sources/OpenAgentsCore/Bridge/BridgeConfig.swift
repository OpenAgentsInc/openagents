import Foundation

public enum BridgeConfig {
    public static let serviceType = "_openagents._tcp"
    public static let defaultPort: UInt16 = 9099
    // Default host for iOS devices to connect to this Mac.
    // Auto-filled in Manual Connect UI and used as first pick on device.
    public static let defaultHost: String = "192.168.1.11"
}
