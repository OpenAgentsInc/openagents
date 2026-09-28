// The transcript painter. Rust lays out every row (crates/rust-native,
// `layout`): exact heights, cumulative offsets, and display lists with each
// text run's position. This file only measures text for Rust with CoreText,
// paints the runs at Rust's positions, scrolls, and handles gestures.
//
// The scrolling rules (follow the tail, re-engage near the bottom, keep the
// reader's place when rows arrive above) and the CoreText painter follow the
// design of zeronsh/comet's transcript (MIT License), reimplemented here.
import Combine
import CoreText
import SwiftUI
import UIKit

// MARK: - Measurement

/// System fonts by Rust's font description, shared by measuring and painting
/// so both use the same face.
enum NativeTextFonts {
    private struct Key: Hashable {
        let size: UInt32
        let weight: UInt8
        let italic: Bool
        let mono: Bool
    }

    private static let lock = NSLock()
    nonisolated(unsafe) private static var fonts: [Key: UIFont] = [:]

    static func font(size: Float, weight: UInt8, italic: Bool, mono: Bool) -> UIFont {
        let key = Key(size: size.bitPattern, weight: weight, italic: italic, mono: mono)
        lock.lock()
        defer { lock.unlock() }
        if let font = fonts[key] { return font }
        let weights: [UIFont.Weight] = [.regular, .medium, .semibold, .bold]
        let uiWeight = weights[min(Int(weight), weights.count - 1)]
        let points = CGFloat(max(1, min(size, 400)))
        var font = mono ? UIFont.monospacedSystemFont(ofSize: points, weight: uiWeight)
                        : UIFont.systemFont(ofSize: points, weight: uiWeight)
        if italic, let descriptor = font.fontDescriptor.withSymbolicTraits(
            font.fontDescriptor.symbolicTraits.union(.traitItalic)) {
            font = UIFont(descriptor: descriptor, size: points)
        }
        fonts[key] = font
        return font
    }

    static func weight(_ name: String) -> UInt8 {
        switch name {
        case "medium": return 1
        case "semibold": return 2
        case "bold": return 3
        default: return 0
        }
    }
}

/// Rust's measurer: breaks a styled paragraph into lines with CoreText and
/// reports each line's range, metrics, and run-boundary offsets.
let nativeLayoutMeasure: RustNativeMeasure = { _, text, textLength, runs, runCount, width, lines,
                                                lineCapacity, lineCount, offsets, offsetCapacity, offsetCount in
    guard let text, let runs, let lineCount, let offsetCount else { return -1 }
    let string = String(decoding: UnsafeBufferPointer(start: text, count: textLength), as: UTF8.self)
    let attributed = NSMutableAttributedString(string: string)
    let length = attributed.length
    var boundaries: [Int] = []
    for index in 0..<runCount {
        let run = runs[index]
        let start = Int(run.start16)
        let end = Int(run.end16)
        guard start <= end, end <= length else { return -1 }
        let font = NativeTextFonts.font(size: run.size, weight: run.weight, italic: run.italic != 0,
                                        mono: run.monospace != 0)
        attributed.addAttribute(.font, value: font, range: NSRange(location: start, length: end - start))
        if index > 0 { boundaries.append(start) }
    }
    let typesetter = CTTypesetterCreateWithAttributedString(attributed)
    let limit = width > 0 ? Double(width) : 1.0e7
    var measured: [RustNativeTextLine] = []
    var xs: [Float] = []
    var start = 0
    while start < length {
        var count = CTTypesetterSuggestLineBreak(typesetter, start, limit)
        if count <= 0 { count = max(1, CTTypesetterSuggestClusterBreak(typesetter, start, limit)) }
        count = min(count, length - start)
        let line = CTTypesetterCreateLine(typesetter, CFRange(location: start, length: count))
        var ascent: CGFloat = 0
        var descent: CGFloat = 0
        var leading: CGFloat = 0
        let typographic = CTLineGetTypographicBounds(line, &ascent, &descent, &leading)
        let trailing = CTLineGetTrailingWhitespaceWidth(line)
        measured.append(RustNativeTextLine(start16: UInt32(start), end16: UInt32(start + count),
                                           width: Float(max(0, typographic - trailing)),
                                           ascent: Float(ascent), descent: Float(descent),
                                           leading: Float(max(0, leading))))
        for boundary in boundaries where boundary > start && boundary < start + count {
            xs.append(Float(CTLineGetOffsetForStringIndex(line, boundary, nil)))
        }
        start += count
    }
    lineCount.pointee = measured.count
    offsetCount.pointee = xs.count
    guard measured.count <= lineCapacity, xs.count <= offsetCapacity else { return 1 }
    if let lines { for (index, line) in measured.enumerated() { lines[index] = line } }
    if let offsets { for (index, x) in xs.enumerated() { offsets[index] = x } }
    return 0
}

// MARK: - Rust layout handle

/// One transcript's Rust layout. Calls stay on the main thread.
@MainActor
final class NativeTranscriptLayout {
    struct Summary: Decodable {
        let count: Int
        let height: CGFloat
        let relaid: Int
        let measured: Int
        let micros: Int
    }

    private let handle: UnsafeMutableRawPointer

    init?() {
        guard let handle = rust_native_layout_create(nil, nativeLayoutMeasure) else { return nil }
        self.handle = handle
    }

    deinit { rust_native_layout_destroy(handle) }

    private static func take(_ buffer: RustNativeBuffer) -> Data? {
        guard let data = buffer.data, buffer.len > 0 else { return nil }
        let bytes = Data(bytes: data, count: buffer.len)
        rust_native_layout_buffer_free(buffer)
        return bytes
    }

    func update(_ request: Data) -> Summary? {
        let reply = request.withUnsafeBytes { bytes in
            rust_native_layout_update(handle, bytes.bindMemory(to: UInt8.self).baseAddress, bytes.count)
        }
        guard let data = Self.take(reply) else { return nil }
        if let summary = try? JSONDecoder().decode(Summary.self, from: data) { return summary }
        #if DEBUG || targetEnvironment(simulator)
        print("transcript layout refused an update: \(String(decoding: data, as: UTF8.self))")
        #endif
        return nil
    }

    var height: CGFloat { CGFloat(rust_native_layout_height(handle)) }

    func rows(_ y0: CGFloat, _ y1: CGFloat) -> [RustNativeRowPlacement] {
        var placements = [RustNativeRowPlacement](repeating: RustNativeRowPlacement(), count: 64)
        var count = rust_native_layout_rows(handle, Float(y0), Float(y1), &placements, placements.count)
        if count > placements.count {
            placements = [RustNativeRowPlacement](repeating: RustNativeRowPlacement(), count: count)
            count = rust_native_layout_rows(handle, Float(y0), Float(y1), &placements, placements.count)
        }
        return Array(placements.prefix(min(count, placements.count)))
    }

    func find(_ key: String) -> RustNativeRowPlacement? {
        var placement = RustNativeRowPlacement()
        let bytes = Array(key.utf8)
        return rust_native_layout_find(handle, bytes, bytes.count, &placement) == 1 ? placement : nil
    }

    func display(_ index: UInt32) -> NativeRowDisplay? {
        guard let data = Self.take(rust_native_layout_display(handle, index)) else { return nil }
        return try? JSONDecoder().decode(NativeRowDisplay.self, from: data)
    }
}

// MARK: - Display lists

/// A row's display list, as `rust_native::layout::display` serializes it.
struct NativeRowDisplay: Decodable {
    struct Font: Decodable {
        let size: Float
        let weight: String
        let italic: Bool?
        let mono: Bool?

        var uiFont: UIFont {
            NativeTextFonts.font(size: size, weight: NativeTextFonts.weight(weight), italic: italic ?? false,
                                 mono: mono ?? false)
        }
    }

    struct Ink: Decodable {
        let role: String?
        let rgba: [UInt8]?

        func color(opacity: CGFloat = 1) -> UIColor {
            let base: UIColor
            if let rgba, rgba.count == 4 {
                base = UIColor(red: CGFloat(rgba[0]) / 255, green: CGFloat(rgba[1]) / 255,
                               blue: CGFloat(rgba[2]) / 255, alpha: CGFloat(rgba[3]) / 255)
            } else {
                switch role {
                case "secondary": base = .secondaryLabel
                case "tertiary": base = .tertiaryLabel
                case "link": base = NativeChatPalette.link
                case "bubble": base = NativeChatPalette.bubble
                case "surface": base = NativeChatPalette.surface
                case "raised": base = NativeChatPalette.raised
                case "border": base = NativeChatPalette.border
                case "inline_code": base = NativeChatPalette.inlineCode
                default: base = .label
                }
            }
            return opacity < 1 ? base.withAlphaComponent(base.cgColor.alpha * opacity) : base
        }
    }

    struct Style: Decodable {
        let font: Font
        let ink: Ink
        let opacity: CGFloat
        let underline: Bool?
        let strike: Bool?
    }

    struct Run: Decodable {
        let text: Int
        let start16: Int
        let len16: Int
        let x: CGFloat
        let baseline: CGFloat
        let width: CGFloat
        let style: Int
        let truncate: CGFloat?
    }

    struct Rect: Decodable {
        let x: CGFloat
        let y: CGFloat
        let w: CGFloat
        let h: CGFloat
        let radii: [CGFloat]
        let fill: Ink?
        let stroke: Ink?

        var frame: CGRect { CGRect(x: x, y: y, width: w, height: h) }
    }

    struct Widget: Decodable {
        let x: CGFloat
        let y: CGFloat
        let w: CGFloat
        let h: CGFloat
        let kind: String
        let text: String?
        let key: String?
        let expanded: Bool?
        let state: String?
        let checked: Bool?
        let loading: Bool?

        var frame: CGRect { CGRect(x: x, y: y, width: w, height: h) }
    }

    struct Accessibility: Decodable {
        let label: String
        let value: String?
        let hint: String?
        let button: Bool?
    }

    let key: String
    let height: CGFloat
    let styles: [Style]
    let texts: [String]
    let runs: [Run]
    let rects: [Rect]
    let widgets: [Widget]
    let accessibility: Accessibility
    let copy: String?
}

/// A display list with its CoreText lines, built once per row version.
final class NativeRowModel {
    let display: NativeRowDisplay
    private let lines: [CTLine?]

    init(_ display: NativeRowDisplay) {
        self.display = display
        let texts = display.texts.map { $0 as NSString }
        lines = display.runs.map { run in
            guard run.text < texts.count, run.style < display.styles.count else { return nil }
            let text = texts[run.text]
            let range = NSRange(location: run.start16, length: run.len16)
            guard range.location >= 0, range.length > 0, NSMaxRange(range) <= text.length else { return nil }
            let font = display.styles[run.style].font.uiFont
            let attributes: [NSAttributedString.Key: Any] = [
                .font: font,
                NSAttributedString.Key(kCTForegroundColorFromContextAttributeName as String): true,
            ]
            let line = CTLineCreateWithAttributedString(
                NSAttributedString(string: text.substring(with: range), attributes: attributes))
            guard let limit = run.truncate else { return line }
            let ellipsis = CTLineCreateWithAttributedString(NSAttributedString(string: "\u{2026}", attributes: attributes))
            return CTLineCreateTruncatedLine(line, Double(max(0, limit)), .end, ellipsis) ?? line
        }
    }

    /// Paints the part of the row inside `band` (row coordinates) into a
    /// context whose origin is the band's top-left.
    func draw(_ band: CGRect, in context: CGContext, scale: CGFloat) {
        context.saveGState()
        context.translateBy(x: 0, y: -band.minY)
        for rect in display.rects where rect.frame.intersects(band.insetBy(dx: 0, dy: -2)) {
            let path = Self.path(rect.frame, radii: rect.radii)
            if let fill = rect.fill {
                context.setFillColor(fill.color().cgColor)
                context.addPath(path)
                context.fillPath()
            }
            if let stroke = rect.stroke {
                let inset = rect.frame.insetBy(dx: 0.5, dy: 0.5)
                context.setStrokeColor(stroke.color().cgColor)
                context.setLineWidth(1)
                context.addPath(Self.path(inset, radii: rect.radii.map { max(0, $0 - 0.5) }))
                context.strokePath()
            }
        }
        context.textMatrix = .identity
        for (index, run) in display.runs.enumerated() {
            guard let line = lines[index], run.style < display.styles.count else { continue }
            let style = display.styles[run.style]
            let size = CGFloat(style.font.size)
            guard run.baseline + size > band.minY, run.baseline - size * 1.2 < band.maxY else { continue }
            let color = style.ink.color(opacity: style.opacity).cgColor
            context.saveGState()
            context.setFillColor(color)
            context.translateBy(x: run.x, y: run.baseline)
            context.scaleBy(x: 1, y: -1)
            context.textPosition = .zero
            CTLineDraw(line, context)
            context.restoreGState()
            let hairline = max(1 / scale, 1)
            if style.underline == true {
                context.setFillColor(color)
                context.fill(CGRect(x: run.x, y: run.baseline + 2, width: run.width, height: hairline))
            }
            if style.strike == true {
                context.setFillColor(color)
                context.fill(CGRect(x: run.x, y: run.baseline - size * 0.3, width: run.width, height: hairline))
            }
        }
        context.restoreGState()
    }

    /// A rounded rectangle with radii: top leading, top trailing, bottom
    /// trailing, bottom leading.
    static func path(_ rect: CGRect, radii: [CGFloat]) -> CGPath {
        let limit = min(rect.width, rect.height) / 2
        let r = (0..<4).map { min(limit, max(0, $0 < radii.count ? radii[$0] : 0)) }
        if r.allSatisfy({ $0 == r[0] }) {
            return CGPath(roundedRect: rect, cornerWidth: r[0], cornerHeight: r[0], transform: nil)
        }
        let path = CGMutablePath()
        path.move(to: CGPoint(x: rect.minX + r[0], y: rect.minY))
        path.addLine(to: CGPoint(x: rect.maxX - r[1], y: rect.minY))
        path.addArc(tangent1End: CGPoint(x: rect.maxX, y: rect.minY),
                    tangent2End: CGPoint(x: rect.maxX, y: rect.minY + r[1]), radius: r[1])
        path.addLine(to: CGPoint(x: rect.maxX, y: rect.maxY - r[2]))
        path.addArc(tangent1End: CGPoint(x: rect.maxX, y: rect.maxY),
                    tangent2End: CGPoint(x: rect.maxX - r[2], y: rect.maxY), radius: r[2])
        path.addLine(to: CGPoint(x: rect.minX + r[3], y: rect.maxY))
        path.addArc(tangent1End: CGPoint(x: rect.minX, y: rect.maxY),
                    tangent2End: CGPoint(x: rect.minX, y: rect.maxY - r[3]), radius: r[3])
        path.addLine(to: CGPoint(x: rect.minX, y: rect.minY + r[0]))
        path.addArc(tangent1End: CGPoint(x: rect.minX, y: rect.minY),
                    tangent2End: CGPoint(x: rect.minX + r[0], y: rect.minY), radius: r[0])
        path.closeSubpath()
        return path
    }
}

// MARK: - Row views

/// One horizontal stripe of a row's painting. Tall rows paint in stripes so
/// no backing store grows with a long message.
private final class NativeRowStripe: UIView {
    var model: NativeRowModel?
    var band: CGRect = .zero

    override init(frame: CGRect) {
        super.init(frame: frame)
        isOpaque = false
        backgroundColor = .clear
        contentMode = .redraw
        isUserInteractionEnabled = false
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    override func draw(_ rect: CGRect) {
        guard let model, let context = UIGraphicsGetCurrentContext() else { return }
        traitCollection.performAsCurrent {
            model.draw(band, in: context, scale: traitCollection.displayScale)
        }
    }
}

/// A painted transcript row with its native widgets.
@MainActor
final class NativeRowView: UIView, UIContextMenuInteractionDelegate {
    static let stripeHeight: CGFloat = 512

    private(set) var key = ""
    private(set) var version: UInt64 = 0
    private(set) var model: NativeRowModel?
    private var stripes: [Int: NativeRowStripe] = [:]
    private var widgetViews: [UIView] = []
    var toggle: ((String) -> Void)?
    var loadEarlier: (() -> Void)?

    override init(frame: CGRect) {
        super.init(frame: frame)
        backgroundColor = .clear
        isAccessibilityElement = true
        addInteraction(UIContextMenuInteraction(delegate: self))
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    func apply(_ model: NativeRowModel, version: UInt64) {
        self.model = model
        self.version = version
        key = model.display.key
        for stripe in stripes.values {
            stripe.model = model
            stripe.setNeedsDisplay()
        }
        rebuildWidgets()
        let access = model.display.accessibility
        accessibilityLabel = access.label
        accessibilityValue = access.value
        accessibilityHint = access.hint
        accessibilityTraits = access.button == true ? .button : .staticText
        accessibilityIdentifier = key == NativeTranscriptView.earlierKey ? "transcript-earlier" : key
        var actions: [UIAccessibilityCustomAction] = []
        if let copy = model.display.copy, !copy.isEmpty {
            actions.append(UIAccessibilityCustomAction(name: "Copy") { _ in
                UIPasteboard.general.string = copy
                return true
            })
        }
        for widget in model.display.widgets where widget.kind == "copy" {
            let text = widget.text ?? ""
            actions.append(UIAccessibilityCustomAction(name: "Copy code") { _ in
                UIPasteboard.general.string = text
                return true
            })
        }
        accessibilityCustomActions = actions
    }

    /// Clears a recycled row so it keeps no content or callbacks.
    func reset() {
        model = nil
        version = 0
        key = ""
        toggle = nil
        loadEarlier = nil
        stripes.values.forEach { $0.removeFromSuperview() }
        stripes.removeAll()
        widgetViews.forEach { $0.removeFromSuperview() }
        widgetViews.removeAll()
        accessibilityLabel = nil
        accessibilityCustomActions = nil
    }

    /// Materializes the stripes that intersect `visible` (row coordinates).
    func show(_ visible: CGRect) {
        guard let model else { return }
        let height = bounds.height
        let first = max(0, Int(floor(max(0, visible.minY) / Self.stripeHeight)))
        let last = max(first, Int(floor(min(height, max(0, visible.maxY)) / Self.stripeHeight)))
        let needed = Set(first...last)
        for (index, stripe) in stripes where !needed.contains(index) {
            stripe.removeFromSuperview()
            stripes[index] = nil
        }
        for index in needed {
            let top = CGFloat(index) * Self.stripeHeight
            guard top < height || index == 0 else { continue }
            let frame = CGRect(x: 0, y: top, width: bounds.width, height: min(Self.stripeHeight, height - top))
            let stripe = stripes[index] ?? {
                let stripe = NativeRowStripe(frame: frame)
                insertSubview(stripe, at: 0)
                stripes[index] = stripe
                return stripe
            }()
            if stripe.frame != frame || stripe.model !== model {
                stripe.frame = frame
                stripe.band = frame
                stripe.model = model
                stripe.setNeedsDisplay()
            }
        }
    }

    func repaint() { stripes.values.forEach { $0.setNeedsDisplay() } }

    private func rebuildWidgets() {
        widgetViews.forEach { $0.removeFromSuperview() }
        widgetViews.removeAll()
        guard let model else { return }
        for widget in model.display.widgets {
            guard let view = makeWidget(widget) else { continue }
            view.frame = widget.frame
            addSubview(view)
            widgetViews.append(view)
        }
    }

    private func makeWidget(_ widget: NativeRowDisplay.Widget) -> UIView? {
        switch widget.kind {
        case "copy":
            return NativeCopyButton(text: widget.text ?? "")
        case "toggle":
            let button = UIButton(type: .custom)
            let key = widget.key ?? ""
            button.addAction(UIAction { [weak self] _ in self?.toggle?(key) }, for: .touchUpInside)
            button.isAccessibilityElement = false
            return button
        case "chevron":
            let image = UIImageView(image: UIImage(systemName: widget.expanded == true ? "chevron.down" : "chevron.right",
                                                   withConfiguration: UIImage.SymbolConfiguration(pointSize: 11, weight: .semibold)))
            image.tintColor = .tertiaryLabel
            image.contentMode = .center
            return image
        case "status":
            switch widget.state {
            case "running":
                let spinner = UIActivityIndicatorView(style: .medium)
                spinner.transform = CGAffineTransform(scaleX: 0.75, y: 0.75)
                spinner.startAnimating()
                return spinner
            case "failed":
                return symbol("xmark.circle.fill", color: .systemRed, size: 15)
            default:
                return symbol("checkmark.circle.fill", color: .systemGreen, size: 15)
            }
        case "checkbox":
            return widget.checked == true ? symbol("checkmark.square.fill", color: .systemGreen, size: 15)
                                          : symbol("square", color: .secondaryLabel, size: 15)
        case "working":
            return NativeWorkingDots()
        case "spinner":
            let spinner = UIActivityIndicatorView(style: .medium)
            spinner.transform = CGAffineTransform(scaleX: 0.8, y: 0.8)
            spinner.startAnimating()
            return spinner
        case "earlier":
            guard widget.loading != true else { return nil }
            let button = UIButton(type: .custom)
            button.addAction(UIAction { [weak self] _ in self?.loadEarlier?() }, for: .touchUpInside)
            button.isAccessibilityElement = false
            return button
        default:
            return nil
        }
    }

    private func symbol(_ name: String, color: UIColor, size: CGFloat) -> UIImageView {
        let view = UIImageView(image: UIImage(systemName: name,
                                              withConfiguration: UIImage.SymbolConfiguration(pointSize: size)))
        view.tintColor = color
        view.contentMode = .center
        return view
    }

    override func accessibilityActivate() -> Bool {
        guard let model else { return false }
        if let toggle = model.display.widgets.first(where: { $0.kind == "toggle" }), let key = toggle.key {
            self.toggle?(key)
            return true
        }
        if model.display.widgets.contains(where: { $0.kind == "earlier" && $0.loading != true }) {
            loadEarlier?()
            return true
        }
        return false
    }

    // A long press on a message offers Copy and Select Text. The painted text
    // is not selectable in place; Select Text opens it in a text view.
    func contextMenuInteraction(_ interaction: UIContextMenuInteraction,
                                configurationForMenuAtLocation location: CGPoint) -> UIContextMenuConfiguration? {
        guard let copy = model?.display.copy, !copy.isEmpty else { return nil }
        return UIContextMenuConfiguration(identifier: nil, previewProvider: nil) { [weak self] _ in
            UIMenu(children: [
                UIAction(title: "Copy", image: UIImage(systemName: "doc.on.doc")) { _ in
                    UIPasteboard.general.string = copy
                },
                UIAction(title: "Select Text", image: UIImage(systemName: "selection.pin.in.out")) { _ in
                    self?.presentSelection(copy)
                },
            ])
        }
    }

    private func presentSelection(_ text: String) {
        var presenter = window?.rootViewController
        while let next = presenter?.presentedViewController { presenter = next }
        let controller = UINavigationController(rootViewController: NativeSelectTextController(text: text))
        if let sheet = controller.sheetPresentationController {
            sheet.detents = [.medium(), .large()]
            sheet.prefersGrabberVisible = true
        }
        presenter?.present(controller, animated: true)
    }
}

/// A message's text in a selectable text view.
private final class NativeSelectTextController: UIViewController {
    private let text: String

    init(text: String) {
        self.text = text
        super.init(nibName: nil, bundle: nil)
        title = "Select Text"
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground
        let textView = UITextView(frame: view.bounds)
        textView.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        textView.isEditable = false
        textView.isSelectable = true
        textView.dataDetectorTypes = []
        textView.font = UIFontMetrics(forTextStyle: .body).scaledFont(for: .systemFont(ofSize: 16))
        textView.adjustsFontForContentSizeCategory = true
        textView.textContainerInset = UIEdgeInsets(top: 16, left: 12, bottom: 16, right: 12)
        textView.text = text
        textView.accessibilityIdentifier = "transcript-select-text"
        view.addSubview(textView)
        navigationItem.rightBarButtonItem = UIBarButtonItem(
            systemItem: .done, primaryAction: UIAction { [weak self] _ in self?.dismiss(animated: true) })
    }
}

private final class NativeCopyButton: UIButton {
    private let text: String
    private var reset: DispatchWorkItem?

    init(text: String) {
        self.text = text
        super.init(frame: .zero)
        show(copied: false)
        addAction(UIAction { [weak self] _ in self?.copyText() }, for: .touchUpInside)
        accessibilityHint = "Copies this code block"
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    private func show(copied: Bool) {
        var configuration = UIButton.Configuration.plain()
        configuration.title = copied ? "Copied" : "Copy"
        configuration.image = UIImage(systemName: copied ? "checkmark" : "doc.on.doc",
                                      withConfiguration: UIImage.SymbolConfiguration(pointSize: 11))
        configuration.imagePadding = 4
        configuration.baseForegroundColor = .secondaryLabel
        configuration.contentInsets = NSDirectionalEdgeInsets(top: 0, leading: 4, bottom: 0, trailing: 4)
        configuration.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { attributes in
            var attributes = attributes
            attributes.font = UIFont.preferredFont(forTextStyle: .caption1)
            return attributes
        }
        self.configuration = configuration
        contentHorizontalAlignment = .trailing
    }

    private func copyText() {
        UIPasteboard.general.string = text
        show(copied: true)
        reset?.cancel()
        let work = DispatchWorkItem { [weak self] in self?.show(copied: false) }
        reset = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.5, execute: work)
    }
}

private final class NativeWorkingDots: UIView {
    override init(frame: CGRect) {
        super.init(frame: frame)
        isUserInteractionEnabled = false
        for index in 0..<3 {
            let dot = CALayer()
            dot.backgroundColor = UIColor.secondaryLabel.cgColor
            dot.cornerRadius = 3
            dot.frame = CGRect(x: CGFloat(index) * 10, y: 0, width: 6, height: 6)
            dot.opacity = 0.25
            let pulse = CABasicAnimation(keyPath: "opacity")
            pulse.fromValue = 0.25
            pulse.toValue = 1
            pulse.duration = 0.6
            pulse.autoreverses = true
            pulse.repeatCount = .infinity
            pulse.beginTime = CACurrentMediaTime() + Double(index) * 0.2
            pulse.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
            dot.add(pulse, forKey: "pulse")
            layer.addSublayer(dot)
        }
        recolor()
        registerForTraitChanges([UITraitUserInterfaceStyle.self]) { (self: Self, _) in self.recolor() }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    private func recolor() {
        layer.sublayers?.forEach { $0.backgroundColor = UIColor.secondaryLabel.resolvedColor(with: traitCollection).cgColor }
    }
}

// MARK: - Transcript view

/// A bottom-anchored conversation painted from Rust's layout. Rows are pulled
/// by visible range, so only what is on screen (plus overscan) exists.
@MainActor
final class NativeTranscriptView: UIScrollView, UIScrollViewDelegate {
    /// Matches `rust_native::layout::EARLIER_KEY`.
    static let earlierKey = "\u{1}earlier"
    /// How far beyond the screen rows are prepared.
    private static let overscan: CGFloat = 700
    /// Coming to rest this close to the bottom resumes following.
    private static let followBand: CGFloat = 70

    var transcriptKey = ""
    var activate: ((String) -> Void)?
    /// Whether the newest row stays in view as rows arrive and grow.
    var following = true {
        didSet { if following != oldValue { updateBottomButton() } }
    }

    private let layout: NativeTranscriptLayout?
    private var order: [String] = []
    private var nodes: [String: NativeNode] = [:]
    private var earlier: NativeEarlier?
    /// Rows whose current content Rust has not received.
    private var unsent: Set<String> = []
    private var sentWidth: CGFloat = 0
    private var sentScale: CGFloat = 0
    private var sentExpanded: Set<String> = []
    private var sentEarlier: NativeEarlier?
    private var sentOrder: [String] = []
    private var needsSync = true
    private var rowViews: [String: NativeRowView] = [:]
    private var pool: [NativeRowView] = []
    private var models: [String: (version: UInt64, model: NativeRowModel)] = [:]
    private var expansion: AnyCancellable?
    private let bottomButton = UIButton(type: .system)
    private var buttonShown = false
    private let fallback = UILabel()
    fileprivate(set) var stats = NativeTranscriptStats()
    #if DEBUG || targetEnvironment(simulator)
    private var bench: NativeTranscriptBench?
    private var prependChecked = false
    #endif

    override init(frame: CGRect) {
        layout = NativeTranscriptLayout()
        super.init(frame: frame)
        delegate = self
        backgroundColor = .clear
        alwaysBounceVertical = true
        keyboardDismissMode = .interactive
        delaysContentTouches = false
        contentInsetAdjustmentBehavior = .never
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
        if layout == nil {
            fallback.text = "This transcript can't be displayed."
            fallback.textColor = .secondaryLabel
            addSubview(fallback)
        }
        // Text size changes scale every row; appearance changes only repaint.
        registerForTraitChanges([UITraitPreferredContentSizeCategory.self]) { (self: Self, _) in
            self.setNeedsLayout()
        }
        registerForTraitChanges([UITraitUserInterfaceStyle.self]) { (self: Self, _) in
            self.rowViews.values.forEach { $0.repaint() }
        }
        expansion = NativeExpansion.shared.$keys.sink { [weak self] _ in
            DispatchQueue.main.async { self?.expansionChanged() }
        }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    var isInteracting: Bool { isTracking || isDragging || isDecelerating }

    private var bottomOffset: CGFloat {
        max(-adjustedContentInset.top, contentSize.height - bounds.height + adjustedContentInset.bottom)
    }

    private var distanceFromBottom: CGFloat { bottomOffset - contentOffset.y }

    private var textScale: CGFloat {
        let scaled = UIFontMetrics(forTextStyle: .body).scaledValue(for: 16, compatibleWith: traitCollection) / 16
        return min(4, max(0.5, (scaled * 100).rounded() / 100))
    }

    /// A row index's key in the layout Rust last received.
    private func key(at index: Int) -> String? {
        let offset = sentEarlier == nil ? 0 : 1
        if offset == 1, index == 0 { return Self.earlierKey }
        let row = index - offset
        return row >= 0 && row < sentOrder.count ? sentOrder[row] : nil
    }

    /// Takes a new revision's rows. Only rows whose content changed go to Rust.
    func apply(rows: [NativeNode], earlier: NativeEarlier?) {
        var next = [String: NativeNode](minimumCapacity: rows.count)
        for row in rows {
            if nodes[row.key] != row { unsent.insert(row.key) }
            next[row.key] = row
        }
        let newOrder = rows.map(\.key)
        let changed = !unsent.isEmpty || newOrder != order || earlier != self.earlier
        nodes = next
        unsent.formIntersection(next.keys)
        self.earlier = earlier
        let anchor = changed && !following ? visibleAnchor() : nil
        order = newOrder
        if changed { needsSync = true }
        sync(anchor: anchor)
    }

    private func expansionChanged() {
        guard NativeExpansion.shared.keys != sentExpanded else { return }
        needsSync = true
        sync(anchor: following ? nil : visibleAnchor())
    }

    /// Sends Rust what changed and lays out the visible rows.
    private func sync(anchor: Anchor?) {
        guard let layout, bounds.width > 0 else { return }
        let scale = textScale
        let expanded = NativeExpansion.shared.keys
        guard needsSync || bounds.width != sentWidth || scale != sentScale || expanded != sentExpanded else {
            tile()
            return
        }
        let started = CACurrentMediaTime()
        let rows = unsent.compactMap { nodes[$0] }
        let request = NativeLayoutUpdate(
            width: Float(bounds.width), scale: Float(scale),
            // A streamed token keeps the order; Rust then updates in place.
            order: order == sentOrder && !sentOrder.isEmpty ? nil : order, rows: rows,
            expanded: Array(expanded.intersection(nodes.keys)),
            earlier: earlier.map { NativeLayoutUpdate.Earlier(label: $0.label, loading: $0.loading) })
        guard let data = try? JSONEncoder().encode(request), let summary = layout.update(data) else {
            return
        }
        let encoded = CACurrentMediaTime()
        unsent.removeAll()
        needsSync = false
        sentWidth = bounds.width
        sentScale = scale
        sentExpanded = expanded
        sentEarlier = earlier
        sentOrder = order
        stats.record(update: summary, encode: encoded - started, total: CACurrentMediaTime() - started,
                     rows: rows.count)
        #if DEBUG || targetEnvironment(simulator)
        if bench == nil, summary.count > 0, NativeTranscriptBench.requested {
            bench = NativeTranscriptBench(view: self)
        }
        if !prependChecked, summary.count > 0, NativeTranscriptBench.prependRequested {
            prependChecked = true
            NativeTranscriptBench.checkPrepend(self)
        }
        #endif
        let height = summary.height
        if contentSize.height != height || contentSize.width != bounds.width {
            contentSize = CGSize(width: bounds.width, height: height)
        }
        if following {
            pin()
        } else if let anchor, let placement = layout.find(anchor.key) {
            // Shift the bounds, not the content offset, so a fling keeps its
            // momentum while rows arrive above.
            let top = -adjustedContentInset.top
            let target = min(bottomOffset, max(top, CGFloat(placement.y) - anchor.offset))
            if abs(target - bounds.origin.y) > 0.25 { bounds.origin.y = target }
        }
        tile()
    }

    private struct Anchor {
        let key: String
        let offset: CGFloat
    }

    private func visibleAnchor() -> Anchor? {
        guard let layout else { return nil }
        let top = contentOffset.y
        for placement in layout.rows(top, top + bounds.height) {
            guard let key = key(at: Int(placement.index)), key != Self.earlierKey else { continue }
            if CGFloat(placement.y) + CGFloat(placement.height) > top {
                return Anchor(key: key, offset: CGFloat(placement.y) - top)
            }
        }
        return nil
    }

    private func pin() {
        guard !isInteracting else { return }
        let bottom = bottomOffset
        if abs(contentOffset.y - bottom) > 0.25 { contentOffset = CGPoint(x: 0, y: bottom) }
    }

    /// Places the rows in the visible range plus overscan, reusing views.
    private func tile() {
        guard let layout else { return }
        let started = CACurrentMediaTime()
        let visible = CGRect(x: 0, y: contentOffset.y, width: bounds.width, height: bounds.height)
        let placements = layout.rows(visible.minY - Self.overscan, visible.maxY + Self.overscan)
        var keep = Set<String>()
        for placement in placements {
            guard let key = key(at: Int(placement.index)) else { continue }
            keep.insert(key)
            let view = rowViews[key] ?? dequeue(key)
            if view.version != placement.version || view.key != key {
                guard let model = model(for: key, index: placement.index, version: placement.version) else { continue }
                view.apply(model, version: placement.version)
                stats.painted += 1
            }
            let frame = CGRect(x: 0, y: CGFloat(placement.y), width: bounds.width, height: CGFloat(placement.height))
            if view.frame != frame { view.frame = frame }
            view.toggle = { key in NativeExpansion.shared.toggle(key) }
            view.loadEarlier = { [weak self] in self?.loadEarlier() }
            view.show(visible.offsetBy(dx: 0, dy: -frame.minY).insetBy(dx: 0, dy: -Self.overscan / 2))
        }
        for (key, view) in rowViews where !keep.contains(key) {
            view.reset()
            view.removeFromSuperview()
            rowViews[key] = nil
            if pool.count < 24 { pool.append(view) }
        }
        bringSubviewToFront(bottomButton)
        stats.record(tile: CACurrentMediaTime() - started)
    }

    #if DEBUG || targetEnvironment(simulator)
    /// The first visible row's key and its distance from the top of the
    /// screen, for the scripted prepend check.
    func debugAnchor() -> (key: String, offset: CGFloat)? {
        visibleAnchor().map { ($0.key, $0.offset) }
    }

    /// Where a row's top sits on screen now.
    func debugScreenOffset(of key: String) -> CGFloat? {
        layout?.find(key).map { CGFloat($0.y) - contentOffset.y }
    }

    func debugLoadEarlier() { loadEarlier() }
    #endif

    /// Starts a fresh measurement window for the benchmark.
    func resetWorst() {
        stats.worstUpdate = 0
        stats.worstEncode = 0
        stats.worstTile = 0
        stats.painted = 0
    }

    private func dequeue(_ key: String) -> NativeRowView {
        let view = pool.popLast() ?? NativeRowView(frame: .zero)
        insertSubview(view, belowSubview: bottomButton)
        rowViews[key] = view
        return view
    }

    private func model(for key: String, index: UInt32, version: UInt64) -> NativeRowModel? {
        if let cached = models[key], cached.version == version { return cached.model }
        guard let display = layout?.display(index) else { return nil }
        let model = NativeRowModel(display)
        if models.count > 400 { models.removeAll(keepingCapacity: true) }
        models[key] = (version, model)
        return model
    }

    private func loadEarlier() {
        guard let earlier, !earlier.loading else { return }
        activate?(transcriptKey)
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        fallback.frame = bounds.insetBy(dx: 16, dy: 16)
        if bounds.width != sentWidth || needsSync {
            sync(anchor: following ? nil : visibleAnchor())
        } else {
            tile()
        }
        if following, !isInteracting { pin() }
        updateBottomButton()
    }


    // MARK: Following

    func scrollViewWillBeginDragging(_ scrollView: UIScrollView) {
        following = false
    }

    func scrollViewWillEndDragging(_ scrollView: UIScrollView, withVelocity velocity: CGPoint,
                                   targetContentOffset: UnsafeMutablePointer<CGPoint>) {
        // Releasing, or momentum that carries the list, into the bottom band
        // resumes following.
        if bottomOffset - targetContentOffset.pointee.y < Self.followBand {
            following = true
        }
    }

    func scrollViewDidEndDragging(_ scrollView: UIScrollView, willDecelerate decelerate: Bool) {
        if !decelerate { settle() }
    }

    func scrollViewDidEndDecelerating(_ scrollView: UIScrollView) { settle() }

    func scrollViewShouldScrollToTop(_ scrollView: UIScrollView) -> Bool {
        following = false
        return true
    }

    func scrollViewDidScrollToTop(_ scrollView: UIScrollView) { settle() }

    func scrollViewDidScroll(_ scrollView: UIScrollView) {
        tile()
        updateBottomButton()
    }

    /// Resumes following when a scroll comes to rest near the bottom.
    private func settle() {
        if !following { following = distanceFromBottom < Self.followBand }
        if following { pin() }
    }

    private func updateBottomButton() {
        let show = !following && bounds.height > 120 && distanceFromBottom > 80
        bottomButton.frame = CGRect(x: bounds.midX - 20, y: bounds.maxY - adjustedContentInset.bottom - 56,
                                    width: 40, height: 40)
        guard show != buttonShown else { return }
        buttonShown = show
        if show { bottomButton.isHidden = false }
        UIView.animate(withDuration: 0.15, animations: { self.bottomButton.alpha = show ? 1 : 0 },
                       completion: { _ in self.bottomButton.isHidden = !self.buttonShown })
    }

    @objc private func jumpToBottom() {
        // Jump without animating; heights are exact, so the bottom is known.
        following = true
        setContentOffset(contentOffset, animated: false)
        contentOffset = CGPoint(x: 0, y: bottomOffset)
        updateBottomButton()
    }
}

#if DEBUG || targetEnvironment(simulator)
/// A scripted fling benchmark: after the transcript settles, scroll up for
/// four seconds and back down at a fixed speed on the display link, then
/// print frame times. A
/// hitch is a frame later than 1.5 times its budget.
@MainActor
final class NativeTranscriptBench {
    static var requested: Bool {
        ProcessInfo.processInfo.arguments.contains("--rust-native-transcript-bench")
    }

    static var prependRequested: Bool {
        ProcessInfo.processInfo.arguments.contains("--rust-native-transcript-prepend-check")
    }

    /// Scrolls up, loads earlier rows, and prints how far the first visible
    /// row moved on screen. It should not move.
    static func checkPrepend(_ view: NativeTranscriptView) {
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) { [weak view] in
            guard let view else { return }
            view.following = false
            view.contentOffset = CGPoint(x: 0, y: max(0, view.contentSize.height / 3))
            view.layoutIfNeeded()
            guard let anchor = view.debugAnchor() else { print("prepend check: no anchor"); return }
            view.debugLoadEarlier()
            DispatchQueue.main.asyncAfter(deadline: .now() + 1) { [weak view] in
                guard let view, let now = view.debugScreenOffset(of: anchor.key) else { return }
                print(String(format: "prepend check: row %@ moved %.2f pt on screen; content height %.0f",
                             anchor.key, now - anchor.offset, view.contentSize.height))
            }
        }
    }

    private weak var view: NativeTranscriptView?
    private var link: CADisplayLink?
    private var last: CFTimeInterval = 0
    private var deltas: [Double] = []
    private var budgets: [Double] = []
    private var direction: CGFloat = -1
    private var legs = 0
    private var legTime: CGFloat = 0
    private let speed: CGFloat = 5_200

    init(view: NativeTranscriptView) {
        self.view = view
        DispatchQueue.main.asyncAfter(deadline: .now() + 2) { [weak self] in self?.start() }
    }

    private func start() {
        guard let view else { return }
        view.following = false
        view.resetWorst()
        let link = CADisplayLink(target: self, selector: #selector(tick(_:)))
        link.add(to: .main, forMode: .common)
        self.link = link
        print("transcript bench: start, content height \(Int(view.contentSize.height))")
    }

    @objc private func tick(_ link: CADisplayLink) {
        guard let view else { link.invalidate(); return }
        let now = link.timestamp
        let budget = link.targetTimestamp - link.timestamp
        if last > 0 {
            deltas.append(now - last)
            budgets.append(budget)
        }
        let step = speed * CGFloat(last > 0 ? now - last : budget)
        last = now
        let top = -view.adjustedContentInset.top
        let bottom = max(top, view.contentSize.height - view.bounds.height + view.adjustedContentInset.bottom)
        var y = view.contentOffset.y + direction * step
        legTime += last > 0 ? step / speed : 0
        if y <= top || y >= bottom || legTime >= 4 {
            legTime = 0
            y = min(bottom, max(top, y))
            direction = -direction
            legs += 1
        }
        view.contentOffset = CGPoint(x: 0, y: y)
        if legs >= 2 { finish() }
    }

    private func finish() {
        link?.invalidate()
        link = nil
        guard let view, !deltas.isEmpty else { return }
        let total = deltas.reduce(0, +)
        var hitches = 0
        var hitchTime = 0.0
        for (delta, budget) in zip(deltas, budgets) where delta > budget * 1.5 {
            hitches += 1
            hitchTime += delta - budget
        }
        let stats = view.stats
        print(String(format: "transcript bench: %d frames in %.2f s, mean %.2f ms, worst %.2f ms, %d hitches, hitch ratio %.1f ms/s; worst update %.2f ms (encode %.2f ms), worst tile %.2f ms, %d rows painted",
                     deltas.count, total, total / Double(deltas.count) * 1000, (deltas.max() ?? 0) * 1000,
                     hitches, hitchTime * 1000 / total, stats.worstUpdate * 1000, stats.worstEncode * 1000,
                     stats.worstTile * 1000, stats.painted))
    }
}
#endif

/// Timings a debug build prints, so layout cost can be read without a
/// profiler.
struct NativeTranscriptStats {
    var updates = 0
    var lastUpdate: NativeTranscriptLayout.Summary?
    var worstUpdate: Double = 0
    var worstEncode: Double = 0
    var worstTile: Double = 0
    var painted = 0

    mutating func record(update: NativeTranscriptLayout.Summary, encode: Double, total: Double, rows: Int) {
        updates += 1
        lastUpdate = update
        worstUpdate = max(worstUpdate, total)
        worstEncode = max(worstEncode, encode)
        #if DEBUG || targetEnvironment(simulator)
        if NativeTranscriptStats.logging {
            print(String(format: "transcript update: %d rows sent, %d rows, %d relaid, %d measured, rust %.2f ms, total %.2f ms",
                         rows, update.count, update.relaid, update.measured, Double(update.micros) / 1000, total * 1000))
        }
        #endif
    }

    mutating func record(tile: Double) {
        worstTile = max(worstTile, tile)
    }

    static var logging: Bool {
        ProcessInfo.processInfo.arguments.contains("--rust-native-transcript-log")
    }
}

/// The JSON update `rust_native_layout_update` reads.
private struct NativeLayoutUpdate: Encodable {
    struct Earlier: Encodable {
        let label: String
        let loading: Bool
    }

    let width: Float
    let scale: Float
    let order: [String]?
    let rows: [NativeNode]
    let expanded: [String]
    let earlier: Earlier?

    private enum Keys: String, CodingKey { case width, scale, order, rows, expanded, earlier }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: Keys.self)
        try container.encode(width, forKey: .width)
        try container.encode(scale, forKey: .scale)
        if let order { try container.encode(order, forKey: .order) }
        try container.encode(rows, forKey: .rows)
        try container.encode(expanded, forKey: .expanded)
        if let earlier { try container.encode(earlier, forKey: .earlier) } else { try container.encodeNil(forKey: .earlier) }
    }
}

/// The SwiftUI mount for a transcript node.
struct NativeTranscript: UIViewRepresentable {
    let key: String
    let label: String
    let rows: [NativeNode]
    let earlier: NativeEarlier?
    let revision: UInt64
    let surface: NativeChat.Surface
    let submit: NativeChat.Submit
    let activate: (String) -> Void

    func makeUIView(context: Context) -> NativeTranscriptView {
        let view = NativeTranscriptView(frame: .zero)
        view.accessibilityIdentifier = key
        return view
    }

    func updateUIView(_ view: NativeTranscriptView, context: Context) {
        view.transcriptKey = key
        view.accessibilityLabel = label
        view.activate = activate
        view.apply(rows: rows, earlier: earlier)
    }
}

// MARK: - Encoding rows for Rust

// Rows go to Rust as `rust_native::view::Node<()>` JSON. Intents stay in the
// application; the layout never reads them, so they encode as null.

extension NativeNode: Encodable {
    private enum EncodingKeys: String, CodingKey { case key, style, element }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: EncodingKeys.self)
        try container.encode(key, forKey: .key)
        try container.encode(style, forKey: .style)
        try container.encode(element, forKey: .element)
    }
}

extension NativeElement: Encodable {
    private enum EncodingKeys: String, CodingKey { case kind, props }
    private enum Props: String, CodingKey {
        case axis, children, label, value, role, enabled, resource, earlier, note, blocks, name, detail, state
        case token, placeholder, max_bytes, busy, stop, intent, loading
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: EncodingKeys.self)
        func props(_ kind: String) throws -> KeyedEncodingContainer<Props> {
            try container.encode(kind, forKey: .kind)
            return container.nestedContainer(keyedBy: Props.self, forKey: .props)
        }
        switch self {
        case let .stack(axis, children):
            var p = try props("stack")
            try p.encode(axis, forKey: .axis)
            try p.encode(children, forKey: .children)
        case let .list(label, children):
            var p = try props("list")
            try p.encode(label, forKey: .label)
            try p.encode(children, forKey: .children)
        case let .text(value, role):
            var p = try props("text")
            try p.encode(value, forKey: .value)
            try p.encode(role, forKey: .role)
        case let .button(label, enabled):
            var p = try props("button")
            try p.encode(label, forKey: .label)
            try p.encode(enabled, forKey: .enabled)
            try p.encodeNil(forKey: .intent)
        case let .surface(resource, label):
            var p = try props("surface")
            try p.encode(resource, forKey: .resource)
            try p.encode(label, forKey: .label)
        case let .transcript(label, children, earlier):
            var p = try props("transcript")
            try p.encode(label, forKey: .label)
            try p.encode(children, forKey: .children)
            if let earlier {
                var e = p.nestedContainer(keyedBy: Props.self, forKey: .earlier)
                try e.encode(earlier.label, forKey: .label)
                try e.encode(earlier.loading, forKey: .loading)
                try e.encodeNil(forKey: .intent)
            } else {
                try p.encodeNil(forKey: .earlier)
            }
        case let .message(role, note, children):
            var p = try props("message")
            try p.encode(role, forKey: .role)
            if let note { try p.encode(note, forKey: .note) } else { try p.encodeNil(forKey: .note) }
            try p.encode(children, forKey: .children)
        case let .markdown(blocks):
            var p = try props("markdown")
            try p.encode(blocks, forKey: .blocks)
        case let .tool(name, detail, state, children):
            var p = try props("tool")
            try p.encode(name, forKey: .name)
            try p.encode(detail, forKey: .detail)
            try p.encode(state, forKey: .state)
            try p.encode(children, forKey: .children)
        case let .working(label):
            var p = try props("working")
            try p.encode(label, forKey: .label)
        case let .composer(composer):
            var p = try props("composer")
            try p.encode(composer.token, forKey: .token)
            try p.encode(composer.placeholder, forKey: .placeholder)
            try p.encode(composer.maxBytes, forKey: .max_bytes)
            try p.encode(composer.enabled, forKey: .enabled)
            try p.encode(composer.busy, forKey: .busy)
            try p.encodeNil(forKey: .stop)
        }
    }
}

extension NativeMarkdownSpan: Encodable {
    private enum EncodingKeys: String, CodingKey { case text, bold, italic, strike, code, link }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: EncodingKeys.self)
        try container.encode(text, forKey: .text)
        if bold { try container.encode(true, forKey: .bold) }
        if italic { try container.encode(true, forKey: .italic) }
        if strike { try container.encode(true, forKey: .strike) }
        if code { try container.encode(true, forKey: .code) }
        if let link { try container.encode(link, forKey: .link) }
    }
}

extension NativeMarkdownItem: Encodable {
    private enum EncodingKeys: String, CodingKey { case checked, blocks }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: EncodingKeys.self)
        if let checked { try container.encode(checked, forKey: .checked) }
        try container.encode(blocks, forKey: .blocks)
    }
}

extension NativeMarkdownBlock: Encodable {
    private enum EncodingKeys: String, CodingKey {
        case kind, level, spans, ordered, start, items, language, text, blocks, align, header, rows
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: EncodingKeys.self)
        switch self {
        case let .heading(level, spans):
            try container.encode("heading", forKey: .kind)
            try container.encode(level, forKey: .level)
            try container.encode(spans, forKey: .spans)
        case let .paragraph(spans):
            try container.encode("paragraph", forKey: .kind)
            try container.encode(spans, forKey: .spans)
        case let .list(ordered, start, items):
            try container.encode("list", forKey: .kind)
            try container.encode(ordered, forKey: .ordered)
            try container.encode(start, forKey: .start)
            try container.encode(items, forKey: .items)
        case let .code(language, text):
            try container.encode("code", forKey: .kind)
            if let language { try container.encode(language, forKey: .language) } else {
                try container.encodeNil(forKey: .language)
            }
            try container.encode(text, forKey: .text)
        case let .quote(blocks):
            try container.encode("quote", forKey: .kind)
            try container.encode(blocks, forKey: .blocks)
        case let .table(align, header, rows):
            try container.encode("table", forKey: .kind)
            try container.encode(align, forKey: .align)
            try container.encode(header, forKey: .header)
            try container.encode(rows, forKey: .rows)
        case .rule:
            try container.encode("rule", forKey: .kind)
        }
    }
}
