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

/// Fonts by Rust's font description, shared by measuring and painting
/// so both use the same face. With `bundled`, Rust shapes the text itself
/// (`layout::shape`), and these are the bundled faces with the variations
/// Rust measured, so painting draws exactly what Rust laid out. Without it,
/// they are the app's faces: `UIFont.paper` for text and `UIFont.code`
/// for code.
enum NativeTextFonts {
    /// Draw with the fonts Rust bundles and shapes. `--rust-native-shaped`
    /// turns it on for a launch.
    static let bundled = ProcessInfo.processInfo.arguments.contains("--rust-native-shaped")

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
        if bundled, let font = bundledFont(size: size, weight: weight, italic: italic, mono: mono) {
            fonts[key] = font
            return font
        }
        let weights: [UIFont.Weight] = [.regular, .medium, .semibold, .bold]
        let uiWeight = weights[min(Int(weight), weights.count - 1)]
        let points = CGFloat(max(1, min(size, 400)))
        var font = mono ? UIFont.code(points, weight: uiWeight) : UIFont.paper(points, weight: uiWeight)
        // A face with no italic (Paper Mono has none) draws italic runs upright.
        if italic, let slanted = font.fontDescriptor.withSymbolicTraits(
            font.fontDescriptor.symbolicTraits.union(.traitItalic)) {
            let italicFont = UIFont(descriptor: slanted, size: points)
            if italicFont.familyName == font.familyName { font = italicFont }
        }
        fonts[key] = font
        return font
    }

    nonisolated(unsafe) private static var faces: [UInt32: CTFontDescriptor] = [:]

    /// A bundled face at Rust's variations for this font. Called under `lock`.
    private static func bundledFont(size: Float, weight: UInt8, italic: Bool, mono: Bool) -> UIFont? {
        let spec = rust_native_font_spec(size, weight, italic ? 1 : 0, mono ? 1 : 0)
        let face: CTFontDescriptor
        if let found = faces[spec.face] {
            face = found
        } else {
            var length = 0
            guard let bytes = rust_native_font_data(spec.face, &length) else { return nil }
            // The font file lives as long as the process; no copy is needed.
            let data = Data(bytesNoCopy: UnsafeMutableRawPointer(mutating: bytes), count: length, deallocator: .none)
            guard let descriptors = CTFontManagerCreateFontDescriptorsFromData(data as CFData) as? [CTFontDescriptor],
                  let first = descriptors.first else { return nil }
            faces[spec.face] = first
            face = first
        }
        func tag(_ name: String) -> Int { name.unicodeScalars.reduce(0) { $0 << 8 | Int($1.value) } }
        var variation: [Int: Double] = [tag("wght"): Double(spec.weight)]
        if spec.optical > 0 { variation[tag("opsz")] = Double(spec.optical) }
        var attributes: [CFString: Any] = [kCTFontVariationAttribute: variation]
        if spec.calt == 0 {
            attributes[kCTFontFeatureSettingsAttribute] = [[kCTFontOpenTypeFeatureTag: "calt",
                                                           kCTFontOpenTypeFeatureValue: 0]]
        }
        let descriptor = CTFontDescriptorCreateCopyWithAttributes(face, attributes as CFDictionary)
        return CTFontCreateWithFontDescriptor(descriptor, CGFloat(max(1, min(size, 400))), nil) as UIFont
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

/// One transcript's Rust layout. Updates are encoded and laid out in order on
/// a serial worker queue, so a cold layout of thousands of rows never blocks
/// the main thread. Each finished update hands the main thread an immutable
/// frame; the view keeps painting the previous frame until then.
final class NativeTranscriptLayout: @unchecked Sendable {
    struct Summary: Decodable {
        let count: Int
        let height: CGFloat
        let relaid: Int
        let measured: Int
        let micros: Int
    }

    /// What one update produced. `frame` is nil when Rust refused it.
    struct Outcome {
        let summary: Summary?
        let frame: NativeTranscriptFrame?
        /// Seconds spent encoding the rows, and in the whole update.
        let encode: Double
        let total: Double
    }

    private let handle: UnsafeMutableRawPointer
    private let queue = DispatchQueue(label: "com.openagents.transcript-layout", qos: .userInitiated)

    init?() {
        let created = NativeTextFonts.bundled ? rust_native_layout_create_shaped()
                                              : rust_native_layout_create(nil, nativeLayoutMeasure)
        guard let handle = created else { return nil }
        self.handle = handle
    }

    deinit {
        // Pending updates retain the layout, so none is running now.
        rust_native_layout_destroy(handle)
    }

    static func take(_ buffer: RustNativeBuffer) -> Data? {
        guard let data = buffer.data, buffer.len > 0 else { return nil }
        let bytes = Data(bytes: data, count: buffer.len)
        rust_native_layout_buffer_free(buffer)
        return bytes
    }

    /// Lays out `request` on the worker queue and calls `done` on the main
    /// queue.
    func update(_ request: NativeLayoutUpdate, done: @escaping @MainActor (Outcome) -> Void) {
        queue.async {
            let started = CACurrentMediaTime()
            var summary: Summary?
            var frame: NativeTranscriptFrame?
            let data = try? JSONEncoder().encode(request)
            let encoded = CACurrentMediaTime()
            if let data {
                let reply = data.withUnsafeBytes { bytes in
                    rust_native_layout_update(self.handle, bytes.bindMemory(to: UInt8.self).baseAddress, bytes.count)
                }
                if let reply = Self.take(reply) {
                    summary = try? JSONDecoder().decode(Summary.self, from: reply)
                    #if DEBUG || targetEnvironment(simulator) || RUST_NATIVE_BENCH
                    if summary == nil {
                        print("transcript layout refused an update: \(String(decoding: reply, as: UTF8.self))")
                    }
                    #endif
                }
                if summary != nil, let raw = rust_native_layout_frame(self.handle) {
                    frame = NativeTranscriptFrame(raw)
                }
            }
            let result = Outcome(summary: summary, frame: frame, encode: encoded - started,
                                total: CACurrentMediaTime() - started)
            DispatchQueue.main.async { MainActor.assumeIsolated { done(result) } }
        }
    }
}

/// One immutable layout from Rust: keys, versions, offsets, and display
/// lists. Reading it never waits for an update in progress.
final class NativeTranscriptFrame: @unchecked Sendable {
    private let raw: UnsafeRawPointer
    private var keys: [UInt32: String] = [:]

    init(_ raw: UnsafeRawPointer) { self.raw = raw }

    deinit { rust_native_frame_release(raw) }

    var height: CGFloat { CGFloat(rust_native_frame_height(raw)) }

    var count: Int { rust_native_frame_count(raw) }

    func rows(_ y0: CGFloat, _ y1: CGFloat) -> [RustNativeRowPlacement] {
        var placements = [RustNativeRowPlacement](repeating: RustNativeRowPlacement(), count: 64)
        var count = rust_native_frame_rows(raw, Float(y0), Float(y1), &placements, placements.count)
        if count > placements.count {
            placements = [RustNativeRowPlacement](repeating: RustNativeRowPlacement(), count: count)
            count = rust_native_frame_rows(raw, Float(y0), Float(y1), &placements, placements.count)
        }
        return Array(placements.prefix(min(count, placements.count)))
    }

    func find(_ key: String) -> RustNativeRowPlacement? {
        var placement = RustNativeRowPlacement()
        let bytes = Array(key.utf8)
        return rust_native_frame_find(raw, bytes, bytes.count, &placement) == 1 ? placement : nil
    }

    /// A row index's key; the earlier control is `NativeTranscriptView.earlierKey`.
    func key(_ index: UInt32) -> String? {
        if let key = keys[index] { return key }
        guard let data = NativeTranscriptLayout.take(rust_native_frame_key(raw, index)) else { return nil }
        let key = String(decoding: data, as: UTF8.self)
        keys[index] = key
        return key
    }

    func display(_ index: UInt32) -> NativeRowDisplay? {
        guard let data = NativeTranscriptLayout.take(rust_native_frame_display(raw, index)) else { return nil }
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
        /// A `surface` widget's resource and spoken label.
        let resource: String?
        let label: String?

        var frame: CGRect { CGRect(x: x, y: y, width: w, height: h) }
    }

    /// A region that scrolls sideways; its items are listed by range.
    struct Scroller: Decodable {
        let x: CGFloat
        let y: CGFloat
        let w: CGFloat
        let h: CGFloat
        let content_w: CGFloat
        let runs: [Int]
        let rects: [Int]

        var frame: CGRect { CGRect(x: x, y: y, width: w, height: h) }
    }

    struct Accessibility: Decodable {
        let label: String
        let value: String?
        let hint: String?
        let button: Bool?
    }

    /// A code paragraph and its fence's language, for syntax colors.
    struct CodeBlock: Decodable {
        let text: Int
        let language: String
    }

    let key: String
    let height: CGFloat
    let styles: [Style]
    let texts: [String]
    let code_blocks: [CodeBlock]?
    let runs: [Run]
    let rects: [Rect]
    let widgets: [Widget]
    let scrollers: [Scroller]?
    let accessibility: Accessibility
    let copy: String?
}

/// Paint-only syntax colors for code blocks, from Rust Native's shared
/// highlighter (`rust_native_syntax_spans`). Highlighting runs on a worker;
/// a row paints plain code until its spans arrive, then `ready` repaints it.
/// Fonts, text, and layout never change.
final class NativeSyntax {
    static let shared = NativeSyntax()
    static let ready = Notification.Name("NativeSyntaxReady")

    struct Span {
        let start: Int
        let length: Int
        let color: UIColor
    }

    private struct Key: Hashable {
        let language: String
        let text: String
        let light: Bool
    }

    // Main thread only.
    private var cache: [Key: [Span]] = [:]
    private var pending: Set<Key> = []
    private let queue = DispatchQueue(label: "com.openagents.syntax", qos: .utility)

    /// The spans for `text`, or nil while they are being worked out.
    func spans(language: String, text: String, light: Bool) -> [Span]? {
        let key = Key(language: language, text: text, light: light)
        if let spans = cache[key] { return spans }
        guard !language.isEmpty, text.utf8.count <= 64 * 1024, pending.count < 16,
              !pending.contains(key) else { return nil }
        pending.insert(key)
        queue.async {
            let spans = Self.highlight(key)
            DispatchQueue.main.async {
                self.pending.remove(key)
                if self.cache.count >= 64 { self.cache.removeAll() }
                self.cache[key] = spans
                if !spans.isEmpty { NotificationCenter.default.post(name: Self.ready, object: nil) }
            }
        }
        return nil
    }

    private static func highlight(_ key: Key) -> [Span] {
        let language = Array(key.language.utf8)
        let text = Array(key.text.utf8)
        let buffer = language.withUnsafeBufferPointer { language in
            text.withUnsafeBufferPointer { text in
                rust_native_syntax_spans(language.baseAddress, language.count, text.baseAddress, text.count,
                                         key.light ? 1 : 0)
            }
        }
        defer { rust_native_layout_buffer_free(buffer) }
        guard let pointer = buffer.data, buffer.len > 0,
              let rows = try? JSONSerialization.jsonObject(with: Data(bytes: pointer, count: buffer.len))
                as? [[Any]] else { return [] }
        return rows.compactMap { row in
            guard row.count == 3, let start = row[0] as? Int, let length = row[1] as? Int,
                  let rgba = row[2] as? [Int], rgba.count == 4 else { return nil }
            return Span(start: start, length: length,
                        color: UIColor(red: CGFloat(rgba[0]) / 255, green: CGFloat(rgba[1]) / 255,
                                       blue: CGFloat(rgba[2]) / 255, alpha: CGFloat(rgba[3]) / 255))
        }
    }
}

/// A place in a row's text: a run and a UTF-16 offset within it.
struct NativeTextPosition: Comparable {
    let run: Int
    let offset: Int

    static func < (a: Self, b: Self) -> Bool { (a.run, a.offset) < (b.run, b.offset) }
}

/// A display list with its CoreText lines, built once per row version.
final class NativeRowModel {
    let display: NativeRowDisplay
    let scrollers: [NativeRowDisplay.Scroller]
    private let lines: [CTLine?]
    /// The scroller each run and rectangle belongs to, or -1 for the row.
    private let runScroller: [Int]
    private let rectScroller: [Int]

    /// `light` picks the syntax palette for code blocks' colors.
    init(_ display: NativeRowDisplay, light: Bool = false) {
        self.display = display
        let texts = display.texts.map { $0 as NSString }
        // Paint-only syntax spans for each code paragraph, when Rust has
        // them; until then (and for unknown languages) code stays plain.
        var spans: [Int: [NativeSyntax.Span]] = [:]
        for block in display.code_blocks ?? [] where block.text < texts.count {
            spans[block.text] = NativeSyntax.shared.spans(language: block.language,
                                                          text: display.texts[block.text], light: light)
        }
        let fromContext = NSAttributedString.Key(kCTForegroundColorFromContextAttributeName as String)
        lines = display.runs.map { run in
            guard run.text < texts.count, run.style < display.styles.count else { return nil }
            let text = texts[run.text]
            let range = NSRange(location: run.start16, length: run.len16)
            guard range.location >= 0, range.length > 0, NSMaxRange(range) <= text.length else { return nil }
            let font = display.styles[run.style].font.uiFont
            let attributes: [NSAttributedString.Key: Any] = [.font: font, fromContext: true]
            let string = NSMutableAttributedString(string: text.substring(with: range), attributes: attributes)
            for span in spans[run.text] ?? [] {
                let overlap = NSIntersectionRange(range, NSRange(location: span.start, length: span.length))
                guard overlap.length > 0 else { continue }
                let local = NSRange(location: overlap.location - range.location, length: overlap.length)
                string.removeAttribute(fromContext, range: local)
                string.addAttribute(.foregroundColor, value: span.color, range: local)
            }
            let line = CTLineCreateWithAttributedString(string)
            guard let limit = run.truncate else { return line }
            let ellipsis = CTLineCreateWithAttributedString(NSAttributedString(string: "\u{2026}", attributes: attributes))
            return CTLineCreateTruncatedLine(line, Double(max(0, limit)), .end, ellipsis) ?? line
        }
        let scrollers = (display.scrollers ?? []).filter {
            $0.runs.count == 2 && $0.rects.count == 2 && $0.runs[0] <= $0.runs[1] && $0.runs[1] <= display.runs.count
                && $0.rects[0] <= $0.rects[1] && $0.rects[1] <= display.rects.count && $0.w > 0 && $0.h > 0
        }
        self.scrollers = scrollers
        var runScroller = [Int](repeating: -1, count: display.runs.count)
        var rectScroller = [Int](repeating: -1, count: display.rects.count)
        for (index, scroller) in scrollers.enumerated() {
            for run in scroller.runs[0]..<scroller.runs[1] { runScroller[run] = index }
            for rect in scroller.rects[0]..<scroller.rects[1] { rectScroller[rect] = index }
        }
        self.runScroller = runScroller
        self.rectScroller = rectScroller
    }

    var hasText: Bool { lines.contains { $0 != nil } }

    /// Paints the items of `scroller` (-1 for the row itself) inside `band`
    /// (row coordinates, unscrolled) into a context whose origin is the
    /// band's top-left corner.
    func draw(_ band: CGRect, in context: CGContext, scale: CGFloat, scroller: Int = -1) {
        context.saveGState()
        context.translateBy(x: -band.minX, y: -band.minY)
        for (index, rect) in display.rects.enumerated()
            where rectScroller[index] == scroller && rect.frame.intersects(band.insetBy(dx: -2, dy: -2)) {
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
        for (index, run) in display.runs.enumerated() where runScroller[index] == scroller {
            guard let line = lines[index], run.style < display.styles.count else { continue }
            let style = display.styles[run.style]
            let size = CGFloat(style.font.size)
            guard run.baseline + size > band.minY, run.baseline - size * 1.2 < band.maxY,
                  run.x < band.maxX, run.x + run.width > band.minX else { continue }
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

    // MARK: Text positions

    /// The scroller a run belongs to, or -1.
    func scroller(ofRun run: Int) -> Int { run >= 0 && run < runScroller.count ? runScroller[run] : -1 }

    private func metrics(_ run: Int) -> (ascent: CGFloat, descent: CGFloat) {
        guard let line = lines[run] else { return (0, 0) }
        var ascent: CGFloat = 0
        var descent: CGFloat = 0
        _ = CTLineGetTypographicBounds(line, &ascent, &descent, nil)
        return (ascent, descent)
    }

    /// A position's caret, unscrolled: x, and the line's top and bottom.
    func caret(_ position: NativeTextPosition) -> (x: CGFloat, top: CGFloat, bottom: CGFloat)? {
        guard position.run >= 0, position.run < lines.count, let line = lines[position.run] else { return nil }
        let run = display.runs[position.run]
        let x = run.x + CTLineGetOffsetForStringIndex(line, min(position.offset, run.len16), nil)
        let (ascent, descent) = metrics(position.run)
        return (x, run.baseline - ascent, run.baseline + descent)
    }

    /// The text position nearest `point` among the runs of `scroller`
    /// (point in unscrolled row coordinates).
    func position(at point: CGPoint, scroller: Int) -> NativeTextPosition? {
        var best: (score: CGFloat, run: Int)?
        for (index, run) in display.runs.enumerated() where runScroller[index] == scroller && lines[index] != nil {
            let (ascent, descent) = metrics(index)
            let top = run.baseline - ascent
            let bottom = run.baseline + descent
            let dy = point.y < top ? top - point.y : (point.y > bottom ? point.y - bottom : 0)
            let dx = point.x < run.x ? run.x - point.x : (point.x > run.x + run.width ? point.x - run.x - run.width : 0)
            let score = dy * 1_000 + dx
            if best == nil || score < best!.score { best = (score, index) }
        }
        guard let index = best?.run, let line = lines[index] else { return nil }
        let run = display.runs[index]
        let offset = CTLineGetStringIndexForPosition(line, CGPoint(x: point.x - run.x, y: 0))
        return NativeTextPosition(run: index, offset: max(0, min(run.len16, offset == kCFNotFound ? 0 : offset)))
    }

    /// The first and last positions of the row's text.
    var textBounds: (NativeTextPosition, NativeTextPosition)? {
        guard let first = lines.firstIndex(where: { $0 != nil }),
              let last = lines.lastIndex(where: { $0 != nil }) else { return nil }
        return (NativeTextPosition(run: first, offset: 0),
                NativeTextPosition(run: last, offset: display.runs[last].len16))
    }

    /// The selection's highlight rectangles by scroller, unscrolled.
    func highlights(_ start: NativeTextPosition, _ end: NativeTextPosition) -> [Int: [CGRect]] {
        var out: [Int: [CGRect]] = [:]
        guard start < end else { return out }
        for index in start.run...min(end.run, lines.count - 1) {
            guard let line = lines[index] else { continue }
            let run = display.runs[index]
            let a = index == start.run ? start.offset : 0
            let b = index == end.run ? end.offset : run.len16
            guard b > a else { continue }
            let x0 = CTLineGetOffsetForStringIndex(line, a, nil)
            let x1 = CTLineGetOffsetForStringIndex(line, b, nil)
            let (ascent, descent) = metrics(index)
            out[runScroller[index], default: []].append(
                CGRect(x: run.x + x0, y: run.baseline - ascent, width: max(1, x1 - x0), height: ascent + descent))
        }
        return out
    }

    /// The selected text. Pieces of one paragraph join with the text between
    /// them; separate paragraphs join with a tab on one baseline, else a line
    /// break.
    func text(_ start: NativeTextPosition, _ end: NativeTextPosition) -> String {
        guard start < end else { return "" }
        let texts = display.texts.map { $0 as NSString }
        var out = ""
        var previous: (text: Int, end: Int, baseline: CGFloat)?
        for index in start.run...min(end.run, display.runs.count - 1) where lines[index] != nil {
            let run = display.runs[index]
            guard run.text < texts.count else { continue }
            let a = index == start.run ? start.offset : 0
            let b = index == end.run ? end.offset : run.len16
            guard b > a else { continue }
            let text = texts[run.text]
            if let previous {
                if previous.text == run.text, previous.end <= run.start16 + a {
                    out += text.substring(with: NSRange(location: previous.end, length: run.start16 + a - previous.end))
                } else {
                    out += abs(previous.baseline - run.baseline) < 0.5 ? "\t" : "\n"
                }
            }
            let range = NSRange(location: run.start16 + a, length: b - a)
            guard NSMaxRange(range) <= text.length else { continue }
            out += text.substring(with: range)
            previous = (run.text, run.start16 + b, run.baseline)
        }
        return out
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

/// One tile of a row's painting, or of a scroller's content. Tall rows and
/// wide scrollers paint in tiles so no backing store grows with a long
/// message.
private final class NativeRowStripe: UIView {
    var model: NativeRowModel?
    /// The painted region, in unscrolled row coordinates.
    var band: CGRect = .zero
    /// Which scroller's items this tile paints; -1 is the row itself.
    var scroller = -1

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
            model.draw(band, in: context, scale: traitCollection.displayScale, scroller: scroller)
        }
    }
}

/// A code block or table wider than its row: its items scroll sideways and
/// paint in tiles.
private final class NativeScrollerView: UIScrollView, UIScrollViewDelegate {
    static let tile: CGFloat = 512

    let index: Int
    private(set) var model: NativeRowModel?
    private var tiles: [Int: NativeRowStripe] = [:]
    /// The row's visible band, in row coordinates.
    private var visibleBand: CGRect = .zero
    let highlight = CAShapeLayer()
    var scrolled: (() -> Void)?

    init(index: Int) {
        self.index = index
        super.init(frame: .zero)
        delegate = self
        backgroundColor = .clear
        showsVerticalScrollIndicator = false
        alwaysBounceVertical = false
        isDirectionalLockEnabled = true
        scrollsToTop = false
        delaysContentTouches = false
        highlight.fillColor = UIColor.systemBlue.withAlphaComponent(0.3).cgColor
        layer.addSublayer(highlight)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    func configure(_ model: NativeRowModel, scroller: NativeRowDisplay.Scroller) {
        if frame != scroller.frame { frame = scroller.frame }
        let size = CGSize(width: max(scroller.w, scroller.content_w), height: scroller.h)
        if contentSize != size {
            contentSize = size
            if contentOffset.x > size.width - scroller.w { contentOffset.x = max(0, size.width - scroller.w) }
        }
        if self.model !== model {
            self.model = model
            for tile in tiles.values {
                tile.model = model
                tile.setNeedsDisplay()
            }
        }
        retile()
    }

    func show(_ rowVisible: CGRect) {
        visibleBand = rowVisible
        retile()
    }

    func repaint() { tiles.values.forEach { $0.setNeedsDisplay() } }

    private func retile() {
        guard let model else { return }
        let content = CGRect(origin: .zero, size: contentSize)
        let visible = CGRect(x: contentOffset.x, y: visibleBand.minY - frame.minY,
                             width: bounds.width, height: visibleBand.height).intersection(content)
        var needed = Set<Int>()
        if !visible.isNull, !visible.isEmpty {
            let columns = Int(floor(visible.minX / Self.tile))...Int(floor(max(visible.minX, visible.maxX - 0.01) / Self.tile))
            let rows = Int(floor(visible.minY / Self.tile))...Int(floor(max(visible.minY, visible.maxY - 0.01) / Self.tile))
            for column in columns { for row in rows { needed.insert(column * 4_096 + row) } }
        }
        for (key, tile) in tiles where !needed.contains(key) {
            tile.removeFromSuperview()
            tiles[key] = nil
        }
        for key in needed {
            let rect = CGRect(x: CGFloat(key / 4_096) * Self.tile, y: CGFloat(key % 4_096) * Self.tile,
                              width: Self.tile, height: Self.tile).intersection(content)
            guard !rect.isEmpty else { continue }
            let tile = tiles[key] ?? {
                let tile = NativeRowStripe(frame: rect)
                tile.scroller = index
                insertSubview(tile, at: 0)
                tiles[key] = tile
                return tile
            }()
            let band = rect.offsetBy(dx: frame.minX, dy: frame.minY)
            if tile.frame != rect || tile.band != band || tile.model !== model {
                tile.frame = rect
                tile.band = band
                tile.model = model
                tile.setNeedsDisplay()
            }
        }
    }

    func scrollViewDidScroll(_ scrollView: UIScrollView) {
        retile()
        scrolled?()
    }
}

/// A selection handle: a bar the height of the line with a knob above the
/// start or below the end.
private final class NativeSelectionHandle: UIView {
    let start: Bool
    private let bar = UIView()
    private let knob = UIView()

    init(start: Bool) {
        self.start = start
        super.init(frame: .zero)
        bar.backgroundColor = .systemBlue
        knob.backgroundColor = .systemBlue
        knob.layer.cornerRadius = 5
        addSubview(bar)
        addSubview(knob)
        isAccessibilityElement = false
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    /// Places the handle at a caret, in its superview's coordinates.
    func place(x: CGFloat, top: CGFloat, bottom: CGFloat) {
        // A generous touch target around a two-point bar.
        frame = CGRect(x: x - 16, y: top - 12, width: 32, height: bottom - top + 24)
        bar.frame = CGRect(x: 15, y: 12, width: 2, height: bottom - top)
        knob.frame = CGRect(x: 11, y: start ? 2 : bottom - top + 12, width: 10, height: 10)
    }
}

/// A painted transcript row with its native widgets.
@MainActor
final class NativeRowView: UIView, UIContextMenuInteractionDelegate, UIEditMenuInteractionDelegate {
    static let stripeHeight: CGFloat = 512

    private(set) var key = ""
    private(set) var version: UInt64 = 0
    /// The layout geometry the row was painted for; see `NativeTranscriptView`.
    private(set) var epoch = 0
    private(set) var model: NativeRowModel?
    private var stripes: [Int: NativeRowStripe] = [:]
    private var scrollers: [NativeScrollerView] = []
    private var widgetViews: [UIView] = []
    private var visibleBand: CGRect = .zero
    var toggle: ((String) -> Void)?
    var loadEarlier: (() -> Void)?
    /// Draws a `surface` widget (a resource and its label) the application
    /// registered, such as a link card; nil leaves the box empty.
    var surface: ((String, String) -> UIView?)?
    /// Called when this row starts a selection, so others clear theirs.
    var selecting: ((NativeRowView) -> Void)?
    /// Give feedback on selected text (#10127): the text and this row's
    /// key. Set by an app that files feedback; nil leaves the item out.
    static var giveFeedback: ((_ text: String, _ row: String) -> Void)?

    private var selection: (start: NativeTextPosition, end: NativeTextPosition)?
    /// The selection's highlight, above the painted stripes.
    private let highlightView = UIView()
    private let highlight = CAShapeLayer()
    private let startHandle = NativeSelectionHandle(start: true)
    private let endHandle = NativeSelectionHandle(start: false)
    private lazy var editMenu = UIEditMenuInteraction(delegate: self)

    override init(frame: CGRect) {
        super.init(frame: frame)
        backgroundColor = .clear
        isAccessibilityElement = true
        addInteraction(UIContextMenuInteraction(delegate: self))
        addInteraction(editMenu)
        highlight.fillColor = UIColor.systemBlue.withAlphaComponent(0.3).cgColor
        highlightView.isUserInteractionEnabled = false
        highlightView.layer.addSublayer(highlight)
        addSubview(highlightView)
        for handle in [startHandle, endHandle] {
            handle.isHidden = true
            handle.addGestureRecognizer(UIPanGestureRecognizer(target: self, action: #selector(dragHandle(_:))))
            addSubview(handle)
        }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    /// Paint the row again from a new model at its next tile, as when code
    /// colors arrive.
    func forget() { version = 0 }

    func apply(_ model: NativeRowModel, version: UInt64, epoch: Int) {
        clearSelection()
        self.model = model
        self.version = version
        self.epoch = epoch
        key = model.display.key
        for stripe in stripes.values {
            stripe.model = model
            stripe.setNeedsDisplay()
        }
        rebuildScrollers()
        rebuildWidgets()
        let access = model.display.accessibility
        accessibilityLabel = access.label
        accessibilityValue = access.value
        accessibilityHint = access.hint
        accessibilityTraits = access.button == true ? .button : .staticText
        // A row with a drawn surface lets the surface speak for itself.
        isAccessibilityElement = !widgetViews.contains { $0 is NativeHostedSurface }
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
        clearSelection()
        model = nil
        version = 0
        epoch = 0
        key = ""
        toggle = nil
        loadEarlier = nil
        surface = nil
        selecting = nil
        isAccessibilityElement = true
        stripes.values.forEach { $0.removeFromSuperview() }
        stripes.removeAll()
        scrollers.forEach { $0.removeFromSuperview() }
        scrollers.removeAll()
        widgetViews.forEach { $0.removeFromSuperview() }
        widgetViews.removeAll()
        accessibilityLabel = nil
        accessibilityCustomActions = nil
        layer.removeAnimation(forKey: "stream")
    }

    /// Materializes the stripes that intersect `visible` (row coordinates).
    func show(_ visible: CGRect) {
        guard let model else { return }
        visibleBand = visible
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
        scrollers.forEach { $0.show(visible) }
    }

    func repaint() {
        stripes.values.forEach { $0.setNeedsDisplay() }
        scrollers.forEach { $0.repaint() }
    }

    private func rebuildScrollers() {
        guard let model else { return }
        // Keep existing scrollers, and their offsets, when the row streams.
        while scrollers.count > model.scrollers.count { scrollers.removeLast().removeFromSuperview() }
        for (index, scroller) in model.scrollers.enumerated() {
            let view: NativeScrollerView
            if index < scrollers.count {
                view = scrollers[index]
            } else {
                view = NativeScrollerView(index: index)
                view.scrolled = { [weak self] in self?.layoutSelection() }
                addSubview(view)
                scrollers.append(view)
            }
            view.configure(model, scroller: scroller)
            view.show(visibleBand)
        }
        // Scrollers sit above the stripes and below widgets and handles.
        for view in scrollers { bringSubviewToFront(view) }
    }

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
        bringSubviewToFront(startHandle)
        bringSubviewToFront(endHandle)
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
        case "surface":
            guard let resource = widget.resource, let drawn = surface?(resource, widget.label ?? "") else {
                return nil
            }
            return drawn
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

    // A long press on a message offers Copy and Select Text. Select Text
    // selects the painted text in place, with handles to adjust it.
    func contextMenuInteraction(_ interaction: UIContextMenuInteraction,
                                configurationForMenuAtLocation location: CGPoint) -> UIContextMenuConfiguration? {
        guard let copy = model?.display.copy, !copy.isEmpty else { return nil }
        return UIContextMenuConfiguration(identifier: nil, previewProvider: nil) { [weak self] _ in
            UIMenu(children: [
                UIAction(title: "Copy", image: UIImage(systemName: "doc.on.doc")) { _ in
                    UIPasteboard.general.string = copy
                },
                UIAction(title: "Select Text", image: UIImage(systemName: "selection.pin.in.out")) { _ in
                    // Let the menu finish dismissing before the edit menu shows.
                    DispatchQueue.main.async { self?.selectAll(nil) }
                },
            ] + (self?.feedbackAction(copy).map { [$0] } ?? []))
        }
    }

    // MARK: Selection

    var hasSelection: Bool { selection != nil }

    override var canBecomeFirstResponder: Bool { selection != nil }

    override func canPerformAction(_ action: Selector, withSender sender: Any?) -> Bool {
        switch action {
        case #selector(copy(_:)): return selection != nil
        case #selector(selectAll(_:)): return model?.hasText == true
        default: return false
        }
    }

    override func copy(_ sender: Any?) {
        guard let model, let selection else { return }
        UIPasteboard.general.string = model.text(selection.start, selection.end)
    }

    override func selectAll(_ sender: Any?) {
        guard let bounds = model?.textBounds else { return }
        select(bounds.0, bounds.1)
    }

    private func select(_ start: NativeTextPosition, _ end: NativeTextPosition) {
        selection = start <= end ? (start, end) : (end, start)
        selecting?(self)
        layoutSelection()
        becomeFirstResponder()
        showMenu()
    }

    func clearSelection() {
        guard selection != nil else { return }
        selection = nil
        editMenu.dismissMenu()
        layoutSelection()
        if isFirstResponder { resignFirstResponder() }
    }

    private func scrollOffset(_ scroller: Int) -> CGFloat {
        scroller >= 0 && scroller < scrollers.count ? scrollers[scroller].contentOffset.x : 0
    }

    /// Draws the highlight and places the handles for the current selection.
    private func layoutSelection() {
        guard let model, let selection else {
            highlight.path = nil
            scrollers.forEach { $0.highlight.path = nil }
            startHandle.isHidden = true
            endHandle.isHidden = true
            return
        }
        let rects = model.highlights(selection.start, selection.end)
        let path = CGMutablePath()
        rects[-1]?.forEach { path.addRect($0) }
        highlightView.frame = bounds
        highlight.path = path
        for (index, scroller) in scrollers.enumerated() {
            let path = CGMutablePath()
            // Scroller content coordinates start at the scroller's origin.
            let origin = scroller.frame.origin
            rects[index]?.forEach { path.addRect($0.offsetBy(dx: -origin.x, dy: -origin.y)) }
            scroller.highlight.path = path
        }
        for (handle, position) in [(startHandle, selection.start), (endHandle, selection.end)] {
            guard let caret = model.caret(position) else { handle.isHidden = true; continue }
            let scroller = model.scroller(ofRun: position.run)
            var x = caret.x - scrollOffset(scroller)
            if scroller >= 0, scroller < scrollers.count {
                let frame = scrollers[scroller].frame
                x = min(max(x, frame.minX), frame.maxX)
            }
            handle.place(x: x, top: caret.top, bottom: caret.bottom)
            handle.isHidden = false
            bringSubviewToFront(handle)
        }
    }

    /// The text position under a point in this view.
    private func position(at point: CGPoint) -> NativeTextPosition? {
        guard let model else { return nil }
        for (index, scroller) in scrollers.enumerated() where scroller.frame.contains(point) {
            if let found = model.position(at: CGPoint(x: point.x + scroller.contentOffset.x, y: point.y),
                                          scroller: index) {
                return found
            }
        }
        return model.position(at: point, scroller: -1)
    }

    @objc private func dragHandle(_ gesture: UIPanGestureRecognizer) {
        guard let handle = gesture.view as? NativeSelectionHandle, let selection else { return }
        switch gesture.state {
        case .began:
            editMenu.dismissMenu()
        case .changed:
            // Aim at the middle of the line, not the knob.
            var point = gesture.location(in: self)
            point.y += handle.start ? 8 : -8
            guard let position = position(at: point) else { return }
            if handle.start {
                self.selection = position <= selection.end ? (position, selection.end) : (selection.end, position)
            } else {
                self.selection = position >= selection.start ? (selection.start, position) : (position, selection.start)
            }
            layoutSelection()
        case .ended, .cancelled:
            showMenu()
        default:
            break
        }
    }

    private func showMenu() {
        guard let model, let selection, let caret = model.caret(selection.start) else { return }
        let point = CGPoint(x: min(max(caret.x - scrollOffset(model.scroller(ofRun: selection.start.run)), 0), bounds.width),
                            y: caret.top)
        editMenu.presentEditMenu(with: UIEditMenuConfiguration(identifier: nil, sourcePoint: point))
    }

    func editMenuInteraction(_ interaction: UIEditMenuInteraction, menuFor configuration: UIEditMenuConfiguration,
                             suggestedActions: [UIMenuElement]) -> UIMenu? {
        guard let model, let selection,
              let feedback = feedbackAction(model.text(selection.start, selection.end))
        else { return UIMenu(children: suggestedActions) }
        return UIMenu(children: suggestedActions + [feedback])
    }

    /// Give feedback on `text`, when this app files feedback.
    private func feedbackAction(_ text: String) -> UIAction? {
        guard let giveFeedback = Self.giveFeedback,
              !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return nil }
        let row = key
        return UIAction(title: "Give feedback", image: UIImage(systemName: "flag")) { _ in
            giveFeedback(text, row)
        }
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
            attributes.font = UIFont.paper(.caption1)
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
final class NativeTranscriptView: UIScrollView, UIScrollViewDelegate, UIGestureRecognizerDelegate {
    /// Matches `rust_native::layout::EARLIER_KEY`.
    static let earlierKey = "\u{1}earlier"
    /// How far beyond the screen rows are prepared.
    private static let overscan: CGFloat = 700
    /// Coming to rest this close to the bottom resumes following.
    private static let followBand: CGFloat = 70
    /// How long a streamed row's new text takes to fade in.
    private static let fade: CFTimeInterval = 0.18
    /// Nominal sizes the layout uses, with the text style whose Dynamic Type
    /// curve scales each.
    private static let textStyles: [(CGFloat, UIFont.TextStyle)] = [
        (12, .caption1), (13, .footnote), (15, .subheadline), (16, .callout), (17, .body), (19, .title3),
        (22, .title2),
    ]

    var transcriptKey = ""
    var activate: ((String) -> Void)?
    /// Draws a row's `surface` widget; see `NativeRowView.surface`.
    var surface: ((String, String) -> UIView?)? {
        didSet { if (surface == nil) != (oldValue == nil) { rowViews.values.forEach { $0.forget() }; tile() } }
    }
    /// Room kept clear below the last row, as for a composer floating over
    /// the transcript; the scroll-to-bottom button sits above it.
    var bottomInset: CGFloat = 0 {
        didSet {
            guard abs(bottomInset - oldValue) > 0.5 else { return }
            contentInset.bottom = bottomInset
            verticalScrollIndicatorInsets.bottom = bottomInset
            if following, !isInteracting { pin() }
            updateBottomButton()
        }
    }
    /// Whether the newest row stays in view as rows arrive and grow.
    var following = true {
        didSet { if following != oldValue { updateBottomButton() } }
    }

    private let layout: NativeTranscriptLayout?
    /// The frame on screen. It changes only when an update finishes.
    private var current: NativeTranscriptFrame?
    /// The geometry the frame on screen was laid out for; it changes with the
    /// width and the text size, not with content.
    private var frameEpoch = 0
    private var epoch = 0
    private var order: [String] = []
    private var nodes: [String: NativeNode] = [:]
    private var earlier: NativeEarlier?
    /// The transcript source Rust reads rows from, when the application
    /// keeps them in Rust; `order` and `nodes` are then empty.
    private var source: String?
    private var sourceRevision: UInt64 = 0
    /// Rows whose current content Rust has not received.
    private var unsent: Set<String> = []
    // What Rust has received, or will have once the queued update runs.
    private var sentWidth: CGFloat = 0
    private var sentScale: CGFloat = 0
    private var sentCurve: [[Float]] = []
    private var sentExpanded: Set<String> = []
    private var sentOrder: [String] = []
    private var needsSync = true
    /// One update runs at a time; changes that arrive meanwhile go in the next.
    private var inFlight = false
    private var pending = false
    private var rowViews: [String: NativeRowView] = [:]
    private var pool: [NativeRowView] = []
    private var models: [String: (version: UInt64, model: NativeRowModel)] = [:]
    private weak var selectedRow: NativeRowView?
    private var expansion: AnyCancellable?
    /// Ends a selection on a tap. It is the only recognizer the delegate
    /// methods below decide for: a scroll view is also its own pan
    /// recognizer's delegate, so answering for every recognizer would keep
    /// touches from the pan and leave the transcript unscrollable.
    private let selectionTap = UITapGestureRecognizer()
    private let bottomButton = UIButton(type: .system)
    private var buttonShown = false
    private let fallback = UILabel()
    fileprivate(set) var stats = NativeTranscriptStats()
    #if DEBUG || targetEnvironment(simulator) || RUST_NATIVE_BENCH
    private var bench: NativeTranscriptBench?
    private var prependChecked = false
    private var selectedOnce = false
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
        // A tap anywhere ends a selection.
        selectionTap.addTarget(self, action: #selector(tapped))
        selectionTap.cancelsTouchesInView = false
        selectionTap.delegate = self
        addGestureRecognizer(selectionTap)
        // Text size changes scale every row; appearance changes only repaint.
        registerForTraitChanges([UITraitPreferredContentSizeCategory.self]) { (self: Self, _) in
            self.setNeedsLayout()
        }
        registerForTraitChanges([UITraitUserInterfaceStyle.self]) { (self: Self, _) in
            self.rowViews.values.forEach { $0.repaint() }
            self.syntaxReady()
        }
        // Code colors arrive after the rows show: paint those rows again.
        NotificationCenter.default.addObserver(self, selector: #selector(syntaxReady),
                                               name: NativeSyntax.ready, object: nil)
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

    /// Dynamic Type's size for each nominal size, from its own text style.
    private var textCurve: [[Float]] {
        Self.textStyles.map { size, style in
            let scaled = UIFontMetrics(forTextStyle: style).scaledValue(for: size, compatibleWith: traitCollection)
            return [Float(size), Float((scaled * 100).rounded() / 100)]
        }
    }

    /// Takes a new revision whose rows Rust holds in the source `source`.
    /// Nothing is encoded here: the update names the source, and Rust lays
    /// out the rows whose content changed since its last read.
    func apply(source: String, revision: UInt64, earlier: NativeEarlier?) {
        if source != self.source || revision != sourceRevision || earlier != self.earlier {
            needsSync = true
        }
        if self.source == nil {
            nodes = [:]
            order = []
            unsent = []
            sentOrder = []
        }
        self.source = source
        sourceRevision = revision
        self.earlier = earlier
        sync()
    }

    /// Takes a new revision's rows. Only rows whose content changed go to Rust.
    func apply(rows: [NativeNode], earlier: NativeEarlier?) {
        if source != nil {
            source = nil
            sentOrder = []
        }
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
        order = newOrder
        if changed { needsSync = true }
        sync()
    }

    private func expansionChanged() {
        guard NativeExpansion.shared.keys != sentExpanded else { return }
        needsSync = true
        sync()
    }

    /// Sends Rust what changed. Layout runs on the worker queue; the new
    /// frame replaces the one on screen when it is ready.
    private func sync() {
        guard let layout, bounds.width > 0 else { return }
        let scale = textScale
        let curve = textCurve
        let expanded = NativeExpansion.shared.keys
        guard needsSync || bounds.width != sentWidth || scale != sentScale || curve != sentCurve
                || expanded != sentExpanded else { return }
        guard !inFlight else {
            pending = true
            return
        }
        let rows = unsent.compactMap { nodes[$0] }
        let request: NativeLayoutUpdate
        if let source {
            request = NativeLayoutUpdate(width: Float(bounds.width), scale: Float(scale), order: nil, rows: [],
                                         expanded: Array(expanded), earlier: nil, curve: curve, source: source)
        } else {
            request = NativeLayoutUpdate(
                width: Float(bounds.width), scale: Float(scale),
                // A streamed token keeps the order; Rust then updates in place.
                order: order == sentOrder && !sentOrder.isEmpty ? nil : order, rows: rows,
                expanded: Array(expanded.intersection(nodes.keys)),
                earlier: earlier.map { NativeLayoutUpdate.Earlier(label: $0.label, loading: $0.loading) },
                curve: curve)
        }
        if bounds.width != sentWidth || scale != sentScale || curve != sentCurve { epoch += 1 }
        let requestEpoch = epoch
        unsent.removeAll()
        needsSync = false
        sentWidth = bounds.width
        sentScale = scale
        sentCurve = curve
        sentExpanded = expanded
        sentOrder = order
        inFlight = true
        layout.update(request) { [weak self] result in
            guard let self else { return }
            self.inFlight = false
            if let summary = result.summary, let frame = result.frame {
                self.present(frame, epoch: requestEpoch)
                self.stats.record(update: summary, encode: result.encode, total: result.total, rows: rows.count)
                #if DEBUG || targetEnvironment(simulator) || RUST_NATIVE_BENCH
                if self.bench == nil, summary.count > 0, NativeTranscriptBench.requested {
                    self.bench = NativeTranscriptBench(view: self)
                }
                if !self.prependChecked, summary.count > 0, NativeTranscriptBench.prependRequested {
                    self.prependChecked = true
                    NativeTranscriptBench.checkPrepend(self)
                }
                if !self.prependChecked, summary.count > 0, let y = NativeTranscriptBench.startOffset {
                    // Start the reader at a fixed offset, for screenshots.
                    self.prependChecked = true
                    self.following = false
                    self.contentOffset = CGPoint(x: 0, y: min(y, self.bottomOffset))
                }
                if !self.selectedOnce, let key = NativeTranscriptBench.selectKey {
                    self.selectedOnce = true
                    // Select a row's text in place, for screenshots.
                    DispatchQueue.main.asyncAfter(deadline: .now() + 1) { [weak self] in
                        self?.rowViews[key]?.selectAll(nil)
                    }
                }
                #endif
            } else {
                // Refused: send every row with its order next time. The next
                // revision retries; a refusal does not loop.
                self.unsent = Set(self.nodes.keys)
                self.sentOrder = []
            }
            if self.pending {
                self.pending = false
                self.sync()
            }
        }
    }

    /// Puts a finished frame on screen, keeping the reader's place.
    private func present(_ frame: NativeTranscriptFrame, epoch: Int) {
        let anchor = following ? nil : visibleAnchor()
        current = frame
        frameEpoch = epoch
        let height = frame.height
        if contentSize.height != height || contentSize.width != bounds.width {
            contentSize = CGSize(width: bounds.width, height: height)
        }
        if following {
            pin()
        } else if let anchor, let placement = frame.find(anchor.key) {
            // Shift the bounds, not the content offset, so a fling keeps its
            // momentum while rows arrive above.
            let top = -adjustedContentInset.top
            let target = min(bottomOffset, max(top, CGFloat(placement.y) - anchor.offset))
            if abs(target - bounds.origin.y) > 0.25 { bounds.origin.y = target }
        }
        tile()
        updateBottomButton()
    }

    private struct Anchor {
        let key: String
        let offset: CGFloat
    }

    private func visibleAnchor() -> Anchor? {
        guard let frame = current else { return nil }
        let top = contentOffset.y
        for placement in frame.rows(top, top + bounds.height) {
            guard let key = frame.key(placement.index), key != Self.earlierKey else { continue }
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
    @objc private func syntaxReady() {
        models.removeAll(keepingCapacity: true)
        rowViews.values.forEach { $0.forget() }
        tile()
    }

    private func tile() {
        guard let frame = current else { return }
        let started = CACurrentMediaTime()
        let visible = CGRect(x: 0, y: contentOffset.y, width: bounds.width, height: bounds.height)
        let placements = frame.rows(visible.minY - Self.overscan, visible.maxY + Self.overscan)
        var keep = Set<String>()
        for placement in placements {
            guard let key = frame.key(placement.index) else { continue }
            keep.insert(key)
            let view = rowViews[key] ?? dequeue(key)
            view.surface = surface
            if view.version != placement.version || view.key != key {
                guard let model = model(for: key, in: frame, index: placement.index, version: placement.version) else {
                    continue
                }
                // New text in a row that keeps its geometry, such as a
                // streamed reply, fades in; the unchanged text does not move.
                if view.key == key, view.version != 0, view.epoch == frameEpoch {
                    let transition = CATransition()
                    transition.type = .fade
                    transition.duration = Self.fade
                    view.layer.add(transition, forKey: "stream")
                }
                view.apply(model, version: placement.version, epoch: frameEpoch)
                stats.painted += 1
            }
            let rowFrame = CGRect(x: 0, y: CGFloat(placement.y), width: bounds.width, height: CGFloat(placement.height))
            if view.frame != rowFrame { view.frame = rowFrame }
            view.toggle = { key in NativeExpansion.shared.toggle(key) }
            view.loadEarlier = { [weak self] in self?.loadEarlier() }
            view.selecting = { [weak self] row in self?.selected(row) }
            view.show(visible.offsetBy(dx: 0, dy: -rowFrame.minY).insetBy(dx: 0, dy: -Self.overscan / 2))
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

    private func selected(_ row: NativeRowView) {
        if selectedRow !== row { selectedRow?.clearSelection() }
        selectedRow = row
    }

    @objc private func tapped(_ gesture: UITapGestureRecognizer) {
        selectedRow?.clearSelection()
        selectedRow = nil
    }

    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
        // The scroll view's own pan always gets its touches.
        guard gestureRecognizer === selectionTap else { return true }
        // Only a selection needs the tap; its handles keep their drags.
        return selectedRow?.hasSelection == true && !(touch.view is NativeSelectionHandle)
    }

    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer,
                           shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer) -> Bool {
        gestureRecognizer === selectionTap
    }

    #if DEBUG || targetEnvironment(simulator) || RUST_NATIVE_BENCH
    /// The first visible row's key and its distance from the top of the
    /// screen, for the scripted prepend check.
    func debugAnchor() -> (key: String, offset: CGFloat)? {
        visibleAnchor().map { ($0.key, $0.offset) }
    }

    /// Where a row's top sits on screen now.
    func debugScreenOffset(of key: String) -> CGFloat? {
        current?.find(key).map { CGFloat($0.y) - contentOffset.y }
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

    private func model(for key: String, in frame: NativeTranscriptFrame, index: UInt32,
                       version: UInt64) -> NativeRowModel? {
        if let cached = models[key], cached.version == version { return cached.model }
        guard let display = frame.display(index) else { return nil }
        let model = NativeRowModel(display, light: traitCollection.userInterfaceStyle == .light)
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
        sync()
        tile()
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

#if DEBUG || targetEnvironment(simulator) || RUST_NATIVE_BENCH
/// A scripted fling benchmark: after the transcript settles, scroll up for
/// four seconds and back down at a fixed speed on the display link, then
/// print frame times. A
/// hitch is a frame later than 1.5 times its budget.
@MainActor
final class NativeTranscriptBench {
    static var requested: Bool {
        ProcessInfo.processInfo.arguments.contains("--rust-native-transcript-bench")
    }

    /// `--rust-native-transcript-offset Y` starts the reader at Y points.
    static var startOffset: CGFloat? {
        let arguments = ProcessInfo.processInfo.arguments
        guard let index = arguments.firstIndex(of: "--rust-native-transcript-offset"), index + 1 < arguments.count,
              let y = Double(arguments[index + 1]) else { return nil }
        return CGFloat(max(0, y))
    }

    /// `--rust-native-transcript-select KEY` selects that row's text.
    static var selectKey: String? {
        let arguments = ProcessInfo.processInfo.arguments
        guard let index = arguments.firstIndex(of: "--rust-native-transcript-select"), index + 1 < arguments.count
        else { return nil }
        return arguments[index + 1]
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
        #if DEBUG || targetEnvironment(simulator) || RUST_NATIVE_BENCH
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
struct NativeLayoutUpdate: Encodable {
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
    /// Dynamic Type's `[nominal, scaled]` size for each text style.
    let curve: [[Float]]
    /// A published transcript source to read rows from, instead of `order`,
    /// `rows`, and `earlier`.
    var source: String? = nil

    private enum Keys: String, CodingKey { case width, scale, order, rows, expanded, earlier, curve, source }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: Keys.self)
        try container.encode(width, forKey: .width)
        try container.encode(scale, forKey: .scale)
        if let order { try container.encode(order, forKey: .order) }
        try container.encode(rows, forKey: .rows)
        try container.encode(expanded, forKey: .expanded)
        if let earlier { try container.encode(earlier, forKey: .earlier) } else { try container.encodeNil(forKey: .earlier) }
        try container.encode(curve, forKey: .curve)
        if let source { try container.encode(source, forKey: .source) }
    }
}

/// A SwiftUI surface the application draws inside a painted row, at the
/// box the layout reserved for it.
final class NativeHostedSurface: UIView {
    private let host: UIHostingController<AnyView>

    init(_ content: AnyView) {
        host = UIHostingController(rootView: content)
        super.init(frame: .zero)
        backgroundColor = .clear
        clipsToBounds = true
        host.view.backgroundColor = .clear
        host.sizingOptions = []
        addSubview(host.view)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    override func layoutSubviews() {
        super.layoutSubviews()
        host.view.frame = bounds
    }
}

/// The SwiftUI mount for a transcript node.
struct NativeTranscript: UIViewRepresentable {
    let key: String
    let label: String
    let rows: [NativeNode]
    let earlier: NativeEarlier?
    /// The transcript source Rust publishes the rows to, when the view
    /// carries none.
    let source: String?
    let revision: UInt64
    let surface: NativeChat.Surface
    let submit: NativeChat.Submit
    let activate: (String) -> Void

    @Environment(\.nativeTranscriptBottomInset) private var bottomInset

    func makeUIView(context: Context) -> NativeTranscriptView {
        let view = NativeTranscriptView(frame: .zero)
        view.accessibilityIdentifier = key
        return view
    }

    func updateUIView(_ view: NativeTranscriptView, context: Context) {
        view.transcriptKey = key
        view.accessibilityLabel = label
        view.activate = activate
        view.bottomInset = bottomInset
        if let surface {
            view.surface = { resource, label in NativeHostedSurface(surface(resource, label)) }
        } else {
            view.surface = nil
        }
        if let source {
            view.apply(source: source, revision: revision, earlier: earlier)
        } else {
            view.apply(rows: rows, earlier: earlier)
        }
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
        case token, placeholder, max_bytes, busy, stop, intent, loading, source
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
        case let .button(label, enabled, _):
            var p = try props("button")
            try p.encode(label, forKey: .label)
            try p.encode(enabled, forKey: .enabled)
            try p.encodeNil(forKey: .intent)
        case let .surface(resource, label):
            var p = try props("surface")
            try p.encode(resource, forKey: .resource)
            try p.encode(label, forKey: .label)
        case let .transcript(label, children, earlier, source):
            var p = try props("transcript")
            try p.encode(label, forKey: .label)
            try p.encode(children, forKey: .children)
            if let source { try p.encode(source, forKey: .source) }
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
