// The app's type: the web's (#11120). Text draws in the system face (SF Pro,
// the first family of the web's --font-sans stack on Apple) and code in the
// system monospaced face (SF Mono, the first of --font-mono), at the web's
// type scale (crates/oa-tokens/src/typography.rs). The app bundles no font.
//
// The views shared with Coder (bins/coder-ios/host/App) call `.paper(...)`
// for text and `.code(...)` for code; Coder's own helper answers them
// with its bundled face, and this file answers them with the system faces,
// so the OpenAgents app compiles the shared views unchanged.
// crates/openagents-mobile/src/typeface_tests.rs holds this file to the
// token table.
import SwiftUI
import UIKit

enum Typeface {
    /// A text style's size at the default content size category: the web's
    /// --font-text-* and --font-heading-* sizes in points.
    static func size(_ style: Font.TextStyle) -> CGFloat {
        switch style {
        case .largeTitle: 32 // heading-xl
        case .title: 24 // heading-lg
        case .title2: 20 // heading-md
        case .title3: 18 // heading-sm
        case .headline: 16 // heading-xs
        case .body, .callout: 16 // text-md
        case .subheadline: 14 // text-sm
        case .footnote, .caption: 12 // text-xs
        case .caption2: 10 // text-2xs
        default: 16 // text-md
        }
    }

    static func size(_ style: UIFont.TextStyle) -> CGFloat {
        switch style {
        case .largeTitle: 32
        case .title1: 24
        case .title2: 20
        case .title3: 18
        case .headline: 16
        case .body, .callout: 16
        case .subheadline: 14
        case .footnote, .caption1: 12
        case .caption2: 10
        default: 16
        }
    }

    /// A style's own weight: semibold for a heading (--font-heading-*-weight),
    /// else regular.
    static func weight(_ style: Font.TextStyle) -> Font.Weight {
        switch style {
        case .largeTitle, .title, .title2, .title3, .headline: .semibold
        default: .regular
        }
    }

    static func weight(_ style: UIFont.TextStyle) -> UIFont.Weight {
        switch style {
        case .largeTitle, .title1, .title2, .title3, .headline: .semibold
        default: .regular
        }
    }
}

extension Font {
    /// The system face at a fixed point size that does not follow Dynamic Type.
    static func paper(_ size: CGFloat, weight: Font.Weight = .regular) -> Font {
        .system(size: size, weight: weight)
    }

    /// The system face at a text style's size on the web's scale, scaled with
    /// Dynamic Type. A `nil` weight uses the style's own.
    static func paper(_ style: Font.TextStyle, weight: Font.Weight? = nil) -> Font {
        Font(UIFont.paper(UIFont.TextStyle(style), weight: weight.map(UIFont.Weight.init)))
    }

    /// The system monospaced face, for code, keys, and identifiers.
    static func code(_ size: CGFloat, weight: Font.Weight = .regular) -> Font {
        .system(size: size, weight: weight, design: .monospaced)
    }

    static func code(_ style: Font.TextStyle, weight: Font.Weight? = nil) -> Font {
        Font(UIFont.code(UIFont.TextStyle(style), weight: weight.map(UIFont.Weight.init)))
    }
}

extension UIFont {
    /// The system face at a fixed point size.
    static func paper(_ size: CGFloat, weight: UIFont.Weight = .regular) -> UIFont {
        .systemFont(ofSize: size, weight: weight)
    }

    /// The system face at a text style's size on the web's scale, scaled with
    /// Dynamic Type.
    static func paper(_ style: UIFont.TextStyle, weight: UIFont.Weight? = nil,
                      compatibleWith traits: UITraitCollection? = nil) -> UIFont {
        UIFontMetrics(forTextStyle: style).scaledFont(
            for: paper(Typeface.size(style), weight: weight ?? Typeface.weight(style)),
            compatibleWith: traits)
    }

    /// The system monospaced face at a fixed point size.
    static func code(_ size: CGFloat, weight: UIFont.Weight = .regular) -> UIFont {
        .monospacedSystemFont(ofSize: size, weight: weight)
    }

    static func code(_ style: UIFont.TextStyle, weight: UIFont.Weight? = nil,
                     compatibleWith traits: UITraitCollection? = nil) -> UIFont {
        UIFontMetrics(forTextStyle: style).scaledFont(
            for: code(Typeface.size(style), weight: weight ?? .regular), compatibleWith: traits)
    }
}

extension UIFont.TextStyle {
    init(_ style: Font.TextStyle) {
        self = switch style {
        case .largeTitle: .largeTitle
        case .title: .title1
        case .title2: .title2
        case .title3: .title3
        case .headline: .headline
        case .subheadline: .subheadline
        case .callout: .callout
        case .footnote: .footnote
        case .caption: .caption1
        case .caption2: .caption2
        default: .body
        }
    }
}

extension UIFont.Weight {
    init(_ weight: Font.Weight) {
        self = switch weight {
        case .ultraLight: .ultraLight
        case .thin: .thin
        case .light: .light
        case .medium: .medium
        case .semibold: .semibold
        case .bold: .bold
        case .heavy: .heavy
        case .black: .black
        default: .regular
        }
    }
}
