import SwiftUI

/// Centralized font accessors so we can swap families in one place.
enum OAFonts {
    /// Primary UI font family (defaults to Inter). Swap here to try `.system`.
    static var primary: (Font.TextStyle, CGFloat) -> Font = { style, size in
        InterFont.font(relativeTo: style, size: size)
    }

    /// Monospace/code font family (defaults to Berkeley Mono).
    static var code: (Font.TextStyle, CGFloat) -> Font = { style, size in
        BerkeleyFont.font(relativeTo: style, size: size)
    }

    static func ui(_ style: Font.TextStyle = .body, _ size: CGFloat = 16) -> Font { primary(style, size) }
    static func mono(_ style: Font.TextStyle = .body, _ size: CGFloat = 14) -> Font { code(style, size) }
}

