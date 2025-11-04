import SwiftUI

// OpenAgents app theme for Apple-native surfaces
// Mirrors packages/openagents-theme/colors.js (dark/offblack palette)

struct OATheme {
    struct Colors {
        // Base
        static let background = Color(hex: "#08090a")
        static let sidebarBackground = Color(hex: "#0e0e12")
        static let border = Color(hex: "#23252a")

        // Text
        static let textPrimary = Color(hex: "#f7f8f8")
        static let textSecondary = Color(hex: "#d0d6e0")
        static let textTertiary = Color(hex: "#8a8f98")
        static let textQuaternary = Color(hex: "#62666d")

        // Accents and states
        static let accent = Color(hex: "#f7f8f8")
        static let success = Color(hex: "#04A545")
        static let warning = Color(hex: "#FEBF00")
        static let danger = Color(hex: "#e7040f")

        // Surfaces
        static let card = Color(hex: "#08090a")
        static let popover = Color(hex: "#08090a")
        static let selection = Color.white.opacity(0.08)
    }
}

// MARK: - Hex convenience
extension Color {
    init(hex: String) {
        let r, g, b, a: Double
        var s = hex.trimmingCharacters(in: .whitespacesAndNewlines)
        if s.hasPrefix("#") { s.removeFirst() }
        if s.count == 6 { s.append("ff") }
        var v: UInt64 = 0
        Scanner(string: s).scanHexInt64(&v)
        r = Double((v & 0xff000000) >> 24) / 255.0
        g = Double((v & 0x00ff0000) >> 16) / 255.0
        b = Double((v & 0x0000ff00) >> 8) / 255.0
        a = Double(v & 0x000000ff) / 255.0
        self = Color(.sRGB, red: r, green: g, blue: b, opacity: a)
    }
}

