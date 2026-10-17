import UIKit

/// Coder Light and Coder Noir, from `crates/oa-tokens/src/palette.rs`.
/// UIKit follows the device's selected appearance.
enum Palette {
    static func dynamic(_ light: UInt32, _ dark: UInt32, alpha: CGFloat = 1) -> UIColor {
        UIColor { traits in
            UIColor(hex: traits.userInterfaceStyle == .dark ? dark : light, alpha: alpha)
        }
    }

    static let background = dynamic(0xFFFFFF, 0x0A0A0A)
    static let rowActive = dynamic(0xF3F3F3, 0x191919)
    static let elevated = dynamic(0xFFFFFF, 0x191919)
    static let card = dynamic(0xF9F9F9, 0x101010)
    static let text = dynamic(0x0D0D0D, 0xEDEDED)
    static let secondary = dynamic(0x5D5D5D, 0x818181)
    static let tertiary = dynamic(0x8F8F8F, 0x656565)
    static let hairline = dual(UIColor(hex: 0x0D0D0D, alpha: 0.10), UIColor(hex: 0x2C2C2C))
    static let accent = dynamic(0x181818, 0xEDEDED)
    static let controlFill = dynamic(0x0D0D0D, 0xEDEDED, alpha: 0.075)
    static let accentSoft = dynamic(0xF3F3F3, 0x191919)
    static let danger = dynamic(0x911E1B, 0xFF4D42)
    static let success = dynamic(0x00692A, 0x9FD08A)
    static let warning = dynamic(0x923B0F, 0xE6C15C)
    static let userBubble = dynamic(0xF3F3F3, 0x191919)
    static let codeBackground = dynamic(0xF9F9F9, 0x0E0E0E)
    static let codeBorder = hairline
    static let chip = dynamic(0xF3F3F3, 0x1E1E1E)

    /// Project tones — the desktop's monogram palette (project_icon.rs):
    /// slate, blue, violet, rose, amber, emerald, teal, orange. Indexed by the
    /// core's `project_color_index` (FNV-1a of the project path).
    static let projectDots: [UIColor] = [
        dynamic(0x475569, 0x94A3B8), dynamic(0x2563EB, 0x93C5FD), dynamic(0x7C3AED, 0xC4B5FD),
        dynamic(0xBE123C, 0xFDA4AF), dynamic(0xA16207, 0xFCD34D), dynamic(0x047857, 0x6EE7B7),
        dynamic(0x0F766E, 0x5EEAD4), dynamic(0xC2410C, 0xFDBA74),
    ]

    /// Secondary project and branch labels.
    static let subline = secondary

    static func dual(_ light: UIColor, _ dark: UIColor) -> UIColor {
        UIColor { $0.userInterfaceStyle == .dark ? dark : light }
    }

    static func color(_ role: ColorRole) -> UIColor {
        switch role {
        case .text: text
        case .textSecondary: secondary
        case .textTertiary: tertiary
        case .link, .accent: accent
        case .danger: danger
        case .success: success
        case .warning: warning
        case .inlineCodeText: text
        case .inlineCodeBackground: chip
        case .codeText: text
        case .codeBackground: codeBackground
        case .codeBorder: codeBorder
        case .quoteBar: hairline
        case .rule: hairline
        case .tableBorder: hairline
        case .tableHeaderBackground: card
        case .userBubble: userBubble
        case .chipBackground: chip
        case .syntaxKeyword: dynamic(0x532D8D, 0xD093D0)
        case .syntaxString: success
        case .syntaxComment: secondary
        case .syntaxNumber, .syntaxConstant: warning
        case .syntaxFunction: dynamic(0x0169CC, 0x7FB2E8)
        case .syntaxType: dynamic(0x0E7490, 0x74CFD1)
        case .syntaxVariable: text
        case .syntaxProperty: dynamic(0x0E7490, 0x74CFD1)
        case .syntaxOperator, .syntaxPunctuation: dynamic(0x5D5D5D, 0xC9C9C9)
        case .syntaxTag: dynamic(0x532D8D, 0xD093D0)
        case .syntaxAttribute: warning
        case .syntaxEscape: dynamic(0x0E7490, 0x74CFD1)
        // Tool groups — desktop openagents tokens (see tools.rs).
        case .textFaint: tertiary
        case .textSoft: text.withAlphaComponent(0.85)
        case .toolRail: dual(UIColor(white: 0, alpha: 0.162), UIColor(white: 1, alpha: 0.12))
        case .toolBadge: dual(UIColor(white: 0, alpha: 0.06), UIColor(white: 1, alpha: 0.06))
        case .toolWell: dual(UIColor(white: 1, alpha: 0.16), UIColor(white: 0, alpha: 0.16))
        case .agentCard: dual(UIColor(white: 0, alpha: 0.03), UIColor(white: 1, alpha: 0.03))
        case .agentCardBorder: dual(UIColor(white: 0, alpha: 0.0945), UIColor(white: 1, alpha: 0.07))
        case .agentTile: dual(UIColor(white: 0, alpha: 0.08), UIColor(white: 1, alpha: 0.08))
        case .diffAddWash: success.withAlphaComponent(0.055)
        case .diffDelWash: danger.withAlphaComponent(0.055)
        case .diffAddBar: success.withAlphaComponent(0.55)
        case .diffDelBar: danger.withAlphaComponent(0.55)
        case .diffHunk: controlFill
        }
    }
}

extension UIColor {
    convenience init(hex: UInt32, alpha: CGFloat = 1) {
        self.init(
            red: CGFloat((hex >> 16) & 0xFF) / 255,
            green: CGFloat((hex >> 8) & 0xFF) / 255,
            blue: CGFloat(hex & 0xFF) / 255,
            alpha: alpha
        )
    }
}

/// Dynamic Type for fixed-height UI: one factor per content size category
/// (rows stay fixed-height, so lists still skip self-sizing).
enum TypeScale {
    static var factor: CGFloat {
        min(1.6, max(0.85, UIFontMetrics(forTextStyle: .body).scaledValue(for: 17) / 17))
    }

    static func size(_ base: CGFloat) -> CGFloat { (base * factor).rounded(.toNearestOrAwayFromZero) }
}
