// Firm reason: 6 (AppKit lifecycle, active editors and display-link scheduling over compiled Coax).
import AppKit
import QuartzCore

@MainActor final class GPUWindow: NSObject, NSTextFieldDelegate {
    private var library:AppChatLibraryNative?
    let owner: UnsafeMutableRawPointer
    let window: NSWindow
    let canvas: NSView
    let runtime: AppRuntime
    let identity: AppIdentityNative
    private let editor: KitMacEditor
    private let backup: KitMacEditor
    private let secret = KitMacSecure(frame: .zero)
    private var active: NSView?
    private var activeID = ""
    private var display: CADisplayLink?
    private var stepping = false
    private var retired = false
    private var watches: [NSObjectProtocol] = []
    private let store: AppViewStore
    private var saved: AppViewState
    private var attempted: AppViewState
    init(resources: String, hidden: Bool, auth: AppAuth, devices: AppDeviceService, store: AppViewStore) throws {
        let held = store.load(); self.store = store; saved = held; attempted = held
        guard let owner = oc_open_window(resources, 720, 850, hidden ? 1 : 0), let rawView = oc_view(owner), let rawWindow = oc_window(owner) else { throw AppAuth.Failure.said("The GPU surface could not start.") }
        self.owner = owner; canvas = Unmanaged<NSView>.fromOpaque(rawView).takeUnretainedValue(); window = Unmanaged<NSWindow>.fromOpaque(rawWindow).takeUnretainedValue()
        var style = [Float](repeating: 0, count: 14); _ = oc_style(&style)
        editor = KitMacEditor(style: style); backup = KitMacEditor(style: style); backup.privateValue = true; backup.isEditable = false
        identity = AppIdentityNative(auth: auth)
        guard let runtime = AppRuntime(auth: auth, platform: MacPlatform(), devices: devices, identity: identity) else { oc_close(owner); throw AppAuth.Failure.said("The app core could not start.") }
        self.runtime = runtime
        super.init()
        secret.isBordered = false; secret.drawsBackground = false; secret.textColor = editor.textColor; secret.font = editor.font; secret.delegate = self; secret.contentType = .password
        secret.setAccessibilityLabel("Private key"); secret.setAccessibilityIdentifier("identity.secret")
        identity.anchor = { [weak self] in self?.window }
        identity.input = { [weak self] in guard let self,self.activeID=="identity.secret" else {return ""};return self.secret.stringValue }
        identity.clearInput = { [weak self] in guard let self else {return};if self.activeID=="identity.secret" {self.secret.stringValue=""};self.backup.string="" }
        runtime.provider.input = { [weak self] in guard let self,self.activeID=="usage.code" else {return ""};return self.secret.stringValue }
        runtime.provider.clearInput = { [weak self] in guard let self,self.activeID=="usage.code" else {return};self.secret.stringValue="";oc_secret_ready(owner,0) }
        editor.committed = { [weak self] text in self?.commit(text) }
        editor.command = { [weak self] selector in self?.command(selector) ?? false }
        oc_attach(owner, runtime.core.raw, held.email)
        oc_env_restore(owner, held.environment)
        oc_native_scroll(owner, 1)
        library=AppChatLibraryNative(auth:auth,owner:owner,wake:{[weak self] in self?.wake()})
        runtime.core.after = { [weak self] in guard let self else { return }; oc_step(self.owner); self.keep(); self.wake() }
        runtime.bridge.changed = { [weak self] in self?.wake() }
        oc_on_change(owner, { raw in guard let raw else { return }; MainActor.assumeIsolated { Unmanaged<GPUWindow>.fromOpaque(raw).takeUnretainedValue().wake() } }, Unmanaged.passUnretained(self).toOpaque())
        if !hidden { display = canvas.displayLink(target: self, selector: #selector(tick)); display?.isPaused = true; display?.add(to: .main, forMode: .common) }
        let center = NotificationCenter.default
        func watch(_ name: Notification.Name, _ action: @escaping @MainActor () -> Void) { watches.append(center.addObserver(forName: name, object: nil, queue: .main) { _ in MainActor.assumeIsolated { action() } }) }
        watch(NSApplication.didBecomeActiveNotification) { [weak self] in self?.library?.accountChanged(); self?.runtime.bridge.lifecycle(foreground: true); self?.wake() }
        watch(NSApplication.didResignActiveNotification) { [weak self] in self?.library?.background(); self?.keep(); self?.clearPrivate(); self?.runtime.bridge.lifecycle(foreground: false) }
        watches.append(center.addObserver(forName: NSWindow.willCloseNotification, object: window, queue: .main) { [weak self] _ in MainActor.assumeIsolated { self?.retire() } })
        watch(AppAuth.didChangeSession) { [weak self] in self?.library?.accountChanged() }
        runtime.launch(rejoin: held.rejoin)
        if !hidden { AppDeviceLink.shared.onLink = { [weak self] link in guard let self, !self.retired else { return }; self.runtime.core.action(19, text: link); self.runtime.bridge.heard() } }
        step()
    }
    func wake() { guard !retired else { return }; display?.isPaused = false }
    @objc private func tick() { step() }
    func step() {
        guard !retired, !stepping else { return }; stepping = true; defer { stepping = false }
        let focused = active != nil
        oc_env(owner, 0, 0, 0, focused ? 1 : 0, 0, NSWorkspace.shared.accessibilityDisplayShouldReduceMotion ? 1 : 0)
        library?.restore()
        oc_frame(owner); library?.frame(); runtime.pump(); _ = oc_action(owner)
        followFocus()
        keep()
        if oc_moving(owner) == 0 { display?.isPaused = true }
    }
    private func read(_ get: (UnsafeMutableRawPointer, UnsafeMutablePointer<CChar>, Int32) -> Int32, cap: Int = 17000) -> String { var b = [CChar](repeating: 0, count: cap); _ = get(owner, &b, Int32(cap)); return String(cString: b) }
    private func semanticLabel(_ id: String) -> String {
        for index in 0..<oc_rows(owner) {
            var box = [Float](repeating: 0, count: 8), key = [CChar](repeating: 0, count: 65), label = [CChar](repeating: 0, count: 4097)
            let keyCapacity = Int32(key.count), labelCapacity = Int32(label.count)
            if oc_row(owner, index, &box, &key, keyCapacity, &label, labelCapacity) != 0, String(cString: key) == id { return String(cString: label) }
        }
        return ""
    }
    private func rect(_ b: [Float]) -> NSRect { NSRect(x: CGFloat(b[0]), y: canvas.bounds.height - CGFloat(b[1] + b[3]), width: CGFloat(b[2]), height: CGFloat(b[3])) }
    private func followFocus() { editor.synchronize { followFocusNow() } }
    private func followFocusNow() {
        let id = read(oc_focus, cap: 65), mode = oc_private_mode(owner)
        if mode == 0 { secret.stringValue = ""; backup.string = ""; identity.hideBackup(); oc_secret_ready(owner, 0) }
        let next: NSView?
        if id == "chat.composer" { next = editor; editor.contentType = nil }
        else if oc_field_kind(owner) >= 0 && oc_field_kind(owner) != 3 { next = editor; editor.contentType = oc_field_kind(owner) == 2 ? .oneTimeCode : oc_field_kind(owner) == 1 ? .emailAddress : nil }
        else if oc_field_kind(owner)==3 { next = mode == 2 ? backup : secret }
        else { next = nil }
        if active !== next || activeID != id {
            editor.discardComposition()
            if active != nil { window.makeFirstResponder(canvas); active?.removeFromSuperview() }
            if active === secret || next === secret {secret.stringValue=""}
            active = next; activeID = id
            if let next { canvas.addSubview(next); next.setAccessibilityIdentifier(id); next.setAccessibilityLabel(semanticLabel(id)) }
        }
        guard let next else { oc_access_editor(owner, "", nil); return }
        var b = [Float](repeating: 0, count: 6)
        let valid = id == "chat.composer" ? oc_editor(owner, &b) : oc_field_text_box(owner, &b)
        guard valid != 0 else { next.isHidden = true; oc_access_editor(owner, "", nil); return }
        next.isHidden = false
        next.frame = rect(b)
        if next === editor, !editor.hasMarkedText() {
            let text = read(id == "chat.composer" ? oc_text : oc_field_text)
            if editor.string != text { editor.string = text }
        }
        if next === backup { backup.string = identity.backup }
        oc_access_editor(owner, id, Unmanaged.passUnretained(next).toOpaque())
        if next !== backup, window.firstResponder !== next { window.makeFirstResponder(next) }
    }
    private func commit(_ text: String) {
        guard active === editor, activeID == read(oc_focus, cap: 65) else { return }
        if activeID == "chat.composer" { _ = oc_set_text(owner, text) } else { _ = oc_set_field(owner, text) }
        wake()
    }
    private func command(_ selector: Selector) -> Bool {
        if selector == #selector(NSResponder.insertTab(_:)) || selector == #selector(NSResponder.insertBacktab(_:)) { _ = drive(selector == #selector(NSResponder.insertBacktab(_:)) ? "ax-key shift-tab" : "ax-key tab"); return true }
        if selector == #selector(NSResponder.cancelOperation(_:)) { oc_blur(owner); wake(); return true }
        if selector == #selector(NSResponder.insertNewline(_:)), activeID != "chat.composer", oc_field_kind(owner) != 4 { _ = oc_submit(owner); wake(); return true }
        return false
    }
    func controlTextDidChange(_ notification: Notification) { guard active === secret else { return }; oc_secret_ready(owner, secret.stringValue.isEmpty ? 0 : 1); wake() }
    private func clearPrivate() { identity.clear(); runtime.provider.clear(); secret.stringValue=""; oc_secret_ready(owner, 0); if oc_field_kind(owner)==3 { oc_blur(owner) }; wake() }
    private func keep(force: Bool = false) {
        let next = AppViewState(email: read(oc_draft, cap: 600), rejoin: runtime.core.keep(), environment: read(oc_env_save, cap: 8192))
        guard next != saved, force || next != attempted else { return }
        attempted = next
        if store.save(next) { saved = next }
    }
    func retire() { guard !retired else { return }; library?.background();keep(force: true); clearPrivate(); retired = true; display?.invalidate(); display = nil; runtime.retire(); oc_on_change(owner, nil, nil); oc_access_editor(owner, "", nil); for w in watches { NotificationCenter.default.removeObserver(w) }; watches = [] }
    func drive(_ op: String) -> String {
        if op.hasPrefix("tap ") { _ = oc_activate(owner, String(op.dropFirst(4))); step(); return "" }
        var out = [CChar](repeating: 0, count: 65536); _ = oc_drive(owner, op, &out, Int32(out.count)); step(); return String(cString: out)
    }
    /// SDK-only observer for hidden integration checks. Product scheduling
    /// remains the core's sleeping effect timer and paused display link.
    func inspectUntil(_ ready: () -> Bool) async throws {
        let deadline = ContinuousClock.now.advanced(by: .seconds(60))
        while !ready() {
            guard ContinuousClock.now < deadline else { throw AppAuth.Failure.said("The hidden SDK check reached its deadline.") }
            step()
            try await Task.sleep(for: .milliseconds(50))
        }
    }
    /// Real SDK qualification: no window is shown and no message or login request is sent.
    func inspectControls() throws {
        _ = drive("tap nav.menu"); _ = drive("clock +1000"); _ = drive("tap drawer.settings"); _ = drive("clock +1000")
        guard drive("ax").contains("settings.title AXHeading") else { throw AppAuth.Failure.said("GPU Settings accessibility is missing.") }
        _ = drive("tap settings.login"); _ = drive("clock +1000"); _ = drive("tap login.email"); _ = drive("clock +17")
        guard active === editor, activeID == "login.email", editor.contentType == .emailAddress else { throw AppAuth.Failure.said("The active platform email editor is missing.") }
        editor.insertText("fixture@example.test", replacementRange: NSRange(location: 0, length: (editor.string as NSString).length))
        guard read(oc_field_text) == "fixture@example.test", editor.string == "fixture@example.test" else { throw AppAuth.Failure.said("Committed email did not reach Coax: editor bytes \(editor.string.utf8.count), core bytes \(read(oc_field_text).utf8.count), editable \(editor.isEditable), marked \(editor.hasMarkedText()), field \(activeID).") }
        let committedBeforeMark = read(oc_field_text)
        editor.setMarkedText("に", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: 0, length: 0)); editor.didChangeText()
        guard read(oc_field_text) == committedBeforeMark else { throw AppAuth.Failure.said("An IME composition escaped the SDK editor.") }
        editor.insertText("日本", replacementRange: NSRange(location: NSNotFound, length: 0)); editor.didChangeText()
        guard read(oc_field_text) == editor.string, !editor.hasMarkedText() else { throw AppAuth.Failure.said("IME commit did not reach Coax.") }
        let committed = read(oc_field_text), callback = editor.committed
        var programmaticEdits = 0
        editor.committed = { _ in programmaticEdits += 1 }
        editor.synchronize { editor.insertText("SDK-only synchronization", replacementRange: NSRange(location: 0, length: (editor.string as NSString).length)); editor.discardComposition(); editor.string = committed }
        editor.committed = callback
        guard programmaticEdits == 0, read(oc_field_text) == committed else { throw AppAuth.Failure.said("Programmatic editor synchronization emitted a user edit.") }
        let ax = drive("ax")
        guard ax.components(separatedBy: "login.email AX").count == 2, ax.contains("login.email AXTextArea") else { throw AppAuth.Failure.said("The active field is missing, duplicated or lacks its text role in accessibility.") }
        oc_blur(owner); step()
        guard active == nil else { throw AppAuth.Failure.said("The platform editor did not detach after blur.") }
        print("Mac GPU: shared page/core, Settings/login, one editor, marked/committed text, accessibility replacement and blur passed")
        retire()
    }
    deinit { oc_close(owner) }
}

@MainActor final class MacPlatform: AppPlatform {
    func copy(_ text: String) -> Bool { NSPasteboard.general.clearContents(); return NSPasteboard.general.setString(text, forType: .string) }
    func open(_ link: String, done: @escaping (Bool) -> Void) { guard let url = URL(string: link), ["https", "http"].contains(url.scheme?.lowercased() ?? ""), url.host?.isEmpty == false else { done(false); return }; done(NSWorkspace.shared.open(url)) }
}
