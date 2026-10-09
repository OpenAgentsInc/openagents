// Native decoding and rendering of Rust Native's bounded view contract.
// Application intents stay opaque; native callbacks return identity only.
import SwiftUI

struct NativeColor: Codable, Equatable {
    let red: UInt8
    let green: UInt8
    let blue: UInt8
    let alpha: UInt8

    var color: Color {
        Color(.sRGB, red: Double(red) / 255, green: Double(green) / 255,
              blue: Double(blue) / 255, opacity: Double(alpha) / 255)
    }
}

struct NativeStyle: Codable, Equatable {
    let foreground: NativeColor?
    let background: NativeColor?
    let padding_top: String?
    let padding_end: String?
    let padding_bottom: String?
    let padding_start: String?
    let gap: String?
    let weight: String?
    let align: String?
    /// `context`: the stack's first child is a card and its other buttons
    /// are the card's menu (`rust_native::style::Menu`).
    var menu: String? = nil

    static func points(_ space: String?) -> CGFloat {
        switch space {
        case "none": return 0
        case "xs": return 4
        case "sm": return 8
        case "md": return 16
        case "lg": return 24
        default: return 0
        }
    }

    var insets: EdgeInsets {
        EdgeInsets(top: Self.points(padding_top), leading: Self.points(padding_start),
                   bottom: Self.points(padding_bottom), trailing: Self.points(padding_end))
    }
}

struct NativeNode: Decodable, Equatable, Identifiable {
    let key: String
    let style: NativeStyle
    let element: NativeElement
    var id: String { key }
}

indirect enum NativeElement: Decodable, Equatable {
    case stack(String, [NativeNode])
    case list(String, [NativeNode])
    case text(String, String)
    /// A label, an enabled state, and an optional glyph.
    case button(String, Bool, NativeIcon?)
    case surface(String, String)
    /// A conversation, oldest row first, its optional older-rows control,
    /// and, when Rust holds the rows, the transcript source that publishes
    /// them (the rows are then empty; `rust_native::layout::source`).
    case transcript(String, [NativeNode], NativeEarlier?, String?)
    /// A role (`user`, `assistant`, or `system`), a short note, and children.
    case message(String, String?, [NativeNode])
    case markdown([NativeMarkdownBlock])
    /// A tool call's name, one-line detail, state, and expandable children.
    case tool(String, String, String, [NativeNode])
    case working(String)
    case composer(NativeComposerProps)

    private enum Keys: String, CodingKey { case kind, props }
    private enum Props: String, CodingKey {
        case axis, children, label, value, role, enabled, resource, icon
        case earlier, note, blocks, name, detail, state, source
        case token, placeholder, max_bytes, busy, stop, choices, draft, focus
    }

    init(from decoder: Decoder) throws {
        let object = try decoder.container(keyedBy: Keys.self)
        let kind = try object.decode(String.self, forKey: .kind)
        let props = try object.nestedContainer(keyedBy: Props.self, forKey: .props)
        switch kind {
        case "stack": self = .stack(try props.decode(String.self, forKey: .axis),
                                     try props.decode([NativeNode].self, forKey: .children))
        case "list": self = .list(try props.decode(String.self, forKey: .label),
                                   try props.decode([NativeNode].self, forKey: .children))
        case "text": self = .text(try props.decode(String.self, forKey: .value),
                                   try props.decode(String.self, forKey: .role))
        case "button": self = .button(try props.decode(String.self, forKey: .label),
                                       try props.decode(Bool.self, forKey: .enabled),
                                       try props.decodeIfPresent(NativeIcon.self, forKey: .icon))
        case "surface": self = .surface(try props.decode(String.self, forKey: .resource),
                                         try props.decode(String.self, forKey: .label))
        case "transcript": self = .transcript(try props.decode(String.self, forKey: .label),
                                               try props.decode([NativeNode].self, forKey: .children),
                                               try props.decodeIfPresent(NativeEarlier.self, forKey: .earlier),
                                               try props.decodeIfPresent(String.self, forKey: .source))
        case "message": self = .message(try props.decode(String.self, forKey: .role),
                                         try props.decodeIfPresent(String.self, forKey: .note),
                                         try props.decode([NativeNode].self, forKey: .children))
        case "markdown": self = .markdown(try props.decode([NativeMarkdownBlock].self, forKey: .blocks))
        case "tool": self = .tool(try props.decode(String.self, forKey: .name),
                                   try props.decode(String.self, forKey: .detail),
                                   try props.decode(String.self, forKey: .state),
                                   try props.decode([NativeNode].self, forKey: .children))
        case "working": self = .working(try props.decode(String.self, forKey: .label))
        case "composer":
            // The stop intent stays opaque; only its presence matters here.
            let stop = props.contains(.stop) ? !(try props.decodeNil(forKey: .stop)) : false
            self = .composer(NativeComposerProps(
                token: try props.decode(String.self, forKey: .token),
                placeholder: try props.decode(String.self, forKey: .placeholder),
                maxBytes: try props.decode(Int.self, forKey: .max_bytes),
                enabled: try props.decode(Bool.self, forKey: .enabled),
                busy: try props.decode(Bool.self, forKey: .busy),
                stoppable: stop,
                choices: try props.decodeIfPresent([NativeComposerChoice].self, forKey: .choices) ?? [],
                draft: try props.decodeIfPresent(String.self, forKey: .draft),
                focus: try props.decodeIfPresent(Bool.self, forKey: .focus) ?? false))
        default:
            throw DecodingError.dataCorruptedError(forKey: .kind, in: object,
                                                    debugDescription: "Unsupported native element")
        }
    }
}

/// A button's glyph. An unknown glyph decodes to no symbol, and the button
/// shows its label.
struct NativeIcon: Decodable, Equatable {
    let glyph: String
    let circular: Bool
    /// The glyph and label in a filled capsule, as a suggestion chip.
    let pill: Bool?

    var symbol: String? {
        switch glyph {
        case "back": "chevron.backward"
        case "compose": "square.and.pencil"
        case "menu": "line.3.horizontal"
        case "history": "clock.arrow.circlepath"
        case "folder": "folder"
        case "computer": "desktopcomputer"
        case "cloud": "cloud"
        case "add": "plus"
        case "check": "checkmark"
        case "ask": "text.bubble"
        case "flag": "flag"
        case "terminal": "terminal"
        case "wallet": "creditcard"
        case "key": "key"
        case "person": "person"
        case "pin": "pin"
        case "archive": "archivebox"
        case "restore": "arrow.uturn.backward"
        case "paperclip": "paperclip"
        case "more": "ellipsis"
        default: nil
        }
    }
}

/// A glyph button: a circle with the glyph and a spoken label, or a glyph
/// before a visible label, as a back link.
private struct NativeIconButton: View {
    let key: String
    let label: String
    let enabled: Bool
    let icon: NativeIcon
    let activate: (String) -> Void

    var body: some View {
        Button { activate(key) } label: {
            if icon.circular {
                Image(systemName: icon.symbol ?? "circle")
                    .font(.paper(17, weight: .medium))
                    .frame(width: 44, height: 44)
                    .background(Circle().fill(Color(uiColor: NativeChatPalette.raised)))
                    .overlay(Circle().strokeBorder(Color(uiColor: NativeChatPalette.border), lineWidth: 0.5))
                    .contentShape(Circle())
            } else if icon.pill == true {
                // A chip: the glyph and one line of label in a capsule, as
                // tall as the round buttons' touch target allows.
                HStack(spacing: 6) {
                    Image(systemName: icon.symbol ?? "circle").font(.paper(13, weight: .semibold))
                    Text(label).font(.paper(.subheadline)).lineLimit(1).truncationMode(.middle)
                }
                .padding(.horizontal, 14)
                .frame(minHeight: 36)
                .background(Capsule().fill(Color(uiColor: NativeChatPalette.raised)))
                .overlay(Capsule().strokeBorder(Color(uiColor: NativeChatPalette.border), lineWidth: 0.5))
                .contentShape(Capsule())
            } else {
                // Top-aligned rows keep the label level with text beside it;
                // the padding widens the tap target without moving it.
                HStack(spacing: 4) {
                    Image(systemName: icon.symbol ?? "circle").font(.paper(17, weight: .semibold))
                    Text(label)
                }
                .padding(.vertical, 10)
                .contentShape(Rectangle())
                .padding(.vertical, -10)
            }
        }
        .buttonStyle(.plain)
        .disabled(!enabled)
        .opacity(enabled ? 1 : 0.4)
        .accessibilityLabel(label)
        .accessibilityIdentifier(key)
    }
}

struct NativeView: Decodable {
    let schema: String
    let instance: String
    let revision: UInt64
    let root: NativeNode
}

/// The cell font for Rust Native's terminal text role. The terminal screen
/// sizes its grid from these metrics.
enum TerminalMetrics {
    static let font = UIFont.paper(12)
    static var cell: CGSize {
        let width = ("M" as NSString).size(withAttributes: [.font: font]).width
        return CGSize(width: ceil(width * 100) / 100, height: ceil(font.lineHeight))
    }
}

struct NativeRenderer: View {
    let node: NativeNode
    let revision: UInt64
    let followTarget: String?
    let followChanged: ((Bool) -> Void)?
    var surface: ((String, String) -> AnyView)? = nil
    /// Receives a composer send as the input token and the text.
    var submit: ((String, String) -> Void)? = nil
    let activate: (String) -> Void
    /// Activates a node of the newest view, for a menu item chosen after
    /// the view that opened the menu was replaced. Menu item keys name
    /// their item and action, so a key that is gone refuses. Without it,
    /// items activate against the view that drew them.
    var activateCurrent: ((String) -> Void)? = nil
    @Environment(\.nativeFloatingComposer) private var floating

    var body: some View {
        content
            .padding(node.style.insets)
            .modifier(NativeForeground(explicit: node.style.foreground?.color))
            .background(node.style.background?.color ?? .clear)
            .fontWeight(node.style.weight == "bold" ? .bold :
                        node.style.weight == "normal" ? .regular : nil)
            .multilineTextAlignment(node.style.align == "center" ? .center :
                                   node.style.align == "end" ? .trailing : .leading)
    }

    private var content: AnyView {
        switch node.element {
        case let .stack(_, children) where node.style.menu == "context" && children.count > 1:
            // A card with a context menu: a long press offers the other
            // buttons as a native menu (UIMenu). Each item activates its
            // own node, so a menu from an older view is refused.
            return AnyView(render(children[0]).contextMenu {
                ForEach(children.dropFirst()) { item in
                    if case let .button(label, enabled, icon) = item.element {
                        Button {
                            (activateCurrent ?? activate)(item.key)
                        } label: {
                            if let symbol = icon?.symbol {
                                Label(label, systemImage: symbol)
                            } else {
                                Text(label)
                            }
                        }
                        .disabled(!enabled)
                        .accessibilityIdentifier(item.key)
                    }
                }
            })
        case let .stack(axis, children):
            if axis == "wrap" {
                // Left to right, continuing on the next line, as a row of
                // suggestion chips.
                let gap = NativeStyle.points(node.style.gap)
                return AnyView(NativeFlow(spacing: gap) {
                    ForEach(children) { child in render(child) }
                }.frame(maxWidth: .infinity, alignment: .leading))
            }
            if axis == "horizontal" {
                // A centered row, such as a header, shares one line: a title
                // beside a round button, or a back link beside its status.
                // Other rows keep their text level at the top.
                let alignment: VerticalAlignment = node.style.align == "center" ? .center : .top
                return AnyView(HStack(alignment: alignment, spacing: NativeStyle.points(node.style.gap)) {
                    ForEach(children) { child in render(child) }
                })
            }
            // A floating composer (opt-in): the transcript fills the space
            // and what follows it floats over its end.
            if let backdrop = floating,
               let at = children.firstIndex(where: { if case .transcript = $0.element { true } else { false } }),
               at < children.count - 1 {
                let gap = NativeStyle.points(node.style.gap)
                return AnyView(NativeFloatingStack(
                    spacing: gap, backdrop: backdrop,
                    above: Array(children[..<at]).map(render),
                    transcript: render(children[at]),
                    below: Array(children[(at + 1)...]).map(render)))
            }
            return AnyView(VStack(alignment: .leading, spacing: NativeStyle.points(node.style.gap)) {
                ForEach(children) { child in render(child) }
            }.frame(maxWidth: .infinity, alignment: .leading))
        case let .list(label, children):
            return AnyView(NativeList(key: node.key, rows: children, label: label, revision: revision,
                                     followTarget: followTarget, followChanged: followChanged,
                                     surface: surface, activate: activate, activateCurrent: activateCurrent))
        case let .button(label, enabled, icon?) where icon.symbol != nil:
            // An end-aligned glyph button takes the rest of its row and sits
            // at its end, as a toolbar button does.
            let button = NativeIconButton(key: node.key, label: label, enabled: enabled, icon: icon,
                                          activate: activate)
            if node.style.align == "end" {
                return AnyView(button.frame(maxWidth: .infinity, alignment: .trailing))
            }
            return AnyView(button)
        case let .button(label, enabled, _):
            return AnyView(Button(label) { activate(node.key) }.disabled(!enabled)
                .accessibilityIdentifier(node.key))
        case let .text(value, "terminal"):
            // One row or run of a fixed-cell grid: one line, never wrapped.
            return AnyView(Text(verbatim: value).font(Font(TerminalMetrics.font))
                .lineLimit(1).fixedSize(horizontal: true, vertical: true)
                .accessibilityIdentifier(node.key))
        case let .text(value, role):
            return AnyView(NativeText(key: node.key, value: value, role: role))
        case let .surface(resource, label):
            return surface?(resource, label) ?? AnyView(
                Text("This device cannot display \(label).")
                    .accessibilityIdentifier("\(node.key)-unsupported"))
        case .transcript, .message, .markdown, .tool, .working, .composer:
            return NativeChat.render(node, revision: revision, surface: surface,
                                     submit: submit, activate: activate)
        }
    }

    private func render(_ child: NativeNode) -> NativeRenderer {
        NativeRenderer(node: child, revision: revision, followTarget: followTarget,
                       followChanged: followChanged, surface: surface, submit: submit,
                       activate: activate, activateCurrent: activateCurrent)
    }
}

private struct NativeFloatingComposerKey: EnvironmentKey {
    static let defaultValue: Color? = nil
}

private struct NativeTranscriptBottomInsetKey: EnvironmentKey {
    static let defaultValue: CGFloat = 0
}

extension EnvironmentValues {
    /// The backdrop color of a composer floating over the transcript; nil
    /// (the default) stacks the composer below the transcript.
    var nativeFloatingComposer: Color? {
        get { self[NativeFloatingComposerKey.self] }
        set { self[NativeFloatingComposerKey.self] = newValue }
    }

    /// Room a transcript keeps clear below its last row.
    var nativeTranscriptBottomInset: CGFloat {
        get { self[NativeTranscriptBottomInsetKey.self] }
        set { self[NativeTranscriptBottomInsetKey.self] = newValue }
    }
}

/// The rows above a transcript, then the transcript filling the rest, with
/// the rows after it (chips, cards, the composer) floating over its end on
/// a soft fade. The transcript keeps their height clear below its last row,
/// so it scrolls underneath them and its end can still be read.
private struct NativeFloatingStack: View {
    let spacing: CGFloat
    let backdrop: Color
    let above: [NativeRenderer]
    let transcript: NativeRenderer
    let below: [NativeRenderer]
    @State private var height: CGFloat = 0

    var body: some View {
        VStack(alignment: .leading, spacing: spacing) {
            ForEach(above, id: \.node.key) { $0 }
            ZStack(alignment: .bottom) {
                transcript
                    .environment(\.nativeTranscriptBottomInset, height)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                VStack(alignment: .leading, spacing: spacing) {
                    ForEach(below, id: \.node.key) { $0 }
                }
                .padding(.top, 12)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(alignment: .top) {
                    LinearGradient(stops: [.init(color: backdrop.opacity(0), location: 0),
                                           .init(color: backdrop.opacity(0.92), location: 0.35),
                                           .init(color: backdrop, location: 1)],
                                   startPoint: .top, endPoint: .bottom)
                        .padding(.horizontal, -24)
                        .ignoresSafeArea(.container, edges: .bottom)
                        .allowsHitTesting(false)
                }
                .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { height = $0 }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// Applies a node's foreground color, or the tone its container sets.
private struct NativeForeground: ViewModifier {
    let explicit: Color?
    @Environment(\.nativeTone) private var tone

    func body(content: Content) -> some View {
        content.foregroundStyle(explicit ?? tone.color)
    }
}

private struct NativeText: View {
    let key: String
    let value: String
    let role: String
    @State private var original = false

    private var text: Text {
        if role == "markdown", !original,
           let parsed = try? AttributedString(markdown: value, options: .init(
               interpretedSyntax: .inlineOnlyPreservingWhitespace)) {
            return Text(parsed)
        }
        return Text(verbatim: value)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            text
                .font(role == "code" ? .paper(.body) :
                      role == "heading" ? .paper(.headline) : role == "status" ? .paper(.caption) : .paper(.body))
                .textSelection(.enabled)
                .fixedSize(horizontal: false, vertical: true)
                .accessibilityAddTraits(role == "heading" ? .isHeader : [])
                .accessibilityIdentifier(key)
        }
        .contextMenu {
            if role == "markdown" {
                Button(original ? "Formatted text" : "Original Markdown") { original.toggle() }
            }
        }
        // Viewing untrusted Markdown never opens an external destination.
        .environment(\.openURL, OpenURLAction { _ in .discarded })
    }
}

private struct NativeList: View {
    let key: String
    let rows: [NativeNode]
    let label: String
    let revision: UInt64
    let followTarget: String?
    let followChanged: ((Bool) -> Void)?
    let surface: ((String, String) -> AnyView)?
    let activate: (String) -> Void
    let activateCurrent: ((String) -> Void)?
    @State private var following = true

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            if followChanged != nil {
                Toggle("Follow new messages", isOn: Binding(get: { following }, set: changeFollowing))
                    .accessibilityHint("Turn off to keep your reading position during refreshes.")
                    .accessibilityIdentifier("\(key)-follow")
            }
            ScrollViewReader { proxy in
                List(rows) { row in
                    NativeRenderer(node: row, revision: revision, followTarget: nil,
                                   followChanged: nil, surface: surface, activate: activate,
                                   activateCurrent: activateCurrent)
                        .id(row.key)
                }
                .listStyle(.plain)
                .accessibilityLabel(label)
                .simultaneousGesture(DragGesture(minimumDistance: 8).onChanged { _ in changeFollowing(false) })
                .onChange(of: revision) { _, _ in follow(proxy) }
                .onChange(of: following) { _, _ in follow(proxy) }
                .onChange(of: followTarget) { _, current in following = current != nil }
                .onAppear { following = followTarget != nil; follow(proxy) }
            }
        }.frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func follow(_ proxy: ScrollViewProxy) {
        guard following, let target = followTarget, rows.contains(where: { $0.key == target }) else { return }
        proxy.scrollTo(target, anchor: .bottom)
    }

    private func changeFollowing(_ enabled: Bool) {
        guard enabled != following else { return }
        following = enabled
        followChanged?(enabled)
    }
}

/// Children left to right at their ideal sizes, continuing on the next line
/// when the next one does not fit; a child wider than the line is narrowed
/// to it.
struct NativeFlow: Layout {
    var spacing: CGFloat

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let width = proposal.width ?? .infinity
        let frames = place(subviews, width: width)
        let used = frames.map(\.maxX).max() ?? 0
        return CGSize(width: proposal.width ?? used, height: frames.map(\.maxY).max() ?? 0)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        for (subview, frame) in zip(subviews, place(subviews, width: bounds.width)) {
            subview.place(at: CGPoint(x: bounds.minX + frame.minX, y: bounds.minY + frame.minY),
                          proposal: ProposedViewSize(frame.size))
        }
    }

    private func place(_ subviews: Subviews, width: CGFloat) -> [CGRect] {
        var frames: [CGRect] = []
        var x: CGFloat = 0, y: CGFloat = 0, line: CGFloat = 0
        for subview in subviews {
            var size = subview.sizeThatFits(.unspecified)
            if size.width > width {
                size = subview.sizeThatFits(ProposedViewSize(width: width, height: nil))
                size.width = min(size.width, width)
            }
            if x > 0 && x + size.width > width {
                x = 0
                y += line + spacing
                line = 0
            }
            frames.append(CGRect(origin: CGPoint(x: x, y: y), size: size))
            x += size.width + spacing
            line = max(line, size.height)
        }
        return frames
    }
}
