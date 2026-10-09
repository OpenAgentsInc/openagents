// Native conversation elements for Rust Native: transcript, message,
// Markdown, tool, working, and composer. Rust decides what each row says;
// this file only paints, scrolls, and handles gestures.
//
// The bottom-anchored transcript, the message and tool-row layouts, the
// selectable inline Markdown text, and the code-block chrome follow the
// design of the Swift iOS app in pingdotgg/t3code (MIT License), reimplemented
// here. Inside a transcript, Rust lays out and NativeTranscriptPainter.swift
// paints each row; these SwiftUI views draw the same elements elsewhere.
import SwiftUI
import UIKit

// MARK: - Decoded properties

/// The control that loads older rows. Its intent stays in the application.
struct NativeEarlier: Decodable, Equatable {
    let label: String
    let loading: Bool
}

struct NativeComposerProps: Equatable {
    let token: String
    let placeholder: String
    let maxBytes: Int
    let enabled: Bool
    let busy: Bool
    /// The view carries a stop intent, so the busy control can stop.
    let stoppable: Bool
    /// Other ways to send the text, offered on a long press of send. Each
    /// answers with its own token, whose meaning stays in Rust.
    var choices: [NativeComposerChoice] = []
    /// Text to put in the field when the token is new, such as a message to
    /// edit.
    var draft: String? = nil
    /// Put the text cursor in the field when the token is new.
    var focus: Bool = false
}

/// Another way to send a composer's text.
struct NativeComposerChoice: Decodable, Equatable, Hashable {
    let token: String
    let label: String
}

/// One inline run of parsed Markdown. A link destination is never opened.
struct NativeMarkdownSpan: Decodable, Hashable {
    let text: String
    let bold: Bool
    let italic: Bool
    let strike: Bool
    let code: Bool
    let link: String?

    private enum Keys: String, CodingKey { case text, bold, italic, strike, code, link }

    init(from decoder: Decoder) throws {
        let object = try decoder.container(keyedBy: Keys.self)
        text = try object.decode(String.self, forKey: .text)
        bold = try object.decodeIfPresent(Bool.self, forKey: .bold) ?? false
        italic = try object.decodeIfPresent(Bool.self, forKey: .italic) ?? false
        strike = try object.decodeIfPresent(Bool.self, forKey: .strike) ?? false
        code = try object.decodeIfPresent(Bool.self, forKey: .code) ?? false
        link = try object.decodeIfPresent(String.self, forKey: .link)
    }
}

struct NativeMarkdownItem: Decodable, Hashable {
    /// A task-list item's state; `nil` for an ordinary item.
    let checked: Bool?
    let blocks: [NativeMarkdownBlock]
}

/// A block the application parsed with `rust_native::markdown`.
enum NativeMarkdownBlock: Decodable, Hashable {
    case heading(Int, [NativeMarkdownSpan])
    case paragraph([NativeMarkdownSpan])
    case list(ordered: Bool, start: UInt64, items: [NativeMarkdownItem])
    case code(language: String?, text: String)
    case quote([NativeMarkdownBlock])
    /// Column alignments (`none`, `left`, `center`, `right`), header cells,
    /// and body rows.
    case table(align: [String], header: [[NativeMarkdownSpan]], rows: [[[NativeMarkdownSpan]]])
    case rule

    private enum Keys: String, CodingKey {
        case kind, level, spans, ordered, start, items, language, text, blocks, align, header, rows
    }

    init(from decoder: Decoder) throws {
        let object = try decoder.container(keyedBy: Keys.self)
        switch try object.decode(String.self, forKey: .kind) {
        case "heading":
            self = .heading(try object.decode(Int.self, forKey: .level),
                            try object.decode([NativeMarkdownSpan].self, forKey: .spans))
        case "paragraph":
            self = .paragraph(try object.decode([NativeMarkdownSpan].self, forKey: .spans))
        case "list":
            self = .list(ordered: try object.decode(Bool.self, forKey: .ordered),
                         start: try object.decode(UInt64.self, forKey: .start),
                         items: try object.decode([NativeMarkdownItem].self, forKey: .items))
        case "code":
            self = .code(language: try object.decodeIfPresent(String.self, forKey: .language),
                         text: try object.decode(String.self, forKey: .text))
        case "quote":
            self = .quote(try object.decode([NativeMarkdownBlock].self, forKey: .blocks))
        case "table":
            self = .table(align: try object.decode([String].self, forKey: .align),
                          header: try object.decode([[NativeMarkdownSpan]].self, forKey: .header),
                          rows: try object.decode([[[NativeMarkdownSpan]]].self, forKey: .rows))
        case "rule":
            self = .rule
        default:
            throw DecodingError.dataCorruptedError(forKey: .kind, in: object,
                                                    debugDescription: "Unsupported Markdown block")
        }
    }
}

// MARK: - Plain text

/// Plain text for copying, matching `rust_native::markdown::plain`.
enum NativePlainText {
    static func of(_ node: NativeNode) -> String {
        switch node.element {
        case let .text(value, _): return value
        case let .markdown(blocks): return of(blocks)
        case let .button(label, _, _): return label
        case let .working(label): return label
        case let .tool(name, detail, _, children):
            return ([detail.isEmpty ? name : "\(name) \(detail)"] + children.map(of)).joined(separator: "\n")
        case let .stack(_, children), let .list(_, children), let .message(_, _, children),
             let .transcript(_, children, _, _):
            return children.map(of).filter { !$0.isEmpty }.joined(separator: "\n\n")
        case .surface, .composer: return ""
        }
    }

    static func of(_ blocks: [NativeMarkdownBlock]) -> String {
        var lines: [String] = []
        for block in blocks {
            switch block {
            case let .heading(_, spans), let .paragraph(spans):
                lines.append(text(spans))
            case let .list(ordered, start, items):
                for (index, item) in items.enumerated() {
                    let marker = switch (item.checked, ordered) {
                    case (true?, _): "[x] "
                    case (false?, _): "[ ] "
                    case (nil, true): "\(start + UInt64(index)). "
                    case (nil, false): "- "
                    }
                    lines.append(marker + of(item.blocks).replacingOccurrences(of: "\n", with: " "))
                }
            case let .code(_, code):
                lines.append(String(code.reversed().drop(while: \.isWhitespace).reversed()))
            case let .quote(inner):
                lines.append(contentsOf: of(inner).split(separator: "\n", omittingEmptySubsequences: false)
                    .map { "> \($0)" })
            case let .table(_, header, rows):
                lines.append(header.map(text).joined(separator: " | "))
                lines.append(contentsOf: rows.map { $0.map(text).joined(separator: " | ") })
            case .rule:
                lines.append("---")
            }
        }
        return lines.joined(separator: "\n")
    }

    private static func text(_ spans: [NativeMarkdownSpan]) -> String {
        spans.map(\.text).joined()
    }
}

// MARK: - Shared appearance

/// The default text tone a container gives the nodes inside it.
enum NativeTone: Hashable {
    case primary, secondary

    var color: Color { self == .primary ? .primary : .secondary }
    var uiColor: UIColor { self == .primary ? .label : .secondaryLabel }
}

private struct NativeToneKey: EnvironmentKey {
    static let defaultValue = NativeTone.primary
}

/// The plain text of the message around a node, for its Copy Message action.
private struct NativeCopyTextKey: EnvironmentKey {
    static let defaultValue: String? = nil
}

extension EnvironmentValues {
    var nativeTone: NativeTone {
        get { self[NativeToneKey.self] }
        set { self[NativeToneKey.self] = newValue }
    }

    var nativeCopyText: String? {
        get { self[NativeCopyTextKey.self] }
        set { self[NativeCopyTextKey.self] = newValue }
    }
}

/// Neutral surfaces that read on both a black and a white background.
enum NativeChatPalette {
    private static func dynamic(dark: CGFloat, light: CGFloat, alpha: CGFloat = 1) -> UIColor {
        UIColor { $0.userInterfaceStyle == .dark
            ? UIColor(white: dark, alpha: alpha) : UIColor(white: light, alpha: alpha) }
    }

    static let bubble = dynamic(dark: 0.16, light: 0.92)
    static let surface = dynamic(dark: 0.08, light: 0.96)
    static let raised = dynamic(dark: 0.13, light: 0.93)
    static let border = dynamic(dark: 0.22, light: 0.84)
    static let inlineCode = dynamic(dark: 1, light: 0, alpha: 0.1)
    static let link = UIColor.systemBlue

    /// The widest a transcript row or composer grows, centered beyond it.
    static let readingWidth: CGFloat = 720
}

/// A text color: the node's explicit color, or its container's tone.
struct NativeInk: Hashable {
    let rgba: [UInt8]?
    let tone: NativeTone

    init(_ color: NativeColor?, tone: NativeTone) {
        rgba = color.map { [$0.red, $0.green, $0.blue, $0.alpha] }
        self.tone = tone
    }

    private init(rgba: [UInt8]?, tone: NativeTone) {
        self.rgba = rgba
        self.tone = tone
    }

    var uiColor: UIColor {
        guard let rgba else { return tone.uiColor }
        return UIColor(red: CGFloat(rgba[0]) / 255, green: CGFloat(rgba[1]) / 255,
                       blue: CGFloat(rgba[2]) / 255, alpha: CGFloat(rgba[3]) / 255)
    }

    var color: Color { Color(uiColor: uiColor) }

    /// The quieter color for quoted text.
    var quieter: NativeInk {
        guard let rgba else { return NativeInk(rgba: nil, tone: .secondary) }
        return NativeInk(rgba: [rgba[0], rgba[1], rgba[2], UInt8(Double(rgba[3]) * 0.7)], tone: tone)
    }
}

/// Which tool rows the reader expanded, by node key. It is adapter state and
/// survives revisions and cell reuse.
@MainActor
final class NativeExpansion: ObservableObject {
    static let shared = NativeExpansion()
    @Published private(set) var keys: Set<String> = []

    func toggle(_ key: String) {
        if keys.remove(key) == nil { keys.insert(key) }
    }
}

// MARK: - Dispatch

@MainActor
enum NativeChat {
    typealias Surface = ((String, String) -> AnyView)?
    typealias Submit = ((String, String) -> Void)?

    static func render(_ node: NativeNode, revision: UInt64, surface: Surface, submit: Submit,
                       activate: @escaping (String) -> Void) -> AnyView {
        switch node.element {
        case let .transcript(label, rows, earlier, source):
            return AnyView(NativeTranscript(key: node.key, label: label, rows: rows, earlier: earlier,
                                            source: source, revision: revision, surface: surface,
                                            submit: submit, activate: activate))
        case let .message(role, note, children):
            return AnyView(NativeMessage(key: node.key, role: role, note: note, children: children,
                                         revision: revision, surface: surface, submit: submit,
                                         activate: activate))
        case let .markdown(blocks):
            return AnyView(NativeMarkdown(key: node.key, blocks: blocks, foreground: node.style.foreground))
        case let .tool(name, detail, state, children):
            return AnyView(NativeTool(key: node.key, name: name, detail: detail, state: state,
                                      children: children, revision: revision, surface: surface,
                                      submit: submit, activate: activate))
        case let .working(label):
            return AnyView(NativeWorking(key: node.key, label: label))
        case let .composer(props):
            return AnyView(NativeComposer(key: node.key, props: props, submit: submit, activate: activate))
        case .stack, .list, .text, .button, .surface:
            return AnyView(EmptyView())
        }
    }

    static func children(_ nodes: [NativeNode], revision: UInt64, surface: Surface, submit: Submit,
                         activate: @escaping (String) -> Void) -> some View {
        ForEach(nodes) { child in
            NativeRenderer(node: child, revision: revision, followTarget: nil, followChanged: nil,
                           surface: surface, submit: submit, activate: activate)
        }
    }
}

// MARK: - Transcript

// `NativeTranscript` lives in NativeTranscriptPainter.swift: Rust lays out
// the rows and the painter draws them.

// MARK: - Message

private struct NativeMessage: View {
    let key: String
    let role: String
    let note: String?
    let children: [NativeNode]
    let revision: UInt64
    let surface: NativeChat.Surface
    let submit: NativeChat.Submit
    let activate: (String) -> Void

    private var plainText: String {
        children.map(NativePlainText.of).filter { !$0.isEmpty }.joined(separator: "\n\n")
    }

    var body: some View {
        let copy = plainText
        content
            .environment(\.nativeCopyText, copy)
            .contextMenu {
                Button("Copy", systemImage: "doc.on.doc") { UIPasteboard.general.string = copy }
            }
            .accessibilityElement(children: .contain)
            .accessibilityIdentifier(key)
    }

    private var rendered: some View {
        NativeChat.children(children, revision: revision, surface: surface, submit: submit,
                            activate: activate)
    }

    @ViewBuilder private var content: some View {
        switch role {
        case "user":
            VStack(alignment: .trailing, spacing: 4) {
                HStack(spacing: 0) {
                    Spacer(minLength: 48)
                    VStack(alignment: .leading, spacing: 8) { rendered }
                        .padding(.horizontal, 14)
                        .padding(.vertical, 10)
                        .background(Color(uiColor: NativeChatPalette.bubble),
                                    in: UnevenRoundedRectangle(topLeadingRadius: 18, bottomLeadingRadius: 18,
                                                               bottomTrailingRadius: 4, topTrailingRadius: 18,
                                                               style: .continuous))
                }
                noteView
            }
            .frame(maxWidth: .infinity, alignment: .trailing)
        case "system":
            HStack(spacing: 6) {
                VStack(alignment: .center, spacing: 4) { rendered }
                if let note { Text("·"); Text(note) }
            }
            .font(.paper(.footnote))
            .environment(\.nativeTone, .secondary)
            .foregroundStyle(.secondary)
            .frame(maxWidth: .infinity, alignment: .center)
        default:
            VStack(alignment: .leading, spacing: 10) {
                rendered
                noteView
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    @ViewBuilder private var noteView: some View {
        if let note {
            Text(note).font(.paper(.caption)).foregroundStyle(.secondary)
        }
    }
}

// MARK: - Markdown

private struct NativeMarkdown: View {
    let key: String
    let blocks: [NativeMarkdownBlock]
    let foreground: NativeColor?
    @Environment(\.nativeTone) private var tone

    var body: some View {
        NativeMarkdownBlocks(blocks: blocks, ink: NativeInk(foreground, tone: tone), spacing: 12)
            .accessibilityIdentifier(key)
    }
}

private struct NativeMarkdownBlocks: View {
    let blocks: [NativeMarkdownBlock]
    let ink: NativeInk
    let spacing: CGFloat

    var body: some View {
        VStack(alignment: .leading, spacing: spacing) {
            ForEach(blocks.indices, id: \.self) { index in
                NativeMarkdownBlockView(block: blocks[index], ink: ink).equatable()
            }
        }
    }
}

private struct NativeMarkdownBlockView: View, Equatable {
    let block: NativeMarkdownBlock
    let ink: NativeInk

    var body: some View {
        switch block {
        case let .heading(level, spans):
            NativeInlineText(spans: spans, style: .heading(level), ink: ink)
                .padding(.top, level <= 2 ? 4 : 2)
                .accessibilityAddTraits(.isHeader)
        case let .paragraph(spans):
            NativeInlineText(spans: spans, style: .body, ink: ink)
        case let .list(ordered, start, items):
            NativeMarkdownList(ordered: ordered, start: start, items: items, ink: ink)
        case let .code(language, text):
            NativeCodeBlock(language: language, text: text, ink: ink)
        case let .quote(blocks):
            NativeMarkdownBlocks(blocks: blocks, ink: ink.quieter, spacing: 8)
                .padding(.leading, 12)
                .overlay(alignment: .leading) {
                    Capsule().fill(Color(uiColor: NativeChatPalette.border)).frame(width: 3)
                }
        case let .table(align, header, rows):
            NativeMarkdownTable(align: align, header: header, rows: rows, ink: ink)
        case .rule:
            Rectangle().fill(Color(uiColor: NativeChatPalette.border)).frame(height: 1)
                .padding(.vertical, 4)
                .accessibilityHidden(true)
        }
    }
}

private struct NativeMarkdownList: View {
    let ordered: Bool
    let start: UInt64
    let items: [NativeMarkdownItem]
    let ink: NativeInk

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            ForEach(items.indices, id: \.self) { index in
                HStack(alignment: .top, spacing: 8) {
                    marker(items[index], number: start + UInt64(index))
                        .frame(minWidth: 18, alignment: .trailing)
                    NativeMarkdownBlocks(blocks: items[index].blocks, ink: ink, spacing: 6)
                }
                .accessibilityElement(children: .combine)
            }
        }
    }

    @ViewBuilder private func marker(_ item: NativeMarkdownItem, number: UInt64) -> some View {
        if let checked = item.checked {
            Image(systemName: checked ? "checkmark.square.fill" : "square")
                .font(.paper(15))
                .foregroundStyle(checked ? Color.green : ink.color.opacity(0.7))
                .frame(height: 20)
                .accessibilityLabel(checked ? "Completed" : "Not completed")
        } else if ordered {
            Text("\(number).").font(.paper(16))
                .foregroundStyle(ink.color.opacity(0.7))
        } else {
            Text("•").font(.paper(16, weight: .bold))
                .foregroundStyle(ink.color.opacity(0.7))
                .accessibilityHidden(true)
        }
    }
}

private struct NativeCodeBlock: View {
    let language: String?
    let text: String
    let ink: NativeInk
    @State private var copied = false

    private var code: String {
        text.hasSuffix("\n") ? String(text.dropLast()) : text
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 8) {
                Text(language.flatMap { $0.isEmpty ? nil : $0 } ?? "code")
                    .font(.paper(.caption, weight: .semibold))
                    .foregroundStyle(.secondary)
                Spacer(minLength: 8)
                Button {
                    UIPasteboard.general.string = code
                    copied = true
                } label: {
                    Label(copied ? "Copied" : "Copy", systemImage: copied ? "checkmark" : "doc.on.doc")
                        .font(.paper(.caption))
                        .frame(minHeight: 32)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .foregroundStyle(.secondary)
                .accessibilityHint("Copies this code block")
            }
            .padding(.horizontal, 12)
            Rectangle().fill(Color(uiColor: NativeChatPalette.border)).frame(height: 1)
            ScrollView(.horizontal) {
                Text(verbatim: code)
                    .font(.paper(13))
                    .foregroundStyle(ink.color)
                    .lineSpacing(3)
                    .fixedSize(horizontal: true, vertical: true)
                    .textSelection(.enabled)
                    .padding(12)
            }
            .scrollIndicators(.hidden)
        }
        .background(Color(uiColor: NativeChatPalette.surface))
        .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
        .overlay {
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .stroke(Color(uiColor: NativeChatPalette.border), lineWidth: 1)
        }
        .task(id: copied) {
            guard copied else { return }
            try? await Task.sleep(for: .seconds(1.5))
            copied = false
        }
    }
}

private struct NativeMarkdownTable: View {
    let align: [String]
    let header: [[NativeMarkdownSpan]]
    let rows: [[[NativeMarkdownSpan]]]
    let ink: NativeInk

    var body: some View {
        ScrollView(.horizontal) {
            Grid(alignment: .leading, horizontalSpacing: 0, verticalSpacing: 0) {
                row(header, header: true)
                ForEach(rows.indices, id: \.self) { index in
                    Rectangle().fill(Color(uiColor: NativeChatPalette.border)).frame(height: 1)
                        .gridCellUnsizedAxes(.horizontal)
                    row(rows[index], header: false)
                }
            }
            .fixedSize(horizontal: true, vertical: true)
            .background(Color(uiColor: NativeChatPalette.surface))
            .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
            .overlay {
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .stroke(Color(uiColor: NativeChatPalette.border), lineWidth: 1)
            }
        }
        .scrollIndicators(.hidden)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Table with \(header.count) columns and \(rows.count) rows")
    }

    private func row(_ cells: [[NativeMarkdownSpan]], header: Bool) -> some View {
        GridRow {
            ForEach(0..<self.header.count, id: \.self) { column in
                NativeInlineText(spans: column < cells.count ? cells[column] : [],
                                 style: header ? .headerCell : .cell, ink: ink)
                    .padding(.horizontal, 10)
                    .padding(.vertical, 7)
                    .gridColumnAlignment(alignment(column))
            }
        }
        .background(header ? Color(uiColor: NativeChatPalette.raised) : Color.clear)
    }

    private func alignment(_ column: Int) -> HorizontalAlignment {
        switch column < align.count ? align[column] : "none" {
        case "center": return .center
        case "right": return .trailing
        default: return .leading
        }
    }
}

/// The type role of one run of inline Markdown text.
enum NativeInlineStyle: Hashable {
    case body, heading(Int), cell, headerCell
}

/// Attributed strings for inline Markdown, cached by content so a revision
/// that repeats a paragraph does not rebuild it.
@MainActor
final class NativeMarkdownCache {
    static let shared = NativeMarkdownCache()

    private struct Key: Hashable {
        let spans: [NativeMarkdownSpan]
        let style: NativeInlineStyle
        let ink: NativeInk
        let category: UIContentSizeCategory
    }

    private var strings: [Key: NSAttributedString] = [:]

    func text(_ spans: [NativeMarkdownSpan], style: NativeInlineStyle, ink: NativeInk,
              category: UIContentSizeCategory) -> NSAttributedString {
        let key = Key(spans: spans, style: style, ink: ink, category: category)
        if let cached = strings[key] { return cached }
        if strings.count >= 2_048 { strings.removeAll(keepingCapacity: true) }
        let built = Self.build(spans, style: style, ink: ink, category: category)
        strings[key] = built
        return built
    }

    private static func build(_ spans: [NativeMarkdownSpan], style: NativeInlineStyle, ink: NativeInk,
                              category: UIContentSizeCategory) -> NSAttributedString {
        let traits = UITraitCollection(preferredContentSizeCategory: category)
        let paragraph = NSMutableParagraphStyle()
        paragraph.lineSpacing = style == .body ? 3 : 2
        let result = NSMutableAttributedString()
        for span in spans {
            var attributes: [NSAttributedString.Key: Any] = [
                .font: font(style, span: span, traits: traits),
                .foregroundColor: ink.uiColor,
                .paragraphStyle: paragraph,
            ]
            if span.code { attributes[.backgroundColor] = NativeChatPalette.inlineCode }
            if span.strike { attributes[.strikethroughStyle] = NSUnderlineStyle.single.rawValue }
            // A link is styled but carries no `.link` attribute, so it stays
            // inert.
            if span.link != nil {
                attributes[.foregroundColor] = NativeChatPalette.link
                attributes[.underlineStyle] = NSUnderlineStyle.single.rawValue
            }
            result.append(NSAttributedString(string: span.text, attributes: attributes))
        }
        return result
    }

    private static func font(_ style: NativeInlineStyle, span: NativeMarkdownSpan,
                             traits: UITraitCollection) -> UIFont {
        let (size, weight, textStyle): (CGFloat, UIFont.Weight, UIFont.TextStyle) = switch style {
        case .body: (16, .regular, .body)
        case .heading(1): (22, .bold, .title2)
        case .heading(2): (19, .bold, .title3)
        case .heading(3): (17, .semibold, .headline)
        case .heading: (16, .semibold, .headline)
        case .cell: (15, .regular, .subheadline)
        case .headerCell: (15, .semibold, .subheadline)
        }
        // Paper Mono has no italic face, so italic spans draw upright.
        let font = span.code
            ? UIFont.code(size * 0.9, weight: span.bold ? .semibold : weight)
            : UIFont.paper(size, weight: span.bold ? .bold : weight)
        return UIFontMetrics(forTextStyle: textStyle).scaledFont(for: font, compatibleWith: traits)
    }
}

/// Selectable inline Markdown text backed by `UITextView`.
private struct NativeInlineText: UIViewRepresentable {
    let spans: [NativeMarkdownSpan]
    let style: NativeInlineStyle
    let ink: NativeInk
    @Environment(\.nativeCopyText) private var copyText
    @Environment(\.sizeCategory) private var sizeCategory

    /// The widest an unconstrained run grows before it wraps, such as a
    /// table cell.
    private static let naturalWidth: CGFloat = 280

    func makeCoordinator() -> Coordinator { Coordinator() }

    func makeUIView(context: Context) -> UITextView {
        let view = UITextView()
        view.backgroundColor = .clear
        view.isEditable = false
        view.isSelectable = true
        view.isScrollEnabled = false
        view.dataDetectorTypes = []
        view.textContainerInset = .zero
        view.textContainer.lineFragmentPadding = 0
        view.textContainer.widthTracksTextView = true
        view.adjustsFontForContentSizeCategory = false
        view.delegate = context.coordinator
        view.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        view.setContentHuggingPriority(.defaultHigh, for: .horizontal)
        return view
    }

    func updateUIView(_ view: UITextView, context: Context) {
        context.coordinator.copyText = copyText
        context.coordinator.apply(text, to: view)
    }

    func sizeThatFits(_ proposal: ProposedViewSize, uiView: UITextView, context: Context) -> CGSize? {
        context.coordinator.apply(text, to: uiView)
        let width = proposal.width.flatMap { $0.isFinite && $0 > 0 ? $0 : nil } ?? Self.naturalWidth
        return context.coordinator.size(of: uiView, width: width)
    }

    private var text: NSAttributedString {
        NativeMarkdownCache.shared.text(spans, style: style, ink: ink,
                                        category: UIContentSizeCategory(sizeCategory))
    }

    final class Coordinator: NSObject, UITextViewDelegate {
        var copyText: String?
        private var applied: NSAttributedString?
        private var sizes: [CGFloat: CGSize] = [:]

        func apply(_ text: NSAttributedString, to view: UITextView) {
            guard applied !== text else { return }
            applied = text
            sizes.removeAll()
            view.attributedText = text
        }

        func size(of view: UITextView, width: CGFloat) -> CGSize {
            if let cached = sizes[width] { return cached }
            guard let text = applied, text.length > 0 else { return CGSize(width: 0, height: 0) }
            let natural = text.boundingRect(with: CGSize(width: width, height: .greatestFiniteMagnitude),
                                            options: [.usesLineFragmentOrigin, .usesFontLeading], context: nil)
            // A little slack keeps the text view from wrapping a line that the
            // measurement fitted exactly.
            let fitted = min(width, ceil(natural.width) + 2)
            let height = view.sizeThatFits(CGSize(width: fitted, height: .greatestFiniteMagnitude)).height
            let size = CGSize(width: fitted, height: ceil(height))
            sizes[width] = size
            return size
        }

        func textView(_ textView: UITextView, editMenuForTextIn range: NSRange,
                      suggestedActions: [UIMenuElement]) -> UIMenu? {
            guard let copyText, !copyText.isEmpty else { return nil }
            let copyMessage = UIAction(title: "Copy Message", image: UIImage(systemName: "doc.on.doc")) { _ in
                UIPasteboard.general.string = copyText
            }
            return UIMenu(children: suggestedActions + [copyMessage])
        }

        func textView(_ textView: UITextView, primaryActionFor textItem: UITextItem,
                      defaultAction: UIAction) -> UIAction? {
            // Nothing in a document opens a destination.
            nil
        }
    }
}

// MARK: - Tool

private struct NativeTool: View {
    let key: String
    let name: String
    let detail: String
    let state: String
    let children: [NativeNode]
    let revision: UInt64
    let surface: NativeChat.Surface
    let submit: NativeChat.Submit
    let activate: (String) -> Void
    @ObservedObject private var expansion = NativeExpansion.shared

    private var expanded: Bool { expansion.keys.contains(key) }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Button {
                var transaction = Transaction(animation: nil)
                transaction.disablesAnimations = true
                withTransaction(transaction) { expansion.toggle(key) }
            } label: {
                HStack(spacing: 8) {
                    icon.frame(width: 18, height: 18)
                    Text(name).font(.paper(.body, weight: .semibold))
                    Text(detail).foregroundStyle(.secondary).lineLimit(1).truncationMode(.tail)
                    Spacer(minLength: 4)
                    if !children.isEmpty {
                        Image(systemName: expanded ? "chevron.down" : "chevron.right")
                            .font(.paper(.caption, weight: .semibold))
                            .foregroundStyle(.tertiary)
                    }
                }
                .font(.paper(.subheadline))
                .frame(minHeight: 36)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .disabled(children.isEmpty)
            .accessibilityLabel("\(name), \(stateLabel)")
            .accessibilityValue(detail)
            .accessibilityHint(children.isEmpty ? "" : expanded ? "Collapses the output" : "Expands the output")
            .accessibilityIdentifier(key)
            if expanded {
                VStack(alignment: .leading, spacing: 6) {
                    NativeChat.children(children, revision: revision, surface: surface, submit: submit,
                                        activate: activate)
                }
                .font(.paper(.footnote))
                // Tool output is secondary; draw a code child's body font a
                // step smaller than the conversation's.
                .dynamicTypeSize(.xSmall)
                .padding(10)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color(uiColor: NativeChatPalette.surface),
                            in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                .padding(.top, 4)
            }
        }
    }

    private var stateLabel: String {
        switch state {
        case "running": return "running"
        case "failed": return "failed"
        default: return "done"
        }
    }

    @ViewBuilder private var icon: some View {
        switch state {
        case "running":
            ProgressView().controlSize(.mini)
        case "failed":
            Image(systemName: "xmark.circle.fill").foregroundStyle(.red)
        default:
            Image(systemName: "checkmark.circle.fill").foregroundStyle(.green)
        }
    }
}

// MARK: - Working

private struct NativeWorking: View {
    let key: String
    let label: String
    @State private var pulsing = false

    var body: some View {
        HStack(spacing: 10) {
            HStack(spacing: 4) {
                ForEach(0..<3, id: \.self) { index in
                    Circle()
                        .frame(width: 6, height: 6)
                        .opacity(pulsing ? 1 : 0.25)
                        .animation(.easeInOut(duration: 0.6).repeatForever().delay(Double(index) * 0.2),
                                   value: pulsing)
                }
            }
            Text(label).font(.paper(.subheadline))
        }
        .foregroundStyle(.secondary)
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.vertical, 4)
        .onAppear { pulsing = true }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(label)
        .accessibilityIdentifier(key)
    }
}

// MARK: - Composer

private struct NativeComposer: View {
    let key: String
    let props: NativeComposerProps
    let submit: NativeChat.Submit
    let activate: (String) -> Void
    @State private var text = ""

    private var canSend: Bool {
        props.enabled && !props.busy && submit != nil
            && !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            && text.utf8.count <= props.maxBytes
    }

    // One capsule, as in Zeron's resting composer: the text between the
    // capsule's ends and a round send control inside its trailing end.
    var body: some View {
        HStack(alignment: .bottom, spacing: 8) {
            NativeComposerField(text: $text, token: props.token, placeholder: props.placeholder,
                                maxBytes: props.maxBytes, enabled: props.enabled, draft: props.draft,
                                focusToken: props.focus ? props.token : nil, send: send)
                .frame(maxWidth: .infinity, alignment: .leading)
                .overlay(alignment: .topLeading) {
                    if text.isEmpty {
                        Text(props.placeholder)
                            .font(.paper(16))
                            .foregroundStyle(.tertiary)
                            .lineLimit(1)
                            .allowsHitTesting(false)
                            .accessibilityHidden(true)
                    }
                }
                .padding(.leading, 18)
                .padding(.vertical, 14)
            control
                .padding(.trailing, 8)
                .padding(.bottom, 8)
        }
        .frame(minHeight: 50)
        .modifier(NativeComposerSurface())
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .frame(maxWidth: NativeChatPalette.readingWidth + 24)
        .frame(maxWidth: .infinity)
        .opacity(props.enabled ? 1 : 0.6)
        .accessibilityIdentifier(key)
    }

    /// The control stops only when the view carries a stop intent. A busy
    /// composer without one (a computer that cannot stop its reply) shows
    /// the send arrow, disabled, never a stop icon that does nothing.
    private var stops: Bool { props.busy && props.stoppable }

    private var controlEnabled: Bool { stops || canSend }

    private var controlImage: some View {
        Image(systemName: stops ? "stop.fill" : "arrow.up")
            .font(.paper(stops ? 11 : 15, weight: .bold))
            .foregroundStyle(controlEnabled ? Color(uiColor: .systemBackground)
                                            : Color(uiColor: .tertiaryLabel))
            .frame(width: 34, height: 34)
            .background(Circle().fill(controlEnabled ? Color(uiColor: .label)
                                                     : Color(uiColor: .label).opacity(0.075)))
    }

    /// The round send control. With choices, a tap sends and a long press
    /// opens a menu of the other ways to send.
    @ViewBuilder private var control: some View {
        if !props.busy && !props.choices.isEmpty {
            Menu {
                ForEach(props.choices, id: \.token) { choice in
                    Button(choice.label) { send(as: choice.token) }
                        .accessibilityIdentifier("\(key)-choice-\(choice.label)")
                }
            } label: {
                controlImage
            } primaryAction: {
                send()
            }
            .menuStyle(.button)
            .buttonStyle(.plain)
            .disabled(!controlEnabled)
            .accessibilityLabel("Send")
            .accessibilityHint("Touch and hold for other ways to send.")
            .accessibilityIdentifier("\(key)-send")
        } else {
            Button(action: stops ? stop : send) { controlImage }
                .buttonStyle(.plain)
                .disabled(!controlEnabled)
                .accessibilityLabel(stops ? "Stop" : "Send")
                .accessibilityIdentifier("\(key)-\(stops ? "stop" : "send")")
        }
    }

    private func send() { send(as: props.token) }

    /// The draft stays until Rust accepts it: the next composer's new token
    /// clears the shared draft, and a refused send keeps the words.
    private func send(as token: String) {
        guard canSend, let submit else { return }
        submit(token, text)
    }

    private func stop() {
        guard props.busy, props.stoppable else { return }
        activate(key)
    }
}

/// The composer's capsule: Liquid Glass where the system has it, else a
/// raised fill with a hairline edge. It rounds to a card as the text grows.
private struct NativeComposerSurface: ViewModifier {
    func body(content: Content) -> some View {
        let shape = RoundedRectangle(cornerRadius: 25, style: .continuous)
        if #available(iOS 26.0, *) {
            content.glassEffect(.regular, in: shape)
        } else {
            content
                .background(Color(uiColor: NativeChatPalette.raised), in: shape)
                .overlay(shape.strokeBorder(Color(uiColor: NativeChatPalette.border), lineWidth: 0.5))
        }
    }
}

/// A composer draft edited through Rust Native's shared editor
/// (`rust_native::edit::mirror`): the text view reports each change, and
/// the draft Rust returns is what it shows. Deletion never splits a
/// grapheme, an IME composition is one undo step, undo and redo are the
/// shared history's, and a stamp from an older draft is refused.
final class NativeComposerEditor {
    struct Stamp: Codable, Equatable {
        let token: String
        let lifetime: UInt64
        let revision: UInt64
    }

    struct State: Decodable, Equatable {
        let stamp: Stamp
        let text: String
        /// Anchor and caret, in UTF-16 code units.
        let selection: [Int]
        let marked: [Int]?
        let can_undo: Bool
        let can_redo: Bool

        var range: NSRange {
            let low = min(selection.first ?? 0, selection.last ?? 0)
            let high = max(selection.first ?? 0, selection.last ?? 0)
            return NSRange(location: low, length: high - low)
        }
    }

    private struct Reply: Decodable {
        let state: State?
        let replaced: Bool?
        let error: String?
    }

    private let handle = rust_native_editor_create()
    private(set) var state: State?

    deinit { if let handle { rust_native_editor_destroy(handle) } }

    /// Milliseconds on a monotonic clock, for undo coalescing.
    static var now: UInt64 { UInt64(ProcessInfo.processInfo.systemUptime * 1000) }

    /// Mounts the view's composer. Returns whether the draft was replaced:
    /// a new token starts a new draft (its `draft`, or empty).
    func mount(token: String, maxBytes: Int, draft: String?) -> Bool {
        var request: [String: Any] = ["op": "mount", "token": token, "max_bytes": maxBytes]
        if let draft { request["draft"] = draft }
        guard let reply = call(request) else { return false }
        if let state = reply.state { self.state = state }
        return reply.replaced == true
    }

    /// Applies one change to the current draft; the reply's state is the
    /// draft to show, also after a refusal.
    @discardableResult
    func apply(_ change: [String: Any]) -> State? {
        guard let stamp = state?.stamp else { return nil }
        let request: [String: Any] = [
            "op": "apply",
            "stamp": ["token": stamp.token, "lifetime": stamp.lifetime, "revision": stamp.revision],
            "change": change,
        ]
        if let reply = call(request), let state = reply.state { self.state = state }
        return state
    }

    private func call(_ request: [String: Any]) -> Reply? {
        guard let handle, let body = try? JSONSerialization.data(withJSONObject: request) else { return nil }
        let data = body.withUnsafeBytes { bytes -> Data? in
            let buffer = rust_native_editor_call(handle, bytes.bindMemory(to: UInt8.self).baseAddress, bytes.count)
            defer { rust_native_layout_buffer_free(buffer) }
            guard let pointer = buffer.data, buffer.len > 0 else { return nil }
            return Data(bytes: pointer, count: buffer.len)
        }
        return data.flatMap { try? JSONDecoder().decode(Reply.self, from: $0) }
    }
}

/// Undo and redo from the shared draft's history: the edit menu, the
/// three-finger gestures, shake, and Command-Z all reach it. The text view
/// registers nothing of its own.
final class NativeComposerUndo: UndoManager {
    var editor: NativeComposerEditor?
    var changed: ((NativeComposerEditor.State) -> Void)?

    override init() {
        super.init()
        disableUndoRegistration()
    }

    override var canUndo: Bool { editor?.state?.can_undo ?? false }
    override var canRedo: Bool { editor?.state?.can_redo ?? false }
    override var undoActionName: String { "" }
    override var redoActionName: String { "" }

    override func undo() {
        if let state = editor?.apply(["op": "undo"]) { changed?(state) }
    }

    override func redo() {
        if let state = editor?.apply(["op": "redo"]) { changed?(state) }
    }
}

/// A text view that grows from one to six lines and sends on Command-Return.
private struct NativeComposerField: UIViewRepresentable {
    @Binding var text: String
    let token: String
    let placeholder: String
    let maxBytes: Int
    let enabled: Bool
    /// The draft a new token puts in the field, such as a message to edit.
    let draft: String?
    /// The composer token to focus the field for, once, when it first shows.
    let focusToken: String?
    let send: () -> Void

    private static let font = UIFontMetrics(forTextStyle: .body).scaledFont(for: .paper(16))
    private static let maxLines: CGFloat = 6

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    func makeUIView(context: Context) -> NativeComposerTextView {
        let view = NativeComposerTextView()
        view.font = Self.font
        view.adjustsFontForContentSizeCategory = true
        view.backgroundColor = .clear
        view.textColor = .label
        view.textContainerInset = .zero
        view.textContainer.lineFragmentPadding = 0
        view.isScrollEnabled = false
        view.delegate = context.coordinator
        view.accessibilityLabel = placeholder
        view.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        let coordinator = context.coordinator
        view.undo.editor = coordinator.editor
        view.undo.changed = { [weak coordinator, weak view] state in
            guard let coordinator, let view else { return }
            coordinator.show(state, in: view)
        }
        view.onDeleteBackward = { [weak coordinator, weak view] in
            guard let coordinator, let view else { return false }
            return coordinator.deleteBackward(in: view)
        }
        return view
    }

    func updateUIView(_ view: NativeComposerTextView, context: Context) {
        let coordinator = context.coordinator
        coordinator.parent = self
        view.onCommandReturn = { [weak coordinator] in coordinator?.parent.send() }
        if coordinator.editor.mount(token: token, maxBytes: maxBytes, draft: draft),
           let state = coordinator.editor.state {
            coordinator.show(state, in: view)
        } else if view.markedTextRange == nil, let state = coordinator.editor.state, view.text != state.text {
            coordinator.show(state, in: view)
        }
        view.isEditable = enabled
        view.accessibilityLabel = placeholder
        if enabled, let token = focusToken, coordinator.focused != token {
            coordinator.focused = token
            DispatchQueue.main.async { view.becomeFirstResponder() }
        }
    }

    func sizeThatFits(_ proposal: ProposedViewSize, uiView: NativeComposerTextView,
                      context: Context) -> CGSize? {
        let width = proposal.width.flatMap { $0.isFinite && $0 > 0 ? $0 : nil } ?? 240
        let line = uiView.font?.lineHeight ?? Self.font.lineHeight
        let fitted = uiView.sizeThatFits(CGSize(width: width, height: .greatestFiniteMagnitude)).height
        let limit = ceil(line * Self.maxLines)
        let scrolls = fitted > limit + 0.5
        if uiView.isScrollEnabled != scrolls {
            DispatchQueue.main.async { uiView.isScrollEnabled = scrolls }
        }
        return CGSize(width: width, height: min(max(ceil(fitted), ceil(line)), limit))
    }

    final class Coordinator: NSObject, UITextViewDelegate {
        var parent: NativeComposerField
        /// The composer token the field last took focus for.
        var focused: String?
        let editor = NativeComposerEditor()
        /// The field is showing Rust's draft: its own callbacks are echoes.
        private var showing = false

        init(_ parent: NativeComposerField) { self.parent = parent }

        /// Show the shared draft: its text and selection.
        func show(_ state: NativeComposerEditor.State, in view: UITextView) {
            showing = true
            defer { showing = false }
            if view.text != state.text { view.text = state.text }
            let length = (state.text as NSString).length
            let range = state.range
            if NSMaxRange(range) <= length, view.selectedRange != range { view.selectedRange = range }
            if parent.text != state.text {
                let text = state.text
                DispatchQueue.main.async { self.parent.text = text }
            }
        }

        /// The delete key outside a composition: one whole grapheme, or the
        /// selection, through the shared draft.
        func deleteBackward(in view: UITextView) -> Bool {
            guard view.markedTextRange == nil, parent.enabled, editor.state != nil else { return false }
            let before = editor.state
            guard let state = editor.apply(["op": "delete", "backwards": true, "at_ms": NativeComposerEditor.now])
            else { return false }
            if state != before { show(state, in: view) }
            return true
        }

        func textViewDidChange(_ textView: UITextView) {
            guard !showing, editor.state != nil else { return }
            let selected = textView.selectedRange
            var change: [String: Any] = [
                "op": "sync",
                "text": textView.text ?? "",
                "selection": [selected.location, selected.location + selected.length],
                "at_ms": NativeComposerEditor.now,
            ]
            if let marked = textView.markedTextRange {
                let start = textView.offset(from: textView.beginningOfDocument, to: marked.start)
                let end = textView.offset(from: textView.beginningOfDocument, to: marked.end)
                change["marked"] = [start, end]
            }
            guard let state = editor.apply(change) else { return }
            if textView.markedTextRange == nil, textView.text != state.text {
                // Rust refused the change or kept a grapheme whole: show its draft.
                show(state, in: textView)
            } else if parent.text != state.text {
                parent.text = state.text
            }
        }

        func textViewDidChangeSelection(_ textView: UITextView) {
            guard !showing, textView.markedTextRange == nil, let state = editor.state,
                  textView.text == state.text else { return }
            let selected = textView.selectedRange
            guard selected != state.range else { return }
            editor.apply(["op": "select", "selection": [selected.location, selected.location + selected.length]])
        }

        /// A phone has no Command-Z: the field's edit menu offers the shared
        /// draft's undo and redo while they would change it.
        func textView(_ textView: UITextView, editMenuForTextIn range: NSRange,
                      suggestedActions: [UIMenuElement]) -> UIMenu? {
            guard let view = textView as? NativeComposerTextView, let state = editor.state else { return nil }
            var history: [UIMenuElement] = []
            if state.can_undo {
                history.append(UIAction(title: "Undo", image: UIImage(systemName: "arrow.uturn.backward")) {
                    [weak view] _ in view?.undo.undo()
                })
            }
            if state.can_redo {
                history.append(UIAction(title: "Redo", image: UIImage(systemName: "arrow.uturn.forward")) {
                    [weak view] _ in view?.undo.redo()
                })
            }
            guard !history.isEmpty else { return nil }
            return UIMenu(children: [UIMenu(options: .displayInline, children: history)] + suggestedActions)
        }

        func textView(_ textView: UITextView, shouldChangeTextIn range: NSRange,
                      replacementText replacement: String) -> Bool {
            let current = textView.text as NSString? ?? ""
            let next = current.replacingCharacters(in: range, with: replacement)
            // Refuse an edit past the byte bound, but always allow deleting.
            return next.utf8.count <= parent.maxBytes || replacement.isEmpty
        }
    }
}

final class NativeComposerTextView: UITextView {
    var onCommandReturn: (() -> Void)?
    /// Handles the delete key; false lets the text view delete, as while an
    /// IME composes.
    var onDeleteBackward: (() -> Bool)?
    let undo = NativeComposerUndo()

    override var undoManager: UndoManager? { undo }

    override var keyCommands: [UIKeyCommand]? {
        let send = UIKeyCommand(title: "Send", action: #selector(commandReturn), input: "\r",
                                modifierFlags: .command)
        send.wantsPriorityOverSystemBehavior = true
        let undo = UIKeyCommand(title: "Undo", action: #selector(undoDraft), input: "z", modifierFlags: .command)
        undo.wantsPriorityOverSystemBehavior = true
        let redo = UIKeyCommand(title: "Redo", action: #selector(redoDraft), input: "z",
                                modifierFlags: [.command, .shift])
        redo.wantsPriorityOverSystemBehavior = true
        return (super.keyCommands ?? []) + [send, undo, redo]
    }

    override func deleteBackward() {
        if onDeleteBackward?() == true { return }
        super.deleteBackward()
    }

    /// The composer takes text only: Paste is offered only for text, so an
    /// image alone on the pasteboard never reaches a draft (#10093).
    override func canPerformAction(_ action: Selector, withSender sender: Any?) -> Bool {
        if action == #selector(paste(_:)) && !UIPasteboard.general.hasStrings { return false }
        return super.canPerformAction(action, withSender: sender)
    }

    @objc private func commandReturn() { onCommandReturn?() }
    @objc private func undoDraft() { undo.undo() }
    @objc private func redoDraft() { undo.redo() }
}
