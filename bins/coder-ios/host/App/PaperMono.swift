// Paper Mono, the one typeface on every Coder surface. The app bundles
// the four static faces from crates/paper-mono/fonts and lists them under
// UIAppFonts. Every SwiftUI and UIKit font in this host comes from here.
//
// Paper Mono has Regular, Medium, SemiBold, and Bold faces and no italic.
// A lighter weight maps to Regular, a heavier one to Bold, and italic text
// draws upright. Coder compiles this file; the OpenAgents app draws the
// system faces instead (bins/openagents-ios/host/App/Typeface.swift).
import SwiftUI
import UIKit

enum PaperMono {
    static let family = "Paper Mono"

    /// The PostScript name of the face that draws `weight`.
    static func faceName(_ weight: Font.Weight) -> String {
        switch weight {
        case .medium: "PaperMono-Medium"
        case .semibold: "PaperMono-SemiBold"
        case .bold, .heavy, .black: "PaperMono-Bold"
        default: "PaperMono-Regular"
        }
    }

    static func faceName(_ weight: UIFont.Weight) -> String {
        if weight >= .bold { return "PaperMono-Bold" }
        if weight >= .semibold { return "PaperMono-SemiBold" }
        if weight >= .medium { return "PaperMono-Medium" }
        return "PaperMono-Regular"
    }

    /// The point size of a text style at the default content size category.
    static func size(_ style: Font.TextStyle) -> CGFloat {
        switch style {
        case .largeTitle: 34
        case .title: 28
        case .title2: 22
        case .title3: 20
        case .headline, .body: 17
        case .callout: 16
        case .subheadline: 15
        case .footnote: 13
        case .caption: 12
        case .caption2: 11
        default: 17
        }
    }

    static func size(_ style: UIFont.TextStyle) -> CGFloat {
        switch style {
        case .largeTitle: 34
        case .title1: 28
        case .title2: 22
        case .title3: 20
        case .headline, .body: 17
        case .callout: 16
        case .subheadline: 15
        case .footnote: 13
        case .caption1: 12
        case .caption2: 11
        default: 17
        }
    }

    /// Sets Paper Mono on the UIKit bars SwiftUI draws: navigation titles,
    /// bar buttons, tab items, and segmented pickers.
    @MainActor static func installAppearance() {
        let title: [NSAttributedString.Key: Any] = [.font: UIFont.paper(.headline)]
        UINavigationBar.appearance().titleTextAttributes = title
        UINavigationBar.appearance().largeTitleTextAttributes = [.font: UIFont.paper(.largeTitle, weight: .bold)]
        UIBarButtonItem.appearance().setTitleTextAttributes([.font: UIFont.paper(.body)], for: .normal)
        UITabBarItem.appearance().setTitleTextAttributes([.font: UIFont.paper(10, weight: .medium)], for: .normal)
        UISegmentedControl.appearance().setTitleTextAttributes([.font: UIFont.paper(.footnote, weight: .medium)], for: .normal)
    }
}

extension Font {
    /// Paper Mono at a fixed point size that does not follow Dynamic Type.
    static func paper(_ size: CGFloat, weight: Font.Weight = .regular) -> Font {
        .custom(PaperMono.faceName(weight), fixedSize: size)
    }

    /// Paper Mono at a text style's default size, scaled with Dynamic Type.
    /// A `nil` weight uses the style's own: semibold for `.headline`, else
    /// regular.
    static func paper(_ style: Font.TextStyle, weight: Font.Weight? = nil) -> Font {
        let weight = weight ?? (style == .headline ? .semibold : .regular)
        return .custom(PaperMono.faceName(weight), size: PaperMono.size(style), relativeTo: style)
    }

    /// Code, keys, and identifiers. Paper Mono is already monospaced, so this
    /// is `paper`; the OpenAgents app, which draws text in the system face,
    /// answers it with the system monospaced face (its Typeface.swift).
    static func code(_ size: CGFloat, weight: Font.Weight = .regular) -> Font {
        paper(size, weight: weight)
    }

    static func code(_ style: Font.TextStyle, weight: Font.Weight? = nil) -> Font {
        paper(style, weight: weight)
    }
}

extension UIFont {
    /// Paper Mono at a fixed point size. Falls back to the system monospaced
    /// face only when the bundle is missing the font.
    static func paper(_ size: CGFloat, weight: UIFont.Weight = .regular) -> UIFont {
        UIFont(name: PaperMono.faceName(weight), size: size)
            ?? .monospacedSystemFont(ofSize: size, weight: weight)
    }

    /// Paper Mono at a text style's default size, scaled with Dynamic Type.
    static func paper(_ style: UIFont.TextStyle, weight: UIFont.Weight? = nil,
                      compatibleWith traits: UITraitCollection? = nil) -> UIFont {
        let weight = weight ?? (style == .headline ? .semibold : .regular)
        return UIFontMetrics(forTextStyle: style)
            .scaledFont(for: paper(PaperMono.size(style), weight: weight), compatibleWith: traits)
    }

    /// Code at a fixed point size: Paper Mono, as `paper`.
    static func code(_ size: CGFloat, weight: UIFont.Weight = .regular) -> UIFont {
        paper(size, weight: weight)
    }
}
