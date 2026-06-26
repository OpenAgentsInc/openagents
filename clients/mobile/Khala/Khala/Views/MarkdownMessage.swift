import SwiftUI

/// Renders assistant markdown content as a stack of native SwiftUI views:
/// headings, paragraphs (with inline **bold**/*italic*/`code`), bullet and
/// numbered lists, and dedicated `CodeBlockView`s for fenced code. Used for the
/// full-width assistant turns; user turns render plain (see `MessageBubble`).
struct MarkdownMessage: View {
    let content: String

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            ForEach(MarkdownParser.parseIdentified(content)) { identified in
                view(for: identified.block)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    @ViewBuilder
    private func view(for block: MarkdownBlock) -> some View {
        switch block {
        case let .heading(level, text):
            Text(MarkdownParser.inlineAttributed(text))
                .font(headingFont(level))
                .foregroundStyle(.primary)
                .fixedSize(horizontal: false, vertical: true)
                .textSelection(.enabled)

        case let .paragraph(text):
            Text(MarkdownParser.inlineAttributed(text))
                .font(.body)
                .foregroundStyle(.primary)
                .fixedSize(horizontal: false, vertical: true)
                .textSelection(.enabled)

        case let .bullet(items):
            VStack(alignment: .leading, spacing: 6) {
                ForEach(Array(items.enumerated()), id: \.offset) { _, item in
                    listRow(marker: bulletMarker(for: item.indent), item: item)
                }
            }

        case let .numbered(items):
            VStack(alignment: .leading, spacing: 6) {
                ForEach(Array(numbered(items).enumerated()), id: \.offset) { _, pair in
                    listRow(marker: "\(pair.ordinal).", item: pair.item)
                }
            }

        case let .code(language, code):
            CodeBlockView(language: language, code: code)
        }
    }

    /// Restart numbering at each nesting level so a nested numbered list reads
    /// 1, 2, 3 under its parent rather than continuing the parent's count.
    private func numbered(_ items: [MarkdownListItem]) -> [(ordinal: Int, item: MarkdownListItem)] {
        var counters: [Int: Int] = [:]
        var lastIndent = items.first?.indent ?? 0
        return items.map { item in
            // Reset deeper counters when we pop back out to a shallower level.
            if item.indent < lastIndent {
                for level in counters.keys where level > item.indent { counters[level] = 0 }
            }
            lastIndent = item.indent
            let next = (counters[item.indent] ?? 0) + 1
            counters[item.indent] = next
            return (next, item)
        }
    }

    /// Vary the bullet glyph by depth so nested bullets are visually distinct.
    private func bulletMarker(for indent: Int) -> String {
        switch indent {
        case 0: return "•"
        case 1: return "◦"
        default: return "▪"
        }
    }

    private func listRow(marker: String, item: MarkdownListItem) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Text(marker)
                .font(.body.monospacedDigit())
                .foregroundStyle(.secondary)
                .frame(minWidth: 18, alignment: .trailing)
            Text(MarkdownParser.inlineAttributed(item.text))
                .font(.body)
                .foregroundStyle(.primary)
                .fixedSize(horizontal: false, vertical: true)
                .textSelection(.enabled)
        }
        // Indent nested items so the hierarchy is legible.
        .padding(.leading, CGFloat(item.indent) * 18)
    }

    private func headingFont(_ level: Int) -> Font {
        switch level {
        case 1: return .title2.weight(.bold)
        case 2: return .title3.weight(.bold)
        case 3: return .headline
        default: return .subheadline.weight(.semibold)
        }
    }
}
