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
            return Color(hex: "#08090a") // color-bg-primary / level-0
            #endif
        }
        static var sidebarBackground: Color {
            #if os(iOS)
            return Color(UIColor.secondarySystemBackground)
            #else
            return Color(hex: "#1c1c1f") // color-bg-secondary
            #endif
        }
        static var border: Color {
            #if os(iOS)
            return Color(UIColor.separator)
            #else
            return Color(hex: "#23252a") // color-border-primary
            #endif
        }

        // Text (semantic system colors ensure contrast in both modes)
        static var textPrimary: Color {
            #if os(iOS)
            return Color(UIColor.label)
            #else
            return Color(hex: "#f7f8f8") // color-text-primary
            #endif
        }
        static var textSecondary: Color {
            #if os(iOS)
            return Color(UIColor.secondaryLabel)
            #else
            return Color(hex: "#d0d6e0") // color-text-secondary
            #endif
        }
        static var textTertiary: Color {
            #if os(iOS)
            return Color(UIColor.tertiaryLabel)
            #else
            return Color(hex: "#8a8f98") // color-text-tertiary
            #endif
        }
        static var textQuaternary: Color {
            #if os(iOS)
            return Color(UIColor.quaternaryLabel)
            #else
            return Color(hex: "#62666d") // color-text-quaternary
            #endif
        }

        // Accents and states
        static var accent: Color { Color(hex: "#7170ff") } // color-accent
        static var success: Color { Color(hex: "#04A545") }  // green from palette
        static var warning: Color { Color(hex: "#FEBF00") }  // yellow from palette
        static var danger: Color { Color(hex: "#e7040f") }   // brightRed from palette

        // Surfaces
        static var card: Color {
            #if os(iOS)
            return Color(UIColor.secondarySystemBackground)
            #else
            return Color(hex: "#232326") // color-bg-tertiary
            #endif
        }
        static var popover: Color { card }
        static var selection: Color {
            #if os(iOS)
            return Color(UIColor.systemGray5)
            #else
            return Color(hex: "#23252a").opacity(0.5) // border translucent
            #endif
        }

        // Extended grayscale surfaces (for chat UI neutrality)
        static var bgTertiary: Color { Color(hex: "#232326") }
        static var bgQuaternary: Color { Color(hex: "#28282c") }
        static var borderSecondary: Color { Color(hex: "#34343a") }
        static var borderTertiary: Color { Color(hex: "#3e3e44") }
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
