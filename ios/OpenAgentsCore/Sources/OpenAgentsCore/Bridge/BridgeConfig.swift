import Foundation

public enum BridgeConfig {
    public static let serviceType = "_openagents._tcp"
    public static let defaultPort: UInt16 = 9099
    // Default host used only as a neutral fallback when no prior
    // connection is known. Simulator uses loopback automatically.
    // We deliberately avoid hard‑coding a developer‑specific LAN IP.
    public static let defaultHost: String = "127.0.0.1"
}
