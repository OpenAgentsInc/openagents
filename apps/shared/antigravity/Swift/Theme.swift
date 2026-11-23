// Firm reason: 6 (cross-Apple iOS/macOS SwiftUI OLED design tokens, vector mark, and clipboard helper).
import SwiftUI
#if os(iOS)
import UIKit
#elseif os(macOS)
import AppKit
#endif

enum OATheme {
    static let background = Color(red: 0, green: 0, blue: 0) // #000000 pure OLED black
    static let surface = Color(red: 0.051, green: 0.051, blue: 0.051) // #0D0D0D
    static let elevated = Color(red: 0.086, green: 0.086, blue: 0.086) // #161616
    static let hairline = Color(red: 0.149, green: 0.149, blue: 0.149) // #262626
    static let hairlineStrong = Color(red: 0.25, green: 0.25, blue: 0.25) // #404040
    static let textPrimary = Color.white
    static let textSecondary = Color(red: 0.639, green: 0.639, blue: 0.639) // #A3A3A3
    static let textMuted = Color(red: 0.451, green: 0.451, blue: 0.451) // #737373

    static let statusReady = Color(red: 0.20, green: 0.83, blue: 0.48)
    static let statusWarn = Color(red: 0.96, green: 0.72, blue: 0.24)
    static let statusError = Color(red: 0.94, green: 0.33, blue: 0.31)

    static let diffAddBg = Color(red: 0.05, green: 0.18, blue: 0.09)
    static let diffAddText = Color(red: 0.56, green: 0.94, blue: 0.66)
    static let diffDelBg = Color(red: 0.21, green: 0.06, blue: 0.06)
    static let diffDelText = Color(red: 0.98, green: 0.60, blue: 0.60)
}

enum OAClipboard {
    static func copy(_ text: String) {
        #if os(iOS)
        UIPasteboard.general.string = text
        #elseif os(macOS)
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
        #endif
    }
}

/// Minimal geometric Antigravity badge rendered natively in SwiftUI.
struct AntigravityMark: View {
    var size: CGFloat = 14
    var color: Color = OATheme.textSecondary

    var body: some View {
        Canvas { ctx, sz in
            let w = sz.width
            let h = sz.height
            var path = Path()
            path.move(to: CGPoint(x: w * 0.5, y: h * 0.08))
            path.addLine(to: CGPoint(x: w * 0.88, y: h * 0.88))
            path.addLine(to: CGPoint(x: w * 0.67, y: h * 0.88))
            path.addLine(to: CGPoint(x: w * 0.5, y: h * 0.46))
            path.addLine(to: CGPoint(x: w * 0.33, y: h * 0.88))
            path.addLine(to: CGPoint(x: w * 0.12, y: h * 0.88))
            path.closeSubpath()
            ctx.fill(path, with: .color(color))
        }
        .frame(width: size, height: size)
        .accessibilityHidden(true)
    }
}

enum OATimeFormat {
    static func shortAge(updatedAtMs: Int64, now: Date = Date()) -> String {
        guard updatedAtMs > 0 else { return "" }
        let delta = max(0, Int(now.timeIntervalSince1970) - Int(updatedAtMs / 1000))
        if delta < 60 { return "now" }
        if delta < 3600 { return "\(delta / 60)m" }
        if delta < 86_400 { return "\(delta / 3600)h" }
        if delta < 86_400 * 7 { return "\(delta / 86_400)d" }
        return "\(delta / (86_400 * 7))w"
    }
}
