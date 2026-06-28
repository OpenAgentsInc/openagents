import Foundation
import SwiftUI

/// A parsed block of assistant markdown. The Khala coding-dogfood payoff is
/// that fenced code blocks are first-class (`.code`), so the message renderer
/// can give them monospace + horizontal scroll + a copy button instead of
/// folding them into prose, which `AttributedString(markdown:)` does poorly.
///
/// Prose blocks (`.heading`, `.paragraph`, `.bullet`, `.numbered`) are rendered
/// with SwiftUI-native inline markdown (`AttributedString(markdown:)`), which
/// handles **bold**, *italics*, and `inline code` well enough for chat.
/// One list item, carrying its nesting depth so the renderer can indent nested
/// bullets/numbers the way the Khala coding answers expect them.
struct MarkdownListItem: Equatable {
    /// Nesting depth: 0 = top level, 1 = one level of indent, etc.
    let indent: Int
    let text: String
}

enum MarkdownBlock {
    case heading(level: Int, text: String)
    case paragraph(text: String)
    case bullet(items: [MarkdownListItem])
    case numbered(items: [MarkdownListItem])
    case code(language: String?, code: String)
}

/// A block paired with its parse-order position. The position guarantees a
/// stable, UNIQUE SwiftUI identity even when two blocks have identical content
/// (e.g. two identical fenced code blocks or repeated paragraphs). The previous
/// content-hash id collided for identical blocks, which dropped a duplicate
/// block from the rendered message and emitted a SwiftUI "duplicate ID" warning.
struct IdentifiedMarkdownBlock: Identifiable {
    let id: Int
    let block: MarkdownBlock
}

/// Splits raw assistant markdown into renderable blocks. Intentionally small
/// and deterministic: fenced code blocks are extracted exactly (including the
/// optional language hint), and the remaining prose is grouped into headings,
/// bullet/numbered lists, and paragraphs.
enum MarkdownParser {
    /// Parse into position-identified blocks for stable, unique SwiftUI identity.
    static func parseIdentified(_ raw: String) -> [IdentifiedMarkdownBlock] {
        parse(raw).enumerated().map { IdentifiedMarkdownBlock(id: $0.offset, block: $0.element) }
    }

    static func parse(_ raw: String) -> [MarkdownBlock] {
        var blocks: [MarkdownBlock] = []
        // Normalize newlines so the line scanner is stable.
        let normalized = raw.replacingOccurrences(of: "\r\n", with: "\n")
        let lines = normalized.components(separatedBy: "\n")

        var i = 0
        var paragraphBuffer: [String] = []

        func flushParagraph() {
            let joined = paragraphBuffer.joined(separator: "\n")
                .trimmingCharacters(in: .whitespacesAndNewlines)
            if !joined.isEmpty {
                blocks.append(.paragraph(text: joined))
            }
            paragraphBuffer.removeAll()
        }

        while i < lines.count {
            let line = lines[i]
            let trimmed = line.trimmingCharacters(in: .whitespaces)

            // Fenced code block: ``` optionally followed by a language hint.
            if trimmed.hasPrefix("```") {
                flushParagraph()
                let lang = String(trimmed.dropFirst(3))
                    .trimmingCharacters(in: .whitespaces)
                var codeLines: [String] = []
                i += 1
                while i < lines.count {
                    let inner = lines[i]
                    if inner.trimmingCharacters(in: .whitespaces).hasPrefix("```") {
                        break
                    }
                    codeLines.append(inner)
                    i += 1
                }
                // Skip the closing fence (if present).
                if i < lines.count { i += 1 }
                let code = codeLines.joined(separator: "\n")
                blocks.append(.code(language: lang.isEmpty ? nil : lang, code: code))
                continue
            }

            // Heading: one to six leading `#`.
            if let heading = parseHeading(trimmed) {
                flushParagraph()
                blocks.append(.heading(level: heading.level, text: heading.text))
                i += 1
                continue
            }

            // Bullet list: a run of `-`, `*`, or `+` items, possibly nested.
            if isBullet(trimmed) {
                flushParagraph()
                var items: [MarkdownListItem] = []
                while i < lines.count {
                    let raw = lines[i]
                    let t = raw.trimmingCharacters(in: .whitespaces)
                    guard isBullet(t) else { break }
                    items.append(MarkdownListItem(
                        indent: indentLevel(of: raw),
                        text: stripBulletMarker(t)
                    ))
                    i += 1
                }
                blocks.append(.bullet(items: items))
                continue
            }

            // Numbered list: a run of `1.`, `2)` style items, possibly nested.
            if isNumbered(trimmed) {
                flushParagraph()
                var items: [MarkdownListItem] = []
                while i < lines.count {
                    let raw = lines[i]
                    let t = raw.trimmingCharacters(in: .whitespaces)
                    guard isNumbered(t) else { break }
                    items.append(MarkdownListItem(
                        indent: indentLevel(of: raw),
                        text: stripNumberMarker(t)
                    ))
                    i += 1
                }
                blocks.append(.numbered(items: items))
                continue
            }

            // Blank line separates paragraphs.
            if trimmed.isEmpty {
                flushParagraph()
                i += 1
                continue
            }

            paragraphBuffer.append(line)
            i += 1
        }
        flushParagraph()
        return blocks
    }

    // MARK: - Inline markdown

    /// Render inline markdown (**bold**, *italics*, `code`, links) for a prose
    /// span using SwiftUI-native parsing. Falls back to plain text if the
    /// markdown is malformed so a partial stream never throws.
    static func inlineAttributed(_ text: String) -> AttributedString {
        let options = AttributedString.MarkdownParsingOptions(
            allowsExtendedAttributes: true,
            interpretedSyntax: .inlineOnlyPreservingWhitespace
        )
        if let attributed = try? AttributedString(markdown: text, options: options) {
            return attributed
        }
        return AttributedString(text)
    }

    // MARK: - Line classification helpers

    private static func parseHeading(_ trimmed: String) -> (level: Int, text: String)? {
        guard trimmed.hasPrefix("#") else { return nil }
        var level = 0
        var idx = trimmed.startIndex
        while idx < trimmed.endIndex, trimmed[idx] == "#", level < 6 {
            level += 1
            idx = trimmed.index(after: idx)
        }
        // A real ATX heading requires a space after the hashes.
        guard idx < trimmed.endIndex, trimmed[idx] == " " else { return nil }
        let text = String(trimmed[idx...]).trimmingCharacters(in: .whitespaces)
        guard !text.isEmpty else { return nil }
        return (level, text)
    }

    /// Leading-whitespace nesting depth of a raw list line. Two spaces (or one
    /// tab) per level, matching how assistant markdown indents nested lists.
    /// Capped so a pathologically deep indent can't push content off-screen.
    private static func indentLevel(of rawLine: String) -> Int {
        var spaces = 0
        for ch in rawLine {
            if ch == " " { spaces += 1 }
            else if ch == "\t" { spaces += 2 }
            else { break }
        }
        return min(spaces / 2, 4)
    }

    private static func isBullet(_ trimmed: String) -> Bool {
        guard trimmed.count >= 2 else { return false }
        let first = trimmed.first!
        guard first == "-" || first == "*" || first == "+" else { return false }
        let second = trimmed[trimmed.index(after: trimmed.startIndex)]
        return second == " "
    }

    private static func stripBulletMarker(_ trimmed: String) -> String {
        String(trimmed.dropFirst(2)).trimmingCharacters(in: .whitespaces)
    }

    private static func isNumbered(_ trimmed: String) -> Bool {
        // Match a leading run of digits followed by `.` or `)` then a space.
        var idx = trimmed.startIndex
        var digits = 0
        while idx < trimmed.endIndex, trimmed[idx].isNumber {
            digits += 1
            idx = trimmed.index(after: idx)
        }
        guard digits > 0, idx < trimmed.endIndex else { return false }
        let marker = trimmed[idx]
        guard marker == "." || marker == ")" else { return false }
        let afterMarker = trimmed.index(after: idx)
        guard afterMarker < trimmed.endIndex, trimmed[afterMarker] == " " else { return false }
        return true
    }

    private static func stripNumberMarker(_ trimmed: String) -> String {
        guard let markerIdx = trimmed.firstIndex(where: { $0 == "." || $0 == ")" }) else {
            return trimmed
        }
        let after = trimmed.index(after: markerIdx)
        guard after <= trimmed.endIndex else { return trimmed }
        return String(trimmed[after...]).trimmingCharacters(in: .whitespaces)
    }
}
