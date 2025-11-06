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
            return Color(NSColor.windowBackgroundColor)
            #endif
        }
        static var sidebarBackground: Color {
            #if os(iOS)
            return Color(UIColor.secondarySystemBackground)
            #else
            return Color(NSColor.underPageBackgroundColor)
            #endif
        }
        static var border: Color {
            #if os(iOS)
            return Color(UIColor.separator)
            #else
            return Color(NSColor.separatorColor)
            #endif
        }

        // Text (semantic system colors ensure contrast in both modes)
        static var textPrimary: Color {
            #if os(iOS)
            return Color(UIColor.label)
            #else
            return Color(NSColor.labelColor)
            #endif
        }
        static var textSecondary: Color {
            #if os(iOS)
            return Color(UIColor.secondaryLabel)
            #else
            return Color(NSColor.secondaryLabelColor)
            #endif
        }
        static var textTertiary: Color {
            #if os(iOS)
            return Color(UIColor.tertiaryLabel)
            #else
            return Color(NSColor.tertiaryLabelColor)
            #endif
        }
        static var textQuaternary: Color {
            #if os(iOS)
            return Color(UIColor.quaternaryLabel)
            #else
            return Color(NSColor.quaternaryLabelColor)
            #endif
        }

        // Accents and states
        static var accent: Color { Color.accentColor }
        static var success: Color { Color(red: 0.02, green: 0.64, blue: 0.27) } // #04A545
        static var warning: Color { Color(red: 1.0, green: 0.75, blue: 0.0) }    // #FEBF00
        static var danger: Color { Color(red: 0.90, green: 0.02, blue: 0.06) }   // #E7040F

        // Surfaces
        static var card: Color {
            #if os(iOS)
            return Color(UIColor.secondarySystemBackground)
            #else
            return Color(NSColor.underPageBackgroundColor)
            #endif
        }
        static var popover: Color { card }
        static var selection: Color {
            #if os(iOS)
            return Color(UIColor.systemGray5)
            #else
            return Color(NSColor.selectedTextBackgroundColor).opacity(0.2)
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
