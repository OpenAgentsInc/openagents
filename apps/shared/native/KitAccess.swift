// Firm reason: 6 (UIKit accessibility: VoiceOver reads platform elements, never a GPU surface).
// The one accessibility bridge for GPU kit surfaces on iOS (components/gpu-components.md,
// "Accessibility"; GitHub #13645). A surface's Coax commits its semantic rows (core/elements.coax,
// paint/surface.coax surface_access_row and surface_say); this file turns them into VoiceOver's
// elements and nothing else. It decides no label, no state and no order of its own beyond reading
// order: what each row says, whether it can be pressed and what a press does stay in Coax.
//
//   - one element per row that shows, read top to bottom and then left to right; a row none of
//     whose box its clips leave is not read;
//   - a role's traits (button, header, link, image, toggle), its state's
//     (not enabled, selected, expanded or collapsed, busy) and its value;
//   - activation is the surface's own press by row ID; a field's activation gives it the keyboard;
//   - an element keeps its object while its row's ID stays, so VoiceOver's cursor stays on it;
//   - a new layer (a sheet, a sidebar, a card) moves the cursor to it (screenChanged);
//   - a live row (EL_S_LIVE) that is new or says more is announced once it has been still for a
//     moment and is not busy, so a reply being written is read when it is done, and only what it
//     added; rows already there when the bridge first reads, or several arriving at once (a
//     conversation loaded), are not announced;
//   - VoiceOver's three-finger scroll and its escape gesture go to the host's handlers.
//
// The host gives it the rows (KitAccessSource) and may stand its own views for some rows: the
// platform editor for a field that holds the keyboard, a system control (KitAccess.stand).
import UIKit

/// A committed semantic row, in the container view's points.
struct KitRow {
    var id: String
    var label: String
    var value: String
    var role: Int
    var state: Int
    var parent: Int
    var focus: Int
    var frame: CGRect
}

/// The roles and state bits of components/core/elements.coax, as the bridge reads them.
enum KitRole {
    static let button = 0, row = 1, tab = 2, field = 3, region = 4, item = 5, toggle = 6, slider = 7
    static let text = 8, heading = 9, link = 10, image = 11, progress = 12, notice = 13
}
enum KitState {
    static let disabled = 1, busy = 2, pressed = 4, expanded = 8, current = 16, checked = 32, focused = 64
    static let expands = 128, checks = 256, toggles = 512, live = 1024
}

/// What a surface gives the bridge: its committed rows and its press.
@MainActor protocol KitAccessSource: AnyObject {
    /// Changes when any committed row does.
    var accessStamp: Int32 { get }
    /// 0 the page; a higher number for what covers it (a sheet, a sidebar, a card).
    var accessLayer: Int { get }
    var accessCount: Int { get }
    func accessRow(_ i: Int) -> KitRow?
    /// The surface's own press on a row, as a tap's. True when it did something.
    func accessActivate(_ id: String) -> Bool
}

@MainActor final class KitAccess {
    /// What the host stands for a row: nil for the bridge's own element, `.skip` to leave it out.
    enum Stand { case own, skip, view(NSObject) }
    private weak var container: UIView?
    private unowned let source: KitAccessSource
    /// The host's own element for a row, if any (the platform editor, a system control).
    var stand: (KitRow) -> Stand = { _ in .own }
    /// VoiceOver's three-finger scroll on an element: true when the surface moved.
    var scroll: (UIAccessibilityScrollDirection) -> Bool = { _ in false }
    /// VoiceOver's escape gesture (two fingers in a Z): true when something closed.
    var escape: () -> Bool = { false }
    /// Debug builds: called with each announcement, so a check can read what VoiceOver would say.
    var announced: (String) -> Void = { _ in }
    private var stamp: Int32 = -1
    private var layer = -1
    private var shown: [String] = []
    private var cache: [String: KitElement] = [:]
    private(set) var elements: [Any] = []
    // live rows: what was announced of each, and what waits to be
    private var said: [String: String] = [:]
    private var waiting: [String: (text: String, since: CFTimeInterval)] = [:]
    private var started = false
    private var timer = false
    /// A live row is announced once it has said the same this long, in seconds.
    static let still: CFTimeInterval = 0.8

    init(container: UIView, source: KitAccessSource) {
        self.container = container; self.source = source
    }

    /// After each frame: builds the elements again when any row changed.
    func refresh() {
        let now = source.accessStamp, top = source.accessLayer
        guard now != stamp || top != layer, let container else { return }
        stamp = now
        var rows: [KitRow] = []
        for i in 0..<source.accessCount { if let row = source.accessRow(i) { rows.append(row) } }
        let order = KitAccess.reading(rows)
        var next: [Any] = [], kept: [String: KitElement] = [:]
        for row in order {
            switch stand(row) {
            case .skip: continue
            case .view(let view): next.append(view); continue
            case .own: break
            }
            guard KitAccess.read(row) else { continue }
            let element = cache[row.id] ?? KitElement(accessibilityContainer: container)
            element.bridge = self
            element.fill(row)
            kept[row.id] = element; next.append(element)
        }
        let ids = order.map { $0.id }, grew = ids != shown
        cache = kept; elements = next; shown = ids
        container.accessibilityElements = next
        let moved = top != layer && layer >= 0
        layer = top
        if moved { UIAccessibility.post(notification: .screenChanged, argument: next.first) }
        else if grew {
            // rows came or went: VoiceOver stays on the element it reads, when it is still there
            let at = UIAccessibility.focusedElement(using: .notificationVoiceOver) as? NSObject
            UIAccessibility.post(notification: .layoutChanged, argument: next.contains { ($0 as? NSObject) === at } ? at : nil)
        }
        live(rows)
    }

    /// Reading order: top to bottom, and left to right along a line. Rows whose tops are within a few
    /// points are on one line; a parent comes before its children.
    static func reading(_ rows: [KitRow]) -> [KitRow] {
        let band: CGFloat = 4
        return rows.enumerated().sorted { a, b in
            let ay = a.element.frame.minY, by = b.element.frame.minY
            if abs(ay - by) > band { return ay < by }
            let ax = a.element.frame.minX, bx = b.element.frame.minX
            if abs(ax - bx) > 0.5 { return ax < bx }
            return a.offset < b.offset
        }.map { $0.element }
    }

    /// Whether a row is read: some of it shows, and it says something. A region is a container of
    /// rows read on their own; a field is read even while empty.
    static func read(_ row: KitRow) -> Bool {
        guard row.frame.width > 0, row.frame.height > 0 else { return false }
        if row.role == KitRole.region { return false }
        return !row.label.isEmpty || !row.value.isEmpty || row.role == KitRole.field
    }

    // MARK: announcements

    private func live(_ rows: [KitRow]) {
        let clock = CACurrentMediaTime()
        // several live rows new at once are history arriving (a conversation loaded or rejoined):
        // they are read where they stand, not announced
        let fresh = rows.filter { $0.state & KitState.live != 0 && $0.state & KitState.busy == 0 && said[$0.id] == nil && waiting[$0.id] == nil }.count
        if fresh > 1 { started = false }
        for row in rows where row.state & KitState.live != 0 {
            let text = row.role == KitRole.field ? row.value : row.label
            if !started { said[row.id] = text; continue }
            if said[row.id] == text || text.isEmpty { waiting[row.id] = nil; continue }
            if waiting[row.id]?.text != text || row.state & KitState.busy != 0 { waiting[row.id] = (text, clock) }
        }
        started = true
        settle()
    }
    /// Announces the newest live row that has been still long enough; checks again later while any waits.
    private func settle() {
        let clock = CACurrentMediaTime()
        var ready: [String] = []
        for (id, w) in waiting where clock - w.since >= KitAccess.still { ready.append(id) }
        if !ready.isEmpty {
            // the row lowest on the page is the newest: one announcement, not one per row
            let byPlace = elements.compactMap { ($0 as? KitElement)?.rowID }
            let last = ready.max { (byPlace.firstIndex(of: $0) ?? -1) < (byPlace.firstIndex(of: $1) ?? -1) }!
            for id in ready {
                guard let w = waiting.removeValue(forKey: id) else { continue }
                let before = said[id] ?? ""
                said[id] = w.text
                if id == last {
                    // a message that grew is read from where the last announcement stopped
                    let words = !before.isEmpty && w.text.hasPrefix(before) ? String(w.text.dropFirst(before.count)).trimmingCharacters(in: .whitespacesAndNewlines) : w.text
                    if !words.isEmpty {
                        UIAccessibility.post(notification: .announcement, argument: NSAttributedString(string: words, attributes: [.accessibilitySpeechQueueAnnouncement: true]))
                        announced(words)
                    }
                }
            }
        }
        guard !waiting.isEmpty, !timer else { return }
        timer = true
        DispatchQueue.main.asyncAfter(deadline: .now() + KitAccess.still / 2) { [weak self] in
            guard let self else { return }
            self.timer = false
            // a row still busy waits on
            self.settle()
        }
    }

    fileprivate func activate(_ id: String) -> Bool { source.accessActivate(id) }

    #if DEBUG
    /// Debug builds: the elements as lines (`label, traits, value, frame`), for a launch argument to
    /// print, so a check can read what VoiceOver reads without a hand on the simulator.
    func dump() -> String {
        var out: [String] = []
        for e in elements {
            guard let o = e as? NSObject else { continue }
            let id = (o as? UIView)?.accessibilityIdentifier ?? (o as? UIAccessibilityElement)?.accessibilityIdentifier ?? ""
            var t: [String] = []
            let traits = o.accessibilityTraits
            if traits.contains(.button) { t.append("button") }
            if traits.contains(.header) { t.append("header") }
            if traits.contains(.link) { t.append("link") }
            if traits.contains(.staticText) { t.append("text") }
            if traits.contains(.notEnabled) { t.append("dimmed") }
            if traits.contains(.selected) { t.append("selected") }
            if traits.contains(.updatesFrequently) { t.append("updates") }
            if o is UITextInput { t.append("editor") }
            // in the container's points: an element's own, a view's frame
            let f = (o as? UIAccessibilityElement)?.accessibilityFrameInContainerSpace ?? (o as? UIView).map { v in container.map { v.convert(v.bounds, to: $0) } ?? v.frame } ?? .zero
            out.append("\(id) \"\(o.accessibilityLabel ?? "")\" [\(t.joined(separator: " "))] value \"\(o.accessibilityValue ?? "")\" at \(Int(f.minX)) \(Int(f.minY)) size \(Int(f.width))x\(Int(f.height))")
        }
        return out.joined(separator: "\n")
    }
    #endif
}

/// A committed row as VoiceOver sees it. Activation is the surface's own press.
final class KitElement: UIAccessibilityElement {
    fileprivate weak var bridge: KitAccess?
    private(set) var rowID = ""

    func fill(_ row: KitRow) {
        rowID = row.id
        accessibilityIdentifier = row.id
        accessibilityLabel = row.label
        accessibilityFrameInContainerSpace = row.frame
        var traits: UIAccessibilityTraits = []
        var value = row.value
        let s = row.state
        switch row.role {
        case KitRole.button, KitRole.row, KitRole.tab: traits.insert(.button)
        case KitRole.field: traits.insert(.button)        // a field the platform editor takes over when activated
        case KitRole.toggle: traits.insert(.button); if #available(iOS 17.0, *) { traits.insert(.toggleButton) }
        case KitRole.slider: traits.insert(.staticText)   // its value is read; no surface steps one yet
        case KitRole.heading: traits.insert(.header)
        case KitRole.link: traits.insert(.link)
        case KitRole.image: traits.insert(.image)
        case KitRole.progress: traits.insert(.updatesFrequently); traits.insert(.staticText)
        case KitRole.item: traits.insert(.staticText)
        default: traits.insert(.staticText)
        }
        if s & KitState.disabled != 0 { traits.insert(.notEnabled) }
        if s & KitState.current != 0 { traits.insert(.selected) }
        if s & KitState.checks != 0 {
            if s & KitState.checked != 0 { traits.insert(.selected) }
            if row.role != KitRole.toggle, value.isEmpty { value = s & KitState.checked != 0 ? "Checked" : "Not checked" }
        } else if s & KitState.checked != 0 { traits.insert(.selected) }
        if s & KitState.toggles != 0, s & KitState.pressed != 0 { traits.insert(.selected) }
        if s & KitState.expands != 0, value.isEmpty { value = s & KitState.expanded != 0 ? "Expanded" : "Collapsed" }
        if s & KitState.busy != 0, row.role != KitRole.progress, value.isEmpty { value = "In progress" }
        if row.role == KitRole.field { accessibilityHint = s & KitState.focused != 0 ? nil : "Double tap to type." }
        accessibilityValue = value.isEmpty ? nil : value
        accessibilityTraits = traits
    }
    override func accessibilityActivate() -> Bool { bridge?.activate(rowID) ?? false }
    override func accessibilityScroll(_ direction: UIAccessibilityScrollDirection) -> Bool { bridge?.scroll(direction) ?? false }
    override func accessibilityPerformEscape() -> Bool { bridge?.escape() ?? false }
}
