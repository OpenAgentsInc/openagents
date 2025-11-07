import SwiftUI

// OpenAgents app theme for Apple-native surfaces
// Dynamic colors: readable in both Light and Dark modes

struct OATheme {
    struct Colors {
        // Base surfaces
        static var background: Color {
            #if os(iOS)
            return Color(UIColor.systemBackground)
            #else
            return Color(hex: "#08090a") // offblack from palette
            #endif
        }
        static var sidebarBackground: Color {
            #if os(iOS)
            return Color(UIColor.secondarySystemBackground)
            #else
            return Color(hex: "#0e0e12") // slightly lighter offblack
            #endif
        }
        static var border: Color {
            #if os(iOS)
            return Color(UIColor.separator)
            #else
            return Color(hex: "#23252a") // border from palette
            #endif
        }

        // Text (semantic system colors ensure contrast in both modes)
        static var textPrimary: Color {
            #if os(iOS)
            return Color(UIColor.label)
            #else
            return Color(hex: "#f7f8f8") // primary from palette
            #endif
        }
        static var textSecondary: Color {
            #if os(iOS)
            return Color(UIColor.secondaryLabel)
            #else
            return Color(hex: "#d0d6e0") // secondary from palette
            #endif
        }
        static var textTertiary: Color {
            #if os(iOS)
            return Color(UIColor.tertiaryLabel)
            #else
            return Color(hex: "#8a8f98") // tertiary from palette
            #endif
        }
        static var textQuaternary: Color {
            #if os(iOS)
            return Color(UIColor.quaternaryLabel)
            #else
            return Color(hex: "#62666d") // quaternary from palette
            #endif
        }

        // Accents and states
        static var accent: Color { Color.accentColor }
        static var success: Color { Color(hex: "#04A545") }  // green from palette
        static var warning: Color { Color(hex: "#FEBF00") }  // yellow from palette
        static var danger: Color { Color(hex: "#e7040f") }   // brightRed from palette

        // Surfaces
        static var card: Color {
            #if os(iOS)
            return Color(UIColor.secondarySystemBackground)
            #else
            return Color(hex: "#08090a") // card from palette (same as background)
            #endif
        }
        static var popover: Color { card }
        static var selection: Color {
            #if os(iOS)
            return Color(UIColor.systemGray5)
            #else
            return Color(hex: "#23252a").opacity(0.5) // border color with opacity
            #endif
        }
    }
}

// MARK: - Hex convenience (kept for any brand accents if needed)
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
