// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// The command palette (⌘K, page/palette.coax, GitHub #13700), drawn by the page over itself with the
// page's own editor in its field (page/editor.coax), and the keys that go with it: ⌃Tab and ⌃⇧Tab
// for the next and previous chat, ⌘B for the sidebar and ⌘/ for the model menu.
import AppKit

private var paletteKeys: [ObjectIdentifier: Any] = [:]

extension CoderWindow {
    /// File > Search… (Command-K): the palette opens on All, or the one that is up closes.
    func togglePalette(scope: Int32 = 0) {
        if overlay.dialogOpen && !overlay.paletteUp { return }
        if cd_palette_toggle(page, scope) != 0 { overlay.watchPalette() }
        wake()
    }
    /// cd_action 26: the palette opened from the page (the sidebar's Search), or one of its rows
    /// asks what only the app does.
    func paletteAsked() {
        switch cd_palette_command(page) {
        case 0: overlay.watchPalette()
        case 16: renameShown()
        case 17: if canDelete { deleteShown() }
        case 18: stopAllChats()
        default: break
        }
    }
    /// View > Next Chat (Control-Tab) and Previous Chat (Control-Shift-Tab), in the sidebar's order.
    func stepChat(_ dir: Int32) {
        if cd_palette_open(page) != 0 { _ = cd_palette_toggle(page, 0) }
        if cd_chat_step(page, dir) != 0 { act(cd_action(page)) }
        wake()
    }
    /// View > Toggle Sidebar (Command-B).
    func toggleSidebar() { cd_toggle_sidebar(page); wake() }
    /// View > Choose Model (Command-/): the composer's model menu.
    func toggleModels() { cd_toggle_models(page); wake() }
    /// Control-Tab and Control-Shift-Tab: the page's fields would take them as a tab, so they are
    /// heard before them. Under a dialog they wait.
    func installPaletteKeys() {
        guard paletteKeys[ObjectIdentifier(self)] == nil else { return }
        paletteKeys[ObjectIdentifier(self)] = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            guard let self, event.window === self.window, event.keyCode == 48 else { return event }
            let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
            guard flags.contains(.control), !flags.contains(.command), !flags.contains(.option) else { return event }
            if self.overlay.dialogOpen && !self.overlay.paletteUp { return event }
            self.stepChat(flags.contains(.shift) ? -1 : 1)
            return nil
        }
    }
}
