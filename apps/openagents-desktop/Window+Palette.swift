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
        case 19:
            overlay.watchPalette()
            listFiles()
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
    /// Edit > Find (Command-F): Settings search when Settings is open, else in-thread Find bar (#13876).
    func openFind() {
        if settingsOpen { findInSettings(); return }
        if overlay.dialogOpen { return }
        cd_find_open(page)
        focusFind()
    }
    func focusFind() { cd_focus(page, 5); window.makeFirstResponder(canvas); wake() }
    func findNext() { if cd_find_is_open(page) != 0 { _ = cd_find_step(page, 1); wake() } }
    func findPrev() { if cd_find_is_open(page) != 0 { _ = cd_find_step(page, -1); wake() } }
    var findOpen: Bool { cd_find_is_open(page) != 0 }
    /// cd_action 64: Command Palette Content mode full-text search across stored chats (#13876).
    func searchPaletteContent() {
        var qbuf = [CChar](repeating: 0, count: 512)
        guard cd_palette_query(page, &qbuf, Int32(qbuf.count)) > 0 else {
            cd_palette_content_hits(page, "")
            wake()
            return
        }
        let query = String(cString: qbuf)
        let storeRef = store
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            let hits = storeRef.searchContent(query: query, limit: 32)
            let payload = hits.map { hit in
                let cleanTitle = hit.title.replacingOccurrences(of: "\t", with: " ").replacingOccurrences(of: "\n", with: " ")
                let cleanSnip = hit.snippet.replacingOccurrences(of: "\t", with: " ").replacingOccurrences(of: "\n", with: " ")
                return "\(hit.chatId)\t\(cleanTitle)\t\(hit.turnIndex)\t\(cleanSnip)"
            }.joined(separator: "\n")
            DispatchQueue.main.async {
                guard let self, cd_palette_open(self.page) != 0, cd_palette_mode(self.page) == 2 else { return }
                var curBuf = [CChar](repeating: 0, count: 512)
                _ = cd_palette_query(self.page, &curBuf, Int32(curBuf.count))
                guard String(cString: curBuf) == query else { return }
                cd_palette_content_hits(self.page, payload)
                self.wake()
            }
        }
    }
    /// cd_action 65: jump to chat + turn chosen in Palette Content mode (#13876).
    func jumpToPaletteHit() {
        var cbuf = [CChar](repeating: 0, count: 128)
        let turn = cd_palette_jump_turn(page)
        guard cd_palette_jump_chat(page, &cbuf, Int32(cbuf.count)) > 0 else { return }
        let targetId = String(cString: cbuf)
        if chat != targetId {
            open(targetId)
        }
        if turn >= 0 {
            _ = cd_scroll_to_turn(page, turn)
        }
        wake()
    }
    /// Control-Tab and Control-Shift-Tab: the page's fields would take them as a tab, so they are
    /// heard before them. Under a dialog they wait. Also Cmd+1/2/3 switches palette mode when open.
    func installPaletteKeys() {
        guard paletteKeys[ObjectIdentifier(self)] == nil else { return }
        paletteKeys[ObjectIdentifier(self)] = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            guard let self, event.window === self.window else { return event }
            let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
            if cd_palette_open(self.page) != 0 && flags == .command, let ch = event.charactersIgnoringModifiers {
                if ch == "1" { cd_palette_set_mode(self.page, 0); self.wake(); return nil }
                if ch == "2" { cd_palette_set_mode(self.page, 1); self.wake(); return nil }
                if ch == "3" { cd_palette_set_mode(self.page, 2); self.wake(); return nil }
            }
            // Cmd+P: Quick Open file into the right-pane File Viewer (#13992).
            if flags == .command && event.charactersIgnoringModifiers?.lowercased() == "p" && (!self.overlay.dialogOpen || self.overlay.paletteUp) {
                if cd_palette_open(self.page) == 0 {
                    if cd_palette_toggle(self.page, 0) != 0 { self.overlay.watchPalette() }
                }
                cd_palette_set_mode(self.page, 1)
                self.listFiles()
                self.wake()
                return nil
            }
            if !self.overlay.dialogOpen {
                let ch = event.charactersIgnoringModifiers?.lowercased()
                // Cmd+W closes the active File Viewer tab without closing the window (#13992).
                if flags == .command && ch == "w" && cd_file_shown(self.page) != 0 && cd_file_mode(self.page) != 0 {
                    self.closeActiveFileTab()
                    return nil
                }
                // Cmd+Shift+[ / Cmd+Shift+] steps between open File Viewer tabs (#13992).
                if flags == [.command, .shift] && cd_file_shown(self.page) != 0 && (event.keyCode == 33 || event.keyCode == 30) {
                    let dir: Int32 = event.keyCode == 33 ? -1 : 1
                    if cd_file_tab_step(self.page, dir) != 0 { self.act(cd_action(self.page)) }
                    self.wake()
                    return nil
                }
                // Cmd+Shift+Enter toggles full-width expanded File Viewer mode (#13992).
                if flags == [.command, .shift] && cd_file_shown(self.page) != 0 && (event.keyCode == 36 || event.keyCode == 76) {
                    _ = cd_file_toggle_expand(self.page)
                    self.wake()
                    return nil
                }
                // Cmd+\ toggles hide/show of the right-hand File Viewer pane (#13992).
                if flags == .command && event.keyCode == 42 {
                    _ = cd_file_toggle_hide(self.page)
                    let a = cd_action(self.page)
                    if a != 0 { self.act(a) }
                    self.wake()
                    return nil
                }
                // Ctrl+G opens Go to Line inside the File Viewer (#13992).
                if flags == .control && ch == "g" && cd_file_shown(self.page) != 0 && cd_file_mode(self.page) != 0 {
                    cd_file_goto_open(self.page)
                    self.focusFind()
                    return nil
                }
            }
            guard event.keyCode == 48 else { return event }
            guard flags.contains(.control), !flags.contains(.command), !flags.contains(.option) else { return event }
            if self.overlay.dialogOpen && !self.overlay.paletteUp { return event }
            self.stepChat(flags.contains(.shift) ? -1 : 1)
            return nil
        }
    }
}
