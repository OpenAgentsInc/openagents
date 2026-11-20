// Firm reason: 6 (AppKit: the window's transparent title bar, its own buttons' places and moving the window).
// The window's title bar (GitHub #13741): the page runs to the window's top under it, and draws its controls.
import AppKit

/// The window's title bar as Zeron's (zgx `Titlebar::Inset`; the page draws the rest,
/// page/layout/titlebar.coax): the page runs under a transparent bar with no title or separator, and
/// the window's own buttons, the traffic lights, stand at Zeron's places, 14 points across centred
/// at x 21, 44 and 67 on the bar's line 20 points down. AppKit lays them out again as the window
/// resizes, becomes key or comes out of full screen, so a change of a button's frame puts it back.
/// A press on the bar where the page has nothing moves the window, and a double-click there zooms
/// or minimizes it as the system's setting says (Desktop & Dock, "Double-click a window's title bar").
final class CoderTitleBar {
    static let buttons: [NSWindow.ButtonType] = [.closeButton, .miniaturizeButton, .zoomButton]
    static let first: CGFloat = 21, pitch: CGFloat = 23, line: CGFloat = 20   // Zeron's, in points
    private weak var window: NSWindow?
    private weak var canvas: NSView?
    private let page: UnsafeMutableRawPointer
    private var observers: [NSObjectProtocol] = []
    private var monitor: Any?
    private var placing = false

    init(window: NSWindow, canvas: NSView, page: UnsafeMutableRawPointer) {
        self.window = window; self.canvas = canvas; self.page = page
        window.titlebarAppearsTransparent = true
        window.titleVisibility = .hidden
        window.titlebarSeparatorStyle = .none
        let center = NotificationCenter.default
        for type in CoderTitleBar.buttons {
            guard let button = window.standardWindowButton(type) else { continue }
            button.postsFrameChangedNotifications = true
            observers.append(center.addObserver(forName: NSView.frameDidChangeNotification, object: button, queue: .main) { [weak self] _ in self?.place() })
        }
        // full screen hides the lights (the page moves its toggle to the window's edge); out of it they come back to AppKit's places
        observers.append(center.addObserver(forName: NSWindow.willEnterFullScreenNotification, object: window, queue: .main) { [weak self] _ in
            guard let self else { return }; cd_bar_full(self.page, 1)
        })
        observers.append(center.addObserver(forName: NSWindow.didExitFullScreenNotification, object: window, queue: .main) { [weak self] _ in
            guard let self else { return }; cd_bar_full(self.page, 0); self.place()
        })
        for name in [NSWindow.didBecomeKeyNotification, NSWindow.didResignKeyNotification, NSWindow.didResizeNotification, NSWindow.didEndLiveResizeNotification] {
            observers.append(center.addObserver(forName: name, object: window, queue: .main) { [weak self] _ in self?.place() })
        }
        monitor = NSEvent.addLocalMonitorForEvents(matching: .leftMouseDown) { [weak self] event in
            self?.press(event) == true ? nil : event
        }
        place()
    }

    func stop() {
        observers.forEach { NotificationCenter.default.removeObserver($0) }; observers = []
        if let monitor { NSEvent.removeMonitor(monitor) }; monitor = nil
    }

    /// The traffic lights at Zeron's places, from the window's top left. None in full screen, where
    /// AppKit shows them in the menu bar's strip.
    func place() {
        guard !placing, let window, !window.styleMask.contains(.fullScreen) else { return }
        placing = true; defer { placing = false }
        let height = window.frame.height
        for (i, type) in CoderTitleBar.buttons.enumerated() {
            guard let button = window.standardWindowButton(type), let bar = button.superview else { continue }
            let size = button.frame.size
            let at = NSPoint(x: CoderTitleBar.first + CoderTitleBar.pitch * CGFloat(i) - size.width / 2,
                             y: height - CoderTitleBar.line - size.height / 2)
            let origin = bar.convert(at, from: nil)
            if button.frame.origin != origin { button.setFrameOrigin(origin) }
        }
    }

    /// A press in the window: on the bar where the page has nothing, it moves the window (true: the
    /// press is the window's, not the page's).
    private func press(_ event: NSEvent) -> Bool {
        guard let window, event.window === window, window.attachedSheet == nil, let canvas, let content = window.contentView else { return false }
        // the traffic lights, the file pane and the editors over the page keep their presses
        guard let frame = content.superview, frame.hitTest(frame.convert(event.locationInWindow, from: nil)) === canvas else { return false }
        let p = canvas.convert(event.locationInWindow, from: nil)
        let x = p.x, y = canvas.isFlipped ? p.y : canvas.bounds.height - p.y
        guard cd_bar_drags(page, Float(x), Float(y)) != 0 else { return false }
        if event.clickCount == 2 { doubleClick(window) } else { window.performDrag(with: event) }
        return true
    }

    /// A double-click on the bar, as the system's setting says: zoom (Fill and the older Maximize),
    /// minimize, or nothing.
    private func doubleClick(_ window: NSWindow) {
        let defaults = UserDefaults.standard
        let action = defaults.string(forKey: "AppleActionOnDoubleClick")
            ?? (defaults.bool(forKey: "AppleMiniaturizeOnDoubleClick") ? "Minimize" : "Maximize")
        switch action {
        case "Minimize": window.miniaturize(nil)
        case "None": break
        default: window.zoom(nil)
        }
    }
}
