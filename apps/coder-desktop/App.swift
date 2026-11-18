// Firm reason: 6 (AppKit lifecycle, the window and the platform editor over the compiled composer).
import AppKit

/// The window: the compiled composer fills it and docks its bar at the bottom.
/// The platform editor sits over the bar's text box and hands Coax each whole draft.
final class CoderWindow: NSObject {
    let window: NSWindow
    private let bar: UnsafeMutableRawPointer
    private let canvas: NSView
    private let editor: KitMacEditor
    private let line: CGFloat
    private var clock: Timer?
    private var owed = 3   // frames still to draw after a change

    init?(resources: String) {
        guard let bar = gc_open(resources), let raw = gc_view(bar) else { return nil }
        self.bar = bar
        canvas = Unmanaged<NSView>.fromOpaque(raw).takeUnretainedValue()
        var style = [Float](repeating: 0, count: 14); _ = gc_style(&style)
        editor = KitMacEditor(style: style); line = CGFloat(style[1])
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 900, height: 640),
                          styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
        super.init()
        window.title = "Coder"; window.minSize = NSSize(width: 420, height: 320); window.isReleasedWhenClosed = false
        canvas.frame = window.contentLayoutRect; canvas.autoresizingMask = [.width, .height]
        window.contentView = canvas
        // Send stays disabled: nothing receives a message yet.
        gc_set(bar, "coder.composer", "coder.composer.send", "Message", "", 0, 0, 0)
        gc_on_change(bar, { raw in guard let raw else { return }; Unmanaged<CoderWindow>.fromOpaque(raw).takeUnretainedValue().wake() },
                     Unmanaged.passUnretained(self).toOpaque())
        editor.committed = { [weak self] text in guard let self else { return }; _ = gc_set_text(self.bar, text); self.wake() }
        editor.setAccessibilityIdentifier("coder.composer"); editor.setAccessibilityLabel("Message")
        canvas.addSubview(editor)
        canvas.postsFrameChangedNotifications = true
        NotificationCenter.default.addObserver(forName: NSView.frameDidChangeNotification, object: canvas, queue: .main) { [weak self] _ in self?.wake() }
        let clock = Timer(timeInterval: 1.0 / 60.0, repeats: true) { [weak self] _ in self?.tick() }
        RunLoop.main.add(clock, forMode: .common); self.clock = clock
        window.center(); window.makeKeyAndOrderFront(nil); window.makeFirstResponder(editor)
        tick()
    }

    func wake() { owed = 3 }

    private func tick() {
        if gc_action(bar) == 2 { window.makeFirstResponder(editor) }
        let focused = window.firstResponder === editor
        var used = line
        if let layout = editor.layoutManager, let container = editor.textContainer {
            layout.ensureLayout(for: container); used = max(line, layout.usedRect(for: container).height)
        }
        // An empty draft leaves the text to the bar, so its placeholder shows under the caret.
        gc_env(bar, focused && !editor.string.isEmpty ? 1 : 0, Float(used), NSWorkspace.shared.accessibilityDisplayShouldReduceMotion ? 1 : 0)
        if gc_moving(bar) != 0 || owed > 0 { gc_frame(bar); if owed > 0 { owed -= 1 } }
        var box = [Float](repeating: 0, count: 6)
        guard gc_editor(bar, &box) != 0 else { editor.isHidden = true; return }
        editor.isHidden = false
        let frame = NSRect(x: CGFloat(box[0]), y: canvas.bounds.height - CGFloat(box[1] + box[3]), width: CGFloat(box[2]), height: CGFloat(box[3]))
        if editor.frame != frame { editor.frame = frame; wake() }
    }

    func retire() { clock?.invalidate(); clock = nil; gc_on_change(bar, nil, nil); gc_close(bar) }
}

final class CoderDelegate: NSObject, NSApplicationDelegate {
    var main: CoderWindow?
    func applicationDidFinishLaunching(_ notification: Notification) {
        guard let resources = Bundle.main.resourceURL?.appendingPathComponent("Components").path, let main = CoderWindow(resources: resources) else {
            fputs("Coder: the GPU surface could not start.\n", stderr); exit(1)
        }
        self.main = main
        NSApp.activate(ignoringOtherApps: true)
    }
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
    func applicationWillTerminate(_ notification: Notification) { main?.retire() }
}

@main struct CoderApp {
    static func main() {
        let app = NSApplication.shared; app.setActivationPolicy(.regular)
        let delegate = CoderDelegate(); app.delegate = delegate
        let bar = NSMenu(), appItem = NSMenuItem(), editItem = NSMenuItem()
        let appMenu = NSMenu(), edit = NSMenu(title: "Edit")
        appMenu.addItem(withTitle: "Hide Coder", action: #selector(NSApplication.hide(_:)), keyEquivalent: "h")
        appMenu.addItem(.separator())
        appMenu.addItem(withTitle: "Quit Coder", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        edit.addItem(withTitle: "Undo", action: Selector(("undo:")), keyEquivalent: "z")
        edit.addItem(withTitle: "Redo", action: Selector(("redo:")), keyEquivalent: "Z")
        edit.addItem(.separator())
        edit.addItem(withTitle: "Cut", action: #selector(NSText.cut(_:)), keyEquivalent: "x")
        edit.addItem(withTitle: "Copy", action: #selector(NSText.copy(_:)), keyEquivalent: "c")
        edit.addItem(withTitle: "Paste", action: #selector(NSText.paste(_:)), keyEquivalent: "v")
        edit.addItem(withTitle: "Select All", action: #selector(NSText.selectAll(_:)), keyEquivalent: "a")
        appItem.submenu = appMenu; editItem.submenu = edit; bar.addItem(appItem); bar.addItem(editItem)
        app.mainMenu = bar
        app.run()
    }
}
