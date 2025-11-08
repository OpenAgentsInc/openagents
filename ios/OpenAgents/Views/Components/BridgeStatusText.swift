import OpenAgentsCore

enum BridgeStatusText {
    enum PlatformFlavor { case ios, macos }

    static func text(for status: BridgeManager.Status, platform: PlatformFlavor) -> String {
        switch (platform, status) {
        case (.ios, .idle): return "Idle"
        case (.ios, .discovering): return "Discovering Desktop..."
        case (.ios, .connecting(let h, let p)): return "Connecting to \(h):\(p)"
        case (.ios, .handshaking(let h, let p)): return "Handshaking with \(h):\(p)"
        case (.ios, .connected): return "Connected"
        case (.ios, .advertising): return "Advertising (Server Mode)"
        case (.ios, .error): return "Error"

        case (.macos, .idle): return "Idle"
        case (.macos, .advertising): return "Ready for Connections"
        case (.macos, .discovering): return "Discovering..."
        case (.macos, .connecting(let h, let p)): return "Connecting to \(h):\(p)"
        case (.macos, .handshaking(let h, let p)): return "Handshaking with \(h):\(p)"
        case (.macos, .connected(let h, let p)): return "Connected to \(h):\(p)"
        case (.macos, .error): return "Error"
        }
    }
}

