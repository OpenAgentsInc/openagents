// Native conversation elements for Rust Native: transcript, message,
// Markdown, tool, working, and composer. Rust decides what each row says;
// this file only paints, scrolls, and handles gestures.
//
// The bottom-anchored transcript, the message and tool-row layouts, the
// selectable inline Markdown text, and the code-block chrome follow the
// design of the Swift iOS app in pingdotgg/t3code (MIT License), reimplemented
// here. Following zeronsh/comet, the transcript is UIKit: a collection view
// that updates only the rows whose content changed.
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
        case let .button(label, _): return label
        case let .working(label): return label
        case let .tool(name, detail, _, children):
            return ([detail.isEmpty ? name : "\(name) \(detail)"] + children.map(of)).joined(separator: "\n")
        case let .stack(_, children), let .list(_, children), let .message(_, _, children),
             let .transcript(_, children, _):
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
        case let .transcript(label, rows, earlier):
            return AnyView(NativeTranscript(key: node.key, label: label, rows: rows, earlier: earlier,
                                            revision: revision, surface: surface, submit: submit,
                                            activate: activate))
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

/// A bottom-anchored conversation. Rows are diffed by node key, and only the
/// rows whose content changed are reconfigured.
private struct NativeTranscript: UIViewRepresentable {
    let key: String
    let label: String
    let rows: [NativeNode]
    let earlier: NativeEarlier?
    let revision: UInt64
    let surface: NativeChat.Surface
    let submit: NativeChat.Submit
    let activate: (String) -> Void

    func makeCoordinator() -> NativeTranscriptCoordinator { NativeTranscriptCoordinator() }

    func makeUIView(context: Context) -> NativeTranscriptView {
        let view = NativeTranscriptView(frame: .zero, collectionViewLayout: Self.layout())
        view.backgroundColor = .clear
        view.alwaysBounceVertical = true
        view.keyboardDismissMode = .interactive
        view.delaysContentTouches = false
        view.contentInsetAdjustmentBehavior = .never
        view.accessibilityIdentifier = key
        context.coordinator.connect(view)
        return view
    }

    func updateUIView(_ view: NativeTranscriptView, context: Context) {
        let coordinator = context.coordinator
        coordinator.transcriptKey = key
        coordinator.surface = surface
        coordinator.submit = submit
        coordinator.activate = activate
        view.accessibilityLabel = label
        coordinator.update(rows: rows, earlier: earlier, revision: revision, in: view)
    }

    private static func layout() -> UICollectionViewLayout {
        UICollectionViewCompositionalLayout { _, environment in
            let width = environment.container.effectiveContentSize.width
            let side = max(16, (width - NativeChatPalette.readingWidth) / 2)
            let size = NSCollectionLayoutSize(widthDimension: .fractionalWidth(1),
                                              heightDimension: .estimated(80))
            let group = NSCollectionLayoutGroup.vertical(layoutSize: size,
                                                         subitems: [NSCollectionLayoutItem(layoutSize: size)])
            let section = NSCollectionLayoutSection(group: group)
            section.interGroupSpacing = 18
            section.contentInsets = NSDirectionalEdgeInsets(top: 16, leading: side, bottom: 16, trailing: side)
            return section
        }
    }
}

@MainActor
final class NativeTranscriptCoordinator: NSObject, UICollectionViewDelegate {
    /// Node keys are ASCII identifiers, so this row ID cannot collide.
    private static let earlierID = "\u{1}earlier"

    var transcriptKey = ""
    var surface: NativeChat.Surface = nil
    var submit: NativeChat.Submit = nil
    var activate: ((String) -> Void)?
    private var dataSource: UICollectionViewDiffableDataSource<Int, String>?
    private var nodes: [String: NativeNode] = [:]
    private var order: [String] = []
    private var earlier: NativeEarlier?
    private var revision: UInt64 = 0

    func connect(_ view: NativeTranscriptView) {
        let registration = UICollectionView.CellRegistration<UICollectionViewCell, String> {
            [weak self] cell, _, id in
            cell.backgroundConfiguration = UIBackgroundConfiguration.clear()
            guard let self else { return }
            if id == Self.earlierID {
                let earlier = self.earlier ?? NativeEarlier(label: "", loading: true)
                cell.contentConfiguration = UIHostingConfiguration {
                    NativeEarlierRow(earlier: earlier) { [weak self] in self?.loadEarlier() }
                }.margins(.all, 0)
                return
            }
            guard let node = self.nodes[id] else {
                cell.contentConfiguration = nil
                return
            }
            // Callbacks forward to the coordinator so a reused row never
            // keeps an earlier view's handlers.
            let submit: NativeChat.Submit = self.submit == nil ? nil : { [weak self] token, text in
                self?.submit?(token, text)
            }
            cell.contentConfiguration = UIHostingConfiguration {
                NativeRenderer(node: node, revision: self.revision, followTarget: nil, followChanged: nil,
                               surface: self.surface, submit: submit,
                               activate: { [weak self] key in self?.activate?(key) })
                    .frame(maxWidth: .infinity, alignment: .leading)
            }.margins(.all, 0)
        }
        dataSource = UICollectionViewDiffableDataSource<Int, String>(collectionView: view) {
            view, indexPath, id in
            view.dequeueConfiguredReusableCell(using: registration, for: indexPath, item: id)
        }
        view.delegate = self
    }

    func update(rows: [NativeNode], earlier: NativeEarlier?, revision: UInt64, in view: NativeTranscriptView) {
        guard let dataSource else { return }
        let newOrder = rows.map(\.key)
        var newNodes = [String: NativeNode](minimumCapacity: rows.count)
        var changed: [String] = []
        for row in rows {
            if let old = nodes[row.key], old != row { changed.append(row.key) }
            newNodes[row.key] = row
        }
        let earlierChanged = earlier != self.earlier
        let orderChanged = newOrder != order || (earlier == nil) != (self.earlier == nil)
        let firstApply = dataSource.snapshot().numberOfSections == 0
        self.revision = revision
        nodes = newNodes
        guard firstApply || orderChanged || earlierChanged || !changed.isEmpty else { return }

        // While the reader is scrolled up, keep the first visible row still
        // when rows arrive above it.
        let anchor = !view.following && orderChanged ? visibleAnchor(in: view) : nil
        let current = Set(dataSource.snapshot().itemIdentifiers)
        order = newOrder
        self.earlier = earlier

        var snapshot = NSDiffableDataSourceSnapshot<Int, String>()
        snapshot.appendSections([0])
        if earlier != nil { snapshot.appendItems([Self.earlierID]) }
        snapshot.appendItems(newOrder)
        var reconfigure = changed.filter(current.contains)
        if earlierChanged, earlier != nil, current.contains(Self.earlierID) {
            reconfigure.append(Self.earlierID)
        }
        if !reconfigure.isEmpty { snapshot.reconfigureItems(reconfigure) }
        dataSource.apply(snapshot, animatingDifferences: false) { [weak self, weak view] in
            guard let self, let view else { return }
            if view.following {
                view.pinToBottom()
            } else if let anchor, !view.isInteracting {
                self.restore(anchor, in: view)
            }
        }
    }

    private func loadEarlier() {
        guard let earlier, !earlier.loading else { return }
        activate?(transcriptKey)
    }

    private struct Anchor {
        let id: String
        let offset: CGFloat
    }

    private func visibleAnchor(in view: UICollectionView) -> Anchor? {
        guard let dataSource else { return nil }
        for indexPath in view.indexPathsForVisibleItems.sorted() {
            guard let id = dataSource.itemIdentifier(for: indexPath), id != Self.earlierID,
                  let frame = view.layoutAttributesForItem(at: indexPath)?.frame else { continue }
            return Anchor(id: id, offset: frame.minY - view.contentOffset.y)
        }
        return nil
    }

    private func restore(_ anchor: Anchor, in view: UICollectionView) {
        view.layoutIfNeeded()
        guard let indexPath = dataSource?.indexPath(for: anchor.id),
              let frame = view.layoutAttributesForItem(at: indexPath)?.frame else { return }
        let top = -view.adjustedContentInset.top
        let bottom = max(top, view.contentSize.height - view.bounds.height + view.adjustedContentInset.bottom)
        view.setContentOffset(CGPoint(x: view.contentOffset.x,
                                      y: min(bottom, max(top, frame.minY - anchor.offset))),
                              animated: false)
    }

    func scrollViewWillBeginDragging(_ scrollView: UIScrollView) {
        (scrollView as? NativeTranscriptView)?.following = false
    }

    func scrollViewDidEndDragging(_ scrollView: UIScrollView, willDecelerate decelerate: Bool) {
        if !decelerate { (scrollView as? NativeTranscriptView)?.settle() }
    }

    func scrollViewDidEndDecelerating(_ scrollView: UIScrollView) {
        (scrollView as? NativeTranscriptView)?.settle()
    }

    func scrollViewShouldScrollToTop(_ scrollView: UIScrollView) -> Bool {
        (scrollView as? NativeTranscriptView)?.following = false
        return true
    }

    func scrollViewDidScrollToTop(_ scrollView: UIScrollView) {
        (scrollView as? NativeTranscriptView)?.settle()
    }

    func scrollViewDidScroll(_ scrollView: UIScrollView) {
        (scrollView as? NativeTranscriptView)?.updateBottomButton()
    }
}

/// A collection view that stays at the bottom while it follows, and offers a
/// jump back to the bottom once the reader scrolls away.
final class NativeTranscriptView: UICollectionView {
    /// Whether the newest row stays in view as rows arrive and grow.
    var following = true {
        didSet { if following != oldValue { updateBottomButton() } }
    }

    private let bottomButton = UIButton(type: .system)
    private var buttonShown = false

    override init(frame: CGRect, collectionViewLayout layout: UICollectionViewLayout) {
        super.init(frame: frame, collectionViewLayout: layout)
        var configuration = UIButton.Configuration.filled()
        configuration.image = UIImage(systemName: "arrow.down")
        configuration.preferredSymbolConfigurationForImage = .init(pointSize: 15, weight: .semibold)
        configuration.baseForegroundColor = .label
        configuration.baseBackgroundColor = NativeChatPalette.raised
        configuration.cornerStyle = .capsule
        bottomButton.configuration = configuration
        bottomButton.layer.shadowColor = UIColor.black.cgColor
        bottomButton.layer.shadowOpacity = 0.35
        bottomButton.layer.shadowRadius = 6
        bottomButton.layer.shadowOffset = CGSize(width: 0, height: 2)
        bottomButton.accessibilityLabel = "Scroll to bottom"
        bottomButton.accessibilityIdentifier = "transcript-scroll-to-bottom"
        bottomButton.alpha = 0
        bottomButton.isHidden = true
        bottomButton.addTarget(self, action: #selector(jumpToBottom), for: .touchUpInside)
        addSubview(bottomButton)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    var isInteracting: Bool { isTracking || isDragging || isDecelerating }

    private var bottomOffset: CGFloat {
        max(-adjustedContentInset.top, contentSize.height - bounds.height + adjustedContentInset.bottom)
    }

    private var distanceFromBottom: CGFloat { bottomOffset - contentOffset.y }

    override func layoutSubviews() {
        super.layoutSubviews()
        // Rows measure themselves after they appear, so the content keeps
        // growing; hold the bottom until the reader drags.
        if following, !isInteracting, bounds.height > 0, contentSize.height > 0,
           abs(contentOffset.y - bottomOffset) > 0.5 {
            contentOffset = CGPoint(x: contentOffset.x, y: bottomOffset)
        }
        updateBottomButton()
    }

    func pinToBottom() {
        layoutIfNeeded()
        guard !isInteracting else { return }
        contentOffset = CGPoint(x: contentOffset.x, y: bottomOffset)
    }

    /// Resumes following when a scroll comes to rest at the bottom.
    func settle() {
        following = distanceFromBottom < 24
    }

    func updateBottomButton() {
        let show = !following && bounds.height > 120 && distanceFromBottom > 80
        bottomButton.frame = CGRect(x: bounds.midX - 20,
                                    y: bounds.maxY - adjustedContentInset.bottom - 56,
                                    width: 40, height: 40)
        bringSubviewToFront(bottomButton)
        guard show != buttonShown else { return }
        buttonShown = show
        if show { bottomButton.isHidden = false }
        UIView.animate(withDuration: 0.15, animations: { self.bottomButton.alpha = show ? 1 : 0 },
                       completion: { _ in self.bottomButton.isHidden = !self.buttonShown })
    }

    @objc private func jumpToBottom() {
        // Jump without animating so long transcripts do not lay out every
        // row in between.
        following = true
        pinToBottom()
        updateBottomButton()
    }
}

private struct NativeEarlierRow: View {
    let earlier: NativeEarlier
    let load: () -> Void

    var body: some View {
        Button(action: load) {
            HStack(spacing: 8) {
                if earlier.loading { ProgressView().controlSize(.small) }
                Text(earlier.label).font(.subheadline)
            }
            .frame(maxWidth: .infinity, minHeight: 44)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .foregroundStyle(.secondary)
        .disabled(earlier.loading)
        .accessibilityIdentifier("transcript-earlier")
    }
}

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
            .font(.footnote)
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
            Text(note).font(.caption).foregroundStyle(.secondary)
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
                .font(.system(size: 15))
                .foregroundStyle(checked ? Color.green : ink.color.opacity(0.7))
                .frame(height: 20)
                .accessibilityLabel(checked ? "Completed" : "Not completed")
        } else if ordered {
            Text("\(number).").font(.system(size: 16).monospacedDigit())
                .foregroundStyle(ink.color.opacity(0.7))
        } else {
            Text("•").font(.system(size: 16, weight: .bold))
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
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.secondary)
                Spacer(minLength: 8)
                Button {
                    UIPasteboard.general.string = code
                    copied = true
                } label: {
                    Label(copied ? "Copied" : "Copy", systemImage: copied ? "checkmark" : "doc.on.doc")
                        .font(.caption)
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
                    .font(.system(size: 13, design: .monospaced))
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
        var font = span.code
            ? UIFont.monospacedSystemFont(ofSize: size * 0.9, weight: span.bold ? .semibold : weight)
            : UIFont.systemFont(ofSize: size, weight: span.bold ? .bold : weight)
        if span.italic, let italic = font.fontDescriptor.withSymbolicTraits(
            font.fontDescriptor.symbolicTraits.union(.traitItalic)) {
            font = UIFont(descriptor: italic, size: 0)
        }
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
                    Text(name).fontWeight(.semibold)
                    Text(detail).foregroundStyle(.secondary).lineLimit(1).truncationMode(.tail)
                    Spacer(minLength: 4)
                    if !children.isEmpty {
                        Image(systemName: expanded ? "chevron.down" : "chevron.right")
                            .font(.caption.weight(.semibold))
                            .foregroundStyle(.tertiary)
                    }
                }
                .font(.subheadline)
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
                .font(.footnote)
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
            Text(label).font(.subheadline)
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

    var body: some View {
        HStack(alignment: .bottom, spacing: 8) {
            NativeComposerField(text: $text, placeholder: props.placeholder, maxBytes: props.maxBytes,
                                enabled: props.enabled, send: send)
                .frame(maxWidth: .infinity, alignment: .leading)
                .overlay(alignment: .topLeading) {
                    if text.isEmpty {
                        Text(props.placeholder)
                            .font(.system(size: 16))
                            .foregroundStyle(.tertiary)
                            .allowsHitTesting(false)
                            .accessibilityHidden(true)
                    }
                }
                .padding(.horizontal, 14)
                .padding(.vertical, 9)
                .background(Color(uiColor: NativeChatPalette.raised),
                            in: RoundedRectangle(cornerRadius: 20, style: .continuous))
            Button(action: props.busy ? stop : send) {
                Image(systemName: props.busy ? "stop.fill" : "arrow.up")
                    .font(.system(size: 15, weight: .bold))
                    .foregroundStyle(Color(uiColor: .systemBackground))
                    .frame(width: 36, height: 36)
                    .background(Circle().fill(Color(uiColor: .label)))
                    .opacity(controlEnabled ? 1 : 0.3)
            }
            .buttonStyle(.plain)
            .disabled(!controlEnabled)
            .accessibilityLabel(props.busy ? "Stop" : "Send")
            .accessibilityIdentifier("\(key)-\(props.busy ? "stop" : "send")")
            .padding(.bottom, 1)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .frame(maxWidth: NativeChatPalette.readingWidth + 24)
        .frame(maxWidth: .infinity)
        .opacity(props.enabled ? 1 : 0.6)
        .accessibilityIdentifier(key)
    }

    private var controlEnabled: Bool { props.busy ? props.stoppable : canSend }

    private func send() {
        guard canSend, let submit else { return }
        let value = text
        text = ""
        submit(props.token, value)
    }

    private func stop() {
        guard props.busy, props.stoppable else { return }
        activate(key)
    }
}

/// A text view that grows from one to six lines and sends on Command-Return.
private struct NativeComposerField: UIViewRepresentable {
    @Binding var text: String
    let placeholder: String
    let maxBytes: Int
    let enabled: Bool
    let send: () -> Void

    private static let font = UIFontMetrics(forTextStyle: .body).scaledFont(for: .systemFont(ofSize: 16))
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
        return view
    }

    func updateUIView(_ view: NativeComposerTextView, context: Context) {
        context.coordinator.parent = self
        view.onCommandReturn = { [weak coordinator = context.coordinator] in coordinator?.parent.send() }
        if view.text != text { view.text = text }
        view.isEditable = enabled
        view.accessibilityLabel = placeholder
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

        init(_ parent: NativeComposerField) { self.parent = parent }

        func textViewDidChange(_ textView: UITextView) {
            parent.text = textView.text
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

    override var keyCommands: [UIKeyCommand]? {
        let send = UIKeyCommand(title: "Send", action: #selector(commandReturn), input: "\r",
                                modifierFlags: .command)
        send.wantsPriorityOverSystemBehavior = true
        return (super.keyCommands ?? []) + [send]
    }

    @objc private func commandReturn() { onCommandReturn?() }
}
