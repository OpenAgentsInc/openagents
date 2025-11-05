import SwiftUI
import CoreText

enum BerkeleyFont {
    static let fileNames = [
        "BerkeleyMono-Regular",
        "BerkeleyMono-Bold",
        "BerkeleyMono-Italic",
        "BerkeleyMono-BoldItalic",
    ]

    // Candidate PostScript names to try when constructing Font
    static let candidates = [
        "BerkeleyMono-Regular",
        "BerkeleyMono",
        "Berkeley Mono",
    ]

    @discardableResult
    static func registerAll() -> Bool {
        var ok = true
        for name in fileNames {
            if let url = Bundle.main.url(forResource: name, withExtension: "ttf") {
                CTFontManagerRegisterFontsForURL(url as CFURL, .process, nil)
            } else {
                ok = false
            }
        }
        return ok
    }

    static func defaultName() -> String {
        // Return the first candidate that can create a UIFont
        for n in candidates {
            #if canImport(UIKit)
            if UIFont(name: n, size: 17) != nil { return n }
            #else
            return n
            #endif
        }
        // Fallback to system mono design name
        return "Menlo"
    }

    static func font(relativeTo style: Font.TextStyle = .body, size: CGFloat = 16) -> Font {
        Font.custom(defaultName(), size: size, relativeTo: style)
    }
}

// MARK: - Inter variable font loader
enum InterFont {
    // Variable font files (opsz,wght) for regular and italic
    static let fileNames = [
        "Inter-VariableFont_opsz,wght",
        "Inter-Italic-VariableFont_opsz,wght",
    ]

    // Preferred PostScript/family names to try
    static let candidates = [
        "Inter",
        "InterVariable",
    ]

    @discardableResult
    static func registerAll() -> Bool {
        var ok = true
        for name in fileNames {
            if let url = Bundle.main.url(forResource: name, withExtension: "ttf") {
                CTFontManagerRegisterFontsForURL(url as CFURL, .process, nil)
            } else {
                ok = false
            }
        }
        return ok
    }

    static func defaultName() -> String {
        for n in candidates {
            #if canImport(UIKit)
            if UIFont(name: n, size: 17) != nil { return n }
            #else
            return n
            #endif
        }
        // Fallback: prefer declared family name; SwiftUI will fall back to system if missing
        return candidates.first ?? "Inter"
    }

    static func font(relativeTo style: Font.TextStyle = .body, size: CGFloat = 16) -> Font {
        Font.custom(defaultName(), size: size, relativeTo: style)
    }
}
